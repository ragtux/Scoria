//! The interior relief marks: thin tapered lines that cut into the shape.
//!
//! The artwork has exactly one, and it is the only interior line work in the
//! drawing. Measured off `../img/anya_v0_base.svg`, path23:
//!
//! | quantity | measured | as a fraction |
//! |---|---|---|
//! | centreline length | 90.7 units | 0.535 of the mean radius |
//! | waist width | 5.72 units | 0.0337 of R, 0.58 of the outline ribbon |
//! | width at 5% and 95% of the length | 4.23 and 4.29 units | 0.74 of the waist |
//! | width at both ends | 0.0 | a point |
//! | distance from one end to the outline ink | 0.05 units | it TOUCHES |
//! | distance from the other end to it | 64.6 units | free inside the body |
//!
//! So the mark is a stroke of constant width with both ends rounded off to a
//! point, one end merging into the outline and the other dying in open
//! orange. That is what reads as depth: the eye takes it for a branch of the
//! outline folding back over the body, which means a surface that turns away
//! and comes back - a fold.
//!
//! The 0.74 at 5% pins the taper exactly. Over a run of 10% of the length,
//! a linear ramp would give 0.5 there and a smoothstep 0.68; `x * (2 - x)`,
//! the ellipse, gives 0.75 at both ends against the measured 0.74 and 0.75.
//! Two independent measurements agreeing to one percent is a shape, not a
//! coincidence, so [`TAPER_FRAC`] and the parabolic profile are measured
//! values rather than taste.
//!
//! # Where a mark goes
//!
//! Not anywhere: down a CREASE, and only one that reaches the rim. The
//! candidates are the facet cuts, which already run along the solid's
//! iso-brightness lines (see [`crate::solid::DepthMap::iso_dir`]) and are
//! already validated as non-self-intersecting curves inside the body. A cut
//! earns a mark by the mean steepness of the brightness across it: a steep
//! brightness ramp is a place where the surface tilts hard over a short
//! distance, which is a fold, and drawing the fold is drawing the depth.
//! Because the cut is also a tone boundary, the line and the tone step land
//! on the same curve and reinforce each other instead of competing.

use crate::bezier::{catmull_rom_closed, clamp_handles, self_intersects, Cubic};
use crate::facet::Facet;
use crate::math::{point_in_polygon, polyline_len, V2};
use crate::params::Params;
use crate::ring::Ring;
use crate::solid::{DepthMap, Light, GRAD_H_FRAC};
use crate::vesicle::Vesicle;

/// Fraction of the length each end tapers over. Measured; see the module docs.
const TAPER_FRAC: f64 = 0.10;

/// Spacing of the centreline samples, as a fraction of R0.
///
/// The cut this is cut from was traced at 0.01*R0 per Euler step, so resampling
/// at 0.015 neither invents detail nor throws any away, and it keeps the
/// offset's per-sample turn small enough for the curvature guard to pass on
/// anything that is not genuinely hooked.
const SAMPLE_FRAC: f64 = 0.015;

/// How many half-widths of curvature radius the centreline must have.
///
/// Offsetting a polyline by `h` folds the inner side over itself wherever the
/// local curvature radius drops below `h`. 2.5 leaves margin for the
/// Catmull-Rom fit, which can bulge a handle slightly past the polyline.
const MIN_CURVE_RATIO: f64 = 2.5;

/// Minimum clearance from a mark to a vesicle, in half-widths.
///
/// A relief line that runs into a gas hole reads as a crack through it rather
/// than as a fold of the surface, and the artwork has nothing like it: its
/// mark's nearest vesicle is 35 units away, six waists.
const VESICLE_CLEAR: f64 = 2.0;

/// How close two marks may come, as a fraction of R0.
///
/// Two creases side by side read as hatching, which is a different drawing
/// language from a flat three-tone mark.
const MARK_SEP_FRAC: f64 = 0.30;

/// cos(55 degrees): how far off inward-radial a mark may leave the rim.
///
/// Without this the pass picks marks that run TANGENTIALLY, just inside the
/// outline, because that is where the brightness changes fastest - and a
/// line hugging the rim reads as a nick in the ink rather than as a fold of
/// the surface. Anya's leaves the rim 29 degrees off radial: measured, from
/// (292.9, 47.2) on the top rim to (298.2, 125.1) inside the body, against a
/// radius through the body centroid at (233.3, 172.1). 55 degrees is twice
/// that, which admits a diagonal fold without admitting a tangent.
const INWARD_COS: f64 = 0.573_576_436_351_046;

/// Positions along a mark where the tones either side of it are compared,
/// as fractions of its length.
///
/// A mark has to land on a TONE STEP, or it reads as a scratch lying on the
/// surface rather than as the fold the surface makes. The artwork's does:
/// its mark runs down the edge of the top-centre peak, with a different
/// orange either side of it for its whole length. Requiring the step is also
/// what ties this pass to the other two - the cut was chosen to sit on the
/// solid's median brightness contour, and the tones were quantised from the
/// same surface, so a mark that finds a step is a mark on a real fold.
const TONE_PROBES: [f64; 3] = [0.3, 0.55, 0.8];
/// How far either side of the spine those probes sit, in half-widths. Clear
/// of the mark's own ink, so a probe reads the facet rather than the line.
const TONE_PROBE_REACH: f64 = 2.2;

/// Samples at each end of a candidate that the crease score ignores.
///
/// The first few sit inside the ink band, where the depth map's own rim row
/// is the end of the parameterisation rather than a feature of the rock.
const SCORE_SKIP: usize = 3;

/// How far into the ink band a candidate's rim end must reach to count as
/// growing out of the outline, as a fraction of the ribbon width.
///
/// The facet root sits on the ribbon's MIDLINE, so a cut that ends on the
/// root ends halfway through the ink and the mark emerges from it seamlessly,
/// exactly as measured. A cut that ends on an earlier cut instead - an
/// interior junction, about half of them - is strictly inside that, and a
/// mark starting there would float with a tapered point hanging in the
/// orange. Testing the ink band rather than tracking provenance through the
/// subdivision tests the property that actually matters.
const RIM_BAND_FRAC: f64 = 0.25;

/// One finished relief mark.
#[derive(Clone, Debug)]
pub struct Relief {
    /// The centreline, rim end first.
    pub spine: Vec<V2>,
    /// The closed outline of the tapered ribbon.
    pub pts: Vec<V2>,
    /// The emitted curve.
    pub cubics: Vec<Cubic>,
    /// Waist width in canonical units.
    pub width: f64,
    /// Mean brightness gradient along the spine, times R0 - the crease
    /// strength this mark was chosen for.
    pub score: f64,
    /// Handles the Bezier clamp caught.
    pub clamped: usize,
}

/// A facet cut, kept so the relief pass can look for creases along it.
#[derive(Clone, Debug)]
pub struct Cut {
    /// The cut polyline, both ends exactly on the region boundary they
    /// crossed.
    pub pts: Vec<V2>,
}

/// Choose and build up to `p.relief_lines` marks.
///
/// `root` is the facet root ring - the ribbon's midline - which is what the
/// rim test measures against.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build(
    cuts: &[Cut],
    root: &Ring,
    depth: &DepthMap,
    light: &Light,
    facets: &[Facet],
    vesicles: &[Vesicle],
    p: &Params,
    r0: f64,
    w0: f64,
) -> Vec<Relief> {
    if p.relief_lines == 0 || p.relief_weight <= 0.0 {
        return Vec::new();
    }
    let width = p.relief_weight * r0;
    let half = 0.5 * width;
    let spacing = (SAMPLE_FRAC * r0).max(1e-6);
    let want_len = p.relief_len * r0;
    let grad_h = GRAD_H_FRAC * r0;

    // Every rim end of every cut is a candidate, scored by how hard the
    // surface tilts across it.
    let mut cands: Vec<(f64, Vec<V2>)> = Vec::new();
    for cut in cuts {
        if cut.pts.len() < 3 {
            continue;
        }
        for from_start in [true, false] {
            let mut spine: Vec<V2> = if from_start {
                cut.pts.clone()
            } else {
                cut.pts.iter().rev().copied().collect()
            };
            // Does this end emerge from the ink?
            let end = spine[0];
            let d = end - root.c;
            if d.len() < root.radius_toward(d) - RIM_BAND_FRAC * w0 {
                continue;
            }
            spine = truncate(&spine, want_len, spacing);
            if spine.len() < 4 || polyline_len(&spine) < 0.5 * want_len {
                continue;
            }
            // And does it head INTO the body rather than along the rim?
            let inward = (root.c - end).norm();
            if (spine[1] - spine[0]).norm().dot(inward) < INWARD_COS {
                continue;
            }
            let score = crease_score(&spine, depth, light, grad_h, r0);
            cands.push((score, spine));
        }
    }

    // Strongest crease first, with a length tie-break so two equal scores
    // order the same way on every platform.
    cands.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(polyline_len(&b.1).total_cmp(&polyline_len(&a.1)))
    });

    let mut out: Vec<Relief> = Vec::new();
    for (score, spine) in cands {
        if out.len() >= p.relief_lines as usize {
            break;
        }
        if !curvature_ok(&spine, half) {
            continue;
        }
        if hits_a_vesicle(&spine, vesicles, half * VESICLE_CLEAR) {
            continue;
        }
        if !straddles_a_tone_step(&spine, facets, half * TONE_PROBE_REACH) {
            continue;
        }
        if out
            .iter()
            .any(|m| min_spine_gap(&spine, &m.spine) < MARK_SEP_FRAC * r0)
        {
            continue;
        }
        let pts = ribbon(&spine, width);
        if pts.len() < 6 || self_intersects(&pts, true) {
            continue;
        }
        let mut cubics = catmull_rom_closed(&pts, p.cr_tension);
        let clamped = clamp_handles(&mut cubics);
        out.push(Relief {
            spine,
            pts,
            cubics,
            width,
            score,
            clamped,
        });
    }
    out
}

/// Resample the head of `spine` to uniform spacing, stopped at `want` of
/// arclength.
fn truncate(spine: &[V2], want: f64, spacing: f64) -> Vec<V2> {
    let mut out = vec![spine[0]];
    let mut carry = 0.0;
    let mut total = 0.0;
    for w in spine.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = (b - a).len();
        if seg <= 1e-12 {
            continue;
        }
        let mut t = spacing - carry;
        while t <= seg {
            if total + t > want {
                return out;
            }
            out.push(a + (b - a) * (t / seg));
            t += spacing;
        }
        carry = seg - (t - spacing);
        total += seg;
        if total >= want {
            return out;
        }
    }
    out
}

/// How hard the surface FOLDS along the spine: the mean curvature of the
/// brightness measured across it, in units of 1/R0^2.
///
/// The second derivative, not the first, and the difference matters more than
/// it sounds. The brightness gradient is largest near the rim, where the
/// surface rolls away from the viewer, and it is large there whether or not
/// anything interesting is happening - so scoring by the gradient picks the
/// candidate that spends longest near the rim, every time. Measured on the
/// first twelve seeds: every mark chosen that way ran tangentially, a couple
/// of ink widths inside the outline, which reads as a nick in the ink.
///
/// A fold is a place where the gradient CHANGES fast, and a smooth rim
/// roll-off is not one however steep it is. Sampling the brightness at the
/// spine and one step either side of it, across the spine, measures exactly
/// that for three lookups a sample.
fn crease_score(spine: &[V2], depth: &DepthMap, light: &Light, h: f64, r0: f64) -> f64 {
    let n = spine.len();
    if n < 2 * SCORE_SKIP + 3 {
        return 0.0;
    }
    let mut acc = 0.0;
    let mut count = 0.0;
    for i in SCORE_SKIP..n - SCORE_SKIP {
        let t = (spine[(i + 1).min(n - 1)] - spine[i.saturating_sub(1)]).norm();
        let nrm = t.perp() * h;
        let b0 = depth.brightness(spine[i] - nrm, light);
        let b1 = depth.brightness(spine[i], light);
        let b2 = depth.brightness(spine[i] + nrm, light);
        acc += (b2 - 2.0 * b1 + b0).abs();
        count += 1.0;
    }
    if count == 0.0 {
        return 0.0;
    }
    // Divided by h^2 and scaled to R0^2, so the number means the same thing
    // whatever R0 is.
    acc / count / (h * h) * r0 * r0
}

/// Is the spine straight enough to be offset by `half` without folding?
fn curvature_ok(spine: &[V2], half: f64) -> bool {
    for w in spine.windows(3) {
        let a = w[1] - w[0];
        let b = w[2] - w[1];
        let (la, lb) = (a.len(), b.len());
        if la < 1e-12 || lb < 1e-12 {
            return false;
        }
        // The turn between consecutive chords, via the cross product, so no
        // inverse trigonometry: sin(turn) = |a x b| / (|a||b|).
        let s = (a.cross(b) / (la * lb)).abs();
        let chord = 0.5 * (la + lb);
        // radius = chord / turn, and sin(turn) <= turn, so using the sine
        // can only make the test stricter.
        if s > 1e-12 && chord / s < MIN_CURVE_RATIO * half {
            return false;
        }
    }
    true
}

/// Is there a tone step across the mark anywhere along it?
///
/// Looks up the facet containing a point either side of the spine at each of
/// [`TONE_PROBES`]. A probe that lands in no facet - in the ribbon, or in the
/// sliver of underlay between two of them - is simply not counted, so a mark
/// is rejected for being on flat colour, never for a probe missing.
fn straddles_a_tone_step(spine: &[V2], facets: &[Facet], reach: f64) -> bool {
    let n = spine.len();
    if n < 3 || facets.is_empty() {
        return false;
    }
    let tone_at = |q: V2| -> Option<usize> {
        facets
            .iter()
            .find(|f| point_in_polygon(&f.pts, q))
            .map(|f| f.tone)
    };
    for frac in TONE_PROBES {
        let i = ((frac * (n - 1) as f64) as usize).clamp(1, n - 2);
        let t = (spine[i + 1] - spine[i - 1]).norm();
        let off = t.perp() * reach;
        if let (Some(a), Some(b)) = (tone_at(spine[i] + off), tone_at(spine[i] - off)) {
            if a != b {
                return true;
            }
        }
    }
    false
}

/// Does any spine sample come within `clear` of a vesicle?
fn hits_a_vesicle(spine: &[V2], vesicles: &[Vesicle], clear: f64) -> bool {
    spine.iter().any(|q| {
        vesicles.iter().any(|v| {
            let d = *q - v.centre;
            d.len() < v.ring.radius_toward(d) + clear
        })
    })
}

/// Closest approach between two spines.
fn min_spine_gap(a: &[V2], b: &[V2]) -> f64 {
    let mut best = f64::INFINITY;
    for p in a {
        for q in b {
            best = best.min((*p - *q).len());
        }
    }
    best
}

/// The tapered width at arclength fraction `t`.
///
/// `x * (2 - x)` over the first and last [`TAPER_FRAC`] of the length: the
/// measured profile, see the module docs.
fn taper(t: f64) -> f64 {
    let x = if t < TAPER_FRAC {
        t / TAPER_FRAC
    } else if t > 1.0 - TAPER_FRAC {
        (1.0 - t) / TAPER_FRAC
    } else {
        1.0
    };
    let x = x.clamp(0.0, 1.0);
    x * (2.0 - x)
}

/// Offset the spine either side by the tapered half-width and close the loop.
///
/// Both tips are a SINGLE shared point, so the two sides meet there instead of
/// approaching each other asymptotically - which would leave a pair of points
/// closer than the degenerate-segment gate allows, and a needle where the
/// drawing has a rounded end.
fn ribbon(spine: &[V2], width: f64) -> Vec<V2> {
    let n = spine.len();
    if n < 3 {
        return Vec::new();
    }
    let mut cum = Vec::with_capacity(n);
    let mut acc = 0.0;
    cum.push(0.0);
    for w in spine.windows(2) {
        acc += (w[1] - w[0]).len();
        cum.push(acc);
    }
    if acc <= 1e-12 {
        return Vec::new();
    }

    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 1..n - 1 {
        // Averaged tangent, so the offset does not kink at a sample.
        let t = (spine[i + 1] - spine[i - 1]).norm();
        let nrm = t.perp();
        let h = 0.5 * width * taper(cum[i] / acc);
        left.push(spine[i] + nrm * h);
        right.push(spine[i] - nrm * h);
    }

    let mut out = Vec::with_capacity(2 * n);
    out.push(spine[0]);
    out.extend(left);
    out.push(spine[n - 1]);
    right.reverse();
    out.extend(right);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight(n: usize, step: f64) -> Vec<V2> {
        (0..n).map(|i| V2::new(i as f64 * step, 0.0)).collect()
    }

    /// The measured profile, checked at the two points it was measured at:
    /// 0.74 and 0.75 of the waist at 5% and 95% of the length.
    #[test]
    fn the_taper_matches_the_measured_profile() {
        assert!((taper(0.0) - 0.0).abs() < 1e-12, "the ends are points");
        assert!((taper(1.0) - 0.0).abs() < 1e-12, "the ends are points");
        assert!((taper(0.05) - 0.75).abs() < 0.01, "{}", taper(0.05));
        assert!((taper(0.95) - 0.75).abs() < 0.01, "{}", taper(0.95));
        for k in 10..=90 {
            let t = k as f64 / 100.0;
            assert!((taper(t) - 1.0).abs() < 1e-12, "the waist is constant");
        }
    }

    /// A straight spine gives a ribbon of exactly the asked-for width at its
    /// waist, closed, and tapered to a point at both tips.
    #[test]
    fn a_straight_spine_gives_an_even_ribbon() {
        let spine = straight(41, 4.0);
        let pts = ribbon(&spine, 10.0);
        assert_eq!(pts.len(), 2 + 2 * 39);
        assert!(!self_intersects(&pts, true));
        // The waist: the two sides are 10 apart at the middle.
        let mid = 20usize;
        let a = pts[mid];
        let b = pts[pts.len() - mid];
        assert!(
            ((a - b).len() - 10.0).abs() < 1e-9,
            "waist {}",
            (a - b).len()
        );
        // The tips are the spine's own ends.
        assert!((pts[0] - spine[0]).len() < 1e-12);
        assert!((pts[40] - spine[40]).len() < 1e-12);
    }

    /// The curvature guard has to reject a hook, or the offset folds over
    /// itself and the mark fills in as a blot.
    #[test]
    fn the_curvature_guard_rejects_a_hook() {
        assert!(curvature_ok(&straight(20, 4.0), 5.0));
        // A quarter circle of radius 6 sampled every 4 units: radius 6 is
        // under 2.5 half-widths of 5.
        let hook: Vec<V2> = (0..10)
            .map(|i| {
                let a = i as f64 * 0.6;
                V2::new(6.0 * crate::math::cos(a), 6.0 * crate::math::sin(a))
            })
            .collect();
        assert!(!curvature_ok(&hook, 5.0));
    }

    /// `truncate` stops at the asked-for arclength and resamples evenly.
    #[test]
    fn truncate_stops_at_the_length_asked_for() {
        let spine = straight(200, 3.0);
        let out = truncate(&spine, 100.0, 4.0);
        let len = polyline_len(&out);
        assert!(len <= 100.0 + 1e-9, "overran: {len}");
        assert!(len > 90.0, "stopped short: {len}");
        for w in out.windows(2) {
            assert!(((w[1] - w[0]).len() - 4.0).abs() < 1e-9);
        }
    }
}
