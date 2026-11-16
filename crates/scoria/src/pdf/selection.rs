//! Text selection in a PDF: where a point falls among a page's text items,
//! what a selection covers, the text it copies and the link it names.
//!
//! What it does is observed (`tools/obsidian-probe/results/2026-11-15-pdf-selection.json`,
//! Obsidian 1.15.0, written up in
//! [[Specification#Observed PDF text selection, Obsidian 1.15.0]]): pdf.js's
//! text layer is one span per text item, so a drag selects characters of
//! items, a double click a word, a triple click one item (one line of a
//! paragraph, not the paragraph); `Ctrl+C` and "Copy" write the selection
//! with every line break as a space ("Line one Line two"); "Copy as quote"
//! writes `> ` before that, a blank line and the link; "Copy link to
//! selection" writes `[[file.pdf#page=N&selection=a,b,c,d|name, page N]]`
//! with `a,b,c,d` in the numbering of [`TextItem`]s and UTF-16 offsets; a
//! selection over two pages offers "Copy" alone. No pure logic here touches
//! gpui.

use scoria_pdf::{Selection, TextItem};
use std::cmp::Ordering;

/// A place in the text: before character `offset` of item `item` on page
/// `page`, all zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextPos {
    pub page: usize,
    pub item: usize,
    pub offset: usize,
}

impl PartialOrd for TextPos {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextPos {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.page, self.item, self.offset).cmp(&(other.page, other.item, other.offset))
    }
}

/// The character offset in `item` nearest `x`: before the first character
/// whose middle is right of it.
fn offset_at(item: &TextItem, x: f32) -> usize {
    item.edges
        .iter()
        .position(|(l, r)| (l + r) / 2.0 > x)
        .unwrap_or(item.edges.len())
}

/// Where the point `(x, y)` (points on the page) falls in `items`: in the
/// item whose box holds it, else the nearest item on its line, else the
/// nearest item at all. `None` when the page has no text.
pub fn hit(items: &[TextItem], x: f32, y: f32) -> Option<(usize, usize)> {
    let mut best: Option<(f32, usize)> = None;
    for (ix, item) in items.iter().enumerate() {
        if item.is_empty() {
            continue;
        }
        let [l, t, r, b] = item.bounds();
        let dx = if x < l {
            l - x
        } else if x > r {
            x - r
        } else {
            0.0
        };
        let dy = if y < t {
            t - y
        } else if y > b {
            y - b
        } else {
            0.0
        };
        // A point on an item's line is nearer than any point off it.
        let distance = if dy == 0.0 {
            dx
        } else {
            10_000.0 + dy * 100.0 + dx
        };
        if best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, ix));
        }
    }
    best.map(|(_, ix)| (ix, offset_at(&items[ix], x)))
}

/// The word around character `offset` of `item`, as a character range: a
/// run of letters and digits, a run of spaces, or one other character
/// (observed: a double click on `world` of `Hello world` selects `world`).
pub fn word_at(item: &TextItem, offset: usize) -> (usize, usize) {
    let chars: Vec<char> = item.text.chars().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let at = offset.min(chars.len() - 1);
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' || c == '\'' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    let k = class(chars[at]);
    if k == 2 {
        return (at, at + 1);
    }
    let mut from = at;
    while from > 0 && class(chars[from - 1]) == k {
        from -= 1;
    }
    let mut to = at + 1;
    while to < chars.len() && class(chars[to]) == k {
        to += 1;
    }
    (from, to)
}

/// The selected characters of one page: `(item, from, to)` for every item
/// the range `start..end` covers there, in order.
pub fn page_ranges(
    items: &[TextItem],
    page: usize,
    start: TextPos,
    end: TextPos,
) -> Vec<(usize, usize, usize)> {
    if page < start.page || page > end.page || items.is_empty() {
        return Vec::new();
    }
    let first = if page == start.page { start.item } else { 0 };
    let last = if page == end.page {
        end.item.min(items.len().saturating_sub(1))
    } else {
        items.len() - 1
    };
    let mut out = Vec::new();
    for ix in first..=last {
        let Some(item) = items.get(ix) else {
            break;
        };
        let from = if page == start.page && ix == start.item {
            start.offset.min(item.len())
        } else {
            0
        };
        let to = if page == end.page && ix == end.item {
            end.offset.min(item.len())
        } else {
            item.len()
        };
        out.push((ix, from, to.max(from)));
    }
    out
}

/// The selection's text as the text layer reads it: the items' characters,
/// a newline after an item that ends its line and between pages. `pages`
/// gives each page's items, or `None` when a page's text is not read yet.
pub fn selected_text(
    start: TextPos,
    end: TextPos,
    pages: &dyn Fn(usize) -> Option<std::sync::Arc<Vec<TextItem>>>,
) -> String {
    let mut out = String::new();
    for page in start.page..=end.page {
        if page > start.page {
            out.push('\n');
        }
        let Some(items) = pages(page) else {
            continue;
        };
        let ranges = page_ranges(&items, page, start, end);
        let count = ranges.len();
        for (n, (ix, from, to)) in ranges.into_iter().enumerate() {
            let item = &items[ix];
            out.push_str(&item.slice(from, to));
            if item.has_eol && n + 1 < count {
                out.push('\n');
            }
        }
    }
    out
}

/// What "Copy" and `Ctrl+C` write: every line break a space (observed:
/// `Line one Line two`, and the selection's `llo world\nKern ed\nWide gap`
/// copied as `llo world Kern ed Wide gap`).
pub fn copy_text(text: &str) -> String {
    text.replace('\n', " ")
}

/// What "Copy as quote" writes around the link: `> text`, a blank line,
/// the link.
pub fn quote(text: &str, link: &str) -> String {
    format!("> {}\n\n{link}", copy_text(text))
}

/// The `selection=` of a range on one page, offsets in UTF-16 units.
pub fn link_selection(items: &[TextItem], start: TextPos, end: TextPos) -> Option<Selection> {
    let a = items.get(start.item)?;
    let c = items.get(end.item)?;
    Some(Selection {
        start_item: start.item,
        start_offset: a.utf16_offset(start.offset),
        end_item: end.item,
        end_offset: c.utf16_offset(end.offset),
    })
}

/// A link's `selection=` back as positions on `page`, clamped to the
/// items; `None` when it names an item the page does not have (observed:
/// `selection=999,0,999,4` lands on the page and marks nothing).
pub fn resolve_selection(
    items: &[TextItem],
    page: usize,
    selection: Selection,
) -> Option<(TextPos, TextPos)> {
    let a = items.get(selection.start_item)?;
    let c = items.get(selection.end_item)?;
    let start = TextPos {
        page,
        item: selection.start_item,
        offset: a.char_index(selection.start_offset),
    };
    let end = TextPos {
        page,
        item: selection.end_item,
        offset: c.char_index(selection.end_offset),
    };
    (start <= end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn item(text: &str, x: f32, baseline: f32, eol: bool) -> TextItem {
        let edges = text
            .chars()
            .enumerate()
            .map(|(i, _)| (x + i as f32 * 6.0, x + (i + 1) as f32 * 6.0))
            .collect();
        TextItem {
            text: text.into(),
            has_eol: eol,
            edges,
            baseline,
            size: 12.0,
        }
    }

    /// The first five items of the probe's hand-written page.
    fn page() -> Vec<TextItem> {
        vec![
            item("Hello world", 72.0, 72.0, true),
            item("Kern ed", 72.0, 92.0, true),
            item("Wide", 72.0, 112.0, false),
            item(" ", 96.0, 112.0, false),
            item("gap", 123.0, 112.0, true),
        ]
    }

    fn pos(item: usize, offset: usize) -> TextPos {
        TextPos {
            page: 0,
            item,
            offset,
        }
    }

    #[test]
    fn the_observed_multi_item_selection_reads_and_copies_as_obsidian_did() {
        let items = Arc::new(page());
        let pages = |p: usize| (p == 0).then(|| items.clone());
        let text = selected_text(pos(0, 2), pos(4, 3), &pages);
        assert_eq!(text, "llo world\nKern ed\nWide gap");
        assert_eq!(copy_text(&text), "llo world Kern ed Wide gap");
        let sel = link_selection(&items, pos(0, 2), pos(4, 3)).unwrap();
        assert_eq!(sel.to_value(), "0,2,4,3");
        assert_eq!(
            quote(&text, "[[x]]"),
            "> llo world Kern ed Wide gap\n\n[[x]]"
        );
    }

    #[test]
    fn a_partial_selection_keeps_its_trailing_space() {
        let items = Arc::new(page());
        let pages = |_: usize| Some(items.clone());
        assert_eq!(selected_text(pos(1, 2), pos(1, 5), &pages), "rn ");
    }

    #[test]
    fn a_selection_over_two_pages_breaks_between_them() {
        let one = Arc::new(vec![item("Black text", 0.0, 10.0, false)]);
        let two = Arc::new(vec![item("Second page first line", 0.0, 10.0, true)]);
        let pages = |p: usize| Some(if p == 0 { one.clone() } else { two.clone() });
        let end = TextPos {
            page: 1,
            item: 0,
            offset: 22,
        };
        let text = selected_text(pos(0, 0), end, &pages);
        assert_eq!(copy_text(&text), "Black text Second page first line");
    }

    #[test]
    fn a_point_falls_in_its_item_or_the_nearest_on_its_line() {
        let items = page();
        // Inside "Hello world", over the third character's right half.
        assert_eq!(hit(&items, 72.0 + 2.0 * 6.0 + 4.0, 68.0), Some((0, 3)));
        // Right of "Hello world" on its line: its end.
        assert_eq!(hit(&items, 400.0, 68.0), Some((0, 11)));
        // Left of "Kern ed": its start.
        assert_eq!(hit(&items, 10.0, 88.0), Some((1, 0)));
        assert_eq!(hit(&[], 0.0, 0.0), None);
    }

    #[test]
    fn a_double_click_takes_the_word() {
        let it = item("Hello world", 0.0, 0.0, false);
        assert_eq!(word_at(&it, 8), (6, 11));
        assert_eq!(word_at(&it, 5), (5, 6));
        assert_eq!(word_at(&it, 0), (0, 5));
        let dot = item("one.", 0.0, 0.0, false);
        assert_eq!(word_at(&dot, 3), (3, 4));
    }

    #[test]
    fn a_link_selection_resolves_back_or_not_at_all() {
        let items = page();
        let sel = Selection::parse("0,2,4,3").unwrap();
        assert_eq!(
            resolve_selection(&items, 0, sel),
            Some((pos(0, 2), pos(4, 3)))
        );
        let bad = Selection::parse("999,0,999,4").unwrap();
        assert_eq!(resolve_selection(&items, 0, bad), None);
    }
}
