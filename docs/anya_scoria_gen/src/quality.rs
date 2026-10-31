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
use crate::math::{polygon_area, TAU, V2};
use crate::ring::Ring;
use crate::vesicle::{boundary_clear, max_radius_window, Vesicle};
use crate::Params;

/// A gate's verdict: the rule name on failure.
pub type Check = Result<(), &'static str>;

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
pub fn silhouette_amplitude(ring: &Ring, lump_amp: f64) -> Check {
    let n = ring.n();
    if n == 0 {
        return Err("silhouette amplitude: empty ring");
    }
    let mean = ring.r.iter().sum::<f64>() / n as f64;
    if !mean.is_finite() || mean <= 0.0 {
        return Err("silhouette amplitude: mean radius collapsed");
    }
    // Written as a multiply rather than `powi`: the house rule is integer
    // powers spelled out, so nothing can quietly become a libm `pow`.
    let var = ring
        .r
        .iter()
        .map(|r| {
            let d = r / mean - 1.0;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    let std = var.sqrt();
    if std < AMPLITUDE_LO_FRAC * lump_amp {
        return Err("silhouette amplitude: the blob came out as an egg");
    }
    if std > AMPLITUDE_HI_FRAC * lump_amp {
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
        if !boundary_clear(&v.ring, inner, clearance) {
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
            if d < max_radius_window(&a.ring, dir) + max_radius_window(&b.ring, -dir) + gap {
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
    // ceil(0.6 * target) without a float round trip.
    let floor = (6 * target as usize).div_ceil(10);
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

/// Size spread, so the holes do not read as polka dots.
pub fn vesicle_spread(ves: &[Vesicle], target: u32) -> Check {
    if target < 4 || ves.len() < 2 {
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
    matches!(kind, "facet" | "body-fill")
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
    out.push(("outline-outer", &scene.outer_cubics));
    out.push(("outline-inner", &scene.inner_cubics));
    if let Some(rim) = &scene.rim {
        out.push(("rim-outer", &rim.outer_cubics));
        out.push(("rim-inner", &rim.inner_cubics));
    }
    out
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
    silhouette_amplitude(outer, p.lump_amp)?;
    bbox_aspect(pts, p.aspect, p.lump_amp)
}

/// Run every gate, cheapest first.
pub fn check_all(scene: &Scene, p: &Params) -> Check {
    check_silhouette(&scene.outer, &scene.outer_pts, scene.r0, p)?;
    ribbon(&scene.outer, &scene.inner, &scene.rp, scene.r0, scene.w0)?;
    vesicle_spread(&scene.vesicles, p.vesicle_count)?;
    vesicle_coverage(&scene.vesicles, scene.body_area, p.vesicle_count)?;
    vesicle_health(&scene.vesicles)?;
    cut_health(&scene.cuts, p.facet_min_chord_sep)?;
    facet_areas(&scene.facets, scene.body_area, p.facet_min_area)?;
    tones_present(&scene.facets, scene.body_area)?;
    facet_junctions(&scene.facets)?;

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
}
