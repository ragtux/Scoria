//! The silhouette, the ribbon, and the pipeline that assembles a whole logo.
//!
//! The blob is a POLAR radius with a domain-warped periodic fBm over an
//! elliptical base - not a noise-advected circle and not a traced flow curve
//! closed back on itself. Both of those can self-intersect, and neither can be
//! guaranteed closed without a seam. The polar form buys a theorem: a
//! single-valued `r(theta) > 0` is star-shaped about its centre, so every ray
//! from the centre meets the curve exactly once and the curve cannot cross
//! itself. The ribbon then falls out as a scalar subtraction `r - w(theta)`
//! rather than a general polygon offset, which is the error-prone part of
//! every other approach.
//!
//! Construction, in canonical space (R0 = 300):
//!
//! 1. base ellipse, semi-axes `R0*sqrt(aspect)` and `R0/sqrt(aspect)`;
//! 2. 720 samples of theta;
//! 3. domain warp - the flow field moves the NOISE LOOKUP, never the geometry,
//!    which is what keeps `r` single-valued;
//! 4. `r = rho * (1 + lump_amp * F)`;
//! 5. the log-Lipschitz limiter;
//! 6. uniform arclength resample, then Catmull-Rom to cubics.

use std::time::Instant;

use crate::bezier::{catmull_rom_closed, clamp_handles, max_turn, Cubic};
use crate::facet::{self, CutReport, Facet};
use crate::field::{Field, FieldCfg};
use crate::math::{closed_len, resample_closed, sin_cos, PI, TAU, V2};
use crate::noise::Perlin;
use crate::palette::Ramp;
use crate::ring::Ring;
use crate::rng::{splitmix64, stream, Pcg32};
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
/// Noise-space offset range for an octave, so octaves are decorrelated.
const OCTAVE_OFFSET: f64 = 4096.0;
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
    /// What the subdivision did.
    pub cuts: CutReport,
    /// The three tones.
    pub ramp: Ramp,
    /// Handles the Bezier clamp caught across the whole logo.
    pub clamped: usize,
    /// Cubics emitted across the whole logo.
    pub segments: usize,
}

/// Periodic fBm over a circle in noise space.
///
/// Sampling the noise along a CIRCLE rather than a line in theta is the whole
/// trick for a seamless closed silhouette: periodicity is then exact and free,
/// for every octave, at any frequency. Cross-fading the ends of a linear
/// sample leaves a crease at theta = 0 that no amount of blending removes.
///
/// This is deliberately not [`crate::noise::Fbm`]: the warp must be added ONCE
/// and unscaled, so it shears the low octaves most. `Fbm` scales its whole
/// input per octave, which would multiply the warp by `lacunarity^o` and
/// decorrelate the fine octaves between adjacent theta - exactly the crinkle
/// the measured spectrum says the artwork does not have.
struct PeriodicFbm {
    offsets: Vec<V2>,
    lacunarity: f64,
    gain: f64,
}

impl PeriodicFbm {
    /// Draw one noise-space offset per octave.
    fn new(rng: &mut Pcg32, octaves: u32, lacunarity: f64, gain: f64) -> PeriodicFbm {
        let mut offsets = Vec::with_capacity(octaves as usize);
        for _ in 0..octaves.max(1) {
            offsets.push(V2::new(
                rng.range(-OCTAVE_OFFSET, OCTAVE_OFFSET),
                rng.range(-OCTAVE_OFFSET, OCTAVE_OFFSET),
            ));
        }
        PeriodicFbm {
            offsets,
            lacunarity,
            gain,
        }
    }

    /// Unit-RMS sum over octaves at angle `(ct, st)`.
    ///
    /// `freq` is in LOBES PER TURN. The sample walks a circle of radius
    /// `freq / PI` in noise space, because a circle of radius f has
    /// circumference 2*PI*f and a Perlin lattice crossing is half a period,
    /// so it meets about PI*f periods on the way round. Passing `lump_freq`
    /// as the radius directly - which is how the formula reads - gives eight
    /// lobes where the parameter says 2.6, and leaves the artwork's measured
    /// dominant harmonic at k=3 completely empty.
    fn at(&self, perlin: &Perlin, ct: f64, st: f64, lobes: f64, warp: V2) -> f64 {
        let mut f = lobes / PI;
        let mut a = 1.0;
        let mut acc = 0.0;
        let mut sum = 0.0;
        for off in &self.offsets {
            acc += a * perlin.at(ct * f + warp.x + off.x, st * f + warp.y + off.y);
            sum += a;
            a *= self.gain;
            f *= self.lacunarity;
        }
        unit_fbm(acc / sum)
    }
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

    // ---- the silhouette
    let t0 = Instant::now();
    let fbm = PeriodicFbm::new(&mut rng_blob, p.lump_octaves, p.lacunarity, p.gain);
    // Semi-axes chosen so the geometric mean radius is exactly R0 at any
    // aspect, which keeps every other length's fraction of R0 meaningful.
    let semi_a = r0 * p.aspect.sqrt();
    let semi_b = r0 / p.aspect.sqrt();
    let mut outer = Ring::build(c, n, |_theta, u| {
        let rho = semi_a * semi_b
            / ((semi_b * u.x) * (semi_b * u.x) + (semi_a * u.y) * (semi_a * u.y)).sqrt();
        // The warp moves the LOOKUP, not the point: r stays a scalar function
        // of theta, and the star-shape theorem survives.
        let d = field.dir(u * (rho / r0));
        let f = fbm.at(&perlin, u.x, u.y, p.lump_freq, d * p.warp_amt);
        rho * (1.0 + p.lump_amp * f)
    });
    outer.clamp_min(R_FLOOR_FRAC * r0);
    outer.limit_log_lipschitz(p.slope_limit);
    for _ in 0..RING_SMOOTH_PASSES {
        smooth_ring(&mut outer.r);
    }

    // BAIL BEFORE PAYING FOR THE REST. Between them the three silhouette
    // gates reject a little over half of all rolls, and there is nothing a
    // ribbon, a facet tiling or a vesicle packing can do to rescue a
    // silhouette that has already failed - so building them first is 2 ms
    // spent on a document that will be thrown away. `check_all` re-runs
    // these on the finished scene; see `quality::check_silhouette`.
    let outer_probe = outer.points();
    quality::check_silhouette(&outer, &outer_probe, r0, p)?;

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
        &mut rng_facet,
        &mut rng_tone,
        c,
        r0,
        body_area,
    );
    t.facet_us = t0.elapsed().as_micros();

    // ---- vesicles, cleared against the INNER ring
    let t0 = Instant::now();
    let vesicles = vesicle::pack(p, &inner, &field, &perlin, &mut rng_ves, c, r0);
    t.vesicle_us = t0.elapsed().as_micros();

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
    fn periodic_fbm_is_seamless() {
        let mut rng = Pcg32::new(1, stream::BLOB);
        let perlin = Perlin::new(&mut rng);
        let fbm = PeriodicFbm::new(&mut rng, 4, 2.03, 0.45);
        let at = |theta: f64| {
            let (s, c) = sin_cos(theta);
            fbm.at(&perlin, c, s, 2.6, V2::new(0.0, 0.0))
        };
        // theta and theta + TAU are the same point on the circle, exactly.
        let a = at(0.0);
        let b = at(TAU);
        assert!((a - b).abs() < 1e-9, "{a} vs {b}");
        // ... and the neighbourhood of the seam is as smooth as anywhere else.
        let step = 1e-3;
        let seam = (at(step) - at(-step)).abs();
        let elsewhere = (at(1.0 + step) - at(1.0 - step)).abs();
        assert!(seam < 10.0 * elsewhere + 1e-6, "a crease at theta = 0");
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
        for name in Params::PRESETS {
            let mut p = Params::preset(name).expect("listed preset");
            p.seed = 4;
            generate_checked(&p).unwrap_or_else(|e| panic!("preset {name}: {e}"));
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
        let mut checked = false;
        for seed in 0..24u64 {
            let a = Params {
                seed,
                ..Params::default()
            };
            let b = Params {
                seed,
                vesicle_count: 9,
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
        assert!(checked, "no seed in 0..24 produced two clean logos");
    }
}
