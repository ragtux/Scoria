//! Right-to-left text laid out by Scoria rather than by gpui.
//!
//! The pinned gpui shapes a right-to-left run correctly (cosmic-text runs
//! the bidi algorithm and positions the glyphs where they belong), but it
//! keeps the glyphs in byte order with x falling, and its wrapping and its
//! byte-to-x maps assume x rises with the byte: a Hebrew paragraph was
//! never wrapped (each glyph's measured width came out negative) and ran
//! off the left edge as one row, and the caret stood at the wrong end.
//! So a right-to-left line is laid out here: its rows are broken in
//! logical order (`scoria_text::bidi::break_rows`) from the advances of
//! the unwrapped layout, each row is shaped on its own (the shaper then
//! reorders within the row, as the bidi algorithm does per visual line),
//! the rows are right-aligned, and the caret is mapped with
//! `scoria_text::bidi::VisualRow`. The editor uses [`RtlRows`] for its
//! lines and the reading view [`RtlText`] for its paragraphs.

use gpui::{
    point, px, relative, App, Bounds, Element, ElementId, GlobalElementId, HighlightStyle,
    InspectorElementId, IntoElement, LayoutId, LineLayout, Pixels, Point, ShapedLine, SharedString,
    Style, TextAlign, TextRun, TextStyle, Window, WrappedLine,
};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

/// A right-to-left line's rows, right-aligned in `align`.
pub struct RtlRows {
    /// The width the rows are right-aligned in.
    pub align: Pixels,
    pub rows: Vec<RtlRow>,
}

pub struct RtlRow {
    /// The bytes of the line the row holds.
    pub range: Range<usize>,
    pub line: ShapedLine,
    pub map: scoria_text::bidi::VisualRow,
}

impl RtlRows {
    /// The left edge of row `r`, from the text's left edge.
    pub fn offset(&self, r: usize) -> Pixels {
        self.rows
            .get(r)
            .map_or(px(0.), |row| (self.align - row.line.width).max(px(0.)))
    }

    /// The row byte `d` is on: the last row starting at or before it, so
    /// a byte on a break belongs to the row it starts.
    pub fn row_of(&self, d: usize) -> usize {
        self.rows
            .iter()
            .rposition(|row| row.range.start <= d)
            .unwrap_or(0)
    }

    /// The row and x of the caret before byte `d`.
    pub fn caret(&self, d: usize) -> (usize, Pixels) {
        let r = self.row_of(d);
        let row = &self.rows[r];
        let x = row.map.caret_x(d.saturating_sub(row.range.start));
        (r, self.offset(r) + px(x))
    }
}

/// Lays out a right-to-left line from `line`, its unwrapped layout: the
/// glyphs, put left to right, give each byte's advance; the rows break in
/// logical order at `wrap_width`; and each row is shaped again on its own
/// with its share of the runs.
pub fn rtl_rows(
    window: &mut Window,
    text: &str,
    line: &WrappedLine,
    runs: &[TextRun],
    size: Pixels,
    wrap_width: Option<Pixels>,
) -> RtlRows {
    let layout = &line.unwrapped_layout;
    let mut sorted = glyphs(layout);
    sorted.sort_by(|a, b| a.1.total_cmp(&b.1));
    let advances: Vec<(usize, f32)> = sorted
        .iter()
        .enumerate()
        .map(|(i, &(byte, x))| {
            let next = sorted.get(i + 1).map_or(f32::from(layout.width), |g| g.1);
            (byte, (next - x).max(0.))
        })
        .collect();
    let align = wrap_width.unwrap_or(layout.width);
    let ranges = match wrap_width {
        Some(w) if layout.width > w => scoria_text::bidi::break_rows(text, &advances, f32::from(w)),
        _ => std::iter::once(0..text.len()).collect(),
    };
    let rows = ranges
        .into_iter()
        .map(|range| {
            let row_text = &text[range.clone()];
            let shaped = window.text_system().shape_line(
                SharedString::from(row_text.to_string()),
                size,
                &slice_runs(runs, range.clone()),
                None,
            );
            let map = scoria_text::bidi::VisualRow::new(
                row_text,
                &glyphs(&shaped),
                f32::from(shaped.width),
            );
            RtlRow {
                range,
                line: shaped,
                map,
            }
        })
        .collect();
    RtlRows { align, rows }
}

/// Where row `r` of a right-to-left line whose top-left is `origin` is
/// painted: right-aligned, `row_height` per row down.
pub fn rtl_row_origin(
    origin: Point<Pixels>,
    rtl: &RtlRows,
    r: usize,
    row_height: Pixels,
) -> Point<Pixels> {
    point(origin.x + rtl.offset(r), origin.y + row_height * r as f32)
}

/// A layout's glyphs as `(byte, x)`, in the order gpui keeps them (byte
/// order, for a right-to-left run as much as any: the x positions are the
/// visual ones).
fn glyphs(layout: &LineLayout) -> Vec<(usize, f32)> {
    layout
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter())
        .map(|g| (g.index, f32::from(g.position.x)))
        .collect()
}

/// The part of `runs` that covers the bytes `range`.
fn slice_runs(runs: &[TextRun], range: Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0;
    for run in runs {
        let (start, end) = (at, at + run.len);
        at = end;
        let (s, e) = (start.max(range.start), end.min(range.end));
        if s < e {
            let mut piece = run.clone();
            piece.len = e - s;
            out.push(piece);
        }
    }
    out
}

/// A right-to-left paragraph of the reading view: the text with its
/// styled ranges (and the ranges set in another family, inline code's),
/// laid out in rows of the width the block is given and right-aligned.
/// It takes no clicks: a link inside does not follow and the text cannot
/// be selected, which `InteractiveText` gives a left-to-right paragraph.
pub struct RtlText {
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    families: Vec<(Range<usize>, SharedString)>,
    rows: Rc<RefCell<Option<(Pixels, RtlRows)>>>,
    /// The text style in force where the element stands (a heading's
    /// size and weight), taken at layout: the measuring closure runs
    /// outside it.
    style: Option<TextStyle>,
}

impl RtlText {
    pub fn new(
        text: SharedString,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
        families: Vec<(Range<usize>, SharedString)>,
    ) -> RtlText {
        RtlText {
            text,
            highlights,
            families,
            rows: Rc::new(RefCell::new(None)),
            style: None,
        }
    }
}

/// The text runs of `text` in the window's text style with `highlights`
/// over it and `families` swapped in.
fn runs_for(
    base: &TextStyle,
    text: &str,
    highlights: &[(Range<usize>, HighlightStyle)],
    families: &[(Range<usize>, SharedString)],
) -> Vec<TextRun> {
    // Every boundary any range has, so each run is one style.
    let mut cuts: Vec<usize> = vec![0, text.len()];
    for (r, _) in highlights {
        cuts.extend([r.start, r.end]);
    }
    for (r, _) in families {
        cuts.extend([r.start, r.end]);
    }
    cuts.retain(|&c| c <= text.len() && text.is_char_boundary(c));
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .map(|w| {
            let (s, e) = (w[0], w[1]);
            let mut style = base.clone();
            for (r, h) in highlights {
                if r.start <= s && e <= r.end {
                    style = style.highlight(*h);
                }
            }
            for (r, family) in families {
                if r.start <= s && e <= r.end {
                    style.font_family = family.clone();
                }
            }
            style.to_run(e - s)
        })
        .collect()
}

fn layout_rows(
    window: &mut Window,
    style: &TextStyle,
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    families: &[(Range<usize>, SharedString)],
    width: Pixels,
) -> RtlRows {
    let runs = runs_for(style, text, highlights, families);
    let size = style.font_size.to_pixels(window.rem_size());
    let line = window
        .text_system()
        .shape_text(text.clone(), size, &runs, None, None)
        .ok()
        .and_then(|lines| lines.into_iter().next())
        .unwrap_or_default();
    rtl_rows(window, text, &line, &runs, size, Some(width))
}

impl IntoElement for RtlText {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for RtlText {
    type RequestLayoutState = ();
    type PrepaintState = Pixels;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let (text, highlights, families) = (
            self.text.clone(),
            self.highlights.clone(),
            self.families.clone(),
        );
        let slot = self.rows.clone();
        let line_height = window.line_height();
        let text_style = window.text_style();
        self.style = Some(text_style.clone());
        let id = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.unwrap_or(match available.width {
                gpui::AvailableSpace::Definite(w) => w,
                _ => px(f32::MAX / 4.),
            });
            let rows = layout_rows(window, &text_style, &text, &highlights, &families, width);
            let height = line_height * rows.rows.len().max(1) as f32;
            *slot.borrow_mut() = Some((width, rows));
            gpui::size(width, height)
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> Pixels {
        // Laid out again when the final width is not the one measured.
        let stale = self
            .rows
            .borrow()
            .as_ref()
            .is_none_or(|(w, _)| *w != bounds.size.width);
        if stale {
            let style = self.style.clone().unwrap_or_else(|| window.text_style());
            let rows = layout_rows(
                window,
                &style,
                &self.text,
                &self.highlights,
                &self.families,
                bounds.size.width,
            );
            *self.rows.borrow_mut() = Some((bounds.size.width, rows));
        }
        window.line_height()
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        line_height: &mut Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let rows = self.rows.borrow();
        let Some((_, rows)) = rows.as_ref() else {
            return;
        };
        for (r, row) in rows.rows.iter().enumerate() {
            let at = rtl_row_origin(bounds.origin, rows, r, *line_height);
            let _ = row
                .line
                .paint_background(at, *line_height, TextAlign::Left, None, window, cx);
            let _ = row
                .line
                .paint(at, *line_height, TextAlign::Left, None, window, cx);
        }
    }
}

/// Whether `text` runs right to left, by its first strong character.
pub fn is_rtl(text: &str) -> bool {
    scoria_text::bidi::strong_direction(text) == Some(scoria_text::bidi::Direction::Rtl)
}
