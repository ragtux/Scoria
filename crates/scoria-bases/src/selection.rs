//! The table's cell selection: a cursor and the corner a range runs to, what
//! a copy of the range puts on the clipboard, and what a paste or a Delete
//! over it writes into each row's note.
//!
//! Observed on Obsidian 1.15.1
//! (`tools/obsidian-probe/results/2026-11-16-bases-cells-search-menu.json`,
//! prose in [[Specification#Observed Bases cell selection, Obsidian 1.15.1]]).
//! The table keeps a cursor (`activeCell`) and, once Shift has moved or
//! clicked, a rectangle (`selection`: `top`, `bottom`, `start`, `end`, all
//! inclusive). Shift+arrows move the cursor and leave the far corner where it
//! was; Shift+click leaves the cursor and moves the far corner to the clicked
//! cell; a plain arrow or click drops the range, Escape drops it and keeps
//! the cursor, Ctrl+A takes every cell.
//!
//! A copy writes the range as tab-separated text with no header and no
//! trailing newline, quoting a cell CSV-style when it holds a tab, a line
//! break or a double quote. A paste of text that came from outside writes
//! every cell as a **string**, whatever the property's type (`abc` into a
//! number, `true` as the string `"true"`); a paste of the table's own copy
//! writes the values it copied with their types. A single value fills the
//! whole range; anything larger lands at the range's top-left corner and the
//! range becomes what it covered.
//!
//! **Where Scoria departs on purpose.** Obsidian 1.15.1 writes a multi-column
//! paste wrongly: every pasted cell of a row takes the clipboard row's value
//! at the index of the target's *first column* (`a\tb` at column 1 writes
//! `b` twice, `p\tq\tr\ts` at column 2 writes `r` four times), a range wider
//! than one column grows by the clipboard's width less one, and a block
//! reaching past the last column writes nothing. That is a bug, not a format,
//! and following it would scramble notes; each cell here takes its own value.
//!
//! Nothing here knows about gpui: the view keeps a [`Selection`] in row
//! ordinals and column indexes, and asks this module for the range, the
//! clipboard text and the writes.

use crate::value::Value;
use scoria_formats::properties::Value as Prop;

/// A cell: a row ordinal (the rows only, group headings skipped) and a
/// column index.
pub type CellAt = (usize, usize);

/// The rectangle a selection covers, inclusive at both ends, in Obsidian's
/// own field names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRange {
    pub top: usize,
    pub bottom: usize,
    pub start: usize,
    pub end: usize,
}

impl CellRange {
    pub fn of(a: CellAt, b: CellAt) -> CellRange {
        CellRange {
            top: a.0.min(b.0),
            bottom: a.0.max(b.0),
            start: a.1.min(b.1),
            end: a.1.max(b.1),
        }
    }

    pub fn contains(&self, (row, col): CellAt) -> bool {
        (self.top..=self.bottom).contains(&row) && (self.start..=self.end).contains(&col)
    }

    pub fn rows(&self) -> usize {
        self.bottom - self.top + 1
    }

    pub fn columns(&self) -> usize {
        self.end - self.start + 1
    }

    /// Every cell, row by row.
    pub fn cells(&self) -> impl Iterator<Item = CellAt> + '_ {
        (self.top..=self.bottom).flat_map(move |r| (self.start..=self.end).map(move |c| (r, c)))
    }
}

/// The cursor and, while a range is up, its far corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Obsidian's `activeCell`: where the ring is drawn and what Enter,
    /// typing and the plain arrows act on.
    pub cursor: CellAt,
    /// The range's other corner, `None` with no range up.
    pub corner: Option<CellAt>,
    /// Ctrl+A's range, which leaves the cursor where it was (observed: the
    /// cursor stayed on `(0, 1)` under a range of the whole table), so no
    /// corner can stand for it.
    pub all: Option<CellRange>,
}

/// The rows a range may span: a grouped table confines a selection to the
/// group its cursor is in, and an ungrouped one is one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub first_row: usize,
    pub last_row: usize,
    pub columns: usize,
}

impl Selection {
    pub fn at(cursor: CellAt) -> Selection {
        Selection {
            cursor,
            corner: None,
            all: None,
        }
    }

    /// The rectangle, a single cell with no range up.
    pub fn range(&self) -> CellRange {
        self.all
            .unwrap_or_else(|| CellRange::of(self.cursor, self.corner.unwrap_or(self.cursor)))
    }

    /// Whether a range is up: a Shift key, a Shift+click, Ctrl+A or a paste
    /// opened one (Obsidian keeps a one-cell `selection` after a paste over
    /// the cursor, and it draws as a range).
    pub fn has_range(&self) -> bool {
        self.corner.is_some() || self.all.is_some()
    }

    /// The corner a range keeps while the cursor moves: the one set, or for
    /// Ctrl+A's range the corner across from the cursor.
    fn fixed_corner(&self) -> CellAt {
        if let Some(r) = self.all {
            let row = if self.cursor.0 - r.top <= r.bottom - self.cursor.0 {
                r.bottom
            } else {
                r.top
            };
            let col = if self.cursor.1 - r.start <= r.end - self.cursor.1 {
                r.end
            } else {
                r.start
            };
            return (row, col);
        }
        self.corner.unwrap_or(self.cursor)
    }

    /// Shift+arrow: the cursor moves by one row or column inside `bounds`,
    /// and the far corner stays where the range started.
    pub fn extend(&mut self, d_row: i32, d_col: i32, bounds: Bounds) {
        if bounds.columns == 0 {
            return;
        }
        let corner = self.fixed_corner();
        let row = (self.cursor.0 as i64 + d_row as i64)
            .clamp(bounds.first_row as i64, bounds.last_row as i64) as usize;
        let col =
            (self.cursor.1 as i64 + d_col as i64).clamp(0, bounds.columns as i64 - 1) as usize;
        self.cursor = (row, col);
        self.corner = Some(corner);
        self.all = None;
    }

    /// Shift+click: the cursor stays and the far corner goes to the click,
    /// clamped into the cursor's group.
    pub fn extend_to(&mut self, (row, col): CellAt, bounds: Bounds) {
        let row = row.clamp(bounds.first_row, bounds.last_row);
        let col = col.min(bounds.columns.saturating_sub(1));
        self.corner = Some((row, col));
        self.all = None;
    }

    /// Ctrl+A: every cell of the group, the cursor left where it is.
    pub fn select_all(&mut self, bounds: Bounds) {
        if bounds.columns == 0 {
            return;
        }
        self.corner = None;
        self.all = Some(CellRange {
            top: bounds.first_row,
            bottom: bounds.last_row,
            start: 0,
            end: bounds.columns - 1,
        });
    }

    /// What a paste leaves selected: the block it covered, as a range the
    /// cursor stays at the corner of.
    pub fn cover(&mut self, range: CellRange) {
        self.all = Some(range);
        self.corner = None;
    }

    /// Escape, a plain arrow, a plain click: the range goes.
    pub fn collapse(&mut self) {
        self.corner = None;
        self.all = None;
    }
}

// ---------------------------------------------------------------------------
// Copy
// ---------------------------------------------------------------------------

/// A cell's text on the clipboard. Observed: `null` and an empty cell are
/// empty, a list joins with `, ` (`red, green`), a checkbox is `true` or
/// `false`, a number as JavaScript prints it, a date `YYYY-MM-DD`, the name
/// column the file's name.
pub fn copy_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// One field: CSV-style double quotes, every quote doubled, when it holds a
/// tab, a line break or a double quote; bare otherwise. Observed:
/// `"l1\nl2"` and `"say ""hi"""`, `"with\ttab"`; a comma alone stays bare
/// (`red, green`).
fn field(text: &str) -> String {
    if text.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// The range as the clipboard's text: tabs between cells, `\n` between rows,
/// no header and no trailing newline (observed:
/// `todo\t1.5\ndoing\t2\ndone\t30`).
pub fn encode_tsv(grid: &[Vec<String>]) -> String {
    grid.iter()
        .map(|row| row.iter().map(|c| field(c)).collect::<Vec<_>>().join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the table's own copy keeps of a cell besides its text: the note
/// property's value with its type, which a paste of that copy writes back
/// as it was (a list as a list, `1.5` as a number). `None` is a cell that
/// is not a note property — the name, another `file.*`, a formula — which
/// Obsidian copies as `{"type":"file"}` and which a paste then skips.
pub type CopiedCell = Option<Prop>;

/// A cell's value as its copy keeps it.
pub fn copied_value(value: &Value) -> Prop {
    match value {
        Value::Null => Prop::Empty,
        Value::Bool(b) => Prop::Checkbox(*b),
        Value::Number(n) => Prop::Number(*n),
        Value::Str(s) => Prop::Text(s.clone()),
        Value::List(items) => Prop::List(items.iter().map(copy_text).collect()),
        Value::Tags(tags) => Prop::List(tags.clone()),
        other => Prop::Text(copy_text(other)),
    }
}

// ---------------------------------------------------------------------------
// Paste
// ---------------------------------------------------------------------------

/// Text from outside, as cells. Over a single cell, text with no tab is
/// one value, line breaks and all (observed: three lines at the cursor wrote
/// one `|-` block, and so did a Markdown table); otherwise the text splits
/// at `\n` into rows and at tabs into cells. Obsidian splits naively — a
/// quoted field is not unquoted (`"x\ty"` is the two cells `"x` and `y"`) and
/// a trailing newline is an empty last row — and so does this; a `\r`
/// before a split `\n` is dropped, which no run observed.
pub fn parse_text(text: &str, single_target: bool) -> Vec<Vec<String>> {
    if single_target && !text.contains('\t') {
        return vec![vec![text.to_string()]];
    }
    text.split('\n')
        .map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            line.split('\t').map(str::to_string).collect()
        })
        .collect()
}

/// Text from outside as what a paste writes: every cell a string.
/// Observed: `abc` and `12,5` into a number column, `true` into a checkbox
/// column as the string `"true"`, `May 6, 2026` into a date column, `a, b`
/// into a list column as the one string.
pub fn text_cells(grid: Vec<Vec<String>>) -> Vec<Vec<CopiedCell>> {
    grid.into_iter()
        .map(|row| row.into_iter().map(|s| Some(Prop::Text(s))).collect())
        .collect()
}

/// One write a paste or a Delete makes.
#[derive(Debug, Clone, PartialEq)]
pub struct CellWrite {
    pub row: usize,
    pub column: usize,
    pub value: Prop,
}

/// The writes a paste of `cells` over `range` makes, and the range it
/// leaves selected.
///
/// A single value fills every cell of the range (observed: `Q` over three
/// rows wrote three, `F` over a two-by-two block wrote four) and the range
/// stays. Anything larger lands at the range's top-left corner, one cell per
/// cell, with no tiling (observed: two lines over four rows wrote two), and
/// the range becomes the block it covered, cut at the table's last row and
/// column, which is where Scoria's reading of a block wider than the target
/// starts (see the module's note on Obsidian's column bug). Columns
/// `editable` refuses — the name, `file.*`, formulas — take nothing
/// (observed: `Renamed` over the name cell renamed nothing), nor does a
/// copied cell that was not a note property.
pub fn paste_plan(
    range: CellRange,
    cells: &[Vec<CopiedCell>],
    bounds: Bounds,
    editable: &dyn Fn(usize) -> bool,
) -> (Vec<CellWrite>, CellRange) {
    let mut writes = Vec::new();
    let single = cells.len() == 1 && cells[0].len() == 1;
    if single {
        if let Some(value) = &cells[0][0] {
            for (row, column) in range.cells() {
                if editable(column) {
                    writes.push(CellWrite {
                        row,
                        column,
                        value: value.clone(),
                    });
                }
            }
        }
        return (writes, range);
    }
    let height = cells.len();
    let width = cells.iter().map(Vec::len).max().unwrap_or(0);
    if height == 0 || width == 0 || bounds.columns == 0 {
        return (writes, range);
    }
    for (i, line) in cells.iter().enumerate() {
        let row = range.top + i;
        if row > bounds.last_row {
            break;
        }
        for (j, cell) in line.iter().enumerate() {
            let column = range.start + j;
            if column >= bounds.columns {
                break;
            }
            if let Some(value) = cell {
                if editable(column) {
                    writes.push(CellWrite {
                        row,
                        column,
                        value: value.clone(),
                    });
                }
            }
        }
    }
    let covered = CellRange {
        top: range.top,
        bottom: (range.top + height - 1).min(bounds.last_row),
        start: range.start,
        end: (range.start + width - 1).min(bounds.columns - 1),
    };
    (writes, covered)
}

/// Delete or Backspace: every editable cell of the range emptied to
/// `key:` with no value, the key added to a note that lacks it (observed:
/// the whole table deleted wrote `due:` and `items:` into a note that had
/// neither, and a frontmatter block of six empty keys into one with none).
/// The range stays up.
pub fn delete_plan(range: CellRange, editable: &dyn Fn(usize) -> bool) -> Vec<CellWrite> {
    range
        .cells()
        .filter(|(_, column)| editable(*column))
        .map(|(row, column)| CellWrite {
            row,
            column,
            value: Prop::Empty,
        })
        .collect()
}

/// The writes grouped by row, in row order and each row's columns in column
/// order: one note write per row, as the note is written once with every key
/// the range named.
pub fn by_row(writes: &[CellWrite]) -> Vec<(usize, Vec<(usize, Prop)>)> {
    let mut out: Vec<(usize, Vec<(usize, Prop)>)> = Vec::new();
    let mut sorted = writes.to_vec();
    sorted.sort_by_key(|w| (w.row, w.column));
    for w in sorted {
        match out.last_mut() {
            Some((row, cols)) if *row == w.row => cols.push((w.column, w.value)),
            _ => out.push((w.row, vec![(w.column, w.value)])),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Bounds = Bounds {
        first_row: 0,
        last_row: 4,
        columns: 7,
    };
    fn props(_: usize) -> bool {
        true
    }
    fn text(s: &str) -> Prop {
        Prop::Text(s.to_string())
    }

    #[test]
    fn shift_arrows_move_the_cursor_and_keep_the_corner() {
        // s01: from (0,1), Shift+Down twice and Shift+Right: rows 0..2,
        // columns 1..2, the cursor at (2,2).
        let mut sel = Selection::at((0, 1));
        sel.extend(1, 0, ALL);
        sel.extend(1, 0, ALL);
        sel.extend(0, 1, ALL);
        assert_eq!(sel.cursor, (2, 2));
        assert_eq!(
            sel.range(),
            CellRange {
                top: 0,
                bottom: 2,
                start: 1,
                end: 2
            }
        );
        // Shift+Up shrinks from the cursor's side.
        sel.extend(-1, 0, ALL);
        assert_eq!(sel.range().bottom, 1);
        // Two Shift+Lefts cross the corner: columns 0..1.
        sel.extend(0, -1, ALL);
        sel.extend(0, -1, ALL);
        assert_eq!(sel.cursor, (1, 0));
        assert_eq!((sel.range().start, sel.range().end), (0, 1));
        sel.collapse();
        assert!(!sel.has_range());
        assert_eq!(sel.range(), CellRange::of((1, 0), (1, 0)));
    }

    #[test]
    fn shift_click_keeps_the_cursor_and_moves_the_corner() {
        // s02: the cursor on (0,1), Shift+click on (2,3): rows 0..2,
        // columns 1..3, the cursor still on (0,1).
        let mut sel = Selection::at((0, 1));
        sel.extend_to((2, 3), ALL);
        assert_eq!(sel.cursor, (0, 1));
        assert_eq!(
            sel.range(),
            CellRange {
                top: 0,
                bottom: 2,
                start: 1,
                end: 3
            }
        );
        // Then onto the name column of row 3: columns 0..1.
        sel.extend_to((3, 0), ALL);
        assert_eq!(
            sel.range(),
            CellRange {
                top: 0,
                bottom: 3,
                start: 0,
                end: 1
            }
        );
    }

    #[test]
    fn a_range_stays_inside_its_group_and_the_table() {
        let group = Bounds {
            first_row: 2,
            last_row: 3,
            columns: 3,
        };
        let mut sel = Selection::at((2, 1));
        for _ in 0..5 {
            sel.extend(1, 1, group);
        }
        assert_eq!(sel.cursor, (3, 2));
        sel.extend_to((9, 9), group);
        assert_eq!(sel.corner, Some((3, 2)));
        sel.extend_to((0, 0), group);
        assert_eq!(sel.corner, Some((2, 0)));
    }

    #[test]
    fn select_all_is_the_whole_group_and_keeps_the_cursor() {
        // s01: Ctrl+A on (0,1) selected rows 0..4, columns 0..6, and the
        // cursor stayed.
        let mut sel = Selection::at((0, 1));
        sel.select_all(ALL);
        assert_eq!(sel.cursor, (0, 1));
        assert_eq!(
            sel.range(),
            CellRange {
                top: 0,
                bottom: 4,
                start: 0,
                end: 6
            }
        );
        // A Shift+arrow after it keeps the corner across from the cursor.
        sel.extend(1, 0, ALL);
        assert_eq!(sel.range(), CellRange::of((1, 1), (4, 6)));
        sel.collapse();
        assert!(!sel.has_range());
    }

    #[test]
    fn the_copy_is_tab_separated_with_no_header_and_no_trailing_newline() {
        let grid = vec![
            vec!["todo".to_string(), "1.5".to_string()],
            vec!["doing".to_string(), "2".to_string()],
            vec!["done".to_string(), "30".to_string()],
        ];
        assert_eq!(encode_tsv(&grid), "todo\t1.5\ndoing\t2\ndone\t30");
    }

    #[test]
    fn a_field_with_a_tab_a_break_or_a_quote_is_quoted_and_a_comma_is_not() {
        // s03, byte for byte: `"l1\nl2"\t1.5\t\t\t\t"say ""hi"""`.
        let grid = vec![vec![
            "l1\nl2".to_string(),
            "1.5".to_string(),
            String::new(),
            String::new(),
            String::new(),
            "say \"hi\"".to_string(),
        ]];
        assert_eq!(
            encode_tsv(&grid),
            "\"l1\nl2\"\t1.5\t\t\t\t\"say \"\"hi\"\"\""
        );
        assert_eq!(field("with\ttab"), "\"with\ttab\"");
        assert_eq!(field("red, green"), "red, green");
    }

    #[test]
    fn a_cell_copies_as_obsidian_prints_it() {
        assert_eq!(copy_text(&Value::Null), "");
        assert_eq!(copy_text(&Value::Bool(false)), "false");
        assert_eq!(copy_text(&Value::Number(1.5)), "1.5");
        assert_eq!(copy_text(&Value::Number(30.)), "30");
        assert_eq!(
            copy_text(&Value::List(vec![Value::str("red"), Value::str("green")])),
            "red, green"
        );
    }

    #[test]
    fn a_copied_cell_keeps_its_type() {
        assert_eq!(copied_value(&Value::Number(1.5)), Prop::Number(1.5));
        assert_eq!(copied_value(&Value::Bool(true)), Prop::Checkbox(true));
        assert_eq!(copied_value(&Value::Null), Prop::Empty);
        assert_eq!(
            copied_value(&Value::List(vec![Value::str("red"), Value::str("green")])),
            Prop::List(vec!["red".into(), "green".into()])
        );
    }

    #[test]
    fn text_without_a_tab_over_one_cell_is_one_value() {
        assert_eq!(parse_text("r1\nr2\nr3", true), vec![vec!["r1\nr2\nr3"]]);
        assert_eq!(parse_text("c1\r\nc2\r\n", true), vec![vec!["c1\r\nc2\r\n"]]);
        // Over a range, it splits at line breaks.
        assert_eq!(
            parse_text("abc\n12,5", false),
            vec![vec!["abc"], vec!["12,5"]]
        );
        // A trailing newline is an empty last row.
        assert_eq!(
            parse_text("a, b\n[[Alpha]]\n", false),
            vec![vec!["a, b"], vec!["[[Alpha]]"], vec![""]]
        );
    }

    #[test]
    fn a_tab_makes_a_grid_and_quotes_are_not_unquoted() {
        assert_eq!(
            parse_text("x1\ty1\nx2\ty2", true),
            vec![vec!["x1", "y1"], vec!["x2", "y2"]]
        );
        assert_eq!(parse_text("\"x\ty\"", true), vec![vec!["\"x", "y\""]]);
        assert_eq!(
            parse_text("a\tb\r\nc\td", false),
            vec![vec!["a", "b"], vec!["c", "d"]]
        );
    }

    #[test]
    fn one_value_fills_the_range() {
        let range = CellRange {
            top: 0,
            bottom: 1,
            start: 1,
            end: 2,
        };
        let (writes, after) = paste_plan(range, &[vec![Some(text("F"))]], ALL, &props);
        assert_eq!(after, range);
        assert_eq!(writes.len(), 4);
        assert!(writes.iter().all(|w| w.value == text("F")));
    }

    #[test]
    fn a_block_lands_at_the_top_left_without_tiling() {
        // `m\nn` over four rows of one column writes two.
        let range = CellRange {
            top: 0,
            bottom: 3,
            start: 1,
            end: 1,
        };
        let cells = text_cells(parse_text("m\nn", false));
        let (writes, after) = paste_plan(range, &cells, ALL, &props);
        assert_eq!(
            writes,
            vec![
                CellWrite {
                    row: 0,
                    column: 1,
                    value: text("m")
                },
                CellWrite {
                    row: 1,
                    column: 1,
                    value: text("n")
                },
            ]
        );
        assert_eq!(after, CellRange::of((0, 1), (1, 1)));
    }

    #[test]
    fn each_cell_of_a_block_takes_its_own_value() {
        // Obsidian 1.15.1 wrote `b` twice here; Scoria writes `a` then `b`.
        let range = CellRange::of((0, 1), (0, 1));
        let cells = text_cells(parse_text("a\tb", true));
        let (writes, after) = paste_plan(range, &cells, ALL, &props);
        assert_eq!(
            writes
                .iter()
                .map(|w| (w.column, w.value.clone()))
                .collect::<Vec<_>>(),
            vec![(1, text("a")), (2, text("b"))]
        );
        assert_eq!(after, CellRange::of((0, 1), (0, 2)));
    }

    #[test]
    fn a_block_is_cut_at_the_last_row_and_column_and_skips_what_refuses() {
        let range = CellRange::of((3, 5), (3, 5));
        let cells = text_cells(parse_text("i1\tn1\textra\nj1\tm1\tx\nk\tl\tz", false));
        let not_formula = |c: usize| c != 6;
        let (writes, after) = paste_plan(range, &cells, ALL, &not_formula);
        assert_eq!(
            writes.iter().map(|w| (w.row, w.column)).collect::<Vec<_>>(),
            vec![(3, 5), (4, 5)]
        );
        assert_eq!(after, CellRange::of((3, 5), (4, 6)));
        // A copied cell that was no note property is skipped.
        let (writes, _) = paste_plan(
            CellRange::of((0, 6), (1, 6)),
            &[vec![None], vec![None]],
            ALL,
            &props,
        );
        assert!(writes.is_empty());
    }

    #[test]
    fn delete_empties_every_editable_cell() {
        let range = CellRange::of((0, 0), (1, 2));
        let writes = delete_plan(range, &|c| c != 0);
        assert_eq!(writes.len(), 4);
        assert!(writes
            .iter()
            .all(|w| w.value == Prop::Empty && w.column != 0));
        let rows = by_row(&writes);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].1.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }
}
