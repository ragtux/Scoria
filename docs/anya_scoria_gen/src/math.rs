//! Vectors, hand-rolled trigonometry and polygon geometry.
//!
//! Nothing here calls libm. `sin`, `cos`, `atan2`, `powf`, `exp` and `hypot`
//! differ in the last ulp between glibc, musl, MSVC and Apple's libm, and one
//! ulp propagates through the log-Lipschitz limiter and the arclength resample
//! into a different third decimal in the emitted SVG — which would destroy the
//! byte-identity guarantee the crate is built on. Only `+ - * /`, `sqrt`,
//! `abs`, `round`, `floor`, `min`, `max` and `total_cmp` are used, all of which
//! IEEE-754 pins exactly.

// The minimax coefficient tables below are quoted digit for digit from
// SPEC.md so they can be diffed against it; several carry more digits than an
// f64 holds, which is harmless (the literal rounds to the same bits) but
// upsets clippy's excessive_precision.
#![allow(clippy::excessive_precision)]

use std::ops::{Add, Mul, Neg, Sub};

/// Pi. Taken from `std` rather than retyped: the literal is a transcription
/// risk for no gain, and a `const` is resolved at compile time on every
/// target identically.
pub const PI: f64 = std::f64::consts::PI;
/// Two pi — a full turn. The ring's theta grid is `TAU * i / n`.
pub const TAU: f64 = std::f64::consts::TAU;

/// Half pi, used by the octant dispatch and by [`atan2`].
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;
/// 2/pi, the multiplier that turns an angle into an octant count.
const TWO_OVER_PI: f64 = std::f64::consts::FRAC_2_PI;

// Pi/2 split in two so that `n * PIO2_HI` is EXACT: PIO2_HI carries only 33
// significant bits, so the product is exact for |n| < 2^20 and the subtraction
// `x - n*PIO2_HI` is then exact by Sterbenz. Together the pair represents pi/2
// to about 86 bits (fdlibm's `pio2_1` / `pio2_1t`).
const PIO2_HI: f64 = 1.570_796_326_734_125_614_17;
const PIO2_LO: f64 = 6.077_100_506_506_192_249_32e-11;

/// Largest argument [`sin`] and [`cos`] reduce exactly: beyond `2^20` octants
/// the two-part pi/2 above stops being enough and accuracy degrades (still
/// deterministically — it is a polynomial either way).
pub const TRIG_EXACT_LIMIT: f64 = 1.6e6;

// Degree-11 odd minimax sine on [-pi/4, pi/4], and degree-12 even cosine.
// Both tables are quoted verbatim from SPEC.md's prng section.
const S1: f64 = -1.666_666_666_666_663_072_95e-1;
const S2: f64 = 8.333_333_333_322_118_588_78e-3;
const S3: f64 = -1.984_126_982_958_953_859_96e-4;
const S4: f64 = 2.755_731_362_138_572_452_13e-6;
const S5: f64 = -2.505_074_776_285_780_728_66e-8;
const S6: f64 = 1.589_623_015_765_465_680_60e-10;

const C1: f64 = 4.166_666_666_666_660_190_37e-2;
const C2: f64 = -1.388_888_888_887_410_957_49e-3;
const C3: f64 = 2.480_158_728_947_672_941_78e-5;
const C4: f64 = -2.755_731_435_139_066_330_35e-7;
const C5: f64 = 2.087_572_321_298_174_827_90e-9;
const C6: f64 = -1.135_964_755_778_819_482_65e-11;

/// A point or a direction. Scoria is 2D and f64 everywhere; there is no f32 in
/// this crate, because mixing the two invites x87 excess precision on i686 and
/// FMA contraction on aarch64, both of which break byte-identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V2 {
    /// Rightward in SVG user units.
    pub x: f64,
    /// DOWNWARD in SVG user units — positive y is down, as in the artwork.
    pub y: f64,
}

impl V2 {
    /// The origin.
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    /// A vector from its components.
    #[inline]
    #[must_use]
    pub const fn new(x: f64, y: f64) -> V2 {
        V2 { x, y }
    }

    /// Euclidean length. `sqrt` is IEEE-exact, so this is not `hypot` (which is
    /// libm and therefore a determinism hazard); the overflow protection hypot
    /// buys is irrelevant at our 1000-unit canvas scale.
    #[inline]
    #[must_use]
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Squared length — prefer it to [`V2::len`] in comparisons, it avoids the
    /// `sqrt` and is exact.
    #[inline]
    #[must_use]
    pub fn len2(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Unit vector in the same direction, or [`V2::ZERO`] for the zero vector.
    ///
    /// Zero-safe on purpose: returning a fabricated direction such as `(1,0)`
    /// would hide the degenerate input at the one place a caller could still
    /// notice it.
    #[inline]
    #[must_use]
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l == 0.0 {
            V2::ZERO
        } else {
            V2::new(self.x / l, self.y / l)
        }
    }

    /// Dot product.
    #[inline]
    #[must_use]
    pub fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }

    /// Left-hand perpendicular `(-y, x)`. With y pointing down this turns a
    /// direction a quarter turn clockwise on screen.
    #[inline]
    #[must_use]
    pub fn perp(self) -> V2 {
        V2::new(-self.y, self.x)
    }

    /// 2D cross product (the z of the 3D cross): positive when `o` is to the
    /// left of `self`.
    #[inline]
    #[must_use]
    pub fn cross(self, o: V2) -> f64 {
        self.x * o.y - self.y * o.x
    }

    /// Linear interpolation, `t = 0` giving `self`.
    #[inline]
    #[must_use]
    pub fn lerp(self, o: V2, t: f64) -> V2 {
        V2::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }

    /// Rotate by `a` radians (clockwise on screen, since y is down).
    #[inline]
    #[must_use]
    pub fn rot(self, a: f64) -> V2 {
        let (s, c) = sin_cos(a);
        V2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}

impl Add for V2 {
    type Output = V2;
    #[inline]
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    #[inline]
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for V2 {
    type Output = V2;
    #[inline]
    fn mul(self, k: f64) -> V2 {
        V2::new(self.x * k, self.y * k)
    }
}

impl Neg for V2 {
    type Output = V2;
    #[inline]
    fn neg(self) -> V2 {
        V2::new(-self.x, -self.y)
    }
}

// ---------------------------------------------------------------- trigonometry

#[inline]
fn sin_kernel(x: f64) -> f64 {
    let z = x * x;
    x + x * z * (S1 + z * (S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)))))
}

#[inline]
fn cos_kernel(x: f64) -> f64 {
    let z = x * x;
    1.0 - 0.5 * z + z * z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))))
}

/// Octant reduction: returns the octant count `n` and the residue
/// `r = x - n*pi/2` in roughly `[-pi/4, pi/4]`.
#[inline]
fn reduce(x: f64) -> (i64, f64) {
    let n = (x * TWO_OVER_PI).round();
    // `as i64` on f64 saturates in Rust rather than being UB, so a non-finite
    // argument cannot produce a garbage octant — callers guard it anyway.
    let k = n as i64;
    let r = (x - n * PIO2_HI) - n * PIO2_LO;
    (k, r)
}

/// Sine, accurate to about 1 ulp for `|x| <= `[`TRIG_EXACT_LIMIT`].
///
/// Returns NaN for a non-finite argument.
#[must_use]
pub fn sin(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let (k, r) = reduce(x);
    match k & 3 {
        0 => sin_kernel(r),
        1 => cos_kernel(r),
        2 => -sin_kernel(r),
        _ => -cos_kernel(r),
    }
}

/// Cosine, accurate to about 1 ulp for `|x| <= `[`TRIG_EXACT_LIMIT`].
///
/// Returns NaN for a non-finite argument.
#[must_use]
pub fn cos(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let (k, r) = reduce(x);
    match k & 3 {
        0 => cos_kernel(r),
        1 => -sin_kernel(r),
        2 => -cos_kernel(r),
        _ => sin_kernel(r),
    }
}

/// `(sin x, cos x)` from a single range reduction — half the work of calling
/// both, and the two results are guaranteed to come from the same residue.
#[must_use]
pub fn sin_cos(x: f64) -> (f64, f64) {
    if !x.is_finite() {
        return (f64::NAN, f64::NAN);
    }
    let (k, r) = reduce(x);
    let s = sin_kernel(r);
    let c = cos_kernel(r);
    match k & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

// fdlibm's arctangent, restricted to |x| <= 1. Needed because `total_turning`
// and `max_turn` have to report real radians, and because `Ring::radius_toward`
// wants a direction's exact theta. SPEC.md says "atan2 is NOT needed anywhere";
// that is an argument against *libm's* atan2 — this polynomial is as
// deterministic as the sine above.
const AT: [f64; 11] = [
    3.333_333_333_333_293_180_27e-1,
    -1.999_999_999_987_648_324_76e-1,
    1.428_571_427_250_346_637_11e-1,
    -1.111_111_040_546_235_578_80e-1,
    9.090_887_133_436_506_561_96e-2,
    -7.691_876_205_044_829_994_95e-2,
    6.661_073_137_387_531_206_69e-2,
    -5.833_570_133_790_573_486_45e-2,
    4.976_877_994_615_932_360_17e-2,
    -3.653_157_274_421_691_552_70e-2,
    1.628_582_011_536_578_236_23e-2,
];
// atan(0.5) and atan(1.0), each split hi/lo so the reconstruction keeps the
// last bits.
const ATAN_HI: [f64; 2] = [4.636_476_090_008_060_935_15e-1, std::f64::consts::FRAC_PI_4];
const ATAN_LO: [f64; 2] = [
    2.269_877_745_296_168_709_24e-17,
    3.061_616_997_868_383_017_93e-17,
];

/// The odd correction term: `atan(x) = x - x*atan_poly(x)` on `|x| <= 7/16`.
#[inline]
fn atan_poly(x: f64) -> f64 {
    let z = x * x;
    let w = z * z;
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    s1 + s2
}

/// Arctangent of `x` in `[0, 1]`, to about 1 ulp.
fn atan_unit(x: f64) -> f64 {
    // Below 2^-29, x*x underflows out of the polynomial entirely and atan(x) is
    // x to the last bit.
    if x < 1.862_645_149_230_957e-9 {
        return x;
    }
    if x < 0.437_5 {
        return x - x * atan_poly(x);
    }
    let (id, t) = if x < 0.687_5 {
        (0usize, (2.0 * x - 1.0) / (2.0 + x))
    } else {
        (1usize, (x - 1.0) / (x + 1.0))
    };
    ATAN_HI[id] - ((t * atan_poly(t) - ATAN_LO[id]) - t)
}

/// Two-argument arctangent in `(-pi, pi]`, hand-rolled so it is bit-identical
/// on every target. Non-finite inputs give NaN.
///
/// Where only an *ordering* is needed, prefer [`pangle`]: it is exact, costs one
/// divide, and never touches a polynomial.
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    if !x.is_finite() || !y.is_finite() {
        return f64::NAN;
    }
    let ax = x.abs();
    let ay = y.abs();
    // Fold into the first octant, where atan_unit is valid.
    let base = if ax == 0.0 && ay == 0.0 {
        0.0
    } else if ay <= ax {
        atan_unit(ay / ax)
    } else {
        HALF_PI - atan_unit(ax / ay)
    };
    // `is_sign_negative`, not `< 0.0`: atan2(+0, -0) is pi, not 0.
    let r = if x.is_sign_negative() {
        PI - base
    } else {
        base
    };
    if y.is_sign_negative() {
        -r
    } else {
        r
    }
}

/// Unsigned angle between two vectors, in `[0, pi]`. Zero for a zero input.
#[must_use]
pub fn angle_between(u: V2, v: V2) -> f64 {
    let c = u.dot(v);
    let s = u.cross(v).abs();
    if s == 0.0 && c == 0.0 {
        return 0.0;
    }
    if c >= 0.0 {
        if s <= c {
            atan_unit(s / c)
        } else {
            HALF_PI - atan_unit(c / s)
        }
    } else {
        let a = -c;
        if s >= a {
            HALF_PI + atan_unit(a / s)
        } else {
            PI - atan_unit(s / a)
        }
    }
}

// The largest f64 strictly below 4.0. `4.0 + t` with a subnormal negative t
// rounds back up to exactly 4.0, which would push pangle out of its documented
// half-open range; clamping here keeps [0, 4) true without costing a branch on
// any realistic input.
const PANGLE_MAX: f64 = 3.999_999_999_999_999_555_910_790_149_937_383_830_55;

/// Diamond pseudo-angle of a direction, in `[0, 4)`, strictly monotone in
/// `atan2(d.y, d.x)` over a full turn.
///
/// Exact (one divide, no polynomial) and therefore the right key for sorting
/// and binary-searching directions. It is NOT an angle: do not interpolate
/// geometry with it without meaning to.
#[must_use]
pub fn pangle(d: V2) -> f64 {
    // The 1e-300 is a denominator guard, far below any coordinate this crate
    // produces, so it cannot perturb a real direction.
    let t = d.y / (d.x.abs() + d.y.abs() + 1e-300);
    if d.x < 0.0 {
        2.0 - t
    } else if d.y < 0.0 {
        (4.0 + t).min(PANGLE_MAX)
    } else {
        t
    }
}

// -------------------------------------------------------------------- polygons

/// Signed area of the closed loop through `pts` (the last point joins the
/// first). Positive when the loop winds in the +x-to-+y sense, which with y
/// down means clockwise on screen.
#[must_use]
pub fn polygon_area(pts: &[V2]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut acc = 0.0;
    let mut prev = pts[n - 1];
    for &p in pts {
        acc += prev.x * p.y - p.x * prev.y;
        prev = p;
    }
    acc * 0.5
}

/// Area centroid of the closed loop through `pts`.
///
/// Not the vertex mean: the mean drifts toward wherever the outline happens to
/// be sampled densely, which on a resampled ring is every tight notch. The
/// facet bisection seeds its cut from this point, so the drift would bias every
/// split toward the crinkly side of the region.
///
/// Falls back to the vertex mean when the loop encloses no area.
#[must_use]
pub fn polygon_centroid(pts: &[V2]) -> V2 {
    let n = pts.len();
    if n == 0 {
        return V2::ZERO;
    }
    let mean = {
        let mut s = V2::ZERO;
        for &p in pts {
            s = s + p;
        }
        s * (1.0 / n as f64)
    };
    if n < 3 {
        return mean;
    }
    let a = polygon_area(pts);
    // Scale-free degeneracy test: compare the area against the bbox area it
    // would have to live in.
    let (mut lo, mut hi) = (pts[0], pts[0]);
    for &p in pts {
        lo = V2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = V2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let span = (hi.x - lo.x).max(hi.y - lo.y);
    if a.abs() <= 1e-12 * (span * span + 1.0) {
        return mean;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut prev = pts[n - 1];
    for &p in pts {
        let cr = prev.x * p.y - p.x * prev.y;
        cx += (prev.x + p.x) * cr;
        cy += (prev.y + p.y) * cr;
        prev = p;
    }
    let k = 1.0 / (6.0 * a);
    V2::new(cx * k, cy * k)
}

/// Crossing-number point-in-polygon test on the closed loop through `pts`.
///
/// Used for the facet regions, which are not star-shaped and so cannot take
/// [`crate::ring::Ring::contains`]'s O(1) polar shortcut. Points exactly on the
/// boundary are undefined, as they are for every crossing-number test.
#[must_use]
pub fn point_in_polygon(pts: &[V2], q: V2) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut prev = pts[n - 1];
    for &p in pts {
        if (p.y > q.y) != (prev.y > q.y) {
            let t = (q.y - p.y) / (prev.y - p.y);
            if q.x < p.x + t * (prev.x - p.x) {
                inside = !inside;
            }
        }
        prev = p;
    }
    inside
}

/// Intersection of segments `a0->a1` and `b0->b1`, as the pair of parameters
/// `(t, u)` with `t` along a and `u` along b. `None` when they are parallel or
/// when the crossing lies off either segment.
///
/// Returning the parameters rather than the point is what lets the facet split
/// store one crossing and hand the identical `f64` to both children — see the
/// "SHARE THE CROSSING POINTS" rule in SPEC.md.
#[must_use]
pub fn seg_intersect(a0: V2, a1: V2, b0: V2, b1: V2) -> Option<(f64, f64)> {
    let r = a1 - a0;
    let s = b1 - b0;
    let d = r.cross(s);
    if d == 0.0 {
        return None;
    }
    let q = b0 - a0;
    let t = q.cross(s) / d;
    let u = q.cross(r) / d;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((t, u))
    } else {
        None
    }
}

/// Length of an OPEN polyline.
#[must_use]
pub fn polyline_len(pts: &[V2]) -> f64 {
    let mut acc = 0.0;
    for w in pts.windows(2) {
        acc += (w[1] - w[0]).len();
    }
    acc
}

/// Length of a CLOSED polyline — [`polyline_len`] plus the closing chord.
#[must_use]
pub fn closed_len(pts: &[V2]) -> f64 {
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }
    polyline_len(pts) + (pts[0] - pts[n - 1]).len()
}

/// Resample a closed polyline to `m` points at uniform arclength, starting at
/// `pts[0]`.
///
/// This is not cosmetic. Uniform Catmull-Rom overshoots exactly where sample
/// spacing changes fastest, and on a polar blob that is every deep notch; the
/// overshoot becomes a visible loop in the fill. Resampling removes the failure
/// mode rather than patching it, and it is what makes the exact-circle property
/// of [`crate::bezier::catmull_rom_closed`] hold.
#[must_use]
pub fn resample_closed(pts: &[V2], m: usize) -> Vec<V2> {
    if m == 0 {
        return Vec::new();
    }
    let n = pts.len();
    if n == 0 {
        return Vec::new();
    }
    if n < 2 {
        return vec![pts[0]; m];
    }
    let total = closed_len(pts);
    if total == 0.0 {
        return vec![pts[0]; m];
    }
    let step = total / m as f64;
    let seg_len = |i: usize| (pts[(i + 1) % n] - pts[i]).len();

    let mut out = Vec::with_capacity(m);
    let mut seg = 0usize;
    let mut acc = 0.0; // arclength at the start of `seg`
    let mut cur = seg_len(0);
    for k in 0..m {
        let target = k as f64 * step;
        // The walk is monotone in k, so this advances at most n times overall.
        while seg + 1 < n && acc + cur < target {
            acc += cur;
            seg += 1;
            cur = seg_len(seg);
        }
        let t = if cur > 0.0 {
            ((target - acc) / cur).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(pts[seg].lerp(pts[(seg + 1) % n], t));
    }
    out
}

/// Sum of the absolute exterior angles along an OPEN polyline, in radians.
///
/// SPEC.md uses this as the cheap sufficient test that a traced facet cut does
/// not loop back on itself: a curve whose total absolute turning stays under
/// 220 degrees cannot close enough to cross itself.
#[must_use]
pub fn total_turning(pts: &[V2]) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut acc = 0.0;
    let mut prev = pts[1] - pts[0];
    for w in pts.windows(2).skip(1) {
        let d = w[1] - w[0];
        acc += angle_between(prev, d);
        prev = d;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic generator so the tests do not depend on `rng.rs`
    /// (which has its own tests, and must be free to change).
    struct T(u64);
    impl T {
        fn f(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
        }
        fn r(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.f()
        }
    }

    #[test]
    fn sin_cos_match_std_to_1e12() {
        let mut t = T(7);
        let mut worst = 0.0f64;
        for _ in 0..4000 {
            // Small, medium and large arguments, both signs.
            for x in [
                t.r(-PI, PI),
                t.r(-100.0, 100.0),
                t.r(-1.0e5, 1.0e5),
                t.r(-1.0e6, 1.0e6),
            ] {
                worst = worst.max((sin(x) - x.sin()).abs());
                worst = worst.max((cos(x) - x.cos()).abs());
            }
        }
        assert!(worst < 1e-12, "worst sin/cos deviation {worst:e}");
    }

    #[test]
    fn sin_cos_exact_at_quadrant_multiples() {
        for k in -40i32..=40 {
            let x = k as f64 * HALF_PI;
            assert!((sin(x) - x.sin()).abs() < 1e-15, "sin at k={k}");
            assert!((cos(x) - x.cos()).abs() < 1e-15, "cos at k={k}");
        }
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
    }

    #[test]
    fn sin_cos_pair_agrees_with_the_singles() {
        let mut t = T(11);
        for _ in 0..2000 {
            let x = t.r(-2000.0, 2000.0);
            let (s, c) = sin_cos(x);
            assert_eq!(s.to_bits(), sin(x).to_bits());
            assert_eq!(c.to_bits(), cos(x).to_bits());
            assert!((s * s + c * c - 1.0).abs() < 1e-15);
        }
    }

    #[test]
    fn trig_rejects_non_finite() {
        assert!(sin(f64::NAN).is_nan());
        assert!(cos(f64::INFINITY).is_nan());
        let (s, c) = sin_cos(f64::NEG_INFINITY);
        assert!(s.is_nan() && c.is_nan());
    }

    #[test]
    fn atan2_matches_std() {
        let mut t = T(13);
        let mut worst = 0.0f64;
        for _ in 0..5000 {
            let x = t.r(-50.0, 50.0);
            let y = t.r(-50.0, 50.0);
            worst = worst.max((atan2(y, x) - y.atan2(x)).abs());
        }
        assert!(worst < 1e-14, "worst atan2 deviation {worst:e}");
        assert_eq!(atan2(0.0, 1.0), 0.0);
        assert!((atan2(1.0, 0.0) - HALF_PI).abs() < 1e-16);
        assert!((atan2(0.0, -1.0) - PI).abs() < 1e-16);
    }

    #[test]
    fn angle_between_is_the_unsigned_angle() {
        assert!((angle_between(V2::new(1.0, 0.0), V2::new(0.0, 1.0)) - HALF_PI).abs() < 1e-15);
        assert!((angle_between(V2::new(1.0, 0.0), V2::new(-1.0, 0.0)) - PI).abs() < 1e-15);
        assert_eq!(angle_between(V2::new(1.0, 0.0), V2::new(2.0, 0.0)), 0.0);
        assert_eq!(angle_between(V2::ZERO, V2::new(1.0, 0.0)), 0.0);
        let mut t = T(17);
        for _ in 0..3000 {
            let a = t.r(-PI, PI);
            let b = t.r(-PI, PI);
            let (sa, ca) = sin_cos(a);
            let (sb, cb) = sin_cos(b);
            let got = angle_between(V2::new(ca, sa), V2::new(cb, sb));
            let mut want = (a - b).abs();
            while want > PI {
                want = TAU - want;
            }
            assert!((got - want).abs() < 1e-12, "{got} vs {want}");
        }
    }

    #[test]
    fn pangle_is_monotone_in_atan2() {
        // Order a few thousand directions by atan2 and check pangle agrees.
        let mut t = T(19);
        let mut v: Vec<(f64, f64)> = (0..4000)
            .map(|_| {
                let a = t.r(-PI, PI);
                let (s, c) = sin_cos(a);
                let d = V2::new(c * t.r(0.1, 9.0), s * t.r(0.1, 9.0));
                // atan2 in [0, TAU) to match pangle's branch cut.
                let mut th = atan2(d.y, d.x);
                if th < 0.0 {
                    th += TAU;
                }
                (th, pangle(d))
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        for w in v.windows(2) {
            assert!(
                w[0].1 <= w[1].1,
                "pangle not monotone: theta {} -> {}, pangle {} -> {}",
                w[0].0,
                w[1].0,
                w[0].1,
                w[1].1
            );
        }
    }

    #[test]
    fn pangle_stays_in_range_and_sweeps_the_whole_turn() {
        for i in 0..2048 {
            let a = TAU * i as f64 / 2048.0;
            let (s, c) = sin_cos(a);
            let p = pangle(V2::new(c, s));
            assert!((0.0..4.0).contains(&p), "pangle {p} out of [0,4)");
        }
        assert_eq!(pangle(V2::new(1.0, 0.0)), 0.0);
        assert_eq!(pangle(V2::new(0.0, 1.0)), 1.0);
        assert_eq!(pangle(V2::new(-1.0, 0.0)), 2.0);
        assert_eq!(pangle(V2::new(0.0, -1.0)), 3.0);
        // The subnormal case the PANGLE_MAX clamp exists for.
        assert!(pangle(V2::new(1.0, -1e-320)) < 4.0);
    }

    #[test]
    fn polygon_area_and_centroid() {
        let sq = [
            V2::new(0.0, 0.0),
            V2::new(2.0, 0.0),
            V2::new(2.0, 2.0),
            V2::new(0.0, 2.0),
        ];
        assert!((polygon_area(&sq) - 4.0).abs() < 1e-15);
        let c = polygon_centroid(&sq);
        assert!((c.x - 1.0).abs() < 1e-15 && (c.y - 1.0).abs() < 1e-15);
        // Reversing the loop flips the sign but not the centroid.
        let mut rev = sq;
        rev.reverse();
        assert!((polygon_area(&rev) + 4.0).abs() < 1e-15);
        let cr = polygon_centroid(&rev);
        assert!((cr.x - 1.0).abs() < 1e-15 && (cr.y - 1.0).abs() < 1e-15);
    }

    #[test]
    fn centroid_is_the_area_centroid_not_the_vertex_mean() {
        // Same square, but one edge sampled 20x more densely. The vertex mean
        // slides toward that edge; the area centroid must not move.
        let mut pts = vec![V2::new(0.0, 0.0)];
        for i in 1..=20 {
            pts.push(V2::new(2.0 * i as f64 / 20.0, 0.0));
        }
        pts.push(V2::new(2.0, 2.0));
        pts.push(V2::new(0.0, 2.0));
        let c = polygon_centroid(&pts);
        assert!(
            (c.x - 1.0).abs() < 1e-12 && (c.y - 1.0).abs() < 1e-12,
            "{c:?}"
        );
        let mean = pts.iter().fold(V2::ZERO, |a, &b| a + b) * (1.0 / pts.len() as f64);
        assert!(
            (mean.y - 1.0).abs() > 0.3,
            "the mean should be badly biased"
        );
    }

    #[test]
    fn centroid_of_a_degenerate_loop_falls_back_to_the_mean() {
        let line = [V2::new(0.0, 0.0), V2::new(1.0, 0.0), V2::new(2.0, 0.0)];
        let c = polygon_centroid(&line);
        assert!((c.x - 1.0).abs() < 1e-15 && c.y == 0.0);
    }

    #[test]
    fn point_in_polygon_handles_a_concave_shape() {
        // An L, concave at (1,1).
        let l = [
            V2::new(0.0, 0.0),
            V2::new(2.0, 0.0),
            V2::new(2.0, 1.0),
            V2::new(1.0, 1.0),
            V2::new(1.0, 2.0),
            V2::new(0.0, 2.0),
        ];
        assert!(point_in_polygon(&l, V2::new(0.5, 0.5)));
        assert!(point_in_polygon(&l, V2::new(1.5, 0.5)));
        assert!(point_in_polygon(&l, V2::new(0.5, 1.5)));
        assert!(!point_in_polygon(&l, V2::new(1.5, 1.5)));
        assert!(!point_in_polygon(&l, V2::new(-1.0, 1.0)));
        assert!(!point_in_polygon(&l, V2::new(3.0, 1.0)));
    }

    #[test]
    fn seg_intersect_parameters() {
        let r = seg_intersect(
            V2::new(0.0, 0.0),
            V2::new(2.0, 0.0),
            V2::new(1.0, -1.0),
            V2::new(1.0, 1.0),
        );
        let (t, u) = r.expect("the two segments visibly cross");
        assert!((t - 0.5).abs() < 1e-15 && (u - 0.5).abs() < 1e-15);
        // Parallel.
        assert!(seg_intersect(
            V2::new(0.0, 0.0),
            V2::new(1.0, 0.0),
            V2::new(0.0, 1.0),
            V2::new(1.0, 1.0)
        )
        .is_none());
        // Crossing is off the end of b.
        assert!(seg_intersect(
            V2::new(0.0, 0.0),
            V2::new(2.0, 0.0),
            V2::new(1.0, 1.0),
            V2::new(1.0, 2.0)
        )
        .is_none());
    }

    #[test]
    fn lengths_of_a_square() {
        let sq = [
            V2::new(0.0, 0.0),
            V2::new(3.0, 0.0),
            V2::new(3.0, 3.0),
            V2::new(0.0, 3.0),
        ];
        assert!((polyline_len(&sq) - 9.0).abs() < 1e-15);
        assert!((closed_len(&sq) - 12.0).abs() < 1e-15);
    }

    #[test]
    fn resample_closed_hits_exact_arclengths_on_a_rectangle() {
        // A rectangle's perimeter arclength is exactly computable, so this
        // pins the walk — including the wraparound of the closing segment,
        // which is the part that is easy to get wrong.
        let (w, h) = (10.0, 4.0);
        let src = [
            V2::new(0.0, 0.0),
            V2::new(w, 0.0),
            V2::new(w, h),
            V2::new(0.0, h),
        ];
        let perim = 2.0 * (w + h);
        let m = 56;
        let out = resample_closed(&src, m);
        assert_eq!(out.len(), m);
        let at = |s: f64| -> V2 {
            let s = s % perim;
            if s <= w {
                V2::new(s, 0.0)
            } else if s <= w + h {
                V2::new(w, s - w)
            } else if s <= 2.0 * w + h {
                V2::new(w - (s - w - h), h)
            } else {
                V2::new(0.0, h - (s - 2.0 * w - h))
            }
        };
        let step = perim / m as f64;
        for (k, &p) in out.iter().enumerate() {
            let want = at(k as f64 * step);
            assert!((p - want).len() < 1e-12, "k={k}: {p:?} vs {want:?}");
        }
    }

    #[test]
    fn resample_closed_evens_out_a_theta_sampled_ellipse() {
        // An ellipse sampled uniformly in theta is dense at the pointy ends
        // and sparse at the flanks — the exact input pattern that makes
        // uniform Catmull-Rom loop. After the resample the spacing must be
        // flat. The source is fine enough (2000 points) that chord length and
        // arclength agree to well under the tolerance.
        let src: Vec<V2> = (0..2000)
            .map(|i| {
                let (s, c) = sin_cos(TAU * i as f64 / 2000.0);
                V2::new(5.0 * c, 1.0 * s)
            })
            .collect();
        let n = src.len();
        let raw: Vec<f64> = (0..n).map(|i| (src[(i + 1) % n] - src[i]).len()).collect();
        let raw_spread = raw.iter().cloned().fold(0.0f64, f64::max)
            / raw.iter().cloned().fold(f64::MAX, f64::min);
        assert!(raw_spread > 4.0, "the source should be badly non-uniform");

        // Cumulative arclength of the source, so each output point can be
        // located exactly rather than approximated by its chord.
        let mut cum = vec![0.0f64; n + 1];
        for i in 0..n {
            cum[i + 1] = cum[i] + raw[i];
        }
        let locate = |q: V2| -> f64 {
            let mut best = (f64::MAX, 0.0);
            for i in 0..n {
                let a = src[i];
                let b = src[(i + 1) % n];
                let ab = b - a;
                let t = ((q - a).dot(ab) / ab.len2()).clamp(0.0, 1.0);
                let d = (a.lerp(b, t) - q).len();
                if d < best.0 {
                    best = (d, cum[i] + raw[i] * t);
                }
            }
            assert!(best.0 < 1e-9, "output point left the source polyline");
            best.1
        };

        let m = 128;
        let out = resample_closed(&src, m);
        let want = cum[n] / m as f64;
        for (k, &p) in out.iter().enumerate() {
            let s = locate(p);
            assert!(
                (s - k as f64 * want).abs() < 1e-9 * cum[n],
                "point {k} sits at arclength {s}, wanted {}",
                k as f64 * want
            );
        }
    }

    #[test]
    fn resample_closed_on_a_regular_polygon_lands_on_vertices() {
        // 720-gon resampled to 240 must hit every third vertex exactly.
        let n = 720;
        let src: Vec<V2> = (0..n)
            .map(|i| {
                let (s, c) = sin_cos(TAU * i as f64 / n as f64);
                V2::new(100.0 * c, 100.0 * s)
            })
            .collect();
        let out = resample_closed(&src, 240);
        for (k, &p) in out.iter().enumerate() {
            let want = src[k * 3];
            assert!((p - want).len() < 1e-9, "point {k}: {p:?} vs {want:?}");
        }
        // And the spacing between outputs is then equal to 1e-9.
        let d0 = (out[1] - out[0]).len();
        for i in 1..out.len() {
            let d = (out[(i + 1) % out.len()] - out[i]).len();
            assert!((d - d0).abs() < 1e-9, "{d} vs {d0}");
        }
    }

    #[test]
    fn resample_closed_degenerate_inputs() {
        assert!(resample_closed(&[], 8).is_empty());
        assert!(resample_closed(&[V2::ZERO, V2::new(1.0, 0.0)], 0).is_empty());
        let same = vec![V2::new(2.0, 3.0); 5];
        assert_eq!(resample_closed(&same, 4), vec![V2::new(2.0, 3.0); 4]);
    }

    #[test]
    fn total_turning_of_a_staircase() {
        // Three right-angle turns.
        let pts = [
            V2::new(0.0, 0.0),
            V2::new(1.0, 0.0),
            V2::new(1.0, 1.0),
            V2::new(2.0, 1.0),
            V2::new(2.0, 2.0),
        ];
        assert!((total_turning(&pts) - 3.0 * HALF_PI).abs() < 1e-12);
        // A straight line never turns.
        let line = [V2::new(0.0, 0.0), V2::new(1.0, 0.0), V2::new(5.0, 0.0)];
        assert_eq!(total_turning(&line), 0.0);
        assert_eq!(total_turning(&[V2::ZERO]), 0.0);
    }

    #[test]
    fn total_turning_of_a_closed_circle_polyline_is_a_turn() {
        // Open polyline that happens to go all the way round: 64 chords,
        // 63 interior turns, each 2pi/64.
        let n = 65;
        let pts: Vec<V2> = (0..n)
            .map(|i| {
                let (s, c) = sin_cos(TAU * i as f64 / 64.0);
                V2::new(c, s)
            })
            .collect();
        let want = TAU * 63.0 / 64.0;
        assert!((total_turning(&pts) - want).abs() < 1e-12);
    }

    #[test]
    fn vector_algebra() {
        let a = V2::new(3.0, 4.0);
        assert_eq!(a.len(), 5.0);
        assert_eq!(a.len2(), 25.0);
        assert_eq!(a.norm().len(), 1.0);
        assert_eq!(V2::ZERO.norm(), V2::ZERO);
        assert_eq!(a.dot(V2::new(1.0, 0.0)), 3.0);
        assert_eq!(a.perp(), V2::new(-4.0, 3.0));
        assert_eq!(a.cross(V2::new(-4.0, 3.0)), 25.0);
        assert_eq!(a.lerp(V2::new(5.0, 4.0), 0.5), V2::new(4.0, 4.0));
        assert_eq!(-a, V2::new(-3.0, -4.0));
        assert_eq!(a + a - a, a);
        assert_eq!(a * 2.0, V2::new(6.0, 8.0));
        let r = V2::new(1.0, 0.0).rot(HALF_PI);
        assert!(r.x.abs() < 1e-15 && (r.y - 1.0).abs() < 1e-15);
    }
}
