//! Procedural scoria: a generator for the logo mark Anya drew by hand.
//!
//! The target is `docs/img/anya_v0_base.svg` — a lumpy volcanic chunk with a
//! heavy dark outline, flat three-tone orange faceting and a handful of gas
//! holes. Everything here is built to reproduce that visual language from a
//! seed, in the spirit of Tyler Hobbs's Fidenza: one flow field drives the
//! silhouette's lean, the vesicles' long axes and the facet cuts, so a single
//! seed reads as one rock rather than three unrelated effects.
//!
//! Pipeline, in paint order:
//!
//! ```text
//!   params ──► field ──► silhouette ring ──► inner ring (r − w)
//!                │              │
//!                │              ├──► facets   (recursive flow-field bisection, 3 tones)
//!                ├──────────────┤
//!                │              ├──► vesicles (Fidenza collision packing)
//!                │              │
//!                └──────────────┴──► outline ribbon (outer + reversed inner, even-odd)
//! ```
//!
//! The silhouette is a *polar* function `r(θ)`, never an advected point cloud.
//! That one choice buys the two guarantees the rest of the crate leans on: a
//! single-valued `r(θ) > 0` is star-shaped about its centre and therefore
//! cannot self-intersect, and the outline ribbon is then a scalar subtraction
//! `r − w(θ)` rather than a general polygon offset.
//!
//! Determinism is a hard requirement: same seed and params give a byte-identical
//! SVG on every platform. Nothing in this crate calls `sin`, `cos`, `atan2`,
//! `powf` or `hypot` from libm, and nothing iterates a `HashMap`.

#![forbid(unsafe_code)]

pub mod bezier;
pub mod body;
pub mod facet;
pub mod field;
pub mod math;
pub mod noise;
pub mod palette;
pub mod params;
pub mod quality;
pub mod relief;
pub mod ring;
pub mod rng;
pub mod solid;
pub mod svg;
pub mod vesicle;

pub use params::Params;

/// Why a logo could not be generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenError {
    /// A parameter is out of range or two parameters are mutually infeasible.
    /// The string names the parameter and the constraint it broke.
    Params(String),
    /// Every re-roll in `retry_budget` failed the same quality gate. The string
    /// names the gate, which is how a caller learns their parameter
    /// combination cannot produce a clean logo.
    Exhausted { rule: &'static str, retries: u32 },
}

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenError::Params(m) => write!(f, "invalid parameters: {m}"),
            GenError::Exhausted { rule, retries } => {
                write!(
                    f,
                    "no clean logo after {retries} re-rolls: {rule} kept failing"
                )
            }
        }
    }
}

impl std::error::Error for GenError {}

/// Generate one scoria as a complete SVG document.
///
/// Re-rolls internally — advancing the seed through `splitmix64` — until every
/// quality gate in [`quality`] passes or `params.retry_budget` is spent.
pub fn generate(params: &Params) -> Result<String, GenError> {
    params.validate().map_err(GenError::Params)?;
    body::generate_checked(params)
}
