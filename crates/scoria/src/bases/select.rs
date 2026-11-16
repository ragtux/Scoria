//! The table's cell selection in the view: Shift+arrows, Shift+click and
//! Ctrl+A make a range, Ctrl+C copies it, Ctrl+V pastes over it, Delete and
//! Backspace empty it.
//!
//! The rules are [`scoria_bases::selection`]'s, in row ordinals; this module
//! maps them onto the table's flattened lines (group headings and summary
//! rows are lines too, never cells), reads the cells a copy needs, and hands
//! each row's writes to the workspace as one [`BasesEvent::SetProperties`],
//! since a row is another note and only the workspace writes notes. Observed
//! on Obsidian 1.15.1
//! ([[Specification#Observed Bases cell selection, Obsidian 1.15.1]]).

use crate::bases::cells::{row_of, Line};
use crate::bases::edit::editable_key;
use crate::bases::toolbar::{Panel, RedoCells, UndoCells};
use crate::bases::undo::{NoteChange, Step};
use crate::bases::view::{BasesEvent, BasesView};
use gpui::{ClipboardItem, Context, Global, Window};
use scoria_bases::selection::{
    by_row, copied_value, copy_text, delete_plan, encode_tsv, parse_text, paste_plan, text_cells,
    Bounds, CellRange, CopiedCell, Selection,
};

/// The last copy the table made: its text, and the typed cells behind it.
///
/// Obsidian puts `obsidian/bases-cells` on the clipboard beside the text, so
/// a paste of its own copy writes a list as a list and `1.5` as a number.
/// gpui's clipboard carries text, so the typed cells stay here, and a paste
/// whose text is still exactly this text takes them; anything else on the
/// clipboard is outside text and pastes as strings.
#[derive(Default)]
pub struct CopiedCells {
    pub text: String,
    pub cells: Vec<Vec<CopiedCell>>,
}

impl Global for CopiedCells {}

impl BasesView {
    /// The lines that are rows, in order: what a row ordinal indexes.
    fn row_line_indexes(&self) -> Vec<usize> {
        self.table_state
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| matches!(line, Line::Row { .. }))
            .map(|(ix, _)| ix)
            .collect()
    }

    fn columns(&self) -> usize {
        self.table().map_or(0, |t| t.columns.len())
    }

    /// The rows a range from `ordinal` may span: the rows of its group,
    /// observed — Shift+Down stopped at a group's last row.
    fn bounds_at(&self, ordinal: usize) -> Option<Bounds> {
        let rows = self.row_line_indexes();
        let group_of = |ord: usize| match self.table_state.lines.get(rows[ord]) {
            Some(Line::Row { group, .. }) => *group,
            _ => None,
        };
        if ordinal >= rows.len() {
            return None;
        }
        let g = group_of(ordinal);
        let mut first = ordinal;
        while first > 0 && group_of(first - 1) == g {
            first -= 1;
        }
        let mut last = ordinal;
        while last + 1 < rows.len() && group_of(last + 1) == g {
            last += 1;
        }
        Some(Bounds {
            first_row: first,
            last_row: last,
            columns: self.columns(),
        })
    }

    /// The selection in row ordinals, from the cursor the table keeps.
    pub(super) fn selection(&self) -> Option<Selection> {
        let (line, column) = self.table_state.focused?;
        let ordinal = self.row_line_indexes().iter().position(|l| *l == line)?;
        Some(Selection {
            cursor: (ordinal, column),
            corner: self.table_state.corner,
            all: self.table_state.all,
        })
    }

    fn set_selection(&mut self, sel: Selection, cx: &mut Context<Self>) {
        let rows = self.row_line_indexes();
        if let Some(line) = rows.get(sel.cursor.0) {
            self.table_state.focused = Some((*line, sel.cursor.1));
            self.table_state.corner = sel.corner;
            self.table_state.all = sel.all;
            cx.notify();
        }
    }

    /// Drops the range and keeps the cursor: Escape, a plain arrow, a plain
    /// click.
    pub(super) fn collapse_selection(&mut self) {
        self.table_state.corner = None;
        self.table_state.all = None;
    }

    /// The range drawn as selected, `None` with no range up.
    pub(super) fn selected_range(&self) -> Option<CellRange> {
        self.selection()
            .filter(Selection::has_range)
            .map(|s| s.range())
    }

    /// Whether line `line`, column `column` is inside the range.
    pub(super) fn cell_in_range(
        &self,
        range: Option<CellRange>,
        line: usize,
        column: usize,
    ) -> bool {
        let Some(range) = range else {
            return false;
        };
        self.row_line_indexes()
            .iter()
            .position(|l| *l == line)
            .is_some_and(|ord| range.contains((ord, column)))
    }

    /// Shift+arrow.
    pub(super) fn extend_selection(&mut self, d_row: i32, d_col: i32, cx: &mut Context<Self>) {
        let Some(mut sel) = self.selection() else {
            return;
        };
        let Some(bounds) = self.bounds_at(sel.cursor.0) else {
            return;
        };
        sel.extend(d_row, d_col, bounds);
        self.set_selection(sel, cx);
    }

    /// Shift+click on line `line`, column `column`: the range runs from the
    /// cursor to it. With no cursor yet the click only places one.
    pub(super) fn shift_click(
        &mut self,
        line: usize,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(cx);
        let Some(ordinal) = self.row_line_indexes().iter().position(|l| *l == line) else {
            return;
        };
        match self.selection() {
            Some(mut sel) => {
                if let Some(bounds) = self.bounds_at(sel.cursor.0) {
                    sel.extend_to((ordinal, column), bounds);
                    self.set_selection(sel, cx);
                }
            }
            None => self.set_selection(Selection::at((ordinal, column)), cx),
        }
        self.focus_view(window, cx);
    }

    /// A plain click that does not open an editor: the cursor goes to the
    /// cell and the range goes (observed on a name cell's padding).
    pub(super) fn place_cursor(
        &mut self,
        line: usize,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(cx);
        self.collapse_selection();
        self.table_state.focused = Some((line, column));
        self.focus_view(window, cx);
        cx.notify();
    }

    /// Ctrl+A: every cell of the cursor's group.
    pub(super) fn select_all_cells(&mut self, cx: &mut Context<Self>) {
        let Some(mut sel) = self.selection() else {
            return;
        };
        let Some(bounds) = self.bounds_at(sel.cursor.0) else {
            return;
        };
        sel.select_all(bounds);
        self.set_selection(sel, cx);
    }

    /// Whether column `column` takes a write: a note property's.
    fn column_editable(&self, column: usize) -> bool {
        self.table()
            .and_then(|t| t.columns.get(column))
            .and_then(editable_key)
            .is_some()
    }

    /// The row an ordinal stands for: its note's path.
    fn path_at(&self, ordinal: usize) -> Option<String> {
        let line = *self.row_line_indexes().get(ordinal)?;
        let table = self.table()?;
        row_of(table, *self.table_state.lines.get(line)?).map(|r| r.file.path.clone())
    }

    /// Ctrl+C: the range, or the cursor's cell, as tab-separated text.
    pub(super) fn copy_cells(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = self.selection() else {
            return;
        };
        let range = sel.range();
        let rows = self.row_line_indexes();
        let Some(table) = self.table() else {
            return;
        };
        let mut texts = Vec::new();
        let mut cells = Vec::new();
        for ord in range.top..=range.bottom {
            let Some(row) = rows
                .get(ord)
                .and_then(|l| self.table_state.lines.get(*l))
                .and_then(|line| row_of(table, *line))
            else {
                continue;
            };
            let mut text_row = Vec::new();
            let mut cell_row = Vec::new();
            for column in range.start..=range.end {
                let Some(cell) = row.cells.get(column) else {
                    continue;
                };
                text_row.push(copy_text(&cell.value));
                let editable = table.columns.get(column).and_then(editable_key).is_some();
                cell_row.push(editable.then(|| copied_value(&cell.value)));
            }
            texts.push(text_row);
            cells.push(cell_row);
        }
        let text = encode_tsv(&texts);
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        cx.set_global(CopiedCells { text, cells });
    }

    /// Ctrl+V: the clipboard over the range, or at the cursor.
    pub(super) fn paste_cells(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let typed = cx
            .try_global::<CopiedCells>()
            .filter(|c| c.text == text)
            .map(|c| c.cells.clone());
        let Some(sel) = self.selection() else {
            return;
        };
        let cells = typed.unwrap_or_else(|| text_cells(parse_text(&text, !sel.has_range())));
        self.paste_into(sel, &cells, cx);
    }

    /// The paste itself, apart from the clipboard so the socket and the
    /// tests can drive it.
    pub(super) fn paste_into(
        &mut self,
        mut sel: Selection,
        cells: &[Vec<CopiedCell>],
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.bounds_at(sel.cursor.0) else {
            return;
        };
        let editable: Vec<bool> = (0..bounds.columns)
            .map(|c| self.column_editable(c))
            .collect();
        let (writes, covered) = paste_plan(sel.range(), cells, bounds, &|c| {
            editable.get(c).copied().unwrap_or(false)
        });
        self.send_writes(&writes, cx);
        sel.cover(covered);
        self.set_selection(sel, cx);
    }

    /// Delete or Backspace: the range, or the cursor's cell, emptied.
    /// Observed on 1.15.1 for a single cell too, where 1.14.5 opened the
    /// cell's editor instead.
    pub(super) fn delete_cells(&mut self, cx: &mut Context<Self>) {
        let Some(sel) = self.selection() else {
            return;
        };
        let editable: Vec<bool> = (0..self.columns())
            .map(|c| self.column_editable(c))
            .collect();
        let writes = delete_plan(sel.range(), &|c| editable.get(c).copied().unwrap_or(false));
        self.send_writes(&writes, cx);
    }

    /// One [`BasesEvent::SetProperties`] for every row the writes touch,
    /// so the action is one step of the table's undo history.
    fn send_writes(
        &mut self,
        writes: &[scoria_bases::selection::CellWrite],
        cx: &mut Context<Self>,
    ) {
        let Some(columns) = self.table().map(|t| t.columns.clone()) else {
            return;
        };
        let mut rows = Vec::new();
        for (ordinal, row) in by_row(writes) {
            let Some(path) = self.path_at(ordinal) else {
                continue;
            };
            let values: Vec<(String, scoria_formats::properties::Value)> = row
                .into_iter()
                .filter_map(|(column, value)| {
                    let key = columns.get(column).and_then(editable_key)?;
                    Some((key.to_string(), value))
                })
                .collect();
            if !values.is_empty() {
                rows.push((path, values));
            }
        }
        if !rows.is_empty() {
            cx.emit(BasesEvent::SetProperties { rows });
        }
    }

    /// The workspace wrote an action's notes: one step of the table's
    /// history, with the selection as the action left it. Only a table
    /// keeps one (observed: the history is the table view's).
    pub fn record_cell_step(&mut self, changes: Vec<NoteChange>) {
        if !matches!(self.layout(), scoria_bases::Layout::Table(_)) {
            return;
        }
        let selection = self.selection();
        self.cell_history.record(Step { changes, selection });
    }

    /// Whether Ctrl+Z and Ctrl+Shift+Z go to the table's history: the
    /// table is showing and nothing is open over it. Observed: with a field
    /// open they are the field's own, and the cursor need not be up (after
    /// Escape twice the container still undid).
    fn history_takes_keys(&self) -> bool {
        self.editing.cell.is_none()
            && self.editing.menu.is_none()
            && self.toolbar.panel == Panel::None
            && self.toolbar.combo.is_none()
            && self.toolbar.editing.is_none()
            && matches!(self.layout(), scoria_bases::Layout::Table(_))
    }

    pub(super) fn undo_cells_action(
        &mut self,
        _: &UndoCells,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.history_takes_keys() {
            cx.propagate();
            return;
        }
        let step = self.cell_history.undo();
        self.revert_step(step, true, cx);
    }

    pub(super) fn redo_cells_action(
        &mut self,
        _: &RedoCells,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.history_takes_keys() {
            cx.propagate();
            return;
        }
        let step = self.cell_history.redo();
        self.revert_step(step, false, cx);
    }

    /// Puts the step's selection back and asks the workspace to write its
    /// notes back (`undo`) or forward again.
    pub(super) fn revert_step(&mut self, step: Option<Step>, undo: bool, cx: &mut Context<Self>) {
        let Some(step) = step else {
            return;
        };
        match step.selection {
            Some(sel) => self.set_selection(sel, cx),
            None => {
                self.table_state.focused = None;
                self.collapse_selection();
                cx.notify();
            }
        }
        cx.emit(BasesEvent::RevertCells {
            changes: step.changes,
            undo,
        });
    }

    /// The history's depth on each side, as the control socket reports it.
    pub(super) fn cell_history_json(&self) -> serde_json::Value {
        let (undo, redo) = self.cell_history.depth();
        serde_json::json!({ "undo": undo, "redo": redo })
    }

    /// The selection as the control socket reports it, in row ordinals.
    pub(super) fn selection_json(&self) -> serde_json::Value {
        match self.selection() {
            Some(sel) => {
                let r = sel.range();
                serde_json::json!({
                    "cursor": [sel.cursor.0, sel.cursor.1],
                    "range": sel.has_range().then(|| serde_json::json!({
                        "top": r.top, "bottom": r.bottom, "start": r.start, "end": r.end,
                    })),
                })
            }
            None => serde_json::Value::Null,
        }
    }
}
