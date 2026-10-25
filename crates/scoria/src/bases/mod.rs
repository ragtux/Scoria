//! The Bases view: draws a `.base` file.
//!
//! `view` owns the element — the header row, the rows, scrolling, the empty
//! and error states — over the table the `scoria-bases` crate resolves.
//! `rows` is the bridge: it turns the vault's own index and cache into the
//! `FileData` values that crate evaluates against, so nothing below this
//! module knows what a vault is.
//!
//! The view is **read-only** in this first cut: a `.base` is drawn, never
//! written. Obsidian writes one on a toolbar change or a view close, and
//! `scoria_formats::base` already serializes those bytes; nothing here calls
//! it yet.

pub mod rows;
pub mod view;

pub use view::{BasesEvent, BasesView};
