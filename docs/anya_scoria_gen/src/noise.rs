//! Classic improved Perlin noise and the fBm sum built on it.
//!
//! Perlin, not value noise and not simplex. Value noise is ten lines shorter
//! but its gradient discontinuities at cell corners read as axis-aligned
//! creases, which on a lumpy silhouette at logo scale is the one artifact we
//! cannot afford. Simplex buys nothing at 2D and costs the skew/unskew
//! machinery. Perlin 2D needs no trig, no `sqrt` and no float table beyond
//! eight hard-coded gradients, so it is trivially bit-exact.

use crate::math::{V2, V3};
use crate::rng::Pcg32;

/// `1/sqrt(2)`, a compile-time constant so the diagonal gradients never go
/// near a runtime `sqrt` — equal magnitudes across all eight directions is
/// what keeps the noise free of a directional bias.
const S: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The eight unit gradients: four axial, four diagonal.
const GRAD8: [(f64, f64); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (S, S),
    (S, -S),
    (-S, S),
    (-S, -S),
];

/// Raw 2D Perlin peaks at `±1/sqrt(2)`; this scales it to roughly `±1`.
const PERLIN_SCALE: f64 = std::f64::consts::SQRT_2;

/// Perlin's own twelve 3D gradients: the midpoints of a cube's edges.
///
/// All twelve have length `sqrt(2)` exactly, in integer components, so the
/// dot product is three multiplies and no `sqrt` anywhere — and because the
/// set is closed under the cube's symmetry group there is no directional
/// bias to show up as a grain on the solid. The classic reference adds four
/// repeats to pad the table to sixteen so the index can be a mask; indexing
/// twelve through [`Pcg32::below`]-shuffled bytes needs no padding and no
/// `% 12` in the hot path, because the permutation is folded at build time.
const GRAD12: [(f64, f64, f64); 12] = [
    (1.0, 1.0, 0.0),
    (-1.0, 1.0, 0.0),
    (1.0, -1.0, 0.0),
    (-1.0, -1.0, 0.0),
    (1.0, 0.0, 1.0),
    (-1.0, 0.0, 1.0),
    (1.0, 0.0, -1.0),
    (-1.0, 0.0, -1.0),
    (0.0, 1.0, 1.0),
    (0.0, -1.0, 1.0),
    (0.0, 1.0, -1.0),
    (0.0, -1.0, -1.0),
];

/// Raw 3D Perlin on the twelve `sqrt(2)`-length gradients peaks near
/// `±0.866`; this scales it to roughly `±1` so `Fbm` normalisation and
/// [`crate::body::unit_fbm`] mean the same thing in 2D and in 3D.
const PERLIN3_SCALE: f64 = 1.154_700_538_379_251_5;

/// Quintic fade, `6t^5 - 15t^4 + 10t^3`.
///
/// The cubic `3t^2 - 2t^3` is C1 but not C2, and the second-derivative seam
/// shows as a faint lattice on a smooth silhouette. One extra multiply buys
/// C2 continuity.
#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn grad(h: u8, x: f64, y: f64) -> f64 {
    let g = GRAD8[(h & 7) as usize];
    g.0 * x + g.1 * y
}

#[inline]
fn grad3(h: u8, x: f64, y: f64, z: f64) -> f64 {
    let g = GRAD12[(h % 12) as usize];
    g.0 * x + g.1 * y + g.2 * z
}

/// A seeded Perlin lattice: a 256-entry permutation, duplicated to 512 so the
/// `+1` neighbour lookups need no wrap.
#[derive(Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

impl std::fmt::Debug for Perlin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Perlin").finish_non_exhaustive()
    }
}

impl Perlin {
    /// Shuffle a fresh permutation out of `rng`.
    ///
    /// Fisher-Yates driven by [`Pcg32::below`], which is the unbiased form. A
    /// `% n` shuffle is still deterministic but skews the table, and the skew
    /// is visible as a grain running one way across the noise.
    #[must_use]
    pub fn new(rng: &mut Pcg32) -> Perlin {
        let mut p = [0u8; 256];
        for (i, slot) in p.iter_mut().enumerate() {
            // The cast is exact: i < 256.
            *slot = i as u8;
        }
        for i in (1..256usize).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        perm[..256].copy_from_slice(&p);
        perm[256..].copy_from_slice(&p);
        Perlin { perm }
    }

    /// Sample the lattice. Output is roughly `[-1, 1]` and exactly 0 at every
    /// integer lattice point.
    #[must_use]
    pub fn at(&self, x: f64, y: f64) -> f64 {
        let xi = x.floor();
        let yi = y.floor();
        let xf = x - xi;
        let yf = y - yi;
        // `as i64` saturates on a non-finite input rather than being UB, and
        // the & 255 then keeps the index in range whatever happened.
        let xg = (xi as i64 & 255) as usize;
        let yg = (yi as i64 & 255) as usize;
        let u = fade(xf);
        let v = fade(yf);

        let pa = self.perm[xg] as usize;
        let pb = self.perm[xg + 1] as usize;
        let aa = self.perm[pa + yg];
        let ab = self.perm[pa + yg + 1];
        let ba = self.perm[pb + yg];
        let bb = self.perm[pb + yg + 1];

        let x1 = lerp(grad(aa, xf, yf), grad(ba, xf - 1.0, yf), u);
        let x2 = lerp(grad(ab, xf, yf - 1.0), grad(bb, xf - 1.0, yf - 1.0), u);
        (lerp(x1, x2, v) * PERLIN_SCALE).clamp(-1.0, 1.0)
    }

    /// Sample the same lattice in three dimensions.
    ///
    /// The 512-entry table serves 3D unchanged: `perm[x] + y` reaches 510 at
    /// worst, which is why the duplication to 512 exists in the first place.
    /// Sharing one table with [`Perlin::at`] is deliberate — it is one
    /// permutation per seed rather than two, and 2D and 3D samples of the
    /// same lattice are not visibly correlated because nothing in the crate
    /// ever samples both over the same coordinates.
    #[must_use]
    pub fn at3(&self, x: f64, y: f64, z: f64) -> f64 {
        let xi = x.floor();
        let yi = y.floor();
        let zi = z.floor();
        let xf = x - xi;
        let yf = y - yi;
        let zf = z - zi;
        let xg = (xi as i64 & 255) as usize;
        let yg = (yi as i64 & 255) as usize;
        let zg = (zi as i64 & 255) as usize;
        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        let a = self.perm[xg] as usize + yg;
        let b = self.perm[xg + 1] as usize + yg;
        let aa = self.perm[a] as usize + zg;
        let ab = self.perm[a + 1] as usize + zg;
        let ba = self.perm[b] as usize + zg;
        let bb = self.perm[b + 1] as usize + zg;

        // The eight cube corners, folded x then y then z.
        let x1 = lerp(
            grad3(self.perm[aa], xf, yf, zf),
            grad3(self.perm[ba], xf - 1.0, yf, zf),
            u,
        );
        let x2 = lerp(
            grad3(self.perm[ab], xf, yf - 1.0, zf),
            grad3(self.perm[bb], xf - 1.0, yf - 1.0, zf),
            u,
        );
        let y1 = lerp(x1, x2, v);
        let x3 = lerp(
            grad3(self.perm[aa + 1], xf, yf, zf - 1.0),
            grad3(self.perm[ba + 1], xf - 1.0, yf, zf - 1.0),
            u,
        );
        let x4 = lerp(
            grad3(self.perm[ab + 1], xf, yf - 1.0, zf - 1.0),
            grad3(self.perm[bb + 1], xf - 1.0, yf - 1.0, zf - 1.0),
            u,
        );
        let y2 = lerp(x3, x4, v);
        (lerp(y1, y2, w) * PERLIN3_SCALE).clamp(-1.0, 1.0)
    }
}

/// fBm sampled on a SPHERE in 3D noise space.
///
/// The 2D silhouette gets its seamlessness by walking a circle in the noise
/// plane; this is the same trick one dimension up, and in 3D it stops being a
/// trick at all. A displacement field defined as a 3D function restricted to
/// the unit sphere is single-valued at every direction by construction, so
/// there is no seam anywhere on the solid — not at `theta = 0`, not at either
/// pole, which is where a `(theta, u)`-domain noise would pinch.
///
/// `lobes` is in LOBES PER TURN around the equator, so it carries the same
/// meaning as `lump_freq` did in the plane: the sample walks a sphere of
/// radius `lobes / PI` in noise space, whose equator has circumference
/// `2 * PI * lobes / PI` and therefore meets about `lobes` Perlin periods on
/// the way round.
#[derive(Clone, Debug)]
pub struct SphereFbm {
    offsets: Vec<V3>,
    lacunarity: f64,
    gain: f64,
}

impl SphereFbm {
    /// Draw one noise-space offset per octave.
    #[must_use]
    pub fn new(rng: &mut Pcg32, octaves: u32, lacunarity: f64, gain: f64) -> SphereFbm {
        let mut offsets = Vec::with_capacity(octaves.max(1) as usize);
        for _ in 0..octaves.max(1) {
            offsets.push(V3::new(
                rng.range(-4096.0, 4096.0),
                rng.range(-4096.0, 4096.0),
                rng.range(-4096.0, 4096.0),
            ));
        }
        SphereFbm {
            offsets,
            lacunarity,
            gain,
        }
    }

    /// Amplitude-normalised sum over octaves in unit direction `d`.
    ///
    /// `warp` displaces the noise LOOKUP, never the geometry: the result stays
    /// a scalar function of direction, which is the invariant the solid's
    /// star-shapedness — and through it the silhouette's — depends on.
    #[must_use]
    pub fn at(&self, p: &Perlin, d: V3, lobes: f64, warp: V3) -> f64 {
        let mut f = lobes / crate::math::PI;
        let mut a = 1.0;
        let mut acc = 0.0;
        let mut sum = 0.0;
        for off in &self.offsets {
            acc += a * p.at3(
                d.x * f + warp.x + off.x,
                d.y * f + warp.y + off.y,
                d.z * f + warp.z + off.z,
            );
            sum += a;
            a *= self.gain;
            f *= self.lacunarity;
        }
        if sum > 0.0 {
            acc / sum
        } else {
            0.0
        }
    }
}

/// Fractal Brownian motion over a [`Perlin`] lattice.
///
/// Amplitude-normalised by the sum of the octave amplitudes, so a caller's
/// `lump_amp` means exactly what it says rather than "about that, depending on
/// how many octaves you asked for".
#[derive(Clone, Debug)]
pub struct Fbm {
    /// How many octaves are summed.
    pub octaves: u32,
    /// Per-octave frequency multiplier. 2.03, not 2.0 — see [`Fbm::new`].
    pub lacunarity: f64,
    /// Per-octave amplitude multiplier.
    pub gain: f64,
    /// Per-octave translation in noise space, drawn once at construction.
    offsets: Vec<V2>,
}

impl Fbm {
    /// Draw the per-octave offsets and fix the octave count.
    ///
    /// `lacunarity` should not be exactly 2.0: at 2.0 successive Perlin
    /// octaves land on the same lattice nodes and the sum grows faint
    /// axis-aligned ridges. 2.03 decorrelates them for free.
    ///
    /// Offsets are drawn in `[-4096, 4096]` — far enough apart that two
    /// octaves never sample correlated regions of the same lattice.
    #[must_use]
    pub fn new(rng: &mut Pcg32, octaves: u32, lacunarity: f64, gain: f64) -> Fbm {
        let n = octaves as usize;
        let mut offsets = Vec::with_capacity(n);
        for _ in 0..n {
            let x = rng.range(-4096.0, 4096.0);
            let y = rng.range(-4096.0, 4096.0);
            offsets.push(V2::new(x, y));
        }
        Fbm {
            octaves,
            lacunarity,
            gain,
            offsets,
        }
    }

    /// Sum the octaves at `(x, y)`, result in roughly `[-1, 1]`.
    ///
    /// Octave `o` samples at `(x, y) * lacunarity^o + offset_o`. Sampling a
    /// CIRCLE of radius `f` in `(x, y)` therefore stays exactly periodic in
    /// the circle's parameter at every octave — which is how the silhouette
    /// gets a seamless closed outline without any cross-fade at theta = 0.
    #[must_use]
    pub fn at(&self, p: &Perlin, x: f64, y: f64) -> f64 {
        let mut f = 1.0;
        let mut a = 1.0;
        let mut sum = 0.0;
        let mut acc = 0.0;
        for off in self.offsets.iter().take(self.octaves as usize) {
            acc += a * p.at(x * f + off.x, y * f + off.y);
            sum += a;
            a *= self.gain;
            f *= self.lacunarity;
        }
        if sum > 0.0 {
            acc / sum
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{sin_cos, TAU, V3};
    use crate::rng::stream;

    fn perlin(seed: u64) -> Perlin {
        Perlin::new(&mut Pcg32::new(seed, stream::BLOB))
    }

    #[test]
    fn the_permutation_is_a_permutation() {
        let p = perlin(1);
        let mut seen = [false; 256];
        for &v in &p.perm[..256] {
            assert!(!seen[v as usize], "value {v} appears twice");
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        // The second half must mirror the first exactly.
        assert_eq!(p.perm[..256], p.perm[256..]);
        // And it must actually be shuffled, not the identity.
        assert!((0..256).filter(|&i| p.perm[i] as usize == i).count() < 16);
    }

    #[test]
    fn the_permutation_is_deterministic_and_seed_dependent() {
        assert_eq!(perlin(5).perm, perlin(5).perm);
        assert_ne!(perlin(5).perm, perlin(6).perm);
    }

    #[test]
    fn perlin_is_zero_on_the_lattice() {
        let p = perlin(3);
        for ix in -5..5 {
            for iy in -5..5 {
                let v = p.at(f64::from(ix), f64::from(iy));
                assert!(v.abs() < 1e-15, "({ix},{iy}) -> {v}");
            }
        }
    }

    #[test]
    fn perlin_stays_in_range_and_uses_it() {
        let p = perlin(9);
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for i in 0..400 {
            for j in 0..400 {
                let v = p.at(f64::from(i) * 0.137, f64::from(j) * 0.0911);
                assert!((-1.0..=1.0).contains(&v));
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        // Perlin rarely touches the clamp, but it must use most of the band or
        // the amplitude normalisation downstream is a lie.
        assert!(lo < -0.7 && hi > 0.7, "{lo} .. {hi}");
    }

    #[test]
    fn perlin_is_continuous() {
        // A derivative bound, really: the lattice-crossing seam a buggy fade
        // or a mis-indexed permutation produces shows up here immediately.
        let p = perlin(4);
        let h = 1e-6;
        let mut worst = 0.0f64;
        for i in 0..2000 {
            // Walk straight across lattice boundaries.
            let x = f64::from(i) * 0.01 - 10.0;
            let d = (p.at(x + h, 0.3) - p.at(x, 0.3)).abs();
            worst = worst.max(d);
        }
        assert!(worst < 1e-5, "jump of {worst} across a cell boundary");
    }

    #[test]
    fn perlin_has_no_axis_bias() {
        // Eight equal-magnitude gradients: the mean of the field along x and
        // along y must both sit on zero.
        let p = perlin(21);
        let mut sum = 0.0;
        let mut n = 0.0;
        for i in 0..600 {
            for j in 0..600 {
                sum += p.at(f64::from(i) * 0.213 - 60.0, f64::from(j) * 0.173 - 50.0);
                n += 1.0;
            }
        }
        let mean = sum / n;
        assert!(mean.abs() < 0.01, "mean {mean} — the table is skewed");
    }

    #[test]
    fn fbm_is_amplitude_normalised() {
        let mut r = Pcg32::new(77, stream::BLOB);
        let p = Perlin::new(&mut r);
        let f = Fbm::new(&mut r, 4, 2.03, 0.45);
        let mut hi = 0.0f64;
        for i in 0..500 {
            for j in 0..500 {
                let v = f.at(&p, f64::from(i) * 0.07, f64::from(j) * 0.05);
                assert!(v.abs() <= 1.0 + 1e-12, "fbm out of band: {v}");
                hi = hi.max(v.abs());
            }
        }
        assert!(
            hi > 0.5,
            "fbm never gets anywhere near its stated amplitude"
        );
    }

    #[test]
    fn fbm_offsets_decorrelate_the_octaves() {
        let mut r = Pcg32::new(31, stream::BLOB);
        let f = Fbm::new(&mut r, 4, 2.03, 0.45);
        assert_eq!(f.offsets.len(), 4);
        for o in &f.offsets {
            assert!(o.x.abs() <= 4096.0 && o.y.abs() <= 4096.0);
        }
        for i in 0..f.offsets.len() {
            for j in i + 1..f.offsets.len() {
                assert!((f.offsets[i] - f.offsets[j]).len() > 1.0);
            }
        }
    }

    #[test]
    fn fbm_on_a_circle_is_exactly_periodic() {
        // The whole trick behind a seamless closed silhouette: sample a circle
        // in 2D noise space and theta-periodicity is free at every octave.
        let mut r = Pcg32::new(123, stream::BLOB);
        let p = Perlin::new(&mut r);
        let f = Fbm::new(&mut r, 4, 2.03, 0.45);
        let freq = 2.6;
        let sample = |th: f64| {
            let (s, c) = sin_cos(th);
            f.at(&p, c * freq, s * freq)
        };
        // theta and theta + TAU land on the same circle point to the last few
        // bits, so the noise values must agree to the same order.
        for k in 0..64 {
            let th = TAU * f64::from(k) / 64.0;
            let d = (sample(th) - sample(th + TAU)).abs();
            assert!(d < 1e-12, "seam of {d} at k={k}");
        }
    }

    #[test]
    fn fbm_with_zero_octaves_is_flat() {
        let mut r = Pcg32::new(1, stream::MISC);
        let p = Perlin::new(&mut r);
        let f = Fbm::new(&mut r, 0, 2.03, 0.45);
        assert_eq!(f.at(&p, 0.3, 0.7), 0.0);
    }

    #[test]
    fn higher_gain_means_more_fine_detail() {
        let mut r = Pcg32::new(8, stream::BLOB);
        let p = Perlin::new(&mut r);
        let smooth = Fbm::new(&mut Pcg32::new(8, stream::MISC), 4, 2.03, 0.20);
        let crusty = Fbm::new(&mut Pcg32::new(8, stream::MISC), 4, 2.03, 0.60);
        let rough = |f: &Fbm| {
            let mut acc = 0.0;
            for i in 0..1000 {
                let x = f64::from(i) * 0.01;
                acc += (f.at(&p, x + 0.01, 0.0) - f.at(&p, x, 0.0)).abs();
            }
            acc
        };
        assert!(rough(&crusty) > rough(&smooth) * 1.5);
    }

    /// 3D Perlin has the same contract as the 2D lattice: zero at every
    /// lattice point, inside `[-1, 1]` everywhere, and a pure function of
    /// its inputs.
    #[test]
    fn perlin3_is_zero_on_the_lattice_and_bounded_off_it() {
        let p = perlin(5);
        for x in -4..4 {
            for y in -4..4 {
                for z in -4..4 {
                    let v = p.at3(f64::from(x), f64::from(y), f64::from(z));
                    assert!(v.abs() < 1e-12, "lattice point ({x},{y},{z}) gave {v}");
                }
            }
        }
        let mut hi: f64 = 0.0;
        for i in 0..20000 {
            let t = f64::from(i) * 0.011;
            let v = p.at3(t, t * 1.37, t * 0.61);
            assert!(v.is_finite() && v.abs() <= 1.0, "out of range: {v}");
            hi = hi.max(v.abs());
        }
        // Scaled to use the range, not left peaking at 0.87.
        assert!(hi > 0.8, "the scale constant is too small: peak {hi}");
    }

    /// The sphere-sampled fBm is seamless BY CONSTRUCTION, in every
    /// direction - that is the whole reason the solid is displaced in 3D
    /// rather than over a `(theta, u)` domain, which would pinch at the
    /// poles. Walking a full turn of longitude and a full turn of
    /// colatitude both have to come back to the same value.
    #[test]
    fn the_sphere_fbm_has_no_seam_anywhere() {
        let mut rng = Pcg32::new(9, stream::BLOB);
        let p = Perlin::new(&mut rng);
        let f = SphereFbm::new(&mut rng, 4, 2.03, 0.45);
        let z = V3::new(0.0, 0.0, 0.0);
        let at = |th: f64, u: f64| f.at(&p, V3::dir(th, u), 2.6, z);

        for &u in &[-1.2, -0.4, 0.0, 0.7, 1.3] {
            let (a, b) = (at(0.0, u), at(TAU, u));
            assert!((a - b).abs() < 1e-9, "longitude seam at u {u}: {a} vs {b}");
        }
        // Over the pole: dir(th, PI/2 - e) and dir(th + PI, PI/2 + e) are
        // the same direction, so the field must agree there too.
        for i in 0..8 {
            let th = TAU * f64::from(i) / 8.0;
            let e = 0.03;
            let a = f.at(&p, V3::dir(th, crate::math::PI / 2.0 - e), 2.6, z);
            let b = f.at(
                &p,
                V3::dir(th + crate::math::PI, crate::math::PI / 2.0 + e),
                2.6,
                z,
            );
            assert!((a - b).abs() < 1e-9, "pole seam at theta {th}: {a} vs {b}");
        }
    }

    /// Same inputs, same bits - the whole crate's promise, at the one place
    /// 3D noise enters it.
    #[test]
    fn perlin3_is_a_pure_function() {
        let a = perlin(3);
        let b = perlin(3);
        for i in 0..500 {
            let t = f64::from(i) * 0.37;
            assert_eq!(a.at3(t, -t * 0.5, t * 2.1), b.at3(t, -t * 0.5, t * 2.1));
        }
    }
}
