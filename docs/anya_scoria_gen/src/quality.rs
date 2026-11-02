//! The quality gates.
//!
//! Every one of these runs before an SVG is emitted, and any failure re-rolls
//! the whole logo from a fresh split of the seed. They are the difference
//! between a generator and a slot machine. Do not soften one to make a
//! particular seed pass: if a gate fires often, the parameter combination is
//! infeasible and the caller needs to hear that, which is what
//! [`crate::GenError::Exhausted`] is for.
//!
//! Each gate is its own function returning the rule name on failure, and
//! [`check_all`] runs them cheapest-first so a bad seed dies early.

use crate::bezier::{self, Cubic};
use crate::body::Scene;
use crate::facet::{CutReport, Facet, MIN_JUNCTION_COS};
use crate::math::{polygon_area, polygon_centroid, polyline_len, TAU, V2};
use crate::relief::Relief;
use crate::ring::Ring;
use crate::solid::DepthMap;
use crate::vesicle::{boundary_clear, max_radius_window, probe_gap, Vesicle};
use crate::Params;

/// A gate's verdict: the rule name on failure.
pub type Check = Result<(), &'static str>;

/// How squarely the node facing the viewer must face the viewer.
///
/// At the pole of the front surface the normal is the view direction almost
/// exactly; measured over 44 scenes across two presets, the worst pole-row
/// normal has `n.z` 0.883, so 0.70 is margin rather than a tuned threshold.
const NORMAL_POLE_MIN: f64 = 0.70;
/// How much of the map, on average, must face the viewer.
///
/// Measured 0.745 to 0.781 over the same scenes. A map built back to front
/// would come out near its negation and cannot approach this.
const NORMAL_MEAN_MIN: f64 = 0.45;
/// How far the hole set's centroid may sit from the body's, as a fraction
/// of R0.
///
/// The drawing's seven holes SPREAD: their centroid is 7 units from the
/// body's on a mean radius of 169, 0.04 R0, and they occupy all four
/// quadrants. Nothing in the packer asks for that - it enforces minimum
/// gaps and nothing else - and ungated the realised offset over 200 default
/// rolls is median 0.124 R0, p75 0.185, p90 0.237, max 0.40: a fifth of
/// rolls past 0.20, with every hole on one side.
///
/// Gated rather than steered, after steering was tried and measured. A
/// rigid pull of the seed set toward the body centroid inside the
/// relaxation moved nothing - the distribution came out identical to three
/// decimals with and without it - because the relaxed seeds are already
/// balanced; the offset is made AFTER relaxation, by the holes the packer
/// drops and the dart-thrown fallbacks for the ones that did not fit where
/// they were put. Fixing that means re-placing a dropped hole on the far
/// side, which is a different packer.
///
/// 0.20, not the 0.16 first tried: re-rolls per 200 logos run 93 ungated,
/// 199 at 0.20 and 258 at 0.16, and what 0.20 admits (p90 0.17, max 0.21)
/// is a rock with its holes a little off-centre, not one with its holes on
/// one side. Each re-roll is a whole new logo at about 15 ms.
///
/// This is the allowance at the drawing's SEVEN holes; it scales as
/// `sqrt(7 / n)`. The centroid of n points placed by the same process has a
/// spread that falls as `1 / sqrt(n)`, so one fixed number is either too
/// strict for three holes or too loose for nine. Applied flat at 0.20 the
/// gate took 58 of 400 single attempts, nearly all of them small draws,
/// and that selection pushed the mean drawn count among clean rolls from
/// 5.71 to 6.21 against a `vesicle_count` of 5 - the gate was quietly
/// rewriting what the parameter means.
const VESICLE_BALANCE_MAX: f64 = 0.20;
/// The hole count the allowance above was set at.
const VESICLE_BALANCE_REF_COUNT: f64 = 7.0;
/// Fewest holes that still read as scoria, whatever the draw asked for.
/// SPEC.md's own words, which until the count became a draw were only ever
/// a comment on the packing budget.
const MIN_HOLES: usize = 3;

/// Lowest Lambert separation between neighbouring tones that still reads as
/// a step rather than as a rounding accident.
const TONE_LAMBERT_GAP: f64 = 0.004;
/// How far a relief mark's realised mean width may stray from the waist it
/// was BUILT for, as a fraction of it.
///
/// Against the mark's own `width`, not against `relief_weight` - the waist
/// varies with how sharp the fold is, by design, and a gate on the
/// parameter would be a gate on that variation. What is being caught here
/// is an offset that folded over itself, which leaves the outline's length
/// untouched and collapses its area. The taper takes about 8% off the
/// waist, so the band is centred below 1.
const RELIEF_WIDTH_LO: f64 = 0.80;
/// The upper end of the same band.
const RELIEF_WIDTH_HI: f64 = 1.05;
/// How far the waist itself may stray from `relief_weight * R0`. The
/// generator's own spread is 0.22 either way; this is that plus rounding.
const RELIEF_WAIST_SPREAD: f64 = 0.25;

/// Slack on the limiter's own bound, to absorb the rounding of the two sweeps.
const LIMITER_SLACK: f64 = 1.0001;
/// Lower bound on the silhouette's relative radial standard deviation, as a
/// multiple of `lump_amp`.
///
/// SPEC.md has no gate here because it assumes `lump_amp` controls the
/// amplitude. It controls the EXPECTED amplitude: the fbm scatters around it
/// by a factor of three either way, so at the measured `lump_amp` 0.22 the
/// realised radial std runs 0.10 to 0.33 across seeds against the artwork's
/// measured 0.226, and the bottom of that range is an egg - a smooth ellipse
/// with a heavy outline, which is not the mark.
///
/// The band is a RATIO rather than an absolute because the gate's question is
/// "did this roll deliver what `lump_amp` asked for", not "is this the
/// artwork". A fixed band calibrated on the artwork rejects a quarter of
/// `--preset feral`, whose whole point is to ask for more. 0.70 and 1.50
/// reproduce the hand-checked [0.155, 0.330] at the measured `lump_amp` 0.22.
const AMPLITUDE_LO_FRAC: f64 = 0.70;
/// Upper bound on the same. Past it the lumps stop reading as a lump and
/// start reading as a splat; the limiter is still satisfied, so nothing else
/// catches it.
const AMPLITUDE_HI_FRAC: f64 = 1.50;
/// How much of `lump_amp` survives into the finished outline's deviation
/// from its base ellipse.
///
/// Three things take a bite out of it between the displacement field and the
/// emitted ring, and none of them is a defect: the projection's maximum over
/// tilt fills in the dips, the log-Lipschitz limiter erodes the steepest
/// flanks, and forty binomial passes band-limit what is left. Measured over
/// 200 seeds at each of the three presets, whose `lump_amp` spans 0.120 to
/// 0.300, the realised deviation is 0.73, 0.65 and 0.58 of what was asked -
/// mild saturation, so one constant at the default's 0.65 holds every preset
/// well inside the band either side of it.
const PROJECTION_DEV_GAIN: f64 = 0.65;
/// How far the finished bounding box may stray from the `aspect` asked for,
/// per unit of `lump_amp`.
///
/// Measured: the artwork is 457.13974 x 304.92206, aspect 1.49919, and
/// `aspect` defaults to 1.50 to match. But `aspect` only sets the BASE
/// ELLIPSE; the lumps are what the viewBox is finally fitted to, and they
/// swing the realised box from 1.09 to 2.05 across seeds. A square tile and
/// a two-to-one slab do not read as the same mark, which is the one thing a
/// logo generator cannot afford, so the realised box is gated and not just
/// the parameter.
///
/// The tolerance WIDENS with `lump_amp`, for the same reason the amplitude
/// band scales with it: a caller who asks for lumpier is asking for a box
/// that wanders further, and a flat band rejected a fifth of
/// `--preset feral`.
///
/// It never narrows below [`BBOX_ASPECT_TOL_FLOOR`]. Letting it narrow was
/// tried and measured: at `--preset tame`'s `lump_amp` of 0.12 the band
/// closes to [1.33, 1.69] and the failure rate doubles from 3% to 7%, which
/// buys nothing - a tame logo that came out at 1.70 is still the mark.
const BBOX_ASPECT_TOL: f64 = 1.06;
/// Floor on that tolerance: the band at the measured `lump_amp` of 0.22.
///
/// 1.233 puts it at [1.25, 1.90] around the default aspect 1.50, which is
/// the range hand-checked on a twelve-seed sheet. At feral's `lump_amp` of
/// 0.30 the tolerance opens to 1.318, i.e. [1.14, 1.98].
const BBOX_ASPECT_TOL_FLOOR: f64 = 1.233;
/// Lower bound on the ribbon's perceived width, as a fraction of `w0`.
const RIBBON_MIN: f64 = 0.75;
/// Upper bound on the ribbon's perceived width, as a fraction of `w0`.
const RIBBON_MAX: f64 = 1.30;
/// Points this far apart along the curve must not come within `NECK_FACTOR`
/// ribbon widths of each other in space.
const NECK_ARC_FRAC: f64 = 0.10;
/// How many ribbon widths of air a notch must keep.
const NECK_FACTOR: f64 = 2.4;
/// 60 degrees in radians: the turn between consecutive chords of a smooth
/// contour, above which the sampling was too coarse.
const MAX_CONTOUR_TURN: f64 = 1.047_197_551_196_597_6;
/// Share of handles the clamp may catch before the sampling is suspect.
/// Counted over the ribbon contours only: a facet's pieces meet the boundary
/// at an arbitrary point inside a segment, so the first interval of a piece
/// is short by construction and its handle is clamped by design, not by
/// accident. Gating that would reject every facet tiling there is.
const MAX_CLAMPED_FRAC: f64 = 0.02;
/// Minimum share of body area a tone must cover.
const MIN_TONE_SHARE: f64 = 0.06;
/// Vesicle coverage band. Below it the rock is a pebble, above it a sponge.
const COVERAGE_LO: f64 = 0.12;
/// Upper end of the coverage band. Measured 0.207.
const COVERAGE_HI: f64 = 0.26;
/// Largest over smallest placed vesicle radius. Below this the holes read as
/// polka dots rather than as gas.
const MIN_SIZE_SPREAD: f64 = 2.0;
/// Vesicle pinch floor, as a fraction of its own mean radius.
const MIN_VESICLE_PINCH: f64 = 0.35;
/// The health gate checks a hole against its own construction floor, less
/// this much, so the limiter and the smoothing passes have room to move a
/// sample a little below where `clamp_min` left it.
const PINCH_GATE_SLACK: f64 = 0.92;
/// Smallest area any emitted contour may enclose.
const MIN_CONTOUR_AREA: f64 = 1e-6;

/// Star-shapedness of the silhouette.
///
/// Together these two assertions are a PROOF that the contour does not
/// self-intersect - every ray from the centre meets it exactly once - so no
/// intersection test is needed on it. The second one is written in the
/// limiter's own multiplicative form rather than with logarithms, because
/// `ln` is a libm call and libm is the determinism hole.
pub fn star_shaped(ring: &Ring, r0: f64, kappa: f64) -> Check {
    let n = ring.n();
    if n < 16 {
        return Err("silhouette star-shapedness: too few ring samples");
    }
    let g = kappa * (TAU / n as f64);
    let cap = (1.0 + g) * LIMITER_SLACK;
    let flo = 1.0 / ((1.0 + g) * LIMITER_SLACK);
    for i in 0..n {
        let r = ring.r[i];
        if !r.is_finite() || r <= 0.25 * r0 {
            return Err("silhouette star-shapedness: radius collapsed");
        }
        let prev = ring.r[(i + n - 1) % n];
        if r > prev * cap || r < prev * flo {
            return Err("silhouette star-shapedness: slope exceeds the limiter");
        }
    }
    Ok(())
}

/// The silhouette actually has lumps, and not too many.
///
/// Relative radial standard deviation of the sampled ring, held to a band
/// around `lump_amp` (the artwork measures 0.226 at the default 0.22). This
/// is a gate on the ROLL, not on the parameters: `lump_amp` sets the expected
/// amplitude and the fbm scatters around it by a factor of three either way,
/// so the only way to keep every seed on-brand is to reject the tails.
pub fn silhouette_amplitude(ring: &Ring, lump_amp: f64, aspect: f64) -> Check {
    let n = ring.n();
    if n == 0 {
        return Err("silhouette amplitude: empty ring");
    }
    let mean = ring.r.iter().sum::<f64>() / n as f64;
    if !mean.is_finite() || mean <= 0.0 {
        return Err("silhouette amplitude: mean radius collapsed");
    }
    // AGAINST THE BASE ELLIPSE, not against the mean radius.
    //
    // `lump_amp` is documented, and measured, as a fraction of the BASE
    // ELLIPSE radius, and an ellipse's radius already varies a great deal on
    // its own - at the default's pre-stretched base of 1.69 its relative
    // radial std is 0.19 before a single lump is added. Measuring about the
    // mean therefore measures the aspect and the lumps together, which makes
    // this gate a second, worse `bbox_aspect` and leaves `lump_amp` almost
    // no authority over the number it is checked against: measured across
    // the three presets, which ask for 0.120, 0.201 and 0.300, the std about
    // the mean came out 0.199, 0.217 and 0.240 - a 2.5x request compressed
    // into a 1.2x response, so `tame` read as a splat and `feral` as an egg.
    //
    // Dividing by the base ellipse first removes the aspect from the
    // measurement entirely. The same three presets then give 0.087, 0.131
    // and 0.174, which tracks the request at a steady 0.58 to 0.73 of it.
    let base = crate::params::base_aspect(aspect);
    let (semi_a, semi_b) = (base.sqrt(), 1.0 / base.sqrt());
    let dev: Vec<f64> = (0..n)
        .map(|i| {
            let (st, ct) = crate::math::sin_cos(ring.theta(i));
            let rho = semi_a * semi_b
                / ((semi_b * ct) * (semi_b * ct) + (semi_a * st) * (semi_a * st)).sqrt();
            ring.r[i] / rho
        })
        .collect();
    let dm = dev.iter().sum::<f64>() / n as f64;
    if !dm.is_finite() || dm <= 0.0 {
        return Err("silhouette amplitude: the ellipse ratio collapsed");
    }
    // Written as a multiply rather than `powi`: the house rule is integer
    // powers spelled out, so nothing can quietly become a libm `pow`.
    let var = dev
        .iter()
        .map(|x| {
            let d = x / dm - 1.0;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    let std = var.sqrt();
    let want = PROJECTION_DEV_GAIN * lump_amp;
    if std < AMPLITUDE_LO_FRAC * want {
        return Err("silhouette amplitude: the blob came out as an egg");
    }
    if std > AMPLITUDE_HI_FRAC * want {
        return Err("silhouette amplitude: the blob came out as a splat");
    }
    Ok(())
}

/// The finished silhouette still reads as the mark the caller asked for.
///
/// Measured on the bounding box of the outer contour, which is what the
/// viewBox is fitted to and therefore what a viewer actually sees, against
/// the `aspect` parameter with a tolerance that widens as `lump_amp` does.
pub fn bbox_aspect(pts: &[V2], want: f64, lump_amp: f64) -> Check {
    if pts.len() < 4 {
        return Err("bbox aspect: too few points");
    }
    let (mut lo_x, mut hi_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut lo_y, mut hi_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for q in pts {
        lo_x = lo_x.min(q.x);
        hi_x = hi_x.max(q.x);
        lo_y = lo_y.min(q.y);
        hi_y = hi_y.max(q.y);
    }
    let h = hi_y - lo_y;
    if !h.is_finite() || h <= 0.0 {
        return Err("bbox aspect: the silhouette has no height");
    }
    let aspect = (hi_x - lo_x) / h;
    let tol = (1.0 + BBOX_ASPECT_TOL * lump_amp).max(BBOX_ASPECT_TOL_FLOOR);
    if aspect < want / tol {
        return Err("bbox aspect: the silhouette came out too square or tall");
    }
    if aspect > want * tol {
        return Err("bbox aspect: the silhouette came out as a slab");
    }
    Ok(())
}

/// Ribbon positivity and evenness.
///
/// The perceived band is the radial width foreshortened by the boundary slope,
/// `w / sqrt(1 + (r'/r)^2)`. If it leaves [0.75, 1.30] of `w0` the 1.60 cap on
/// the slope correction fired, which means `slope_limit` is too high for this
/// `outline_weight` - a real infeasibility, not a bad roll.
pub fn ribbon(outer: &Ring, inner: &Ring, rp: &[f64], r0: f64, w0: f64) -> Check {
    let n = outer.n();
    if inner.n() != n || rp.len() != n {
        return Err("ribbon: contour sample counts disagree");
    }
    for ((&r, &ri), &d) in outer.r.iter().zip(inner.r.iter()).zip(rp.iter()) {
        if !ri.is_finite() || ri < 0.15 * r0 {
            return Err("ribbon positivity: the body pinches shut in a notch");
        }
        if ri >= r {
            return Err("ribbon positivity: the inner contour reached the outer");
        }
        let slope = d / r;
        let k = (1.0 + slope * slope).sqrt();
        let perceived = (r - ri) / k;
        if perceived < RIBBON_MIN * w0 || perceived > RIBBON_MAX * w0 {
            return Err("ribbon evenness: the band thins or swells past +-30%");
        }
    }
    Ok(())
}

/// The one failure the star-shape theorem does not cover: a notch whose two
/// walls are each compliant but which the ribbon still closes across, filling
/// it with a solid black wedge.
pub fn ribbon_neck(pts: &[V2], w0: f64) -> Check {
    let m = pts.len();
    if m < 8 {
        return Err("ribbon neck: too few points");
    }
    // Uniform arclength resampling means index distance IS arc distance.
    let window = (NECK_ARC_FRAC * m as f64).round() as usize;
    let limit = NECK_FACTOR * w0;
    let limit2 = limit * limit;
    for i in 0..m {
        for j in (i + window)..m {
            if m - (j - i) < window {
                break;
            }
            if (pts[j] - pts[i]).len2() < limit2 {
                return Err("ribbon neck: a notch is narrower than the ribbon can cross");
            }
        }
    }
    Ok(())
}

/// Bezier sanity for one emitted contour: no degenerate segment, no handle
/// past half its chord, at least four segments.
pub fn contour_sanity(cs: &[Cubic]) -> Check {
    if cs.len() < 4 {
        return Err("bezier sanity: a contour has fewer than four segments");
    }
    for c in cs {
        if (c.p3 - c.p0).len2() < 1e-12 {
            return Err("bezier sanity: degenerate segment");
        }
        let chord = (c.p3 - c.p0).len();
        if (c.c1 - c.p0).len() > 0.5 * chord * 1.0001 || (c.c2 - c.p3).len() > 0.5 * chord * 1.0001
        {
            return Err("bezier sanity: a handle outruns half its chord");
        }
    }
    Ok(())
}

/// The sampling-adequacy half of the Bezier check: no turn over 60 degrees
/// between consecutive chords.
///
/// Only for the ribbon contours. SPEC.md's rationale for the 60 degrees is
/// that a turn that sharp means M was too low and the contour should be
/// rebuilt at double the resolution - an argument about sampling a smooth
/// curve. It does not transfer to a 14-segment vesicle, whose measured
/// originals genuinely turn hard (median deviation from chord 6.6%, against
/// 3% for the facets), nor to a facet, whose piece junctions are deliberate
/// corners.
pub fn sampling_turn(cs: &[Cubic]) -> Check {
    if bezier::max_turn(cs) > MAX_CONTOUR_TURN {
        return Err("bezier sanity: turn over 60 degrees, the sampling is too coarse");
    }
    Ok(())
}

/// Every facet must clear the area floor.
pub fn facet_areas(facets: &[Facet], body_area: f64, min_frac: f64) -> Check {
    if facets.is_empty() {
        return Err("facet area floor: no facets");
    }
    for f in facets {
        if f.area < min_frac * body_area {
            return Err("facet area floor: a facet is below the sliver threshold");
        }
    }
    Ok(())
}

/// No facet corner may be thinner than 12 degrees.
///
/// Measured on the SMOOTHED tangents, because a hairline wedge is what the
/// renderer draws, not what the polyline says.
pub fn facet_junctions(facets: &[Facet]) -> Check {
    for f in facets {
        if f.piece_starts.len() < 2 || f.cubics.is_empty() {
            continue;
        }
        for &s in &f.piece_starts {
            let out = &f.cubics[s];
            let inc = &f.cubics[(s + f.cubics.len() - 1) % f.cubics.len()];
            // Back along the incoming edge, forward along the outgoing one.
            let a = (inc.c2 - inc.p3).norm();
            let b = (out.c1 - out.p0).norm();
            if a.len2() < 1e-18 || b.len2() < 1e-18 {
                continue;
            }
            if a.dot(b) > MIN_JUNCTION_COS {
                return Err("facet sliver angle: a corner is thinner than 12 degrees");
            }
        }
    }
    Ok(())
}

/// What the subdivision reported about its own cuts.
pub fn cut_health(report: &CutReport, min_sep: f64) -> Check {
    if report.made == 0 {
        return Err("cut validity: not one cut survived validation");
    }
    if report.min_sep_frac < min_sep {
        return Err("facet cut separation: two crossings landed too close together");
    }
    if report.max_turning >= crate::facet::MAX_CUT_TURNING {
        return Err("cut validity: a cut turned more than 220 degrees");
    }
    Ok(())
}

/// All three tones present, each carrying real area.
///
/// Structural, because tones are assigned by quantile - but a two-tone scoria
/// is a different, worse logo, so assert it anyway.
pub fn tones_present(facets: &[Facet], body_area: f64) -> Check {
    let mut area = [0.0f64; 3];
    let mut count = [0usize; 3];
    for f in facets {
        let t = f.tone.min(2);
        area[t] += f.area;
        count[t] += 1;
    }
    for t in 0..3 {
        if count[t] == 0 || area[t] < MIN_TONE_SHARE * body_area {
            return Err("all three tones present: one tone is missing or vestigial");
        }
    }
    Ok(())
}

/// Vesicle clearance and pairwise gaps, re-run on the finished geometry.
pub fn vesicle_clearance(
    ves: &[Vesicle],
    inner: &Ring,
    clearance: f64,
    p: &Params,
    r0: f64,
) -> Check {
    for v in ves {
        // The larger of the absolute floor and the hole's own share - the
        // same rule the packer placed it by.
        if !boundary_clear(&v.ring, inner, clearance.max(p.vesicle_edge_ratio * v.r_eq)) {
            return Err("vesicle clearance: a hole is too close to the inner edge");
        }
    }
    for (i, a) in ves.iter().enumerate() {
        for b in ves.iter().skip(i + 1) {
            let delta = b.centre - a.centre;
            let d = delta.len();
            if d < 1e-9 {
                return Err("vesicle clearance: two holes share a centre");
            }
            let dir = delta * (1.0 / d);
            let gap = (p.vesicle_gap * r0).max(0.45 * a.r_eq.min(b.r_eq));
            if d < max_radius_window(&a.ring, dir) + max_radius_window(&b.ring, -dir) + gap
                || probe_gap(&a.ring, &b.ring) < gap
            {
                return Err("vesicle clearance: two holes are closer than the gap");
            }
        }
    }
    Ok(())
}

/// No pinched or figure-eight hole.
pub fn vesicle_health(ves: &[Vesicle]) -> Check {
    for v in ves {
        let mean = v.ring.mean_radius();
        if !mean.is_finite() || mean <= 0.0 {
            return Err("vesicle shape health: a hole has no radius");
        }
        // Against the floor this shape was actually built with: a bitten
        // hole is meant to dip, an accidentally pinched one is not.
        let floor = MIN_VESICLE_PINCH.min(v.pinch_floor * PINCH_GATE_SLACK);
        for &r in &v.ring.r {
            if !r.is_finite() || r < floor * mean {
                return Err("vesicle shape health: a hole is pinched");
            }
        }
    }
    Ok(())
}

/// Coverage band and the placement floor.
pub fn vesicle_coverage(ves: &[Vesicle], body_area: f64, target: u32) -> Check {
    if target == 0 {
        return Ok(());
    }
    // `target` is what the POISSON DRAW asked for on this roll, not
    // `vesicle_count`, which is only its mean. Measuring the placing against
    // the mean punishes a roll that drew high for drawing high - and since
    // the radii are compensated by `sqrt(mean / drawn)`, a high draw is
    // exactly the roll with the most holes to place and the least room each.
    // ceil(0.6 * target) without a float round trip, and never fewer than
    // three outright. SPEC.md says "a scoria with two holes does not read
    // as scoria" and until now nothing enforced it: at the bottom of the
    // Poisson range a draw of three with one dropped is 2, which is 67% of
    // the draw and passed. The absolute floor is the rule the proportional
    // one was standing in for.
    let floor = (6 * target as usize)
        .div_ceil(10)
        .max(MIN_HOLES.min(target as usize));
    if ves.len() < floor {
        return Err("vesicle coverage: too many holes could not be placed");
    }
    let total: f64 = ves.iter().map(|v| v.area).sum();
    let frac = total / body_area;
    if frac < COVERAGE_LO {
        return Err("vesicle coverage: under-vesiculated, this reads as a pebble");
    }
    if frac > COVERAGE_HI {
        return Err("vesicle coverage: over-vesiculated, this reads as a sponge");
    }
    Ok(())
}

/// Offset of a set of points' centroid from `body`, in units of `r0`.
fn balance_offset(centres: &[V2], body: V2, r0: f64) -> f64 {
    if centres.is_empty() {
        return 0.0;
    }
    let mut sum = V2::ZERO;
    for &c in centres {
        sum = sum + c;
    }
    (sum * (1.0 / centres.len() as f64) - body).len() / r0
}

/// The holes spread over the body rather than piling up on one side.
///
/// Unweighted centroid of the hole centres against the area centroid of the
/// silhouette, which is how the drawing was measured. Weighting by area
/// would let one big hole on the far side balance three small ones on the
/// near side, and that is exactly the lopsided layout this rejects.
pub fn vesicle_balance(ves: &[Vesicle], outer_pts: &[V2], r0: f64) -> Check {
    if ves.len() < 2 || outer_pts.len() < 3 {
        return Ok(());
    }
    let centres: Vec<V2> = ves.iter().map(|v| v.centre).collect();
    let allow = VESICLE_BALANCE_MAX * (VESICLE_BALANCE_REF_COUNT / centres.len() as f64).sqrt();
    if balance_offset(&centres, polygon_centroid(outer_pts), r0) > allow {
        return Err("vesicle balance: the holes all sit on one side of the body");
    }
    Ok(())
}

/// Size spread, so the holes do not read as polka dots.
pub fn vesicle_spread(ves: &[Vesicle], target: u32) -> Check {
    // Three, not four. The exemption was written when the count was fixed
    // at seven and a target under four meant a caller had deliberately
    // asked for a few holes; now three is the bottom of the Poisson range
    // and the commonest small draw, and it is exactly where a flat set of
    // sizes is most visible - three near-equal holes read as a pattern,
    // where seven read as texture.
    if target < MIN_HOLES as u32 || ves.len() < 2 {
        return Ok(());
    }
    let mut lo = f64::INFINITY;
    let mut hi: f64 = 0.0;
    for v in ves {
        lo = lo.min(v.r_eq);
        hi = hi.max(v.r_eq);
    }
    if hi < MIN_SIZE_SPREAD * lo {
        return Err("vesicle size spread: the packer dropped every big hole");
    }
    Ok(())
}

/// Nothing non-finite, nothing degenerate, anywhere that reaches the file.
pub fn finite_geometry(scene: &Scene) -> Check {
    for (_, cs) in contours(scene) {
        let mut poly = Vec::with_capacity(cs.len());
        for c in cs {
            for q in [c.p0, c.c1, c.c2, c.p3] {
                if !q.x.is_finite() || !q.y.is_finite() {
                    return Err("non-finite output: a coordinate is NaN or infinite");
                }
            }
            poly.push(c.p0);
        }
        if polygon_area(&poly).abs() < MIN_CONTOUR_AREA {
            return Err("degenerate output: a contour encloses no area");
        }
    }
    Ok(())
}

/// Explicit self-intersection test on the emitted contours.
///
/// This should always pass given the construction; it is here to catch an
/// implementation error, not a design one. A failure means a bug upstream,
/// and the re-roll will not fix it - which is exactly why it surfaces as an
/// `Exhausted` naming this rule.
///
/// WHICH CONTOURS, AND WHY IT DEPENDS ON THE BUILD. SPEC.md lists every one
/// of them (outer, inner, each facet, each vesicle), but it also, two
/// sections earlier, proves that it need not: a curve written as `r(theta) *
/// (cos theta, sin theta)` with `r > 0` single-valued is star-shaped about
/// its centre, and a star-shaped curve cannot self-intersect, because every
/// ray from the centre meets it exactly once. The ribbon's outer, inner and
/// rim contours are all polar about the same centre and `star_shaped` has
/// just asserted the hypothesis; each vesicle is polar about its own centre
/// and `vesicle_health` has asserted the same. Only the FACETS fall outside
/// the theorem, being Euler-traced polylines spliced into copied boundary
/// arcs, so they are the only ones a release build pays for.
///
/// This is not a corner cut for its own sake. Measured: the full sweep was
/// 2212 of the 2289 microseconds the whole gate suite cost on seed 1 - 96% of
/// it, and about a quarter of the entire generator. Debug builds, which is
/// what `cargo test` runs, still check every contour, so the tripwire the
/// test exists to be keeps working where implementation errors are actually
/// found.
pub fn no_self_intersection(scene: &Scene) -> Check {
    for (kind, cs) in contours(scene) {
        if !cfg!(debug_assertions) && !theorem_exempt(kind) {
            continue;
        }
        let pts = bezier::flatten(cs, 4);
        if bezier::self_intersects(&pts, true) {
            return Err("self-intersection: an emitted contour crosses itself");
        }
    }
    Ok(())
}

/// Is this contour one the star-shape theorem does NOT cover?
///
/// The facet pieces and the body-fill underlay they sit on are built by
/// tracing and splicing rather than from a polar radius, so nothing proves
/// them simple in advance.
fn theorem_exempt(kind: &str) -> bool {
    matches!(kind, "facet" | "body-fill" | "relief")
}

/// Every contour the writer will emit, in paint order.
fn contours(scene: &Scene) -> Vec<(&'static str, &[Cubic])> {
    let mut out: Vec<(&'static str, &[Cubic])> = Vec::new();
    out.push(("body-fill", &scene.root_cubics));
    for f in &scene.facets {
        out.push(("facet", &f.cubics));
    }
    for v in &scene.vesicles {
        out.push(("vesicle", &v.cubics));
    }
    for m in &scene.relief {
        out.push(("relief", &m.cubics));
    }
    out.push(("outline-outer", &scene.outer_cubics));
    out.push(("outline-inner", &scene.inner_cubics));
    if let Some(rim) = &scene.rim {
        out.push(("rim-outer", &rim.outer_cubics));
        out.push(("rim-inner", &rim.inner_cubics));
    }
    out
}

/// The depth map is the FRONT surface, the right way round.
///
/// If the march ever walked round the back of the solid, every tone and
/// every crease would be computed from a surface the viewer cannot see, and
/// the output would still be a perfectly plausible rock - just one lit and
/// creased by its own far side. Nothing downstream can notice that, so it
/// gets a gate.
///
/// What is NOT checked is that every normal faces the viewer, which was the
/// first attempt and was wrong. The outer rows legitimately face away: that
/// is what a FOLD is, a near bump whose surface curls past the apparent
/// contour and occludes what is behind it, and the map is built to keep the
/// nearest surface precisely so it can represent one. Measured, the worst
/// node at `t = 0.83` of the silhouette radius reaches `n.z` -0.12 at the
/// default and -0.64 at `--preset feral`, and both are real geometry; gating
/// on it re-rolled 91 of 120 rolls.
///
/// The invariants that do hold, and that a flipped map cannot satisfy: the
/// pole faces the viewer squarely, and the map faces the viewer on average.
/// Not, note, that the centre is nearer than the rim - a bump whose
/// silhouette point sits well above the equator is nearer the viewer than
/// the pole of a solid this flat, and 20 rolls in 240 are.
pub fn depth_map(depth: &DepthMap) -> Check {
    let (pts, nrm) = depth.nodes();
    let rows = DepthMap::n_rad() + 1;
    if pts.is_empty() || pts.len() != nrm.len() || pts.len() % rows != 0 {
        return Err("depth map: empty or mismatched node grid");
    }
    let mut mean_z = 0.0;
    for (p, n) in pts.iter().zip(nrm.iter()) {
        if !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite() {
            return Err("depth map: a surface node is not finite");
        }
        if (n.len() - 1.0).abs() > 1e-6 {
            return Err("depth map: a normal is not a unit vector");
        }
        mean_z += n.z;
    }
    mean_z /= pts.len() as f64;
    if mean_z < NORMAL_MEAN_MIN {
        return Err("depth map: the surface faces away from the viewer on average");
    }
    for row_n in nrm.chunks_exact(rows) {
        if row_n[0].z < NORMAL_POLE_MIN {
            return Err("depth map: the node facing the viewer does not face the viewer");
        }
    }
    Ok(())
}

/// The flat tones are a quantisation of the real shading, the right way up.
///
/// The gate for the whole harmonisation: the shadow tone must sit on a
/// darker part of the lit solid than the highlight does, measured as the
/// area-weighted mean Lambert brightness of the facets carrying each. A
/// mirrored light, or a sign slip anywhere between the surface normal and
/// the palette index, shows up here and nowhere else - every such mistake
/// still produces a perfectly plausible picture, just one lit from the wrong
/// side.
///
/// The ENDS only, not all three in order. `tone_noise` deliberately scatters
/// the assignment - 0.35 of it, measured off the artwork, whose own tones
/// overlap by a full quartile - so the mid tone's mean genuinely can cross
/// one of its neighbours on a seed where the quantile boundary lands
/// awkwardly among twelve facets. Pinning all three made the gate a test of
/// the scatter rather than of the direction, and it re-rolled seeds whose
/// shading was correct; measured, it exhausted the retry budget on seed 14.
/// The ends cannot cross without the light itself being wrong.
pub fn tone_follows_the_light(facets: &[Facet]) -> Check {
    let mut sum = [0.0f64; 3];
    let mut area = [0.0f64; 3];
    for f in facets {
        if f.tone > 2 {
            return Err("tone order: a facet carries a tone outside 0..2");
        }
        sum[f.tone] += f.lambert * f.area;
        area[f.tone] += f.area;
    }
    let mut mean = [0.0f64; 3];
    for t in 0..3 {
        if area[t] <= 0.0 {
            // `tones_present` is the gate for an empty tone; this one has
            // nothing to say about it.
            return Ok(());
        }
        mean[t] = sum[t] / area[t];
    }
    if mean[2] < mean[0] + TONE_LAMBERT_GAP {
        return Err("tone order: the flat tones do not follow the solid's own shading");
    }
    Ok(())
}

/// Every relief mark is a stroke on the rock, of the width it was asked for.
///
/// The marks are the one piece of geometry built by offsetting a curve rather
/// than by a polar subtraction, so they are the one piece the star-shape
/// theorem says nothing about. Hence a width check: an offset that folded
/// over itself comes out with the right outline length and the wrong area,
/// which no other gate would see.
pub fn relief_marks(marks: &[Relief], r0: f64, weight: f64) -> Check {
    for m in marks {
        if m.pts.len() < 6 || m.cubics.len() < 4 {
            return Err("relief: a mark has too little geometry to draw");
        }
        for q in &m.pts {
            if !q.x.is_finite() || !q.y.is_finite() {
                return Err("relief: a mark has a non-finite point");
            }
        }
        if bezier::self_intersects(&m.pts, true) {
            return Err("relief: a mark's offset folded over itself");
        }
        // Mean width from area over centreline length: a fold shows up as a
        // collapse here while leaving the outline's length untouched.
        let len = polyline_len(&m.spine);
        if len <= 0.0 {
            return Err("relief: a mark has no length");
        }
        let mean_w = polygon_area(&m.pts).abs() / len;
        if mean_w < RELIEF_WIDTH_LO * m.width || mean_w > RELIEF_WIDTH_HI * m.width {
            return Err("relief: a mark came out the wrong width for its spine");
        }
        let want = weight * r0;
        if m.width < (1.0 - RELIEF_WAIST_SPREAD) * want
            || m.width > (1.0 + RELIEF_WAIST_SPREAD) * want
        {
            return Err("relief: a mark's waist left the band around relief_weight");
        }
    }
    Ok(())
}

/// The gates that need only the outer ring.
///
/// Split out so `attempt_once` can run them the moment the silhouette exists,
/// BEFORE paying for the ribbon, the facet tiling and the vesicle packing.
/// They reject a little over half of all rolls between them - the generator
/// averages 4.3 attempts per logo at the defaults - and running them last
/// meant every rejected roll paid the full 2.5 ms of a logo it would never
/// emit. Running them first cuts a rejected roll to about 0.4 ms and the
/// whole generator from 8.4 ms per logo to 3.8.
///
/// `check_all` runs them again on the finished scene. That is deliberate:
/// they are cheap, and a gate that only ever runs in one of two paths is a
/// gate that quietly stops running.
pub fn check_silhouette(outer: &Ring, pts: &[V2], r0: f64, p: &Params) -> Check {
    star_shaped(outer, r0, p.slope_limit)?;
    silhouette_amplitude(outer, p.lump_amp, p.aspect)?;
    bbox_aspect(pts, p.aspect, p.lump_amp)
}

/// Run every gate, cheapest first.
pub fn check_all(scene: &Scene, p: &Params) -> Check {
    check_silhouette(&scene.outer, &scene.outer_pts, scene.r0, p)?;
    ribbon(&scene.outer, &scene.inner, &scene.rp, scene.r0, scene.w0)?;
    vesicle_spread(&scene.vesicles, scene.vesicles_drawn)?;
    vesicle_coverage(&scene.vesicles, scene.body_area, scene.vesicles_drawn)?;
    vesicle_balance(&scene.vesicles, &scene.outer_pts, scene.r0)?;
    vesicle_health(&scene.vesicles)?;
    cut_health(&scene.cuts, p.facet_min_chord_sep)?;
    facet_areas(&scene.facets, scene.body_area, p.facet_min_area)?;
    tones_present(&scene.facets, scene.body_area)?;
    tone_follows_the_light(&scene.facets)?;
    facet_junctions(&scene.facets)?;
    depth_map(&scene.depth)?;
    relief_marks(&scene.relief, scene.r0, p.relief_weight)?;

    contour_sanity(&scene.outer_cubics)?;
    contour_sanity(&scene.inner_cubics)?;
    sampling_turn(&scene.outer_cubics)?;
    sampling_turn(&scene.inner_cubics)?;
    if let Some(rim) = &scene.rim {
        contour_sanity(&rim.outer_cubics)?;
        contour_sanity(&rim.inner_cubics)?;
        sampling_turn(&rim.outer_cubics)?;
        sampling_turn(&rim.inner_cubics)?;
    }
    for v in &scene.vesicles {
        contour_sanity(&v.cubics)?;
    }
    if scene.segments > 0 && (scene.clamped as f64) > MAX_CLAMPED_FRAC * scene.segments as f64 {
        return Err("bezier sanity: over 2% of handles clamped, the sampling is too coarse");
    }

    let clearance = (1.2 * p.outline_weight * scene.r0).max(p.vesicle_edge_clear * scene.r0);
    vesicle_clearance(&scene.vesicles, &scene.inner, clearance, p, scene.r0)?;
    ribbon_neck(&scene.outer_pts, scene.w0)?;
    finite_geometry(scene)?;
    no_self_intersection(scene)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::sin_cos;

    #[test]
    fn balance_offset_is_the_centroid_distance_over_r0() {
        let body = V2::new(10.0, -4.0);
        // Symmetric about the body: no offset.
        let sym = [
            V2::new(110.0, -4.0),
            V2::new(-90.0, -4.0),
            V2::new(10.0, 56.0),
            V2::new(10.0, -64.0),
        ];
        assert!(balance_offset(&sym, body, 300.0).abs() < 1e-12);
        // Everything 60 units to one side on R0 300: 0.2.
        let one_side = [
            V2::new(70.0, -4.0),
            V2::new(70.0, 26.0),
            V2::new(70.0, -34.0),
        ];
        assert!((balance_offset(&one_side, body, 300.0) - 0.2).abs() < 1e-12);
        assert_eq!(balance_offset(&[], body, 300.0), 0.0);
    }

    fn disc(r: f64, n: usize) -> Ring {
        Ring::build(V2::new(0.0, 0.0), n, |_t, _d| r)
    }

    #[test]
    fn a_circle_passes_star_shapedness() {
        let ring = disc(300.0, 720);
        star_shaped(&ring, 300.0, 1.10).expect("a circle is as star-shaped as it gets");
    }

    #[test]
    fn a_spike_fails_star_shapedness() {
        let mut ring = disc(300.0, 720);
        ring.r[100] = 900.0;
        assert!(star_shaped(&ring, 300.0, 1.10).is_err());
        let mut ring = disc(300.0, 720);
        ring.r[100] = 10.0;
        assert!(star_shaped(&ring, 300.0, 1.10).is_err());
    }

    #[test]
    fn a_clean_annulus_passes_the_ribbon_gates() {
        let outer = disc(300.0, 720);
        let w0 = 0.058 * 300.0;
        let inner = disc(300.0 - w0, 720);
        let rp = vec![0.0; 720];
        ribbon(&outer, &inner, &rp, 300.0, w0).expect("a constant band is even by definition");
        let pts: Vec<V2> = (0..256)
            .map(|i| {
                let (s, c) = sin_cos(TAU * i as f64 / 256.0);
                V2::new(300.0 * c, 300.0 * s)
            })
            .collect();
        ribbon_neck(&pts, w0).expect("a circle has no notch");
    }

    #[test]
    fn a_thin_band_fails_the_ribbon_gate() {
        let outer = disc(300.0, 720);
        let w0 = 0.058 * 300.0;
        let inner = disc(300.0 - 0.5 * w0, 720);
        let rp = vec![0.0; 720];
        assert!(ribbon(&outer, &inner, &rp, 300.0, w0).is_err());
    }

    #[test]
    fn the_neck_gate_catches_a_pinched_waist() {
        // Two lobes joined by a neck one ribbon width across.
        let w0 = 17.4;
        let mut pts = Vec::new();
        let n = 256;
        for i in 0..n {
            let a = TAU * i as f64 / n as f64;
            let (s, c) = sin_cos(a);
            // A peanut: the radius dips hard at +-90 degrees.
            let r = 300.0 * (0.02 + 0.98 * (c * c));
            pts.push(V2::new(r * c, r * s));
        }
        assert!(
            ribbon_neck(&pts, w0).is_err(),
            "a waist this tight must be rejected"
        );
    }

    #[test]
    fn coverage_band_is_two_sided() {
        let target = 7;
        // An empty set cannot satisfy the placement floor.
        assert!(vesicle_coverage(&[], 1000.0, target).is_err());
        assert!(
            vesicle_coverage(&[], 1000.0, 0).is_ok(),
            "zero holes was asked for"
        );
    }

    /// The tone-order gate has to catch a mirrored light, which is the one
    /// mistake that still produces a plausible picture.
    #[test]
    fn the_tone_gate_catches_a_mirrored_light() {
        let facet = |tone: usize, lambert: f64| Facet {
            pts: Vec::new(),
            breaks: Vec::new(),
            cubics: Vec::new(),
            piece_starts: Vec::new(),
            centroid: V2::new(0.0, 0.0),
            area: 1.0,
            tone,
            lambert,
            clamped: 0,
        };
        // Right way up: shadow on the dark part, highlight on the bright.
        let good = [facet(0, -0.3), facet(1, 0.1), facet(2, 0.6)];
        assert!(tone_follows_the_light(&good).is_ok());
        // Mirrored.
        let bad = [facet(0, 0.6), facet(1, 0.1), facet(2, -0.3)];
        assert!(tone_follows_the_light(&bad).is_err());
        // The MID may cross a neighbour - `tone_noise` scatters it on
        // purpose - as long as the ends do not.
        let noisy = [facet(0, -0.3), facet(1, 0.7), facet(2, 0.6)];
        assert!(
            tone_follows_the_light(&noisy).is_ok(),
            "the gate must not forbid the scatter it was built around"
        );
    }

    /// And a relief mark that folded over itself has to be caught by area,
    /// because its outline length is unchanged by the fold.
    #[test]
    fn the_relief_gate_catches_a_collapsed_mark() {
        let spine: Vec<V2> = (0..40).map(|i| V2::new(f64::from(i) * 4.0, 0.0)).collect();
        let good = crate::relief::Relief {
            pts: {
                // A plain 10-wide band over the same spine.
                let mut v: Vec<V2> = spine.iter().map(|q| V2::new(q.x, q.y + 5.0)).collect();
                v.extend(spine.iter().rev().map(|q| V2::new(q.x, q.y - 5.0)));
                v
            },
            cubics: vec![
                Cubic {
                    p0: V2::new(0.0, 0.0),
                    c1: V2::new(1.0, 0.0),
                    c2: V2::new(2.0, 0.0),
                    p3: V2::new(3.0, 0.0),
                };
                8
            ],
            spine: spine.clone(),
            width: 10.0,
            score: 1.0,
            clamped: 0,
        };
        assert!(relief_marks(std::slice::from_ref(&good), 300.0, 10.0 / 300.0).is_ok());

        let collapsed = crate::relief::Relief {
            pts: {
                let mut v: Vec<V2> = spine.iter().map(|q| V2::new(q.x, q.y + 1.0)).collect();
                v.extend(spine.iter().rev().map(|q| V2::new(q.x, q.y - 1.0)));
                v
            },
            ..good
        };
        assert!(
            relief_marks(&[collapsed], 300.0, 10.0 / 300.0).is_err(),
            "a mark a fifth of its asked-for width must be rejected"
        );
    }
}
