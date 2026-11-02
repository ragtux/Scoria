//! The parameter surface.
//!
//! Every knob in SPEC.md's parameter table is a field of [`Params`] with the
//! spec's default, which is a measurement off the artwork wherever one could be
//! taken. On top of that: [`Params::apply_tameness`], the single dial a
//! non-expert touches; [`Params::set`] for `--set key=value`; and
//! [`Params::validate`], which enforces the kappa/aspect interlock that stops
//! the limiter from silently circularising the blob.

use crate::palette::Rgb;
use crate::vesicle;

/// Linear interpolation, used only by the tameness dial.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// How much silhouette AREA the projection adds at a given `lump_amp`,
/// relative to the area at the default.
///
/// The maximum over the out-of-plane tilt can only add radius, and what it
/// adds most of is where the displacement dipped - a notch is filled in by
/// whatever tilt still has radius to offer, while a bump is already the
/// maximum and gains nothing. So the silhouette gets ROUNDER as the
/// displacement gets stronger, and with the mean radius normalised, rounder
/// means more area.
///
/// Measured over 200 seeds at each of the three presets, as
/// `area / (PI * R0^2)` after the limiter and the smoothing: 1.041 at
/// `lump_amp` 0.120, 1.105 at 0.201, 1.206 at 0.300. Linear in `lump_amp` to
/// within the noise, at 0.92 of area per unit of amplitude.
///
/// Vesicle sizes are fractions of R0, so they do not know about this, and
/// coverage - a gated AREA ratio - falls as the body grows. Scaling the
/// radii by the square root of the growth holds it.
pub fn body_growth(lump_amp: f64) -> f64 {
    let at = |a: f64| 1.041 + 0.92 * (a - 0.120);
    at(lump_amp) / at(Params::default().lump_amp)
}

/// The in-plane aspect the base ELLIPSOID needs for the projected silhouette
/// to come out at `aspect`.
///
/// The projection squares the shape up, and the mechanism is worth stating
/// because it is not obvious. Along a given image angle the silhouette is the
/// maximum of `R * cos u` over the out-of-plane tilt `u`, and how wide a
/// window of `u` competes for that maximum depends on how sharply the
/// ellipsoid's own radius falls off along that great circle. Along the LONG
/// in-plane axis the fall-off to the depth axis is steep, so the window is
/// narrow and the maximum sees little of the displacement; along the SHORT
/// axis it is shallow, the window is wide, and the maximum is more likely to
/// catch a big lump. So the short axis gains more radius than the long one,
/// and the aspect drops.
///
/// Measured over 300 seeds at nine base aspects from 1.0 to 3.2, taking the
/// median realised bbox aspect of the finished outline:
///
/// | base | 1.00 | 1.25 | 1.50 | 1.70 | 2.00 | 2.40 | 2.80 | 3.20 |
/// |---|---|---|---|---|---|---|---|---|
/// | realised | 1.01 | 1.20 | 1.38 | 1.51 | 1.70 | 1.99 | 2.26 | 2.51 |
///
/// Inverting that at the default gives a base of 1.69 for a realised 1.50,
/// i.e. a gain of 1.38 on `aspect - 1`. The same constant lands within 3% of
/// the request everywhere else in the parameter's range, which is an eighth
/// of the bbox gate's own tolerance, so one constant is enough and a fitted
/// curve would be false precision.
pub fn base_aspect(aspect: f64) -> f64 {
    1.0 + (aspect - 1.0) * 1.38
}

/// The lower bound `slope_limit` must respect at a given aspect.
///
/// The base ellipse itself has `max|r'/r| = (aspect^2 - 1) / (2*aspect)`
/// (0.417 at the default aspect 1.50). Set kappa below that and the limiter
/// eats the ellipse before it ever sees a lump: the aspect parameter stops
/// working and nobody can tell why. The +0.40 is the budget the lumps need.
pub fn min_slope_limit(aspect: f64) -> f64 {
    (aspect * aspect - 1.0) / (2.0 * aspect) + 0.40
}

/// Which knobs `tameness` drives, in bit order. Bit 7 is `tameness` itself, so
/// a caller can tell whether the dial was ever touched.
/// Lower end of `vesicle_rmin`'s valid range, as a fraction of R0.
const VESICLE_RMIN_MIN: f64 = 0.030;
/// Lower end of `vesicle_rmax`'s valid range, as a fraction of R0.
///
/// Named rather than inlined because the tameness dial clamps into the same
/// range `validate` enforces, and two copies of a bound drift apart.
const VESICLE_RMAX_MIN: f64 = 0.100;
/// Upper end of the same.
const VESICLE_RMAX_MAX: f64 = 0.400;

const TAMED: [&str; 8] = [
    "lump_amp",
    "lump_octaves",
    "slope_limit",
    "warp_amt",
    "vesicle_count",
    "vesicle_amp",
    "vesicle_bite",
    "facet_curviness",
];
/// Bit index of `tameness` in [`Params::explicit`].
const BIT_TAMENESS: u32 = 7;

/// The type of a parameter, for `--list-params` and for `--set` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 64-bit unsigned integer.
    U64,
    /// 32-bit unsigned integer.
    U32,
    /// Double.
    F64,
    /// Double or the word `none`.
    OptF64,
    /// Three comma-separated doubles.
    F64x3,
    /// `#rrggbb`.
    Color,
    /// `#rrggbb` or the word `none`.
    OptColor,
}

impl Kind {
    /// The name shown in the `--list-params` table.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::U64 => "u64",
            Kind::U32 => "u32",
            Kind::F64 => "f64",
            Kind::OptF64 => "f64?",
            Kind::F64x3 => "f64x3",
            Kind::Color => "rgb",
            Kind::OptColor => "rgb?",
        }
    }
}

/// One row of the parameter table.
pub struct ParamInfo {
    /// The name `--set` accepts.
    pub name: &'static str,
    /// How the value is parsed.
    pub kind: Kind,
    /// The default, formatted as `--set` would accept it.
    pub default: &'static str,
    /// The usable range, as prose.
    pub range: &'static str,
    /// One line on what moving it does.
    pub effect: &'static str,
}

/// Everything that decides what a logo looks like.
///
/// Construct with [`Params::default`] (the measured defaults) or one of the
/// presets, then mutate fields directly or through [`Params::set`]. Note that
/// a field assigned directly is not marked explicit; call
/// [`Params::mark_explicit`] if you also intend to use the tameness dial.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// The whole image. Split into five independent PCG32 streams so that
    /// changing one subsystem does not reshuffle the others.
    pub seed: u64,
    /// 0 feral, 1 corporate. Drives seven other knobs; see
    /// [`Params::apply_tameness`], which a caller must invoke explicitly.
    pub tameness: f64,
    /// Width/height of the base ellipse. Semi-axes `R0*sqrt(a)` and
    /// `R0/sqrt(a)`, so the geometric mean radius is R0 at any aspect.
    pub aspect: f64,
    /// How deep the solid is, as a fraction of its vertical semi-axis.
    pub depth: f64,
    /// Canonical mean radius. Everything else is a fraction of it.
    pub r0: f64,
    /// viewBox padding around the silhouette's true bbox, as a fraction of R0.
    pub margin: f64,
    /// Radial fbm amplitude as a fraction of the base ellipse radius: the
    /// primary silhouette-chaos knob.
    pub lump_amp: f64,
    /// Base angular frequency - roughly how many primary lumps go round.
    pub lump_freq: f64,
    /// fbm depth for the silhouette. The measured spectrum supports exactly 4.
    pub lump_octaves: u32,
    /// Per-octave frequency multiplier. Deliberately not 2.0.
    pub lacunarity: f64,
    /// Per-octave amplitude multiplier.
    pub gain: f64,
    /// How far the flow field displaces the noise LOOKUP (never the geometry).
    pub warp_amt: f64,
    /// Flow field spatial frequency, in units of 1/R0.
    pub field_freq: f64,
    /// Hobbs's lambda: maps fbm [-1,1] to `bias +- PI*lambda`.
    pub field_angle_range: f64,
    /// Constant added to every field angle; rotates the whole directional
    /// character at once.
    pub field_bias: f64,
    /// Snap field angles to a multiple of this. `Some(PI/6)` reads crystalline.
    pub field_quantize: Option<f64>,
    /// kappa in `|d(ln r)/d theta| <= kappa`: the anti-cusp, anti-loop guard.
    pub slope_limit: f64,
    /// Samples of `r(theta)` before arclength resampling.
    pub ring_samples: u32,
    /// Points after arclength resampling: the number of emitted cubics.
    pub bezier_points: u32,
    /// tau in the Catmull-Rom handle. 0.50 is true Catmull-Rom.
    pub cr_tension: f64,
    /// Perceived ribbon width as a fraction of R0. The most brand-defining
    /// number in the file.
    pub outline_weight: f64,
    /// Optional second concentric ribbon inside the first, fraction of R0.
    /// The artwork's file carries one (2.49 units, 0.0147 R0) but it lies
    /// entirely under the main ribbon: removing it changes zero rendered
    /// pixels. Off is what the drawing looks like.
    pub inner_rim: f64,
    /// Interior relief marks: thin tapered lines down the sharpest creases of
    /// the solid, each growing out of the outline ribbon.
    pub relief_lines: u32,
    /// Nominal width of a relief mark, as a fraction of R0. The realised
    /// width swells and thins about it along the mark.
    pub relief_weight: f64,
    /// Centreline length of a relief mark, as a fraction of R0.
    pub relief_len: f64,
    /// How much a relief mark's width swells and thins along its length
    /// with the sharpness of the fold it traces. 0 is a monoline.
    pub relief_modulation: f64,
    /// Number of bisections; regions = cuts + 1.
    pub facet_cuts: u32,
    /// How strongly a facet cut follows the solid's iso-brightness lines
    /// rather than the flow field.
    pub facet_relief: f64,
    /// 0 = a frozen straight chord, 1 = a live flow-field trace.
    ///
    /// 0.25, against SPEC.md's table, which says 1.00. The table's own
    /// measurement contradicts it: Part 1 has the facets' long edges (>= 30
    /// units) "deviating less than 6% from their chord", with the longest
    /// running 145 units dead straight. A full flow trace at the measured
    /// `field_freq` sweeps far more than that - measured over twelve seeds,
    /// median deviation goes 7.7% at curviness 0 to 12.7% at 1.0 - and the
    /// rendered tone boundaries come out as soft sweeping bands instead of
    /// the artwork's shards. 0.25 sits just off the straight floor: the cuts
    /// read as chords with a hand-drawn bow, and the field still bends them,
    /// so they keep agreeing with the lumps they run between.
    pub facet_curviness: f64,
    /// Reject a cut whose smaller child is below this fraction of body area.
    pub facet_min_area: f64,
    /// Minimum separation of a cut's two crossings, as a fraction of the
    /// region perimeter measured the short way.
    pub facet_min_chord_sep: f64,
    /// Target area share of shadow / mid / highlight, applied as quantiles.
    pub tone_fractions: [f64; 3],
    /// Direction of the tone gradient, radians, SVG coordinates (y down).
    /// Direction the light comes from, in the image plane, radians.
    ///
    /// 2.30 rad, down and to the left. Measured as the axis through the three
    /// tones' own visible-pixel centroids: (276.9, 135.3) dark to
    /// (179.0, 244.9) light is the unit vector (-0.666, +0.746), which
    /// SPEC.md Part 1 gives and which is 2.30 rad. The artwork is lit from
    /// BELOW, which is what makes it read as glowing lava rather than as a
    /// lit pebble - not an error to fix.
    pub light_dir: f64,
    /// The light's component toward the viewer.
    pub light_elev: f64,
    /// How much of a facet's tone comes from Lambert shading of the solid's
    /// real surface normal, against the flat positional ramp.
    pub tone_lambert: f64,
    /// Blend of the light function between a directional ramp and pure fbm.
    pub tone_noise: f64,
    /// Base hue in degrees: the SHADOW hue of the three-tone ramp.
    pub palette_hue: f64,
    /// HSL saturation for all three tones.
    pub palette_sat: f64,
    /// HSL lightness of the MID tone, which the other two are defined against.
    pub palette_light: f64,
    /// Scales both deltas off the mid tone.
    pub tone_contrast: f64,
    /// The near-black used for the outline ribbon and the vesicles.
    pub ink: Rgb,
    /// `None` emits no background rect, which is what a logo wants.
    pub background: Option<Rgb>,
    /// Target number of gas holes. The packer drops any it cannot place.
    pub vesicle_count: u32,
    /// Smallest vesicle equivalent radius, fraction of R0.
    pub vesicle_rmin: f64,
    /// Largest vesicle equivalent radius, fraction of R0.
    ///
    /// 0.320, above the 0.260 of SPEC.md's table, because the packer never
    /// gets to use the top of the range: clearance against the eroded body
    /// rejects the biggest candidates, and the largest hole actually placed
    /// runs 0.19-0.22 of R0 against the artwork's measured 0.239. Raising the
    /// ceiling raises what survives. Measured over 20 seeds, 0.320 puts total
    /// coverage at 0.20 - the artwork's 0.207 - with no re-rolls.
    pub vesicle_rmax: f64,
    /// `r = rmin + (rmax - rmin) * u^pow`. Integer, so it is multiplication.
    ///
    /// 1, against SPEC.md's parameter table, which says 2. The table and
    /// Part 1 contradict each other and Part 1 is the measurement: the seven
    /// measured equivalent radii scaled to R0 = 300 are 23, 30, 45, 50, 56,
    /// 68, 72 over a range of [21, 78] - near-uniform, with four of seven in
    /// the upper half, which is also what Part 1 says in words ("Do NOT use a
    /// steep power law here" and "biased toward the upper half"). `pow = 2`
    /// biases the opposite way; measured over 20 seeds it holds total vesicle
    /// coverage at 0.152 against the measured 0.207 and leaves the gate's own
    /// 0.12 floor one bad roll away, which is where most re-rolls were going.
    pub vesicle_size_pow: u32,
    /// Radial fbm amplitude for a vesicle's own outline, fraction of its
    /// radius. Scaled by size inside the packer: big holes are lumpier.
    pub vesicle_amp: f64,
    /// Depth of the single concave bite carved into the biggest holes, as a
    /// fraction of the local radius.
    ///
    /// A star-shaped fbm blob bottoms out around 0.75 circularity, however
    /// much amplitude it is given, because lobes are bumps and a bump is not
    /// a bite. The artwork's two largest holes measure 0.405 and 0.459 - an L
    /// with a hooked tail and a kidney bitten out on one side - so the
    /// re-entrancy has to be cut in deliberately rather than hoped for.
    /// Scaled to zero on the small holes, which measure 0.70-0.87 and are
    /// correctly round already.
    pub vesicle_bite: f64,
    /// fbm depth per vesicle, for the largest; small ones use one less.
    pub vesicle_octaves: u32,
    /// Elongation of the largest vesicle; aspect is interpolated by size.
    pub vesicle_aspect_max: f64,
    /// How strongly a vesicle's long axis follows the field at its centre.
    pub vesicle_align: f64,
    /// How strongly a hole's shape follows the surface it lies on: its long
    /// axis turns to the local contour and it foreshortens by the tilt.
    pub vesicle_contour: f64,
    /// Caps a hole at this much of its Voronoi territory. 1.0, the default,
    /// lifts the cap: the territory then decides only where a hole sits,
    /// not how big it is. Below 1 it thins the packing out.
    pub vesicle_fill: f64,
    /// How strongly a hole's size follows the shading, so the holes darken
    /// the shadow side rather than being scattered over it.
    pub vesicle_shade: f64,
    /// Minimum gap to the body's INNER boundary, fraction of R0, floored at
    /// `1.2 * outline_weight * R0`.
    pub vesicle_edge_clear: f64,
    /// Gap to the inner boundary as a multiple of the hole's OWN equivalent
    /// radius; the larger of this and `vesicle_edge_clear` binds. This is
    /// what puts the big holes deep inside and lets only the small ones near
    /// the rim - the drawing's clearance over radius runs 1.55, 0.97, 0.89,
    /// 0.93, 0.87, 0.59, 0.58 largest to smallest. 0 makes the gap absolute.
    pub vesicle_edge_ratio: f64,
    /// Minimum vesicle-to-vesicle gap, fraction of R0, additionally floored at
    /// `0.45 * min(ri, rj)`.
    pub vesicle_gap: f64,
    /// Placement tries per vesicle before giving up on it.
    pub vesicle_attempts: u32,
    /// Global placement budget.
    pub attempts_total: u32,
    /// End the packing loop after this many consecutive failures.
    pub consec_fail_stop: u32,
    /// Decimal places in emitted coordinates.
    pub coord_decimals: u32,
    /// Whole-logo re-rolls if any quality gate fails.
    pub retry_budget: u32,
    /// Bitmask of knobs set explicitly through [`Params::set`], so
    /// [`Params::apply_tameness`] knows what not to overwrite.
    pub explicit: u64,
}

impl Default for Params {
    /// SPEC.md's parameter table, verbatim. Every default is a measurement off
    /// the artwork except where the table says otherwise.
    fn default() -> Self {
        Params {
            seed: 0,
            tameness: 0.55,
            aspect: 1.50,
            depth: 0.72,
            r0: 300.0,
            margin: 0.04,
            lump_amp: 0.22,
            lump_freq: 2.6,
            lump_octaves: 4,
            lacunarity: 2.03,
            gain: 0.45,
            warp_amt: 0.22,
            field_freq: 0.55,
            field_angle_range: 1.0,
            field_bias: 0.0,
            field_quantize: None,
            slope_limit: 1.10,
            ring_samples: 720,
            bezier_points: 256,
            cr_tension: 0.50,
            outline_weight: 0.058,
            inner_rim: 0.0,
            relief_lines: 1,
            relief_weight: 0.044,
            relief_len: 0.54,
            relief_modulation: 0.80,
            facet_cuts: 11,
            facet_relief: 0.80,
            facet_curviness: 0.25,
            facet_min_area: 0.015,
            facet_min_chord_sep: 0.08,
            tone_fractions: [0.30, 0.60, 0.10],
            light_dir: 2.30,
            light_elev: 0.22,
            tone_lambert: 0.85,
            tone_noise: 0.35,
            palette_hue: 13.0,
            palette_sat: 1.00,
            palette_light: 0.565,
            tone_contrast: 1.00,
            ink: Rgb::new(0x23, 0x1f, 0x20),
            background: None,
            vesicle_count: 5,
            vesicle_rmin: 0.090,
            vesicle_rmax: 0.300,
            vesicle_size_pow: 1,
            vesicle_amp: 0.30,
            vesicle_bite: 0.60,
            vesicle_octaves: 3,
            vesicle_aspect_max: 2.40,
            vesicle_align: 0.85,
            vesicle_contour: 0.85,
            vesicle_fill: 1.00,
            vesicle_shade: 0.35,
            vesicle_edge_clear: 0.055,
            vesicle_edge_ratio: 0.80,
            vesicle_gap: 0.090,
            vesicle_attempts: 220,
            attempts_total: 2000,
            consec_fail_stop: 150,
            coord_decimals: 3,
            retry_budget: 20,
            explicit: 0,
        }
    }
}

impl Params {
    /// The artist's drawing, as closely as the parameters can aim at it.
    ///
    /// The defaults already carry the measured values; this adds the three
    /// places where SPEC.md's table deliberately departs from the artwork for
    /// the sake of a general-purpose logo: the drawing bleeds to the canvas
    /// edge (margin 0), it carries the second thin rim, and its tone split was
    /// measured to three places.
    pub fn anya() -> Params {
        Params {
            margin: 0.0,
            // Not the file's 0.0147: that second rim is dead geometry in the
            // artwork, fully occluded by the main ribbon (Part 1, path14, 0
            // visible pixels - re-checked by removing it and diffing the
            // render). Drawing it put a hairline on the mark the drawing has
            // never shown.
            inner_rim: 0.0,
            tone_fractions: [0.298, 0.603, 0.099],
            ..Params::default()
        }
    }

    /// Corporate: smooth, few holes, nearly straight facet cuts.
    pub fn tame() -> Params {
        let mut p = Params {
            tameness: 1.0,
            // A combed, strongly directional field - SPEC.md's named knob for
            // a tamer, more graphic rock.
            field_angle_range: 0.35,
            ..Params::default()
        };
        p.mark_explicit("field_angle_range");
        p.apply_tameness();
        p
    }

    /// Feral: deep lumps, many holes, cuts that visibly curve.
    ///
    /// Deliberately NOT also cranking `vesicle_aspect_max`: eleven holes in a
    /// body this lumpy already strain the packer, and a 3.0 aspect on the
    /// largest means a 200-unit half-extent that almost never fits, so the
    /// big ones get dropped and the size-spread gate rejects every roll.
    pub fn feral() -> Params {
        let mut p = Params {
            tameness: 0.0,
            // Past 1.0 the field winds past a full turn and makes vortices,
            // which is the point of this preset.
            field_angle_range: 1.25,
            ..Params::default()
        };
        p.mark_explicit("field_angle_range");
        p.apply_tameness();
        p
    }

    /// Presets by name, for `--preset`.
    pub fn preset(name: &str) -> Option<Params> {
        match name {
            "anya" => Some(Params::anya()),
            "tame" => Some(Params::tame()),
            "feral" => Some(Params::feral()),
            _ => None,
        }
    }

    /// The names [`Params::preset`] accepts.
    pub const PRESETS: &'static [&'static str] = &["anya", "tame", "feral"];

    /// Has `key` been set explicitly (and so is off limits to the dial)?
    pub fn is_explicit(&self, key: &str) -> bool {
        match Params::bit_of(key) {
            Some(b) => self.explicit & (1u64 << b) != 0,
            None => false,
        }
    }

    /// Mark `key` as explicitly set. Only the eight keys the tameness dial
    /// touches have a bit; everything else is a no-op, because nothing else
    /// can be overwritten behind the caller's back.
    pub fn mark_explicit(&mut self, key: &str) {
        if let Some(b) = Params::bit_of(key) {
            self.explicit |= 1u64 << b;
        }
    }

    /// Bit index for the keys the dial can overwrite.
    fn bit_of(key: &str) -> Option<u32> {
        if key == "tameness" {
            return Some(BIT_TAMENESS);
        }
        TAMED.iter().position(|&k| k == key).map(|i| i as u32)
    }

    /// Drive the seven dependent knobs from `tameness`, skipping any the caller
    /// set explicitly.
    ///
    /// This is NOT called by [`crate::generate`]: a `Params` is used exactly as
    /// handed over, so a caller who never touches the dial gets the measured
    /// defaults. Call it yourself after setting `tameness`.
    pub fn apply_tameness(&mut self) {
        let t = self.tameness.clamp(0.0, 1.0);
        if !self.is_explicit("lump_amp") {
            self.lump_amp = lerp(0.30, 0.12, t);
        }
        if !self.is_explicit("lump_octaves") {
            self.lump_octaves = lerp(5.0, 3.0, t).round() as u32;
        }
        if !self.is_explicit("slope_limit") {
            // The tame end of the dial is 0.80, which is BELOW the interlock at
            // the default aspect (0.817). Taking the max keeps `--preset tame`
            // from producing a parameter set that cannot validate.
            self.slope_limit = lerp(1.80, 0.80, t).max(min_slope_limit(base_aspect(self.aspect)));
        }
        if !self.is_explicit("warp_amt") {
            self.warp_amt = lerp(0.45, 0.10, t);
        }
        if !self.is_explicit("vesicle_count") {
            // 9 at the feral end, against SPEC.md's 11. What has to fit is
            // not a hole's area but its REACH - r * sqrt(aspect) * (1 + amp)
            // - and feral is where all three are at their wild ends at once:
            // at count 11 the largest drawn hole reaches 181 units in a body
            // of mean radius 300, the packer drops it, and 30% of seeds then
            // exhaust the whole re-roll budget on the size-spread gate.
            // Measured over 60 seeds: 11 -> 18 failures, 10 -> 12, 9 -> 4.
            // Coupling `vesicle_aspect_max` instead moves nothing (20/18/18
            // at 1.8/2.0/2.4), so the count is the real constraint.
            //
            // Fewer than the artwork's seven, and bigger - a deliberate
            // departure from the measurement, asked for and recorded in
            // Part 3. lerp(7, 4) passes through 5 at the default t = 0.55,
            // which is `Params::default().vesicle_count`, so the dial still
            // agrees with the parameter table where it is supposed to.
            self.vesicle_count = lerp(7.0, 4.0, t).round() as u32;
        }
        if !self.is_explicit("vesicle_bite") {
            self.vesicle_bite = lerp(0.62, 0.34, t);
        }
        if !self.is_explicit("vesicle_amp") {
            self.vesicle_amp = lerp(0.42, 0.18, t);
        }
        // Size the holes against the REACH the packer tests, not their area.
        //
        // A hole's half-extent along its long axis is
        // `r * sqrt(aspect) * (1 + amp * AMP_BIG)`, and that is what has to
        // fit inside the eroded body with its gap and its edge clearance. Two
        // things on this dial change it: the count (more holes, less room
        // each - total area goes as count * E[r^2], so radius falls as
        // 1/sqrt(count), which also holds the measured 0.207 coverage) and
        // the amplitude (a lumpier hole is a longer hole, linearly).
        //
        // Both ends of the range move together, so the size DISTRIBUTION is
        // untouched - which is what the separate spread gate (largest over
        // smallest placed, >= 2.0) cares about.
        //
        // Measured over 60 feral seeds, which is where every term is at its
        // wild end at once: with the range left at the measured
        // [0.070, 0.320] the packer drops the big holes and 43% of seeds
        // exhaust their whole re-roll budget; scaling by count alone leaves
        // 30%; scaling by both leaves 7%.
        let base = Params::default();
        let count_scale =
            (f64::from(base.vesicle_count) / f64::from(self.vesicle_count.max(1))).sqrt();
        // And against the body the projection actually produces at this
        // amplitude, which grows with it - see `body_growth`.
        let growth_scale = body_growth(self.lump_amp).sqrt();
        // Only ever shrinks. The term exists to stop a lumpier hole from
        // outgrowing the room the packer can find for it; there is no matching
        // reason to INFLATE a smooth one, and letting it (at the tame end,
        // where the amplitude is 0.18) drove the five holes up against the
        // 0.400 ceiling and failed the coverage gate from above on 8% of
        // seeds. `count_scale` already moves coverage in both directions.
        let reach_scale = ((1.0 + base.vesicle_amp * vesicle::AMP_BIG)
            / (1.0 + self.vesicle_amp * vesicle::AMP_BIG))
            .min(1.0);
        let size_scale = count_scale * reach_scale * growth_scale;
        if !self.is_explicit("vesicle_rmax") {
            self.vesicle_rmax =
                (base.vesicle_rmax * size_scale).clamp(VESICLE_RMAX_MIN, VESICLE_RMAX_MAX);
        }
        if !self.is_explicit("vesicle_rmin") {
            self.vesicle_rmin =
                (base.vesicle_rmin * size_scale).clamp(VESICLE_RMIN_MIN, self.vesicle_rmax * 0.5);
        }
        if !self.is_explicit("facet_curviness") {
            // Rescaled with the default: the dial's job is to pass through
            // the measured value at t = 0.55, which lerp(0.55, 0.0) does.
            self.facet_curviness = lerp(0.55, 0.00, t);
        }
    }

    /// Set one parameter by name, as `--set key=value` does.
    ///
    /// Marks the key explicit, so a later [`Params::apply_tameness`] leaves it
    /// alone.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let v = value.trim();
        let f = || -> Result<f64, String> {
            v.parse::<f64>()
                .map_err(|_| format!("{key}: `{v}` is not a number"))
        };
        let u = || -> Result<u32, String> {
            v.parse::<u32>()
                .map_err(|_| format!("{key}: `{v}` is not a non-negative integer"))
        };
        match key {
            "seed" => {
                self.seed = v
                    .parse::<u64>()
                    .map_err(|_| format!("seed: `{v}` is not a u64"))?
            }
            "tameness" => self.tameness = f()?,
            "aspect" => self.aspect = f()?,
            "depth" => self.depth = f()?,
            "r0" => self.r0 = f()?,
            "margin" => self.margin = f()?,
            "lump_amp" => self.lump_amp = f()?,
            "lump_freq" => self.lump_freq = f()?,
            "lump_octaves" => self.lump_octaves = u()?,
            "lacunarity" => self.lacunarity = f()?,
            "gain" => self.gain = f()?,
            "warp_amt" => self.warp_amt = f()?,
            "field_freq" => self.field_freq = f()?,
            "field_angle_range" => self.field_angle_range = f()?,
            "field_bias" => self.field_bias = f()?,
            "field_quantize" => {
                self.field_quantize = if v.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(f()?)
                }
            }
            "slope_limit" => self.slope_limit = f()?,
            "ring_samples" => self.ring_samples = u()?,
            "bezier_points" => self.bezier_points = u()?,
            "cr_tension" => self.cr_tension = f()?,
            "outline_weight" => self.outline_weight = f()?,
            "inner_rim" => self.inner_rim = f()?,
            "relief_lines" => self.relief_lines = u()?,
            "relief_weight" => self.relief_weight = f()?,
            "relief_len" => self.relief_len = f()?,
            "relief_modulation" => self.relief_modulation = f()?,
            "facet_cuts" => self.facet_cuts = u()?,
            "facet_relief" => self.facet_relief = f()?,
            "facet_curviness" => self.facet_curviness = f()?,
            "facet_min_area" => self.facet_min_area = f()?,
            "facet_min_chord_sep" => self.facet_min_chord_sep = f()?,
            "tone_fractions" => {
                let parts: Vec<&str> = v.split(',').collect();
                if parts.len() != 3 {
                    return Err("tone_fractions: want three comma-separated numbers".into());
                }
                let mut out = [0.0; 3];
                for (i, s) in parts.iter().enumerate() {
                    out[i] = s
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("tone_fractions: `{s}` is not a number"))?;
                }
                self.tone_fractions = out;
            }
            "light_dir" => self.light_dir = f()?,
            "light_elev" => self.light_elev = f()?,
            "tone_lambert" => self.tone_lambert = f()?,
            "tone_noise" => self.tone_noise = f()?,
            "palette_hue" => self.palette_hue = f()?,
            "palette_sat" => self.palette_sat = f()?,
            "palette_light" => self.palette_light = f()?,
            "tone_contrast" => self.tone_contrast = f()?,
            "ink" => self.ink = Rgb::parse(v)?,
            "background" => {
                self.background = if v.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(Rgb::parse(v)?)
                }
            }
            "vesicle_count" => self.vesicle_count = u()?,
            "vesicle_rmin" => self.vesicle_rmin = f()?,
            "vesicle_rmax" => self.vesicle_rmax = f()?,
            "vesicle_size_pow" => self.vesicle_size_pow = u()?,
            "vesicle_amp" => self.vesicle_amp = f()?,
            "vesicle_bite" => self.vesicle_bite = f()?,
            "vesicle_octaves" => self.vesicle_octaves = u()?,
            "vesicle_aspect_max" => self.vesicle_aspect_max = f()?,
            "vesicle_align" => self.vesicle_align = f()?,
            "vesicle_contour" => self.vesicle_contour = f()?,
            "vesicle_fill" => self.vesicle_fill = f()?,
            "vesicle_shade" => self.vesicle_shade = f()?,
            "vesicle_edge_clear" => self.vesicle_edge_clear = f()?,
            "vesicle_edge_ratio" => self.vesicle_edge_ratio = f()?,
            "vesicle_gap" => self.vesicle_gap = f()?,
            "vesicle_attempts" => self.vesicle_attempts = u()?,
            "attempts_total" => self.attempts_total = u()?,
            "consec_fail_stop" => self.consec_fail_stop = u()?,
            "coord_decimals" => self.coord_decimals = u()?,
            "retry_budget" => self.retry_budget = u()?,
            _ => return Err(format!("unknown parameter `{key}`")),
        }
        self.mark_explicit(key);
        Ok(())
    }

    /// Check every range in the table, plus the one interlock between two
    /// parameters that is not visible from either on its own.
    pub fn validate(&self) -> Result<(), String> {
        fchk("tameness", self.tameness, 0.0, 1.0)?;
        fchk("aspect", self.aspect, 1.0, 2.2)?;
        fchk("depth", self.depth, 0.35, 1.40)?;
        fchk("r0", self.r0, 1.0, 1.0e6)?;
        fchk("margin", self.margin, 0.0, 0.15)?;
        fchk("lump_amp", self.lump_amp, 0.05, 0.35)?;
        fchk("lump_freq", self.lump_freq, 1.5, 5.0)?;
        uchk("lump_octaves", self.lump_octaves, 1, 6)?;
        fchk("lacunarity", self.lacunarity, 1.80, 2.40)?;
        fchk("gain", self.gain, 0.30, 0.60)?;
        fchk("warp_amt", self.warp_amt, 0.0, 0.60)?;
        fchk("field_freq", self.field_freq, 0.15, 1.50)?;
        fchk("field_angle_range", self.field_angle_range, 0.15, 1.50)?;
        fchk(
            "field_bias",
            self.field_bias,
            -crate::math::TAU,
            crate::math::TAU,
        )?;
        if let Some(q) = self.field_quantize {
            fchk("field_quantize", q, 0.01, crate::math::PI)?;
        }
        fchk("slope_limit", self.slope_limit, 0.50, 2.00)?;
        uchk("ring_samples", self.ring_samples, 256, 2048)?;
        uchk("bezier_points", self.bezier_points, 96, 512)?;
        fchk("cr_tension", self.cr_tension, 0.35, 0.60)?;
        fchk("outline_weight", self.outline_weight, 0.020, 0.100)?;
        fchk("inner_rim", self.inner_rim, 0.0, 0.030)?;
        uchk("relief_lines", self.relief_lines, 0, 6)?;
        fchk("relief_weight", self.relief_weight, 0.008, 0.060)?;
        fchk("relief_len", self.relief_len, 0.15, 1.00)?;
        fchk("relief_modulation", self.relief_modulation, 0.0, 1.0)?;
        uchk("facet_cuts", self.facet_cuts, 4, 20)?;
        fchk("facet_relief", self.facet_relief, 0.0, 1.0)?;
        fchk("facet_curviness", self.facet_curviness, 0.0, 1.50)?;
        fchk("facet_min_area", self.facet_min_area, 0.005, 0.050)?;
        fchk("facet_min_chord_sep", self.facet_min_chord_sep, 0.03, 0.20)?;
        let mut sum = 0.0;
        for (i, &x) in self.tone_fractions.iter().enumerate() {
            fchk(
                [
                    "tone_fractions[0]",
                    "tone_fractions[1]",
                    "tone_fractions[2]",
                ][i],
                x,
                0.06,
                0.80,
            )?;
            sum += x;
        }
        if (sum - 1.0).abs() > 1e-6 {
            return Err(format!("tone_fractions must sum to 1, got {sum}"));
        }
        fchk(
            "light_dir",
            self.light_dir,
            -crate::math::TAU,
            crate::math::TAU,
        )?;
        fchk("tone_noise", self.tone_noise, 0.0, 1.0)?;
        fchk("light_elev", self.light_elev, 0.0, 1.50)?;
        fchk("tone_lambert", self.tone_lambert, 0.0, 1.0)?;
        fchk("palette_hue", self.palette_hue, -360.0, 720.0)?;
        fchk("palette_sat", self.palette_sat, 0.50, 1.00)?;
        fchk("palette_light", self.palette_light, 0.30, 0.75)?;
        fchk("tone_contrast", self.tone_contrast, 0.40, 2.00)?;
        uchk("vesicle_count", self.vesicle_count, 0, 20)?;
        fchk("vesicle_rmin", self.vesicle_rmin, VESICLE_RMIN_MIN, 0.120)?;
        fchk(
            "vesicle_rmax",
            self.vesicle_rmax,
            VESICLE_RMAX_MIN,
            VESICLE_RMAX_MAX,
        )?;
        if self.vesicle_rmin >= self.vesicle_rmax {
            return Err(format!(
                "vesicle_rmin ({}) must be below vesicle_rmax ({})",
                self.vesicle_rmin, self.vesicle_rmax
            ));
        }
        uchk("vesicle_size_pow", self.vesicle_size_pow, 1, 3)?;
        fchk("vesicle_amp", self.vesicle_amp, 0.10, 0.50)?;
        fchk("vesicle_bite", self.vesicle_bite, 0.0, 0.70)?;
        uchk("vesicle_octaves", self.vesicle_octaves, 2, 5)?;
        fchk("vesicle_aspect_max", self.vesicle_aspect_max, 1.0, 3.2)?;
        fchk("vesicle_align", self.vesicle_align, 0.0, 1.0)?;
        fchk("vesicle_contour", self.vesicle_contour, 0.0, 1.0)?;
        fchk("vesicle_fill", self.vesicle_fill, 0.30, 1.00)?;
        fchk("vesicle_shade", self.vesicle_shade, 0.0, 1.0)?;
        fchk("vesicle_edge_clear", self.vesicle_edge_clear, 0.030, 0.120)?;
        fchk("vesicle_edge_ratio", self.vesicle_edge_ratio, 0.0, 1.50)?;
        fchk("vesicle_gap", self.vesicle_gap, 0.040, 0.200)?;
        uchk("vesicle_attempts", self.vesicle_attempts, 50, 1000)?;
        uchk("attempts_total", self.attempts_total, 500, 20000)?;
        uchk("consec_fail_stop", self.consec_fail_stop, 40, 600)?;
        uchk("coord_decimals", self.coord_decimals, 2, 4)?;
        uchk("retry_budget", self.retry_budget, 1, 100)?;

        // THE INTERLOCK. Everything above is a range on one parameter; this is
        // the one place two parameters can each be legal and still be
        // infeasible together.
        // Against the BASE ellipsoid's aspect, not the requested one: the
        // limiter meets the geometry the solid was built from, and
        // `base_aspect` makes that steeper than what the caller asked for.
        let floor = min_slope_limit(base_aspect(self.aspect));
        if self.slope_limit < floor {
            return Err(format!(
                "slope_limit {:.3} is below the floor {:.3} that aspect {:.3} needs \
                 (kappa >= (b^2-1)/(2*b) + 0.40 at the base ellipsoid's own aspect \
                 b = {:.3}, which `base_aspect` pre-stretches from {:.3} so the \
                 PROJECTED outline comes out at the aspect asked for): the \
                 log-Lipschitz limiter would eat the base ellipse and circularise \
                 the blob",
                self.slope_limit,
                floor,
                self.aspect,
                base_aspect(self.aspect),
                self.aspect
            ));
        }
        Ok(())
    }

    /// The whole parameter table: name, type, default, range, effect.
    pub fn describe() -> &'static [ParamInfo] {
        PARAM_TABLE
    }

    /// One parameter's current value, formatted the way [`Params::set`] would
    /// accept it back. `None` for an unknown name.
    ///
    /// Floats go through `Display`, which prints the shortest representation
    /// that round-trips and is implemented in core - not in libm - so it is
    /// identical on every target.
    pub fn value_of(&self, key: &str) -> Option<String> {
        let s = match key {
            "seed" => self.seed.to_string(),
            "tameness" => self.tameness.to_string(),
            "aspect" => self.aspect.to_string(),
            "depth" => self.depth.to_string(),
            "r0" => self.r0.to_string(),
            "margin" => self.margin.to_string(),
            "lump_amp" => self.lump_amp.to_string(),
            "lump_freq" => self.lump_freq.to_string(),
            "lump_octaves" => self.lump_octaves.to_string(),
            "lacunarity" => self.lacunarity.to_string(),
            "gain" => self.gain.to_string(),
            "warp_amt" => self.warp_amt.to_string(),
            "field_freq" => self.field_freq.to_string(),
            "field_angle_range" => self.field_angle_range.to_string(),
            "field_bias" => self.field_bias.to_string(),
            "field_quantize" => match self.field_quantize {
                None => "none".to_string(),
                Some(q) => q.to_string(),
            },
            "slope_limit" => self.slope_limit.to_string(),
            "ring_samples" => self.ring_samples.to_string(),
            "bezier_points" => self.bezier_points.to_string(),
            "cr_tension" => self.cr_tension.to_string(),
            "outline_weight" => self.outline_weight.to_string(),
            "inner_rim" => self.inner_rim.to_string(),
            "relief_lines" => self.relief_lines.to_string(),
            "relief_weight" => self.relief_weight.to_string(),
            "relief_len" => self.relief_len.to_string(),
            "relief_modulation" => self.relief_modulation.to_string(),
            "facet_cuts" => self.facet_cuts.to_string(),
            "facet_relief" => self.facet_relief.to_string(),
            "facet_curviness" => self.facet_curviness.to_string(),
            "facet_min_area" => self.facet_min_area.to_string(),
            "facet_min_chord_sep" => self.facet_min_chord_sep.to_string(),
            "tone_fractions" => format!(
                "{},{},{}",
                self.tone_fractions[0], self.tone_fractions[1], self.tone_fractions[2]
            ),
            "light_dir" => self.light_dir.to_string(),
            "light_elev" => self.light_elev.to_string(),
            "tone_lambert" => self.tone_lambert.to_string(),
            "tone_noise" => self.tone_noise.to_string(),
            "palette_hue" => self.palette_hue.to_string(),
            "palette_sat" => self.palette_sat.to_string(),
            "palette_light" => self.palette_light.to_string(),
            "tone_contrast" => self.tone_contrast.to_string(),
            "ink" => self.ink.hex(),
            "background" => match self.background {
                None => "none".to_string(),
                Some(c) => c.hex(),
            },
            "vesicle_count" => self.vesicle_count.to_string(),
            "vesicle_rmin" => self.vesicle_rmin.to_string(),
            "vesicle_rmax" => self.vesicle_rmax.to_string(),
            "vesicle_size_pow" => self.vesicle_size_pow.to_string(),
            "vesicle_amp" => self.vesicle_amp.to_string(),
            "vesicle_bite" => self.vesicle_bite.to_string(),
            "vesicle_octaves" => self.vesicle_octaves.to_string(),
            "vesicle_aspect_max" => self.vesicle_aspect_max.to_string(),
            "vesicle_align" => self.vesicle_align.to_string(),
            "vesicle_contour" => self.vesicle_contour.to_string(),
            "vesicle_fill" => self.vesicle_fill.to_string(),
            "vesicle_shade" => self.vesicle_shade.to_string(),
            "vesicle_edge_clear" => self.vesicle_edge_clear.to_string(),
            "vesicle_edge_ratio" => self.vesicle_edge_ratio.to_string(),
            "vesicle_gap" => self.vesicle_gap.to_string(),
            "vesicle_attempts" => self.vesicle_attempts.to_string(),
            "attempts_total" => self.attempts_total.to_string(),
            "consec_fail_stop" => self.consec_fail_stop.to_string(),
            "coord_decimals" => self.coord_decimals.to_string(),
            "retry_budget" => self.retry_budget.to_string(),
            _ => return None,
        };
        Some(s)
    }

    /// Every parameter that differs from the default, as `(name, value)`.
    ///
    /// This is what the SVG header comment records, so a generated file can be
    /// regenerated from itself.
    pub fn deltas(&self) -> Vec<(&'static str, String)> {
        let base = Params::default();
        Params::describe()
            .iter()
            .filter_map(|info| {
                let mine = self.value_of(info.name)?;
                let theirs = base.value_of(info.name)?;
                if mine == theirs {
                    None
                } else {
                    Some((info.name, mine))
                }
            })
            .collect()
    }
}

/// Range check for a double.
fn fchk(name: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{name} must be finite, got {v}"));
    }
    if v < lo || v > hi {
        return Err(format!("{name} = {v} is outside {lo} ..= {hi}"));
    }
    Ok(())
}

/// Range check for an unsigned.
fn uchk(name: &str, v: u32, lo: u32, hi: u32) -> Result<(), String> {
    if v < lo || v > hi {
        return Err(format!("{name} = {v} is outside {lo} ..= {hi}"));
    }
    Ok(())
}

/// Shorthand for a table row.
const fn row(
    name: &'static str,
    kind: Kind,
    default: &'static str,
    range: &'static str,
    effect: &'static str,
) -> ParamInfo {
    ParamInfo {
        name,
        kind,
        default,
        range,
        effect,
    }
}

/// SPEC.md's parameter table, in the spec's own order.
static PARAM_TABLE: &[ParamInfo] = &[
    row(
        "seed",
        Kind::U64,
        "0",
        "any u64",
        "The whole image; sequential seeds are unrelated.",
    ),
    row(
        "tameness",
        Kind::F64,
        "0.55",
        "0 ..= 1",
        "0 feral, 1 corporate; drives seven other knobs.",
    ),
    row(
        "depth",
        Kind::F64,
        "0.72",
        "0.35 ..= 1.40",
        "Depth of the solid as a fraction of its vertical semi-axis; 1.0 reads as a ball, 0.4 as a slab.",
    ),
    row(
        "aspect",
        Kind::F64,
        "1.50",
        "1.0 ..= 2.2",
        "Width/height of the base ellipse; 1.0 a pebble, 2.2 a shard.",
    ),
    row(
        "r0",
        Kind::F64,
        "300",
        "canonical",
        "Canonical mean radius; every other length is a fraction of it.",
    ),
    row(
        "margin",
        Kind::F64,
        "0.04",
        "0 ..= 0.15",
        "viewBox padding around the true bbox, as a fraction of R0.",
    ),
    row(
        "lump_amp",
        Kind::F64,
        "0.22",
        "0.05 ..= 0.35",
        "Silhouette chaos: 0.05 an egg, 0.22 the artwork.",
    ),
    row(
        "lump_freq",
        Kind::F64,
        "2.6",
        "1.5 ..= 5.0",
        "How many primary lumps go round; above 4 reads as texture.",
    ),
    row(
        "lump_octaves",
        Kind::U32,
        "4",
        "1 ..= 6",
        "fbm depth for the silhouette; the spectrum supports 4.",
    ),
    row(
        "lacunarity",
        Kind::F64,
        "2.03",
        "1.80 ..= 2.40",
        "Per-octave frequency step; not 2.0, which grows grid ridges.",
    ),
    row(
        "gain",
        Kind::F64,
        "0.45",
        "0.30 ..= 0.60",
        "Per-octave amplitude step; higher is crustier.",
    ),
    row(
        "warp_amt",
        Kind::F64,
        "0.22",
        "0 ..= 0.60",
        "How far the field displaces the noise lookup; lumps lean.",
    ),
    row(
        "field_freq",
        Kind::F64,
        "0.55",
        "0.15 ..= 1.50",
        "Field frequency per R0; 0.55 is one swing across the body.",
    ),
    row(
        "field_angle_range",
        Kind::F64,
        "1.0",
        "0.15 ..= 1.50",
        "Hobbs's lambda; 0.35 is a combed, graphic field.",
    ),
    row(
        "field_bias",
        Kind::F64,
        "0.0",
        "0 ..= 2*PI",
        "Rotates lumps, vesicle axes and cuts together.",
    ),
    row(
        "field_quantize",
        Kind::OptF64,
        "none",
        "none or 0.01 ..= PI",
        "Snap field angles to a multiple; reads crystalline.",
    ),
    row(
        "slope_limit",
        Kind::F64,
        "1.10",
        "0.50 ..= 2.00",
        "kappa in |d(ln r)/d theta|; the anti-cusp guard.",
    ),
    row(
        "ring_samples",
        Kind::U32,
        "720",
        "256 ..= 2048",
        "Samples of r(theta) before the arclength resample.",
    ),
    row(
        "bezier_points",
        Kind::U32,
        "256",
        "96 ..= 512",
        "Emitted cubics on the outer contour; 96 reads hand-drawn.",
    ),
    row(
        "cr_tension",
        Kind::F64,
        "0.50",
        "0.35 ..= 0.60",
        "Catmull-Rom tau; lower is crisper, higher bulges.",
    ),
    row(
        "outline_weight",
        Kind::F64,
        "0.058",
        "0.020 ..= 0.100",
        "Ribbon width over R0; the brand-defining number.",
    ),
    row(
        "inner_rim",
        Kind::F64,
        "0.0",
        "0 ..= 0.030",
        "Optional second thin ribbon; the artist's is 0.0147.",
    ),
    row(
        "relief_lines",
        Kind::U32,
        "1",
        "0 ..= 6",
        "Thin tapered crease marks growing inward out of the outline; the artwork has 1.",
    ),
    row(
        "relief_weight",
        Kind::F64,
        "0.044",
        "0.008 ..= 0.060",
        "Nominal width of a relief mark over R0, 0.76 of the outline. Measured 0.0336, raised so the mark holds its own beside holes this large.",
    ),
    row(
        "relief_len",
        Kind::F64,
        "0.54",
        "0.15 ..= 1.00",
        "Centreline length of a relief mark over R0; measured 0.535.",
    ),
    row(
        "relief_modulation",
        Kind::F64,
        "0.80",
        "0 ..= 1",
        "How much a mark swells and thins along its length with the fold's sharpness; 0 is a monoline.",
    ),
    row(
        "facet_cuts",
        Kind::U32,
        "11",
        "4 ..= 20",
        "Bisections; regions = cuts + 1.",
    ),
    row(
        "facet_relief",
        Kind::F64,
        "0.80",
        "0 ..= 1.0",
        "How far a cut follows the solid's iso-brightness line rather than the flow field.",
    ),
    row(
        "facet_curviness",
        Kind::F64,
        "0.25",
        "0 ..= 1.50",
        "0 a straight chord, 1 a live flow trace.",
    ),
    row(
        "facet_min_area",
        Kind::F64,
        "0.015",
        "0.005 ..= 0.050",
        "Anti-sliver: smallest allowed child, over body area.",
    ),
    row(
        "facet_min_chord_sep",
        Kind::F64,
        "0.08",
        "0.03 ..= 0.20",
        "Anti-wedge: crossing separation over the perimeter.",
    ),
    row(
        "tone_fractions",
        Kind::F64x3,
        "0.30,0.60,0.10",
        "each 0.06 ..= 0.80, sum 1",
        "Area quantiles for shadow / mid / highlight.",
    ),
    row(
        "light_dir",
        Kind::F64,
        "2.30",
        "0 ..= 2*PI",
        "In-plane direction the light comes from; 2.30 rad lights from BELOW LEFT, as measured.",
    ),
    row(
        "light_elev",
        Kind::F64,
        "0.22",
        "0 ..= 1.50",
        "The light's component toward the viewer; small keeps it grazing and the tones spread.",
    ),
    row(
        "tone_lambert",
        Kind::F64,
        "0.85",
        "0 ..= 1.0",
        "How much of a tone is Lambert shading of the real surface normal, against the flat ramp.",
    ),
    row(
        "tone_noise",
        Kind::F64,
        "0.35",
        "0 ..= 1",
        "Ramp versus fbm in the light function.",
    ),
    row(
        "palette_hue",
        Kind::F64,
        "13.0",
        "0 ..= 360",
        "Shadow hue in degrees; moves the whole family.",
    ),
    row(
        "palette_sat",
        Kind::F64,
        "1.00",
        "0.50 ..= 1.00",
        "HSL saturation for all three tones.",
    ),
    row(
        "palette_light",
        Kind::F64,
        "0.565",
        "0.30 ..= 0.75",
        "HSL lightness of the MID tone.",
    ),
    row(
        "tone_contrast",
        Kind::F64,
        "1.00",
        "0.40 ..= 2.00",
        "Scales both deltas off the mid tone.",
    ),
    row(
        "ink",
        Kind::Color,
        "#231f20",
        "any",
        "Near-black for the ribbon and the vesicles.",
    ),
    row(
        "background",
        Kind::OptColor,
        "none",
        "none or any",
        "None emits no rect, which is what a logo wants.",
    ),
    row(
        "vesicle_count",
        Kind::U32,
        "5",
        "0 ..= 20",
        "Target gas holes; the packer may place fewer.",
    ),
    row(
        "vesicle_rmin",
        Kind::F64,
        "0.090",
        "0.030 ..= 0.120",
        "Smallest vesicle radius over R0.",
    ),
    row(
        "vesicle_rmax",
        Kind::F64,
        "0.300",
        "0.100 ..= 0.400",
        "Largest vesicle radius over R0.",
    ),
    row(
        "vesicle_size_pow",
        Kind::U32,
        "1",
        "1 ..= 3",
        "Exponent in the u^pow size law; integer, so no powf.",
    ),
    row(
        "vesicle_amp",
        Kind::F64,
        "0.30",
        "0.10 ..= 0.50",
        "Vesicle outline lumpiness; scaled by size inside.",
    ),
    row(
        "vesicle_bite",
        Kind::F64,
        "0.60",
        "0 ..= 0.70",
        "Concave bite carved into the big holes; 0 leaves potatoes.",
    ),
    row(
        "vesicle_octaves",
        Kind::U32,
        "3",
        "2 ..= 5",
        "fbm depth per vesicle; small ones use one less.",
    ),
    row(
        "vesicle_aspect_max",
        Kind::F64,
        "2.40",
        "1.0 ..= 3.2",
        "Elongation of the largest vesicle.",
    ),
    row(
        "vesicle_align",
        Kind::F64,
        "0.85",
        "0 ..= 1",
        "How strongly a vesicle's long axis follows the field.",
    ),
    row(
        "vesicle_contour",
        Kind::F64,
        "0.85",
        "0 ..= 1",
        "How strongly a hole follows the surface under it: axis to the local contour, foreshortened by the tilt.",
    ),
    row(
        "vesicle_fill",
        Kind::F64,
        "1.00",
        "0.30 ..= 1.00",
        "Caps a hole at this much of its Voronoi territory; 1.0 lifts the cap and lets it take its drawn size.",
    ),
    row(
        "vesicle_shade",
        Kind::F64,
        "0.35",
        "0 ..= 1",
        "How strongly a hole's size follows the shading, so the holes darken the shadow side.",
    ),
    row(
        "vesicle_edge_clear",
        Kind::F64,
        "0.055",
        "0.030 ..= 0.120",
        "Gap to the INNER boundary; floored at 1.2*outline_weight.",
    ),
    row(
        "vesicle_edge_ratio",
        Kind::F64,
        "0.80",
        "0.000 ..= 1.500",
        "Gap to the inner boundary as a multiple of the hole's own radius.",
    ),
    row(
        "vesicle_gap",
        Kind::F64,
        "0.090",
        "0.040 ..= 0.200",
        "Vesicle-to-vesicle gap; floored at 0.45*min radius.",
    ),
    row(
        "vesicle_attempts",
        Kind::U32,
        "220",
        "50 ..= 1000",
        "Placement tries per vesicle before dropping it.",
    ),
    row(
        "attempts_total",
        Kind::U32,
        "2000",
        "500 ..= 20000",
        "Global placement budget.",
    ),
    row(
        "consec_fail_stop",
        Kind::U32,
        "150",
        "40 ..= 600",
        "Stop packing after this many consecutive failures.",
    ),
    row(
        "coord_decimals",
        Kind::U32,
        "3",
        "2 ..= 4",
        "Decimal places in emitted coordinates.",
    ),
    row(
        "retry_budget",
        Kind::U32,
        "20",
        "1 ..= 100",
        "Whole-logo re-rolls before returning Err.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate() {
        Params::default()
            .validate()
            .expect("the measured defaults must be feasible");
    }

    #[test]
    fn every_preset_validates() {
        for name in Params::PRESETS {
            let p = Params::preset(name).expect("listed preset must exist");
            p.validate()
                .unwrap_or_else(|e| panic!("preset {name}: {e}"));
        }
    }

    /// The trap the interlock exists for: a kappa that is legal on its own but
    /// cannot carry the ellipse.
    #[test]
    fn interlock_rejects_a_kappa_below_the_ellipse_slope() {
        let mut p = Params {
            aspect: 2.0,
            ..Params::default()
        };
        // The floor is set by the BASE ellipsoid, which `base_aspect`
        // stretches past the requested aspect, so it is higher than the
        // naive (4-1)/4 + 0.40 = 1.15 that aspect 2.0 would suggest.
        let floor = min_slope_limit(base_aspect(2.0));
        assert!(
            floor > min_slope_limit(2.0),
            "the base is the stretched one"
        );
        p.slope_limit = 0.90;
        let e = p.validate().expect_err("must be rejected");
        assert!(e.contains("slope_limit"), "{e}");
        assert!(e.contains("circularise"), "the error must say WHY: {e}");
        assert!(
            e.contains("base_aspect"),
            "the error must point at the pre-stretch: {e}"
        );
        p.slope_limit = floor;
        p.validate().expect("exactly at the floor is feasible");
    }

    /// The tame end of the dial is below the interlock at the default aspect;
    /// the dial must not be able to produce an invalid parameter set.
    #[test]
    fn tameness_never_breaks_the_interlock() {
        for i in 0..=20 {
            let mut p = Params {
                tameness: i as f64 / 20.0,
                ..Params::default()
            };
            p.apply_tameness();
            p.validate()
                .unwrap_or_else(|e| panic!("tameness {}: {e}", p.tameness));
        }
    }

    #[test]
    fn tameness_moves_the_seven_and_only_the_seven() {
        let mut feral = Params {
            tameness: 0.0,
            ..Params::default()
        };
        feral.apply_tameness();
        let mut tame = Params {
            tameness: 1.0,
            ..Params::default()
        };
        tame.apply_tameness();
        assert!(feral.lump_amp > tame.lump_amp);
        assert!(feral.lump_octaves > tame.lump_octaves);
        assert!(feral.warp_amt > tame.warp_amt);
        assert!(feral.vesicle_count > tame.vesicle_count);
        assert!(feral.vesicle_amp > tame.vesicle_amp);
        assert!(feral.facet_curviness > tame.facet_curviness);
        // untouched
        assert_eq!(feral.lump_freq, tame.lump_freq);
        assert_eq!(feral.outline_weight, tame.outline_weight);
    }

    #[test]
    fn explicit_knobs_survive_the_dial() {
        let mut p = Params::default();
        p.set("lump_amp", "0.33").expect("settable");
        p.tameness = 1.0;
        p.apply_tameness();
        assert_eq!(p.lump_amp, 0.33, "an explicitly set knob is off limits");
        assert!(p.warp_amt < 0.22, "the rest still moved");
    }

    #[test]
    fn the_described_default_is_the_actual_default() {
        // The table's `default` column is hand-written prose, and
        // `--list-params` and the README both copy it. It drifted: three rows
        // still advertised SPEC.md's figures (facet_curviness 1.00,
        // vesicle_rmax 0.260, vesicle_size_pow 2) after `Params::default()`
        // had moved off them, so the CLI documented knobs it was not using.
        // Comparison is numeric where the kind is numeric, because "1.50" and
        // "1.5" are the same default written two ways.
        let d = Params::default();
        for info in Params::describe() {
            let actual = d
                .value_of(info.name)
                .unwrap_or_else(|| panic!("{}: value_of has no case for it", info.name));
            let same = match info.kind {
                Kind::U64 | Kind::U32 | Kind::F64 | Kind::F64x3 => {
                    // Numerically, so "0.60" and "0.6" are the same default
                    // written two ways; `F64x3` is the same, comma-wise.
                    let nums = |t: &str| -> Option<Vec<f64>> {
                        t.split(',').map(|x| x.trim().parse::<f64>().ok()).collect()
                    };
                    match (nums(info.default), nums(&actual)) {
                        (Some(a), Some(b)) => a == b,
                        _ => info.default == actual,
                    }
                }
                _ => info.default.eq_ignore_ascii_case(&actual),
            };
            assert!(
                same,
                "{}: the table says `{}` but Params::default() is `{}`",
                info.name, info.default, actual
            );
        }
    }

    #[test]
    fn set_covers_every_described_parameter() {
        // Each row of the table must be reachable by name, or --list-params
        // advertises a knob --set cannot touch.
        for info in Params::describe() {
            let mut p = Params::default();
            let v = match info.kind {
                Kind::U64 | Kind::U32 => "3",
                Kind::F64 => "0.5",
                Kind::OptF64 => "none",
                Kind::F64x3 => "0.3,0.6,0.1",
                Kind::Color => "#112233",
                Kind::OptColor => "none",
            };
            p.set(info.name, v)
                .unwrap_or_else(|e| panic!("{}: {e}", info.name));
        }
        assert!(Params::default().set("nope", "1").is_err());
    }

    #[test]
    fn tone_fractions_must_sum_to_one() {
        let mut p = Params::default();
        p.set("tone_fractions", "0.3,0.3,0.3").expect("parses");
        assert!(p.validate().is_err());
        p.set("tone_fractions", "0.3,0.6,0.1").expect("parses");
        p.validate().expect("sums to 1");
    }

    #[test]
    fn value_of_round_trips_through_set() {
        let mut p = Params::default();
        p.set("aspect", "1.75").expect("settable");
        p.set("background", "#101010").expect("settable");
        p.set("field_quantize", "0.5235987755982988")
            .expect("settable");
        let mut q = Params::default();
        for (k, v) in p.deltas() {
            q.set(k, &v).unwrap_or_else(|e| panic!("{k}={v}: {e}"));
        }
        assert_eq!(q.aspect, p.aspect);
        assert_eq!(q.background, p.background);
        assert_eq!(q.field_quantize, p.field_quantize);
    }

    #[test]
    fn defaults_have_no_deltas() {
        assert!(Params::default().deltas().is_empty());
    }

    #[test]
    fn anya_is_the_artwork() {
        let p = Params::anya();
        assert_eq!(p.margin, 0.0, "the drawing bleeds to the canvas edge");
        // The file carries a second rim; the DRAWING does not show it -
        // removing it changes no rendered pixel - so the preset leaves it off.
        assert_eq!(p.inner_rim, 0.0, "the drawing shows no second rim");
        p.validate().expect("feasible");
    }
}
