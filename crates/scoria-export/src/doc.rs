//! What an export draws: a note's blocks as plain data, and the colours
//! they are painted in.
//!
//! The app builds these from the reading view's own model (which the one
//! parser, `scoria-markdown`, produced), so an export shows what the
//! reading view shows — embeds resolved, footnotes gathered, frontmatter
//! left out — without this crate reading Markdown itself.

use std::sync::Arc;

/// An sRGB colour with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(hex: u32) -> Color {
        Color {
            r: (hex >> 16) as u8,
            g: (hex >> 8) as u8,
            b: hex as u8,
            a: 255,
        }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }

    /// The same colour at `alpha` (0 to 1) of full opacity.
    pub fn alpha(self, alpha: f32) -> Color {
        Color {
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }

    /// From floats in 0 to 1.
    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Color {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color {
            r: c(r),
            g: c(g),
            b: c(b),
            a: c(a),
        }
    }
}

/// How a run of text is set.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpanStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    /// Inline code: the monospaced face over the code background.
    pub code: bool,
    /// A wash behind the text: a highlight.
    pub background: Option<Color>,
    /// The text colour; `None` is the palette's text colour.
    pub color: Option<Color>,
    /// Footnote numbers and `<sup>`: smaller and raised.
    pub superscript: bool,
}

/// A piece of inline content.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text {
        text: String,
        style: SpanStyle,
        /// Where the run links to: a URL, written as a link annotation.
        link: Option<String>,
    },
    /// TeX, typeset in text style.
    Math(String),
    /// A line break.
    Break,
}

impl Inline {
    pub fn plain(text: impl Into<String>) -> Inline {
        Inline::Text {
            text: text.into(),
            style: SpanStyle::default(),
            link: None,
        }
    }

    pub fn styled(text: impl Into<String>, style: SpanStyle) -> Inline {
        Inline::Text {
            text: text.into(),
            style,
            link: None,
        }
    }
}

/// A list item's marker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Marker {
    Bullet,
    Number(u64),
    Task { done: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub marker: Marker,
    pub children: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A block of the note.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    Code {
        language: Option<String>,
        text: String,
    },
    /// Display math.
    Math(String),
    Rule,
    Quote(Vec<Block>),
    Callout {
        color: Color,
        title: Vec<Inline>,
        children: Vec<Block>,
    },
    /// An embedded note's blocks.
    Embed(Vec<Block>),
    List {
        items: Vec<ListItem>,
        tight: bool,
    },
    Table {
        alignments: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// An image file's bytes (PNG, JPEG, GIF or WebP) and the size the
    /// note asks for, in CSS pixels.
    Image {
        data: Arc<Vec<u8>>,
        width: Option<f32>,
        height: Option<f32>,
    },
}

/// The colours an export paints with. The app fills it from its theme
/// registry; [`Palette::light`] is Obsidian's default light theme, which is
/// what Obsidian prints a note in.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub background: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub link: Color,
    pub accent: Color,
    pub tag: Color,
    pub code_background: Color,
    /// `--code-normal` .. `--code-important`, in
    /// [`scoria_highlight::Paint`]'s order.
    pub code: [Color; 11],
    pub highlight: Color,
    pub rule: Color,
    pub quote_border: Color,
    pub table_border: Color,
    pub math_error: Color,
}

impl Palette {
    /// Obsidian's light theme over white.
    pub fn light() -> Palette {
        let text = Color::rgb(0x222222);
        let muted = Color::rgb(0x5c5c5c);
        let faint = Color::rgb(0xababab);
        let accent = Color::rgb(0xbe4f2d);
        Palette {
            background: Color::rgb(0xffffff),
            text,
            muted,
            faint,
            link: accent,
            accent,
            tag: accent,
            code_background: Color::rgb(0xfafafa),
            code: [
                text,
                faint,
                Color::rgb(0xd53984),
                Color::rgb(0x08b94e),
                Color::rgb(0xe0ac00),
                Color::rgb(0x7852ee),
                Color::rgb(0xe93147),
                muted,
                Color::rgb(0x00bfbc),
                Color::rgb(0xe93147),
                Color::rgb(0xec7500),
            ],
            highlight: Color::rgba(255, 208, 0, 102),
            rule: Color::rgb(0xe0e0e0),
            quote_border: accent,
            table_border: Color::rgb(0xe0e0e0),
            math_error: Color::rgb(0xe93147),
        }
    }

    /// The colour a highlighted code run is painted.
    pub fn code_paint(&self, paint: scoria_highlight::Paint) -> Color {
        use scoria_highlight::Paint;
        self.code[match paint {
            Paint::Normal => 0,
            Paint::Comment => 1,
            Paint::Keyword => 2,
            Paint::String => 3,
            Paint::Function => 4,
            Paint::Value => 5,
            Paint::Operator => 6,
            Paint::Punctuation => 7,
            Paint::Property => 8,
            Paint::Tag => 9,
            Paint::Important => 10,
        }]
    }
}
