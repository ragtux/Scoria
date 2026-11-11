//! Blocks laid out on pages: the measuring, the line breaking and the
//! pagination both exports share.
//!
//! Everything here is in CSS pixels, the reading view's own units, with the
//! origin at the top left of the content box and `y` growing down. A
//! backend scales the result to points (the PDF) or device pixels (the
//! PNG). The look follows Scoria's reading view: 16 px text on a 1.5 line,
//! Obsidian's heading scale, a block gap of 0.9 em.
//!
//! A container (a quote, a callout, a code block, a list item) is a pair
//! of insets plus a decoration, and its decoration is drawn **per slice**:
//! every line or gap placed while it is open paints its own strip of the
//! container's bar or fill. A container broken across a page therefore
//! needs nothing special: each page gets the strips of the lines it holds.

use crate::doc::{Align, Block, Color, Inline, ListItem, Marker, Palette, SpanStyle};
use crate::fonts::{Face, Fonts};
use std::ops::Range;
use std::sync::Arc;

/// The body text size, CSS px.
pub const BASE: f32 = 16.0;
/// The body line height, a multiple of the size.
pub const LINE_HEIGHT: f32 = 1.5;
/// The gap after a paragraph, a list, a code block: the reading view's.
pub const BLOCK_GAP: f32 = BASE * 0.9;
/// Inline code and code blocks are set at `--font-smaller`.
pub const CODE_SCALE: f32 = 0.875;
/// Math is set a little larger than the text around it, as KaTeX does.
pub const MATH_SCALE: f32 = 1.1;
/// How far a list's or a quote's content is indented.
pub const LIST_INDENT: f32 = 28.0;
pub const QUOTE_INDENT: f32 = 24.0;

/// Obsidian's heading scale: size multiple, bold, line height.
pub fn heading_style(level: u8) -> (f32, bool, f32) {
    match level {
        1 => (1.618, true, 1.2),
        2 => (1.462, false, 1.2),
        3 => (1.318, false, 1.3),
        4 => (1.188, false, 1.3),
        5 => (1.076, false, 1.3),
        _ => (1.0, false, 1.3),
    }
}

/// One glyph of a run: its id, its pen position from the run's origin, its
/// advance and the bytes of the run's text it stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    pub id: u16,
    pub x: f32,
    pub advance: f32,
    pub range: Range<usize>,
}

/// Glyphs of one face, size and colour on one baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    pub face: Face,
    pub size: f32,
    /// The pen's start on the baseline.
    pub x: f32,
    pub y: f32,
    pub color: Color,
    pub glyphs: Vec<PlacedGlyph>,
    pub text: String,
}

impl GlyphRun {
    pub fn width(&self) -> f32 {
        self.glyphs.last().map_or(0.0, |g| g.x + g.advance)
    }
}

/// A path step, absolute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathOp {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

/// One drawing instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Text(GlyphRun),
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Color,
        radius: f32,
    },
    /// A rectangle's outline.
    Frame {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Color,
        radius: f32,
        width: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        color: Color,
    },
    /// A filled shape, or a stroked one `stroke` wide.
    Path {
        ops: Vec<PathOp>,
        fill: bool,
        stroke: f32,
        color: Color,
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        data: Arc<Vec<u8>>,
    },
    /// A clickable area to a URL; draws nothing.
    Link {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        url: String,
    },
}

impl Item {
    /// The item moved by `(dx, dy)`.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Item::Text(run) => {
                run.x += dx;
                run.y += dy;
            }
            Item::Rect { x, y, .. }
            | Item::Frame { x, y, .. }
            | Item::Image { x, y, .. }
            | Item::Link { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Item::Line { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Item::Path { ops, .. } => {
                for op in ops {
                    match op {
                        PathOp::Move(x, y) | PathOp::Line(x, y) => {
                            *x += dx;
                            *y += dy;
                        }
                        PathOp::Quad(a, b, x, y) => {
                            *a += dx;
                            *b += dy;
                            *x += dx;
                            *y += dy;
                        }
                        PathOp::Cubic(a, b, c, d, x, y) => {
                            *a += dx;
                            *b += dy;
                            *c += dx;
                            *d += dy;
                            *x += dx;
                            *y += dy;
                        }
                        PathOp::Close => {}
                    }
                }
            }
        }
    }
}

/// A laid-out page: what to draw, backgrounds first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub items: Vec<Item>,
}

impl Page {
    /// The page's text, runs in drawing order joined by spaces: for tests.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            if let Item::Text(run) = item {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&run.text);
            }
        }
        out
    }
}

// ---- inline layout -------------------------------------------------------------

/// The text settings a block lays its inlines out with.
#[derive(Debug, Clone, Copy)]
struct TextCtx {
    size: f32,
    line_height: f32,
    /// Headings are semibold or bold throughout.
    weight: Weight,
    color: Color,
    /// Code blocks keep every space and break anywhere.
    code: bool,
    align: Align,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Weight {
    Regular,
    SemiBold,
    Bold,
}

impl TextCtx {
    fn body(palette: &Palette) -> TextCtx {
        TextCtx {
            size: BASE,
            line_height: LINE_HEIGHT,
            weight: Weight::Regular,
            color: palette.text,
            code: false,
            align: Align::Left,
        }
    }
}

#[derive(Debug, Clone)]
struct Shaped {
    face: Face,
    id: u16,
    advance: f32,
    ch: char,
}

#[derive(Debug, Clone)]
enum PieceKind {
    Word(Vec<Shaped>),
    Space,
    Math(Arc<scoria_math::Formula>),
    Break,
}

#[derive(Debug, Clone)]
struct Piece {
    kind: PieceKind,
    width: f32,
    size: f32,
    /// Above and below the baseline, leading included.
    ascent: f32,
    descent: f32,
    color: Color,
    style: SpanStyle,
    link: Option<Arc<str>>,
    /// Raised this far (superscript).
    rise: f32,
}

/// A line of inlines: drawn relative to its left edge and its baseline.
#[derive(Debug, Clone, Default)]
pub(crate) struct Line {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub back: Vec<Item>,
    pub front: Vec<Item>,
}

impl Line {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

pub(crate) struct Typesetter<'a> {
    pub fonts: &'a Fonts,
    pub palette: &'a Palette,
}

impl Typesetter<'_> {
    fn face_for(&self, ctx: &TextCtx, style: &SpanStyle) -> Face {
        if style.code || ctx.code {
            return Face::Mono;
        }
        let bold = style.bold || ctx.weight == Weight::Bold;
        match (bold, ctx.weight == Weight::SemiBold, style.italic) {
            (true, _, i) => Face::text(true, i),
            (false, true, false) => Face::SemiBold,
            (false, true, true) => Face::BoldItalic,
            (false, false, i) => Face::text(false, i),
        }
    }

    /// The leading model of a text piece of size `size`: the line box is
    /// `size * line_height`, the glyphs sit centred in it.
    fn text_extent(&self, size: f32, line_height: f32) -> (f32, f32) {
        let (asc, desc) = self.fonts.vertical(Face::Regular);
        let content = (asc + desc) * size;
        let half = (size * line_height - content) / 2.0;
        (asc * size + half, desc * size + half)
    }

    fn shape_word(&self, word: &str, face: Face, size: f32) -> (Vec<Shaped>, f32) {
        let mut out: Vec<Shaped> = Vec::new();
        let mut width = 0.0;
        for ch in word.chars() {
            let (face, id) = self.fonts.resolve(face, ch);
            let mut advance = self.fonts.advance(face, id) * size;
            if let Some(prev) = out.last_mut() {
                if prev.face == face {
                    let k = self.fonts.kern(face, prev.id, id) * size;
                    prev.advance += k;
                    width += k;
                }
            }
            if ch == '\t' {
                let (f, space) = self.fonts.resolve(face, ' ');
                advance = self.fonts.advance(f, space) * size * 4.0;
            }
            width += advance;
            out.push(Shaped {
                face,
                id,
                advance,
                ch,
            });
        }
        (out, width)
    }

    /// The pieces of some inlines: words, spaces, formulas and breaks.
    fn pieces(&self, inlines: &[Inline], ctx: &TextCtx) -> Vec<Piece> {
        let mut out = Vec::new();
        for inline in inlines {
            match inline {
                Inline::Break => out.push(Piece {
                    kind: PieceKind::Break,
                    width: 0.0,
                    size: ctx.size,
                    ascent: 0.0,
                    descent: 0.0,
                    color: ctx.color,
                    style: SpanStyle::default(),
                    link: None,
                    rise: 0.0,
                }),
                Inline::Math(tex) => {
                    let size = ctx.size * MATH_SCALE;
                    match scoria_math::typeset(tex, false) {
                        Ok(formula) => {
                            let (a, d) = self.text_extent(ctx.size, ctx.line_height);
                            out.push(Piece {
                                width: formula.width as f32 * size,
                                ascent: (formula.height as f32 * size).max(a),
                                descent: (formula.depth as f32 * size).max(d),
                                kind: PieceKind::Math(formula),
                                size,
                                color: ctx.color,
                                style: SpanStyle::default(),
                                link: None,
                                rise: 0.0,
                            });
                        }
                        Err(_) => {
                            let style = SpanStyle {
                                color: Some(self.palette.math_error),
                                ..SpanStyle::default()
                            };
                            self.text_pieces(tex, &style, None, ctx, &mut out);
                        }
                    }
                }
                Inline::Text { text, style, link } => {
                    let link: Option<Arc<str>> = link.as_deref().map(Arc::from);
                    self.text_pieces(text, style, link, ctx, &mut out);
                }
            }
        }
        out
    }

    fn text_pieces(
        &self,
        text: &str,
        style: &SpanStyle,
        link: Option<Arc<str>>,
        ctx: &TextCtx,
        out: &mut Vec<Piece>,
    ) {
        let mut size = if style.code && !ctx.code {
            ctx.size * CODE_SCALE
        } else {
            ctx.size
        };
        let mut rise = 0.0;
        if style.superscript {
            size *= 0.75;
            rise = ctx.size * 0.35;
        }
        let face = self.face_for(ctx, style);
        let color = style.color.unwrap_or(if link.is_some() {
            self.palette.link
        } else {
            ctx.color
        });
        // The line box is the paragraph's, whatever this run's size.
        let (ascent, descent) = self.text_extent(ctx.size, ctx.line_height);
        let piece = |kind: PieceKind, width: f32| Piece {
            kind,
            width,
            size,
            ascent,
            descent,
            color,
            style: *style,
            link: link.clone(),
            rise,
        };
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut Vec<Piece>| {
            if !word.is_empty() {
                let (glyphs, width) = self.shape_word(word, face, size);
                out.push(piece(PieceKind::Word(glyphs), width));
                word.clear();
            }
        };
        for ch in text.chars() {
            match ch {
                '\n' => {
                    flush(&mut word, out);
                    if ctx.code {
                        out.push(piece(PieceKind::Break, 0.0));
                    } else if !matches!(out.last().map(|p| &p.kind), Some(PieceKind::Space)) {
                        // A soft break the model kept is a line break in
                        // the reading view.
                        out.push(piece(PieceKind::Break, 0.0));
                    }
                }
                ' ' | '\t' if !ctx.code || ch == ' ' => {
                    flush(&mut word, out);
                    let (f, id) = self.fonts.resolve(face, ' ');
                    let width = self.fonts.advance(f, id) * size;
                    // Runs of spaces collapse outside code.
                    if ctx.code || !matches!(out.last().map(|p| &p.kind), Some(PieceKind::Space)) {
                        out.push(piece(PieceKind::Space, width));
                    }
                }
                _ => word.push(ch),
            }
        }
        flush(&mut word, out);
    }

    /// Breaks pieces into lines no wider than `width`.
    fn break_lines(&self, pieces: Vec<Piece>, width: f32, ctx: &TextCtx) -> Vec<Line> {
        let mut lines: Vec<Vec<Piece>> = vec![Vec::new()];
        let mut x = 0.0;
        for piece in pieces {
            match piece.kind {
                PieceKind::Break => {
                    // An empty line still has the paragraph's height.
                    let current = lines.last_mut().expect("one line");
                    current.push(piece);
                    lines.push(Vec::new());
                    x = 0.0;
                }
                PieceKind::Space => {
                    let wrapped = lines.len() > 1;
                    let current = lines.last_mut().expect("one line");
                    // A wrapped line does not start with a space, except in
                    // code, whose indentation is its meaning.
                    if current.is_empty() && !ctx.code && wrapped {
                        continue;
                    }
                    x += piece.width;
                    current.push(piece);
                }
                _ => {
                    if x + piece.width > width + 0.01
                        && lines.last().is_some_and(|l| l.iter().any(is_ink))
                    {
                        lines.push(Vec::new());
                        x = 0.0;
                    }
                    if piece.width > width + 0.01 {
                        // A word wider than the line is broken anywhere.
                        for part in split_piece(piece, width - x, width) {
                            if x + part.width > width + 0.01
                                && lines.last().is_some_and(|l| l.iter().any(is_ink))
                            {
                                lines.push(Vec::new());
                                x = 0.0;
                            }
                            x += part.width;
                            lines.last_mut().expect("one line").push(part);
                        }
                    } else {
                        x += piece.width;
                        lines.last_mut().expect("one line").push(piece);
                    }
                }
            }
        }
        // A trailing break leaves an empty last line, which a paragraph
        // does not draw.
        if lines.len() > 1 && lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        let (a, d) = self.text_extent(ctx.size, ctx.line_height);
        lines
            .into_iter()
            .map(|pieces| self.place_line(pieces, width, ctx, (a, d)))
            .collect()
    }

    fn place_line(
        &self,
        mut pieces: Vec<Piece>,
        width: f32,
        ctx: &TextCtx,
        min: (f32, f32),
    ) -> Line {
        // Trailing spaces neither count nor draw.
        while !ctx.code
            && pieces
                .last()
                .is_some_and(|p| matches!(p.kind, PieceKind::Space | PieceKind::Break))
        {
            pieces.pop();
        }
        let mut line = Line {
            ascent: min.0,
            descent: min.1,
            ..Line::default()
        };
        for p in &pieces {
            line.ascent = line.ascent.max(p.ascent);
            line.descent = line.descent.max(p.descent);
        }
        let total: f32 = pieces.iter().map(|p| p.width).sum();
        let mut x = match ctx.align {
            Align::Left => 0.0,
            Align::Center => ((width - total) / 2.0).max(0.0),
            Align::Right => (width - total).max(0.0),
        };
        let (text_a, text_d) = self.fonts.vertical(Face::Regular);
        // Pieces sharing a style and a link form one span: its background,
        // its rules and its link area are drawn once across it.
        let mut ix = 0;
        while ix < pieces.len() {
            let style = pieces[ix].style;
            let link = pieces[ix].link.clone();
            let start_x = x;
            let mut end_x = x;
            let mut end = ix;
            let mut inked = false;
            while end < pieces.len()
                && pieces[end].style == style
                && pieces[end].link == link
                && !matches!(pieces[end].kind, PieceKind::Break)
            {
                let p = &pieces[end];
                self.draw_piece(p, x, &mut line.front);
                x += p.width;
                if !matches!(p.kind, PieceKind::Space) || ctx.code {
                    end_x = x;
                    inked = true;
                }
                end += 1;
            }
            if end == ix {
                // A break: nothing to draw.
                ix += 1;
                continue;
            }
            let p = &pieces[ix];
            ix = end;
            if !inked {
                continue;
            }
            let span_w = end_x - start_x;
            let size = p.size;
            let top = -(text_a * size) - p.rise;
            let h = (text_a + text_d) * size;
            if style.code {
                line.back.push(Item::Rect {
                    x: start_x - 2.0,
                    y: top - 1.0,
                    w: span_w + 4.0,
                    h: h + 2.0,
                    color: self.palette.code_background,
                    radius: 3.0,
                });
            }
            if let Some(bg) = style.background {
                line.back.push(Item::Rect {
                    x: start_x,
                    y: top,
                    w: span_w,
                    h,
                    color: bg,
                    radius: 0.0,
                });
            }
            let color = style.color.unwrap_or(if link.is_some() {
                self.palette.link
            } else {
                ctx.color
            });
            let thickness = (size / 16.0).max(0.75);
            if style.strike {
                let y = -size * 0.3 - p.rise;
                line.front.push(Item::Line {
                    x1: start_x,
                    y1: y,
                    x2: end_x,
                    y2: y,
                    width: thickness,
                    color,
                });
            }
            if style.underline || link.is_some() {
                let y = size * 0.12;
                line.front.push(Item::Line {
                    x1: start_x,
                    y1: y,
                    x2: end_x,
                    y2: y,
                    width: thickness,
                    color,
                });
            }
            if let Some(url) = &link {
                line.front.push(Item::Link {
                    x: start_x,
                    y: top,
                    w: span_w,
                    h,
                    url: url.to_string(),
                });
            }
        }
        line.width = x;
        line
    }

    /// A word's glyphs as runs, one per face, its pen starting at `x`; a
    /// formula's drawing list.
    fn draw_piece(&self, p: &Piece, x: f32, out: &mut Vec<Item>) {
        match &p.kind {
            PieceKind::Word(glyphs) => {
                let mut run: Option<GlyphRun> = None;
                let mut pen = x;
                for g in glyphs {
                    if !run.as_ref().is_some_and(|r| r.face == g.face) {
                        if let Some(r) = run.take() {
                            out.push(Item::Text(r));
                        }
                        run = Some(GlyphRun {
                            face: g.face,
                            size: p.size,
                            x: pen,
                            y: -p.rise,
                            color: p.color,
                            glyphs: Vec::new(),
                            text: String::new(),
                        });
                    }
                    let r = run.as_mut().expect("a run");
                    let start = r.text.len();
                    r.text.push(g.ch);
                    r.glyphs.push(PlacedGlyph {
                        id: g.id,
                        x: pen - r.x,
                        advance: g.advance,
                        range: start..r.text.len(),
                    });
                    pen += g.advance;
                }
                if let Some(r) = run.take() {
                    out.push(Item::Text(r));
                }
            }
            PieceKind::Math(formula) => math_items(formula, x, -p.rise, p.size, p.color, out),
            PieceKind::Space | PieceKind::Break => {}
        }
    }
}

fn is_ink(p: &Piece) -> bool {
    matches!(p.kind, PieceKind::Word(_) | PieceKind::Math(_))
}

/// A word too wide for a line, cut into parts: the first `first` wide at
/// most, the rest `width` at most. A part holds at least one glyph.
fn split_piece(piece: Piece, first: f32, width: f32) -> Vec<Piece> {
    let PieceKind::Word(glyphs) = &piece.kind else {
        return vec![piece];
    };
    let mut parts = Vec::new();
    let mut current: Vec<Shaped> = Vec::new();
    let mut w = 0.0;
    let mut limit = if first > 0.0 { first } else { width };
    for g in glyphs {
        if w + g.advance > limit + 0.01 && !current.is_empty() {
            parts.push(Piece {
                kind: PieceKind::Word(std::mem::take(&mut current)),
                width: w,
                ..piece.clone()
            });
            w = 0.0;
            limit = width;
        }
        w += g.advance;
        current.push(g.clone());
    }
    if !current.is_empty() {
        parts.push(Piece {
            kind: PieceKind::Word(current),
            width: w,
            ..piece.clone()
        });
    }
    parts
}

/// A formula's drawing list as items, its box's left edge at `x` and its
/// baseline at `baseline`, at `size` px per em.
pub(crate) fn math_items(
    formula: &scoria_math::Formula,
    x: f32,
    baseline: f32,
    size: f32,
    color: Color,
    out: &mut Vec<Item>,
) {
    use scoria_math::Item as M;
    let top = baseline - formula.height as f32 * size;
    let at = |ex: f64, ey: f64| (x + ex as f32 * size, top + ey as f32 * size);
    let colour = |c: &Option<scoria_math::Rgba>| match c {
        Some(c) => Color::from_f32(c.r, c.g, c.b, c.a),
        None => color,
    };
    for item in &formula.items {
        match item {
            M::Glyph {
                x: gx,
                y: gy,
                size: gs,
                face,
                ch,
                color: c,
            } => {
                let (px, py) = at(*gx, *gy);
                out.push(Item::Text(GlyphRun {
                    face: Face::Math(face.file()),
                    size: *gs as f32 * size,
                    x: px,
                    y: py,
                    color: colour(c),
                    glyphs: vec![PlacedGlyph {
                        id: u16::MAX, // resolved by the caller: see `resolve_math`
                        x: 0.0,
                        advance: 0.0,
                        range: 0..ch.len_utf8(),
                    }],
                    text: ch.to_string(),
                }));
            }
            M::Line {
                x: lx,
                y: ly,
                width,
                thickness,
                color: c,
                ..
            } => {
                let (px, py) = at(*lx, *ly);
                out.push(Item::Rect {
                    x: px,
                    y: py,
                    w: *width as f32 * size,
                    h: (*thickness as f32 * size).max(0.5),
                    color: colour(c),
                    radius: 0.0,
                });
            }
            M::Rect {
                x: rx,
                y: ry,
                width,
                height,
                color: c,
            } => {
                let (px, py) = at(*rx, *ry);
                out.push(Item::Rect {
                    x: px,
                    y: py,
                    w: *width as f32 * size,
                    h: *height as f32 * size,
                    color: colour(c),
                    radius: 0.0,
                });
            }
            M::Path {
                x: ox,
                y: oy,
                commands,
                fill,
                color: c,
            } => {
                let (bx, by) = at(*ox, *oy);
                let p = |cx: f64, cy: f64| (bx + cx as f32 * size, by + cy as f32 * size);
                let mut ops = Vec::with_capacity(commands.len());
                for cmd in commands {
                    use scoria_math::PathCommand as C;
                    ops.push(match *cmd {
                        C::MoveTo { x, y } => {
                            let (a, b) = p(x, y);
                            PathOp::Move(a, b)
                        }
                        C::LineTo { x, y } => {
                            let (a, b) = p(x, y);
                            PathOp::Line(a, b)
                        }
                        C::QuadTo { x1, y1, x, y } => {
                            let (a, b) = p(x1, y1);
                            let (c2, d) = p(x, y);
                            PathOp::Quad(a, b, c2, d)
                        }
                        C::CubicTo {
                            x1,
                            y1,
                            x2,
                            y2,
                            x,
                            y,
                        } => {
                            let (a, b) = p(x1, y1);
                            let (c2, d) = p(x2, y2);
                            let (e, f) = p(x, y);
                            PathOp::Cubic(a, b, c2, d, e, f)
                        }
                        C::Close => PathOp::Close,
                    });
                }
                out.push(Item::Path {
                    ops,
                    fill: *fill,
                    stroke: scoria_math::STROKE_WIDTH as f32 * size,
                    color: colour(c),
                });
            }
        }
    }
}

// ---- the flow ------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Deco {
    None,
    /// A vertical bar from `x`, `w` wide.
    Bar {
        x: f32,
        w: f32,
        color: Color,
    },
    /// A fill from `x0` to `x1`.
    Fill {
        x0: f32,
        x1: f32,
        color: Color,
    },
}

#[derive(Debug, Clone, Copy)]
struct Container {
    left: f32,
    right: f32,
    deco: Deco,
}

/// Places blocks down the page and onto new ones.
pub(crate) struct Flow<'a> {
    ts: Typesetter<'a>,
    width: f32,
    /// The page's content height; `None` is one endless page.
    height: Option<f32>,
    pages: Vec<(Vec<Item>, Vec<Item>)>,
    y: f32,
    /// The collapsed margin waiting above the next block.
    pending: f32,
    stack: Vec<Container>,
    marker: Option<Marker>,
}

impl<'a> Flow<'a> {
    pub fn new(
        fonts: &'a Fonts,
        palette: &'a Palette,
        width: f32,
        height: Option<f32>,
    ) -> Flow<'a> {
        Flow {
            ts: Typesetter { fonts, palette },
            width,
            height,
            pages: vec![(Vec::new(), Vec::new())],
            y: 0.0,
            pending: 0.0,
            stack: Vec::new(),
            marker: None,
        }
    }

    pub fn finish(self) -> (Vec<Page>, f32) {
        let y = self.y;
        let pages = self
            .pages
            .into_iter()
            .map(|(mut back, front)| {
                back.extend(front);
                Page { items: back }
            })
            .collect();
        (pages, y)
    }

    fn left(&self) -> f32 {
        self.stack.last().map_or(0.0, |c| c.left)
    }

    fn right(&self) -> f32 {
        self.width - self.stack.last().map_or(0.0, |c| c.right)
    }

    fn inner_width(&self) -> f32 {
        (self.right() - self.left()).max(BASE)
    }

    fn at_top(&self) -> bool {
        self.y <= 0.0
    }

    fn gap(&mut self, h: f32) {
        self.pending = self.pending.max(h);
    }

    fn new_page(&mut self) {
        self.pages.push((Vec::new(), Vec::new()));
        self.y = 0.0;
    }

    /// Paints the open containers' slices for `[y, y + h)`.
    fn decorate(&mut self, y: f32, h: f32) {
        if h <= 0.0 {
            return;
        }
        let page = self.pages.last_mut().expect("a page");
        for c in &self.stack {
            match c.deco {
                Deco::None => {}
                Deco::Bar { x, w, color } => page.0.push(Item::Rect {
                    x,
                    y,
                    w,
                    h,
                    color,
                    radius: 0.0,
                }),
                Deco::Fill { x0, x1, color } => page.0.push(Item::Rect {
                    x: x0,
                    y,
                    w: x1 - x0,
                    h,
                    color,
                    radius: 0.0,
                }),
            }
        }
    }

    /// Takes the pending margin: dropped at the top of a page, a page
    /// break when it would not fit.
    fn settle(&mut self) {
        let gap = std::mem::take(&mut self.pending);
        if gap <= 0.0 || self.at_top() {
            return;
        }
        if let Some(height) = self.height {
            if self.y + gap > height {
                self.new_page();
                return;
            }
        }
        let y = self.y;
        self.decorate(y, gap);
        self.y += gap;
    }

    /// Room for `h` more: the page's top `y`, after a page break if the
    /// page has no room left and is not empty.
    fn place(&mut self, h: f32) -> f32 {
        self.settle();
        if let Some(height) = self.height {
            if self.y + h > height && !self.at_top() {
                self.new_page();
            }
        }
        let y = self.y;
        self.decorate(y, h);
        self.y += h;
        y
    }

    /// Fixed space inside a container: its padding.
    fn pad(&mut self, h: f32) {
        self.pending = 0.0;
        if let Some(height) = self.height {
            if self.y + h > height {
                self.new_page();
                return;
            }
        }
        let y = self.y;
        self.decorate(y, h);
        self.y += h;
    }

    fn push(&mut self, left: f32, right: f32, deco: Deco) {
        self.settle();
        let (l, r) = (
            self.left() + left,
            self.stack.last().map_or(0.0, |c| c.right) + right,
        );
        self.stack.push(Container {
            left: l,
            right: r,
            deco,
        });
    }

    fn pop(&mut self) {
        self.pending = 0.0;
        self.stack.pop();
    }

    fn emit_back(&mut self, item: Item) {
        self.pages.last_mut().expect("a page").0.push(item);
    }

    fn emit(&mut self, item: Item) {
        self.pages.last_mut().expect("a page").1.push(item);
    }

    /// Draws a waiting list marker against a line whose baseline is at
    /// `baseline`.
    fn draw_marker(&mut self, baseline: f32, size: f32) {
        let Some(marker) = self.marker.take() else {
            return;
        };
        let left = self.left();
        let palette = self.ts.palette;
        match marker {
            Marker::Bullet => {
                let r = 2.5;
                let cx = left - LIST_INDENT / 2.0 + 2.0;
                let cy = baseline - size * 0.33;
                self.emit(Item::Rect {
                    x: cx - r,
                    y: cy - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                    color: palette.muted,
                    radius: r,
                });
            }
            Marker::Number(n) => {
                let ctx = TextCtx {
                    align: Align::Right,
                    ..TextCtx::body(palette)
                };
                let pieces = self
                    .ts
                    .pieces(&[Inline::plain(format!("{n}."))], &TextCtx { size, ..ctx });
                let lines = self.ts.break_lines(pieces, LIST_INDENT - 6.0, &ctx);
                if let Some(line) = lines.into_iter().next() {
                    for mut item in line.front {
                        item.translate(left - LIST_INDENT, baseline);
                        self.emit(item);
                    }
                }
            }
            Marker::Task { done } => {
                let s = 13.0;
                let x = left - LIST_INDENT / 2.0 - s / 2.0 + 2.0;
                let y = baseline - size * 0.36 - s / 2.0;
                if done {
                    self.emit(Item::Rect {
                        x,
                        y,
                        w: s,
                        h: s,
                        color: palette.accent,
                        radius: 3.0,
                    });
                    self.emit(Item::Path {
                        ops: vec![
                            PathOp::Move(x + s * 0.25, y + s * 0.52),
                            PathOp::Line(x + s * 0.43, y + s * 0.7),
                            PathOp::Line(x + s * 0.76, y + s * 0.32),
                        ],
                        fill: false,
                        stroke: 1.8,
                        color: palette.background,
                    });
                } else {
                    self.emit(Item::Frame {
                        x,
                        y,
                        w: s,
                        h: s,
                        color: palette.faint,
                        radius: 3.0,
                        width: 1.0,
                    });
                }
            }
        }
    }

    /// Lays out and places a run of inlines, one line at a time.
    fn inlines(&mut self, inlines: &[Inline], ctx: &TextCtx) {
        let width = self.inner_width();
        let pieces = self.ts.pieces(inlines, ctx);
        let lines = self.ts.break_lines(pieces, width, ctx);
        for line in lines {
            self.line(line, ctx.size);
        }
    }

    fn line(&mut self, line: Line, size: f32) {
        let top = self.place(line.height());
        let baseline = top + line.ascent;
        let left = self.left();
        self.draw_marker(baseline, size);
        for mut item in line.back {
            item.translate(left, baseline);
            self.emit_back(item);
        }
        for mut item in line.front {
            item.translate(left, baseline);
            self.emit(item);
        }
    }

    /// Something that is not a line of text: the marker sits on its first
    /// line's height.
    fn marker_at_top(&mut self, top: f32) {
        if self.marker.is_some() {
            let (a, _) = self.ts.text_extent(BASE, LINE_HEIGHT);
            self.draw_marker(top + a, BASE);
        }
    }

    pub fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.block(block, false);
        }
    }

    fn block(&mut self, block: &Block, tight: bool) {
        let palette = self.ts.palette;
        let gap = if tight { 0.0 } else { BLOCK_GAP };
        match block {
            Block::Paragraph(inlines) => {
                self.inlines(inlines, &TextCtx::body(palette));
                self.gap(gap);
            }
            Block::Heading { level, content } => {
                let (scale, bold, lh) = heading_style(*level);
                let top = match level {
                    1 => 1.0,
                    2 => 2.5,
                    _ => 1.5,
                };
                self.gap(BASE * top);
                let ctx = TextCtx {
                    size: BASE * scale,
                    line_height: lh,
                    weight: if bold { Weight::Bold } else { Weight::SemiBold },
                    ..TextCtx::body(palette)
                };
                self.inlines(content, &ctx);
                self.gap(BASE);
            }
            Block::Rule => {
                self.gap(BASE * 1.5);
                let y = self.place(1.0);
                let (l, r) = (self.left(), self.right());
                self.emit(Item::Rect {
                    x: l,
                    y,
                    w: r - l,
                    h: 1.0,
                    color: palette.rule,
                    radius: 0.0,
                });
                self.gap(BASE * 1.5);
            }
            Block::Code { language, text } => {
                self.code(language.as_deref(), text);
                self.gap(gap);
            }
            Block::Math(tex) => {
                self.display_math(tex);
                self.gap(gap);
            }
            Block::Quote(children) => {
                let x = self.left();
                self.push(
                    QUOTE_INDENT,
                    0.0,
                    Deco::Bar {
                        x,
                        w: 2.0,
                        color: palette.quote_border,
                    },
                );
                self.blocks(children);
                self.pop();
                self.gap(gap);
            }
            Block::Embed(children) => {
                let x = self.left();
                self.push(
                    QUOTE_INDENT,
                    0.0,
                    Deco::Bar {
                        x,
                        w: 2.0,
                        color: palette.accent,
                    },
                );
                self.blocks(children);
                self.pop();
                self.gap(gap);
            }
            Block::Callout {
                color,
                title,
                children,
            } => {
                let (x0, x1) = (self.left(), self.right());
                self.push(
                    24.0,
                    12.0,
                    Deco::Fill {
                        x0,
                        x1,
                        color: color.alpha(0.1),
                    },
                );
                self.pad(12.0);
                let ctx = TextCtx {
                    weight: Weight::SemiBold,
                    color: *color,
                    ..TextCtx::body(palette)
                };
                self.inlines(title, &ctx);
                if !children.is_empty() {
                    self.pad(8.0);
                    self.blocks(children);
                }
                self.pad(12.0);
                self.pop();
                self.gap(gap);
            }
            Block::List { items, tight } => {
                self.list(items, *tight);
                self.gap(gap);
            }
            Block::Table {
                alignments,
                header,
                rows,
            } => {
                self.table(alignments, header, rows);
                self.gap(gap);
            }
            Block::Image {
                data,
                width,
                height,
            } => {
                self.image(data, *width, *height);
                self.gap(gap);
            }
        }
    }

    fn list(&mut self, items: &[ListItem], tight: bool) {
        for item in items {
            self.push(LIST_INDENT, 0.0, Deco::None);
            self.marker = Some(item.marker);
            let done = matches!(item.marker, Marker::Task { done: true });
            for child in &item.children {
                if done {
                    // A done task's text is struck through and muted, as
                    // the reading view draws it.
                    self.block(&muted_done(child, self.ts.palette), tight);
                } else {
                    self.block(child, tight);
                }
            }
            if self.marker.is_some() {
                // An empty item still shows its marker.
                let top = self.place(BASE * LINE_HEIGHT);
                self.marker_at_top(top);
            }
            self.pop();
            if !tight {
                self.gap(BLOCK_GAP / 2.0);
            }
        }
    }

    fn code(&mut self, language: Option<&str>, text: &str) {
        let palette = self.ts.palette;
        let (x0, x1) = (self.left(), self.right());
        self.push(
            16.0,
            16.0,
            Deco::Fill {
                x0,
                x1,
                color: palette.code_background,
            },
        );
        self.pad(8.0);
        let body = text.strip_suffix('\n').unwrap_or(text);
        let runs = scoria_highlight::highlight(body, language);
        // Each byte's paint, the innermost run last.
        let mut paint = vec![palette.code[0]; body.len()];
        let mut weight = vec![(false, false); body.len()];
        for run in &runs {
            for ix in run.start..run.end.min(body.len()) {
                paint[ix] = palette.code_paint(run.style.paint);
                weight[ix] = (run.style.bold, run.style.italic);
            }
        }
        let ctx = TextCtx {
            size: BASE * CODE_SCALE,
            code: true,
            color: palette.code[0],
            ..TextCtx::body(palette)
        };
        let mut offset = 0;
        for line in body.split('\n') {
            let mut inlines = Vec::new();
            let mut start = 0;
            let bytes: Vec<(usize, char)> = line.char_indices().collect();
            for (i, &(at, _)) in bytes.iter().enumerate() {
                let next = bytes.get(i + 1).map_or(line.len(), |b| b.0);
                let here = (paint[offset + at], weight[offset + at]);
                let following =
                    (next < line.len()).then(|| (paint[offset + next], weight[offset + next]));
                if following != Some(here) {
                    inlines.push(Inline::styled(
                        &line[start..next],
                        SpanStyle {
                            color: Some(here.0),
                            bold: here.1 .0,
                            italic: here.1 .1,
                            ..SpanStyle::default()
                        },
                    ));
                    start = next;
                }
            }
            if inlines.is_empty() {
                // An empty line keeps its height.
                let top = self.place(ctx.size * ctx.line_height);
                self.marker_at_top(top);
            } else {
                let width = self.inner_width();
                let pieces = self.ts.pieces(&inlines, &ctx);
                for l in self.ts.break_lines(pieces, width, &ctx) {
                    self.line(l, BASE);
                }
            }
            offset += line.len() + 1;
        }
        self.pad(8.0);
        self.pop();
    }

    fn display_math(&mut self, tex: &str) {
        let palette = self.ts.palette;
        let size = BASE * MATH_SCALE;
        match scoria_math::typeset(tex, true) {
            Ok(formula) => {
                let h = formula.total_height() as f32 * size;
                let w = formula.width as f32 * size;
                // KaTeX's display block: a 1em margin above and below.
                self.gap(BASE);
                let top = self.place(h);
                let left = self.left() + ((self.inner_width() - w) / 2.0).max(0.0);
                let mut items = Vec::new();
                math_items(
                    &formula,
                    left,
                    top + formula.height as f32 * size,
                    size,
                    palette.text,
                    &mut items,
                );
                for item in items {
                    self.emit(item);
                }
                self.gap(BASE);
            }
            Err(_) => {
                let ctx = TextCtx {
                    color: palette.math_error,
                    ..TextCtx::body(palette)
                };
                self.inlines(&[Inline::plain(tex)], &ctx);
            }
        }
    }

    fn image(&mut self, data: &Arc<Vec<u8>>, width: Option<f32>, height: Option<f32>) {
        let Some((iw, ih)) = image_size(data) else {
            return;
        };
        let (iw, ih) = (iw as f32, ih as f32);
        let (mut w, mut h) = match (width, height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * ih / iw),
            (None, Some(h)) => (h * iw / ih, h),
            (None, None) => (iw, ih),
        };
        let max_w = self.inner_width();
        if w > max_w {
            h *= max_w / w;
            w = max_w;
        }
        if let Some(page) = self.height {
            if h > page {
                w *= page / h;
                h = page;
            }
        }
        let top = self.place(h);
        self.marker_at_top(top);
        let x = self.left();
        self.emit(Item::Image {
            x,
            y: top,
            w,
            h,
            data: data.clone(),
        });
    }

    fn table(&mut self, alignments: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        let palette = self.ts.palette;
        let cols = header
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        if cols == 0 {
            return;
        }
        const PAD_X: f32 = 8.0;
        const PAD_Y: f32 = 4.0;
        let body = TextCtx::body(palette);
        let head = TextCtx {
            weight: Weight::Bold,
            ..body
        };
        // Natural and minimum widths per column.
        let mut natural = vec![0.0f32; cols];
        let mut minimum = vec![0.0f32; cols];
        let all_rows =
            std::iter::once((header, &head)).chain(rows.iter().map(|r| (r.as_slice(), &body)));
        for (row, ctx) in all_rows.clone() {
            for (c, cell) in row.iter().enumerate() {
                let pieces = self.ts.pieces(cell, ctx);
                let total: f32 = pieces.iter().map(|p| p.width).sum();
                let widest = pieces
                    .iter()
                    .filter(|p| is_ink(p))
                    .map(|p| p.width)
                    .fold(0.0, f32::max);
                natural[c] = natural[c].max(total);
                minimum[c] = minimum[c].max(widest.min(120.0));
            }
        }
        let avail = self.inner_width() - cols as f32 * 2.0 * PAD_X;
        let sum_nat: f32 = natural.iter().sum();
        let sum_min: f32 = minimum.iter().sum();
        let widths: Vec<f32> = if sum_nat <= avail {
            natural.clone()
        } else if sum_min < avail {
            let spare = avail - sum_min;
            let stretch: f32 = natural.iter().zip(&minimum).map(|(n, m)| n - m).sum();
            natural
                .iter()
                .zip(&minimum)
                .map(|(n, m)| m + spare * (n - m) / stretch.max(1.0))
                .collect()
        } else {
            minimum.iter().map(|m| m * avail / sum_min).collect()
        };
        let left = self.left();
        let total_w: f32 = widths.iter().map(|w| w + 2.0 * PAD_X).sum();
        let mut laid = Vec::new();
        for (row, ctx) in all_rows {
            let mut cells = Vec::new();
            let mut row_h: f32 = 0.0;
            for (c, w) in widths.iter().enumerate() {
                let cell = row.get(c).map(Vec::as_slice).unwrap_or(&[]);
                let align = alignments.get(c).copied().unwrap_or(Align::Left);
                let cctx = TextCtx { align, ..*ctx };
                let pieces = self.ts.pieces(cell, &cctx);
                let lines = self.ts.break_lines(pieces, *w, &cctx);
                let h: f32 = lines.iter().map(Line::height).sum();
                row_h = row_h.max(h);
                cells.push(lines);
            }
            laid.push((cells, row_h + 2.0 * PAD_Y));
        }
        // The header row is not left alone at the foot of a page: it goes
        // over with the first body row when the two do not fit together.
        if let (Some(height), Some(first)) = (self.height, laid.get(1)) {
            let both = laid[0].1 + first.1;
            self.settle();
            if self.y + both > height && !self.at_top() && both <= height {
                self.new_page();
            }
        }
        for (r, (cells, row_h)) in laid.into_iter().enumerate() {
            let top = self.place(row_h);
            let border = palette.table_border;
            // The row's top rule, and the table's bottom one after the last.
            self.emit(Item::Rect {
                x: left,
                y: top,
                w: total_w,
                h: 1.0,
                color: border,
                radius: 0.0,
            });
            if r == rows.len() {
                self.emit(Item::Rect {
                    x: left,
                    y: top + row_h - 1.0,
                    w: total_w,
                    h: 1.0,
                    color: border,
                    radius: 0.0,
                });
            }
            let mut x = left;
            for (c, lines) in cells.into_iter().enumerate() {
                self.emit(Item::Rect {
                    x,
                    y: top,
                    w: 1.0,
                    h: row_h,
                    color: border,
                    radius: 0.0,
                });
                let mut y = top + PAD_Y;
                for line in lines {
                    let baseline = y + line.ascent;
                    for mut item in line.back {
                        item.translate(x + PAD_X, baseline);
                        self.emit_back(item);
                    }
                    for mut item in line.front {
                        item.translate(x + PAD_X, baseline);
                        self.emit(item);
                    }
                    y += line.ascent + line.descent;
                }
                x += widths[c] + 2.0 * PAD_X;
            }
            self.emit(Item::Rect {
                x,
                y: top,
                w: 1.0,
                h: row_h,
                color: border,
                radius: 0.0,
            });
        }
    }

    /// The inline title Obsidian prints above the note when asked to.
    pub fn title(&mut self, name: &str) {
        let palette = self.ts.palette;
        let (scale, _, lh) = heading_style(1);
        let ctx = TextCtx {
            size: BASE * scale,
            line_height: lh,
            weight: Weight::Bold,
            ..TextCtx::body(palette)
        };
        self.inlines(&[Inline::plain(name)], &ctx);
        self.gap(BASE * scale * 0.5);
    }
}

/// A done task's child: its text struck through in the muted colour.
fn muted_done(block: &Block, palette: &Palette) -> Block {
    match block {
        Block::Paragraph(inlines) => Block::Paragraph(
            inlines
                .iter()
                .map(|i| match i {
                    Inline::Text { text, style, link } => Inline::Text {
                        text: text.clone(),
                        style: SpanStyle {
                            strike: true,
                            color: Some(style.color.unwrap_or(palette.muted)),
                            ..*style
                        },
                        link: link.clone(),
                    },
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// An image's pixel size, read from its header.
pub fn image_size(data: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// Gives every math glyph its id in its KaTeX face: the layout leaves them
/// unresolved (`u16::MAX`) because it has the face and the character but
/// no font at hand at that point.
pub(crate) fn resolve_math(pages: &mut [Page], fonts: &Fonts) {
    for page in pages {
        for item in &mut page.items {
            if let Item::Text(run) = item {
                if let Face::Math(_) = run.face {
                    if run.glyphs.len() == 1 && run.glyphs[0].id == u16::MAX {
                        let ch = run.text.chars().next().unwrap_or(' ');
                        let (face, id) = fonts.resolve(run.face, ch);
                        run.face = face;
                        run.glyphs[0].id = id;
                        run.glyphs[0].advance = fonts.advance(face, id) * run.size;
                    }
                }
            }
        }
    }
}

/// Lays `blocks` out at `width`, over pages `height` tall (or one endless
/// page), with the note's name as a title first when given.
pub fn layout(
    blocks: &[Block],
    title: Option<&str>,
    width: f32,
    height: Option<f32>,
    fonts: &Fonts,
    palette: &Palette,
) -> (Vec<Page>, f32) {
    let mut flow = Flow::new(fonts, palette, width, height);
    if let Some(title) = title {
        flow.title(title);
    }
    flow.blocks(blocks);
    let (mut pages, h) = flow.finish();
    resolve_math(&mut pages, fonts);
    (pages, h)
}

/// One line of text in one face, unbroken: a canvas label, the product
/// name in the logo strip. Its pen starts at `(0, 0)` on the baseline; the
/// width comes back with it.
pub fn label_run(
    fonts: &Fonts,
    text: &str,
    face: Face,
    size: f32,
    color: Color,
) -> (Vec<GlyphRun>, f32) {
    let mut runs: Vec<GlyphRun> = Vec::new();
    let mut pen = 0.0;
    let mut prev: Option<(Face, u16)> = None;
    for ch in text.chars() {
        let (f, id) = fonts.resolve(face, ch);
        if let Some((pf, pid)) = prev {
            if pf == f {
                pen += fonts.kern(f, pid, id) * size;
            }
        }
        if !runs.last().is_some_and(|r| r.face == f) {
            runs.push(GlyphRun {
                face: f,
                size,
                x: pen,
                y: 0.0,
                color,
                glyphs: Vec::new(),
                text: String::new(),
            });
        }
        let run = runs.last_mut().expect("a run");
        let start = run.text.len();
        run.text.push(ch);
        let advance = fonts.advance(f, id) * size;
        run.glyphs.push(PlacedGlyph {
            id,
            x: pen - run.x,
            advance,
            range: start..run.text.len(),
        });
        pen += advance;
        prev = Some((f, id));
    }
    (runs, pen)
}
