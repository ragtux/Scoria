//! The one flow field.
//!
//! Hobbs's Fidenza stores a single angle per grid node and every element in
//! the piece reads its heading from that one field, which is why unrelated
//! elements still agree with each other. The same idea, with the same
//! discipline: there is exactly ONE field per seed here and it does three
//! jobs — it leans the silhouette's lumps (by warping the noise lookup, never
//! the geometry), it orients the vesicles' long axes, and it curves the facet
//! cuts. Adding a second field for any of them is what makes a seed read as
//! three unrelated effects instead of one rock.
//!
//! Evaluated pointwise, not on a precomputed grid: at roughly four thousand
//! lookups per logo a 128x96 grid costs about four times more noise
//! evaluations than direct evaluation and adds bilinear interpolation error on
//! top. The grid is Hobbs's answer to taking millions of steps; we do not.

use crate::math::{sin_cos, PI, V2};
use crate::noise::{Fbm, Perlin};
use crate::rng::Pcg32;

/// How a [`Field`] is shaped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldCfg {
    /// Spatial frequency in units of `1/R0`. 0.55 means roughly one full
    /// angular swing across the body; low is right. At 1.5 the facet cuts turn
    /// into squiggles and the vesicle orientations stop agreeing with each
    /// other, which loses the coherence that is the whole point of the field.
    pub freq: f64,
    /// Hobbs's lambda: maps fBm `[-1, 1]` to `bias ± PI * angle_range`. 1.0 is
    /// full turbulence; 0.35 gives a combed, strongly directional field.
    pub angle_range: f64,
    /// Constant added to every angle. Rotates the lean of the lumps, the
    /// vesicle axes and the facet cuts together — the cheapest way to make two
    /// seeds feel like a matched pair.
    pub bias: f64,
    /// Hobbs's non-continuous distortion: snap the angle to a multiple of
    /// this. `Some(PI/6)` reads distinctly more crystalline. Off by default
    /// because it fights the organic silhouette.
    pub quantize: Option<f64>,
    /// fBm depth. 3 is enough: the field is deliberately low-frequency.
    pub octaves: u32,
    /// Per-octave frequency multiplier — 2.03, not 2.0, see [`Fbm::new`].
    pub lacunarity: f64,
    /// Per-octave amplitude multiplier.
    pub gain: f64,
}

impl Default for FieldCfg {
    /// The values measured off the artwork, per SPEC.md's parameter table.
    fn default() -> FieldCfg {
        FieldCfg {
            freq: 0.55,
            angle_range: 1.0,
            bias: 0.0,
            quantize: None,
            octaves: 3,
            lacunarity: 2.03,
            gain: 0.45,
        }
    }
}

/// A direction field over body space.
#[derive(Clone, Debug)]
pub struct Field {
    perlin: Perlin,
    fbm: Fbm,
    cfg: FieldCfg,
}

impl Field {
    /// Build the field, consuming one permutation and `cfg.octaves` offsets
    /// from `rng`.
    #[must_use]
    pub fn new(rng: &mut Pcg32, cfg: FieldCfg) -> Field {
        let perlin = Perlin::new(rng);
        let fbm = Fbm::new(rng, cfg.octaves, cfg.lacunarity, cfg.gain);
        Field { perlin, fbm, cfg }
    }

    /// The configuration this field was built from.
    #[must_use]
    pub fn cfg(&self) -> &FieldCfg {
        &self.cfg
    }

    /// Heading at `p`, in radians. `p` is in units of `R0`, not SVG units.
    #[must_use]
    pub fn angle(&self, p: V2) -> f64 {
        let n = self
            .fbm
            .at(&self.perlin, p.x * self.cfg.freq, p.y * self.cfg.freq);
        let a = self.cfg.bias + PI * self.cfg.angle_range * n;
        match self.cfg.quantize {
            Some(step) if step > 0.0 => (a / step).round() * step,
            _ => a,
        }
    }

    /// Unit direction at `p`.
    ///
    /// Always take the direction as a VECTOR. Averaging or interpolating the
    /// angle instead is the single most common flow-field bug: wherever the
    /// field crosses ±PI the interpolated angle spins a full turn over one
    /// step and leaves a kink in the cut curve, and it is invisible in a debug
    /// render of the field itself.
    #[must_use]
    pub fn dir(&self, p: V2) -> V2 {
        let (s, c) = sin_cos(self.angle(p));
        V2::new(c, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::TAU;
    use crate::rng::stream;

    fn field(cfg: FieldCfg) -> Field {
        Field::new(&mut Pcg32::new(42, stream::BLOB), cfg)
    }

    #[test]
    fn dir_is_always_a_unit_vector() {
        let f = field(FieldCfg::default());
        for i in -30..30 {
            for j in -30..30 {
                let d = f.dir(V2::new(f64::from(i) * 0.1, f64::from(j) * 0.1));
                assert!((d.len() - 1.0).abs() < 1e-15, "len {}", d.len());
            }
        }
    }

    #[test]
    fn dir_agrees_with_angle() {
        let f = field(FieldCfg::default());
        for i in 0..200 {
            let p = V2::new(f64::from(i) * 0.03 - 3.0, f64::from(i) * 0.017 - 1.5);
            let a = f.angle(p);
            let d = f.dir(p);
            let (s, c) = sin_cos(a);
            assert_eq!(d.x.to_bits(), c.to_bits());
            assert_eq!(d.y.to_bits(), s.to_bits());
        }
    }

    #[test]
    fn angle_respects_bias_and_range() {
        let cfg = FieldCfg {
            bias: 0.7,
            angle_range: 0.35,
            ..FieldCfg::default()
        };
        let f = field(cfg);
        let span = PI * 0.35;
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for i in 0..200 {
            for j in 0..200 {
                let a = f.angle(V2::new(
                    f64::from(i) * 0.05 - 5.0,
                    f64::from(j) * 0.05 - 5.0,
                ));
                assert!(a >= 0.7 - span - 1e-12 && a <= 0.7 + span + 1e-12, "{a}");
                lo = lo.min(a);
                hi = hi.max(a);
            }
        }
        // And it must actually sweep, or `angle_range` would be a dead knob.
        assert!(hi - lo > span, "{lo} .. {hi}");
    }

    #[test]
    fn quantize_snaps_to_multiples() {
        let step = PI / 6.0;
        let f = field(FieldCfg {
            quantize: Some(step),
            ..FieldCfg::default()
        });
        for i in 0..500 {
            let a = f.angle(V2::new(f64::from(i) * 0.013, f64::from(i) * -0.021));
            let k = (a / step).round();
            assert!(
                (a - k * step).abs() < 1e-12,
                "{a} is not a multiple of {step}"
            );
        }
        // A zero or negative step is ignored rather than dividing by zero.
        let z = field(FieldCfg {
            quantize: Some(0.0),
            ..FieldCfg::default()
        });
        assert!(z.angle(V2::new(0.3, 0.4)).is_finite());
    }

    #[test]
    fn the_field_is_coherent_at_the_default_frequency() {
        // One swing across the body: a step of 0.05 R0 must barely turn the
        // heading. If this fails the field is too high-frequency to orient
        // anything, and the vesicles stop agreeing with each other.
        let f = field(FieldCfg::default());
        let mut worst = 0.0f64;
        for i in -20..20 {
            for j in -20..20 {
                let p = V2::new(f64::from(i) * 0.1, f64::from(j) * 0.1);
                let a = f.dir(p);
                let b = f.dir(p + V2::new(0.05, 0.0));
                worst = worst.max(1.0 - a.dot(b));
            }
        }
        assert!(worst < 0.02, "neighbouring headings disagree by {worst}");
    }

    #[test]
    fn same_seed_same_field() {
        let a = field(FieldCfg::default());
        let b = field(FieldCfg::default());
        let c = Field::new(&mut Pcg32::new(43, stream::BLOB), FieldCfg::default());
        let p = V2::new(0.31, -0.72);
        assert_eq!(a.angle(p).to_bits(), b.angle(p).to_bits());
        assert_ne!(a.angle(p).to_bits(), c.angle(p).to_bits());
    }

    #[test]
    fn bias_rotates_the_whole_field_rigidly() {
        let base = field(FieldCfg::default());
        let turned = field(FieldCfg {
            bias: 1.0,
            ..FieldCfg::default()
        });
        for i in 0..100 {
            let p = V2::new(f64::from(i) * 0.07 - 3.0, f64::from(i) * -0.05 + 2.0);
            let d = turned.angle(p) - base.angle(p);
            assert!((d - 1.0).abs() < 1e-12, "{d}");
        }
    }

    #[test]
    fn a_full_turn_of_bias_is_the_same_field() {
        let a = field(FieldCfg::default());
        let b = field(FieldCfg {
            bias: TAU,
            ..FieldCfg::default()
        });
        let p = V2::new(0.5, 0.25);
        assert!((a.dir(p) - b.dir(p)).len() < 1e-12);
    }
}
