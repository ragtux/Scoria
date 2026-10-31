//! Catmull-Rom smoothing, converted segment by segment to cubic Beziers.
//!
//! UNIFORM Catmull-Rom, which is only safe because every point array reaching
//! here has been resampled to uniform arclength first. Uniform CR overshoots
//! exactly where sample spacing changes fastest — on a polar blob that is
//! every deep notch — and the overshoot becomes a visible loop in the fill.
//! Resampling removes the failure mode; lowering tau to hide it would flatten
//! the whole curve to patch one defect.
//!
//! Why uniform and not centripetal, given centripetal is provably cusp-free:
//! on arclength-uniform data uniform CR reproduces a circle to within
//! `2e-7 * R` at 256 samples (see [`catmull_rom_closed`]), it needs fewer
//! divides, and the exactness is testable. Centripetal remains the correct
//! fallback for anyone who skips the resample.

use crate::math::{angle_between, seg_intersect, V2};

/// One cubic Bezier segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cubic {
    /// Start point.
    pub p0: V2,
    /// Handle out of `p0`.
    pub c1: V2,
    /// Handle into `p3`.
    pub c2: V2,
    /// End point.
    pub p3: V2,
}

impl Cubic {
    /// The same curve traced the other way.
    ///
    /// Load-bearing for the facet split: child1 gets the reverse of the cut
    /// that child2 gets, and because reversal is a pure reordering of the same
    /// four `f64` pairs the shared edge is literally the same cubic. No
    /// arithmetic, so no last-bit drift, so no hairline seam.
    #[must_use]
    pub fn reversed(&self) -> Cubic {
        Cubic {
            p0: self.p3,
            c1: self.c2,
            c2: self.c1,
            p3: self.p0,
        }
    }

    /// The point at parameter `t`, Bernstein form.
    #[must_use]
    pub fn at(&self, t: f64) -> V2 {
        let mt = 1.0 - t;
        let a = mt * mt * mt;
        let b = 3.0 * mt * mt * t;
        let c = 3.0 * mt * t * t;
        let d = t * t * t;
        self.p0 * a + self.c1 * b + self.c2 * c + self.p3 * d
    }

    /// Straight-line distance from start to end.
    #[must_use]
    pub fn chord(&self) -> V2 {
        self.p3 - self.p0
    }
}

/// Uniform Catmull-Rom over a CLOSED point loop, as cubic Beziers.
///
/// For the segment `P_i -> P_{i+1}` with neighbours `P_{i-1}` and `P_{i+2}`,
/// all indices mod `n`:
///
/// ```text
///   B1 = P_i     + tau * (P_{i+1} - P_{i-1}) / 3
///   B2 = P_{i+1} - tau * (P_{i+2} - P_i)     / 3
/// ```
///
/// At `tau = 0.5` that is the familiar exact 1/6 coefficient. Why that is the
/// right number: on a circle of radius R sampled every `dtheta`, the handle
/// length is `|P_{i+1} - P_{i-1}|/6 = (2R sin dtheta)/6 ~= R dtheta/3`, and
/// the exact circular-arc Bezier handle is `(4/3) tan(dtheta/4) R ~= R
/// dtheta/3`. They agree to `O(dtheta^3)`, so a circle comes out visually
/// exact — the single best test that the formula is right.
///
/// Returns an empty vector for fewer than three points.
#[must_use]
pub fn catmull_rom_closed(pts: &[V2], tau: f64) -> Vec<Cubic> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let s = tau / 3.0;
    (0..n)
        .map(|i| {
            let pm = pts[(i + n - 1) % n];
            let p0 = pts[i];
            let p1 = pts[(i + 1) % n];
            let p2 = pts[(i + 2) % n];
            Cubic {
                p0,
                c1: p0 + (p1 - pm) * s,
                c2: p1 - (p2 - p0) * s,
                p3: p1,
            }
        })
        .collect()
}

/// Uniform Catmull-Rom over an OPEN piece, with clamped (one-sided) end
/// tangents.
///
/// The end handling is not a detail. Two facets that share an edge each smooth
/// that edge as an open piece; if the result depended on anything outside the
/// piece the two Beziers would differ near the junctions and antialias into a
/// visible hairline. With one-sided end differences the output satisfies
///
/// ```text
///   reverse_cubics(catmull_rom_open(pts, tau))
///       == catmull_rom_open(reversed(pts), tau)      // BIT for bit
/// ```
///
/// because every tangent of the reversed input is the exact IEEE negation of
/// the corresponding forward tangent, and `a + (-x)` and `a - x` are the same
/// operation. The facet clipping section of SPEC.md leans on this.
///
/// Returns `pts.len() - 1` cubics, or an empty vector for fewer than two.
#[must_use]
pub fn catmull_rom_open(pts: &[V2], tau: f64) -> Vec<Cubic> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let s = tau / 3.0;
    // Tangent proxy at each knot: the central difference inside, the one-sided
    // difference at the two ends.
    let tangent = |i: usize| -> V2 {
        if i == 0 {
            pts[1] - pts[0]
        } else if i == n - 1 {
            pts[n - 1] - pts[n - 2]
        } else {
            pts[i + 1] - pts[i - 1]
        }
    };
    (0..n - 1)
        .map(|i| {
            let p0 = pts[i];
            let p1 = pts[i + 1];
            Cubic {
                p0,
                c1: p0 + tangent(i) * s,
                c2: p1 - tangent(i + 1) * s,
                p3: p1,
            }
        })
        .collect()
}

/// A cubic chain traced backwards: reversed order, each segment reversed.
#[must_use]
pub fn reverse_cubics(cs: &[Cubic]) -> Vec<Cubic> {
    cs.iter().rev().map(Cubic::reversed).collect()
}

/// Clamp every handle to at most half its segment's chord, in place.
///
/// Returns how many SEGMENTS had at least one handle shortened — the quality
/// gate compares that count against the segment total (more than 2% means the
/// sampling is too coarse).
///
/// On well-behaved arclength-uniform data the handle/chord ratio is 1/3 and
/// this never fires. When it does, it has caught data that would otherwise
/// have produced a loop inside one segment.
pub fn clamp_handles(cs: &mut [Cubic]) -> usize {
    let mut fired = 0usize;
    for c in cs.iter_mut() {
        let d = (c.p3 - c.p0).len();
        if d == 0.0 {
            continue;
        }
        let lim = 0.5 * d;
        let mut hit = false;
        let h1 = c.c1 - c.p0;
        let l1 = h1.len();
        if l1 > lim {
            c.c1 = c.p0 + h1 * (lim / l1);
            hit = true;
        }
        let h2 = c.c2 - c.p3;
        let l2 = h2.len();
        if l2 > lim {
            c.c2 = c.p3 + h2 * (lim / l2);
            hit = true;
        }
        if hit {
            fired += 1;
        }
    }
    fired
}

/// Largest angle between consecutive segment chords, in radians.
///
/// A turn over 60 degrees anywhere means the point sampling was too coarse for
/// the curvature and the Bezier can loop; the fix is to double the sample count
/// and rebuild, not to soften the tension.
///
/// If the chain closes (`last.p3 == first.p0`) the wrap-around turn is included,
/// so a closed contour is measured completely.
#[must_use]
pub fn max_turn(cs: &[Cubic]) -> f64 {
    let n = cs.len();
    if n < 2 {
        return 0.0;
    }
    let closed = cs[n - 1].p3 == cs[0].p0;
    let mut worst = 0.0f64;
    for i in 0..n - 1 {
        worst = worst.max(angle_between(cs[i].chord(), cs[i + 1].chord()));
    }
    if closed {
        worst = worst.max(angle_between(cs[n - 1].chord(), cs[0].chord()));
    }
    worst
}

/// Sample a cubic chain into a polyline, `per_segment` points per cubic.
///
/// The terminal point is appended only when the chain does NOT close, so a
/// closed contour comes back without a duplicated first/last point — which is
/// what [`self_intersects`] and the polygon helpers in [`crate::math`] expect.
#[must_use]
pub fn flatten(cs: &[Cubic], per_segment: usize) -> Vec<V2> {
    let k = per_segment.max(1);
    if cs.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(cs.len() * k + 1);
    for c in cs {
        for j in 0..k {
            out.push(c.at(j as f64 / k as f64));
        }
    }
    let last = cs[cs.len() - 1].p3;
    if last != cs[0].p0 {
        out.push(last);
    }
    out
}

/// Does the polyline cross itself?
///
/// Uniform-grid broadphase, so O(n) on the well-spread data this crate
/// produces. Adjacent segments are skipped — they share an endpoint by
/// definition — as is the closing pair when `closed`.
///
/// Given the polar construction this should always be false; it is here to
/// catch an implementation error, not a design one.
#[must_use]
pub fn self_intersects(pts: &[V2], closed: bool) -> bool {
    let n = pts.len();
    let segs = if closed {
        if n < 4 {
            return false;
        }
        n
    } else {
        if n < 4 {
            return false;
        }
        n - 1
    };

    // Bounding box and mean segment length set the cell size: about two
    // segments per cell keeps the per-cell pair count small without letting
    // one segment span many cells.
    let mut lo = pts[0];
    let mut hi = pts[0];
    let mut total = 0.0;
    for i in 0..segs {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        lo = V2::new(lo.x.min(a.x), lo.y.min(a.y));
        hi = V2::new(hi.x.max(a.x), hi.y.max(a.y));
        total += (b - a).len();
    }
    let w = (hi.x - lo.x).max(1e-12);
    let h = (hi.y - lo.y).max(1e-12);
    let mean = (total / segs as f64).max(1e-12);
    let cell = (2.0 * mean).max(1e-12);
    // The 256 cap bounds memory on a pathological aspect ratio; the broadphase
    // degrades to a few extra narrowphase tests, never to a wrong answer.
    let nx = ((w / cell).ceil() as usize).clamp(1, 256);
    let ny = ((h / cell).ceil() as usize).clamp(1, 256);
    let cw = w / nx as f64;
    let ch = h / ny as f64;

    let mut cells: Vec<Vec<u32>> = vec![Vec::new(); nx * ny];
    let bucket = |v: f64, origin: f64, size: f64, count: usize| -> usize {
        (((v - origin) / size).floor().max(0.0) as usize).min(count - 1)
    };
    for i in 0..segs {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let x0 = bucket(a.x.min(b.x), lo.x, cw, nx);
        let x1 = bucket(a.x.max(b.x), lo.x, cw, nx);
        let y0 = bucket(a.y.min(b.y), lo.y, ch, ny);
        let y1 = bucket(a.y.max(b.y), lo.y, ch, ny);
        for gy in y0..=y1 {
            for gx in x0..=x1 {
                cells[gy * nx + gx].push(i as u32);
            }
        }
    }

    // Vec-ordered buckets, never a HashMap: iteration order has to be the
    // insertion order on every platform.
    for bucket in &cells {
        for (k, &ia) in bucket.iter().enumerate() {
            for &ib in &bucket[k + 1..] {
                let (i, j) = (ia as usize, ib as usize);
                // Adjacent segments share an endpoint; so do the first and
                // last of a closed loop.
                if j == i + 1 || (closed && i == 0 && j == segs - 1) {
                    continue;
                }
                let a0 = pts[i];
                let a1 = pts[(i + 1) % n];
                let b0 = pts[j];
                let b1 = pts[(j + 1) % n];
                if seg_intersect(a0, a1, b0, b1).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{polygon_area, sin, sin_cos, PI, TAU};

    fn circle_pts(m: usize, r: f64) -> Vec<V2> {
        (0..m)
            .map(|i| {
                let (s, c) = sin_cos(TAU * i as f64 / m as f64);
                V2::new(r * c, r * s)
            })
            .collect()
    }

    /// A deterministic jitter so the reversal test runs on data with no exact
    /// zeros or repeated coordinates.
    fn scribble(n: usize) -> Vec<V2> {
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
        };
        (0..n)
            .map(|i| {
                let t = i as f64 * 0.37 + 0.11;
                V2::new(
                    t * 13.0 + next() * 3.0 + 0.5,
                    // The crate's own sine, not libm's, even in a fixture:
                    // a one-ulp difference here could flip a borderline
                    // assertion on another target, which would read as a bug
                    // in the code under test rather than in the test data.
                    sin(t * 1.7) * 20.0 + next() * 3.0 - 7.25,
                )
            })
            .collect()
    }

    /// A band-limited lumpy ring at the given fBm-equivalent amplitude: the
    /// artwork's measured spectrum is h2 0.261, h3 0.155 and everything from
    /// h4 up averaging 0.027, so three harmonics in those proportions stand in
    /// for the real silhouette.
    fn lumpy_ring(seed: u32, amp: f64) -> crate::ring::Ring {
        let k = f64::from(seed) * 0.7;
        let mut ring = crate::ring::Ring::build(V2::ZERO, 720, |th, _| {
            let f = 0.60 * crate::math::sin(2.0 * th + k)
                + 0.35 * crate::math::sin(3.0 * th - k)
                + 0.12 * crate::math::sin(7.0 * th + 2.0 * k);
            300.0 * (1.0 + amp * f)
        });
        ring.clamp_min(0.25 * 300.0);
        ring
    }

    #[test]
    fn cubic_endpoints_and_reversal() {
        let c = Cubic {
            p0: V2::new(0.0, 0.0),
            c1: V2::new(1.0, 2.0),
            c2: V2::new(3.0, 2.0),
            p3: V2::new(4.0, 0.0),
        };
        assert_eq!(c.at(0.0), c.p0);
        assert_eq!(c.at(1.0), c.p3);
        assert_eq!(c.chord(), V2::new(4.0, 0.0));
        let r = c.reversed();
        assert_eq!(r.p0, c.p3);
        assert_eq!(r.c1, c.c2);
        assert_eq!(r.c2, c.c1);
        assert_eq!(r.p3, c.p0);
        assert_eq!(r.reversed(), c);
        // Same curve, opposite parameterisation.
        for k in 0..=10 {
            let t = f64::from(k) / 10.0;
            assert!((c.at(t) - r.at(1.0 - t)).len() < 1e-14);
        }
    }

    #[test]
    fn closed_catmull_rom_reproduces_a_circle() {
        // SPEC.md's own sanity check: at tau = 0.5 and M = 256 the maximum
        // radial error is about 2e-7 * R. This is the single best test that
        // the 1/6 coefficient is right — get tau or the neighbour indices
        // wrong and it fails by orders of magnitude.
        let r = 300.0;
        for &m in &[128usize, 256, 512] {
            let pts = circle_pts(m, r);
            let cs = catmull_rom_closed(&pts, 0.5);
            assert_eq!(cs.len(), m);
            let flat = flatten(&cs, 7);
            let mut worst = 0.0f64;
            for p in &flat {
                worst = worst.max((p.len() - r).abs());
            }
            assert!(worst < 1e-6 * r, "m={m}: radial error {worst} on R={r}");
        }
    }

    #[test]
    fn closed_catmull_rom_is_continuous_at_every_joint() {
        let pts = circle_pts(64, 10.0);
        let cs = catmull_rom_closed(&pts, 0.5);
        for i in 0..cs.len() {
            let j = (i + 1) % cs.len();
            assert_eq!(cs[i].p3, cs[j].p0);
            // C1: the incoming and outgoing handles are collinear and equal
            // in length, because both come from the same central difference.
            let out = cs[j].c1 - cs[j].p0;
            let inc = cs[i].p3 - cs[i].c2;
            assert!((out - inc).len() < 1e-12, "tangent break at {i}");
        }
    }

    #[test]
    fn closed_catmull_rom_needs_three_points() {
        assert!(catmull_rom_closed(&[], 0.5).is_empty());
        assert!(catmull_rom_closed(&[V2::ZERO, V2::new(1.0, 1.0)], 0.5).is_empty());
    }

    #[test]
    fn tau_scales_the_handles_linearly() {
        let pts = circle_pts(32, 5.0);
        let a = catmull_rom_closed(&pts, 0.35);
        let b = catmull_rom_closed(&pts, 0.70);
        for (x, y) in a.iter().zip(&b) {
            let ha = (x.c1 - x.p0).len();
            let hb = (y.c1 - y.p0).len();
            assert!((hb - 2.0 * ha).abs() < 1e-12);
        }
    }

    #[test]
    fn open_catmull_rom_is_bit_exact_under_reversal() {
        // The facet-seam property. Equality is compared on raw bit patterns,
        // not with `==`, so a +0.0/-0.0 slip would also be caught.
        for n in 2..40usize {
            let pts = scribble(n);
            let mut rev = pts.clone();
            rev.reverse();
            let fwd = reverse_cubics(&catmull_rom_open(&pts, 0.5));
            let bwd = catmull_rom_open(&rev, 0.5);
            assert_eq!(fwd.len(), bwd.len(), "n={n}");
            for (k, (a, b)) in fwd.iter().zip(&bwd).enumerate() {
                for (x, y) in [(a.p0, b.p0), (a.c1, b.c1), (a.c2, b.c2), (a.p3, b.p3)] {
                    assert_eq!(x.x.to_bits(), y.x.to_bits(), "n={n} cubic {k} x");
                    assert_eq!(x.y.to_bits(), y.y.to_bits(), "n={n} cubic {k} y");
                }
            }
        }
    }

    #[test]
    fn open_catmull_rom_interpolates_its_knots() {
        let pts = scribble(12);
        let cs = catmull_rom_open(&pts, 0.5);
        assert_eq!(cs.len(), pts.len() - 1);
        for (i, c) in cs.iter().enumerate() {
            assert_eq!(c.p0, pts[i]);
            assert_eq!(c.p3, pts[i + 1]);
        }
        assert!(catmull_rom_open(&[V2::ZERO], 0.5).is_empty());
    }

    #[test]
    fn reverse_cubics_round_trips() {
        let cs = catmull_rom_open(&scribble(9), 0.5);
        let back = reverse_cubics(&reverse_cubics(&cs));
        assert_eq!(cs, back);
    }

    #[test]
    fn clamp_handles_fires_only_on_bad_data() {
        // Arclength-uniform data: the ratio is 1/3, so nothing is clamped.
        let mut good = catmull_rom_closed(&circle_pts(256, 300.0), 0.5);
        assert_eq!(clamp_handles(&mut good), 0);

        // A single overlong handle.
        let mut bad = vec![Cubic {
            p0: V2::new(0.0, 0.0),
            c1: V2::new(10.0, 0.0),
            c2: V2::new(1.0, 0.0),
            p3: V2::new(2.0, 0.0),
        }];
        assert_eq!(clamp_handles(&mut bad), 1);
        assert!((bad[0].c1 - bad[0].p0).len() <= 0.5 * 2.0 + 1e-12);
        assert!((bad[0].c2 - bad[0].p3).len() <= 0.5 * 2.0 + 1e-12);
        // Idempotent.
        assert_eq!(clamp_handles(&mut bad), 0);

        // A degenerate segment is left alone rather than dividing by zero.
        let mut zero = vec![Cubic {
            p0: V2::ZERO,
            c1: V2::new(1.0, 0.0),
            c2: V2::new(1.0, 0.0),
            p3: V2::ZERO,
        }];
        assert_eq!(clamp_handles(&mut zero), 0);
    }

    #[test]
    fn max_turn_on_a_circle_is_one_step() {
        let m = 64;
        let cs = catmull_rom_closed(&circle_pts(m, 10.0), 0.5);
        let want = TAU / m as f64;
        let got = max_turn(&cs);
        assert!((got - want).abs() < 1e-9, "{got} vs {want}");
        assert_eq!(max_turn(&[]), 0.0);
        assert_eq!(max_turn(&cs[..1]), 0.0);
    }

    #[test]
    fn max_turn_includes_the_wrap_on_a_closed_chain() {
        // A square: every corner is a right angle, including the one between
        // the last and first chords.
        let pts = [
            V2::new(0.0, 0.0),
            V2::new(1.0, 0.0),
            V2::new(1.0, 1.0),
            V2::new(0.0, 1.0),
        ];
        let line = |a: V2, b: V2| Cubic {
            p0: a,
            c1: a,
            c2: b,
            p3: b,
        };
        let cs: Vec<Cubic> = (0..4).map(|i| line(pts[i], pts[(i + 1) % 4])).collect();
        assert!((max_turn(&cs) - PI / 2.0).abs() < 1e-12);
        // Drop the closing segment and the wrap must not be counted.
        assert!((max_turn(&cs[..3]) - PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn flatten_shapes() {
        let cs = catmull_rom_open(&scribble(5), 0.5);
        let f = flatten(&cs, 4);
        // Open chain: a terminal point is appended.
        assert_eq!(f.len(), cs.len() * 4 + 1);
        assert_eq!(f[0], cs[0].p0);
        assert_eq!(f[f.len() - 1], cs[cs.len() - 1].p3);

        // Closed chain: no duplicate point.
        let closed = catmull_rom_closed(&circle_pts(16, 2.0), 0.5);
        let fc = flatten(&closed, 3);
        assert_eq!(fc.len(), 16 * 3);
        assert_ne!(fc[0], fc[fc.len() - 1]);

        assert!(flatten(&[], 4).is_empty());
        // per_segment = 0 is treated as 1 rather than losing the curve.
        assert_eq!(flatten(&closed, 0).len(), 16);
    }

    #[test]
    fn self_intersects_finds_a_figure_eight() {
        let bow = [
            V2::new(0.0, 0.0),
            V2::new(10.0, 10.0),
            V2::new(10.0, 0.0),
            V2::new(0.0, 10.0),
        ];
        assert!(self_intersects(&bow, true));
    }

    #[test]
    fn self_intersects_clears_convex_and_concave_loops() {
        assert!(!self_intersects(&circle_pts(256, 100.0), true));
        // An L — concave, but simple.
        let l = [
            V2::new(0.0, 0.0),
            V2::new(20.0, 0.0),
            V2::new(20.0, 10.0),
            V2::new(10.0, 10.0),
            V2::new(10.0, 20.0),
            V2::new(0.0, 20.0),
        ];
        assert!(!self_intersects(&l, true));
        // Open polylines: the closing chord must not be tested.
        assert!(!self_intersects(&l, false));
    }

    #[test]
    fn self_intersects_finds_a_pentagram() {
        let star: Vec<V2> = (0..5)
            .map(|i| {
                let (s, c) = sin_cos(TAU * (2 * i) as f64 / 5.0);
                V2::new(c * 50.0, s * 50.0)
            })
            .collect();
        assert!(self_intersects(&star, true));
    }

    #[test]
    fn self_intersects_ignores_shared_endpoints() {
        // A sharp spike folds back on itself: the two segments touch only at
        // the shared vertex, which must not count.
        let spike = [
            V2::new(0.0, 0.0),
            V2::new(10.0, 0.0),
            V2::new(10.0, 0.001),
            V2::new(0.0, 0.002),
            V2::new(-1.0, 5.0),
        ];
        assert!(!self_intersects(&spike, false));
        assert!(!self_intersects(&[V2::ZERO, V2::new(1.0, 0.0)], false));
    }

    #[test]
    fn a_smoothed_star_shaped_ring_never_crosses_itself() {
        // The end-to-end invariant: a nasty single-valued r(theta), resampled
        // to uniform arclength and smoothed, stays simple. If the polar
        // theorem, the resample and the Catmull-Rom formula are all right,
        // this cannot fail.
        // Amplitude 0.22 and the measured band-limited spectrum: the real
        // pipeline at its defaults. Here every Bezier health gate must be
        // comfortably clear, not merely survived.
        for seed in 0..8u32 {
            let mut ring = lumpy_ring(seed, 0.22);
            ring.limit_log_lipschitz(1.10);
            let rs = ring.resampled(256);
            let mut cs = catmull_rom_closed(&rs, 0.5);
            assert_eq!(clamp_handles(&mut cs), 0, "seed {seed}");
            // SPEC.md measures ~14 degrees at these defaults.
            let turn = max_turn(&cs);
            assert!(turn < PI / 6.0, "seed {seed}: max turn {turn} rad");
            let flat = flatten(&cs, 6);
            assert!(!self_intersects(&flat, true), "seed {seed}");
            assert!(polygon_area(&flat).abs() > 0.0);
        }
    }

    #[test]
    fn an_abusive_amplitude_still_cannot_make_the_outline_cross() {
        // Amplitude 0.6 is well past the parameter range, and the limiter
        // mauls it into corners the Bezier has to negotiate at nearly 90
        // degrees per segment. The handle clamp may fire and max_turn goes
        // past the 60-degree gate — both of those are the gates DOING their
        // job — but the star-shape theorem still forbids a crossing.
        for seed in 0..8u32 {
            let mut ring = lumpy_ring(seed, 0.60);
            ring.limit_log_lipschitz(1.10);
            let rs = ring.resampled(256);
            let mut cs = catmull_rom_closed(&rs, 0.5);
            clamp_handles(&mut cs);
            let flat = flatten(&cs, 8);
            assert!(!self_intersects(&flat, true), "seed {seed}");
        }
    }
}
