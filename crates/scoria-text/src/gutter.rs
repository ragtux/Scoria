//! What the editor draws beside the text, decided without a window: the
//! line-number gutter's width (`showLineNumber`) and the indentation
//! guides (`showIndentGuide`) — where each line's guides stand and which
//! of them the caret lights.
//!
//! Observed in Obsidian 1.15.0 (`tools/obsidian-probe/gutter-guides-rtl.mjs`,
//! results of 2026-11-15): the gutter is as wide as a run of nines with as
//! many digits as the note has lines, plus its padding, and never narrower
//! than its minimum; a guide stands in each indent unit of a line's leading
//! whitespace, a tab or `tabSize` columns of spaces; and the guides the
//! caret lights are the ones at its list item's own depth on the lines of
//! the item's children.

use crate::lists::parse_prefix;
use std::ops::Range;

/// How many digits the gutter is sized for: the digits of the line count,
/// which is the run of nines CodeMirror measures the column with (`9` for
/// one to nine lines, `99` from ten, `999` from a hundred).
pub fn number_digits(line_count: usize) -> usize {
    line_count.max(1).to_string().len()
}

/// The gutter's width for `digits` digits each `digit_width` wide, with its
/// padding (`pad`, both sides together) and its minimum.
pub fn gutter_width(digits: usize, digit_width: f32, pad: f32, min: f32) -> f32 {
    (digits as f32 * digit_width + pad).max(min)
}

/// The byte offsets in `line` where each of its leading indent units
/// starts: one per tab, one per `tab_size` columns of spaces. A run of
/// spaces short of a full unit stands no guide, and nothing after the
/// first non-blank byte counts.
pub fn indent_units(line: &str, tab_size: u8) -> Vec<usize> {
    let size = tab_size.max(1) as usize;
    let mut out = Vec::new();
    let mut col = 0;
    let mut unit_start = None;
    for (i, b) in line.bytes().enumerate() {
        match b {
            b'\t' => {
                // A tab finishes whatever unit it is in.
                out.push(unit_start.take().unwrap_or(i));
                col = (col / size + 1) * size;
            }
            b' ' => {
                let start = *unit_start.get_or_insert(i);
                col += 1;
                if col % size == 0 {
                    out.push(start);
                    unit_start = None;
                }
            }
            _ => break,
        }
    }
    out
}

/// The depth of `line` in indent units: how many guides it stands.
pub fn indent_depth(line: &str, tab_size: u8) -> usize {
    indent_units(line, tab_size).len()
}

/// The guides the caret lights: the item's own depth, on its children's
/// lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGuide {
    /// Which guide of each line: the item's depth, counted from zero.
    pub level: usize,
    /// The lines under the item, after it; never empty.
    pub lines: Range<usize>,
}

/// The guide the caret on `line` lights, if any: only a list item's line
/// does, and only when lines indented deeper follow it. `line_text` reads
/// a line of the note (`None` past the end). Blank lines inside the run
/// belong to it; blank lines after its last child do not.
///
/// Obsidian 1.15.0 lit nothing for a caret on a leaf item or on a
/// paragraph continuing an item, and for an item at depth `d` lit guide `d`
/// on every line of its subtree, its grandchildren's included.
pub fn active_guide<'a>(
    line_text: impl Fn(usize) -> Option<&'a str>,
    line: usize,
    tab_size: u8,
) -> Option<ActiveGuide> {
    let text = line_text(line)?;
    let prefix = parse_prefix(text);
    if prefix.marker.is_none() || !prefix.quotes.is_empty() {
        return None;
    }
    let level = indent_depth(text, tab_size);
    let mut last = None;
    let mut n = line + 1;
    while let Some(t) = line_text(n) {
        if t.trim().is_empty() {
            n += 1;
            continue;
        }
        if indent_depth(t, tab_size) <= level {
            break;
        }
        last = Some(n);
        n += 1;
    }
    last.map(|last| ActiveGuide {
        level,
        lines: line + 1..last + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gutter_is_sized_for_the_digits_of_the_line_count() {
        // The probe's spacer: 9 at one to nine lines, 99 at ten, 999 at a
        // hundred, 99999 at ten thousand.
        assert_eq!(number_digits(1), 1);
        assert_eq!(number_digits(9), 1);
        assert_eq!(number_digits(10), 2);
        assert_eq!(number_digits(99), 2);
        assert_eq!(number_digits(100), 3);
        assert_eq!(number_digits(10000), 5);
        assert_eq!(number_digits(0), 1);
    }

    #[test]
    fn the_gutter_width_has_a_floor() {
        // Obsidian's widths: 20 for one digit (the minimum), 21.7 for two,
        // 28.6 for three, 35.5 for four, 42.3 for five: 6.87 a digit plus
        // 8 of padding.
        let w = |d| gutter_width(d, 6.87, 8., 20.);
        assert_eq!(w(1), 20.);
        assert!((w(2) - 21.74).abs() < 0.1);
        assert!((w(3) - 28.61).abs() < 0.1);
        assert!((w(5) - 42.35).abs() < 0.1);
    }

    #[test]
    fn a_unit_is_a_tab_or_a_tab_size_of_spaces() {
        assert_eq!(indent_units("- top", 4), Vec::<usize>::new());
        assert_eq!(indent_units("\t- child", 4), vec![0]);
        assert_eq!(indent_units("\t\t\t- great", 4), vec![0, 1, 2]);
        assert_eq!(indent_units("    - space child", 4), vec![0]);
        assert_eq!(indent_units("        - space grandchild", 4), vec![0, 4]);
        // Short of a unit: nothing.
        assert_eq!(indent_units("  - two spaces", 4), Vec::<usize>::new());
        assert_eq!(indent_units("      six", 4), vec![0]);
        // A tab after two spaces finishes the unit they started.
        assert_eq!(indent_units("  \tx", 4), vec![0]);
        assert_eq!(indent_units("  - two spaces", 2), vec![0]);
        assert_eq!(indent_units("\t\t", 4), vec![0, 1]);
        assert_eq!(indent_units("x\t", 4), Vec::<usize>::new());
    }

    fn lines(src: &'static str) -> impl Fn(usize) -> Option<&'static str> {
        let v: Vec<&'static str> = src.split('\n').collect();
        move |i| v.get(i).copied()
    }

    #[test]
    fn the_caret_lights_its_items_depth_on_its_subtree() {
        let src = "- tab one\n\t- tab child\n\t\t- tab grandchild\n\t\t\t- tab great\n\t- tab child two\n- tab two\n";
        let at = |line| active_guide(lines(src), line, 4);
        // The probe's four carets: top lit guide 0 on lines 1-4, the child
        // guide 1 on 2-3, the grandchild guide 2 on 3, the leaf nothing.
        assert_eq!(
            at(0),
            Some(ActiveGuide {
                level: 0,
                lines: 1..5
            })
        );
        assert_eq!(
            at(1),
            Some(ActiveGuide {
                level: 1,
                lines: 2..4
            })
        );
        assert_eq!(
            at(2),
            Some(ActiveGuide {
                level: 2,
                lines: 3..4
            })
        );
        assert_eq!(at(3), None);
        assert_eq!(at(5), None);
    }

    #[test]
    fn a_continuation_lights_nothing_and_blank_lines_belong_inside() {
        let src = "- parent\n\tcontinuation\n\n- a\n\n\t- b\n\nafter\n";
        let at = |line| active_guide(lines(src), line, 4);
        assert_eq!(at(1), None);
        assert_eq!(
            at(0),
            Some(ActiveGuide {
                level: 0,
                lines: 1..2
            })
        );
        // The blank line between `- a` and its child is inside; the one
        // after the child is not.
        assert_eq!(
            at(3),
            Some(ActiveGuide {
                level: 0,
                lines: 4..6
            })
        );
    }

    #[test]
    fn ordered_and_task_items_light_too_but_quotes_do_not() {
        let src = "1. one\n\t1. child\n- [ ] task\n\t- [ ] sub\n> - quoted\n>\t- child\n";
        let at = |line| active_guide(lines(src), line, 4);
        assert_eq!(
            at(0),
            Some(ActiveGuide {
                level: 0,
                lines: 1..2
            })
        );
        assert_eq!(
            at(2),
            Some(ActiveGuide {
                level: 0,
                lines: 3..4
            })
        );
        assert_eq!(at(4), None);
    }
}
