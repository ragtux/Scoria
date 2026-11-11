//! Exports that draw without gpui: `workspace:export-pdf`.
//!
//! [`doc`] is what the app hands over — a note's blocks, built from the
//! reading view's model, and the colours to paint them in. [`layout`]
//! measures, breaks and paginates them, and [`pdf`] writes the pages with
//! krilla. The faces are [`fonts`]: the app's Inter, KaTeX's faces for
//! math and its Typewriter for code.

pub mod doc;
pub mod fonts;
pub mod layout;
pub mod pdf;

pub use doc::{Align, Block, Color, Inline, ListItem, Marker, Palette, SpanStyle};
pub use fonts::{Face, FontFiles, Fonts};
pub use pdf::{export_pdf, paginate, Margin, PageSize, PdfSettings};

#[cfg(test)]
mod tests;
