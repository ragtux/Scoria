//! Mermaid diagrams laid out into a drawing list.
//!
//! Obsidian does not run Mermaid itself any more: since 1.15 it renders a
//! ```` ```mermaid ```` fence with Pele, its own MIT-licensed Mermaid renderer
//! (Pele 1.13.0 in Obsidian 1.15.1, `lib/pele.min.js`). Scoria lays the same
//! fence out with limu, a Rust port of Pele whose tests compare its numbers
//! with Pele's own (MIT; Pele's licence travels with it), and turns limu's
//! scene into the drawing list here: filled and stroked paths and anchored
//! text runs, in CSS pixels, with the colours of a [`Theme`] already settled.
//! The app paints the list with gpui, and the PDF export can draw it too.
//!
//! Layout depends on the source, the width the note gives the diagram and
//! the faces text is measured in; painting depends on the theme as well.
//! Pele measures text in the face the diagram is drawn in, which in Obsidian
//! is Inter (see `fonts`), so layout here shapes text in the Inter the app
//! bundles, and a painter sets each run in the same face from the run's
//! anchor, as the browser sets an SVG `<text>` from its `text-anchor`.
//!
//! Every result is cached, a diagram that does not parse with its message:
//! the views ask for the same diagram every frame.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

mod fonts;

pub use fonts::{set_fonts, Fonts};

use limu::layout::Options as LayoutOptions;
use limu::scene::style::{Cap, Color, Ink, Join, Look, Theme as LimuTheme};
use limu::scene::{Align, Canvas, Path, Pen, Point, Run, Step, TextPaint};

/// The size text is set at in a diagram with no size of its own: Pele's 16.
pub const FONT_SIZE: f64 = 16.0;

/// A colour, straight (not premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 255 }
    }

    /// `0xRRGGBB`.
    pub const fn hex(v: u32) -> Rgba {
        Rgba::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    fn of(c: Color) -> Rgba {
        Rgba {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }

    fn color(self) -> Color {
        Color::new(self.r, self.g, self.b, self.a)
    }
}

/// The colours a diagram is drawn in: Obsidian's `--mermaid-*` variables,
/// which it hands Pele as `--pele-*`. Each is one of the theme's own
/// variables (observed declarations, Obsidian 1.15.1):
///
/// | field | variable | Obsidian's |
/// | --- | --- | --- |
/// | `background` | `--mermaid-background` | `--background-primary` |
/// | `text` | `--mermaid-text-color` | `--text-normal` |
/// | `muted` | `--mermaid-muted-color` | `--text-muted` |
/// | `line` | `--mermaid-line-color` | `--text-normal` |
/// | `surface` | `--mermaid-surface` | `--background-secondary` |
/// | `surface_alt` | `--mermaid-surface-alt` | `--background-primary-alt` |
/// | `border` | `--mermaid-border-color` | `--background-modifier-border-hover` |
/// | `accent` | `--mermaid-accent` | `--interactive-accent` |
/// | `critical` | `--mermaid-critical` | `--color-red` |
/// | `error` | `--mermaid-error` | `--text-error` |
/// | `series` | `--mermaid-color-1..8` | `--color-blue`, `-orange`, `-green`, `-red`, `-cyan`, `-yellow`, `-purple`, `-pink` |
/// | `radius` | `--mermaid-radius` | `--radius-s` |
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub background: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub line: Rgba,
    pub surface: Rgba,
    pub surface_alt: Rgba,
    pub border: Rgba,
    pub accent: Rgba,
    pub critical: Rgba,
    pub error: Rgba,
    pub series: [Rgba; 8],
    pub radius: f64,
}

impl Theme {
    /// Obsidian's light base theme, as its computed variables read.
    pub fn light() -> Theme {
        Theme {
            background: Rgba::hex(0xffffff),
            text: Rgba::hex(0x222222),
            muted: Rgba::hex(0x5c5c5c),
            line: Rgba::hex(0x222222),
            surface: Rgba::hex(0xf6f6f6),
            surface_alt: Rgba::hex(0xfafafa),
            border: Rgba::hex(0xdadada),
            accent: Rgba::hex(0x9873f7),
            critical: Rgba::hex(0xe93147),
            error: Rgba::hex(0xe93147),
            series: [
                Rgba::hex(0x086ddd),
                Rgba::hex(0xec7500),
                Rgba::hex(0x08b94e),
                Rgba::hex(0xe93147),
                Rgba::hex(0x00bfbc),
                Rgba::hex(0xe0ac00),
                Rgba::hex(0x7852ee),
                Rgba::hex(0xd53984),
            ],
            radius: 4.0,
        }
    }

    /// Obsidian's dark base theme.
    pub fn dark() -> Theme {
        Theme {
            background: Rgba::hex(0x1c1c1c),
            text: Rgba::hex(0xdadada),
            muted: Rgba::hex(0xb3b3b3),
            line: Rgba::hex(0xdadada),
            surface: Rgba::hex(0x282828),
            surface_alt: Rgba::hex(0x232323),
            border: Rgba::hex(0x3f3f3f),
            accent: Rgba::hex(0x8a5cf5),
            critical: Rgba::hex(0xfb464c),
            error: Rgba::hex(0xfb464c),
            series: [
                Rgba::hex(0x027aff),
                Rgba::hex(0xe9973f),
                Rgba::hex(0x44cf6e),
                Rgba::hex(0xfb464c),
                Rgba::hex(0x53dfdd),
                Rgba::hex(0xe0de71),
                Rgba::hex(0xa882ff),
                Rgba::hex(0xfa99cd),
            ],
            radius: 4.0,
        }
    }

    fn limu(&self) -> LimuTheme {
        LimuTheme {
            bg: self.background.color(),
            fg: self.text.color(),
            muted: self.muted.color(),
            line: self.line.color(),
            surface: self.surface.color(),
            surface_alt: self.surface_alt.color(),
            border: self.border.color(),
            accent: self.accent.color(),
            critical: self.critical.color(),
            series: self.series.map(Rgba::color),
            radius: self.radius,
            stroke_width: 1.0,
        }
    }

    fn key(&self) -> Vec<u32> {
        let c = |c: Rgba| u32::from_be_bytes([c.r, c.g, c.b, c.a]);
        let mut k = vec![
            c(self.background),
            c(self.text),
            c(self.muted),
            c(self.line),
            c(self.surface),
            c(self.surface_alt),
            c(self.border),
            c(self.accent),
            c(self.critical),
            c(self.error),
            self.radius.to_bits() as u32,
        ];
        k.extend(self.series.map(c));
        k
    }
}

/// A diagram laid out and coloured: its size in CSS pixels at
/// [`FONT_SIZE`], and what to draw, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagram {
    /// The kind of diagram, as limu names it: "flowchart", "sequence"…
    pub kind: String,
    pub width: f64,
    pub height: f64,
    pub shapes: Vec<Shape>,
}

/// One thing drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A path, filled and then stroked, with what is given of the two. A
    /// path under a rotation has had it applied to its points already.
    Path {
        segments: Vec<Segment>,
        fill: Option<Rgba>,
        stroke: Option<Stroke>,
    },
    /// A line of text: its spans set one after another in the painter's
    /// faces from `x` as `anchor` says, on the baseline `y`.
    Text(TextRun),
}

/// A step of a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Quad(f64, f64, f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub color: Rgba,
    pub width: f64,
    /// Dash and gap lengths in turn; empty is solid.
    pub dash: Vec<f64>,
    pub cap: LineCap,
    pub join: LineJoin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub x: f64,
    pub y: f64,
    pub anchor: Anchor,
    pub spans: Vec<TextSpan>,
    pub color: Rgba,
    /// Degrees clockwise about `(x, y)`, for the few labels a diagram turns
    /// (an axis title); 0 for nearly all.
    pub rotate: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub text: String,
    /// Pixels.
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub underline: bool,
    /// How wide layout measured the span (Arial's advances), for a painter
    /// that wants to place spans where layout left room for them.
    pub measured: f64,
    /// A sub- or superscript's shift of the baseline, in pixels down.
    pub shift: f64,
}

/// What a diagram that does not lay out shows: Pele's message, drawn by
/// Obsidian in the monospace face at 0.8 em in `--text-error`, the lines as
/// they are (`white-space: pre-wrap`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MermaidError {
    pub message: String,
}

impl std::fmt::Display for MermaidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MermaidError {}

pub type Result = std::result::Result<Arc<Diagram>, MermaidError>;

/// Whether a fence's info string is a Mermaid diagram: its first word is
/// `mermaid`, as Obsidian's code block processor is registered for.
pub fn is_mermaid(language: Option<&str>) -> bool {
    language.is_some_and(|l| l == "mermaid")
}

/// Lays out `source` for a note that gives it `max_width` pixels and draws
/// it in `theme`. Cached on all three.
pub fn render(source: &str, max_width: f64, theme: &Theme) -> Result {
    // A width is a layout input only through which side of a breakpoint or
    // a diagram's natural width it falls on; whole pixels are plenty.
    let width = if max_width.is_finite() {
        max_width.floor().max(1.0)
    } else {
        f64::INFINITY
    };
    let key = (source.to_string(), width.to_bits(), theme.key());
    type Key = (String, u64, Vec<u32>);
    static CACHE: OnceLock<Mutex<HashMap<Key, Result>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let out = render_uncached(source, width, theme);
    let mut map = cache.lock().unwrap();
    if map.len() > 512 {
        map.clear();
    }
    map.insert(key, out.clone());
    out
}

fn render_uncached(source: &str, max_width: f64, theme: &Theme) -> Result {
    thread_local! {
        static SHAPER: Option<fonts::Shaper> = fonts::fonts().map(fonts::Shaper::new);
    }
    let scene = SHAPER.with(|shaper| {
        let mut opts = LayoutOptions {
            max_width,
            ..LayoutOptions::default()
        };
        if let Some(shaper) = shaper {
            opts.measurer = shaper;
        }
        limu::layout(source, &opts)
    });
    let scene = scene.map_err(|e| MermaidError {
        message: e.to_string(),
    })?;
    let look = Look::new(theme.limu());
    let mut list = List::default();
    limu::scene::draw(&scene, &look, &mut list);
    Ok(Arc::new(Diagram {
        kind: scene.diagram.clone(),
        width: scene.width,
        height: scene.height,
        shapes: list.shapes,
    }))
}

/// A limu canvas that records what it is asked to draw.
#[derive(Default)]
struct List {
    shapes: Vec<Shape>,
    /// The rotations in force, innermost last: degrees and centre.
    turns: Vec<(f64, Point)>,
}

impl List {
    fn turn(&self, x: f64, y: f64) -> (f64, f64) {
        let (mut x, mut y) = (x, y);
        for (deg, c) in self.turns.iter().rev() {
            let (s, k) = deg.to_radians().sin_cos();
            let (dx, dy) = (x - c.x, y - c.y);
            x = c.x + dx * k - dy * s;
            y = c.y + dx * s + dy * k;
        }
        (x, y)
    }

    fn segments(&self, path: &Path) -> Vec<Segment> {
        let p = |q: Point| self.turn(q.x, q.y);
        path.steps()
            .map(|s| match s {
                Step::MoveTo(a) => {
                    let (x, y) = p(a);
                    Segment::Move(x, y)
                }
                Step::LineTo(a) => {
                    let (x, y) = p(a);
                    Segment::Line(x, y)
                }
                Step::QuadTo(c, a) => {
                    let ((cx, cy), (x, y)) = (p(c), p(a));
                    Segment::Quad(cx, cy, x, y)
                }
                Step::CubeTo(c1, c2, a) => {
                    let ((ax, ay), (bx, by), (x, y)) = (p(c1), p(c2), p(a));
                    Segment::Cubic(ax, ay, bx, by, x, y)
                }
                Step::Close => Segment::Close,
            })
            .collect()
    }
}

fn ink(i: Option<&Ink>) -> Option<Rgba> {
    i.filter(|i| i.visible()).map(|i| Rgba::of(i.color))
}

fn stroke(pen: Option<&Pen<'_>>) -> Option<Stroke> {
    let pen = pen.filter(|p| p.ink.visible() && p.width > 0.0)?;
    Some(Stroke {
        color: Rgba::of(pen.ink.color),
        width: pen.width,
        dash: pen.dash.to_vec(),
        cap: match pen.cap {
            Cap::Butt => LineCap::Butt,
            Cap::Round => LineCap::Round,
            Cap::Square => LineCap::Square,
        },
        join: match pen.join {
            Join::Miter => LineJoin::Miter,
            Join::Round => LineJoin::Round,
            Join::Bevel => LineJoin::Bevel,
        },
    })
}

impl Canvas for List {
    fn begin_rotate(&mut self, degrees: f64, about: Point) {
        self.turns.push((degrees, about));
    }

    fn end_rotate(&mut self) {
        self.turns.pop();
    }

    fn path(&mut self, path: &Path, fill: Option<&Ink>, pen: Option<&Pen<'_>>) {
        let (fill, stroke) = (ink(fill), stroke(pen));
        if fill.is_none() && stroke.is_none() {
            return;
        }
        self.shapes.push(Shape::Path {
            segments: self.segments(path),
            fill,
            stroke,
        });
    }

    fn text(&mut self, run: &Run<'_>) {
        // A halo is the stroke some labels get under them; the painters
        // here cannot stroke glyphs, and the label reads without it.
        let TextPaint::Fill(ink) = run.paint else {
            return;
        };
        if !ink.visible() {
            return;
        }
        use limu::scene::text::Face;
        let (x, y) = self.turn(run.at.x, run.at.y);
        let rotate: f64 = self.turns.iter().map(|t| t.0).sum();
        let spans = run
            .spans
            .iter()
            .filter(|s| !s.text.is_empty())
            .map(|s| {
                let size = s.face.size_at(run.size);
                let shift = if s.face.contains(Face::SUB) {
                    run.size * 0.2
                } else if s.face.contains(Face::SUP) {
                    -run.size * 0.35
                } else {
                    0.0
                };
                TextSpan {
                    text: s.text.clone(),
                    size,
                    bold: s.face.contains(Face::BOLD),
                    italic: s.face.contains(Face::ITALIC),
                    mono: s.face.contains(Face::MONO),
                    underline: s.face.contains(Face::UNDERLINE),
                    measured: s.width,
                    shift,
                }
            })
            .collect::<Vec<_>>();
        if spans.is_empty() {
            return;
        }
        self.shapes.push(Shape::Text(TextRun {
            x,
            y,
            anchor: match run.anchor {
                Align::Start => Anchor::Start,
                Align::Middle => Anchor::Middle,
                Align::End => Anchor::End,
            },
            spans,
            color: Rgba::of(ink.color),
            rotate,
        }));
    }
}

#[cfg(test)]
mod tests;
