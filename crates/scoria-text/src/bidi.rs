//! Right-to-left text, decided without a window: which way a line runs,
//! where its rows break, and where a caret stands among glyphs laid out in
//! visual order.
//!
//! Observed in Obsidian 1.15.0 (`tools/obsidian-probe/gutter-guides-rtl.mjs`,
//! 2026-11-15): every editor line takes its direction from its first
//! strong character, Hebrew or Arabic making it right-to-left and Latin
//! left-to-right, whatever `rightToLeft` says; a line with no strong
//! character (a blank line, a fence, a lone number) runs the way the line
//! before it does. `rightToLeft` only decides the direction of such lines
//! at the top of the note, and which side the gutter is on.
//!
//! The text system shapes a right-to-left run and hands its glyphs back
//! in visual order, but measures and wraps a line as if glyph order were
//! byte order; the editor works around that with the functions here,
//! which take the glyphs as `(byte, x)` pairs.

use std::ops::Range;
use unicode_bidi::{bidi_class, BidiClass, BidiInfo};

/// Which way a line runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// The direction the first strong character of `text` gives it, or `None`
/// when it has none.
pub fn strong_direction(text: &str) -> Option<Direction> {
    text.chars().find_map(|c| match bidi_class(c) {
        BidiClass::L => Some(Direction::Ltr),
        BidiClass::R | BidiClass::AL => Some(Direction::Rtl),
        _ => None,
    })
}

/// The direction of line `line`: its own strong direction, else the
/// nearest line above with one, else `base` (`rightToLeft`). `line_text`
/// reads a line of the note. The walk up stops after `limit` lines, which
/// only a very long run of blank lines reaches.
pub fn line_direction<'a>(
    line_text: impl Fn(usize) -> &'a str,
    line: usize,
    base: Direction,
    limit: usize,
) -> Direction {
    let mut n = line;
    for _ in 0..=limit {
        if let Some(d) = strong_direction(line_text(n)) {
            return d;
        }
        if n == 0 {
            break;
        }
        n -= 1;
    }
    base
}

/// The glyphs of one laid-out row, left to right, each with the byte it
/// starts at, its left edge and its advance; and whether each runs right
/// to left.
#[derive(Debug, Clone, Default)]
pub struct VisualRow {
    glyphs: Vec<Glyph>,
    width: f32,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct Glyph {
    byte: usize,
    /// The first byte of the next cluster in the text: where the caret
    /// goes past this glyph.
    end: usize,
    x: f32,
    advance: f32,
    rtl: bool,
}

impl VisualRow {
    /// A row of `text` from the `(byte, x)` of its glyphs, in any order,
    /// and its width. The bidi levels are worked out for `text` itself,
    /// as the shaper did, so each glyph knows which way it runs.
    pub fn new(text: &str, glyphs: &[(usize, f32)], width: f32) -> VisualRow {
        let levels = if text.is_empty() {
            Vec::new()
        } else {
            BidiInfo::new(text, None).levels
        };
        // Left to right, whatever order they came in: the text system keeps
        // a right-to-left run's glyphs in byte order with falling x.
        let mut glyphs = glyphs.to_vec();
        glyphs.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut starts: Vec<usize> = glyphs.iter().map(|g| g.0).collect();
        starts.sort_unstable();
        starts.dedup();
        let end_of = |byte: usize| {
            let i = starts.partition_point(|&s| s <= byte);
            starts.get(i).copied().unwrap_or(text.len())
        };
        let out = glyphs
            .iter()
            .enumerate()
            .map(|(i, &(byte, x))| {
                let next = glyphs.get(i + 1).map_or(width, |g| g.1);
                Glyph {
                    byte,
                    end: end_of(byte),
                    x,
                    advance: (next - x).max(0.),
                    rtl: levels.get(byte).is_some_and(|l| l.is_rtl()),
                }
            })
            .collect();
        VisualRow {
            glyphs: out,
            width,
            len: text.len(),
        }
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    /// Where the caret before byte `index` stands: the leading edge of the
    /// glyph that starts there (its right edge when it runs right to
    /// left), or past the logically last glyph at the end of the row.
    pub fn caret_x(&self, index: usize) -> f32 {
        if self.glyphs.is_empty() {
            return 0.;
        }
        // The glyph whose cluster holds the byte.
        let holder = self
            .glyphs
            .iter()
            .filter(|g| g.byte <= index && index < g.end)
            .max_by_key(|g| g.byte);
        match holder {
            Some(g) if g.rtl => g.x + g.advance,
            Some(g) => g.x,
            None => {
                // Past the end: the trailing edge of the last glyph.
                let last = self.glyphs.iter().max_by_key(|g| g.byte).unwrap();
                if last.rtl {
                    last.x
                } else {
                    last.x + last.advance
                }
            }
        }
    }

    /// The byte of the glyph under `x`, or `None` off the row's ends.
    pub fn glyph_at(&self, x: f32) -> Option<usize> {
        self.glyphs
            .iter()
            .find(|g| x >= g.x && x < g.x + g.advance)
            .map(|g| g.byte)
    }

    /// The caret position nearest `x`: the nearer edge of the glyph under
    /// it, and past the row's ends the edge of the end glyph there.
    pub fn index_at(&self, x: f32) -> usize {
        let (Some(first), Some(last)) = (self.glyphs.first(), self.glyphs.last()) else {
            return 0;
        };
        let leading = |g: &Glyph| if g.rtl { g.end } else { g.byte };
        let trailing = |g: &Glyph| if g.rtl { g.byte } else { g.end };
        if x < first.x {
            return leading(first);
        }
        if x >= last.x + last.advance {
            return trailing(last);
        }
        let g = self
            .glyphs
            .iter()
            .find(|g| x < g.x + g.advance)
            .unwrap_or(last);
        // The left half of a glyph is its left edge's caret: its start on
        // a left-to-right glyph, its end on a right-to-left one.
        let left_half = x < g.x + g.advance / 2.;
        match (left_half, g.rtl) {
            (true, false) | (false, true) => g.byte,
            (true, true) | (false, false) => g.end,
        }
        .min(self.len)
    }
}

/// Breaks a line into rows no wider than `width`, in logical order: the
/// line's glyphs as `(byte, advance)` (any order), each row ending after a
/// space where one fits, or mid-word when a word is wider than the row.
/// Every row but the last keeps its trailing space, as gpui's wrapping
/// does.
pub fn break_rows(text: &str, glyphs: &[(usize, f32)], width: f32) -> Vec<Range<usize>> {
    let mut sorted: Vec<(usize, f32)> = glyphs.to_vec();
    sorted.sort_by_key(|g| g.0);
    let mut rows = Vec::new();
    let mut start = 0;
    let mut used = 0.;
    // The byte after the last space of the row so far, and the width up
    // to it.
    let mut candidate: Option<(usize, f32)> = None;
    for (i, &(byte, advance)) in sorted.iter().enumerate() {
        if byte < start {
            continue;
        }
        // A space hangs past the edge rather than starting a row.
        let space = text[byte..].starts_with(' ');
        if used + advance > width && byte > start && !space {
            let (cut, upto) = candidate.unwrap_or((byte, used));
            rows.push(start..cut);
            start = cut;
            used -= upto;
            candidate = None;
        }
        used += advance;
        let next = sorted.get(i + 1).map_or(text.len(), |g| g.0);
        if space && next > start {
            candidate = Some((next, used));
        }
    }
    rows.push(start..text.len());
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_runs_the_way_its_first_strong_character_does() {
        assert_eq!(strong_direction("English only line."), Some(Direction::Ltr));
        assert_eq!(strong_direction("שורה בעברית בלבד."), Some(Direction::Rtl));
        assert_eq!(strong_direction("Mixed שלום world"), Some(Direction::Ltr));
        assert_eq!(strong_direction("שלום English"), Some(Direction::Rtl));
        // Markers and digits are weak or neutral: the text decides.
        assert_eq!(strong_direction("- פריט אחד"), Some(Direction::Rtl));
        assert_eq!(strong_direction("1. ראשון"), Some(Direction::Rtl));
        assert_eq!(strong_direction("# כותרת"), Some(Direction::Rtl));
        assert_eq!(strong_direction("مرحبا"), Some(Direction::Rtl));
        assert_eq!(strong_direction("```"), None);
        assert_eq!(strong_direction(""), None);
    }

    #[test]
    fn a_line_without_one_runs_the_way_the_line_above_does() {
        // The probe's note: the blank line after the Hebrew heading was
        // right-to-left and the one after the English line left-to-right,
        // with `rightToLeft` off and on alike.
        let lines = ["# כותרת ראשית", "", "English only line.", "", "```"];
        let at = |base, line| line_direction(|i| lines[i], line, base, 256);
        assert_eq!(at(Direction::Ltr, 1), Direction::Rtl);
        assert_eq!(at(Direction::Rtl, 3), Direction::Ltr);
        assert_eq!(at(Direction::Rtl, 4), Direction::Ltr);
        let blank = ["", "1."];
        assert_eq!(
            line_direction(|i| blank[i], 1, Direction::Rtl, 256),
            Direction::Rtl
        );
        assert_eq!(
            line_direction(|i| blank[i], 1, Direction::Ltr, 256),
            Direction::Ltr
        );
    }

    /// A right-to-left word laid out the way the shaper lays it out: the
    /// last letter leftmost. Each letter of Hebrew is two bytes here.
    fn hebrew_row() -> (String, VisualRow) {
        let text = "אבג".to_string();
        // ג at 0, ב at 10, א at 20; width 30. The text system hands them
        // over in byte order.
        let row = VisualRow::new(&text, &[(0, 20.), (2, 10.), (4, 0.)], 30.);
        (text, row)
    }

    #[test]
    fn the_caret_in_a_right_to_left_word_starts_at_its_right_edge() {
        let (_, row) = hebrew_row();
        assert_eq!(row.caret_x(0), 30.);
        assert_eq!(row.caret_x(2), 20.);
        assert_eq!(row.caret_x(4), 10.);
        // The end of the line is its left edge.
        assert_eq!(row.caret_x(6), 0.);
    }

    #[test]
    fn a_click_in_a_right_to_left_word_finds_the_nearer_edge() {
        let (_, row) = hebrew_row();
        // The right half of א (x 25..30) is before it; the left half after.
        assert_eq!(row.index_at(28.), 0);
        assert_eq!(row.index_at(22.), 2);
        assert_eq!(row.index_at(1.), 6);
        // Off the ends: the right is the start, the left the end.
        assert_eq!(row.index_at(40.), 0);
        assert_eq!(row.index_at(-5.), 6);
        assert_eq!(row.glyph_at(15.), Some(2));
        assert_eq!(row.glyph_at(31.), None);
    }

    #[test]
    fn a_left_to_right_row_maps_as_before() {
        let row = VisualRow::new("abc", &[(0, 0.), (1, 8.), (2, 16.)], 24.);
        assert_eq!(row.caret_x(0), 0.);
        assert_eq!(row.caret_x(2), 16.);
        assert_eq!(row.caret_x(3), 24.);
        assert_eq!(row.index_at(5.), 1);
        assert_eq!(row.index_at(3.), 0);
        assert_eq!(row.index_at(100.), 3);
    }

    #[test]
    fn rows_break_after_spaces_in_logical_order() {
        // "אב גד הו": three two-letter words, letters 10 wide, spaces 5.
        let text = "אב גד הו";
        let glyphs: Vec<(usize, f32)> = text
            .char_indices()
            .map(|(i, c)| (i, if c == ' ' { 5. } else { 10. }))
            .collect();
        // Room for one word and a space, then the next: a row each.
        let rows = break_rows(text, &glyphs, 26.);
        let words: Vec<&str> = rows.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(words, vec!["אב ", "גד ", "הו"]);
        // Room for two words.
        let rows = break_rows(text, &glyphs, 46.);
        let words: Vec<&str> = rows.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(words, vec!["אב גד ", "הו"]);
        // Everything fits.
        assert_eq!(break_rows(text, &glyphs, 500.), vec![0..text.len()]);
    }

    #[test]
    fn a_word_wider_than_the_row_breaks_inside() {
        let text = "אבגדה";
        let glyphs: Vec<(usize, f32)> = text.char_indices().map(|(i, _)| (i, 10.)).collect();
        let rows = break_rows(text, &glyphs, 25.);
        let parts: Vec<&str> = rows.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(parts, vec!["אב", "גד", "ה"]);
    }
}
