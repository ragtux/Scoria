//! Drawing items into pixels with tiny-skia: the PNG backend.
//!
//! Glyphs are filled from their outlines (read with `ab_glyph`), so the
//! PNG uses the same faces, ids and positions the PDF does.

use crate::doc::Color;
use crate::fonts::{Face, Fonts};
use crate::layout::{Item, PathOp};
use ab_glyph::{Font, GlyphId, OutlineCurve};
use std::collections::HashMap;
use tiny_skia::{
    FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

pub(crate) struct Raster<'a> {
    pub pixmap: Pixmap,
    fonts: &'a Fonts,
    /// Each glyph's outline in em, `y` down.
    outlines: HashMap<(Face, u16), Option<Path>>,
    images: HashMap<usize, Option<Pixmap>>,
    /// Privacy mode: every run of text a bar of its colour.
    pub privacy: bool,
}

pub(crate) fn paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;
    paint
}

/// A rectangle, its corners rounded by `r`.
pub(crate) fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.0 {
        return tiny_skia::Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))
            .map(PathBuilder::from_rect);
    }
    let c = r * 0.552_284_8;
    let mut pb = PathBuilder::new();
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

pub(crate) fn ops_path(ops: &[PathOp]) -> Option<Path> {
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

impl<'a> Raster<'a> {
    pub fn new(pixmap: Pixmap, fonts: &'a Fonts) -> Raster<'a> {
        Raster {
            pixmap,
            fonts,
            outlines: HashMap::new(),
            images: HashMap::new(),
            privacy: false,
        }
    }

    fn outline(&mut self, face: Face, id: u16) -> Option<Path> {
        let fonts = self.fonts;
        self.outlines
            .entry((face, id))
            .or_insert_with(|| {
                let font = fonts.font(face)?;
                let upem = font.units_per_em().unwrap_or(1000.0);
                let outline = font.outline(GlyphId(id))?;
                let mut pb = PathBuilder::new();
                let mut last: Option<ab_glyph::Point> = None;
                let p = |q: ab_glyph::Point| (q.x / upem, -q.y / upem);
                for curve in &outline.curves {
                    let start = match curve {
                        OutlineCurve::Line(a, _)
                        | OutlineCurve::Quad(a, _, _)
                        | OutlineCurve::Cubic(a, _, _, _) => *a,
                    };
                    if last != Some(start) {
                        if last.is_some() {
                            pb.close();
                        }
                        let (x, y) = p(start);
                        pb.move_to(x, y);
                    }
                    match curve {
                        OutlineCurve::Line(_, b) => {
                            let (x, y) = p(*b);
                            pb.line_to(x, y);
                            last = Some(*b);
                        }
                        OutlineCurve::Quad(_, c, b) => {
                            let (cx, cy) = p(*c);
                            let (x, y) = p(*b);
                            pb.quad_to(cx, cy, x, y);
                            last = Some(*b);
                        }
                        OutlineCurve::Cubic(_, c1, c2, b) => {
                            let (ax, ay) = p(*c1);
                            let (bx, by) = p(*c2);
                            let (x, y) = p(*b);
                            pb.cubic_to(ax, ay, bx, by, x, y);
                            last = Some(*b);
                        }
                    }
                }
                if last.is_some() {
                    pb.close();
                }
                pb.finish()
            })
            .clone()
    }

    fn image(&mut self, data: &std::sync::Arc<Vec<u8>>) -> Option<Pixmap> {
        let key = std::sync::Arc::as_ptr(data) as usize;
        self.images
            .entry(key)
            .or_insert_with(|| {
                let rgba = image::load_from_memory(data).ok()?.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut raw = rgba.into_raw();
                // tiny-skia wants premultiplied alpha.
                for px in raw.as_chunks_mut::<4>().0 {
                    let a = u16::from(px[3]);
                    for c in &mut px[..3] {
                        *c = ((u16::from(*c) * a + 127) / 255) as u8;
                    }
                }
                Pixmap::from_vec(raw, tiny_skia::IntSize::from_wh(w, h)?)
            })
            .clone()
    }

    /// Draws `item`, laid out in layout pixels, through `t`.
    pub fn draw(&mut self, item: &Item, t: Transform, mask: Option<&Mask>) {
        match item {
            Item::Text(run) => {
                if self.privacy {
                    let w = run.width();
                    if let Some(path) = rounded_rect(
                        run.x,
                        run.y - run.size * 0.72,
                        w,
                        run.size * 0.8,
                        run.size * 0.2,
                    ) {
                        self.pixmap
                            .fill_path(&path, &paint(run.color), FillRule::Winding, t, mask);
                    }
                    return;
                }
                let p = paint(run.color);
                for g in &run.glyphs {
                    if let Some(path) = self.outline(run.face, g.id) {
                        let gt = t
                            .pre_translate(run.x + g.x, run.y)
                            .pre_scale(run.size, run.size);
                        self.pixmap
                            .fill_path(&path, &p, FillRule::Winding, gt, mask);
                    }
                }
            }
            Item::Rect {
                x,
                y,
                w,
                h,
                color,
                radius,
            } => {
                if let Some(path) = rounded_rect(*x, *y, *w, *h, *radius) {
                    self.pixmap
                        .fill_path(&path, &paint(*color), FillRule::Winding, t, mask);
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
                let half = width / 2.0;
                if let Some(path) = rounded_rect(x + half, y + half, w - width, h - width, *radius)
                {
                    let stroke = Stroke {
                        width: *width,
                        ..Stroke::default()
                    };
                    self.pixmap
                        .stroke_path(&path, &paint(*color), &stroke, t, mask);
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
                let mut pb = PathBuilder::new();
                pb.move_to(*x1, *y1);
                pb.line_to(*x2, *y2);
                if let Some(path) = pb.finish() {
                    let stroke = Stroke {
                        width: *width,
                        ..Stroke::default()
                    };
                    self.pixmap
                        .stroke_path(&path, &paint(*color), &stroke, t, mask);
                }
            }
            Item::Path {
                ops,
                fill,
                stroke,
                color,
            } => {
                if let Some(path) = ops_path(ops) {
                    if *fill {
                        self.pixmap
                            .fill_path(&path, &paint(*color), FillRule::Winding, t, mask);
                    } else {
                        let s = Stroke {
                            width: *stroke,
                            line_cap: tiny_skia::LineCap::Round,
                            line_join: tiny_skia::LineJoin::Round,
                            ..Stroke::default()
                        };
                        self.pixmap.stroke_path(&path, &paint(*color), &s, t, mask);
                    }
                }
            }
            Item::Image { x, y, w, h, data } => {
                if let Some(img) = self.image(data) {
                    let (iw, ih) = (img.width() as f32, img.height() as f32);
                    let it = t.pre_translate(*x, *y).pre_scale(w / iw, h / ih);
                    let pp = PixmapPaint {
                        quality: FilterQuality::Bicubic,
                        ..PixmapPaint::default()
                    };
                    self.pixmap.draw_pixmap(0, 0, img.as_ref(), &pp, it, mask);
                }
            }
            Item::Link { .. } => {}
        }
    }
}

/// The pixmap as a PNG: RGBA, or RGB with the alpha dropped (over the
/// pixmap's own opaque ground).
pub(crate) fn encode_png(pixmap: &Pixmap, alpha: bool) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
        encoder.set_color(if alpha {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        let mut data = Vec::with_capacity(pixmap.data().len());
        for px in pixmap.pixels() {
            let c = px.demultiply();
            data.extend_from_slice(&[c.red(), c.green(), c.blue()]);
            if alpha {
                data.push(c.alpha());
            }
        }
        writer.write_image_data(&data).map_err(|e| e.to_string())?;
    }
    Ok(out)
}
