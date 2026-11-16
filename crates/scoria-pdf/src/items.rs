//! A page's text as positioned items, numbered the way a `#selection=` link
//! counts them.
//!
//! Obsidian's PDF view is pdf.js, and the `selection=a,b,c,d` of a link it
//! copies is "item `a`, character `b`" to "item `c`, character `d`" in
//! pdf.js's `getTextContent()` items, marked content left out
//! (`tools/obsidian-probe/results/2026-11-15-pdf-selection.json`, steps s01
//! and s02). So the items here are cut where pdf.js cuts them, as observed on
//! the probe's two files (`tests/fixtures/probe-items.json`):
//!
//! - glyphs of one font at one size on one line run together, one text-show
//!   operator or several (`(Hello) Tj ( world) Tj` is the one item
//!   `Hello world`);
//! - a gap of more than a tenth of the size inside such a run is a space in
//!   the item (`[(Kern)-120(ed)] TJ` is `Kern ed`), and so is a space glyph;
//! - a gap wider than the size cuts the item, and the gap becomes an item of
//!   its own, `" "` (`Wide`, `" "`, `gap`);
//! - a change of font or size cuts it too, and a space glyph left at the cut
//!   becomes its own `" "` item (`Plain`, `" "`, `bold`, `" "`, `again`);
//! - so does the start or end of marked content, and a space glyph with no
//!   item open is always an item of its own;
//! - a move to another line ends the open item with `hasEOL`; when the item
//!   before was already cut, an empty item with `hasEOL` stands for the line
//!   break instead.
//!
//! Positions are in points on the displayed page (rotation applied), from
//! its top-left corner, so they line up with [`crate::Document::render`].

use hayro::hayro_interpret::font::{Glyph, GlyphRun};
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::page::Page;
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image,
    ImageDrawProps, InterpreterCache, InterpreterSettings, SoftMask,
};
use hayro::kurbo::{BezPath, Point, Rect};

/// Above the baseline, as a share of the size: the top of an item's box.
const ASCENT: f32 = 0.8;
/// A gap wider than this share of the size, inside a run, is a space.
const SPACE_GAP: f32 = 0.1;
/// A gap wider than this share of the size cuts the item.
const CUT_GAP: f32 = 1.0;
/// A baseline that moves more than this share of the size is a new line.
const LINE_MOVE: f32 = 0.5;

/// One pdf.js text item: its string, whether a line break follows it, and
/// where each of its characters sits.
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    pub text: String,
    pub has_eol: bool,
    /// Each character's left and right edge along the line, in points from
    /// the page's left edge; one pair per `char` of `text`.
    pub edges: Vec<(f32, f32)>,
    /// The baseline, in points from the page's top edge.
    pub baseline: f32,
    /// The font size on the page, in points.
    pub size: f32,
}

impl TextItem {
    /// The item's box, `[left, top, right, bottom]` in points: its
    /// characters' extent, from [`ASCENT`] of the size above the baseline to
    /// the rest below it. An empty item has no width.
    pub fn bounds(&self) -> [f32; 4] {
        let top = self.baseline - self.size * ASCENT;
        let bottom = top + self.size;
        match (self.edges.first(), self.edges.last()) {
            (Some(first), Some(last)) => [first.0, top, last.1.max(first.0), bottom],
            _ => [0.0, top, 0.0, bottom],
        }
    }

    /// The extent of characters `from..to` along the line, or `None` for an
    /// empty range.
    pub fn span(&self, from: usize, to: usize) -> Option<(f32, f32)> {
        let to = to.min(self.edges.len());
        if from >= to {
            return None;
        }
        Some((
            self.edges[from].0,
            self.edges[to - 1].1.max(self.edges[from].0),
        ))
    }

    /// How many characters the item has.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Character `index` as a UTF-16 offset, which is what a link's
    /// `selection=` counts (a JavaScript string offset).
    pub fn utf16_offset(&self, index: usize) -> usize {
        self.text.chars().take(index).map(char::len_utf16).sum()
    }

    /// A UTF-16 offset as a character index, clamped to the item.
    pub fn char_index(&self, utf16: usize) -> usize {
        let mut units = 0;
        for (ix, c) in self.text.chars().enumerate() {
            if units >= utf16 {
                return ix;
            }
            units += c.len_utf16();
        }
        self.edges.len()
    }

    /// The text of characters `from..to`.
    pub fn slice(&self, from: usize, to: usize) -> String {
        self.text
            .chars()
            .skip(from)
            .take(to.saturating_sub(from))
            .collect()
    }
}

/// One glyph on its way into an item.
struct Placed {
    text: String,
    x: f32,
    right: f32,
    y: f32,
    size: f32,
    font: u128,
    space: bool,
}

/// The item being built and what it was built with.
struct Open {
    item: TextItem,
    font: u128,
    size: f32,
    right: f32,
}

#[derive(Default)]
struct Builder {
    items: Vec<TextItem>,
    open: Option<Open>,
    /// The last glyph's baseline, size and right edge, item or not.
    last: Option<(f32, f32, f32)>,
    /// A space glyph inside the open item, waiting to see whether the item
    /// goes on: its edges.
    pending: Option<(f32, f32)>,
}

impl Builder {
    fn whitespace(&mut self, left: f32, right: f32, baseline: f32, size: f32) {
        self.items.push(TextItem {
            text: " ".to_string(),
            has_eol: false,
            edges: vec![(left, right.max(left))],
            baseline,
            size,
        });
    }

    /// Ends the open item. A space glyph waiting at its end stays waiting:
    /// the next glyph on the line makes it an item of its own, a new line
    /// drops it.
    fn cut(&mut self, eol: bool) {
        if let Some(mut open) = self.open.take() {
            open.item.has_eol = eol;
            self.items.push(open.item);
        }
        if eol {
            self.pending = None;
        }
    }

    fn glyph(&mut self, g: Placed) {
        if let Some(open) = &self.open {
            if open.font != g.font || (open.size - g.size).abs() > open.size.max(1.0) * 0.01 {
                self.cut(false);
            }
        }
        let mut new_line = false;
        if let Some((y, size, _)) = self.last {
            if (g.y - y).abs() > LINE_MOVE * size.max(g.size) {
                new_line = true;
                self.pending = None;
                if self.open.is_some() {
                    self.cut(true);
                } else if self.items.last().is_some_and(|item| !item.has_eol) {
                    self.items.push(TextItem {
                        text: String::new(),
                        has_eol: true,
                        edges: Vec::new(),
                        baseline: g.y,
                        size: g.size,
                    });
                }
            }
        }
        let last_right = self.last.map(|(_, _, right)| right);
        self.last = Some((g.y, g.size, g.right));
        if !new_line {
            if let Some(open) = &self.open {
                let gap = g.x - open.right;
                if gap.abs() > CUT_GAP * g.size {
                    self.cut(false);
                } else if !g.space {
                    let space = self
                        .pending
                        .take()
                        .or((gap > SPACE_GAP * g.size).then_some((open.right, g.x)));
                    if let (Some((left, right)), Some(open)) = (space, self.open.as_mut()) {
                        open.item.text.push(' ');
                        open.item.edges.push((left, right.max(left)));
                    }
                }
            }
            // A gap (or a space glyph) between an item already cut and this
            // glyph is an item of its own.
            if self.open.is_none() && !g.space {
                if let Some(right) = last_right {
                    let gap = g.x - right;
                    let space = self
                        .pending
                        .take()
                        .or((gap > SPACE_GAP * g.size).then_some((right, g.x)));
                    if let Some((left, end)) = space {
                        self.whitespace(left, end, g.y, g.size);
                    }
                }
            }
        }
        if g.space {
            match &mut self.open {
                Some(open) => {
                    self.pending = Some((g.x, g.right));
                    open.right = g.right;
                }
                None => {
                    self.pending = None;
                    self.whitespace(g.x, g.right, g.y, g.size);
                }
            }
            return;
        }
        self.pending = None;
        let open = self.open.get_or_insert_with(|| Open {
            item: TextItem {
                text: String::new(),
                has_eol: false,
                edges: Vec::new(),
                baseline: g.y,
                size: g.size,
            },
            font: g.font,
            size: g.size,
            right: g.x,
        });
        // A glyph that maps to several characters (a ligature) shares its
        // advance among them.
        let n = g.text.chars().count().max(1) as f32;
        let step = (g.right - g.x) / n;
        for (ix, c) in g.text.chars().enumerate() {
            let left = g.x + step * ix as f32;
            open.item.text.push(c);
            open.item.edges.push((left, left + step));
        }
        open.right = g.right;
    }

    fn finish(mut self) -> Vec<TextItem> {
        self.cut(true);
        if let Some(last) = self.items.last_mut() {
            last.has_eol = false;
        }
        self.items
    }
}

#[derive(Default)]
struct ItemDevice {
    builder: Builder,
    /// The last run read: its length and first glyph's origin.
    last_run: Option<(usize, u64, u64)>,
}

impl<'a> Device<'a> for ItemDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn begin_marked_content(&mut self, _tag: &[u8], _mcid: Option<i32>) {
        self.builder.cut(false);
    }

    fn end_marked_content(&mut self) {
        self.builder.cut(false);
    }

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, mode: &DrawMode) {
        // A run filled and then stroked is handed over twice; read it once.
        let key = run.glyphs().first().map(|first| {
            let at = (props.transform * first.transform()) * Point::ZERO;
            (run.glyphs().len(), at.x.to_bits(), at.y.to_bits())
        });
        let again = matches!(mode, DrawMode::Stroke(_)) && key.is_some() && key == self.last_run;
        self.last_run = key;
        if again {
            return;
        }
        for glyph in run.glyphs() {
            let Some(unicode) = glyph.as_unicode() else {
                continue;
            };
            let text = match unicode {
                BfString::Char(c) => c.to_string(),
                BfString::String(s) => s,
            };
            if text.is_empty() {
                continue;
            }
            let to_page = props.transform * glyph.transform();
            let origin = to_page * Point::ZERO;
            let size = (to_page.determinant().abs().sqrt() * 1000.0).max(0.1) as f32;
            let (advance, font) = match &**glyph {
                Glyph::Outline(outline) => (
                    outline.advance_width().map(f64::from),
                    outline.font_cache_key(),
                ),
                Glyph::Type3(_) => (None, 3),
            };
            let right = match advance {
                Some(advance) => (to_page * Point::new(advance, 0.0)).x as f32,
                None => origin.x as f32 + size / 2.0,
            };
            self.builder.glyph(Placed {
                space: text.chars().all(char::is_whitespace),
                text,
                x: origin.x as f32,
                right,
                y: origin.y as f32,
                size,
                font,
            });
        }
    }
}

pub(crate) fn text_items(page: &Page<'_>, settings: &InterpreterSettings) -> Vec<TextItem> {
    let (width, height) = page.render_dimensions();
    let transform = page.initial_transform(true).to_kurbo();
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        transform,
        Rect::new(0.0, 0.0, width as f64, height as f64),
        &cache,
        page.xref(),
        settings.clone(),
    );
    let mut device = ItemDevice::default();
    interpret_page(page, &mut context, &mut device);
    device.builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(text: &str, x: f32, y: f32, size: f32, font: u128) -> Placed {
        let w = size * 0.5 * text.chars().count() as f32;
        Placed {
            text: text.to_string(),
            x,
            right: x + w,
            y,
            size,
            font,
            space: text.chars().all(char::is_whitespace),
        }
    }

    fn strs(items: &[TextItem]) -> Vec<(String, bool)> {
        items.iter().map(|i| (i.text.clone(), i.has_eol)).collect()
    }

    #[test]
    fn a_space_glyph_with_no_item_open_is_its_own_item() {
        let mut b = Builder::default();
        b.glyph(glyph(" ", 0.0, 10.0, 10.0, 1));
        b.glyph(glyph("a", 5.0, 10.0, 10.0, 1));
        assert_eq!(
            strs(&b.finish()),
            vec![(" ".into(), false), ("a".into(), false)]
        );
    }

    #[test]
    fn utf16_offsets_count_surrogate_pairs() {
        let item = TextItem {
            text: "a😀b".into(),
            has_eol: false,
            edges: vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
            baseline: 0.0,
            size: 1.0,
        };
        assert_eq!(item.utf16_offset(2), 3);
        assert_eq!(item.char_index(3), 2);
        assert_eq!(item.char_index(99), 3);
        assert_eq!(item.slice(1, 3), "😀b");
    }
}
