//! The silhouette, the ribbon, and the pipeline that assembles a whole logo.
//!
//! The silhouette is the APPARENT CONTOUR of a 3D rock - see [`crate::solid`]
//! for the solid itself and for the three-line proof that projecting it keeps
//! the property everything here leans on. That property: the outline is a
//! single-valued `r(theta) > 0`, hence star-shaped about its centre, hence
//! unable to cross itself, and the ribbon then falls out as a scalar
//! subtraction `r - w(theta)` rather than a general polygon offset - which is
//! the error-prone part of every other approach.
//!
//! Construction, in canonical space (R0 = 300):
//!
//! 1. base ellipsoid, semi-axes `R0*sqrt(aspect)`, `R0/sqrt(aspect)` and
//!    `depth` times the second, displaced along its radii by fBm sampled on
//!    the sphere;
//! 2. 720 samples of theta, each the maximum of `R(theta, u) * cos u` over the
//!    out-of-plane tilt `u` - the projected radius of the solid;
//! 3. domain warp - the flow field moves the NOISE LOOKUP, never the geometry,
//!    which is what keeps `r` single-valued;
//! 4. the log-Lipschitz limiter, then the band-limiting smoothing passes;
//! 5. uniform arclength resample, then Catmull-Rom to cubics;
//! 6. the depth map, whose normals light the facets and find the creases the
//!    relief marks are drawn along.

use std::time::Instant;

use crate::bezier::{catmull_rom_closed, clamp_handles, max_turn, Cubic};
use crate::facet::{self, CutReport, Facet};
use crate::field::{Field, FieldCfg};
use crate::math::{closed_len, resample_closed, sin_cos, TAU, V2, V3};
use crate::noise::{Perlin, SphereFbm};
use crate::palette::Ramp;
use crate::relief::{self, Relief};
use crate::ring::Ring;
use crate::rng::{splitmix64, stream, Pcg32};
use crate::solid::{DepthMap, Light, Solid};
use crate::vesicle::{self, Vesicle};
use crate::{quality, svg, GenError, Params};

/// Octaves in the flow field's own fbm. SPEC.md's `field_dir` sketch.
const FIELD_OCTAVES: u32 = 3;
/// Radial floor applied before the limiter, as a fraction of R0. With
/// `lump_amp <= 0.35` and a normalised fbm this never fires; it is the guard
/// against an abusive parameter set, not a shaping step.
const R_FLOOR_FRAC: f64 = 0.25;
/// Floor for the inner contour, as a fraction of R0.
const R_IN_FLOOR_FRAC: f64 = 0.15;
/// Cap on the ribbon's slope correction. At kappa 1.10 the limiter already
/// bounds k at sqrt(1+1.21) = 1.487, so this is a safety net that never fires
/// - the two constants were chosen to interlock.
const SLOPE_CORRECTION_CAP: f64 = 1.60;
/// Boundary points of the facet root, as a fraction of `bezier_points`. The
/// facets run half a ribbon width proud of the silhouette and the ribbon is
/// drawn over that edge, so the root does not need the outline's resolution -
/// and at half of it the arcs come out near the artwork's measured 8.5 to
/// 11.4 unit median facet chord.
const FACET_ROOT_FRAC: usize = 2;
/// Binomial smoothing passes over the silhouette after the limiter.
///
/// This is the band-limiter, and it has to be far wider than it looks.
///
/// The limiter is purely erosive, so a deep notch survives with walls at
/// exactly kappa and a corner at the bottom - a V, where every concave notch
/// in the artwork is rounded (measured tightest curvature radius 12.5 units
/// on a mean radius of 169.5, i.e. 0.074 of it). Six passes at 720 samples
/// is a 1.2-degree window: far too narrow to touch a notch 20 degrees wide,
/// and it left the sharpest generated convex corner at 0.045 of the mean
/// radius, twice as sharp as anything in the drawing. On the sheet those
/// read as spikes, and they were the clearest remaining tell.
///
/// Forty passes is a Gaussian of sigma about 2.5 degrees. Over the same
/// seeds it lifts the sharpest convex corner to 0.09-0.10, inside the
/// artwork's band, and attenuates harmonic k by exp(-(k*sigma)^2/2): under
/// a tenth at h12, half by h20, gone by h40. That is the measured spectrum,
/// which carries real energy out to h12 and collapses to 0.004 above it.
///
/// Eighty measures well too but starts rounding the lumps themselves back
/// toward an egg.
const RING_SMOOTH_PASSES: usize = 40;
/// The same, for the inner contour, whose eroder leaves kinks of its own.
const INNER_SMOOTH_PASSES: usize = 3;
/// 60 degrees. A turn between consecutive chords above this means the
/// contour is under-sampled; SPEC.md's remedy is to double M and rebuild.
const MAX_CHORD_TURN: f64 = 1.047_197_551_196_597_6;
/// How far the doubling may go, as a multiple of `bezier_points`.
///
/// One, measured over 300 seeds: allowing two doublings cuts the re-roll rate
/// from 2.1 to 1.6 per logo but takes the largest document from 129 KB to
/// 204 KB, and the second doubling only helps where the contour has a real
/// cusp rather than a sampling problem - which is the case the gate is
/// supposed to reject, not paper over.
const MAX_DOUBLINGS: u32 = 1;
/// Ceiling on how far the inner eroder may widen the band, as a fraction of
/// `w0 * k`. The ribbon-evenness gate allows 1.30; this keeps a margin under
/// it. See `erode_log_lipschitz` for why the two have to be reconciled.
const WIDTH_CLAMP_FRAC: f64 = 1.25;
/// Minimum points on the inner contour, which is shorter than the outer.
const MIN_INNER_POINTS: usize = 128;
/// Angular resolution of the depth map.
///
/// A quarter of `ring_samples`. At 180 angles the nodes near the rim sit
/// about 11 units apart, just inside the brightness gradient's own 12-unit
/// step, so the central difference always spans a cell and never measures the
/// bilinear interpolation's seams rather than the surface.
///
/// The map is read a few thousand times per logo - the facet tones, the cut
/// seeding, the crease scores - and NOT per Euler step, since the cut tracer
/// takes its direction from the surface once at its seed. So the grid is
/// sized by what the gradient step can resolve, not by the number of
/// lookups, and building it finer only buys noise evaluations.
const DEPTH_THETA: usize = 180;
/// Gain that turns the amplitude-normalised fbm into a unit-RMS one.
///
/// Measured over 40 seeds x 720 samples at every octave/gain combination in
/// range: the amplitude-normalised sum has an RMS of 0.195 and a peak of
/// 0.66, stable to within 15%. Without this correction `lump_amp` means a
/// fifth of what SPEC.md says it means - 0.22 produced a relative radial std
/// of 0.03 against the measured 0.25, and the silhouette came out as a fuzzy
/// ellipse with no gestural lumps at all. With it: std 0.21, Rmin/Rmean 0.54
/// against the measured 0.52.
pub const FBM_RMS_GAIN: f64 = 5.13;
/// Where the unit-RMS fbm starts bending over instead of growing. The noise
/// peaks at about 3.4 times its RMS, and at `lump_amp` 0.35 an unlimited tail
/// would drive the radius negative. The knee keeps the deep notches deep
/// (measured Rmin/Rmean 0.52) while bounding the result to +-2.
const FBM_KNEE: f64 = 2.0;

/// Scale an amplitude-normalised fbm sample to unit RMS and soft-limit it.
///
/// Algebraic, so there is no `tanh` and no libm. Shared with the vesicle
/// outlines and the tone map, which draw on the same noise and would be just
/// as washed out without it.
pub fn unit_fbm(raw: f64) -> f64 {
    let x = raw * FBM_RMS_GAIN;
    x / (1.0 + (x / FBM_KNEE) * (x / FBM_KNEE)).sqrt()
}

/// Per-stage timings, for `--time`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timings {
    /// Whole-logo attempts spent, including the one that succeeded.
    pub attempts: u32,
    /// Building the Perlin table and the flow field.
    pub field_us: u128,
    /// Silhouette, limiter, ribbon offsets and their cubics.
    pub ring_us: u128,
    /// Recursive bisection and tone assignment.
    pub facet_us: u128,
    /// Collision packing.
    pub vesicle_us: u128,
    /// Crease scoring and the tapered marks.
    pub relief_us: u128,
    /// Every quality gate.
    pub quality_us: u128,
    /// Document writing.
    pub svg_us: u128,
    /// End to end, re-rolls included.
    pub total_us: u128,
}

/// The second, thinner concentric ribbon (`inner_rim`).
#[derive(Debug, Clone)]
pub struct Rim {
    /// Outer loop of the rim band - the main ribbon's inner contour.
    pub outer_cubics: Vec<Cubic>,
    /// Inner loop, reversed.
    pub inner_cubics: Vec<Cubic>,
}

/// Everything one finished logo consists of, before it is written out.
///
/// The gates read this; so does the writer. Keeping the geometry in one struct
/// is what lets a gate be a plain function of the output rather than a hook
/// inside the construction.
#[derive(Debug, Clone)]
pub struct Scene {
    /// The seed this attempt actually used (not necessarily `params.seed`).
    pub seed: u64,
    /// Body centre in canonical units.
    pub c: V2,
    /// Canonical mean radius.
    pub r0: f64,
    /// Perceived ribbon width in canonical units.
    pub w0: f64,
    /// Silhouette area.
    pub body_area: f64,
    /// The silhouette as `r(theta)`.
    pub outer: Ring,
    /// The ribbon's inner edge, `r - w`.
    pub inner: Ring,
    /// Radial derivative of the silhouette, per sample.
    pub rp: Vec<f64>,
    /// Radial ribbon width, per sample.
    pub w: Vec<f64>,
    /// The emitted outer contour, uniform arclength.
    pub outer_pts: Vec<V2>,
    /// The emitted inner contour, already reversed.
    pub inner_pts: Vec<V2>,
    /// Outer subpath.
    pub outer_cubics: Vec<Cubic>,
    /// Inner subpath, wound the other way.
    pub inner_cubics: Vec<Cubic>,
    /// The optional second rim.
    pub rim: Option<Rim>,
    /// The facet root, as one closed curve. Painted under the facets in the
    /// mid tone so no antialiased seam between two of them can show through.
    pub root_cubics: Vec<Cubic>,
    /// The facets, in paint order.
    pub facets: Vec<Facet>,
    /// The vesicles, in placement order.
    pub vesicles: Vec<Vesicle>,
    /// How many the Poisson draw asked for, which is what the coverage gate
    /// measures the placing against - the parameter is only the MEAN.
    pub vesicles_drawn: u32,
    /// The interior relief marks, strongest crease first.
    pub relief: Vec<Relief>,
    /// The solid's front surface: the normals that lit the facets and the
    /// brightness the creases were found in. The gates read it to check that
    /// the flat tones and the real shading still agree.
    pub depth: DepthMap,
    /// The light those normals were shaded against.
    pub light: Light,
    /// What the subdivision did.
    pub cuts: CutReport,
    /// The three tones.
    pub ramp: Ramp,
    /// Handles the Bezier clamp caught across the whole logo.
    pub clamped: usize,
    /// Cubics emitted across the whole logo.
    pub segments: usize,
}

/// Five-tap binomial smoothing over a wrapped array.
///
/// Applied to the radial derivative before it becomes the ribbon's slope
/// correction. The derivative is a central difference of a ring the limiter
/// has just clamped, so wherever the limiter bit, r' steps and the correction
/// k steps with it: measured, the raw version makes the band jump by 0.28*w0
/// between adjacent samples, which pushes the inner contour well past the
/// Lipschitz bound, fires the eroder on a third of the ring and swells the
/// band by half - failing the very evenness the correction exists to produce.
/// Two samples either side is one degree at the default 720 and costs nothing
/// in shape.
fn smooth_wrapped(v: &[f64]) -> Vec<f64> {
    let n = v.len() as i64;
    if n < 5 {
        return v.to_vec();
    }
    (0..v.len())
        .map(|i| {
            let g = |k: i64| v[((i as i64 + k).rem_euclid(n)) as usize];
            (g(-2) + 4.0 * g(-1) + 6.0 * g(0) + 4.0 * g(1) + g(2)) / 16.0
        })
        .collect()
}

/// One binomial pass over a wrapped radius array, in place.
///
/// Why the silhouette needs it at all: the limiter is a min/max filter, so
/// wherever it bites it leaves a C0 kink - the slope jumps from the raw value
/// to exactly +-kappa in one sample, which is a 48-degree corner on the
/// emitted polyline. The artwork has NO sharp corner anywhere (measured
/// minimum convex curvature radius 19.1 units, 10th percentile 12.6, concave
/// notches bottoming out at 12.5), so those kinks are wrong twice over: they
/// are an artefact of the guard rather than of the design, and they are not
/// what was drawn.
///
/// This is safe for the star-shape invariant. If `r[i+1] <= cap * r[i]` holds
/// for every i, it still holds after the pass: both sides are the same
/// weighted average of terms that each obey the bound.
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

/// Resample a ring and fit cubics, doubling the sample count while any turn
/// between consecutive chords exceeds 60 degrees.
///
/// SPEC.md's own remedy for an over-sharp turn is "double M and rebuild", not
/// "re-roll the logo": the turn measures whether the sampling resolves the
/// curve, and the answer is more samples. The cap stops a genuinely cusped
/// ring from inflating the file.
fn fit_contour(ring: &Ring, m0: usize, tau: f64, reverse: bool) -> (Vec<V2>, Vec<Cubic>, usize) {
    let mut m = m0;
    for _ in 0..=MAX_DOUBLINGS {
        let mut pts = ring.resampled(m);
        if reverse {
            pts.reverse();
        }
        let mut cubics = catmull_rom_closed(&pts, tau);
        let clamped = clamp_handles(&mut cubics);
        if max_turn(&cubics) <= MAX_CHORD_TURN || m >= m0 << MAX_DOUBLINGS {
            return (pts, cubics, clamped);
        }
        m *= 2;
    }
    unreachable!("the loop returns on its last iteration")
}

/// The erosion half of the log-Lipschitz limiter, applied in place.
///
/// The inner contour needs this and not the dilation half: subtracting a
/// varying `w` can locally steepen `r_in` past kappa even though `r_out` was
/// compliant, but filling its notches would eat into the band.
fn erode_log_lipschitz(r: &mut [f64], kappa: f64) {
    let n = r.len();
    if n < 3 {
        return;
    }
    let g = kappa * (TAU / n as f64);
    let cap = 1.0 + g;
    for _ in 0..2 {
        for i in 0..n {
            let j = (i + n - 1) % n;
            r[i] = r[i].min(r[j] * cap);
        }
        for i in (0..n).rev() {
            let j = (i + 1) % n;
            r[i] = r[i].min(r[j] * cap);
        }
    }
}

/// Stop the eroder from widening the band past the evenness gate.
///
/// Eroding `r_in` can only lower it, which can only make the ribbon thicker.
/// The inner contour is inherently steeper than the outer by a factor
/// `r/(r-w)`, so at the outer's own budget the eroder fires wherever the
/// silhouette sits on the limiter and the band swells by up to 50%. Evenness
/// wins: the ribbon being a constant width is the measured, brand-defining
/// property, while a steep inner contour is harmless - the star-shape theorem
/// still forbids self-intersection, and the handle clamp plus the
/// self-intersection gate catch a Bezier that overshoots.
fn clamp_band(inner: &mut [f64], outer: &[f64], w: &[f64]) {
    for ((ri, &ro), &wi) in inner.iter_mut().zip(outer.iter()).zip(w.iter()) {
        *ri = ri.max(ro - WIDTH_CLAMP_FRAC * wi);
    }
}

/// The seed for attempt number `attempt`.
///
/// SPEC.md writes this as `splitmix64(seed + attempt)`, which makes adjacent
/// seeds share a re-roll ladder: seed 5's second re-roll IS seed 6's first,
/// so two different seeds can emit byte-identical logos whenever the first
/// one needs a retry. Measured, that happens inside the first ten seeds at
/// the defaults. Mixing the attempt through the golden ratio before the xor
/// keeps the ladders disjoint, and attempt 0 still uses the caller's seed.
fn reroll_seed(seed: u64, attempt: u32) -> u64 {
    if attempt == 0 {
        return seed;
    }
    let mut s = seed ^ (attempt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    splitmix64(&mut s)
}

/// Generate one logo, re-rolling until every gate passes.
pub fn generate_checked(p: &Params) -> Result<String, GenError> {
    generate_timed(p).map(|(doc, _)| doc)
}

/// As [`generate_checked`], but also reporting where the time went.
pub fn generate_timed(p: &Params) -> Result<(String, Timings), GenError> {
    let start = Instant::now();
    let mut last = "no gate ran";
    for attempt in 0..=p.retry_budget {
        let seed = reroll_seed(p.seed, attempt);
        match attempt_once(p, seed) {
            Ok((scene, mut t)) => {
                let t0 = Instant::now();
                let doc = svg::render(&scene, p);
                t.svg_us = t0.elapsed().as_micros();
                t.attempts = attempt + 1;
                t.total_us = start.elapsed().as_micros();
                return Ok((doc, t));
            }
            Err(rule) => last = rule,
        }
    }
    Err(GenError::Exhausted {
        rule: last,
        retries: p.retry_budget,
    })
}

/// One roll of the dice: build everything, then put it through the gates.
fn attempt_once(p: &Params, seed: u64) -> Result<(Scene, Timings), &'static str> {
    let mut t = Timings::default();
    let r0 = p.r0;
    // Canonical space is centred on the origin; the viewBox is fitted to the
    // finished silhouette afterwards, exactly as the artwork is.
    let c = V2::new(0.0, 0.0);
    let w0 = p.outline_weight * r0;
    let n = p.ring_samples as usize;

    // FIVE STREAMS off one seed. If vesicle placement drew from the same
    // stream as the silhouette, changing vesicle_count would reshuffle the
    // rock - and a parameter surface you cannot explore is a slot machine.
    let mut rng_blob = Pcg32::new(seed ^ stream::BLOB, stream::BLOB);
    let mut rng_ves = Pcg32::new(seed ^ stream::VESICLE, stream::VESICLE);
    let mut rng_facet = Pcg32::new(seed ^ stream::FACET, stream::FACET);
    let mut rng_tone = Pcg32::new(seed ^ stream::TONE, stream::TONE);
    let mut rng_misc = Pcg32::new(seed ^ stream::MISC, stream::MISC);

    let t0 = Instant::now();
    let perlin = Perlin::new(&mut rng_misc);
    // ONE field per seed. It leans the lumps, orients the vesicles and curves
    // the facet cuts; that shared origin is what makes a seed read as one rock
    // rather than three unrelated effects.
    let field = Field::new(
        &mut rng_misc,
        FieldCfg {
            freq: p.field_freq,
            angle_range: p.field_angle_range,
            bias: p.field_bias,
            quantize: p.field_quantize,
            octaves: FIELD_OCTAVES,
            lacunarity: p.lacunarity,
            gain: p.gain,
        },
    );
    t.field_us = t0.elapsed().as_micros();

    // ---- the solid, and the silhouette as its apparent contour
    let t0 = Instant::now();
    let fbm = SphereFbm::new(&mut rng_blob, p.lump_octaves, p.lacunarity, p.gain);
    // In-plane semi-axes chosen so the geometric mean radius is exactly R0 at
    // any aspect, which keeps every other length's fraction of R0 meaningful.
    // The projected outline of an ellipsoid is the ellipse of its two IN-PLANE
    // axes whatever its depth - but the DISPLACED solid's is not, because the
    // maximisation favours the short axis, so the base is pre-stretched. See
    // `params::base_aspect` for the measurement and the mechanism.
    let base_aspect = crate::params::base_aspect(p.aspect);
    let semi_a = r0 * base_aspect.sqrt();
    let semi_b = r0 / base_aspect.sqrt();
    let solid = Solid {
        a: semi_a,
        b: semi_b,
        c: p.depth * semi_b,
        amp: p.lump_amp,
        lobes: p.lump_freq,
        floor: R_FLOOR_FRAC,
        warp: p.warp_amt,
        fbm,
    };

    // One pass over theta collecting both the projected radius and the tilt
    // that achieved it. The tilt is not a diagnostic: the depth map starts its
    // march from it, which is what makes the map's rim row land exactly on the
    // silhouette instead of on a second, slightly different evaluation of it.
    let mut r_raw = Vec::with_capacity(n);
    let mut u_star = Vec::with_capacity(n);
    for i in 0..n {
        let theta = TAU * i as f64 / n as f64;
        let (st, ct) = sin_cos(theta);
        let u2 = V2::new(ct, st);
        // The unwarped in-plane boundary estimate, for the field lookup - the
        // same quantity the 2D construction used, so a seed's lumps lean the
        // way they always did.
        let rho2 = semi_a * semi_b
            / ((semi_b * u2.x) * (semi_b * u2.x) + (semi_a * u2.y) * (semi_a * u2.y)).sqrt();
        // The warp moves the LOOKUP, not the geometry: R stays a scalar
        // function of direction, so the solid is star-shaped, so its
        // projection is too, and the theorem survives into 3D.
        let d2 = field.dir(u2 * (rho2 / r0));
        let (rho, u) = solid.silhouette(&perlin, theta, V3::new(d2.x, d2.y, 0.0));
        r_raw.push(rho);
        u_star.push(u);
    }
    let mut outer = Ring { c, r: r_raw };
    outer.clamp_min(R_FLOOR_FRAC * r0);
    outer.limit_log_lipschitz(p.slope_limit);
    for _ in 0..RING_SMOOTH_PASSES {
        smooth_ring(&mut outer.r);
    }

    // NORMALISE THE MEAN RADIUS. Taking a maximum over the tilt can only add
    // radius, never remove it, so the projection comes out 6.7% larger than
    // the ellipsoid it was built from (measured, 300 seeds at the defaults).
    // R0 is the unit every other length in the crate is a fraction of - the
    // ribbon's width, every vesicle radius, every clearance - so a body 6.7%
    // too big silently shrinks all of them against it: measured, the vesicle
    // coverage gate failed on 58 seeds in 200 for exactly that reason, the
    // packing having been asked to fill 14% more area with the same holes.
    //
    // A uniform scale is free of side effects here. It leaves
    // |d(ln r)/d(theta)| untouched, so the limiter's guarantee survives it;
    // it leaves the relative radial std and the bbox aspect untouched, so
    // neither silhouette gate moves; and it makes R0 mean R0 exactly, for
    // every seed, rather than in expectation.
    let mean_r = outer.r.iter().sum::<f64>() / n as f64;
    let solid_scale = if mean_r > 0.0 { r0 / mean_r } else { 1.0 };
    for r in &mut outer.r {
        *r *= solid_scale;
    }

    // The depth map, on the finished silhouette. Built from the ring AFTER
    // the limiter and the smoothing passes so the map's rim row and the
    // emitted outline are the same curve - a map built on the raw maximum
    // would have its rim a few units outside the drawn silhouette, and every
    // normal near the edge would be the normal of a surface that is not
    // there.
    let light = Light::new(p.light_dir, p.light_elev);

    // BAIL BEFORE PAYING FOR THE REST. Between them the three silhouette
    // gates reject a little over half of all rolls, and there is nothing a
    // ribbon, a facet tiling or a vesicle packing can do to rescue a
    // silhouette that has already failed - so building them first is 2 ms
    // spent on a document that will be thrown away. `check_all` re-runs
    // these on the finished scene; see `quality::check_silhouette`.
    let outer_probe = outer.points();
    quality::check_silhouette(&outer, &outer_probe, r0, p)?;

    let depth = DepthMap::build(
        &solid,
        &perlin,
        c,
        DEPTH_THETA,
        &outer.r,
        &u_star,
        solid_scale,
    );

    // ---- the ribbon, by radial offset with the slope correction
    let rp = smooth_wrapped(&outer.radial_derivative());
    let mut w = Vec::with_capacity(n);
    for (r, d) in outer.r.iter().zip(rp.iter()) {
        // A radial step of w produces a PERPENDICULAR band of only
        // w*cos(phi), so the radial step has to be inflated by 1/cos(phi).
        // Without this the band visibly thins on every steep flank, which is
        // the tell that says "generated".
        let slope = d / r;
        let k = (1.0 + slope * slope).sqrt().min(SLOPE_CORRECTION_CAP);
        w.push(w0 * k);
    }
    let mut inner = outer.offset_inward(&w, R_IN_FLOOR_FRAC * r0);
    erode_log_lipschitz(&mut inner.r, p.slope_limit);
    for _ in 0..INNER_SMOOTH_PASSES {
        smooth_ring(&mut inner.r);
    }
    // The clamp goes last so the band's measured constant width is the final
    // word: everything above it can only make the ribbon thicker.
    clamp_band(&mut inner.r, &outer.r, &w);

    // ---- contours
    let m_out = p.bezier_points as usize;
    let len_out = closed_len(&outer.points()).max(1e-9);
    let len_in = closed_len(&inner.points());
    let m_in =
        ((m_out as f64 * len_in / len_out).round() as i64).max(MIN_INNER_POINTS as i64) as usize;

    let mut clamped = 0usize;
    let (outer_pts, outer_cubics, c1) = fit_contour(&outer, m_out, p.cr_tension, false);
    // Reverse the POINTS, not the cubics: the inner subpath must wind the
    // other way or an even-odd fill turns the ribbon into a black blob.
    let (inner_pts, inner_cubics, c2) = fit_contour(&inner, m_in, p.cr_tension, true);
    clamped += c1 + c2;

    let rim = if p.inner_rim > 0.0 {
        let mut w2 = Vec::with_capacity(n);
        for &wi in &w {
            // The identical construction, one band further in: same slope
            // correction, so the second rim stays parallel to the first.
            w2.push(p.inner_rim * r0 * (wi / w0));
        }
        let mut rim_inner = inner.offset_inward(&w2, R_IN_FLOOR_FRAC * r0);
        erode_log_lipschitz(&mut rim_inner.r, p.slope_limit);
        clamp_band(&mut rim_inner.r, &inner.r, &w2);
        let (_, ro, c3) = fit_contour(&inner, m_in, p.cr_tension, false);
        let (_, ri, c4) = fit_contour(&rim_inner, m_in, p.cr_tension, true);
        clamped += c3 + c4;
        Some(Rim {
            outer_cubics: ro,
            inner_cubics: ri,
        })
    } else {
        None
    };
    t.ring_us = t0.elapsed().as_micros();

    let body_area = outer.area().abs();

    // ---- facets, on the ribbon's MIDLINE
    let t0 = Instant::now();
    let mut root_raw = Vec::with_capacity(n);
    for (i, &wi) in w.iter().enumerate() {
        let (s, cs) = sin_cos(outer.theta(i));
        // Half a ribbon width UNDER the silhouette, not proud of it.
        //
        // SPEC.md says to dilate the facet root to `r_i + 0.5*w_i` so "the
        // ribbon covers the overshoot". It cannot: the ribbon occupies
        // [r - w, r], so anything past r is outside it and nothing is painted
        // over it. Built that way the tiling showed as a bright orange fringe
        // all the way round the outside of the black outline - the most
        // visible defect in the first render of all twelve seeds.
        //
        // The midline is what the instruction was reaching for. The facets
        // then run half a band PAST the ribbon's inner edge and stop halfway
        // through the ink, so the ribbon drawn last covers the overshoot with
        // half a width to spare and no antialiasing seam can open up between
        // the tiling and the outline - which is the seam class the dilation
        // exists to close.
        root_raw.push(c + V2::new(cs, s) * (outer.r[i] - 0.5 * wi));
    }
    let root = resample_closed(&root_raw, (m_out / FACET_ROOT_FRAC).max(96));
    // An UNDERLAY in the mid tone. Two abutting antialiased facet edges
    // composite to about 87% coverage over whatever is behind them, so
    // without this every shared edge draws itself as a pale hairline. The
    // artwork solves the same problem by overlapping its facet paths (path11
    // is 13468 units as drawn against 8505 visible); one backing shape is the
    // same trick with less geometry.
    let mut root_cubics = catmull_rom_closed(&root, p.cr_tension);
    clamp_handles(&mut root_cubics);
    let (facets, cuts) = facet::build(
        &root,
        p,
        &field,
        &perlin,
        &depth,
        &light,
        &mut rng_facet,
        &mut rng_tone,
        c,
        r0,
        body_area,
    );
    t.facet_us = t0.elapsed().as_micros();

    // ---- vesicles, cleared against the INNER ring
    let t0 = Instant::now();
    let (vesicles, vesicles_drawn) = vesicle::pack(
        p,
        &inner,
        &field,
        &perlin,
        &depth,
        &light,
        &mut rng_ves,
        c,
        r0,
    );
    t.vesicle_us = t0.elapsed().as_micros();

    // ---- the relief marks, down the steepest creases
    //
    // Last, because a mark has to dodge the vesicles: a thin line running
    // into a gas hole reads as a crack through it rather than as a fold of
    // the surface. The root ring is the ribbon's midline, which is what the
    // rim test measures against - a mark starts inside the ink and emerges
    // from it, exactly as the artwork's does.
    let t0 = Instant::now();
    let root_ring = Ring {
        c,
        r: outer
            .r
            .iter()
            .zip(w.iter())
            .map(|(r, wi)| r - 0.5 * wi)
            .collect(),
    };
    let relief = relief::build(&root_ring, &depth, &light, &facets, &vesicles, p, r0, w0);
    t.relief_us = t0.elapsed().as_micros();

    // `clamped` and `segments` cover the RIBBON contours only - see the note
    // on the clamp gate in `quality`.
    let mut segments = outer_cubics.len() + inner_cubics.len();
    if let Some(r) = &rim {
        segments += r.outer_cubics.len() + r.inner_cubics.len();
    }

    let scene = Scene {
        seed,
        c,
        r0,
        w0,
        body_area,
        outer,
        inner,
        rp,
        w,
        outer_pts,
        inner_pts,
        outer_cubics,
        inner_cubics,
        rim,
        root_cubics,
        facets,
        vesicles,
        vesicles_drawn,
        relief,
        depth,
        light,
        cuts,
        ramp: Ramp::build(
            p.palette_hue,
            p.palette_sat,
            p.palette_light,
            p.tone_contrast,
        ),
        clamped,
        segments,
    };

    let t0 = Instant::now();
    let verdict = quality::check_all(&scene, p);
    t.quality_us = t0.elapsed().as_micros();
    verdict?;
    Ok((scene, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{cos, sin};

    /// The foundation's `sin_cos` must return (sin, cos) in that order. If it
    /// ever returns (cos, sin) the light direction mirrors and the facet
    /// shading reads as lit from above - a bug that looks like an artistic
    /// choice, which is the worst kind.
    #[test]
    fn sin_cos_is_sine_first() {
        for k in 0..8 {
            let a = 0.37 * k as f64;
            let (s, c) = sin_cos(a);
            assert!((s - sin(a)).abs() < 1e-12, "sin_cos().0 must be the sine");
            assert!((c - cos(a)).abs() < 1e-12, "sin_cos().1 must be the cosine");
        }
    }

    #[test]
    fn the_eroder_enforces_the_bound_and_only_lowers() {
        let n = 360;
        let mut r = vec![100.0; n];
        r[10] = 400.0;
        let before = r.clone();
        erode_log_lipschitz(&mut r, 1.10);
        let g = 1.10 * (TAU / n as f64);
        for i in 0..n {
            let j = (i + n - 1) % n;
            assert!(r[i] <= r[j] * (1.0 + g) * 1.0001, "bound broken at {i}");
            assert!(
                r[i] <= before[i] + 1e-12,
                "erosion must never raise a radius"
            );
        }
    }

    #[test]
    fn the_defaults_generate_a_logo() {
        let p = Params::default();
        let doc = generate_checked(&p).expect("the measured defaults must produce art");
        assert!(doc.starts_with("<svg"), "{}", &doc[..40.min(doc.len())]);
        assert!(doc.ends_with("</svg>\n"));
        assert!(doc.contains("id=\"outline\""));
        assert!(doc.contains("fill-rule=\"evenodd\""));
        assert!(doc.contains("id=\"facet-00\""));
        assert!(!doc.contains("stroke"), "the artwork has no strokes");
        assert!(!doc.contains("gradient"));
        assert!(!doc.contains("opacity"));
        assert!(!doc.contains("-0.000"), "a negative zero reached the file");
    }

    #[test]
    fn the_same_seed_gives_the_same_bytes() {
        for seed in [0u64, 1, 42, 9_999_999] {
            let p = Params {
                seed,
                ..Params::default()
            };
            let a = generate_checked(&p).expect("generates");
            let b = generate_checked(&p).expect("generates");
            assert_eq!(a, b, "seed {seed} is not reproducible");
        }
    }

    #[test]
    fn sequential_seeds_are_unrelated() {
        let a = generate_checked(&Params {
            seed: 0,
            ..Params::default()
        })
        .expect("generates");
        let b = generate_checked(&Params {
            seed: 1,
            ..Params::default()
        })
        .expect("generates");
        assert_ne!(a, b);
    }

    /// SPEC.md's re-roll formula aliased adjacent seeds; this is the
    /// regression test for that.
    #[test]
    fn a_run_of_seeds_is_a_run_of_different_logos() {
        let mut docs = Vec::new();
        for seed in 0..24u64 {
            let p = Params {
                seed,
                ..Params::default()
            };
            docs.push(generate_checked(&p).unwrap_or_else(|e| panic!("seed {seed}: {e}")));
        }
        for (i, a) in docs.iter().enumerate() {
            for (j, b) in docs.iter().enumerate().skip(i + 1) {
                assert_ne!(a, b, "seeds {i} and {j} produced the same logo");
            }
        }
    }

    #[test]
    fn reroll_ladders_never_alias() {
        let mut seen: Vec<u64> = Vec::new();
        for seed in 0..64u64 {
            for attempt in 0..=20u32 {
                seen.push(reroll_seed(seed, attempt));
            }
        }
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "two (seed, attempt) pairs collided");
    }

    #[test]
    fn a_run_of_seeds_all_pass_the_gates() {
        // The gates are only worth anything if the common case survives them.
        let mut ok = 0;
        for seed in 0..12u64 {
            let p = Params {
                seed,
                ..Params::default()
            };
            match generate_checked(&p) {
                Ok(_) => ok += 1,
                Err(e) => panic!("seed {seed}: {e}"),
            }
        }
        assert_eq!(ok, 12);
    }

    #[test]
    fn every_preset_generates() {
        // A RATE over a sweep, not one seed. One seed per preset is not a
        // test of a generator that re-rolls: it passes unchanged with 95% of
        // seeds failing, which is exactly what it did while `--preset feral`
        // was quietly exhausting its whole retry budget on one seed in
        // thirty.
        //
        // Measured over 120 seeds with the full retry budget, against the
        // same sweep run on the 2D generator this replaced:
        //
        // | preset | here | before | re-rolls here | before |
        // |---|---|---|---|---|
        // | anya  | 120/120 | 120/120 |  41 |  70 |
        // | tame  | 120/120 | 120/120 |  43 | 113 |
        // | feral | 120/120 | 111/120 | 252 | 626 |
        //
        // `feral` was the honest outlier for a long time - it asked for the
        // lumpiest body and nine big lumpy holes in it at once, and it sat
        // on the vesicle coverage floor. Fewer, larger holes drawn from a
        // Poisson with a per-roll size compensation fixed it: there is no
        // longer a setting of the dial that asks the packer for more than
        // the body can take.
        for name in Params::PRESETS {
            let want = 40;
            let mut ok = 0;
            let mut last = String::new();
            for seed in 0..40u64 {
                let p = Params {
                    seed,
                    ..Params::preset(name).expect("listed preset")
                };
                match generate_checked(&p) {
                    Ok(_) => ok += 1,
                    Err(e) => last = e.to_string(),
                }
            }
            assert!(
                ok >= want,
                "preset {name}: only {ok} of 40 seeds generated (want {want}); last error: {last}"
            );
        }
    }

    #[test]
    fn the_rim_adds_a_second_path() {
        let p = Params {
            inner_rim: 0.015,
            seed: 2,
            ..Params::default()
        };
        let doc = generate_checked(&p).expect("generates");
        assert!(doc.contains("id=\"rim\""));
    }

    #[test]
    fn changing_the_vesicles_leaves_the_silhouette_alone() {
        // The point of five streams: the outline must be byte-identical when
        // only a vesicle parameter moves. Some seeds legitimately fail a gate
        // at one of the two counts, so take the first seed that passes both.
        //
        // 7 rather than 9 for the second count. `vesicle_count` is the MEAN
        // of a Poisson draw now, and the draw reaches twice the mean plus
        // four - at a mean of 9 that is 22 holes, which the packer cannot
        // place in a body this size, so no seed passed both and the test
        // failed for a reason that had nothing to do with what it checks.
        // The property is the same at any feasible pair, and the draw still
        // changes between them, which is the thing that must not reach the
        // silhouette.
        let mut checked = false;
        for seed in 0..40u64 {
            let a = Params {
                seed,
                ..Params::default()
            };
            let b = Params {
                seed,
                vesicle_count: 7,
                ..Params::default()
            };
            if let (Ok((sa, _)), Ok((sb, _))) = (attempt_once(&a, seed), attempt_once(&b, seed)) {
                assert_eq!(sa.outer.r.len(), sb.outer.r.len());
                for (x, y) in sa.outer.r.iter().zip(sb.outer.r.iter()) {
                    assert_eq!(x, y, "the silhouette moved when only the holes changed");
                }
                checked = true;
                break;
            }
        }
        assert!(checked, "no seed in 0..40 produced two clean logos");
    }
}

#[cfg(test)]
mod relief_marks {
    use super::*;
    use crate::math::polyline_len;

    /// The marks reproduce what Part 1 measures of Anya's: a stroke 0.535 of
    /// the mean radius long and 0.58 of the outline's width at the waist,
    /// growing OUT OF the outline ink and heading into the body rather than
    /// along the rim.
    ///
    /// Checked on the finished scene rather than on the emitted text,
    /// because the property that matters - that the rim end is buried in the
    /// ribbon, so the mark reads as a branch of the outline rather than as a
    /// scratch beside it - is about where the mark sits relative to geometry
    /// the document does not label.
    #[test]
    fn the_marks_grow_out_of_the_outline_and_cut_inward() {
        let p = Params::default();
        let mut seen = 0;
        let mut with_mark = 0;
        let mut lens: Vec<f64> = Vec::new();
        let mut widths: Vec<f64> = Vec::new();
        // Doubled when the proportional clearance and the balance gate took
        // the single-attempt clean rate from about 75% to about 45%.
        for seed in 0..120u64 {
            let Ok((scene, _)) = attempt_once(&p, seed) else {
                continue;
            };
            seen += 1;
            if !scene.relief.is_empty() {
                with_mark += 1;
            }
            for m in &scene.relief {
                let end = m.spine[0];
                let d = end - scene.c;
                let rim = scene.outer.radius_toward(d);
                // Inside the ink band [r - w, r], with the facet root's
                // midline as the target.
                assert!(
                    d.len() >= rim - 1.25 * scene.w0 && d.len() <= rim + 0.1 * scene.w0,
                    "seed {seed}: the mark starts {:.1} from the centre, ink band is {:.1}..{:.1}",
                    d.len(),
                    rim - scene.w0,
                    rim
                );
                // Heading into the body, not along the rim. Anya's leaves
                // 29 degrees off radial; the rule is 55.
                let inward = (scene.c - end).norm();
                let heading = (m.spine[1] - m.spine[0]).norm();
                assert!(
                    heading.dot(inward) >= 0.573,
                    "seed {seed}: the mark runs along the rim, not into the body"
                );
                // Length is capped by `relief_len`, not set by it - the
                // mark stops where its crease fades.
                let len = polyline_len(&m.spine);
                let cap = p.relief_len * scene.r0;
                // The cap bounds the TRACE; the anchor back out to the ink
                // adds up to `MAX_ANCHOR_FRAC` ribbon widths on top.
                assert!(
                    len > 0.15 * scene.r0 && len <= cap + 4.5 * scene.w0,
                    "seed {seed}: length {len:.1} against the cap {cap:.1}"
                );
                // 0.76 of the outline is the CENTRE of the band, not the
                // whole of it: a sharper fold is drawn heavier.
                let ratio = m.width / scene.w0;
                assert!(
                    (ratio - 0.759).abs() < 0.759 * 0.24,
                    "seed {seed}: the mark is {ratio:.3} of the outline, band is 0.58..0.94"
                );
                lens.push(len / scene.r0);
                widths.push(ratio);
            }
        }
        assert!(seen > 40, "too few scenes to conclude anything: {seen}");
        // The feature has to actually fire. A mark needs a cut that reaches
        // the ink heading inward, past a tone step and clear of the holes,
        // so it is not guaranteed on every seed - but it must be the norm,
        // or the default is quietly drawing nothing.
        assert!(
            with_mark * 4 >= seen * 3,
            "only {with_mark} of {seen} scenes got a relief mark"
        );

        // AND THEY MUST DIFFER. The first design took the mark from a facet
        // cut and truncated it at a constant, which gave 14 of 16 logos a
        // stroke of exactly 164.2 units and 9.41 width - the same object,
        // rotated. Tracing the rock's own crease is the fix, and a spread
        // is the only evidence that it is working.
        let spread = |v: &[f64]| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt() / m
        };
        assert!(
            spread(&lens) > 0.12,
            "the marks are all the same length: relative spread {:.3}",
            spread(&lens)
        );
        assert!(
            spread(&widths) > 0.04,
            "the marks are all the same width: relative spread {:.3}",
            spread(&widths)
        );
        let wm = widths.iter().sum::<f64>() / widths.len() as f64;
        assert!(
            (wm - 0.759).abs() < 0.10,
            "the mean nominal width is {wm:.3} of the outline, want 0.76"
        );
    }
}

#[cfg(test)]
mod amplitude {
    use super::*;
    use crate::math::PI;
    use crate::noise::SphereFbm;

    /// The realised radial statistics of one seed's finished silhouette:
    /// relative radial std, Rmin/Rmean, Rmax/Rmean. Mirrors
    /// `attempt_once`'s ring stage exactly, so what it measures is what the
    /// outline gate sees.
    fn realised(seed: u64, base_aspect: f64, p: &Params) -> (f64, f64, f64, f64) {
        let r0 = p.r0;
        let n = p.ring_samples as usize;
        let mut rng_blob = Pcg32::new(seed ^ stream::BLOB, stream::BLOB);
        let mut rng_misc = Pcg32::new(seed ^ stream::MISC, stream::MISC);
        let perlin = Perlin::new(&mut rng_misc);
        let field = Field::new(
            &mut rng_misc,
            FieldCfg {
                freq: p.field_freq,
                angle_range: p.field_angle_range,
                bias: p.field_bias,
                quantize: p.field_quantize,
                octaves: FIELD_OCTAVES,
                lacunarity: p.lacunarity,
                gain: p.gain,
            },
        );
        let fbm = SphereFbm::new(&mut rng_blob, p.lump_octaves, p.lacunarity, p.gain);
        let semi_a = r0 * base_aspect.sqrt();
        let semi_b = r0 / base_aspect.sqrt();
        let solid = Solid {
            a: semi_a,
            b: semi_b,
            c: p.depth * semi_b,
            amp: p.lump_amp,
            lobes: p.lump_freq,
            floor: R_FLOOR_FRAC,
            warp: p.warp_amt,
            fbm,
        };
        let mut r = Vec::with_capacity(n);
        for i in 0..n {
            let theta = TAU * i as f64 / n as f64;
            let (st, ct) = sin_cos(theta);
            let u2 = V2::new(ct, st);
            let rho2 = semi_a * semi_b
                / ((semi_b * u2.x) * (semi_b * u2.x) + (semi_a * u2.y) * (semi_a * u2.y)).sqrt();
            let d2 = field.dir(u2 * (rho2 / r0));
            r.push(solid.silhouette(&perlin, theta, V3::new(d2.x, d2.y, 0.0)).0);
        }
        let mut ring = Ring {
            c: V2::new(0.0, 0.0),
            r,
        };
        ring.clamp_min(R_FLOOR_FRAC * r0);
        ring.limit_log_lipschitz(p.slope_limit);
        for _ in 0..RING_SMOOTH_PASSES {
            smooth_ring(&mut ring.r);
        }
        let mean = ring.r.iter().sum::<f64>() / n as f64;
        let std = (ring
            .r
            .iter()
            .map(|x| (x / mean - 1.0) * (x / mean - 1.0))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        // Against the BASE ELLIPSE rather than against the mean radius.
        let rho = |i: usize| {
            let theta = TAU * i as f64 / n as f64;
            let (st, ct) = sin_cos(theta);
            semi_a * semi_b / ((semi_b * ct) * (semi_b * ct) + (semi_a * st) * (semi_a * st)).sqrt()
        };
        let ratios: Vec<f64> = (0..n).map(|i| ring.r[i] / rho(i)).collect();
        let rm = ratios.iter().sum::<f64>() / n as f64;
        let rel = (ratios
            .iter()
            .map(|x| (x / rm - 1.0) * (x / rm - 1.0))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        // The ellipse's own contribution, for the quadrature alternative.
        let em = (0..n).map(rho).sum::<f64>() / n as f64;
        let estd = ((0..n)
            .map(|i| (rho(i) / em - 1.0) * (rho(i) / em - 1.0))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        let quad = (std * std - estd * estd).max(0.0).sqrt();
        let area = ring.area().abs() / (PI * r0 * r0);
        (std, rel, quad, area)
    }

    fn mean_std(base_aspect: f64, seeds: std::ops::Range<u64>) -> f64 {
        let p = Params::default();
        let n = (seeds.end - seeds.start) as f64;
        seeds.map(|s| realised(s, base_aspect, &p).0).sum::<f64>() / n
    }

    #[test]
    fn lump_amp_is_the_realised_outline_amplitude() {
        let p = Params::default();
        let want = p.lump_amp;
        let base = crate::params::base_aspect(p.aspect);
        for seeds in [0..400u64, 400..800] {
            let got = mean_std(base, seeds.clone());
            assert!(
                (got - want).abs() < 0.012,
                "seeds {seeds:?}: realised {got:.4} against lump_amp {want}"
            );
        }
    }

    /// Why that needs no correction, when for a while it did: the amplitude
    /// calibration and the aspect calibration turned out to be the same one.
    ///
    /// Taking a maximum over the out-of-plane tilt both inflates the radius
    /// and squares the shape up, and it does the first BECAUSE of the second.
    /// The wider the window of tilt competing for the maximum, the more
    /// displacement it catches and the more radius it adds - and that window
    /// is widest along the SHORT in-plane axis, which is exactly why the
    /// aspect drops. Pre-stretching the base ellipsoid to fix the aspect
    /// narrows the window, so it removes the amplitude loss as a side effect.
    ///
    /// Measured: on the unstretched base the realised amplitude is 0.193
    /// against the 0.22 asked for, and a gain of 1.40 was carried for a
    /// while to buy it back; on the stretched base it is 0.220 and no gain
    /// is needed. Kept as a test because a coupling like this is the kind of
    /// thing that gets re-discovered the hard way - if a future change to
    /// the solid breaks it, the test above fails and this one says where to
    /// look.
    #[test]
    fn the_amplitude_loss_was_the_aspect_loss() {
        let p = Params::default();
        let unstretched = mean_std(p.aspect, 0..400);
        let stretched = mean_std(crate::params::base_aspect(p.aspect), 0..400);
        assert!(
            unstretched < 0.93 * p.lump_amp,
            "the unstretched base no longer loses amplitude: {unstretched:.4}"
        );
        assert!(
            stretched > unstretched + 0.015,
            "pre-stretching no longer recovers it: {stretched:.4} against {unstretched:.4}"
        );
    }
}

#[cfg(test)]
mod tonecal {
    use super::*;

    /// The artwork's own tone axis: the unit vector from the dark tone's
    /// visible-pixel centroid (276.9, 135.3) to the light tone's
    /// (179.0, 244.9), which SPEC.md Part 1 gives as (-0.666, +0.746).
    const U: V2 = V2 {
        x: -0.666,
        y: 0.746,
    };

    /// Area-weighted mean and quartiles of each tone's position along `U`,
    /// with the body's extent along `U` normalised to 0..1 - the quantity
    /// Part 1 measures as 0.302 / 0.486 / 0.695 with interquartile ranges
    /// 0.14-0.45, 0.31-0.68 and 0.51-0.92.
    fn tone_positions(p: &Params, seeds: u64) -> [(f64, f64, f64); 3] {
        // (position, area) per tone, pooled over seeds.
        let mut pool: [Vec<(f64, f64)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for seed in 0..seeds {
            let q = Params { seed, ..p.clone() };
            let Ok((scene, _)) = attempt_once(&q, seed) else {
                continue;
            };
            let proj: Vec<f64> = scene.outer_pts.iter().map(|q| U.dot(*q)).collect();
            let lo = proj.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = proj.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let span = (hi - lo).max(1e-9);
            for f in &scene.facets {
                pool[f.tone].push(((U.dot(f.centroid) - lo) / span, f.area));
            }
        }
        let mut out = [(0.0, 0.0, 0.0); 3];
        for (t, v) in pool.iter_mut().enumerate() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            let total: f64 = v.iter().map(|x| x.1).sum();
            if total <= 0.0 {
                continue;
            }
            let mean = v.iter().map(|x| x.0 * x.1).sum::<f64>() / total;
            let at = |frac: f64| -> f64 {
                let want = frac * total;
                let mut acc = 0.0;
                for &(pos, a) in v.iter() {
                    acc += a;
                    if acc >= want {
                        return pos;
                    }
                }
                v.last().map_or(0.0, |x| x.0)
            };
            out[t] = (at(0.25), mean, at(0.75));
        }
        out
    }

    /// The defaults put the three tones where the artwork puts them.
    ///
    /// This is the calibration `light_dir` and `light_elev` were set from,
    /// kept as an assertion. SPEC.md Part 1 measures the artwork's tones by
    /// projecting every orange pixel onto `U` and normalising the body's
    /// extent to 0..1, which gives mean positions 0.302 / 0.486 / 0.695 with
    /// interquartile ranges 0.14-0.45, 0.31-0.68 and 0.51-0.92. The same
    /// measurement on 120 generated logos is what the tolerance below is
    /// against.
    ///
    /// Two findings came out of running it, and both are in the defaults:
    ///
    /// - `light_dir` is 2.30, not SPEC.md's 1.90. 2.30 is the measurement -
    ///   the axis through the three tones' own centroids - while 1.90 was
    ///   inferred from where the lightest facets sit. At 1.90 the dark tone
    ///   lands at 0.399 against the measured 0.302; at 2.30 it lands at
    ///   0.368, and the other two improve with it.
    ///
    /// - `tone_noise` stays at 0.35 even though less of it reads as more
    ///   three-dimensional by eye. The artwork's tones overlap by a full
    ///   quartile, and matching that overlap is what makes a generated rock
    ///   belong to the same family. At 0.20 the three means land 0.013,
    ///   0.058 and 0.078 off; at 0.35 they land 0.066, 0.040 and 0.015 off,
    ///   and the interquartile WIDTHS match too - 0.30 against the measured
    ///   0.31 for the shadow, where 0.20 gives 0.22.
    #[test]
    fn the_tones_land_where_the_artwork_puts_them() {
        let want = [0.302, 0.486, 0.695];
        let got = tone_positions(&Params::default(), 120);
        for (t, w) in want.iter().enumerate() {
            let (q1, mean, q3) = got[t];
            assert!(
                (mean - w).abs() < 0.09,
                "tone {t}: mean {mean:.3} against the artwork's {w:.3}"
            );
            assert!(q1 < mean && mean < q3, "tone {t}: quartiles out of order");
        }
        assert!(
            got[0].1 < got[1].1 && got[1].1 < got[2].1,
            "the three tones must lie along the light axis in order: {:?}",
            [got[0].1, got[1].1, got[2].1]
        );
    }
}

#[cfg(test)]
mod contouring {
    use super::*;

    /// A hole lies ON the rock, so the steeper the surface under it, the
    /// more it is foreshortened and the more its long axis turns to follow
    /// the local contour. That is the whole of what makes the holes read as
    /// sitting on a three-dimensional thing rather than being painted over
    /// a picture of one, and it is a MONOTONE claim, which is what this
    /// checks: both effects have to grow as the surface tilts away.
    ///
    /// Measured over 120 seeds, bucketed by the normal's `z` under each
    /// hole:
    ///
    /// | `n.z` >= | holes | ellipse aspect | \|axis . contour\| |
    /// |---|---|---|---|
    /// | 0.95 | 146 | 1.68 | 0.648 |
    /// | 0.80 | 247 | 1.72 | 0.699 |
    /// | 0.60 | 106 | 2.01 | 0.770 |
    /// | 0.00 |  21 | 2.52 | 0.869 |
    ///
    /// The face-on bucket is the control: there the surface has no contour
    /// to offer and the flow field still sets the axis, which is why its
    /// alignment sits near the 0.64 a free axis would give rather than at
    /// zero.
    #[test]
    fn a_hole_follows_the_surface_it_lies_on() {
        let p = Params::default();
        // (sum aspect, sum alignment, n) for face-on and for tilted.
        let mut flat = (0.0, 0.0, 0.0f64);
        let mut steep = (0.0, 0.0, 0.0f64);
        // Doubled when the proportional clearance and the balance gate took
        // the single-attempt clean rate from about 75% to about 45%.
        for seed in 0..240u64 {
            let Ok((sc, _)) = attempt_once(&p, seed) else {
                continue;
            };
            for v in &sc.vesicles {
                let n = sc.depth.sample(v.centre).1;
                let tilt = n.xy();
                if tilt.len2() <= 1e-12 {
                    continue;
                }
                let align = v.axis.dot(tilt.perp().norm()).abs();
                let bucket = if n.z >= 0.90 {
                    &mut flat
                } else if n.z < 0.70 {
                    &mut steep
                } else {
                    continue;
                };
                bucket.0 += v.aspect;
                bucket.1 += align;
                bucket.2 += 1.0;
            }
        }
        assert!(
            flat.2 > 40.0 && steep.2 > 40.0,
            "too few holes in a bucket to conclude anything: {} flat, {} steep",
            flat.2,
            steep.2
        );
        let (fa, fl) = (flat.0 / flat.2, flat.1 / flat.2);
        let (sa, sl) = (steep.0 / steep.2, steep.1 / steep.2);
        assert!(
            sa > fa + 0.15,
            "a hole on a steep patch is not more elongated: {sa:.2} against {fa:.2}"
        );
        assert!(
            sl > fl + 0.06,
            "a hole on a steep patch does not follow the contour: {sl:.3} against {fl:.3}"
        );
    }

    /// The count is a draw, not a constant - and it has to actually vary,
    /// or the Poisson machinery is an expensive way to write `5`.
    #[test]
    fn the_hole_count_varies() {
        let p = Params::default();
        let mut counts = Vec::new();
        // Doubled when the proportional clearance and the balance gate took
        // the single-attempt clean rate from about 75% to about 45%.
        for seed in 0..240u64 {
            if let Ok((sc, _)) = attempt_once(&p, seed) {
                counts.push(sc.vesicles_drawn);
            }
        }
        assert!(counts.len() > 60, "too few clean rolls: {}", counts.len());
        let lo = counts.iter().copied().min().expect("non-empty");
        let hi = counts.iter().copied().max().expect("non-empty");
        assert!(
            lo >= 3,
            "the draw went below the floor that keeps it reading as scoria: {lo}"
        );
        assert!(hi >= lo + 3, "the count barely varies: {lo}..{hi}");
        let mean = counts.iter().map(|c| f64::from(*c)).sum::<f64>() / counts.len() as f64;
        let want = f64::from(p.vesicle_count);
        // Truncation at three pulls the mean up a little, and the gates
        // pull it further: a low draw scales its radii up by `sqrt(mean /
        // drawn)` and, held a `vesicle_edge_ratio` of their own radius off
        // the ribbon, those big holes are the ones the packer drops, so
        // low draws fail the coverage gate more often than high ones.
        // Measured among clean single attempts over 400 seeds: 5.71 before
        // the proportional clearance, 5.93 after (6.21 with the balance gate
        // flat at 0.20, which is why that gate scales with the count). The
        // ceiling is the measured value plus a little; it is a record of
        // how far the gates move the parameter, not a licence to let them
        // move it further.
        assert!(
            mean > want - 0.3 && mean < want + 1.2,
            "the drawn mean is {mean:.2} against the parameter's {want}"
        );
    }
}
