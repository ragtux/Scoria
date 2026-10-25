//! The element that draws a resolved base.
//!
//! The shape is Obsidian's own, read off a running 1.14.5 over CDP
//! (`tools/obsidian-probe/results/2026-10-25-bases.json` and
//! `-bases-expr.json`; the prose is in
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
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
//!   rendered as a `span.internal-link` rather than as plain text;
//! - a **parse error** as `.bases-error`, two paragraphs: the banner
//!   [`PARSE_ERROR_BANNER`] and then the message, with nothing joining them;
//! - an **unknown view type** as the same block with one paragraph,
//!   "Unknown view type: x", and no rows.
//!
//! The scroll body is virtualised: Obsidian's own table keeps `scrollEl` and
//! `theadEl` apart and calls `updateVirtualDisplay()` on resize, so the header
//! stays put while the rows scroll, and a few thousand rows cost only what is
//! on screen. The list is variable-height because a group header is not a row,
//! and a variable-height [`gpui::list`] must be **reset** when the data
//! changes — a splice lands at the end — so every rebuild goes through
//! [`BasesView::rebuild`].
//!
//! Read-only: a `.base` reaches here and is drawn, never written.

use crate::assets::icon;
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER};
use crate::workspace::VaultState;
use gpui::{
    div, list, prelude::*, px, AnyElement, Context, EventEmitter, FocusHandle, Focusable,
    ListAlignment, ListState, Render, SharedString, Window,
};
use scoria_bases::{Cell, CellKind, Column, ColumnSource, Row, Table, Value};
use scoria_formats::base::{Base, View, PARSE_ERROR_BANNER};
use serde_json::{json, Value as Json};
use std::sync::Arc;

/// The height of a column header, observed: the right-click target of a
/// header on `Layout.base` is 30 px tall.
const HEADER_HEIGHT: f32 = 30.;

/// The height of a row at the default `rowHeight` (`""`, Short).
///
/// **Unprobed.** The table layout carries `getRowBaseHeight()` and
/// `getRowHeightMult()`, but both are Obsidian's own source and were left out
/// of the run, and no row's rect was measured. A Short row the height of the
/// header it sits under is the reading consistent with the rest of the shell.
const ROW_HEIGHT: f32 = 30.;

/// The multiplier each `rowHeight` step adds over [`ROW_HEIGHT`].
///
/// The four values are observed — the Configure view page's Row height
/// dropdown offers `""` Short (the default, and the key is then absent),
/// `medium`, `tall` and `extra` (Extra tall) — but the **heights are
/// unprobed**: `getRowHeightMult()` is the function that answers them and it
/// is Obsidian's own source. One step per name is the plain reading.
fn row_height_mult(name: Option<&str>) -> f32 {
    match name {
        Some("medium") => 2.,
        Some("tall") => 3.,
        Some("extra") => 4.,
        // `""` Short, an absent key, and anything this build does not know.
        _ => 1.,
    }
}

/// The narrowest a column is drawn at.
///
/// Read off the two unsized columns of `Layout.base`: "file name" came out
/// 92 px and "status" 88 px, four pixels apart over three characters of
/// difference, so 88 is a floor the shorter label did not clear rather than
/// its content width. The table layout's own `minColWidth` field agrees that
/// there is one.
const MIN_COLUMN_WIDTH: f32 = 88.;

/// The widest an unsized column is drawn at. **Unprobed** — `maxColWidth` is
/// a field of the layout, but no column ever reached it in the run.
const MAX_COLUMN_WIDTH: f32 = 400.;

/// Padding inside a cell and a header, left and right. **Unprobed**: the
/// headers abut exactly (344 + 92 = 436, 436 + 88 = 524), so the padding is
/// inside the box, but its size was never read. 8 px is the shell's own
/// (`0 8px` on a dock tab header).
const CELL_PAD: f32 = 8.;

/// What the view asks the workspace to do; it owns no vault of its own.
pub enum BasesEvent {
    /// A row was activated: open this file.
    Open(String),
}

/// One line of the scroll body. A group header is not a row, which is why the
/// list is variable-height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// A `groupBy` header, by index into [`Table::groups`].
    Group(usize),
    /// A row: its group, and its index inside that group — or `None` and an
    /// index into [`Table::rows`] when the view does not group.
    Row { group: Option<usize>, ix: usize },
}

pub struct BasesView {
    focus: FocusHandle,
    path: Option<String>,
    /// The bytes the view last read, so the workspace can tell its own write
    /// coming back through the watcher from somebody else's.
    source: String,
    /// Which of the base's views is showing, by index into `base.views`.
    view_ix: usize,
    /// The file as it parsed, kept for the view's own layout keys
    /// (`rowHeight`, `columnSize`) and for its `groupBy`.
    base: Option<Base>,
    table: Option<Table>,
    /// A file that does not parse shows this instead of a table, under
    /// [`scoria_formats::base::PARSE_ERROR_BANNER`].
    error: Option<String>,
    state: Option<Arc<VaultState>>,
    /// The scroll body, flattened: group headers interleaved with rows.
    lines: Vec<Line>,
    /// One width per column, in the order the table lists them.
    widths: Vec<f32>,
    /// The height of one row, from the view's `rowHeight`.
    row_height: f32,
    list: ListState,
}

impl BasesView {
    pub fn new(cx: &mut Context<Self>) -> BasesView {
        BasesView {
            focus: cx.focus_handle(),
            path: None,
            source: String::new(),
            view_ix: 0,
            base: None,
            table: None,
            error: None,
            state: None,
            lines: Vec::new(),
            widths: Vec::new(),
            row_height: ROW_HEIGHT,
            list: ListState::new(0, ListAlignment::Top, px(400.)),
        }
    }

    /// Shows `path` with the bytes just read from it. An empty file is a base
    /// with one default view, which is what `bases:new-file` writes.
    pub fn load(&mut self, path: &str, text: &str, state: Arc<VaultState>, cx: &mut Context<Self>) {
        self.path = Some(path.to_string());
        self.source = text.to_string();
        self.state = Some(state);
        // A different file starts on its first view, as opening one does.
        self.view_ix = 0;
        self.resolve(cx);
    }

    /// The file changed underneath the view.
    ///
    /// Obsidian reloads the live query from the new bytes and keeps the view
    /// the leaf was on (the external rewrite of the fixture came back with its
    /// renamed view already in the query), so the index is kept here too and
    /// only clamped if the rewrite dropped views.
    pub fn reload(&mut self, text: &str, cx: &mut Context<Self>) {
        self.source = text.to_string();
        self.resolve(cx);
    }

    /// Lets go of the file, as the canvas does when another kind opens.
    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.path = None;
        self.base = None;
        self.table = None;
        self.error = None;
        self.source.clear();
        self.state = None;
        self.rebuild();
        cx.notify();
    }

    /// The vault changed under an open base: a note it queries was added,
    /// edited or removed. The bytes of the `.base` itself are unchanged, so
    /// the query is the same one and only its rows are resolved again.
    ///
    /// A base is a view over the whole vault rather than over one file, which
    /// is what makes this necessary at all: the canvas rebuilds a card when
    /// the file behind it changes, but every change is potentially a change
    /// to a base's table.
    pub fn invalidate(&mut self, state: Arc<VaultState>, cx: &mut Context<Self>) {
        if self.path.is_none() {
            return;
        }
        self.state = Some(state);
        self.resolve(cx);
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Whether the bytes on disk are the ones the view read.
    pub fn matches_file(&self, text: &str) -> bool {
        self.source == text
    }

    /// Parses [`BasesView::source`] and resolves the showing view against the
    /// vault. A file that does not parse leaves no table at all, which is what
    /// Obsidian does: the query is the error object, and nothing is drawn but
    /// the banner.
    fn resolve(&mut self, cx: &mut Context<Self>) {
        self.base = None;
        self.table = None;
        self.error = None;
        match Base::from_yaml(&self.source) {
            Err(e) => self.error = Some(e.to_string()),
            Ok(base) => {
                // A base with no views has one default view, so index 0 is
                // always valid; anything past the end falls back to it.
                if self.view_ix > 0 && self.view_ix >= base.views.len() {
                    self.view_ix = 0;
                }
                let env = match (self.state.as_ref(), self.path.as_deref()) {
                    (Some(state), Some(path)) => crate::bases::rows::env_for(state, path),
                    _ => scoria_bases::Env {
                        files: Vec::new(),
                        base_file: None,
                        now: chrono::Local::now().naive_local(),
                    },
                };
                self.table = Some(scoria_bases::table::build(&base, self.view_ix, &env));
                self.base = Some(base);
            }
        }
        self.rebuild();
        cx.notify();
    }

    /// The view whose rows are showing, when the file parsed and has one.
    fn view(&self) -> Option<&View> {
        self.base.as_ref()?.views.get(self.view_ix)
    }

    /// Rebuilds the scroll body and the measurements that depend on it.
    ///
    /// The list is **reset** rather than spliced: a splice on a
    /// variable-height list lands at the end, so rows would silently stop
    /// following the data. A base is redrawn only when the file or the vault
    /// changed, where Obsidian starts the view from its top too.
    fn rebuild(&mut self) {
        let grouped = self.view().is_some_and(|v| v.group_by.is_some());
        self.lines = match self.table.as_ref() {
            Some(table) => lines_for(table, grouped),
            None => Vec::new(),
        };
        self.widths = match self.table.as_ref() {
            Some(table) => column_widths(&table.columns, self.view()),
            None => Vec::new(),
        };
        self.row_height = ROW_HEIGHT
            * row_height_mult(
                self.view()
                    .and_then(|v| v.preserved.get("rowHeight"))
                    .and_then(|v| v.as_str()),
            );
        self.list.reset(self.lines.len());
    }

    /// What the control socket reports under `bases`.
    pub fn state_json(&self) -> Json {
        let columns: Vec<Json> = self
            .table
            .iter()
            .flat_map(|t| t.columns.iter())
            .map(|c| json!({ "id": c.id, "display": c.display }))
            .collect();
        let rows: Vec<Json> = self
            .table
            .iter()
            .flat_map(|t| t.rows.iter())
            .take(STATE_ROWS)
            .map(|r| {
                json!({
                    "file": r.file.path,
                    "cells": r.cells.iter().map(cell_text).collect::<Vec<_>>(),
                })
            })
            .collect();
        let groups: Vec<Json> = self
            .table
            .iter()
            .flat_map(|t| t.groups.iter())
            .map(|g| json!({ "key": g.key, "rows": g.rows.len() }))
            .collect();
        json!({
            "path": self.path,
            "error": self.error,
            "view": self.view_ix,
            "view_name": self.view().map(|v| v.name.clone()),
            "unknown_kind": self.table.as_ref().and_then(|t| t.unknown_kind.clone()),
            "columns": columns,
            "rows": rows,
            "rows_total": self.table.as_ref().map_or(0, |t| t.rows.len()),
            "limited": self.table.as_ref().is_some_and(|t| t.limited),
            "groups": groups,
        })
    }

    // ----- rendering ----------------------------------------------------------

    /// `.bases-error`: one paragraph per message, in the view's body.
    ///
    /// A parse failure draws two — [`PARSE_ERROR_BANNER`] and then the
    /// message, with nothing joining them — and an unknown view type draws one.
    /// The message is kept line for line in the mono family because the YAML
    /// library's own text carries a caret under the offending column, which
    /// only lines up in a fixed pitch. **Unprobed**: the block's own type and
    /// colour were never read off the running app.
    fn render_error(&self, banner: bool, message: &str, theme: &Theme) -> AnyElement {
        let mut block = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(24.))
            .text_size(px(theme.base_font_size))
            .text_color(theme.text);
        if banner {
            block = block.child(SharedString::from(PARSE_ERROR_BANNER));
        }
        block
            .child(
                div()
                    .flex()
                    .flex_col()
                    .font_family(theme.mono_font.clone())
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.text_muted)
                    .children(message.lines().map(|line| {
                        // An empty line still needs a line box, or the caret
                        // under the snippet climbs onto the text above it.
                        div().child(SharedString::from(if line.is_empty() {
                            " ".to_string()
                        } else {
                            line.to_string()
                        }))
                    })),
            )
            .into_any_element()
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
                let width = self.widths.get(ix).copied().unwrap_or(MIN_COLUMN_WIDTH);
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
        let Some(line) = self.lines.get(ix).copied() else {
            return div().into_any_element();
        };
        let Some(table) = self.table.as_ref() else {
            return div().into_any_element();
        };
        match line {
            Line::Group(g) => {
                let Some(group) = table.groups.get(g) else {
                    return div().into_any_element();
                };
                self.render_group(ix, group.key.as_deref(), group.rows.len(), &theme)
            }
            Line::Row { .. } => {
                let Some(row) = row_of(table, line) else {
                    return div().into_any_element();
                };
                self.render_row(ix, row, &theme, cx)
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
        let columns = self.table.as_ref().map(|t| t.columns.as_slice());
        div()
            .id(("bases-row", ix))
            .flex()
            .flex_row()
            .w_full()
            .h(px(self.row_height))
            .border_b_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover(|s| s.bg(theme.bg_alt))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(BasesEvent::Open(path.clone()));
            }))
            .children(row.cells.iter().enumerate().map(|(c, cell)| {
                let width = self.widths.get(c).copied().unwrap_or(MIN_COLUMN_WIDTH);
                let link = columns
                    .and_then(|cols| cols.get(c))
                    .is_some_and(|col| renders_as_link(col, cell));
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

    /// A view that resolved to no rows at all.
    ///
    /// **Unprobed**: the run never rendered an empty table, so neither the
    /// wording nor the box is observed. This is the shell's own `pane-empty` —
    /// 13 px faint, centred, padded `24px 16px` — which is what the outline
    /// pane draws for the same situation.
    fn render_empty(&self, theme: &Theme) -> AnyElement {
        div()
            .flex_1()
            .w_full()
            .flex()
            .items_start()
            .justify_center()
            .py(px(24.))
            .px(px(16.))
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.text_faint)
            .child(SharedString::from("No results"))
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
fn lines_for(table: &Table, grouped: bool) -> Vec<Line> {
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
fn row_of(table: &Table, line: Line) -> Option<&Row> {
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
/// the view's layout keys, so it lives in the view's preserved map. A column
/// the map does not name is sized from its label, floored at
/// [`MIN_COLUMN_WIDTH`], which is the only reading that fits both unsized
/// columns of the fixture (92 for "file name", 88 for "status"). The label's
/// own width is **estimated** rather than measured: the real table content-fits
/// its header with the text it has already laid out, and this element sizes
/// its columns before the text exists.
fn column_widths(columns: &[Column], view: Option<&View>) -> Vec<f32> {
    let sizes = view.and_then(|v| v.preserved.get("columnSize"));
    columns
        .iter()
        .map(|col| {
            let given = sizes
                .and_then(|m| m.get(col.id.as_str()))
                .and_then(|v| v.as_f64())
                .filter(|w| *w > 0.);
            match given {
                Some(w) => w as f32,
                // ~7 px a glyph at 13 px of Inter, plus the cell's two pads
                // and the room Obsidian leaves for a header's type icon.
                None => (col.display.chars().count() as f32 * 7. + 2. * CELL_PAD + 24.)
                    .clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH),
            }
        })
        .collect()
}

/// What a cell draws.
///
/// [`Cell::text`] already carries an evaluation refusal as its content, which
/// is what Obsidian renders for one; this prefers the error anyway so a cell
/// whose text was never filled in still says why.
fn cell_text(cell: &Cell) -> String {
    match (&cell.error, cell.text.is_empty()) {
        (Some(e), true) => e.clone(),
        _ => cell.text.clone(),
    }
}

/// Whether a cell draws as an internal link.
///
/// The one link the run rendered is the `file.name` column's — a
/// `span.internal-link` inside a `markdown-rendered` cell where every other
/// cell is bare text — and a Link value reaches a cell as its display text, so
/// both are drawn in the link colour. **Unprobed**: whether `file.path` and
/// the other file columns are links too.
fn renders_as_link(column: &Column, cell: &Cell) -> bool {
    matches!(cell.value, Value::Link(_) | Value::File(_))
        || matches!(&column.source, ColumnSource::File(f) if f.eq_ignore_ascii_case("name"))
}

/// Obsidian's `<input disabled type="checkbox">`: the box the reading view's
/// property rows draw, dimmed, because a base's cell is never editable here.
fn checkbox(checked: bool, theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .opacity(0.5)
        .w(px(theme.base_font_size))
        .h(px(theme.base_font_size))
        .border_1()
        .border_color(theme.border)
        .rounded_sm()
        .when(checked, |d| {
            d.bg(theme.accent).text_color(theme.bg).child(icon(
                "check",
                theme.base_font_size - 4.,
                theme.bg,
            ))
        })
}

/// How many rows [`BasesView::state_json`] lists before it stops; the whole
/// count is always there as `rows_total`.
const STATE_ROWS: usize = 100;

impl EventEmitter<BasesEvent> for BasesView {}

impl Focusable for BasesView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BasesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let body: Vec<AnyElement> = if let Some(message) = self.error.clone() {
            vec![self.render_error(true, &message, &theme)]
        } else if let Some(kind) = self.table.as_ref().and_then(|t| t.unknown_kind.clone()) {
            // The file parsed; only the layout is one this build cannot draw.
            vec![self.render_error(false, &format!("Unknown view type: {kind}"), &theme)]
        } else if let Some(table) = self.table.as_ref() {
            let header = self.render_header(table, &theme);
            let rows = if self.lines.is_empty() {
                self.render_empty(&theme)
            } else {
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .child(list(self.list.clone(), cx.processor(Self::render_line)).size_full())
                    .into_any_element()
            };
            vec![header, rows]
        } else {
            Vec::new()
        };
        div()
            .track_focus(&self.focus)
            .key_context("Bases")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.text)
            .children(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_bases::{FileData, Group};

    fn file(path: &str) -> Arc<FileData> {
        Arc::new(FileData {
            name: path.to_string(),
            basename: path.to_string(),
            fullname: path.to_string(),
            path: path.to_string(),
            folder: String::new(),
            ext: "md".into(),
            size: 0.,
            ctime: None,
            mtime: None,
            tags: Vec::new(),
            links: Vec::new(),
            embeds: Vec::new(),
            backlinks: Vec::new(),
            properties: Default::default(),
        })
    }

    fn cell(text: &str, kind: CellKind, value: Value) -> Cell {
        Cell {
            value,
            error: None,
            kind,
            text: text.to_string(),
        }
    }

    fn row(path: &str) -> Row {
        Row {
            file: file(path),
            cells: vec![cell(path, CellKind::Text, Value::Str(path.to_string()))],
        }
    }

    fn column(id: &str, display: &str, source: ColumnSource) -> Column {
        Column {
            id: id.to_string(),
            display: display.to_string(),
            source,
        }
    }

    fn view_with(yaml: &str) -> View {
        let base = Base::from_yaml(yaml).expect("fixture parses");
        base.views.into_iter().next().expect("one view")
    }

    #[test]
    fn an_ungrouped_table_is_one_line_a_row() {
        let table = Table {
            columns: vec![column(
                "file.name",
                "file name",
                ColumnSource::File("name".into()),
            )],
            rows: vec![row("A.md"), row("B.md")],
            groups: vec![Group {
                key: None,
                rows: vec![row("A.md"), row("B.md")],
            }],
            limited: false,
            unknown_kind: None,
        };
        let lines = lines_for(&table, false);
        assert_eq!(
            lines,
            vec![
                Line::Row { group: None, ix: 0 },
                Line::Row { group: None, ix: 1 },
            ]
        );
        assert_eq!(row_of(&table, lines[1]).unwrap().file.path, "B.md");
    }

    #[test]
    fn a_grouped_table_puts_a_header_before_each_group() {
        let table = Table {
            columns: Vec::new(),
            rows: vec![row("A.md"), row("B.md"), row("C.md")],
            groups: vec![
                Group {
                    key: Some("todo".into()),
                    rows: vec![row("A.md"), row("B.md")],
                },
                Group {
                    key: None,
                    rows: vec![row("C.md")],
                },
            ],
            limited: false,
            unknown_kind: None,
        };
        let lines = lines_for(&table, true);
        assert_eq!(
            lines,
            vec![
                Line::Group(0),
                Line::Row {
                    group: Some(0),
                    ix: 0
                },
                Line::Row {
                    group: Some(0),
                    ix: 1
                },
                Line::Group(1),
                Line::Row {
                    group: Some(1),
                    ix: 0
                },
            ]
        );
        assert_eq!(row_of(&table, lines[4]).unwrap().file.path, "C.md");
        assert!(row_of(&table, lines[0]).is_none());
    }

    #[test]
    fn a_grouped_view_with_no_groups_falls_back_to_the_flat_rows() {
        let table = Table {
            columns: Vec::new(),
            rows: vec![row("A.md")],
            groups: Vec::new(),
            limited: false,
            unknown_kind: None,
        };
        assert_eq!(
            lines_for(&table, true),
            vec![Line::Row { group: None, ix: 0 }]
        );
    }

    #[test]
    fn row_height_follows_the_views_own_key() {
        // The four values the Configure view page's dropdown offers.
        assert_eq!(row_height_mult(None), 1.);
        assert_eq!(row_height_mult(Some("")), 1.);
        assert_eq!(row_height_mult(Some("medium")), 2.);
        assert_eq!(row_height_mult(Some("tall")), 3.);
        assert_eq!(row_height_mult(Some("extra")), 4.);
        // A value this build does not know draws a Short row rather than none.
        assert_eq!(row_height_mult(Some("enormous")), 1.);
    }

    #[test]
    fn a_column_size_the_view_carries_wins_over_the_label() {
        // The bytes a header drag wrote on the fixture.
        let view = view_with(
            "views:\n  - type: table\n    name: My table\n    columnSize:\n      file.name: 232\n",
        );
        let columns = vec![
            column("file.name", "file name", ColumnSource::File("name".into())),
            column("status", "status", ColumnSource::Property("status".into())),
        ];
        let widths = column_widths(&columns, Some(&view));
        assert_eq!(widths[0], 232.);
        // Unsized, and short enough to sit on the floor the fixture showed.
        assert_eq!(widths[1], MIN_COLUMN_WIDTH);
    }

    #[test]
    fn an_unsized_column_never_leaves_the_floor_and_ceiling() {
        let columns = vec![
            column("a", "a", ColumnSource::Property("a".into())),
            column(
                "long",
                &"x".repeat(200),
                ColumnSource::Property("long".into()),
            ),
        ];
        let widths = column_widths(&columns, None);
        assert_eq!(widths[0], MIN_COLUMN_WIDTH);
        assert_eq!(widths[1], MAX_COLUMN_WIDTH);
    }

    #[test]
    fn a_cell_draws_its_error_when_it_has_no_text_of_its_own() {
        let plain = cell("abcd", CellKind::Text, Value::Str("abcd".into()));
        assert_eq!(cell_text(&plain), "abcd");
        // An evaluation refusal is the cell's content, which `Cell::of` puts
        // in both fields; a cell carrying only the error still says why.
        let mut failed = cell("", CellKind::Text, Value::Null);
        failed.error = Some("Cannot find function \"nope\"".into());
        assert_eq!(cell_text(&failed), "Cannot find function \"nope\"");
        // A boolean's text is empty even when the value is `true`.
        let boxed = cell("", CellKind::Checkbox, Value::Bool(true));
        assert_eq!(cell_text(&boxed), "");
    }

    #[test]
    fn the_file_name_column_draws_as_a_link() {
        let name = column("file.name", "file name", ColumnSource::File("name".into()));
        let status = column("status", "status", ColumnSource::Property("status".into()));
        let text = cell("Scoria", CellKind::Text, Value::Str("Scoria".into()));
        assert!(renders_as_link(&name, &text));
        assert!(!renders_as_link(&status, &text));
        // A Link value is one wherever it sits.
        let link = cell(
            "Pumice",
            CellKind::Text,
            Value::Link(scoria_bases::LinkValue {
                path: "Pumice".into(),
                display: None,
            }),
        );
        assert!(renders_as_link(&status, &link));
    }

    #[test]
    fn a_base_that_does_not_parse_has_a_message_and_no_table() {
        // The fixture's YAML syntax error, which Obsidian never writes back.
        let err = Base::from_yaml("filters:\n  and:\n    - 'a == 1'\n  bad: [unclosed\nviews:\n")
            .expect_err("broken YAML");
        assert!(!err.to_string().is_empty());
        // And the banner the view puts above it is Obsidian's own wording.
        assert_eq!(PARSE_ERROR_BANNER, "Unable to parse your base file:");
    }
}
