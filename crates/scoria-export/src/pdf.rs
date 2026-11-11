//! Export to PDF: the modal's five settings, the page geometry they make,
//! and the pages written with krilla.
//!
//! Obsidian prints its reading view through Chromium
//! ([[Specification#Export to PDF]]). Scoria lays the same blocks out
//! itself ([`crate::layout`]) and writes the result: the page sizes are
//! Chromium's, in points, measured on the probe's files; the text's left
//! edge sits where the probe measured it, 51.75 pt in with the Default
//! margin and 24 pt with Minimal and None; Downscale percent scales the
//! content inside the margins, as Chromium's print scale does.

use crate::doc::{Block, Color, Palette};
use crate::fonts::{Face, Fonts};
use crate::layout::{layout, Item, Page, PathOp};
use krilla::action::LinkAction;
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::Document;
use std::collections::HashMap;

/// The Page size dropdown, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    A3,
    A4,
    A5,
    Legal,
    Letter,
    Tabloid,
}

impl PageSize {
    pub const ALL: [PageSize; 6] = [
        PageSize::A3,
        PageSize::A4,
        PageSize::A5,
        PageSize::Legal,
        PageSize::Letter,
        PageSize::Tabloid,
    ];

    /// The name the dropdown shows and `pdfExportSettings.pageSize` holds.
    pub fn name(self) -> &'static str {
        match self {
            PageSize::A3 => "A3",
            PageSize::A4 => "A4",
            PageSize::A5 => "A5",
            PageSize::Legal => "Legal",
            PageSize::Letter => "Letter",
            PageSize::Tabloid => "Tabloid",
        }
    }

    pub fn parse(name: &str) -> Option<PageSize> {
        PageSize::ALL.into_iter().find(|p| p.name() == name)
    }

    /// Portrait width and height in points, as Chromium wrote them.
    pub fn points(self) -> (f32, f32) {
        match self {
            PageSize::A3 => (842.88, 1191.12),
            PageSize::A4 => (595.92, 842.88),
            PageSize::A5 => (420.0, 595.92),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Tabloid => (792.0, 1224.0),
        }
    }
}

/// The Margin dropdown: values `0`, `2`, `1` in that order, as Obsidian
/// lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Margin {
    Default,
    Minimal,
    None,
}

impl Margin {
    pub const ALL: [Margin; 3] = [Margin::Default, Margin::Minimal, Margin::None];

    /// The string `pdfExportSettings.margin` holds.
    pub fn value(self) -> &'static str {
        match self {
            Margin::Default => "0",
            Margin::None => "1",
            Margin::Minimal => "2",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Margin::Default => "Default",
            Margin::Minimal => "Minimal",
            Margin::None => "None",
        }
    }

    pub fn parse(value: &str) -> Option<Margin> {
        Margin::ALL.into_iter().find(|m| m.value() == value)
    }

    /// The page's own margin in points: what lies between the paper's edge
    /// and the printed reading view, whose padding is 24 pt more.
    pub fn points(self) -> f32 {
        match self {
            Margin::Default => 27.75,
            Margin::Minimal | Margin::None => 0.0,
        }
    }
}

/// The reading view's padding inside the page margin, points.
pub const PADDING: f32 = 24.0;

/// The modal's five rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfSettings {
    pub include_name: bool,
    pub page_size: PageSize,
    pub landscape: bool,
    pub margin: Margin,
    /// 10 to 100.
    pub downscale_percent: u8,
}

impl Default for PdfSettings {
    /// Obsidian's defaults (observed).
    fn default() -> PdfSettings {
        PdfSettings {
            include_name: false,
            page_size: PageSize::Letter,
            landscape: false,
            margin: Margin::Default,
            downscale_percent: 100,
        }
    }
}

/// Where the content goes on a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// The page, points.
    pub width: f32,
    pub height: f32,
    /// The content box's top left corner, points.
    pub origin: (f32, f32),
    /// Points per CSS pixel of layout: 0.75 times the downscale.
    pub scale: f32,
    /// The content box in layout pixels.
    pub content: (f32, f32),
}

impl PdfSettings {
    pub fn geometry(&self) -> Geometry {
        let (mut w, mut h) = self.page_size.points();
        if self.landscape {
            std::mem::swap(&mut w, &mut h);
        }
        let inset = self.margin.points() + PADDING;
        let scale = 0.75 * f32::from(self.downscale_percent.clamp(10, 100)) / 100.0;
        Geometry {
            width: w,
            height: h,
            origin: (inset, inset),
            scale,
            content: ((w - 2.0 * inset) / scale, (h - 2.0 * inset) / scale),
        }
    }
}

/// The pages `blocks` make under `settings`, before they are written.
pub fn paginate(
    name: &str,
    blocks: &[Block],
    settings: &PdfSettings,
    fonts: &Fonts,
    palette: &Palette,
) -> (Geometry, Vec<Page>) {
    let geometry = settings.geometry();
    let title = settings.include_name.then_some(name);
    let (pages, _) = layout(
        blocks,
        title,
        geometry.content.0,
        Some(geometry.content.1),
        fonts,
        palette,
    );
    (geometry, pages)
}

/// The note as a PDF: `name` is its basename, the document's title.
pub fn export_pdf(
    name: &str,
    blocks: &[Block],
    settings: &PdfSettings,
    fonts: &Fonts,
    palette: &Palette,
) -> Result<Vec<u8>, String> {
    let (geometry, pages) = paginate(name, blocks, settings, fonts, palette);
    write_pdf(name, &geometry, &pages, fonts, palette)
}

/// Writes laid-out pages.
pub fn write_pdf(
    name: &str,
    geometry: &Geometry,
    pages: &[Page],
    fonts: &Fonts,
    palette: &Palette,
) -> Result<Vec<u8>, String> {
    let mut document = Document::new();
    document.set_metadata(
        Metadata::new()
            .title(name.to_string())
            .creator("Scoria".to_string())
            .producer(format!("Scoria {}", env!("CARGO_PKG_VERSION"))),
    );
    let mut cache: HashMap<Face, Option<Font>> = HashMap::new();
    let mut images: HashMap<usize, Option<Image>> = HashMap::new();
    let k = geometry.scale;
    let (ox, oy) = geometry.origin;
    for page in pages {
        let settings =
            PageSettings::from_wh(geometry.width, geometry.height).ok_or("the page has no size")?;
        let mut pdf_page = document.start_page_with(settings);
        let mut links = Vec::new();
        {
            let mut surface = pdf_page.surface();
            // The page is white, as Chromium prints it, whatever the theme.
            if palette.background.a > 0 {
                fill(&mut surface, palette.background);
                if let Some(path) = rect_path(0.0, 0.0, geometry.width, geometry.height, 0.0) {
                    surface.draw_path(&path);
                }
            }
            surface.push_transform(&Transform::from_row(k, 0.0, 0.0, k, ox, oy));
            for item in &page.items {
                match item {
                    Item::Link { x, y, w, h, url } => {
                        links.push((
                            Rect::from_xywh(ox + x * k, oy + y * k, w * k, h * k),
                            url.clone(),
                        ));
                    }
                    other => draw(&mut surface, other, fonts, &mut cache, &mut images),
                }
            }
            surface.pop();
        }
        for (rect, url) in links {
            if let Some(rect) = rect {
                pdf_page.add_annotation(Annotation::new_link(
                    LinkAnnotation::new(rect, Target::Action(LinkAction::new(url).into())),
                    None,
                ));
            }
        }
        pdf_page.finish();
    }
    document.finish().map_err(|e| format!("{e:?}"))
}

fn paint(color: Color) -> (rgb::Color, NormalizedF32) {
    (
        rgb::Color::new(color.r, color.g, color.b),
        NormalizedF32::new(f32::from(color.a) / 255.0).unwrap_or(NormalizedF32::ONE),
    )
}

fn fill(surface: &mut Surface<'_>, color: Color) {
    let (c, opacity) = paint(color);
    surface.set_stroke(None);
    surface.set_fill(Some(Fill {
        paint: c.into(),
        opacity,
        rule: FillRule::NonZero,
    }));
}

fn stroke(surface: &mut Surface<'_>, color: Color, width: f32) {
    let (c, opacity) = paint(color);
    surface.set_fill(None);
    surface.set_stroke(Some(Stroke {
        paint: c.into(),
        width,
        opacity,
        ..Stroke::default()
    }));
}

/// A rectangle, its corners rounded by `r`.
fn rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<krilla::geom::Path> {
    let mut pb = PathBuilder::new();
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.0 {
        pb.push_rect(Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))?);
        return pb.finish();
    }
    // A quarter circle as one cubic: the control points 0.5523 of the
    // radius along the tangents.
    let c = r * 0.552_284_8;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + c, y, x + w, y + r - c, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + c, x + w - r + c, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - c, y + h, x, y + h - r + c, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - c, x + r - c, y, x + r, y);
    pb.close();
    pb.finish()
}

fn ops_path(ops: &[PathOp]) -> Option<krilla::geom::Path> {
    let mut pb = PathBuilder::new();
    for op in ops {
        match *op {
            PathOp::Move(x, y) => pb.move_to(x, y),
            PathOp::Line(x, y) => pb.line_to(x, y),
            PathOp::Quad(a, b, x, y) => pb.quad_to(a, b, x, y),
            PathOp::Cubic(a, b, c, d, x, y) => pb.cubic_to(a, b, c, d, x, y),
            PathOp::Close => pb.close(),
        }
    }
    pb.finish()
}

/// An image's bytes as krilla takes them: PNG, JPEG, GIF and WebP as they
/// are, anything else `image` can read as RGBA.
fn pdf_image(data: &[u8]) -> Option<Image> {
    let owned = data.to_vec();
    let made = match image::guess_format(data).ok()? {
        image::ImageFormat::Png => Image::from_png(owned.into(), true),
        image::ImageFormat::Jpeg => Image::from_jpeg(owned.into(), true),
        image::ImageFormat::Gif => Image::from_gif(owned.into(), true),
        image::ImageFormat::WebP => Image::from_webp(owned.into(), true),
        _ => {
            let rgba = image::load_from_memory(data).ok()?.to_rgba8();
            let (w, h) = rgba.dimensions();
            return Some(Image::from_rgba8(rgba.into_raw(), w, h));
        }
    };
    made.ok()
}

fn draw(
    surface: &mut Surface<'_>,
    item: &Item,
    fonts: &Fonts,
    cache: &mut HashMap<Face, Option<Font>>,
    images: &mut HashMap<usize, Option<Image>>,
) {
    match item {
        Item::Text(run) => {
            let font = cache
                .entry(run.face)
                .or_insert_with(|| {
                    fonts
                        .bytes(run.face)
                        .and_then(|bytes| Font::new(bytes.into(), 0))
                })
                .clone();
            let Some(font) = font else {
                return;
            };
            if run.size <= 0.0 {
                return;
            }
            let glyphs: Vec<KrillaGlyph> = run
                .glyphs
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    // The pen moves by the gap to the next glyph, which is
                    // the advance plus any kerning the layout applied.
                    let next = run.glyphs.get(i + 1).map_or(g.x + g.advance, |n| n.x);
                    KrillaGlyph {
                        glyph_id: GlyphId::new(u32::from(g.id)),
                        text_range: g.range.clone(),
                        x_advance: (next - g.x) / run.size,
                        x_offset: 0.0,
                        y_offset: 0.0,
                        y_advance: 0.0,
                        location: None,
                    }
                })
                .collect();
            fill(surface, run.color);
            surface.draw_glyphs(
                Point::from_xy(run.x + run.glyphs.first().map_or(0.0, |g| g.x), run.y),
                &glyphs,
                font,
                &run.text,
                run.size,
                false,
            );
        }
        Item::Rect {
            x,
            y,
            w,
            h,
            color,
            radius,
        } => {
            fill(surface, *color);
            if let Some(path) = rect_path(*x, *y, *w, *h, *radius) {
                surface.draw_path(&path);
            }
        }
        Item::Frame {
            x,
            y,
            w,
            h,
            color,
            radius,
            width,
        } => {
            stroke(surface, *color, *width);
            let half = width / 2.0;
            if let Some(path) = rect_path(x + half, y + half, w - width, h - width, *radius) {
                surface.draw_path(&path);
            }
        }
        Item::Line {
            x1,
            y1,
            x2,
            y2,
            width,
            color,
        } => {
            stroke(surface, *color, *width);
            let mut pb = PathBuilder::new();
            pb.move_to(*x1, *y1);
            pb.line_to(*x2, *y2);
            if let Some(path) = pb.finish() {
                surface.draw_path(&path);
            }
        }
        Item::Path {
            ops,
            fill: filled,
            stroke: width,
            color,
        } => {
            if *filled {
                fill(surface, *color);
            } else {
                stroke(surface, *color, *width);
            }
            if let Some(path) = ops_path(ops) {
                surface.draw_path(&path);
            }
        }
        Item::Image { x, y, w, h, data } => {
            let key = std::sync::Arc::as_ptr(data) as usize;
            let image = images.entry(key).or_insert_with(|| pdf_image(data)).clone();
            if let (Some(image), Some(size)) = (image, Size::from_wh(*w, *h)) {
                surface.push_transform(&Transform::from_translate(*x, *y));
                surface.draw_image(image, size);
                surface.pop();
            }
        }
        Item::Link { .. } => {}
    }
    surface.set_fill(None);
    surface.set_stroke(None);
}
