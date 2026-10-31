//! PCG32 seeded through SplitMix64, with an unbiased bounded draw.
//!
//! Hobbs's Fidenza uses a bare LCG; we need better, because this generator
//! draws a lot of low-entropy decisions in a row (an octave offset, then a
//! vesicle radius, then a grid cell) and an LCG's correlated low bits show up
//! as visible structure — vesicle size drifting with position, for instance.
//! PCG32's output permutation fixes that for ten extra lines.
//!
//! The state never touches a float, so there is no rounding-mode or x87
//! exposure: every target produces the same stream from the same seed.

/// SplitMix64, used to spread a seed before it reaches PCG32 and to advance a
/// seed between whole-logo re-rolls.
///
/// Without it, seeds 0, 1 and 2 would give visibly related images — which
/// matters a lot for a batch generator, where people use sequential seeds.
#[inline]
pub fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// PCG32 (XSH-RR), 64 bits of state and a per-stream odd increment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// A generator for `seed`, on the independent sequence `stream`.
    ///
    /// Each subsystem takes its own stream constant from [`stream`] so that
    /// changing `vesicle_count` cannot reshuffle the silhouette. That
    /// separation is the thing that makes the parameter surface explorable
    /// rather than a slot machine.
    #[must_use]
    pub fn new(seed: u64, stream: u64) -> Pcg32 {
        let mut s = seed;
        let st = splitmix64(&mut s);
        // The canonical PCG seeding dance: step, inject, step.
        let mut r = Pcg32 {
            state: 0,
            inc: (stream << 1) | 1,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(st);
        r.next_u32();
        r
    }

    /// The next 32 bits.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// The next 64 bits, high word drawn first.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let hi = u64::from(self.next_u32());
        let lo = u64::from(self.next_u32());
        (hi << 32) | lo
    }

    /// A float in `[0, 1)`, exact: 53 significand bits scaled by `2^-53`.
    ///
    /// The scale is a power of two, so the multiply is exact and the result is
    /// identical on every target. Do not be tempted by `x / u64::MAX as f64`,
    /// which rounds and can return exactly 1.0.
    #[inline]
    #[allow(clippy::cast_precision_loss)] // the shift leaves exactly 53 bits
    pub fn f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// A float in `[lo, hi)`.
    #[inline]
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// An integer in `[0, n)`, UNBIASED (Lemire's multiply-shift with
    /// rejection). Returns 0 when `n` is 0.
    ///
    /// Never `% n`: that is still deterministic, but it skews the draw, and in
    /// the Fisher-Yates shuffle of the Perlin permutation table the skew makes
    /// the noise measurably directional — a bias you then see as a grain
    /// running one way across every silhouette.
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let mut m = u64::from(self.next_u32()) * u64::from(n);
        let mut l = m as u32;
        if l < n {
            // Reject the first `2^32 mod n` products, which are the ones that
            // would be over-represented.
            let t = n.wrapping_neg() % n;
            while l < t {
                m = u64::from(self.next_u32()) * u64::from(n);
                l = m as u32;
            }
        }
        (m >> 32) as u32
    }
}

/// Stream constants: one per subsystem, distinct and odd.
///
/// Five separate streams off one SplitMix64 split. If vesicle placement drew
/// from the same stream as the silhouette, changing `vesicle_count` would
/// reshuffle the rock — see the "ONE SHARED PRNG STREAM" pitfall in SPEC.md.
pub mod stream {
    /// The silhouette ring: its noise permutation, octave offsets and warp.
    pub const BLOB: u64 = 0x9E37_79B9_7F4A_7C15;
    /// Vesicle sizes, placement attempts and per-hole shape noise.
    pub const VESICLE: u64 = 0xC2B2_AE3D_27D4_EB4F;
    /// Facet bisection: seed jitter and cut tracing.
    pub const FACET: u64 = 0x1656_67B1_9E37_79F9;
    /// Tone assignment noise.
    pub const TONE: u64 = 0x27D4_EB2F_1656_67C5;
    /// Everything that does not deserve a stream of its own.
    pub const MISC: u64 = 0x85EB_CA77_C2B2_AE63;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_constants_are_distinct_and_odd() {
        let all = [
            stream::BLOB,
            stream::VESICLE,
            stream::FACET,
            stream::TONE,
            stream::MISC,
        ];
        for (i, a) in all.iter().enumerate() {
            assert_eq!(a & 1, 1, "stream {i} must be odd");
            for b in &all[i + 1..] {
                assert_ne!(a, b);
                // `new` uses (stream << 1) | 1, which drops the top bit, so
                // two constants that differ only there would collide.
                assert_ne!(a << 1, b << 1);
            }
        }
    }

    #[test]
    fn same_seed_gives_the_same_bytes() {
        let mut a = Pcg32::new(12345, stream::BLOB);
        let mut b = Pcg32::new(12345, stream::BLOB);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_eq!(a, b);
    }

    #[test]
    fn golden_outputs_are_frozen() {
        // A regression lock, not a reference vector: if a refactor changes any
        // of these, every seed in the wild renders a different logo.
        let mut r = Pcg32::new(0, stream::BLOB);
        let got: Vec<u32> = (0..6).map(|_| r.next_u32()).collect();
        assert_eq!(
            got,
            vec![
                429_509_017,
                3_310_995_014,
                2_365_295_503,
                214_865_384,
                502_597_750,
                1_368_386_365
            ]
        );
        let mut s = Pcg32::new(0, stream::BLOB);
        let f: Vec<f64> = (0..3).map(|_| s.f64()).collect();
        assert_eq!(
            f.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            vec![
                0x3FB9_99C9_99C5_59C0,
                0x3FE1_9F71_71E1_99D2,
                0x3FBD_F508_7651_8FE8
            ]
        );
    }

    #[test]
    fn different_streams_diverge() {
        let mut a = Pcg32::new(7, stream::BLOB);
        let mut b = Pcg32::new(7, stream::VESICLE);
        let mut same = 0;
        for _ in 0..256 {
            if a.next_u32() == b.next_u32() {
                same += 1;
            }
        }
        assert_eq!(same, 0, "streams with the same seed must not overlap");
    }

    #[test]
    fn sequential_seeds_are_unrelated() {
        // SplitMix64 on the way in is what buys this.
        let mut prev: Option<u32> = None;
        for seed in 0..64u64 {
            let v = Pcg32::new(seed, stream::BLOB).next_u32();
            if let Some(p) = prev {
                // Not a statistical test, just a smoke alarm for a seeding bug
                // that leaves consecutive seeds a constant apart.
                assert_ne!(v, p);
            }
            prev = Some(v);
        }
    }

    #[test]
    fn f64_stays_in_the_unit_interval() {
        let mut r = Pcg32::new(99, stream::MISC);
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        let mut sum = 0.0;
        const N: usize = 200_000;
        for _ in 0..N {
            let v = r.f64();
            assert!((0.0..1.0).contains(&v));
            lo = lo.min(v);
            hi = hi.max(v);
            sum += v;
        }
        assert!(lo < 1e-4 && hi > 1.0 - 1e-4, "{lo} .. {hi}");
        let mean = sum / N as f64;
        assert!((mean - 0.5).abs() < 0.01, "mean {mean}");
    }

    #[test]
    fn range_respects_its_bounds() {
        let mut r = Pcg32::new(5, stream::MISC);
        for _ in 0..10_000 {
            let v = r.range(-4096.0, 4096.0);
            assert!((-4096.0..4096.0).contains(&v));
        }
        // A degenerate range is a point, not a panic.
        assert_eq!(r.range(3.0, 3.0), 3.0);
    }

    #[test]
    fn below_is_in_range_and_unbiased() {
        let mut r = Pcg32::new(2024, stream::MISC);
        assert_eq!(r.below(0), 0);
        assert_eq!(r.below(1), 0);
        // 7 is the worst case for `% 2^32`: 2^32 mod 7 = 4, so a biased
        // implementation over-represents 0..4 by about 1 part in 6e8 — too
        // small to catch here. Instead check the far cruder property that no
        // bucket is anywhere near empty or double, which a broken `below`
        // (wrong shift, missing rejection loop) would fail loudly.
        const N: u32 = 7;
        const DRAWS: usize = 140_000;
        let mut hist = [0usize; N as usize];
        for _ in 0..DRAWS {
            let v = r.below(N);
            assert!(v < N);
            hist[v as usize] += 1;
        }
        let want = DRAWS as f64 / f64::from(N);
        for (i, &h) in hist.iter().enumerate() {
            let dev = (h as f64 - want).abs() / want;
            assert!(dev < 0.05, "bucket {i} off by {:.1}%", dev * 100.0);
        }
    }

    #[test]
    fn below_matches_a_brute_force_reference() {
        // The rejection loop must consume exactly the same draws as the
        // straightforward form, or a refactor silently reshuffles every table.
        fn reference(r: &mut Pcg32, n: u32) -> u32 {
            let mut m = u64::from(r.next_u32()) * u64::from(n);
            let mut l = m as u32;
            let t = n.wrapping_neg() % n;
            while l < t {
                m = u64::from(r.next_u32()) * u64::from(n);
                l = m as u32;
            }
            (m >> 32) as u32
        }
        let mut a = Pcg32::new(31337, stream::FACET);
        let mut b = Pcg32::new(31337, stream::FACET);
        for n in 2..200u32 {
            assert_eq!(a.below(n), reference(&mut b, n), "n = {n}");
        }
        assert_eq!(a, b, "the two forms must consume the same number of draws");
    }

    #[test]
    fn splitmix_advances_and_spreads() {
        let mut x = 0u64;
        let a = splitmix64(&mut x);
        let b = splitmix64(&mut x);
        assert_ne!(a, b);
        assert_ne!(x, 0);
        // Zero must not be a fixed point.
        let mut z = 0u64;
        assert_ne!(splitmix64(&mut z), 0);
    }
}
