//! Gas holes, packed the way Fidenza packs anything: largest first, collision
//! tested against what is already down, dropped rather than shrunk.
//!
//! Measured in the artwork: 7 holes, 19.8% of the silhouette, equivalent
//! diameters 80.8 down to 26.2 (only a 3.08x span), minimum gap between two of
//! them 17.4 units, minimum clearance to the inner edge of the ribbon 7.6.
//! The eye reads a much wider size range than 3x because the big ones are
//! ELONGATED (circularity 0.41-0.59, 5-8 concave lobes) while the small ones
//! are compact and smooth (0.70-0.87, 2-3 lobes). That correlation, not a
//! steep radius law, is what this module reproduces: aspect, amplitude, lobe
//! frequency and octave count all scale with size.

use crate::bezier::{catmull_rom_closed, clamp_handles, Cubic};
use crate::field::Field;
use crate::math::{closed_len, cos, polygon_area, sin_cos, PI, TAU, V2};
use crate::noise::{Fbm, Perlin};
use crate::params::min_slope_limit;
use crate::ring::Ring;
use crate::rng::Pcg32;
use crate::Params;

/// Samples of a vesicle's own `r(theta)`. 72 is one every 5 degrees, which
/// resolves 8 lobes with room to spare and keeps a rejected candidate cheap.
const VES_SAMPLES: usize = 72;
/// Every third sample is a clearance probe, giving SPEC.md's M = 24.
const PROBE_STRIDE: usize = 3;
/// Half-width of the narrowphase radius window, in samples of 9 degrees.
/// A lumpy blob is not convex, so the centre-line radius alone can miss a side
/// lobe; +-18 degrees is conservative, and conservative can only over-reject.
const WINDOW_HALF: i32 = 2;
/// 9 degrees, the window stride.
const WINDOW_STEP: f64 = PI / 20.0;
/// Amplitude multiplier for the smallest vesicle.
const AMP_SMALL: f64 = 0.55;
/// Amplitude multiplier for the largest. Measured: the three biggest holes are
/// strongly re-entrant, the four smallest nearly convex.
///
/// Public because the tameness dial has to size the holes against the same
/// reach the packer tests them with; see `Params::apply_tameness`.
pub const AMP_BIG: f64 = 1.25;
/// Lobes round the smallest vesicle (measured 2-3 concave runs).
const LOBES_SMALL: f64 = 2.2;
/// Lobes round the largest (measured 5-8 concave runs).
const LOBES_BIG: f64 = 4.4;
/// Size fraction below which no bite is cut. Measured: the four smallest
/// holes are near-convex (circularity 0.70-0.87), so only the big ones are
/// re-entrant and a bite in a small hole just reads as a chip.
const BITE_SPLIT: f64 = 0.45;
/// Half-width of the bite's angular window, in radians.
///
/// Narrow is the whole point. A wide window (the first thing I tried, at
/// 0.75-1.15 rad) does not bite - it scales one entire flank down smoothly
/// and leaves a teardrop, which measures as MORE circular than the unbitten
/// blob, not less. Concavity needs the notch to be deep relative to its
/// width: at 0.38-0.62 rad (22-36 degrees) the boundary genuinely turns back
/// on itself.
const BITE_W_LO: f64 = 0.38;
const BITE_W_HI: f64 = 0.62;
/// Slope budget a bitten hole is given, overriding the body's.
///
/// The limiter caps |d(ln r)/d(theta)| at kappa, so a notch of half-width w
/// can only reach exp(-kappa*w) of its shoulders: at the body's 1.10 over
/// 0.5 rad that is 0.58, and the limiter spends the rest widening the notch
/// back into the teardrop. 2.10 lets a 0.45-deep bite stand; the smoothing
/// passes that follow round the corner it leaves, which is what the measured
/// holes look like - deeply re-entrant and still smooth.
const BITE_KAPPA: f64 = 2.10;
/// Most bites a single hole can carry. The measured big holes show five and
/// eight separate concave runs; the fbm lobes supply the shallow ones, and
/// these supply the deep ones that make the shape read as bitten. Two is
/// where a hole stops being a kidney and starts being a starfish - three
/// roughly evenly spaced bites make a rounded triangle, which measures back
/// up near the unbitten circularity.
const BITE_MAX: usize = 2;
/// How far a bite may wander off the short axis, in radians.
///
/// Direction is not free. A bite into the END of an elongated hole only
/// makes it shorter; a bite into its SIDE makes the crescent the artwork
/// actually has. So aim across the long axis and jitter by this much.
const BITE_AIM_JITTER: f64 = 0.55;
/// Below this size fraction a vesicle loses an octave: a fourth octave's
/// detail is under a unit across on the smallest hole.
const OCT_SPLIT: f64 = 0.45;
/// Target chord of an emitted vesicle segment, as a fraction of R0. Measured
/// median chord 13.4 units on R0 = 169.2, i.e. 0.079.
const SEG_FRAC: f64 = 0.08;
/// Binomial smoothing passes over a vesicle's ring after its limiter.
///
/// At 72 samples a five-tap pass is a 20-degree window: it leaves the two to
/// eight measured lobes alone (under 10% off at k=5) and takes the top off
/// the fine detail the limiter leaves as corners. The artwork's holes are
/// smooth all-cubic blobs with no sharp corner anywhere - median deviation
/// from chord 6.6%, longest chord 53 units - so a spiky hole is wrong even
/// when its lobe count is right.
const VESICLE_SMOOTH_PASSES: usize = 2;
/// Lower bound on a vesicle's own radius, as a fraction of its mean, applied
/// before the limiter. The health gate asserts 0.35; 0.36 leaves it room.
const PINCH_FLOOR: f64 = 0.36;
/// How much of the pinch floor a full-depth bite is allowed to spend. At the
/// default bite of 0.50 the floor drops to 0.22 of the mean, which is the
/// depth the measured kidney reaches; at 1.0 it would let a hole close up.
const BITE_FLOOR_RELIEF: f64 = 0.78;

/// Linear interpolation.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// One binomial pass over a wrapped radius array. Same filter the silhouette
/// uses, for the same reason: the limiter leaves C0 kinks where it bites.
fn smooth_ring(r: &mut [f64]) {
    let n = r.len() as i64;
    if n < 5 {
        return;
    }
    let v = r.to_owned();
    for (i, out) in r.iter_mut().enumerate() {
        let g = |k: i64| v[((i as i64 + k).rem_euclid(n)) as usize];
        *out = (g(-2) + 4.0 * g(-1) + 6.0 * g(0) + 4.0 * g(1) + g(2)) / 16.0;
    }
}

/// A placed gas hole.
#[derive(Clone, Debug)]
pub struct Vesicle {
    /// Centre in canonical units.
    pub centre: V2,
    /// Equivalent radius it was drawn for.
    pub r_eq: f64,
    /// Elongation of its base ellipse.
    pub aspect: f64,
    /// Unit long axis, mostly the flow field at `centre`.
    pub axis: V2,
    /// Its own star-shaped ring; the packer's radius lookups go through this.
    pub ring: Ring,
    /// The emitted boundary, uniform arclength.
    pub pts: Vec<V2>,
    /// The emitted outline.
    pub cubics: Vec<Cubic>,
    /// The floor its own radius was clamped to, as a fraction of its mean.
    ///
    /// A bitten hole legitimately dips below the unbitten floor, so the health
    /// gate has to check each shape against the floor it was built with rather
    /// than one constant.
    pub pinch_floor: f64,
    /// Area of `pts`.
    pub area: f64,
    /// Handles the Bezier clamp caught here.
    pub clamped: usize,
}

/// A candidate mid-packing: everything the collision tests need, and nothing
/// that costs anything to throw away.
struct Cand {
    centre: V2,
    r_eq: f64,
    aspect: f64,
    axis: V2,
    ring: Ring,
    /// Smallest radius the shape can possibly have; used by the cheap
    /// definitely-overlapping rejection before the ring is built.
    r_low: f64,
    /// The floor `clamp_min` used, as a fraction of the mean radius.
    pinch_floor: f64,
}

/// Uniform grid broadphase. `Vec<Vec<u16>>` by instruction: a HashMap would
/// iterate in an order that changes between runs.
struct Grid {
    lo: V2,
    sx: f64,
    sy: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u16>>,
}

impl Grid {
    /// Cover `lo`..`hi` with cells of about `cell` units.
    fn new(lo: V2, hi: V2, cell: f64) -> Grid {
        let cell = cell.max(1e-6);
        let span_x = (hi.x - lo.x).max(1e-9);
        let span_y = (hi.y - lo.y).max(1e-9);
        let nx = ((span_x / cell).ceil() as i64).clamp(1, 128) as usize;
        let ny = ((span_y / cell).ceil() as i64).clamp(1, 128) as usize;
        Grid {
            lo,
            sx: span_x / nx as f64,
            sy: span_y / ny as f64,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
        }
    }

    /// Cell range of a box, clamped into the grid.
    fn range(&self, lo: V2, hi: V2) -> (usize, usize, usize, usize) {
        let fx = |v: f64| {
            ((v - self.lo.x) / self.sx)
                .floor()
                .clamp(0.0, (self.nx - 1) as f64) as usize
        };
        let fy = |v: f64| {
            ((v - self.lo.y) / self.sy)
                .floor()
                .clamp(0.0, (self.ny - 1) as f64) as usize
        };
        (fx(lo.x), fx(hi.x), fy(lo.y), fy(hi.y))
    }

    /// Record `idx` in every cell its box touches. Duplicates across cells are
    /// fine: the narrowphase is idempotent.
    fn insert(&mut self, idx: u16, lo: V2, hi: V2) {
        let (x0, x1, y0, y1) = self.range(lo, hi);
        let stride = self.nx;
        for iy in y0..=y1 {
            for ix in x0..=x1 {
                self.cells[iy * stride + ix].push(idx);
            }
        }
    }

    /// Everything that could be near the box, deduplicated.
    fn query(&self, lo: V2, hi: V2, out: &mut Vec<u16>) {
        out.clear();
        let (x0, x1, y0, y1) = self.range(lo, hi);
        for iy in y0..=y1 {
            for ix in x0..=x1 {
                out.extend_from_slice(&self.cells[iy * self.nx + ix]);
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

/// Pack up to `p.vesicle_count` holes into the body.
///
/// Clearance is measured against the INNER ring - the orange boundary - not
/// the silhouette: a hole placed against the outer edge bites into the ribbon
/// and the logo looks chewed. The floor is tied to `outline_weight`, so
/// thickening the outline pushes the holes in rather than silently breaking
/// the composition.
pub fn pack(
    p: &Params,
    inner: &Ring,
    field: &Field,
    perlin: &Perlin,
    rng: &mut Pcg32,
    c: V2,
    r0: f64,
) -> Vec<Vesicle> {
    let count = p.vesicle_count as usize;
    if count == 0 {
        return Vec::new();
    }
    let rmin = p.vesicle_rmin * r0;
    let rmax = p.vesicle_rmax * r0;
    let span = (rmax - rmin).max(1e-12);
    // Measured tightest clearance to the inner edge: 11.3/169.2 = 0.067 R0,
    // and 1.2*0.058 = 0.070 - the ribbon-derived floor is the binding one at
    // the defaults, which is the right coupling.
    let clearance = (1.2 * p.outline_weight * r0).max(p.vesicle_edge_clear * r0);

    // Radii first, then aspects, in one fixed order: the draw order must not
    // depend on how the packing goes, or a dropped vesicle would reshuffle
    // every later size.
    let mut radii = Vec::with_capacity(count);
    for k in 0..count {
        // Stratified, not independent: one draw per 1/count band of the range.
        // Seven independent uniforms land within 2x of each other about a
        // third of the time, which is the polka-dot failure the spread gate
        // then has to reject; the artwork's seven span 3.08x with a clear
        // two-big / three-mid / two-small hierarchy, and stratifying
        // reproduces that by construction instead of by re-rolling for it.
        let u = (k as f64 + rng.f64()) / count as f64;
        // u^pow by multiplication - powf is a libm call and a determinism
        // hazard. pow 2 biases toward small, matching the measured clustering.
        let mut up = 1.0;
        for _ in 0..p.vesicle_size_pow {
            up *= u;
        }
        radii.push(rmin + (rmax - rmin) * up);
    }
    let mut aspects = Vec::with_capacity(count);
    for &r in &radii {
        let t = ((r - rmin) / span).clamp(0.0, 1.0);
        aspects.push(1.0 + (p.vesicle_aspect_max - 1.0) * t * rng.range(0.6, 1.0));
    }

    // Largest first. This alone lifts achieved coverage from about 12% to the
    // measured 21%, with no relaxation pass.
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| radii[b].total_cmp(&radii[a]).then(a.cmp(&b)));

    let ipts = inner.points();
    let (lo, hi) = bbox(&ipts);
    let mut grid = Grid::new(lo, hi, 2.0 * rmax);
    // The largest radius any vesicle could reach, for the query box.
    let reach = rmax * p.vesicle_aspect_max.sqrt() * (1.0 + p.vesicle_amp * AMP_BIG);
    let gap_max = (p.vesicle_gap * r0).max(0.45 * rmax);

    let mut placed: Vec<Cand> = Vec::with_capacity(count);
    let mut near: Vec<u16> = Vec::with_capacity(16);
    let mut total = 0u32;
    let mut consec = 0u32;

    'next: for &k in &order {
        let r_k = radii[k];
        let asp = aspects[k];
        let t = ((r_k - rmin) / span).clamp(0.0, 1.0);
        let amp = p.vesicle_amp * lerp(AMP_SMALL, AMP_BIG, t);
        let semi_a = r_k * asp.sqrt();
        let semi_b = r_k / asp.sqrt();
        let r_low = semi_b * (1.0 - amp);
        let r_high = semi_a * (1.0 + amp);

        for _ in 0..p.vesicle_attempts {
            if total >= p.attempts_total || consec >= p.consec_fail_stop {
                break 'next;
            }
            total += 1;
            let centre = V2::new(rng.range(lo.x, hi.x), rng.range(lo.y, hi.y));

            // Necessary condition, O(1) against the star-shaped inner ring:
            // the nearest boundary point of the candidate is at least r_low
            // away from its centre, so the centre must clear by that much.
            if !inner.contains(centre, clearance + r_low) {
                consec += 1;
                continue;
            }

            let q = V2::new(r_high + reach + gap_max, r_high + reach + gap_max);
            grid.query(centre - q, centre + q, &mut near);

            // Cheap definite rejection: if even the minimum radii overlap, no
            // shape this centre could produce would fit.
            let mut doomed = false;
            for &j in &near {
                let other = &placed[j as usize];
                let gap = (p.vesicle_gap * r0).max(0.45 * r_k.min(other.r_eq));
                if (other.centre - centre).len() < other.r_low + r_low + gap {
                    doomed = true;
                    break;
                }
            }
            if doomed {
                consec += 1;
                continue;
            }

            // The shape depends on the centre - the long axis follows the
            // field there - so it has to be built before it can be tested.
            let cand = build_cand(p, field, perlin, rng, c, r0, centre, r_k, asp, t);

            if !boundary_clear(&cand.ring, inner, clearance) {
                consec += 1;
                continue;
            }
            let mut clash = false;
            for &j in &near {
                let other = &placed[j as usize];
                let gap = (p.vesicle_gap * r0).max(0.45 * cand.r_eq.min(other.r_eq));
                if !separated(&cand, other, gap) {
                    clash = true;
                    break;
                }
            }
            if clash {
                consec += 1;
                continue;
            }

            let bl = V2::new(centre.x - r_high, centre.y - r_high);
            let br = V2::new(centre.x + r_high, centre.y + r_high);
            grid.insert(placed.len() as u16, bl, br);
            placed.push(cand);
            consec = 0;
            continue 'next;
        }
        // Out of attempts for this radius: DROP it. A shrunk-to-fit hole reads
        // as dirt; a missing one reads as composition.
    }

    placed.into_iter().map(|c| finish(c, p, r0)).collect()
}

/// Build one candidate's star-shaped outline about `centre`.
#[allow(clippy::too_many_arguments)]
fn build_cand(
    p: &Params,
    field: &Field,
    perlin: &Perlin,
    rng: &mut Pcg32,
    c: V2,
    r0: f64,
    centre: V2,
    r_eq: f64,
    aspect: f64,
    t: f64,
) -> Cand {
    let amp = p.vesicle_amp * lerp(AMP_SMALL, AMP_BIG, t);
    let octaves = if t < OCT_SPLIT {
        p.vesicle_octaves.saturating_sub(1).max(2)
    } else {
        p.vesicle_octaves
    };
    // Lobes per turn, converted to a noise-space circle radius: a circle of
    // radius f meets about PI*f Perlin periods on the way round. See
    // `body::PeriodicFbm::at`.
    let freq = lerp(LOBES_SMALL, LOBES_BIG, t) / PI;
    let fbm = Fbm::new(rng, octaves, p.lacunarity, p.gain);
    // Its own noise offset, so two vesicles of the same size differ.
    let off = V2::new(rng.range(-4096.0, 4096.0), rng.range(-4096.0, 4096.0));

    let fdir = field.dir((centre - c) * (1.0 / r0));
    let (s, cs) = sin_cos(rng.range(0.0, TAU));
    let mut rdir = V2::new(cs, s);
    // A long axis is a LINE, not an arrow. Flip the random direction into the
    // field's half-plane before blending, or two nearly opposite unit vectors
    // cancel and the axis spins out.
    if rdir.dot(fdir) < 0.0 {
        rdir = -rdir;
    }

    let blended = fdir * p.vesicle_align + rdir * (1.0 - p.vesicle_align);
    let axis = if blended.len2() < 1e-18 {
        fdir
    } else {
        blended.norm()
    };
    let perp = axis.perp();

    // Ramped in across BITE_SPLIT..1, so the small holes stay convex.
    let bite_t = if t <= BITE_SPLIT {
        0.0
    } else {
        (t - BITE_SPLIT) / (1.0 - BITE_SPLIT)
    };
    let bite = p.vesicle_bite * bite_t;
    // How many: one by the time a hole is worth biting, three at full size.
    let n_bites = if bite <= 0.0 {
        0
    } else {
        (1.0 + (BITE_MAX - 1) as f64 * bite_t).round() as usize
    };
    // Always drawn, all three, whatever the size: the stream must stay in
    // step or changing one hole's size reshuffles every later one.
    let mut bites = [(V2::ZERO, 0.0_f64); BITE_MAX];
    for (i, b) in bites.iter_mut().enumerate() {
        // Across the short axis, alternating sides so two bites face each
        // other into a waist rather than landing on top of one another.
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let (js, jc) = sin_cos(rng.range(-BITE_AIM_JITTER, BITE_AIM_JITTER));
        let aim = perp * side;
        let dir = V2::new(aim.x * jc - aim.y * js, aim.x * js + aim.y * jc);
        // The window is built from the dot product against cos(half-width),
        // so a bite costs no arccos and stays C1 - a corner here would read
        // as a nick rather than as a bite.
        *b = (dir, cos(rng.range(BITE_W_LO, BITE_W_HI)));
    }
    let bites = &bites[..n_bites];

    let semi_a = r_eq * aspect.sqrt();
    let semi_b = r_eq / aspect.sqrt();
    let mut ring = Ring::build(centre, VES_SAMPLES, |_theta, d| {
        // In the vesicle's own frame: (cl, cp) walks a unit circle as theta
        // goes round, so the noise lookup is exactly periodic - no seam.
        let cl = d.dot(axis);
        let cp = d.dot(perp);
        let rho = semi_a * semi_b
            / ((semi_b * cl) * (semi_b * cl) + (semi_a * cp) * (semi_a * cp)).sqrt();
        // Unit-RMS, like the silhouette's: the raw amplitude-normalised sum
        // has an RMS of 0.195, which would make every hole a smooth ellipse
        // where the measured big ones are strongly re-entrant (circularity
        // 0.405 and 0.458, five to eight concave lobes).
        let f = crate::body::unit_fbm(fbm.at(perlin, cl * freq + off.x, cp * freq + off.y));
        // The concave bites, smoothstepped over their angular windows: this
        // is what turns a lumpy oval into the measured kidney. Lobes alone
        // cannot do it - a bump is not a bite. Deepest wins where two
        // overlap, so two adjacent bites cannot multiply into a pinch.
        let mut cut = 0.0_f64;
        for &(dir, edge) in bites {
            let x = ((d.dot(dir) - edge) / (1.0 - edge)).clamp(0.0, 1.0);
            cut = cut.max(x * x * (3.0 - 2.0 * x));
        }
        rho * (1.0 + amp * f) * (1.0 - bite * cut)
    });
    // A bitten hole has to be allowed to dip where it was bitten, or the
    // clamp fills the bite straight back in.
    let pinch_floor = PINCH_FLOOR * (1.0 - BITE_FLOOR_RELIEF * bite);
    ring.clamp_min(pinch_floor * ring.mean_radius());
    // The vesicle's own aspect needs its own kappa floor, exactly as the body
    // does: at aspect 2.4 the ellipse alone wants 0.99, so the body's 1.10
    // would quietly circularise the most elongated holes.
    let kappa = p.slope_limit.max(min_slope_limit(aspect));
    ring.limit_log_lipschitz(if bite > 0.0 {
        kappa.max(BITE_KAPPA)
    } else {
        kappa
    });
    for _ in 0..VESICLE_SMOOTH_PASSES {
        smooth_ring(&mut ring.r);
    }

    Cand {
        centre,
        r_eq,
        aspect,
        axis,
        ring,
        r_low: semi_b * (1.0 - amp) * (1.0 - bite),
        pinch_floor,
    }
}

/// Turn an accepted candidate into the geometry the writer emits.
fn finish(c: Cand, p: &Params, r0: f64) -> Vesicle {
    let perim = closed_len(&c.ring.points());
    let m = ((perim / (SEG_FRAC * r0)).round() as i64).clamp(12, 48) as usize;
    let pts = c.ring.resampled(m);
    let mut cubics = catmull_rom_closed(&pts, p.cr_tension);
    let clamped = clamp_handles(&mut cubics);
    let area = polygon_area(&pts).abs();
    Vesicle {
        centre: c.centre,
        r_eq: c.r_eq,
        aspect: c.aspect,
        axis: c.axis,
        ring: c.ring,
        pts,
        cubics,
        pinch_floor: c.pinch_floor,
        area,
        clamped,
    }
}

/// Is every probe of this outline inside the inner ring by `clearance`?
///
/// Every third ring sample, which is SPEC.md's M = 24 probes. The quality
/// gate re-runs exactly this test on the finished geometry.
pub fn boundary_clear(ring: &Ring, inner: &Ring, clearance: f64) -> bool {
    if !inner.contains(ring.c, clearance) {
        return false;
    }
    let n = ring.n();
    let mut i = 0;
    while i < n {
        if !inner.contains(ring.point(i), clearance) {
            return false;
        }
        i += PROBE_STRIDE;
    }
    true
}

/// Star-vs-star separation along the centre line, widened to a +-18 degree
/// window so a side-lobe graze cannot slip through.
fn separated(a: &Cand, b: &Cand, gap: f64) -> bool {
    let delta = b.centre - a.centre;
    let d = delta.len();
    if d < 1e-9 {
        return false;
    }
    let dir = delta * (1.0 / d);
    d >= max_radius_window(&a.ring, dir) + max_radius_window(&b.ring, -dir) + gap
}

/// Largest radius of `ring` within +-18 degrees of `dir`.
///
/// Shared with the quality gate so the packer and the gate cannot drift apart.
pub fn max_radius_window(ring: &Ring, dir: V2) -> f64 {
    let mut m = 0.0f64;
    for k in -WINDOW_HALF..=WINDOW_HALF {
        m = m.max(ring.radius_toward(dir.rot(k as f64 * WINDOW_STEP)));
    }
    m
}

/// Axis-aligned bounds of a point set.
fn bbox(pts: &[V2]) -> (V2, V2) {
    let mut lo = pts[0];
    let mut hi = pts[0];
    for q in pts {
        lo.x = lo.x.min(q.x);
        lo.y = lo.y.min(q.y);
        hi.x = hi.x.max(q.x);
        hi.y = hi.y.max(q.y);
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldCfg;
    use crate::rng::stream;

    /// A circular body of radius 300, as a stand-in for the inner ring.
    fn disc(r: f64) -> Ring {
        Ring::build(V2::new(0.0, 0.0), 360, |_t, _d| r)
    }

    fn test_field(rng: &mut Pcg32) -> Field {
        Field::new(
            rng,
            FieldCfg {
                freq: 0.55,
                angle_range: 1.0,
                bias: 0.0,
                quantize: None,
                octaves: 3,
                lacunarity: 2.03,
                gain: 0.45,
            },
        )
    }

    #[test]
    fn sizes_obey_the_law_and_sort_largest_first() {
        let p = Params::default();
        let r0 = p.r0;
        let (rmin, rmax) = (p.vesicle_rmin * r0, p.vesicle_rmax * r0);
        let mut rng = Pcg32::new(7, stream::VESICLE);
        let mut radii = Vec::new();
        for _ in 0..200 {
            let u = rng.f64();
            radii.push(rmin + (rmax - rmin) * u * u);
        }
        assert!(radii
            .iter()
            .all(|&r| r >= rmin - 1e-12 && r <= rmax + 1e-12));
        // u^2 biases low: the median must sit well below the midpoint.
        let mut sorted = radii.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let median = sorted[sorted.len() / 2];
        assert!(median < 0.5 * (rmin + rmax), "u^2 must bias toward small");

        let mut order: Vec<usize> = (0..radii.len()).collect();
        order.sort_by(|&a, &b| radii[b].total_cmp(&radii[a]).then(a.cmp(&b)));
        for w in order.windows(2) {
            assert!(radii[w[0]] >= radii[w[1]]);
        }
    }

    #[test]
    fn window_radius_of_a_circle_is_its_radius() {
        let ring = disc(50.0);
        let m = max_radius_window(&ring, V2::new(1.0, 0.0));
        assert!((m - 50.0).abs() < 1e-6, "got {m}");
    }

    #[test]
    fn packing_respects_clearance_and_gaps() {
        let p = Params::default();
        let r0 = p.r0;
        let inner = disc(r0);
        let mut rng = Pcg32::new(99, stream::MISC);
        let perlin = Perlin::new(&mut rng);
        let field = test_field(&mut rng);
        let mut vrng = Pcg32::new(99, stream::VESICLE);
        let ves = pack(
            &p,
            &inner,
            &field,
            &perlin,
            &mut vrng,
            V2::new(0.0, 0.0),
            r0,
        );

        assert!(!ves.is_empty(), "a plain disc must take some holes");
        assert!(
            ves.len() <= p.vesicle_count as usize,
            "never more than asked"
        );

        // The two invariants the packer promises, asserted exactly as it
        // enforces them: probe clearance against the inner ring, and the
        // windowed centre-line separation between every pair.
        let clearance = (1.2 * p.outline_weight * r0).max(p.vesicle_edge_clear * r0);
        for v in &ves {
            assert!(
                boundary_clear(&v.ring, &inner, clearance),
                "a hole bit into the ribbon"
            );
            for q in &v.pts {
                assert!(q.len() < r0, "a hole left the body entirely");
            }
        }
        for (i, a) in ves.iter().enumerate() {
            for b in ves.iter().skip(i + 1) {
                let delta = b.centre - a.centre;
                let d = delta.len();
                assert!(d > 1.0, "two holes landed on top of each other");
                let dir = delta * (1.0 / d);
                let gap = (p.vesicle_gap * r0).max(0.45 * a.r_eq.min(b.r_eq));
                let sep = max_radius_window(&a.ring, dir) + max_radius_window(&b.ring, -dir) + gap;
                assert!(d >= sep, "holes are closer than the gap: {d} < {sep}");
            }
        }
    }

    #[test]
    fn big_holes_are_lumpier_than_small_ones() {
        let p = Params::default();
        let r0 = p.r0;
        let mut rng = Pcg32::new(5, stream::MISC);
        let perlin = Perlin::new(&mut rng);
        let field = test_field(&mut rng);
        let mut vrng = Pcg32::new(5, stream::VESICLE);
        let c = V2::new(0.0, 0.0);
        let small = build_cand(&p, &field, &perlin, &mut vrng, c, r0, c, 25.0, 1.1, 0.05);
        let big = build_cand(&p, &field, &perlin, &mut vrng, c, r0, c, 75.0, 2.3, 0.95);
        let rel_spread = |ring: &Ring| {
            let mean = ring.mean_radius();
            let lo = ring.r.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = ring.r.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (hi - lo) / mean
        };
        assert!(
            rel_spread(&big.ring) > rel_spread(&small.ring),
            "size must drive lumpiness: small {} big {}",
            rel_spread(&small.ring),
            rel_spread(&big.ring)
        );
    }

    #[test]
    fn the_same_seed_packs_the_same_holes() {
        let p = Params::default();
        let r0 = p.r0;
        let inner = disc(r0);
        let run = || {
            let mut rng = Pcg32::new(3, stream::MISC);
            let perlin = Perlin::new(&mut rng);
            let field = test_field(&mut rng);
            let mut vrng = Pcg32::new(3, stream::VESICLE);
            pack(
                &p,
                &inner,
                &field,
                &perlin,
                &mut vrng,
                V2::new(0.0, 0.0),
                r0,
            )
        };
        let a = run();
        let b = run();
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.centre.x, y.centre.x);
            assert_eq!(x.centre.y, y.centre.y);
            assert_eq!(x.pts.len(), y.pts.len());
        }
    }
}

#[cfg(test)]
mod bite_tests {
    use super::*;

    /// Circularity of a closed ring: 1.0 is a circle, the artwork's three
    /// biggest holes measure 0.405, 0.459 and 0.587.
    fn circularity(ring: &Ring) -> f64 {
        let pts = ring.points();
        let per = closed_len(&pts);
        4.0 * PI * polygon_area(&pts).abs() / (per * per)
    }

    fn big_hole(bite: f64) -> Cand {
        let p = Params {
            vesicle_bite: bite,
            ..Params::default()
        };
        let mut rng = Pcg32::new(7, 3);
        let perlin = Perlin::new(&mut rng);
        let field = Field::new(&mut rng, crate::field::FieldCfg::default());
        build_cand(
            &p,
            &field,
            &perlin,
            &mut rng,
            V2::ZERO,
            300.0,
            V2::ZERO,
            70.0,
            2.0,
            0.95,
        )
    }

    /// The point of the whole bite mechanism: a big hole must come out
    /// re-entrant, not as a lumpy oval.
    ///
    /// Without it a star-shaped fbm blob sits near 0.79 however much
    /// amplitude it is given, because a lobe is a bump and a bump is not a
    /// bite. This is the regression that matters - two earlier versions of
    /// the geometry (a wide smooth window, and three evenly spaced bites)
    /// both measured BACK UP above the unbitten shape while looking, in the
    /// code, exactly as correct.
    #[test]
    fn a_bite_makes_a_hole_re_entrant() {
        let plain = circularity(&big_hole(0.0).ring);
        let bitten = circularity(&big_hole(0.60).ring);
        assert!(
            plain > 0.75,
            "unbitten should be a lumpy oval, got {plain:.3}"
        );
        assert!(
            bitten < 0.65,
            "bitten should reach the measured band, got {bitten:.3}"
        );
    }

    /// Deeper bites keep biting, monotonically. Guards the direction aiming:
    /// a bite across the long axis crescents the hole, one along it merely
    /// shortens it and measures rounder.
    #[test]
    fn deeper_bites_measure_less_circular() {
        let c = [0.0, 0.3, 0.6].map(|b| circularity(&big_hole(b).ring));
        assert!(
            c[0] > c[1] && c[1] > c[2],
            "circularity should fall with bite depth, got {c:?}"
        );
    }

    /// Small holes are left alone: the four smallest measured 0.70-0.87 and
    /// are correctly convex, so a bite there would only read as a chip.
    #[test]
    fn small_holes_are_not_bitten() {
        let p = Params {
            vesicle_bite: 0.60,
            ..Params::default()
        };
        let mut rng = Pcg32::new(11, 3);
        let perlin = Perlin::new(&mut rng);
        let field = Field::new(&mut rng, crate::field::FieldCfg::default());
        let small = build_cand(
            &p,
            &field,
            &perlin,
            &mut rng,
            V2::ZERO,
            300.0,
            V2::ZERO,
            24.0,
            1.1,
            0.1,
        );
        assert_eq!(
            small.pinch_floor, PINCH_FLOOR,
            "a hole under BITE_SPLIT must keep the unbitten floor"
        );
        assert!(circularity(&small.ring) > 0.70);
    }
}
