//! Export as image: a canvas rasterised to a PNG.
//!
//! What the probe settled ([[Specification#Export as image]]): Full canvas
//! is the nodes' bounds plus 100 px on every side at zoom 1, RGBA, in the
//! canvas's own background with its dot grid, edges, labels and the
//! groups' colours; Show logo adds an 80 px strip below with the product's
//! mark at the bottom left; Viewport only is the pane's visible area, RGB
//! without alpha; Privacy mode draws each run of text as a rounded bar of
//! its colour. The Zoom slider's value `z` scales the image by `2^z`.
//!
//! The pinned gpui cannot paint off screen, so the canvas is drawn here
//! from its model with the view's own geometry (`scoria-canvas`) and the
//! same colours the view paints with, which the app hands in. The mark is
//! Scoria's (`docs/img/marks/clast-orange.svg`, as the app bundles it),
//! never Obsidian's.

use crate::doc::{Block, Color, Palette};
use crate::fonts::{Face, Fonts};
use crate::layout::{label_run, layout, Item};
use crate::raster::{encode_png, paint, rounded_rect, Raster};
use scoria_canvas::{
    anchor, arrow_head, edge_ends, edge_path, edge_stub, facing_side, side_normal, Color as CColor,
    Point, Rect, Viewport, ARROW, BORDER_WIDTH, CARD_RADIUS, EDGE_WIDTH, GRID,
};
use scoria_formats::canvas::{Canvas, NodeType, Side};
use std::collections::HashMap;
use std::sync::Arc;
use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Stroke, Transform};

/// The margin around the nodes' bounds, canvas units.
pub const MARGIN: f64 = 100.0;
/// The logo strip's height at zoom 1.
pub const LOGO_STRIP: f64 = 80.0;
/// The longest side an export may have, pixels.
pub const MAX_SIDE: f64 = 16384.0;
/// A card's content padding, canvas units (the view's `CARD_PADDING`).
const CARD_PADDING: f32 = 16.0;
const CARD_LABEL_SIZE: f64 = 15.0;
const LABEL_LINE_HEIGHT: f64 = 1.3;
const LABEL_GAP: f64 = 4.0;
const LABEL_RADIUS: f64 = 4.0;
const GROUP_LABEL_SIZE: f64 = 22.5;
const GROUP_LABEL_PADDING: (f64, f64) = (8.0, 4.0);
const EDGE_LABEL_SIZE: f64 = 20.0;
const EDGE_LABEL_LINE: f64 = 26.0;
const EDGE_LABEL_PADDING: f64 = 6.0;
const MIN_DOT_SPACING: f64 = 14.142_135_623_730_951;
const DOT_RADIUS: f64 = 0.7;

/// The canvas view's colours, as the app's theme has them.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPalette {
    pub background: Color,
    pub dot: Color,
    /// A card's ground: `--background-primary`.
    pub card: Color,
    /// An uncoloured card's border and edge colour.
    pub card_border: Color,
    /// The name above a file card.
    pub label: Color,
    pub text: Color,
    pub muted: Color,
    /// Text on a coloured group's label.
    pub on_color: Color,
    /// `"1"` .. `"6"`.
    pub presets: [Color; 6],
    pub border_alpha: f32,
    pub tint_alpha: f32,
    pub wash_alpha: f32,
}

impl CanvasPalette {
    fn color_of(&self, color: Option<&str>) -> Option<Color> {
        match CColor::parse(color) {
            CColor::Default => None,
            CColor::Preset(n) => self.presets.get(usize::from(n) - 1).copied(),
            CColor::Hex(v) => Some(Color::rgb(v)),
        }
    }
}

/// What a card shows, which only the app can work out: a text card's or a
/// note's blocks (built by the reading view's model), an image file's
/// bytes, or a line of text (a link's address, a file Scoria cannot draw).
#[derive(Debug, Clone, PartialEq)]
pub enum CardContent {
    Blocks(Vec<Block>),
    Image(Arc<Vec<u8>>),
    Text(String),
}

/// The modal's Viewport row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Area {
    /// "Full canvas".
    Full,
    /// "Viewport only": the view's position and its size in pixels.
    Viewport {
        viewport: Viewport,
        size: (f64, f64),
    },
}

/// The modal's four rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageOptions {
    pub area: Area,
    /// The Zoom slider, -1 to 1: the image is `2^zoom` times its size.
    pub zoom: f64,
    pub show_logo: bool,
    pub privacy: bool,
}

impl Default for ImageOptions {
    fn default() -> ImageOptions {
        ImageOptions {
            area: Area::Full,
            zoom: 0.0,
            show_logo: true,
            privacy: false,
        }
    }
}

/// What gets drawn: the canvas rectangle, the view scale it is drawn at
/// (labels and edges follow its square root, as on screen) and the pixels
/// per screen pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    area: Rect,
    view_scale: f64,
    ratio: f64,
}

fn frame(canvas: &Canvas, options: &ImageOptions) -> Frame {
    let mut ratio = 2f64.powf(options.zoom.clamp(-1.0, 1.0));
    let (area, view_scale) = match options.area {
        Area::Full => {
            let bounds = scoria_canvas::bounds(&canvas.nodes).unwrap_or_default();
            (bounds.inflate(MARGIN), 1.0)
        }
        Area::Viewport { viewport, size } => (viewport.visible(size), viewport.scale()),
    };
    let longest = area.width.max(area.height) * view_scale * ratio;
    if longest > MAX_SIDE {
        ratio *= MAX_SIDE / longest;
    }
    Frame {
        area,
        view_scale,
        ratio,
    }
}

/// The image's size without the logo strip: the modal's "Estimated image
/// dimensions".
pub fn estimated_size(canvas: &Canvas, options: &ImageOptions) -> (u32, u32) {
    let f = frame(canvas, options);
    let s = f.view_scale * f.ratio;
    (
        (f.area.width * s).round().max(1.0) as u32,
        (f.area.height * s).round().max(1.0) as u32,
    )
}

/// The modal's estimate line, `Estimated image dimensions: 1,050px x 700px`.
pub fn estimate_text(size: (u32, u32)) -> String {
    fn grouped(n: u32) -> String {
        let digits = n.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
        }
        out
    }
    format!(
        "Estimated image dimensions: {}px x {}px",
        grouped(size.0),
        grouped(size.1)
    )
}

/// Everything the image is drawn from.
pub struct CanvasImage<'a> {
    pub canvas: &'a Canvas,
    /// By node id.
    pub contents: &'a HashMap<String, CardContent>,
    pub palette: &'a CanvasPalette,
    /// The colours a card's text is set in.
    pub text: &'a Palette,
    pub fonts: &'a Fonts,
    /// The mark the logo strip shows, an SVG.
    pub logo: &'a [u8],
}

/// Draws the canvas and returns the pixels.
pub fn render_image(image: &CanvasImage<'_>, options: &ImageOptions) -> Result<Pixmap, String> {
    let f = frame(image.canvas, options);
    let (w, h) = estimated_size(image.canvas, options);
    let strip = if options.show_logo {
        (LOGO_STRIP * f.ratio).round() as u32
    } else {
        0
    };
    let pixmap = Pixmap::new(w, h + strip).ok_or("the image has no size")?;
    let mut r = Raster::new(pixmap, image.fonts);
    let p = image.palette;
    r.pixmap.fill(tiny_skia::Color::from_rgba8(
        p.background.r,
        p.background.g,
        p.background.b,
        p.background.a,
    ));
    let s = (f.view_scale * f.ratio) as f32;
    // Canvas units to pixels.
    let base = Transform::from_row(
        s,
        0.0,
        0.0,
        s,
        (-f.area.x * f64::from(s)) as f32,
        (-f.area.y * f64::from(s)) as f32,
    );
    let ctx = Ctx {
        image,
        f,
        base,
        s,
        // A label's or an edge's screen size follows the view scale's
        // square root, in pixels of the export.
        ls: (f.view_scale.sqrt() * f.ratio) as f32,
    };
    let mut clip = Mask::new(w, h + strip).ok_or("the image has no size")?;
    if let Some(path) = rounded_rect(0.0, 0.0, w as f32, h as f32, 0.0) {
        clip.fill_path(&path, FillRule::Winding, false, Transform::identity());
    }
    ctx.grid(&mut r, &clip);
    ctx.groups(&mut r, &clip, options.privacy);
    ctx.edges(&mut r, &clip);
    ctx.cards(&mut r, &clip, options.privacy);
    ctx.edge_labels(&mut r, &clip, options.privacy);
    if options.show_logo {
        logo_strip(&mut r, image, h as f32, strip as f32, f.ratio as f32)?;
    }
    Ok(r.pixmap)
}

/// The canvas as PNG bytes: RGBA for the full canvas, RGB for a viewport.
pub fn export_image(image: &CanvasImage<'_>, options: &ImageOptions) -> Result<Vec<u8>, String> {
    let pixmap = render_image(image, options)?;
    encode_png(&pixmap, matches!(options.area, Area::Full))
}

struct Ctx<'a, 'b> {
    image: &'b CanvasImage<'a>,
    f: Frame,
    base: Transform,
    s: f32,
    ls: f32,
}

impl Ctx<'_, '_> {
    /// A canvas point in pixels.
    fn px(&self, p: Point) -> (f32, f32) {
        (
            ((p.x - self.f.area.x) * f64::from(self.s)) as f32,
            ((p.y - self.f.area.y) * f64::from(self.s)) as f32,
        )
    }

    fn rect_px(&self, r: &Rect) -> (f32, f32, f32, f32) {
        let (x, y) = self.px(Point::new(r.x, r.y));
        (x, y, r.width as f32 * self.s, r.height as f32 * self.s)
    }

    fn grid(&self, r: &mut Raster<'_>, clip: &Mask) {
        let mut spacing = GRID;
        while spacing * self.f.view_scale < MIN_DOT_SPACING {
            spacing *= 2.0;
        }
        let area = self.f.area;
        let radius = (DOT_RADIUS * self.f.ratio) as f32;
        let dot = paint(self.image.palette.dot);
        let mut pb = PathBuilder::new();
        let mut y = (area.y / spacing).floor() * spacing;
        while y <= area.bottom() {
            let mut x = (area.x / spacing).floor() * spacing;
            while x <= area.right() {
                let (px, py) = self.px(Point::new(x, y));
                pb.push_circle(px, py, radius);
                x += spacing;
            }
            y += spacing;
        }
        if let Some(path) = pb.finish() {
            r.pixmap.fill_path(
                &path,
                &dot,
                FillRule::Winding,
                Transform::identity(),
                Some(clip),
            );
        }
    }

    fn border_px(&self) -> f32 {
        (BORDER_WIDTH * self.s).max(1.0)
    }

    /// A card's or a group's frame: a ground, a tint and a border.
    fn frame_box(
        &self,
        r: &mut Raster<'_>,
        clip: &Mask,
        rect: &Rect,
        ground: Option<Color>,
        color: Option<Color>,
    ) {
        let p = self.image.palette;
        let (x, y, w, h) = self.rect_px(rect);
        let radius = CARD_RADIUS as f32 * self.s;
        if let Some(ground) = ground {
            if let Some(path) = rounded_rect(x, y, w, h, radius) {
                r.pixmap.fill_path(
                    &path,
                    &paint(ground),
                    FillRule::Winding,
                    Transform::identity(),
                    Some(clip),
                );
            }
        }
        let tint = color.unwrap_or(p.card_border);
        let tinted = color.is_some() || ground.is_none();
        if tinted {
            if let Some(path) = rounded_rect(x, y, w, h, radius) {
                r.pixmap.fill_path(
                    &path,
                    &paint(tint.alpha(p.tint_alpha)),
                    FillRule::Winding,
                    Transform::identity(),
                    Some(clip),
                );
            }
        }
        let bw = self.border_px();
        let border = match color {
            Some(c) => c.alpha(p.border_alpha),
            None => p.card_border,
        };
        if let Some(path) = rounded_rect(
            x + bw / 2.0,
            y + bw / 2.0,
            w - bw,
            h - bw,
            (radius - bw / 2.0).max(0.0),
        ) {
            let stroke = Stroke {
                width: bw,
                ..Stroke::default()
            };
            r.pixmap.stroke_path(
                &path,
                &paint(border),
                &stroke,
                Transform::identity(),
                Some(clip),
            );
        }
    }

    fn groups(&self, r: &mut Raster<'_>, clip: &Mask, privacy: bool) {
        let p = self.image.palette;
        for node in &self.image.canvas.nodes {
            if node.kind != NodeType::Group {
                continue;
            }
            let color = p.color_of(node.color.as_deref());
            let rect = Rect::of(node);
            self.frame_box(r, clip, &rect, None, color);
            let Some(label) = node.label.as_deref().filter(|l| !l.is_empty()) else {
                continue;
            };
            let (bg, fg) = match color {
                Some(c) => (c, p.on_color),
                None => (p.card_border.alpha(p.wash_alpha), p.muted),
            };
            let size = (GROUP_LABEL_SIZE as f32) * self.ls;
            let pad = (
                GROUP_LABEL_PADDING.0 as f32 * self.ls,
                GROUP_LABEL_PADDING.1 as f32 * self.ls,
            );
            let (runs, width) = label_run(self.image.fonts, label, Face::Regular, size, fg);
            let (x, y, w, _) = self.rect_px(&rect);
            let width = width.min(w - pad.0 * 2.0);
            let ph = size + pad.1 * 2.0;
            let top = y - ph - LABEL_GAP as f32 * self.ls;
            if let Some(path) = rounded_rect(
                x,
                top,
                width + pad.0 * 2.0,
                ph,
                LABEL_RADIUS as f32 * self.ls,
            ) {
                r.pixmap.fill_path(
                    &path,
                    &paint(bg),
                    FillRule::Winding,
                    Transform::identity(),
                    Some(clip),
                );
            }
            let baseline = top + pad.1 + size * 0.82;
            self.draw_runs(r, runs, x + pad.0, baseline, clip, privacy);
        }
    }

    fn draw_runs(
        &self,
        r: &mut Raster<'_>,
        runs: Vec<crate::layout::GlyphRun>,
        x: f32,
        baseline: f32,
        clip: &Mask,
        privacy: bool,
    ) {
        let was = r.privacy;
        r.privacy = privacy;
        for mut run in runs {
            run.x += x;
            run.y += baseline;
            r.draw(&Item::Text(run), Transform::identity(), Some(clip));
        }
        r.privacy = was;
    }

    fn edges(&self, r: &mut Raster<'_>, clip: &Mask) {
        let p = self.image.palette;
        let canvas = self.image.canvas;
        let width = EDGE_WIDTH * self.ls;
        for edge in &canvas.edges {
            let (Some(from), Some(to)) = (
                canvas.nodes.iter().find(|n| n.id == edge.from_node),
                canvas.nodes.iter().find(|n| n.id == edge.to_node),
            ) else {
                continue;
            };
            let (fr, tr) = (Rect::of(from), Rect::of(to));
            let from_side = edge
                .from_side
                .unwrap_or_else(|| facing_side(&fr, tr.center()));
            let to_side = edge
                .to_side
                .unwrap_or_else(|| facing_side(&tr, fr.center()));
            let path = edge_path(&fr, from_side, &tr, to_side);
            let color = p.color_of(edge.color.as_deref()).unwrap_or(p.card_border);
            let stroke = Stroke {
                width,
                line_cap: tiny_skia::LineCap::Butt,
                ..Stroke::default()
            };
            let mut pb = PathBuilder::new();
            let a = self.px(path.from);
            let c1 = self.px(path.c1);
            let c2 = self.px(path.c2);
            let b = self.px(path.to);
            pb.move_to(a.0, a.1);
            pb.cubic_to(c1.0, c1.1, c2.0, c2.1, b.0, b.1);
            if let Some(curve) = pb.finish() {
                r.pixmap.stroke_path(
                    &curve,
                    &paint(color),
                    &stroke,
                    Transform::identity(),
                    Some(clip),
                );
            }
            let (from_arrow, to_arrow) = edge_ends(edge);
            for (rect, side, arrow) in [(&fr, from_side, from_arrow), (&tr, to_side, to_arrow)] {
                self.edge_end(r, clip, anchor(rect, side), side, arrow, &stroke, color);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn edge_end(
        &self,
        r: &mut Raster<'_>,
        clip: &Mask,
        at: Point,
        side: Side,
        arrow: bool,
        stroke: &Stroke,
        color: Color,
    ) {
        let mut pb = PathBuilder::new();
        if arrow {
            let n = side_normal(side);
            // The head's length follows the stroke, in canvas units.
            let length = ARROW * f64::from(self.ls) / f64::from(self.s);
            let [t, l, rr] = arrow_head(at, Point::new(-n.x, -n.y), length);
            let (t, l, rr) = (self.px(t), self.px(l), self.px(rr));
            pb.move_to(t.0, t.1);
            pb.line_to(l.0, l.1);
            pb.line_to(rr.0, rr.1);
            pb.close();
            if let Some(path) = pb.finish() {
                r.pixmap.fill_path(
                    &path,
                    &paint(color),
                    FillRule::Winding,
                    Transform::identity(),
                    Some(clip),
                );
            }
        } else {
            let (a, b) = edge_stub(at, side);
            let (a, b) = (self.px(a), self.px(b));
            pb.move_to(a.0, a.1);
            pb.line_to(b.0, b.1);
            if let Some(path) = pb.finish() {
                r.pixmap.stroke_path(
                    &path,
                    &paint(color),
                    stroke,
                    Transform::identity(),
                    Some(clip),
                );
            }
        }
    }

    fn edge_labels(&self, r: &mut Raster<'_>, clip: &Mask, privacy: bool) {
        let p = self.image.palette;
        let canvas = self.image.canvas;
        for edge in &canvas.edges {
            let Some(label) = edge.label.as_deref().filter(|l| !l.is_empty()) else {
                continue;
            };
            let (Some(from), Some(to)) = (
                canvas.nodes.iter().find(|n| n.id == edge.from_node),
                canvas.nodes.iter().find(|n| n.id == edge.to_node),
            ) else {
                continue;
            };
            let (fr, tr) = (Rect::of(from), Rect::of(to));
            let from_side = edge
                .from_side
                .unwrap_or_else(|| facing_side(&fr, tr.center()));
            let to_side = edge
                .to_side
                .unwrap_or_else(|| facing_side(&tr, fr.center()));
            let mid = self.px(edge_path(&fr, from_side, &tr, to_side).midpoint());
            let size = EDGE_LABEL_SIZE as f32 * self.ls;
            let line = EDGE_LABEL_LINE as f32 * self.ls;
            let pad = EDGE_LABEL_PADDING as f32 * self.ls;
            let (runs, width) = label_run(self.image.fonts, label, Face::Regular, size, p.text);
            let (w, h) = (width + pad * 2.0, line + pad * 2.0);
            let (x, y) = (mid.0 - w / 2.0, mid.1 - h / 2.0);
            if let Some(path) = rounded_rect(x, y, w, h, LABEL_RADIUS as f32 * self.ls) {
                r.pixmap.fill_path(
                    &path,
                    &paint(p.card),
                    FillRule::Winding,
                    Transform::identity(),
                    Some(clip),
                );
            }
            let baseline = y + pad + (line - size) / 2.0 + size * 0.82;
            self.draw_runs(r, runs, x + pad, baseline, clip, privacy);
        }
    }

    fn cards(&self, r: &mut Raster<'_>, clip: &Mask, privacy: bool) {
        let p = self.image.palette;
        for node in &self.image.canvas.nodes {
            if node.kind == NodeType::Group {
                continue;
            }
            let rect = Rect::of(node);
            let color = p.color_of(node.color.as_deref());
            self.frame_box(r, clip, &rect, Some(p.card), color);
            let (x, y, w, h) = self.rect_px(&rect);
            // The card's inside, for its content.
            let bw = self.border_px();
            let Some(mut inner) = Mask::new(clip.width(), clip.height()) else {
                continue;
            };
            if let Some(path) = rounded_rect(
                x + bw,
                y + bw,
                w - 2.0 * bw,
                h - 2.0 * bw,
                (CARD_RADIUS as f32 * self.s - bw).max(0.0),
            ) {
                inner.fill_path(&path, FillRule::Winding, true, Transform::identity());
            }
            // Within the image too.
            let mut both = inner;
            {
                let (a, b) = (both.data_mut(), clip.data());
                for (m, c) in a.iter_mut().zip(b) {
                    *m = ((u16::from(*m) * u16::from(*c)) / 255) as u8;
                }
            }
            r.privacy = privacy;
            match self.image.contents.get(&node.id) {
                Some(CardContent::Blocks(blocks)) => {
                    let width = rect.width as f32 - 2.0 * CARD_PADDING;
                    let (pages, _) = layout(
                        blocks,
                        None,
                        width.max(1.0),
                        None,
                        self.image.fonts,
                        self.image.text,
                    );
                    let t = self
                        .base
                        .pre_translate(node.x as f32 + CARD_PADDING, node.y as f32 + CARD_PADDING);
                    for page in &pages {
                        for item in &page.items {
                            r.draw(item, t, Some(&both));
                        }
                    }
                }
                Some(CardContent::Image(data)) => {
                    if let Some((iw, ih)) = crate::layout::image_size(data) {
                        let (iw, ih) = (iw as f32, ih as f32);
                        let fit = (w / iw).min(h / ih);
                        let (dw, dh) = (iw * fit, ih * fit);
                        let item = Item::Image {
                            x: x + (w - dw) / 2.0,
                            y: y + (h - dh) / 2.0,
                            w: dw,
                            h: dh,
                            data: data.clone(),
                        };
                        r.draw(&item, Transform::identity(), Some(&both));
                    }
                }
                Some(CardContent::Text(text)) => {
                    let size = 16.0 * self.s;
                    let (runs, width) =
                        label_run(self.image.fonts, text, Face::Regular, size, p.muted);
                    let tx = x + ((w - width) / 2.0).max(CARD_PADDING * self.s);
                    let baseline = y + h / 2.0 + size * 0.35;
                    self.draw_runs(r, runs, tx, baseline, &both, privacy);
                }
                None => {}
            }
            r.privacy = false;
            // The name above a file card, the address above a link card.
            let label = match node.kind {
                NodeType::File => node.file.as_deref().map(|f| {
                    let name = f.rsplit('/').next().unwrap_or(f);
                    name.strip_suffix(".md").unwrap_or(name).to_string()
                }),
                NodeType::Link => node.url.clone(),
                _ => None,
            };
            if let Some(label) = label.filter(|l| !l.is_empty()) {
                let size = CARD_LABEL_SIZE as f32 * self.ls;
                let line = size * LABEL_LINE_HEIGHT as f32;
                let top = y - line - LABEL_GAP as f32 * self.ls;
                let (runs, _) = label_run(self.image.fonts, &label, Face::Regular, size, p.label);
                let baseline = top + (line - size) / 2.0 + size * 0.82;
                self.draw_runs(r, runs, x, baseline, clip, privacy);
            }
        }
    }
}

/// The strip under the canvas: Scoria's mark and its name at the bottom
/// left, on the canvas's ground.
fn logo_strip(
    r: &mut Raster<'_>,
    image: &CanvasImage<'_>,
    top: f32,
    height: f32,
    ratio: f32,
) -> Result<(), String> {
    let p = image.palette;
    let tree = resvg::usvg::Tree::from_data(image.logo, &resvg::usvg::Options::default())
        .map_err(|e| e.to_string())?;
    let size = tree.size();
    let mark_h = 36.0 * ratio;
    let scale = mark_h / size.height();
    let left = 24.0 * ratio;
    let mark_top = top + (height - mark_h) / 2.0;
    resvg::render(
        &tree,
        Transform::from_row(scale, 0.0, 0.0, scale, left, mark_top),
        &mut r.pixmap.as_mut(),
    );
    let font = 26.0 * ratio;
    let (runs, _) = label_run(image.fonts, "Scoria", Face::Display, font, p.text);
    let x = left + size.width() * scale + 12.0 * ratio;
    let baseline = top + height / 2.0 + font * 0.36;
    for mut run in runs {
        run.x += x;
        run.y += baseline;
        r.draw(&Item::Text(run), Transform::identity(), None);
    }
    Ok(())
}
