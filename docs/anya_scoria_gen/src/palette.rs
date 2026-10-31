//! Colour: the near-black ink, hex formatting, and the three-tone orange ramp.
//!
//! The ramp is the one place where the obvious implementation is wrong. Three
//! evenly spaced lightness steps give a competent, generic gradient; the
//! artwork does something else. Measured off `../img/anya_v0_base.svg`:
//!
//! | tone      | hex       | HSL                      |
//! |-----------|-----------|--------------------------|
//! | shadow    | `#ee3300` | 12.86deg, 100%, 46.67%   |
//! | mid       | `#ff5721` | 14.60deg, 100%, 56.47%   |
//! | highlight | `#ff6f21` | 21.18deg, 100%, 56.47%   |
//!
//! The highlight is **not** lighter than the mid. It has the same lightness and
//! differs only by a hue rotation toward yellow. Preserving that asymmetry is
//! what keeps a recoloured palette looking like the same artist's hand.

/// An 8-bit-per-channel opaque colour. The artwork uses no opacity and neither
/// do we, so there is no alpha channel anywhere in the crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red, 0..=255.
    pub r: u8,
    /// Green, 0..=255.
    pub g: u8,
    /// Blue, 0..=255.
    pub b: u8,
}

impl Rgb {
    /// A colour from its three channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    /// Lowercase `#rrggbb`, the form every SVG in this crate emits.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Parse `#rrggbb`, `rrggbb`, `#rgb` or `rgb`.
    pub fn parse(s: &str) -> Result<Rgb, String> {
        let t = s.trim().trim_start_matches('#');
        let hx = |a: &str| -> Result<u8, String> {
            u8::from_str_radix(a, 16).map_err(|_| format!("`{s}` is not a hex colour"))
        };
        match t.len() {
            6 => Ok(Rgb::new(hx(&t[0..2])?, hx(&t[2..4])?, hx(&t[4..6])?)),
            // #rgb expands by digit doubling, as in CSS: f -> ff.
            3 => {
                let d = |i: usize| -> Result<u8, String> {
                    let v = hx(&t[i..i + 1])?;
                    Ok(v * 17)
                };
                Ok(Rgb::new(d(0)?, d(1)?, d(2)?))
            }
            _ => Err(format!("`{s}` is not a hex colour")),
        }
    }
}

/// Round a 0..=1 channel to a byte. `round` is IEEE-exact, unlike anything in
/// libm, so this is bit-identical on every target.
fn byte(v: f64) -> u8 {
    let x = (v * 255.0).round();
    if x <= 0.0 {
        0
    } else if x >= 255.0 {
        255
    } else {
        x as u8
    }
}

/// HSL to RGB. `h` in degrees (any real, wrapped), `s` and `l` in 0..=1.
///
/// No trig: the standard chroma/hue-sector form needs only `abs`, `%` and
/// multiplication, all IEEE-exact.
pub fn hsl(h: f64, s: f64, l: f64) -> Rgb {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        // hp < 6 always, because h was wrapped into [0, 360).
        _ => (c, 0.0, x),
    };
    let m = l - 0.5 * c;
    Rgb::new(byte(r1 + m), byte(g1 + m), byte(b1 + m))
}

/// The three facet tones, in the order the generator indexes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ramp {
    /// Tone 0, the shadow. Measured `#ee3300`, 21.5% of the silhouette.
    pub shadow: Rgb,
    /// Tone 1, the workhorse mid. Measured `#ff5721`, 38.6%.
    pub mid: Rgb,
    /// Tone 2, the highlight. Measured `#ff6f21`, and rare on purpose: 6.6%.
    pub highlight: Rgb,
}

/// Hue step from the shadow to the mid, degrees at `tone_contrast` 1.0.
/// Measured: 14.60 - 12.86 = 1.74, rounded to the spec's 1.6.
const HUE_SHADOW_TO_MID: f64 = 1.6;
/// Hue step from the mid to the highlight, degrees at `tone_contrast` 1.0.
/// Measured: 21.18 - 14.60 = 6.58. This step, not a lightness step, IS the
/// highlight.
const HUE_MID_TO_HIGHLIGHT: f64 = 6.6;
/// How much darker the shadow is than the mid, in HSL lightness, at
/// `tone_contrast` 1.0. Measured: 0.5647 - 0.4667 = 0.098.
const LIGHT_MID_TO_SHADOW: f64 = 0.098;

impl Ramp {
    /// Build the ramp from the four palette parameters.
    ///
    /// `hue` is the SHADOW hue (13.0 measured off `#ee3300`), because that is
    /// the end of the family that stays put; the mid and the highlight are
    /// defined as rotations off it. `light` is the MID lightness, which the
    /// highlight shares exactly.
    pub fn build(hue: f64, sat: f64, light: f64, contrast: f64) -> Ramp {
        let mid_hue = hue + HUE_SHADOW_TO_MID * contrast;
        Ramp {
            shadow: hsl(hue, sat, light - LIGHT_MID_TO_SHADOW * contrast),
            mid: hsl(mid_hue, sat, light),
            highlight: hsl(mid_hue + HUE_MID_TO_HIGHLIGHT * contrast, sat, light),
        }
    }

    /// Tone by index: 0 shadow, 1 mid, 2 highlight. Any other index is the mid,
    /// which is the tone that can never look wrong on a stray facet.
    pub fn tone(&self, i: usize) -> Rgb {
        match i {
            0 => self.shadow,
            2 => self.highlight,
            _ => self.mid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the module: the default parameters must land on the
    /// artist's three inks, not on a generic three-step gradient.
    #[test]
    fn default_ramp_reproduces_the_artwork() {
        let r = Ramp::build(13.0, 1.00, 0.565, 1.00);
        let close = |a: Rgb, b: Rgb, what: &str| {
            let d = |x: u8, y: u8| (x as i32 - y as i32).abs();
            assert!(
                d(a.r, b.r) <= 2 && d(a.g, b.g) <= 2 && d(a.b, b.b) <= 2,
                "{what}: got {} want {}",
                a.hex(),
                b.hex()
            );
        };
        close(r.shadow, Rgb::new(0xee, 0x33, 0x00), "shadow");
        close(r.mid, Rgb::new(0xff, 0x57, 0x21), "mid");
        close(r.highlight, Rgb::new(0xff, 0x6f, 0x21), "highlight");
    }

    /// The measured family rule, asserted directly: same lightness, warmer hue.
    #[test]
    fn highlight_is_not_lighter_than_the_mid() {
        let r = Ramp::build(13.0, 1.00, 0.565, 1.00);
        let lum = |c: Rgb| {
            (c.r as i32).max(c.g as i32).max(c.b as i32)
                + (c.r as i32).min(c.g as i32).min(c.b as i32)
        };
        assert_eq!(lum(r.highlight), lum(r.mid), "HSL lightness must be equal");
        assert!(
            r.highlight.g > r.mid.g,
            "the highlight is hue-rotated toward yellow"
        );
        assert!(lum(r.shadow) < lum(r.mid), "the shadow IS darker");
    }

    /// Moving `palette_hue` alone must transport the whole family coherently.
    #[test]
    fn hue_moves_the_family_together() {
        let a = Ramp::build(13.0, 1.0, 0.565, 1.0);
        let b = Ramp::build(205.0, 1.0, 0.565, 1.0);
        assert_ne!(a.mid, b.mid);
        // An ice-blue variant: blue dominant in all three, nothing washed out.
        for c in [b.shadow, b.mid, b.highlight] {
            assert!(c.b > c.r, "{} should be blue-dominant", c.hex());
        }
    }

    #[test]
    fn hex_round_trips() {
        for c in [
            Rgb::new(0x23, 0x1f, 0x20),
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
        ] {
            assert_eq!(Rgb::parse(&c.hex()), Ok(c));
        }
        assert_eq!(Rgb::parse("#f0a"), Ok(Rgb::new(255, 0, 170)));
        assert!(Rgb::parse("nope").is_err());
    }

    #[test]
    fn hsl_hits_the_sector_corners() {
        assert_eq!(hsl(0.0, 1.0, 0.5), Rgb::new(255, 0, 0));
        assert_eq!(hsl(120.0, 1.0, 0.5), Rgb::new(0, 255, 0));
        assert_eq!(hsl(240.0, 1.0, 0.5), Rgb::new(0, 0, 255));
        assert_eq!(hsl(-120.0, 1.0, 0.5), Rgb::new(0, 0, 255), "hue must wrap");
        assert_eq!(hsl(37.0, 0.0, 0.5), Rgb::new(128, 128, 128));
    }
}
