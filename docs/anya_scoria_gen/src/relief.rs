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
//! Down a CREASE of the solid, traced as one. The first version took the
//! mark from a facet cut, which is convenient - a cut is already a validated
//! curve inside the body and already a tone boundary - and it is why every
//! mark came out looking like the same mark: a cut is a near-straight chord
//! by design, and truncating it at a fixed length gave 14 of 16 logos a
//! stroke of 164.2 units and 9.41 width deviating 2-7% from its chord. The
//! same object, rotated.
//!
//! A fold in a lumpy rock is not a chord. So the mark is now traced along
//! the crest of the surface's own fold - see [`DepthMap::fold`] - from where
//! that fold meets the rim, inward, for as far as the fold actually runs.
//! Length, curvature and waist all then come from the rock rather than from
//! a constant: the mark stops where the crease fades, bends with it, and is
//! drawn heavier where the crease is sharper.
//!
//! What stays measured is the mark's LANGUAGE rather than its dimensions -
//! the taper profile, the constant waist between the tapers, the rim end
//! buried in the ink, the inward heading - because those are what make it
//! read as a branch of the outline. What varies is everything Part 1 could
//! only measure one sample of.

use crate::bezier::{catmull_rom_closed, clamp_handles, self_intersects, Cubic};
use crate::facet::Facet;
use crate::math::{point_in_polygon, polyline_len, V2};
use crate::params::Params;
use crate::ring::Ring;
use crate::solid::{DepthMap, Light, GRAD_H_FRAC};
use crate::vesicle::Vesicle;

/// Fraction of the length each end tapers over. Measured; see the module docs.
const TAPER_FRAC: f64 = 0.10;

/// Euler step along the crest, as a fraction of R0.
///
/// The same 0.01*R0 Hobbs uses and the facet cuts use. Three canonical
/// units: short enough that the trace resolves a curving crest, long enough
/// that a 160-unit mark is 50 steps rather than 500.
const STEP_FRAC: f64 = 0.01;

/// How far the crest may turn in one step, radians.
///
/// A crest direction comes from diagonalising a Hessian, and near a point
/// where the two principal curvatures are equal the eigenvectors are
/// undefined and swing arbitrarily. Without a rate limit the trace takes
/// that swing literally and the mark kinks. 0.11 rad over a 3-unit step is
/// a floor of 27 units on the mark's curvature radius - well inside what
/// the offset can take, and still enough to follow a real crease round a
/// lump.
const MAX_STEP_TURN: f64 = 0.11;

/// Total absolute turning a mark may accumulate, radians (80 degrees).
///
/// The crest direction comes from an eigen-decomposition of a field with
/// swirl in it, so a trace left to itself will happily curl round into a
/// fishhook - and a fold in a rock does not hook. Measured on Anya's: it
/// runs (298.2, 125.1) to (313.4, 75.0) to (292.9, 47.2), which is a turn
/// of 53 degrees over its length. 80 allows that and a little more, and
/// TRUNCATES rather than rejects, so a crease that starts well and curls
/// later is drawn up to the point where it stops being a fold.
const MAX_TOTAL_TURN: f64 = 1.396_263_401_595_463_7;

/// Fraction of its starting strength at which a fold is over.
///
/// This is what makes the length come from the rock: the trace stops when
/// the crease it is drawing has faded to a quarter of what it was at the
/// rim, so a mark is as long as its fold and no longer.
const FADE_FRAC: f64 = 0.25;

/// Shortest mark worth drawing, as a fraction of R0.
///
/// Anya's is 0.535 of the mean radius. Letting a trace stop at 0.18 gave
/// five logos in sixteen a 60-unit tick sitting against the rim, which
/// reads as a blemish in the ink rather than as relief - the mark has to be
/// long enough to be legible as a line before it can be read as a fold.
/// 0.30 is a little over half the measured length, and it is a FLOOR, not a
/// target: the ranking below prefers longer still.
const MIN_LEN_FRAC: f64 = 0.30;

/// How many candidates are built before the best are chosen. The rim yields
/// a handful of crests and only `relief_lines` are drawn; this bounds the
/// work when a craggy seed yields many.
const MAX_BUILT: usize = 8;

/// Cap on the walk back out to the ink, in ribbon widths.
///
/// The probe sits [`RIM_PROBE_INSET`] in and the crest may run up to 55
/// degrees off radial, so the walk needs at most `1.6 / cos(55 deg)` = 2.8
/// widths; 4.0 is that with slack. A cap is needed rather than just a step
/// budget because the walk can head outward into a NOTCH, where stepping
/// away from the centre leaves the radius falling for a while and the loop
/// runs on - measured, one candidate reached 240 units against a 162-unit
/// mark. A candidate that cannot reach the ink inside the cap is dropped,
/// which is the right answer: its crest does not meet the rim.
const MAX_ANCHOR_FRAC: f64 = 4.0;

/// Rim positions probed for a fold, per logo.
///
/// One every two degrees. The crests that reach the rim are a handful of
/// features tens of units wide, so this resolves them several times over
/// and the local-maximum pass below collapses each to one candidate.
const RIM_PROBES: usize = 180;

/// How far inside the ribbon's midline a rim probe sits, in ribbon widths.
///
/// Far enough in that the probe's Hessian stencil - which reaches `h` in
/// each axis and `h*sqrt(2)` to its corners, 17 units at the default - stays
/// inside the silhouette, where the depth map has a surface. Sampling past
/// it is not an error (the map clamps to the rim) but it biases every
/// second difference toward the rim's own roll-off, which is the one
/// feature the inward test exists to reject.
///
/// The mark does not START here, though: it is anchored back out to the ink
/// afterwards, see `anchor_to_ink`.
const RIM_PROBE_INSET: f64 = 1.6;

/// Half-width of the window, in probes, a candidate must be the strongest
/// fold within. Stops one broad crest from yielding five marks.
const PEAK_WINDOW: usize = 6;

/// How far a mark's waist may stray from `relief_weight`, either way.
///
/// The artwork gives one sample, so its 0.58 of the outline is the CENTRE of
/// the band rather than the whole of it; a sharper fold gets a heavier line,
/// the way it would be drawn. Part 1 makes the same point about the gas
/// holes: giving every one the same lobe count is the most obvious tell of
/// a naive generator.
const WIDTH_SPREAD: f64 = 0.22;

/// cos(55 degrees): how far off inward-radial a crest may run and still be
/// taken for a fold rather than for the rim.
///
/// This is the test that separates the rock's creases from its own edge. A
/// rim roll-off has a crest as surely as a fold does - it is the strongest
/// one anywhere on the shape, because that is where the surface turns away
/// fastest - but its crest runs ALONG the rim, and a line drawn there reads
/// as a nick in the ink. Anya's leaves the rim 29 degrees off radial:
/// measured, from (292.9, 47.2) on the top rim to (298.2, 125.1) inside the
/// body, against a radius through the body centroid at (233.3, 172.1). 55
/// is twice that, which admits a diagonal fold without admitting a tangent.
const INWARD_COS: f64 = 0.573_576_436_351_046;

/// How many half-widths of curvature radius the centreline must have.
///
/// Offsetting a polyline by `h` folds the inner side over itself wherever
/// the local curvature radius drops below `h`. 2.5 leaves margin for the
/// Catmull-Rom fit, which can bulge a handle slightly past the polyline.
const MIN_CURVE_RATIO: f64 = 2.5;

/// Minimum clearance from a mark to a vesicle, in half-widths.
///
/// A relief line running into a gas hole reads as a crack through it rather
/// than as a fold of the surface, and the artwork has nothing like it: its
/// mark's nearest vesicle is 35 units away, six waists.
const VESICLE_CLEAR: f64 = 2.0;

/// How close two marks may come, as a fraction of R0.
///
/// Two creases side by side read as hatching, which is a different drawing
/// language from a flat three-tone mark.
const MARK_SEP_FRAC: f64 = 0.30;

/// Positions along a mark where the tones either side of it are compared,
/// as fractions of its length.
///
/// A mark that lands on a TONE STEP reads better - Anya's does, running down
/// the edge of the top-centre peak with a different orange either side of
/// it for its whole length - and it is also the sign that the three passes
/// agree, since the cuts lean along the same surface's iso-brightness lines
/// and the tones are quantised from the same normals.
///
/// It is a PREFERENCE and not a rule, though, and that is a change: while
/// the mark was a facet cut it was a tone boundary by construction, so
/// requiring one cost nothing. A crest traced through the surface need not
/// follow a cut, and demanding a step then threw away 166 good candidates
/// over 28 scenes and left only a third of logos with a mark at all. The
/// mark's justification is the FOLD; the tone step just breaks ties.
const TONE_PROBES: [f64; 3] = [0.3, 0.55, 0.8];

/// How far either side of the spine those probes sit, in half-widths. Clear
/// of the mark's own ink, so a probe reads the facet rather than the line.
const TONE_PROBE_REACH: f64 = 2.2;

/// How much a candidate's score is multiplied by when it straddles a tone
/// step. Enough to outrank a slightly sharper crease that does not.
const TONE_STEP_BONUS: f64 = 1.35;

/// The fold strength, in units of `1/R0^2`, that maps to the measured waist.
///
/// A GLOBAL reference, not this logo's own median, and the difference is the
/// whole point. Normalising per logo makes the mark's width depend on where
/// it sits in its own logo's distribution - and since the strongest crease
/// is always the one drawn, that is nearly constant, which is how the first
/// attempt ended up with every mark the same width again. Against a fixed
/// scale, a seed whose creases are genuinely sharper gets a genuinely
/// heavier line.
///
/// Measured: over 50 seeds the selected fold's `strength * R0^2` runs 161 at
/// the 10th percentile, 245 at the median and 356 at the 90th. Referencing
/// the median puts the typical mark at the measured waist and the tails at
/// 0.81 and 1.21 of it - just inside [`WIDTH_SPREAD`], so the clamp catches
/// outliers without flattening the range.
const WIDTH_REF_STRENGTH: f64 = 245.0;

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
    /// The fold strength this mark was chosen for: the larger principal
    /// curvature of the brightness at its rim end, in units of `1/length^2`.
    pub score: f64,
    /// Handles the Bezier clamp caught.
    pub clamped: usize,
}

/// Find the rock's strongest creases and draw up to `p.relief_lines` of
/// them.
///
/// `root` is the facet root ring - the ribbon's midline - which is what the
/// rim probes and the inside test measure against.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn build(
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
    let h = GRAD_H_FRAC * r0;
    let step = STEP_FRAC * r0;
    let max_len = p.relief_len * r0;
    let min_len = MIN_LEN_FRAC * r0;

    // ---- where the folds meet the rim
    //
    // One probe every two degrees, just inside the ink. A probe qualifies
    // when the crest there heads INTO the body: a rim roll-off has a crest
    // too, running along the rim, and it is the strongest "fold" anywhere on
    // the shape - so the inward test is what separates the rock's creases
    // from its own edge, and it does it before any strength is compared.
    let mut probes: Vec<Option<(f64, V2, V2)>> = Vec::with_capacity(RIM_PROBES);
    for i in 0..RIM_PROBES {
        let theta = crate::math::TAU * i as f64 / RIM_PROBES as f64;
        let (st, ct) = crate::math::sin_cos(theta);
        let dir = V2::new(ct, st);
        let rim = root.radius_toward(dir);
        let q = root.c + dir * (rim - RIM_PROBE_INSET * w0);
        let Some(f) = depth.fold(q, light, h) else {
            probes.push(None);
            continue;
        };
        // The crest has no heading of its own; take the one that goes in.
        let inward = -dir;
        let along = if f.along.dot(inward) < 0.0 {
            -f.along
        } else {
            f.along
        };
        if along.dot(inward) < INWARD_COS {
            probes.push(None);
            continue;
        }
        probes.push(Some((f.strength, q, along)));
    }

    // Local maxima only, so one broad crest yields one mark.
    let mut cands: Vec<(f64, V2, V2)> = Vec::new();
    for i in 0..RIM_PROBES {
        let Some((s_i, q, along)) = probes[i] else {
            continue;
        };
        let peak = (1..=PEAK_WINDOW).all(|d| {
            let a = probes[(i + d) % RIM_PROBES].map_or(0.0, |x| x.0);
            let b = probes[(i + RIM_PROBES - d) % RIM_PROBES].map_or(0.0, |x| x.0);
            // Strictly greater one way, so a plateau yields its first probe
            // and not all of them.
            s_i > a && s_i >= b
        });
        if peak {
            cands.push((s_i, q, along));
        }
    }
    if cands.is_empty() {
        return Vec::new();
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));

    // The trace needs a clearance before the width is known, and the width
    // needs the trace. The nominal half-width is within 22% of the real one
    // either way, which is well inside the two half-widths of margin the
    // vesicle rule asks for.
    let half_hint = 0.5 * p.relief_weight * r0;

    // Two passes, so a tone step can promote a candidate without a hard
    // rule throwing the others away: everything that survives the geometry
    // is built once, scored with the bonus, and the best kept.
    let mut built: Vec<(f64, Relief)> = Vec::new();
    for (strength, seed, along) in cands {
        if built.len() >= MAX_BUILT {
            break;
        }
        let mut spine = anchor_to_ink(seed, along, root, step, w0);
        let anchored = spine.len();
        spine.extend(trace(
            seed, along, depth, light, root, vesicles, h, step, max_len, strength, half_hint,
        ));
        if anchored == 0 || polyline_len(&spine) < min_len || spine.len() < 4 {
            continue;
        }
        // A sharper crease is drawn heavier. Square root, because strength
        // is a second derivative and noisy; clamped, because the measured
        // 0.58 of the outline is the centre of the band and not a licence.
        let scale = (strength * r0 * r0 / WIDTH_REF_STRENGTH)
            .sqrt()
            .clamp(1.0 - WIDTH_SPREAD, 1.0 + WIDTH_SPREAD);
        let width = p.relief_weight * r0 * scale;
        let half = 0.5 * width;

        if !curvature_ok(&spine, half) {
            continue;
        }
        if hits_a_vesicle(&spine, vesicles, half * VESICLE_CLEAR) {
            continue;
        }
        let pts = ribbon(&spine, width);
        if pts.len() < 6 || self_intersects(&pts, true) {
            continue;
        }
        let mut cubics = catmull_rom_closed(&pts, p.cr_tension);
        let clamped = clamp_handles(&mut cubics);
        // Sharpness, weighted by length and by whether the shading already
        // agrees there.
        //
        // Length counts as its SQUARE ROOT against sharpness linearly. A
        // long crease is a more significant feature of a rock than a sharp
        // short one, and the measured mark is long - 0.535 of the mean
        // radius; ranking on sharpness alone picks the sharpest fold
        // wherever it is, which is often a small one. The root tips the
        // choice toward the feature a person would have drawn without
        // letting length override sharpness entirely, and it is a root
        // rather than a `powf` because the house rule keeps libm out of
        // the crate.
        let rank = strength
            * (polyline_len(&spine) / r0).max(1e-9).sqrt()
            * if straddles_a_tone_step(&spine, facets, half * TONE_PROBE_REACH) {
                TONE_STEP_BONUS
            } else {
                1.0
            };
        built.push((
            rank,
            Relief {
                spine,
                pts,
                cubics,
                width,
                score: strength,
                clamped,
            },
        ));
    }

    built.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(polyline_len(&b.1.spine).total_cmp(&polyline_len(&a.1.spine)))
    });
    let mut out: Vec<Relief> = Vec::new();
    for (_, m) in built {
        if out.len() >= p.relief_lines as usize {
            break;
        }
        // Spacing is decided here rather than during the build, so a weaker
        // mark cannot reserve the space a stronger one wanted.
        if out
            .iter()
            .any(|k| min_spine_gap(&m.spine, &k.spine) < MARK_SEP_FRAC * r0)
        {
            continue;
        }
        out.push(m);
    }
    out
}

/// The run from the outline ink out to `seed`, so the mark starts buried in
/// the ribbon rather than floating a probe's inset inside it.
///
/// The fold has to be measured where its Hessian is well conditioned, which
/// is a little inside the rim; the mark has to START in the ink, which is
/// where the measurement is worst. So the two are separated: probe inside,
/// then walk back out along the crest's own direction to the ribbon's
/// midline. The first stretch of the mark is therefore straight, which is
/// how such a line is drawn anyway - it leaves the outline and then picks up
/// the form - and the taper covers most of it.
///
/// Returns the points from the ink inward, `seed` excluded, or empty if the
/// walk cannot reach the ink.
fn anchor_to_ink(seed: V2, along: V2, root: &Ring, step: f64, w0: f64) -> Vec<V2> {
    let mut back = Vec::new();
    let mut q = seed;
    let budget = ((MAX_ANCHOR_FRAC * w0 / step).ceil() as usize).max(2);
    // To the MIDLINE, which is where `root` is. A step can overshoot it by
    // three units, so the tip lands within a fifth of a ribbon width of the
    // middle of the ink - buried, with the band's outer half still to
    // spare, and never proud of the silhouette.
    for _ in 0..budget {
        let next = q - along * step;
        let d = next - root.c;
        back.push(next);
        if d.len() >= root.radius_toward(d) {
            back.reverse();
            return back;
        }
        q = next;
    }
    Vec::new()
}

/// Walk the crest inward from `seed` until the fold fades, the body ends, or
/// `max_len` is spent.
///
/// Three things have to be carried along the walk. The crest direction comes
/// out of an eigen-decomposition and so has no SIGN - it has to be oriented
/// against the previous step every time, or the trace reverses the moment
/// the eigenvector flips. The turn is RATE LIMITED, because where the two
/// principal curvatures are equal the eigenvectors are undefined and swing
/// arbitrarily. And the fold's strength at the start sets the bar for
/// stopping, which is what makes a mark as long as its own crease.
#[allow(clippy::too_many_arguments)]
fn trace(
    seed: V2,
    along: V2,
    depth: &DepthMap,
    light: &Light,
    root: &Ring,
    vesicles: &[Vesicle],
    h: f64,
    step: f64,
    max_len: f64,
    s0: f64,
    clear: f64,
) -> Vec<V2> {
    let mut pts = vec![seed];
    let mut p = seed;
    let mut dir = along;
    let mut len = 0.0;
    let mut turned = 0.0;
    let floor = FADE_FRAC * s0;
    while len < max_len {
        let next = p + dir * step;
        // Inside the body, with room for the mark's own half-width.
        let d = next - root.c;
        if d.len() > root.radius_toward(d) {
            break;
        }
        // A hole STOPS the mark rather than disqualifying it. A crease
        // that runs up to a vesicle is a real crease; it is only the line
        // crossing the hole that would read as a crack through it, and
        // ending the stroke short is what an illustrator would do anyway.
        if hits_a_vesicle(&[next], vesicles, clear * VESICLE_CLEAR) {
            break;
        }
        let Some(f) = depth.fold(next, light, h) else {
            break;
        };
        if f.strength < floor {
            break;
        }
        let mut want = if f.along.dot(dir) < 0.0 {
            -f.along
        } else {
            f.along
        };
        // Rate limit, as an exact rotation onto the cone boundary: a unit
        // `want` is `dir*c + dir.perp()*s` with `c^2 + s^2 = 1`, so clamping
        // is a matter of rebuilding it with the limit values and `s`'s sign.
        let c = dir.dot(want);
        if c < MAX_STEP_TURN.cos() {
            let sgn = if dir.cross(want) < 0.0 { -1.0 } else { 1.0 };
            want = (dir * MAX_STEP_TURN.cos() + dir.perp() * (MAX_STEP_TURN.sin() * sgn)).norm();
        }
        turned += crate::math::angle_between(dir, want);
        if turned > MAX_TOTAL_TURN {
            break;
        }
        dir = want;
        pts.push(next);
        len += step;
        p = next;
    }
    pts
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
}
