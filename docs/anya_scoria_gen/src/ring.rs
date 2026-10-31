//! A star-shaped closed curve, stored as `r(theta)` on a uniform theta grid.
//!
//! THE THEOREM the whole crate rests on: a curve written as
//! `r(theta) * (cos theta, sin theta)` with `r(theta) > 0` single-valued is
//! star-shaped about its centre, and a star-shaped curve cannot self-intersect
//! — every ray from the centre meets it exactly once. Non-self-intersection is
//! therefore an invariant maintained by construction, not a cleanup pass.
//!
//! It also makes the outline ribbon a SUBTRACTION. A general polygon offset
//! has to detect and remove the loops that appear at every concavity tighter
//! than the offset distance; `r(theta) - w(theta)` has no topology to resolve,
//! no loops to remove, no ordering to get wrong, and inherits
//! non-self-intersection from the same theorem.

use crate::math::{atan2, polygon_area, resample_closed, sin, sin_cos, TAU, V2};

/// A closed curve sampled as `n` radii about a centre, at `theta_i = TAU*i/n`.
#[derive(Clone, Debug)]
pub struct Ring {
    /// The centre every radius is measured from.
    pub c: V2,
    /// Radii on the uniform theta grid; `n = r.len()`.
    pub r: Vec<f64>,
}

impl Ring {
    /// Sample `radius(theta, unit_dir)` at `n` equally spaced angles.
    ///
    /// The callback gets both the angle and its unit direction because almost
    /// every caller needs the direction anyway (to evaluate the flow field at
    /// the boundary estimate, say) and recomputing it would double the trig.
    #[must_use]
    pub fn build(c: V2, n: usize, radius: impl Fn(f64, V2) -> f64) -> Ring {
        let mut r = Vec::with_capacity(n);
        for i in 0..n {
            let th = TAU * i as f64 / n as f64;
            let (s, ct) = sin_cos(th);
            r.push(radius(th, V2::new(ct, s)));
        }
        Ring { c, r }
    }

    /// Number of samples.
    #[must_use]
    pub fn n(&self) -> usize {
        self.r.len()
    }

    /// The angle of sample `i`.
    #[must_use]
    pub fn theta(&self, i: usize) -> f64 {
        TAU * i as f64 / self.r.len() as f64
    }

    /// Sample `i` as a point in SVG space.
    #[must_use]
    pub fn point(&self, i: usize) -> V2 {
        let (s, c) = sin_cos(self.theta(i));
        V2::new(self.c.x + self.r[i] * c, self.c.y + self.r[i] * s)
    }

    /// Every sample as a closed polyline (the last point joins the first).
    #[must_use]
    pub fn points(&self) -> Vec<V2> {
        (0..self.r.len()).map(|i| self.point(i)).collect()
    }

    /// Raise every radius to at least `rmin`.
    ///
    /// The first of the four defences against breaking star-shapedness: `r`
    /// must stay strictly positive. Run it BEFORE the limiter, so the limiter
    /// smooths whatever the clamp left behind.
    pub fn clamp_min(&mut self, rmin: f64) {
        for v in &mut self.r {
            if *v < rmin {
                *v = rmin;
            }
        }
    }

    /// Enforce `|d(ln r)/d theta| <= kappa`, in place.
    ///
    /// Equivalently `|r'| <= kappa * r`: the boundary never makes an angle
    /// steeper than `atan(kappa)` with the radial direction. That is the
    /// anti-cusp guard — a radial slope steeper than this produces a near-cusp
    /// that the Bezier interpolation turns into a visible loop.
    ///
    /// Two wraparound sweeps per direction, erode (clip spikes) then dilate
    /// (fill notches), exactly as SPEC.md specifies. A full forward sweep
    /// followed by a full backward sweep already propagates a constraint the
    /// whole way round in both directions, so the pair of iterations is belt
    /// and braces; the result is the exact fixed point and therefore
    /// idempotent.
    ///
    /// A finding SPEC.md does not state: the dilate half can never change
    /// anything, because `r_i >= r_j*flo` is the same inequality as
    /// `r_j <= r_i*cap`, which the erode half has already enforced in both
    /// directions. It is kept because the spec specifies it, it costs 4N
    /// comparisons, and it makes the two-sided bound obvious at the call site.
    /// The practical consequence is that this operator is purely EROSIVE: a
    /// deep notch is not filled in, it drags its neighbourhood down to meet it.
    /// `tests::the_dilate_half_is_a_no_op_after_the_erode_half` pins that.
    ///
    /// TRAP: the base ellipse itself needs `kappa >= (aspect^2-1)/(2*aspect)`
    /// — 0.417 at aspect 1.50, 0.75 at aspect 2.0. Below that this quietly
    /// circularises the blob and the `aspect` parameter appears broken. Callers
    /// must validate kappa, not clamp it.
    pub fn limit_log_lipschitz(&mut self, kappa: f64) {
        let n = self.r.len();
        if n < 2 {
            return;
        }
        // Per-sample budget. Using the ratio (1 + g) rather than exp(g) is
        // slightly stricter than the stated bound, which is the safe side, and
        // it keeps the inner loop to one multiply.
        let g = kappa * (TAU / n as f64);
        let cap = 1.0 + g;
        let flo = 1.0 / (1.0 + g);
        for _ in 0..2 {
            for i in 0..n {
                let j = (i + n - 1) % n;
                self.r[i] = self.r[i].min(self.r[j] * cap);
            }
            for i in (0..n).rev() {
                let j = (i + 1) % n;
                self.r[i] = self.r[i].min(self.r[j] * cap);
            }
        }
        for _ in 0..2 {
            for i in 0..n {
                let j = (i + n - 1) % n;
                self.r[i] = self.r[i].max(self.r[j] * flo);
            }
            for i in (0..n).rev() {
                let j = (i + 1) % n;
                self.r[i] = self.r[i].max(self.r[j] * flo);
            }
        }
    }

    /// `dr/dtheta` by central difference with wraparound.
    ///
    /// The ribbon's slope correction needs this: the boundary normal makes an
    /// angle `phi` with the radial direction where `tan(phi) = r'/r`, so a
    /// radial step of `w` only produces a perpendicular band of `w*cos(phi)`.
    #[must_use]
    pub fn radial_derivative(&self) -> Vec<f64> {
        let n = self.r.len();
        if n < 2 {
            return vec![0.0; n];
        }
        let two_dtheta = 2.0 * (TAU / n as f64);
        (0..n)
            .map(|i| (self.r[(i + 1) % n] - self.r[(i + n - 1) % n]) / two_dtheta)
            .collect()
    }

    /// A new ring at `r_i - w_i`, floored at `floor`.
    ///
    /// Both rings are polar functions of the SAME theta grid, so both are
    /// star-shaped by the same theorem and neither can cross the other — which
    /// is the entire argument for building the silhouette in polar form.
    ///
    /// Entries of `w` past the end of the ring count as zero rather than
    /// panicking; callers pass one width per sample.
    #[must_use]
    pub fn offset_inward(&self, w: &[f64], floor: f64) -> Ring {
        let r = self
            .r
            .iter()
            .enumerate()
            .map(|(i, &ri)| {
                let wi = if i < w.len() { w[i] } else { 0.0 };
                (ri - wi).max(floor)
            })
            .collect();
        Ring { c: self.c, r }
    }

    /// The radius in the direction `d`, linearly interpolated between the two
    /// bracketing samples.
    ///
    /// O(1): the samples sit on a uniform theta grid, so the bracketing index
    /// is an index computation, not a search. The vesicle packer calls this
    /// tens of thousands of times per logo.
    ///
    /// A zero direction returns `r[0]`.
    #[must_use]
    pub fn radius_toward(&self, d: V2) -> f64 {
        let n = self.r.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.r[0];
        }
        let mut th = atan2(d.y, d.x);
        if !th.is_finite() {
            return self.r[0];
        }
        if th < 0.0 {
            th += TAU;
        }
        let u = th * (n as f64 / TAU);
        let fl = u.floor();
        // `fl` can land on n exactly when th rounds up to TAU; the modulo
        // catches it.
        let i0 = (fl as usize) % n;
        let f = u - fl;
        let a = self.r[i0];
        let b = self.r[(i0 + 1) % n];
        a + (b - a) * f
    }

    /// Is `q` inside the ring with at least `clearance` to spare?
    ///
    /// O(1) by star-shapedness — no polygon walk. This is the cheapest of the
    /// three vesicle collision tests and the one that runs first.
    #[must_use]
    pub fn contains(&self, q: V2, clearance: f64) -> bool {
        let d = q - self.c;
        d.len() + clearance <= self.radius_toward(d)
    }

    /// The ring as `m` points at uniform arclength.
    #[must_use]
    pub fn resampled(&self, m: usize) -> Vec<V2> {
        resample_closed(&self.points(), m)
    }

    /// Mean of the radii. Zero for an empty ring.
    #[must_use]
    pub fn mean_radius(&self) -> f64 {
        let n = self.r.len();
        if n == 0 {
            return 0.0;
        }
        self.r.iter().sum::<f64>() / n as f64
    }

    /// Area of the sampled polygon, unsigned.
    ///
    /// Closed form rather than a shoelace over [`Ring::points`]: on a uniform
    /// theta grid every triangle shares the same apex angle, so the whole sum
    /// collapses to `0.5 * sin(dtheta) * sum(r_i * r_{i+1})`. It is the
    /// POLYGON's area, i.e. it under-estimates the smooth polar area by
    /// `O(dtheta^2)` — which is the right answer, because the polygon is what
    /// downstream area gates compare against.
    #[must_use]
    pub fn area(&self) -> f64 {
        let n = self.r.len();
        if n < 3 {
            return 0.0;
        }
        let dtheta = TAU / n as f64;
        let mut acc = 0.0;
        for i in 0..n {
            acc += self.r[i] * self.r[(i + 1) % n];
        }
        (0.5 * sin(dtheta) * acc).abs()
    }

    /// Signed area of the sampled polygon, for callers that care about
    /// winding. Convenience over [`crate::math::polygon_area`] on
    /// [`Ring::points`].
    #[must_use]
    pub fn signed_area(&self) -> f64 {
        polygon_area(&self.points())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{closed_len, cos, pangle, PI};
    use crate::rng::{stream, Pcg32};

    const KAPPA: f64 = 1.10;

    fn circle(n: usize, rad: f64) -> Ring {
        Ring::build(V2::ZERO, n, |_, _| rad)
    }

    /// `r(theta)` of an ellipse with the given aspect and geometric mean
    /// radius `r0`, exactly as SPEC.md's base ellipse.
    fn ellipse(n: usize, r0: f64, aspect: f64) -> Ring {
        let a = r0 * aspect.sqrt();
        let b = r0 / aspect.sqrt();
        Ring::build(V2::ZERO, n, |_, d| {
            a * b / ((b * d.x) * (b * d.x) + (a * d.y) * (a * d.y)).sqrt()
        })
    }

    /// The property the limiter promises, checked at every sample including
    /// the wrap.
    fn lipschitz_ok(ring: &Ring, kappa: f64) -> f64 {
        let n = ring.n();
        let dtheta = TAU / n as f64;
        let mut worst = 0.0f64;
        for i in 0..n {
            let j = (i + n - 1) % n;
            // ln without libm: ln(a/b) is what we want, and f64::ln is not on
            // the banned list for TESTS — only the generator must avoid it.
            let d = (ring.r[i] / ring.r[j]).ln().abs();
            worst = worst.max(d / (kappa * dtheta));
        }
        worst
    }

    #[test]
    fn a_circle_round_trips() {
        let ring = circle(360, 50.0);
        assert_eq!(ring.n(), 360);
        assert!((ring.mean_radius() - 50.0).abs() < 1e-13);
        for i in 0..ring.n() {
            let p = ring.point(i);
            assert!((p.len() - 50.0).abs() < 1e-12);
            assert!((ring.theta(i) - TAU * i as f64 / 360.0).abs() < 1e-15);
        }
        // Polygon area, not pi r^2: a 360-gon falls 0.005% short.
        let want = PI * 2500.0;
        let got = ring.area();
        assert!(got < want && got > want * 0.9999, "{got} vs {want}");
        assert!((closed_len(&ring.points()) - TAU * 50.0).abs() < 0.01);
    }

    #[test]
    fn radius_toward_is_exact_on_a_circle() {
        let ring = circle(64, 7.5);
        for i in 0..1000 {
            let th = TAU * f64::from(i) / 1000.0 - PI;
            let (s, c) = sin_cos(th);
            let got = ring.radius_toward(V2::new(c * 3.0, s * 3.0));
            assert!((got - 7.5).abs() < 1e-12, "{got} at {th}");
        }
        assert_eq!(ring.radius_toward(V2::ZERO), 7.5);
    }

    #[test]
    fn radius_toward_hits_the_samples_exactly() {
        // A ring with every radius different, so an off-by-one shows up.
        let n = 32;
        let ring = Ring::build(V2::new(10.0, -4.0), n, |_, d| 20.0 + 5.0 * d.x + 2.0 * d.y);
        for i in 0..n {
            let (s, c) = sin_cos(ring.theta(i));
            let got = ring.radius_toward(V2::new(c, s));
            assert!(
                (got - ring.r[i]).abs() < 1e-11,
                "sample {i}: {got} vs {}",
                ring.r[i]
            );
        }
        // And between samples it interpolates, monotonically.
        let (s0, c0) = sin_cos(ring.theta(0));
        let (s1, c1) = sin_cos(ring.theta(1));
        let mid = V2::new((c0 + c1) * 0.5, (s0 + s1) * 0.5);
        let got = ring.radius_toward(mid);
        let lo = ring.r[0].min(ring.r[1]);
        let hi = ring.r[0].max(ring.r[1]);
        assert!(got > lo && got < hi, "{got} not between {lo} and {hi}");
    }

    #[test]
    fn radius_toward_agrees_with_the_pseudo_angle_ordering() {
        // The samples' pseudo-angles are monotone, which is what makes the
        // uniform-theta grid searchable at all; assert it here so a change to
        // `pangle` cannot silently break the packer's assumption.
        let n = 128;
        let ring = circle(n, 3.0);
        let mut prev = -1.0;
        for i in 0..n {
            let (s, c) = sin_cos(ring.theta(i));
            let p = pangle(V2::new(c, s));
            assert!(p > prev, "pangle not increasing at {i}");
            prev = p;
        }
    }

    #[test]
    fn contains_respects_clearance() {
        let ring = circle(256, 100.0);
        assert!(ring.contains(V2::ZERO, 50.0));
        assert!(ring.contains(V2::new(80.0, 0.0), 19.0));
        assert!(!ring.contains(V2::new(80.0, 0.0), 21.0));
        assert!(!ring.contains(V2::new(200.0, 0.0), 0.0));
        // Off-centre ring.
        let off = Ring::build(V2::new(50.0, 50.0), 128, |_, _| 10.0);
        assert!(off.contains(V2::new(50.0, 50.0), 9.0));
        assert!(!off.contains(V2::ZERO, 0.0));
    }

    #[test]
    fn clamp_min_lifts_only_what_is_low() {
        let mut ring = Ring::build(V2::ZERO, 8, |_, d| 10.0 + 9.0 * d.x);
        let before = ring.r.clone();
        ring.clamp_min(5.0);
        for (a, b) in before.iter().zip(&ring.r) {
            assert_eq!(*b, a.max(5.0));
        }
        assert!(ring.r.iter().all(|&v| v >= 5.0));
    }

    #[test]
    fn the_limiter_tames_a_single_huge_spike() {
        let n = 512;
        let mut ring = circle(n, 100.0);
        ring.r[123] = 4000.0;
        ring.limit_log_lipschitz(KAPPA);
        let w = lipschitz_ok(&ring, KAPPA);
        assert!(w <= 1.0001, "worst ratio {w}");
    }

    #[test]
    fn the_limiter_turns_a_deep_notch_into_a_funnel() {
        let n = 512;
        let mut ring = circle(n, 100.0);
        for i in 200..206 {
            ring.r[i] = 0.5;
        }
        ring.limit_log_lipschitz(KAPPA);
        assert!(lipschitz_ok(&ring, KAPPA) <= 1.0001);
        // The operator is erosive, so the notch floor stays where it was and
        // the cliff beside it is what gets removed: the radius now climbs
        // smoothly, sample by sample, away from the notch.
        assert_eq!(ring.r[202], 0.5);
        // The two walls of the funnel meet roughly opposite the notch, so
        // check monotonicity only up to there.
        for i in 206..450 {
            assert!(ring.r[i] >= ring.r[i - 1], "not monotone leaving the notch");
        }
        assert!(ring.r[450] > 5.0, "the far side should have recovered");
        assert!(ring.r.iter().all(|&v| v > 0.0));
    }

    #[test]
    fn the_dilate_half_is_a_no_op_after_the_erode_half() {
        // `r_i >= r_j*flo` and `r_j <= r_i*cap` are the same inequality, so
        // the dilate sweeps cannot touch a ring the erode sweeps have already
        // made compliant. Asserted here because the behaviour reads as
        // surprising — SPEC.md calls the second half "fill notches".
        let n = 300;
        let mut rng = Pcg32::new(606, stream::BLOB);
        let mut ring = Ring::build(V2::ZERO, n, |_, _| 0.0);
        for v in &mut ring.r {
            *v = rng.range(0.5, 400.0);
        }
        let mut erode_only = ring.clone();
        let g = KAPPA * (TAU / n as f64);
        let cap = 1.0 + g;
        for _ in 0..2 {
            for i in 0..n {
                let j = (i + n - 1) % n;
                erode_only.r[i] = erode_only.r[i].min(erode_only.r[j] * cap);
            }
            for i in (0..n).rev() {
                let j = (i + 1) % n;
                erode_only.r[i] = erode_only.r[i].min(erode_only.r[j] * cap);
            }
        }
        ring.limit_log_lipschitz(KAPPA);
        let a: Vec<u64> = ring.r.iter().map(|v| v.to_bits()).collect();
        let b: Vec<u64> = erode_only.r.iter().map(|v| v.to_bits()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn the_limiter_survives_the_wrap() {
        // Spike at index 0 and notch at n-1: both constraints have to travel
        // across the seam, in both directions.
        let n = 256;
        let mut ring = circle(n, 50.0);
        ring.r[0] = 900.0;
        ring.r[n - 1] = 1.0;
        ring.limit_log_lipschitz(KAPPA);
        assert!(lipschitz_ok(&ring, KAPPA) <= 1.0001);
    }

    #[test]
    fn the_limiter_handles_full_amplitude_noise() {
        let mut rng = Pcg32::new(4, stream::BLOB);
        for _ in 0..20 {
            let n = 360;
            let mut ring = Ring::build(V2::ZERO, n, |_, _| 0.0);
            for v in &mut ring.r {
                // Amplitude 1.0 about a unit base: r in [0.01, 2.0].
                *v = (1.0 + rng.range(-1.0, 1.0)).max(0.01) * 100.0;
            }
            ring.limit_log_lipschitz(KAPPA);
            let w = lipschitz_ok(&ring, KAPPA);
            assert!(w <= 1.0001, "worst ratio {w}");
            assert!(ring.r.iter().all(|&v| v > 0.0));
        }
    }

    #[test]
    fn the_limiter_is_idempotent() {
        let mut rng = Pcg32::new(11, stream::BLOB);
        let n = 400;
        let mut ring = Ring::build(V2::ZERO, n, |_, _| 0.0);
        for v in &mut ring.r {
            *v = rng.range(10.0, 300.0);
        }
        ring.limit_log_lipschitz(KAPPA);
        let once: Vec<u64> = ring.r.iter().map(|v| v.to_bits()).collect();
        ring.limit_log_lipschitz(KAPPA);
        let twice: Vec<u64> = ring.r.iter().map(|v| v.to_bits()).collect();
        assert_eq!(once, twice, "a second pass must change nothing at all");
    }

    #[test]
    fn an_aspect_1_5_ellipse_survives_kappa_1_10_untouched() {
        // The ellipse's own max |r'/r| is (aspect^2-1)/(2*aspect) = 0.4167, so
        // kappa 1.10 must leave it alone. If this starts failing, the limiter
        // has begun eating the `aspect` parameter.
        let n = 720;
        let ring = ellipse(n, 300.0, 1.5);
        let mut limited = ring.clone();
        limited.limit_log_lipschitz(KAPPA);
        let mut worst = 0.0f64;
        for (a, b) in ring.r.iter().zip(&limited.r) {
            worst = worst.max((a - b).abs() / a);
        }
        assert!(worst < 1e-12, "the ellipse moved by {worst} relative");
    }

    #[test]
    fn a_kappa_below_the_ellipses_own_slope_circularises_it() {
        // The documented trap, asserted so nobody "fixes" the validation away.
        let n = 720;
        let ring = ellipse(n, 300.0, 1.5);
        let spread = |r: &Ring| {
            let hi = r.r.iter().cloned().fold(f64::MIN, f64::max);
            let lo = r.r.iter().cloned().fold(f64::MAX, f64::min);
            hi / lo
        };
        let before = spread(&ring);
        let mut crushed = ring.clone();
        crushed.limit_log_lipschitz(0.10);
        let after = spread(&crushed);
        assert!(before > 1.4, "aspect 1.5 should span ~1.5x in radius");
        assert!(
            after < 1.2,
            "kappa 0.10 should have flattened it, got {after}"
        );
    }

    #[test]
    fn radial_derivative_matches_the_analytic_one() {
        // r = R*(1 + 0.1*cos theta)  ->  r' = -0.1*R*sin theta
        let n = 720;
        let big = 200.0;
        let ring = Ring::build(V2::ZERO, n, |th, _| big * (1.0 + 0.1 * cos(th)));
        let d = ring.radial_derivative();
        // A central difference of a sinusoid is exact up to the factor
        // sin(dtheta)/dtheta = 1 - dtheta^2/6, which at n = 720 is 1.3e-5
        // relative — about 2.5e-4 absolute on this amplitude.
        for (i, &got) in d.iter().enumerate() {
            let want = -0.1 * big * sin(ring.theta(i));
            assert!((got - want).abs() < 5e-4, "i={i}: {got} vs {want}");
        }
        // Including the two wrap-around samples, which is the easy bug: a
        // missing `% n` leaves a huge spike at index 0 or n-1.
        assert!(d[0].abs() < 1e-9, "wrap at 0 is broken: {}", d[0]);
        assert!((d[n - 1] - (-0.1 * big * sin(ring.theta(n - 1)))).abs() < 5e-4);
    }

    #[test]
    fn offset_inward_subtracts_and_floors() {
        let ring = circle(16, 100.0);
        let w: Vec<f64> = (0..16).map(|i| f64::from(i) * 10.0).collect();
        let inner = ring.offset_inward(&w, 15.0);
        assert_eq!(inner.c, ring.c);
        for (i, &got) in inner.r.iter().enumerate() {
            assert_eq!(got, (100.0 - i as f64 * 10.0).max(15.0));
        }
        // A short width array treats the rest as zero rather than panicking.
        let short = ring.offset_inward(&[5.0], 0.0);
        assert_eq!(short.r[0], 95.0);
        assert_eq!(short.r[1], 100.0);
    }

    #[test]
    fn the_ribbon_pair_never_crosses() {
        // The property the whole polar construction exists to guarantee: a
        // lumpy outer ring and its slope-corrected inward offset stay nested,
        // sample for sample, with no loop removal anywhere.
        let n = 720;
        let mut rng = Pcg32::new(2026, stream::BLOB);
        let r0 = 300.0;
        let mut outer = Ring::build(V2::ZERO, n, |_, _| 0.0);
        // A deliberately nasty profile: several harmonics at full amplitude.
        for i in 0..n {
            let th = TAU * i as f64 / n as f64;
            let f = 0.30 * sin(2.0 * th + 0.4) + 0.22 * sin(5.0 * th) + 0.14 * sin(11.0 * th + 1.0);
            outer.r[i] = r0 * (1.0 + f) * rng.range(0.98, 1.02);
        }
        outer.clamp_min(0.25 * r0);
        outer.limit_log_lipschitz(KAPPA);

        let w0 = 0.058 * r0;
        let rp = outer.radial_derivative();
        let w: Vec<f64> = (0..n)
            .map(|i| {
                let k = (1.0 + (rp[i] / outer.r[i]) * (rp[i] / outer.r[i])).sqrt();
                w0 * k.min(1.60)
            })
            .collect();
        let mut inner = outer.offset_inward(&w, 0.15 * r0);
        inner.limit_log_lipschitz(KAPPA);

        for i in 0..n {
            assert!(inner.r[i] > 0.0);
            assert!(
                inner.r[i] < outer.r[i],
                "sample {i}: inner {} >= outer {}",
                inner.r[i],
                outer.r[i]
            );
        }
        assert!(inner.area() < outer.area());
        // The slope cap must not have fired at the interlocked defaults:
        // sqrt(1 + kappa^2) = 1.487 < 1.60.
        assert!(w.iter().all(|&v| v < 1.60 * w0));
    }

    #[test]
    fn resampled_is_evenly_spaced() {
        let ring = ellipse(720, 300.0, 1.5);
        let pts = ring.resampled(256);
        assert_eq!(pts.len(), 256);
        let want = closed_len(&ring.points()) / 256.0;
        // Chord under-reads arclength by O(curvature^2); the ellipse at this
        // resolution is smooth enough for 1e-3 relative.
        for i in 0..256 {
            let d = (pts[(i + 1) % 256] - pts[i]).len();
            assert!((d - want).abs() < 1e-3 * want, "{d} vs {want} at {i}");
        }
    }

    #[test]
    fn area_matches_the_shoelace() {
        let ring = ellipse(360, 123.0, 1.8);
        let shoe = polygon_area(&ring.points()).abs();
        assert!(
            (ring.area() - shoe).abs() < 1e-7 * shoe,
            "{} vs {shoe}",
            ring.area()
        );
        assert!((ring.signed_area().abs() - shoe).abs() < 1e-9 * shoe);
    }

    #[test]
    fn degenerate_rings_do_not_panic() {
        let empty = Ring {
            c: V2::ZERO,
            r: Vec::new(),
        };
        assert_eq!(empty.n(), 0);
        assert_eq!(empty.area(), 0.0);
        assert_eq!(empty.mean_radius(), 0.0);
        assert_eq!(empty.radius_toward(V2::new(1.0, 1.0)), 0.0);
        assert!(empty.points().is_empty());
        assert!(empty.radial_derivative().is_empty());
        let mut one = Ring {
            c: V2::ZERO,
            r: vec![3.0],
        };
        one.limit_log_lipschitz(1.0);
        assert_eq!(one.r, vec![3.0]);
        assert_eq!(one.radius_toward(V2::new(0.0, -1.0)), 3.0);
        assert_eq!(one.area(), 0.0);
    }
}
