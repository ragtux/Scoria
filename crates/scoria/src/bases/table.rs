//! The `table` layout: a header row over virtualised rows.
//!
//! The shape is Obsidian's own, read off a running 1.14.5 over CDP
//! (`tools/obsidian-probe/results/2026-10-25-bases.json`; the prose is in
//! [[Specification#Observed Bases table view, Obsidian 1.14.5 (installer build 1.13.7)]]):
//!
//! - a **header row** of column display names, each header a fixed-width box
//!   (`columnSize` when the view carries one) with the last column taking the
//!   rest of the width. Observed on `Layout.base`: three headers at x 344, 436
//!   and 524, 92, 88 and 487 wide, 30 px tall, abutting with no gap, the first
//!   flush with the pane's left edge (ribbon 44 + sidebar 300 = 344), and the
//!   last one far wider than its text;
//! - the **rows** beneath it, each cell drawn by its `CellKind`, which is
//!   Obsidian's `data-property-type`: `text`, a `number`, or a **disabled
//!   checkbox** whose text is empty even when the value is `true`
//!   (`<div class="bases-table-cell bases-rendered-value"
//!   data-property-type="checkbox"><input disabled type="checkbox">`);
//! - the `file.name` column as an internal link, which is the one cell the run
//!   rendered as a `span.internal-link` rather than as plain text.
//!
//! The scroll body is virtualised: Obsidian's own table keeps `scrollEl` and
//! `theadEl` apart and calls `updateVirtualDisplay()` on resize, so the header
//! stays put while the rows scroll, and a few thousand rows cost only what is
//! on screen. The list is variable-height because a group header is not a row,
//! and a variable-height [`gpui::list`] must be **reset** when the data
//! changes — a splice lands at the end — so every rebuild goes through
//! [`BasesView::rebuild`](crate::bases::BasesView).

use crate::bases::cells::{cell_text, checkbox, renders_as_link, CELL_PAD};
use crate::bases::view::BasesView;
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER};
use gpui::{
    div, list, prelude::*, px, AnyElement, Context, ListAlignment, ListState, SharedString, Window,
};
use scoria_bases::{CellKind, Column, Row, Table, TableLayout, Value};
use serde_json::{json, Value as Json};

/// The height of a column header, observed: the right-click target of a
/// header on `Layout.base` is 30 px tall.
pub const HEADER_HEIGHT: f32 = 30.;

/// The height of a row at the default `rowHeight` (`""`, Short).
///
/// **Unprobed.** The table layout carries `getRowBaseHeight()` and
/// `getRowHeightMult()`, but both are Obsidian's own source and were left out
/// of the run, and no row's rect was measured. A Short row the height of the
/// header it sits under is the reading consistent with the rest of the shell.
pub const ROW_HEIGHT: f32 = 30.;

/// The narrowest a column is drawn at.
///
/// Read off the two unsized columns of `Layout.base`: "file name" came out
/// 92 px and "status" 88 px, four pixels apart over three characters of
/// difference, so 88 is a floor the shorter label did not clear rather than
/// its content width. The table layout's own `minColWidth` field agrees that
/// there is one.
pub const MIN_COLUMN_WIDTH: f32 = 88.;

/// The widest an unsized column is drawn at. **Unprobed** — `maxColWidth` is
/// a field of the layout, but no column ever reached it in the run.
pub const MAX_COLUMN_WIDTH: f32 = 400.;

/// One line of the scroll body. A group header is not a row, which is why the
/// list is variable-height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// A `groupBy` header, by index into [`Table::groups`].
    Group(usize),
    /// A row: its group, and its index inside that group — or `None` and an
    /// index into [`Table::rows`] when the view does not group.
    Row { group: Option<usize>, ix: usize },
}

/// What the table layout keeps between renders.
pub struct TableState {
    /// The scroll body, flattened: group headers interleaved with rows.
    pub lines: Vec<Line>,
    /// One width per column, in the order the table lists them.
    pub widths: Vec<f32>,
    /// The height of one row, from the view's `rowHeight`.
    pub row_height: f32,
    pub list: ListState,
}

impl Default for TableState {
    fn default() -> TableState {
        TableState {
            lines: Vec::new(),
            widths: Vec::new(),
            row_height: ROW_HEIGHT,
            list: ListState::new(0, ListAlignment::Top, px(400.)),
        }
    }
}

impl TableState {
    /// What the control socket reports for the table layout.
    pub fn state_json(&self) -> Json {
        json!({
            "lines": self.lines.len(),
            "widths": self.widths,
            "row_height": self.row_height,
        })
    }
}

impl BasesView {
    /// Rebuilds the scroll body and the measurements that depend on it.
    ///
    /// The list is **reset** rather than spliced: a splice on a
    /// variable-height list lands at the end, so rows would silently stop
    /// following the data. A base is redrawn only when the file or the vault
    /// changed, where Obsidian starts the view from its top too.
    pub(super) fn rebuild_table(&mut self) {
        let grouped = self.view().is_some_and(|v| v.group_by.is_some());
        let layout = match self.layout() {
            scoria_bases::Layout::Table(table) => table.clone(),
            _ => TableLayout::default(),
        };
        let (lines, widths) = match self.table() {
            Some(table) => (
                lines_for(table, grouped),
                column_widths(&table.columns, &layout),
            ),
            None => (Vec::new(), Vec::new()),
        };
        self.table_state.lines = lines;
        self.table_state.widths = widths;
        self.table_state.row_height = ROW_HEIGHT * layout.row_height.mult();
        let count = self.table_state.lines.len();
        self.table_state.list.reset(count);
    }

    /// The header row over the virtualised body, or the empty state when the
    /// view resolved to no rows at all.
    pub(super) fn render_table(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(table) = self.table() else {
            return Vec::new();
        };
        let header = self.render_header(table, theme);
        let body = if self.table_state.lines.is_empty() {
            self.render_empty(theme)
        } else {
            div()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .child(
                    list(
                        self.table_state.list.clone(),
                        cx.processor(Self::render_line),
                    )
                    .size_full(),
                )
                .into_any_element()
        };
        vec![header, body]
    }

    /// The header row: one box per column, abutting, the last one taking the
    /// rest of the width, over a hairline.
    fn render_header(&self, table: &Table, theme: &Theme) -> AnyElement {
        let last = table.columns.len().saturating_sub(1);
        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .border_b_1()
            .border_color(theme.border)
            .children(table.columns.iter().enumerate().map(|(ix, col)| {
                let width = self
                    .table_state
                    .widths
                    .get(ix)
                    .copied()
                    .unwrap_or(MIN_COLUMN_WIDTH);
                let mut cell = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h_full()
                    .px(px(CELL_PAD))
                    .overflow_hidden()
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.text_muted)
                    .when(ix > 0, |d| d.border_l_1().border_color(theme.border));
                cell = if ix == last {
                    cell.flex_1().min_w(px(width))
                } else {
                    cell.flex_shrink_0().w(px(width))
                };
                cell.child(
                    div()
                        .min_w_0()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(SharedString::from(col.display.clone())),
                )
            }))
            .into_any_element()
    }

    /// One line of the scroll body.
    fn render_line(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.global::<Theme>().clone();
        let Some(line) = self.table_state.lines.get(ix).copied() else {
            return div().into_any_element();
        };
        let Some(table) = self.table() else {
            return div().into_any_element();
        };
        match line {
            Line::Group(g) => {
                let Some(group) = table.groups.get(g) else {
                    return div().into_any_element();
                };
                let (key, rows) = (group.key.clone(), group.rows.len());
                self.render_group(ix, key.as_deref(), rows, &theme)
            }
            Line::Row { .. } => {
                let Some(row) = row_of(table, line).cloned() else {
                    return div().into_any_element();
                };
                self.render_row(ix, &row, &theme, cx)
            }
        }
    }

    /// A `groupBy` header.
    ///
    /// **Unprobed**: no grouped table's DOM was captured, so the shape here is
    /// the shell's own — the key at the row height in muted 13 px with the
    /// group's size as a faint flair, the way the search view labels a file.
    /// A group with no key is Obsidian's "no value for this property" group,
    /// and the word it uses for one was never read; "Empty" is what the
    /// property editor writes for a blank value.
    fn render_group(&self, ix: usize, key: Option<&str>, rows: usize, theme: &Theme) -> AnyElement {
        div()
            .id(("bases-group", ix))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .px(px(CELL_PAD))
            .gap(px(8.))
            .bg(theme.bg_secondary)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.text)
                    .child(SharedString::from(key.unwrap_or("Empty").to_string())),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(FONT_UI_SMALLER))
                    .text_color(theme.text_faint)
                    .child(SharedString::from(rows.to_string())),
            )
            .into_any_element()
    }

    /// One row: a cell per column, activating the file it came from.
    fn render_row(
        &self,
        ix: usize,
        row: &Row,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = row.file.path.clone();
        let last = row.cells.len().saturating_sub(1);
        let columns: Vec<Column> = self.table().map(|t| t.columns.clone()).unwrap_or_default();
        div()
            .id(("bases-row", ix))
            .flex()
            .flex_row()
            .w_full()
            .h(px(self.table_state.row_height))
            .border_b_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover(|s| s.bg(theme.bg_alt))
            .on_click(cx.listener(move |this, _, _, cx| this.open(path.clone(), cx)))
            .children(row.cells.iter().enumerate().map(|(c, cell)| {
                let width = self
                    .table_state
                    .widths
                    .get(c)
                    .copied()
                    .unwrap_or(MIN_COLUMN_WIDTH);
                let link = columns.get(c).is_some_and(|col| renders_as_link(col, cell));
                let mut box_ = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h_full()
                    .px(px(CELL_PAD))
                    .overflow_hidden()
                    .text_size(px(FONT_UI_SMALL))
                    .when(c > 0, |d| d.border_l_1().border_color(theme.border));
                box_ = if c == last {
                    box_.flex_1().min_w(px(width))
                } else {
                    box_.flex_shrink_0().w(px(width))
                };
                match cell.kind {
                    // A boolean is a disabled checkbox, so there is no text
                    // even when the value is `true`.
                    CellKind::Checkbox => {
                        box_.child(checkbox(matches!(cell.value, Value::Bool(true)), theme))
                    }
                    // **Unprobed**: no number column's alignment was measured;
                    // right is the convention a number column reads in.
                    CellKind::Number => box_.justify_end().text_color(theme.text).child(
                        div()
                            .min_w_0()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .child(SharedString::from(cell_text(cell))),
                    ),
                    CellKind::Text => box_
                        .text_color(if link { theme.link } else { theme.text })
                        .child(
                            div()
                                .min_w_0()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(SharedString::from(cell_text(cell))),
                        ),
                }
            }))
            .into_any_element()
    }
}

/// The scroll body of a resolved table: group headers interleaved with rows
/// when the view groups, and the flat rows when it does not.
///
/// A view with no `groupBy` still has one group holding the same rows, so the
/// caller says whether the view groups rather than this guessing from the
/// shape; a grouped view whose groups came back empty falls back to the flat
/// rows so nothing disappears.
pub fn lines_for(table: &Table, grouped: bool) -> Vec<Line> {
    if grouped && !table.groups.is_empty() {
        let mut out = Vec::with_capacity(table.rows.len() + table.groups.len());
        for (g, group) in table.groups.iter().enumerate() {
            out.push(Line::Group(g));
            out.extend((0..group.rows.len()).map(|ix| Line::Row { group: Some(g), ix }));
        }
        out
    } else {
        (0..table.rows.len())
            .map(|ix| Line::Row { group: None, ix })
            .collect()
    }
}

/// The row a line points at, through its group when it has one.
pub fn row_of(table: &Table, line: Line) -> Option<&Row> {
    match line {
        Line::Group(_) => None,
        Line::Row { group: Some(g), ix } => table.groups.get(g)?.rows.get(ix),
        Line::Row { group: None, ix } => table.rows.get(ix),
    }
}

/// How wide each column is drawn.
///
/// `columnSize` is observed — a header drag wrote `columnSize:` /
/// `  file.name: 232` and the header then measured 232 px — and it is one of
/// the view's layout keys, so it arrives through
/// [`scoria_bases::TableLayout`]. A column the map does not name is sized
/// from its label, floored at [`MIN_COLUMN_WIDTH`], which is the only reading
/// that fits both unsized columns of the fixture (92 for "file name", 88 for
/// "status"). The label's own width is **estimated** rather than measured: the
/// real table content-fits its header with the text it has already laid out,
/// and this element sizes its columns before the text exists.
pub fn column_widths(columns: &[Column], layout: &TableLayout) -> Vec<f32> {
    columns
        .iter()
        .map(|col| match layout.width_of(&col.id) {
            Some(w) => w,
            // ~7 px a glyph at 13 px of Inter, plus the cell's two pads
            // and the room Obsidian leaves for a header's type icon.
            None => (col.display.chars().count() as f32 * 7. + 2. * CELL_PAD + 24.)
                .clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_bases::{Cell, ColumnSource, FileData, Group};
    use std::sync::Arc;

    fn file(path: &str) -> Arc<FileData> {
        Arc::new(FileData {
            path: path.to_string(),
            name: path.to_string(),
            basename: path.trim_end_matches(".md").to_string(),
            ..FileData::default()
        })
    }

    fn row(path: &str) -> Row {
        Row {
            file: file(path),
            cells: vec![Cell {
                value: Value::Str(path.to_string()),
                error: None,
                kind: CellKind::Text,
                text: path.to_string(),
            }],
            image: None,
        }
    }

    fn column(id: &str, display: &str, source: ColumnSource) -> Column {
        Column {
            id: id.to_string(),
            display: display.to_string(),
            source,
        }
    }

    fn table_of(rows: Vec<Row>, groups: Vec<Group>) -> Table {
        Table {
            columns: vec![column(
                "file.name",
                "file name",
                ColumnSource::File("name".into()),
            )],
            rows,
            groups,
            limited: false,
            unknown_kind: None,
        }
    }

    #[test]
    fn an_ungrouped_table_is_one_line_a_row() {
        let rows = vec![row("A.md"), row("B.md")];
        let table = table_of(
            rows.clone(),
            vec![Group {
                key: None,
                rows: rows.clone(),
            }],
        );
        assert_eq!(
            lines_for(&table, false),
            vec![
                Line::Row { group: None, ix: 0 },
                Line::Row { group: None, ix: 1 },
            ]
        );
        assert_eq!(
            row_of(&table, Line::Row { group: None, ix: 1 }),
            Some(&rows[1])
        );
    }

    #[test]
    fn a_grouped_table_puts_a_header_before_each_group() {
        let a = row("A.md");
        let b = row("B.md");
        let table = table_of(
            vec![a.clone(), b.clone()],
            vec![
                Group {
                    key: None,
                    rows: vec![a.clone()],
                },
                Group {
                    key: Some("todo".to_string()),
                    rows: vec![b.clone()],
                },
            ],
        );
        assert_eq!(
            lines_for(&table, true),
            vec![
                Line::Group(0),
                Line::Row {
                    group: Some(0),
                    ix: 0
                },
                Line::Group(1),
                Line::Row {
                    group: Some(1),
                    ix: 0
                },
            ]
        );
        assert_eq!(
            row_of(
                &table,
                Line::Row {
                    group: Some(1),
                    ix: 0
                }
            ),
            Some(&b)
        );
        assert_eq!(row_of(&table, Line::Group(0)), None);
    }

    #[test]
    fn a_grouped_view_with_no_groups_falls_back_to_the_flat_rows() {
        let table = table_of(vec![row("A.md")], Vec::new());
        assert_eq!(
            lines_for(&table, true),
            vec![Line::Row { group: None, ix: 0 }]
        );
    }

    #[test]
    fn a_column_size_the_view_carries_wins_over_the_label() {
        let mut layout = TableLayout::default();
        layout.column_size.insert("file.name".to_string(), 232.);
        let widths = column_widths(
            &[column(
                "file.name",
                "file name",
                ColumnSource::File("name".into()),
            )],
            &layout,
        );
        assert_eq!(widths, vec![232.]);
    }

    #[test]
    fn an_unsized_column_never_leaves_the_floor_and_ceiling() {
        let layout = TableLayout::default();
        let widths = column_widths(
            &[
                column("s", "s", ColumnSource::Property("s".into())),
                column(
                    "long",
                    &"x".repeat(200),
                    ColumnSource::Property("long".into()),
                ),
            ],
            &layout,
        );
        assert_eq!(widths[0], MIN_COLUMN_WIDTH);
        assert_eq!(widths[1], MAX_COLUMN_WIDTH);
    }
}
