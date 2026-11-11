//! Exports that draw without gpui: `workspace:export-pdf` and
//! `canvas:export-as-image`.
//!
//! [`doc`] is what the app hands over — a note's blocks, built from the
//! reading view's model, and the colours to paint them in. [`layout`]
//! measures, breaks and paginates them; [`pdf`] writes the pages with
//! krilla, and [`canvas`] draws a canvas with tiny-skia, its cards' text
//! through the same layout. Both draw from the same faces ([`fonts`]): the
//! app's Inter, KaTeX's faces for math and its Typewriter for code.

pub mod canvas;
pub mod doc;
pub mod fonts;
pub mod layout;
pub mod pdf;
mod raster;

pub use canvas::{
    estimate_text, estimated_size, export_image, render_image, Area, CanvasImage, CanvasPalette,
    CardContent, ImageOptions,
};
pub use doc::{Align, Block, Color, Inline, ListItem, Marker, Palette, SpanStyle};
pub use fonts::{Face, FontFiles, Fonts};
pub use pdf::{export_pdf, paginate, Margin, PageSize, PdfSettings};

#[cfg(test)]
mod canvas_tests;
#[cfg(test)]
mod tests;
