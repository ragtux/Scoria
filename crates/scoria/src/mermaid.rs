//! Mermaid diagrams on screen: paints what `scoria-mermaid` lays out.
//!
//! A ```` ```mermaid ```` fence is drawn as Obsidian 1.15 draws it with
//! Pele: the diagram at its own size, left-aligned in the block, shrunk to
//! the block's width when it is wider (Pele's `max-width: 100%;
//! height: auto`), laid out again for the width the block gives it (Pele
//! turns a wide `LR` diagram to run down and wraps long text to fit). Paths
//! go through gpui's tessellator, filled and stroked; text is shaped in the
//! interface face (Inter, which is also what layout measured it in) and
//! set from each run's anchor. A fence that does not lay out shows Pele's
//! message in the monospace face at 0.8 em in `--text-error`, the way
//! Obsidian shows it. Three callers share this: the reading view's code
//! block, Live Preview's widget (which is the reading view's block), and
//! everything that embeds the reading view (embeds, hover previews).

use std::sync::Arc;

use gpui::{
    div, point, px, size, AnyElement, App, Bounds, Element, ElementId, FontStyle, FontWeight,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, ParentElement, PathBuilder,
    Pixels, Point, SharedString, Size, Style, Styled, TextAlign, TextRun, UnderlineStyle, Window,
};
use gpui::{AvailableSpace, StrikethroughStyle};
use scoria_mermaid::{Anchor, Diagram, LineCap, LineJoin, Rgba, Segment, Shape};

use crate::theme::Theme;

/// The bottom gap an inline `<svg>` leaves under itself in its line box:
/// Obsidian's block measured 6 px taller than its diagram, every case.
pub const BOTTOM_GAP: f32 = 6.;

/// The diagram colours of a theme: Obsidian's `--mermaid-*` variables are
/// its own tokens (see `scoria_mermaid::Theme`).
pub fn colors(t: &Theme) -> scoria_mermaid::Theme {
    let k = &t.tokens;
    scoria_mermaid::Theme {
        background: rgba(k.background_primary),
        text: rgba(k.text_normal),
        muted: rgba(k.text_muted),
        line: rgba(k.text_normal),
        surface: rgba(k.background_secondary),
        surface_alt: rgba(k.background_primary_alt),
        border: rgba(k.background_modifier_border_hover),
        accent: rgba(k.interactive_accent),
        critical: rgba(k.color_red),
        error: rgba(k.text_error),
        series: [
            rgba(k.color_blue),
            rgba(k.color_orange),
            rgba(k.color_green),
            rgba(k.color_red),
            rgba(k.color_cyan),
            rgba(k.color_yellow),
            rgba(k.color_purple),
            rgba(k.color_pink),
        ],
        radius: 4.0,
    }
}

fn rgba(c: Hsla) -> Rgba {
    let c = gpui::Rgba::from(c);
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Rgba {
        r: b(c.r),
        g: b(c.g),
        b: b(c.b),
        a: b(c.a),
    }
}

fn hsla(c: Rgba) -> Hsla {
    gpui::Rgba {
        r: c.r as f32 / 255.,
        g: c.g as f32 / 255.,
        b: c.b as f32 / 255.,
        a: c.a as f32 / 255.,
    }
    .into()
}

/// The block a fence is drawn as: the diagram, or the error.
pub fn block(source: &str, t: &Theme) -> AnyElement {
    let source: Arc<str> = source.into();
    let colors = colors(t);
    // Layout fails or succeeds whatever the width: ask once at no limit to
    // know which, and draw the error here, where it can be text.
    match scoria_mermaid::render(&source, f64::INFINITY, &colors) {
        Err(e) => error(&e.message, t),
        Ok(_) => DiagramElement {
            source,
            colors: Arc::new(colors),
            font: t.ui_font.clone(),
        }
        .into_any_element(),
    }
}

/// Pele's message as Obsidian shows it: `.pele-error`, the monospace face
/// at 0.8 em, `--mermaid-error`, lines kept and wrapped.
fn error(message: &str, t: &Theme) -> AnyElement {
    let size = t.base_font_size * 0.8;
    div()
        .w_full()
        .font_family(t.mono_font.clone())
        .text_size(px(size))
        .line_height(px(size * 1.5))
        .text_color(t.tokens.text_error)
        .children(
            message
                .split('\n')
                .map(|l| {
                    div()
                        .min_h(px(size * 1.5))
                        .child(SharedString::from(l.to_string()))
                })
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

/// A diagram as an element: laid out for the width it is given and shrunk
/// to it when it is still wider.
pub struct DiagramElement {
    source: Arc<str>,
    colors: Arc<scoria_mermaid::Theme>,
    font: SharedString,
}

/// The size a diagram takes in `width`: its own, or scaled down to fit.
fn fitted(source: &str, colors: &scoria_mermaid::Theme, width: Option<f32>) -> Size<Pixels> {
    let max = width.map(f64::from).unwrap_or(f64::INFINITY);
    match scoria_mermaid::render(source, max, colors) {
        Ok(d) => {
            let k = scale(&d, width);
            size(px(d.width as f32 * k), px(d.height as f32 * k))
        }
        Err(_) => size(px(0.), px(0.)),
    }
}

fn scale(d: &Diagram, width: Option<f32>) -> f32 {
    match width {
        Some(w) if (d.width as f32) > w && d.width > 0. => (w / d.width as f32).max(0.01),
        _ => 1.,
    }
}

impl IntoElement for DiagramElement {
    type Element = DiagramElement;

    fn into_element(self) -> DiagramElement {
        self
    }
}

impl Element for DiagramElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            flex_shrink: 0.,
            ..Default::default()
        };
        let source = self.source.clone();
        let colors = self.colors.clone();
        let id = window.request_measured_layout(style, move |known, available, _, _| {
            let width = known.width.map(f32::from).or(match available.width {
                AvailableSpace::Definite(w) => Some(f32::from(w)),
                // Asked for its narrowest: nothing it could not shrink to.
                AvailableSpace::MinContent => Some(0.),
                AvailableSpace::MaxContent => None,
            });
            fitted(&source, &colors, width)
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Laid out for the width it was given; the bounds say what that
        // came to (a fitted diagram fills them).
        let Ok(d) = scoria_mermaid::render(&self.source, f64::INFINITY, &self.colors) else {
            return;
        };
        let given = f32::from(bounds.size.width);
        let d = if (d.width as f32) > given + 0.5 {
            match scoria_mermaid::render(&self.source, f64::from(given), &self.colors) {
                Ok(d) => d,
                Err(_) => return,
            }
        } else {
            d
        };
        let k = scale(&d, Some(given));
        paint(&d, bounds.origin, k, &self.font, window, cx);
    }
}

/// Paints `d` with its top left at `origin`, `k` pixels per CSS pixel.
pub fn paint(
    d: &Diagram,
    origin: Point<Pixels>,
    k: f32,
    font: &SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    let at = |x: f64, y: f64| point(origin.x + px(x as f32 * k), origin.y + px(y as f32 * k));
    for shape in &d.shapes {
        match shape {
            Shape::Path {
                segments,
                fill,
                stroke,
            } => {
                if let Some(fill) = fill {
                    // SVG fills by the non-zero rule; lyon's default is even-odd.
                    let mut b = PathBuilder::fill()
                        .with_style(gpui::PathStyle::Fill(gpui::FillOptions::non_zero()));
                    trace(&mut b, segments, &at);
                    if let Ok(p) = b.build() {
                        window.paint_path(p, hsla(*fill));
                    }
                }
                if let Some(s) = stroke {
                    let options = gpui::StrokeOptions::default()
                        .with_line_width(s.width as f32 * k)
                        .with_line_cap(match s.cap {
                            LineCap::Butt => lyon::tessellation::LineCap::Butt,
                            LineCap::Round => lyon::tessellation::LineCap::Round,
                            LineCap::Square => lyon::tessellation::LineCap::Square,
                        })
                        .with_line_join(match s.join {
                            LineJoin::Miter => lyon::tessellation::LineJoin::Miter,
                            LineJoin::Round => lyon::tessellation::LineJoin::Round,
                            LineJoin::Bevel => lyon::tessellation::LineJoin::Bevel,
                        })
                        .with_miter_limit(4.0);
                    let mut b = PathBuilder::stroke(px(s.width as f32 * k))
                        .with_style(gpui::PathStyle::Stroke(options));
                    if !s.dash.is_empty() {
                        let dash: Vec<Pixels> = s.dash.iter().map(|v| px(*v as f32 * k)).collect();
                        b = b.dash_array(&dash);
                    }
                    trace(&mut b, segments, &at);
                    if let Ok(p) = b.build() {
                        window.paint_path(p, hsla(s.color));
                    }
                }
            }
            Shape::Text(t) => {
                let mut text = String::new();
                let mut runs = Vec::with_capacity(t.spans.len());
                // One shaping per run, each span its own font; the sizes of a
                // run's spans differ only for a sub- or superscript, which is
                // shaped at the run's first size (gpui shapes one size).
                let size = t.spans.first().map(|s| s.size).unwrap_or(16.);
                for s in &t.spans {
                    text.push_str(&s.text);
                    runs.push(TextRun {
                        len: s.text.len(),
                        font: gpui::Font {
                            family: font.clone(),
                            weight: if s.bold {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            },
                            style: if s.italic {
                                FontStyle::Italic
                            } else {
                                FontStyle::Normal
                            },
                            ..Default::default()
                        },
                        color: hsla(t.color),
                        background_color: None,
                        underline: s.underline.then(|| UnderlineStyle {
                            thickness: px(1.),
                            color: Some(hsla(t.color)),
                            wavy: false,
                        }),
                        strikethrough: None::<StrikethroughStyle>,
                    });
                }
                let line = window.text_system().shape_line(
                    SharedString::from(text),
                    px(size as f32 * k),
                    &runs,
                    None,
                );
                let w = line.width;
                let x = match t.anchor {
                    Anchor::Start => px(0.),
                    Anchor::Middle => w / 2.,
                    Anchor::End => w,
                };
                let base = at(t.x, t.y);
                let lh = line.ascent + line.descent;
                let shift = t.spans.first().map(|s| s.shift).unwrap_or(0.) as f32 * k;
                let o = point(base.x - x, base.y - line.ascent + px(shift));
                let _ = line.paint(o, lh, TextAlign::Left, None, window, cx);
            }
        }
    }
}

fn trace(b: &mut PathBuilder, segments: &[Segment], at: &impl Fn(f64, f64) -> Point<Pixels>) {
    for s in segments {
        match *s {
            // A move ends the figure before it open, as SVG's does.
            Segment::Move(x, y) => b.move_to(at(x, y)),
            Segment::Line(x, y) => b.line_to(at(x, y)),
            Segment::Quad(cx, cy, x, y) => b.curve_to(at(x, y), at(cx, cy)),
            Segment::Cubic(ax, ay, bx, by, x, y) => {
                b.cubic_bezier_to(at(x, y), at(ax, ay), at(bx, by))
            }
            Segment::Close => b.close(),
        }
    }
}
