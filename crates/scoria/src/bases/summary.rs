//! Column summaries in the table layout: the footer row, the row under each
//! group heading, and the **Summarize** page that picks what a column sums.
//!
//! **Observed** (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`,
//! `s08` `summaryRow`, `s09` `tableDom`, `s18` `tableEnd`/`footer`/
//! `footerMenu`/`summaryMenus`, `s24` `formulaSummaryMenu`):
//!
//! - a view with any `summaries` draws a **footer** row
//!   (`.bases-tr.bases-table-footer`, 30 px) under its rows, and a grouped
//!   view draws a `.bases-table-group-summary-row` right **under each group
//!   heading**, before the group's rows, and no footer. A view with no
//!   summaries draws neither (the `Minimal.base` embed had no footer class
//!   at all);
//! - a summary cell with nothing set shows the placeholder *Summary* in
//!   `#666`, with a `chevrons-up-down` expand button 30 px in (14 px wide)
//!   and the text at 44; one with a summary set shows its name and the
//!   value (`Average` `3.71`);
//! - a click on any summary cell, and *Summarize...* on the column header,
//!   open the same page: a `bases-toolbar-menu` under the cell (at y + 4)
//!   with a *Find or create...* search, the options with their computed
//!   values and the current one `mod-active`, then **Add summary**
//!   (`square-function`) and, while one is set, **Hide summary**
//!   (`trash-2`). What the options are and compute is
//!   [`scoria_bases::summary`]'s.
//!
//! **Unprobed**: what *Add summary* creates — it is drawn and inert — and
//! whether the page's search field takes focus on open; it does here, like
//! the properties panel's.

use crate::assets::icon;
use crate::bases::cells::CELL_PAD;
use crate::bases::table::{HEADER_ROW_KEY, MIN_COLUMN_WIDTH};
use crate::bases::toolbar::{Field, Page, Panel, ROW_HEIGHT};
use crate::bases::view::BasesView;
use crate::bases::writes::same_property;
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER, ICON_SIZE};
use gpui::{div, prelude::*, px, AnyElement, Context, Pixels, Point, SharedString, Window};
use scoria_bases::summary::{class_of, compute, icon_of, options, SummaryOption, ValueClass};
use scoria_bases::Value;
use scoria_formats::base::{Base, View};
use serde_json::json;

/// The height of a summary row: the footer measured 30 px, as did the row
/// under a group heading.
pub const SUMMARY_HEIGHT: f32 = 30.;

/// The anchor key a summary row records its box under: the footer, or the
/// row of group `g` at `SUMMARY_ROW_KEY + 1 + g`.
pub const SUMMARY_ROW_KEY: u64 = 0x5355_4d4d_4152_5900;

/// Which column, and which group's rows, the open Summarize page is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryTarget {
    pub column: usize,
    /// `None` for the footer, which sums every row.
    pub group: Option<usize>,
}

/// The ids the view's `summaries` names, so the table knows whether to draw
/// the rows at all.
pub fn has_summaries(view: &View) -> bool {
    !view.summaries.is_empty()
}

/// The summary the view sets for a column, by the column's id.
pub fn summary_of<'a>(view: &'a View, id: &str) -> Option<&'a str> {
    view.summaries
        .iter()
        .find(|(k, _)| same_property(k, id))
        .map(|(_, v)| v.as_str())
}

impl BasesView {
    // ----- the values ---------------------------------------------------------

    /// The values of column `column` over the footer's rows or a group's.
    fn summary_values(&self, column: usize, group: Option<usize>) -> Vec<Value> {
        let Some(table) = self.table() else {
            return Vec::new();
        };
        let rows = match group {
            Some(g) => table.groups.get(g).map(|g| &g.rows[..]).unwrap_or(&[]),
            None => &table.rows[..],
        };
        rows.iter()
            .filter_map(|r| r.cells.get(column).map(|c| c.value.clone()))
            .collect()
    }

    /// The text a summary cell draws for `name` over `values`.
    fn summary_text(&self, name: &str, values: &[Value]) -> String {
        let custom = self
            .base()
            .and_then(|b| b.summaries.as_ref())
            .and_then(|m| m.get(name))
            .cloned();
        compute(name, custom.as_deref(), values)
    }

    /// The page's options for the target column, with their values.
    fn summary_rows(&self, target: SummaryTarget) -> (ValueClass, Vec<(SummaryOption, String)>) {
        let values = self.summary_values(target.column, target.group);
        let class = class_of(&values);
        let custom: Vec<String> = self
            .base()
            .and_then(|b| b.summaries.as_ref())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let rows = options(class, &custom)
            .into_iter()
            .map(|o| {
                let text = self.summary_text(&o.name, &values);
                (o, text)
            })
            .collect();
        (class, rows)
    }

    /// The rows the page shows after its search field narrows them.
    fn summary_shown(
        &self,
        target: SummaryTarget,
        cx: &Context<Self>,
    ) -> (ValueClass, Vec<(SummaryOption, String)>) {
        let (class, rows) = self.summary_rows(target);
        let query = match self.toolbar.editing {
            Some(Field::Search) => self.field_text(cx).trim().to_ascii_lowercase(),
            _ => String::new(),
        };
        let shown = rows
            .into_iter()
            .filter(|(o, _)| query.is_empty() || o.name.to_ascii_lowercase().contains(&query))
            .collect();
        (class, shown)
    }

    // ----- opening and writing ----------------------------------------------

    /// Opens the Summarize page for a column over the footer's rows or a
    /// group's, hung at `at` -- under the header for the menu's row, under
    /// the summary row for a click on its cell.
    pub(super) fn open_summary_page(
        &mut self,
        column: usize,
        group: Option<usize>,
        at: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(cx);
        self.editing.menu = None;
        self.toolbar.panel = Panel::Summary;
        self.toolbar.page = Page::List;
        self.toolbar.pinned_at = at;
        self.toolbar.summary = Some(SummaryTarget { column, group });
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.header_menu = false;
        self.toolbar.rows.clear();
        self.request_field(Field::Search, String::new());
        cx.notify();
    }

    /// Where a page opened from a summary row hangs: under the row, at the
    /// column's left edge (observed: a cell at y 178..208 opened its page
    /// at 212).
    fn summary_row_anchor(&self, column: usize, group: Option<usize>) -> Option<Point<Pixels>> {
        let row_key = SUMMARY_ROW_KEY + 1 + group.map_or(0, |g| g as u64 + 1);
        let row = self
            .anchor(row_key)
            .or_else(|| self.anchor(HEADER_ROW_KEY))?;
        let before: f32 = self.table_state.widths.iter().take(column).sum();
        Some(Point::new(
            row.origin.x + px(before),
            row.origin.y + row.size.height + px(4.),
        ))
    }

    /// The header menu's *Summarize...*: the page under the header.
    pub(super) fn summarize_from_header(&mut self, column: usize, cx: &mut Context<Self>) {
        let at = self.header_page_anchor(column);
        self.open_summary_page(column, None, at, cx);
    }

    /// Picks a summary for the target column: `summaries.<id>: <name>`,
    /// the id as `order` spells it.
    fn pick_summary(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.toolbar.summary else {
            return;
        };
        let Some(id) = self
            .table()
            .and_then(|t| t.columns.get(target.column))
            .map(|c| c.id.clone())
        else {
            return;
        };
        self.edit_view(
            move |view| {
                view.summaries.retain(|k, _| !same_property(k, &id));
                view.summaries.insert(id, name);
            },
            cx,
        );
        self.close_panel(window, cx);
    }

    /// *Hide summary*: the key leaves `summaries`.
    fn hide_summary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.toolbar.summary else {
            return;
        };
        let Some(id) = self
            .table()
            .and_then(|t| t.columns.get(target.column))
            .map(|c| c.id.clone())
        else {
            return;
        };
        self.edit_view(
            move |view| view.summaries.retain(|k, _| !same_property(k, &id)),
            cx,
        );
        self.close_panel(window, cx);
    }

    /// How many rows the page's list has: the options, *Add summary*, and
    /// *Hide summary* while one is set.
    pub(super) fn summary_list_len(&self, cx: &Context<Self>) -> usize {
        let Some(target) = self.toolbar.summary else {
            return 0;
        };
        let (_, shown) = self.summary_shown(target, cx);
        shown.len() + 1 + usize::from(self.summary_current(target).is_some())
    }

    /// The summary the target column has now.
    fn summary_current(&self, target: SummaryTarget) -> Option<String> {
        let view = self.view()?;
        let id = &self.table()?.columns.get(target.column)?.id;
        summary_of(view, id).map(str::to_string)
    }

    /// Enter on the page: the selected row, or the first shown one.
    pub(super) fn summary_activate_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.toolbar.summary else {
            return;
        };
        let (_, shown) = self.summary_shown(target, cx);
        let ix = self.toolbar.selected.unwrap_or(0);
        if let Some((option, _)) = shown.get(ix) {
            let name = option.name.clone();
            self.pick_summary(name, window, cx);
        } else if ix == shown.len() + 1 && self.summary_current(target).is_some() {
            self.hide_summary(window, cx);
        }
    }

    // ----- drawing --------------------------------------------------------------

    /// The Summarize page: search, the options with their values, the
    /// actions.
    pub(super) fn render_summary_panel(
        &mut self,
        _base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(target) = self.toolbar.summary else {
            return div().into_any_element();
        };
        let (class, shown) = self.summary_shown(target, cx);
        let current = self
            .table()
            .and_then(|t| t.columns.get(target.column))
            .and_then(|c| summary_of(view, &c.id).map(str::to_string));
        self.toolbar.set_rows(
            shown
                .iter()
                .map(|(o, v)| json!({ "name": o.name, "value": v, "custom": o.custom, "active": current.as_deref() == Some(o.name.as_str()) }))
                .collect(),
        );
        let selected = self.toolbar.selected;
        let rows: Vec<AnyElement> = shown
            .iter()
            .enumerate()
            .map(|(row, (option, value))| {
                let name = option.name.clone();
                let active = current.as_deref() == Some(option.name.as_str());
                div()
                    .id(("bases-summary-option", row))
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .pl(px(6.))
                    .pr(px(4.))
                    .py(px(4.))
                    .mb(px(1.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(theme.tokens.text_normal)
                    .when(selected == Some(row) || active, |d| {
                        d.bg(theme.tokens.background_modifier_hover)
                    })
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.pick_summary(name.clone(), window, cx);
                    }))
                    .child(icon(
                        icon_of(&option.name, class, option.custom),
                        ICON_SIZE,
                        theme.tokens.text_muted,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(SharedString::from(option.name.clone())),
                    )
                    .child(
                        // `.bases-summary-menu-item .bases-toolbar-menu-item-value`.
                        div()
                            .flex_shrink_0()
                            .text_size(px(FONT_UI_SMALLER))
                            .text_color(theme.tokens.text_muted)
                            .child(SharedString::from(value.clone())),
                    )
                    .child(icon("chevron-right", ICON_SIZE, theme.tokens.text_faint))
                    .into_any_element()
            })
            .collect();
        let mut actions: Vec<AnyElement> = vec![self.action_row(
            "bases-summary-add",
            0,
            "square-function",
            SharedString::from("Add summary"),
            false,
            theme,
            |_, _, _| {},
        )];
        let hide_row = shown.len() + 1;
        if current.is_some() {
            let on = selected == Some(hide_row);
            let hide = self.action_row(
                "bases-summary-hide",
                0,
                "trash-2",
                SharedString::from("Hide summary"),
                true,
                theme,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.hide_summary(window, cx);
                }),
            );
            actions.push(if on {
                div()
                    .bg(theme.tokens.background_modifier_hover)
                    .rounded(px(4.))
                    .child(hide)
                    .into_any_element()
            } else {
                hide
            });
        }
        div()
            .flex()
            .flex_col()
            .child(self.search_box(Field::Search, "Find or create...", theme, cx))
            .child(self.group(rows, false, theme))
            .child(self.group(actions, true, theme))
            .into_any_element()
    }

    /// A summary row: the footer (`group` is `None`) or the row under group
    /// `group`'s heading, one cell per column at the column's width.
    pub(super) fn render_summary_row(
        &self,
        group: Option<usize>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(view) = self.view().cloned() else {
            return div().into_any_element();
        };
        let columns: Vec<(String, usize)> = self
            .table()
            .map(|t| {
                t.columns
                    .iter()
                    .map(|c| c.id.clone())
                    .enumerate()
                    .map(|(i, id)| (id, i))
                    .collect()
            })
            .unwrap_or_default();
        let last = columns.len().saturating_sub(1);
        let stretch_last = self.table_state.stretch_last;
        let row_key = SUMMARY_ROW_KEY + 1 + group.map_or(0, |g| g as u64 + 1);
        div()
            .id(("bases-summary-row", group.map_or(0, |g| g + 1)))
            .relative()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .w_full()
            .h(px(SUMMARY_HEIGHT))
            // A group's summary row sits on `--bases-table-group-background`,
            // the table's footer on `--bases-table-summary-background`.
            .bg(if group.is_some() {
                theme.tokens.bases_table_group_background
            } else {
                theme.tokens.bases_table_summary_background
            })
            .border_b_1()
            .border_color(theme.tokens.table_border_color)
            .child(self.measure(row_key))
            .children(columns.into_iter().map(|(id, c)| {
                let width = self
                    .table_state
                    .widths
                    .get(c)
                    .copied()
                    .unwrap_or(MIN_COLUMN_WIDTH);
                let set = summary_of(&view, &id).map(str::to_string);
                let text = set
                    .as_deref()
                    .map(|name| self.summary_text(name, &self.summary_values(c, group)));
                // Observed: the expand button sits 30 px in (14 wide) and
                // shows on hover; the content starts at 44 either way.
                let mut cell = div()
                    .id(("bases-summary-cell", group.map_or(0, |g| g + 1) * 4096 + c))
                    .group("bases-summary-cell")
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h_full()
                    .pl(px(44.))
                    .pr(px(CELL_PAD))
                    .gap(px(4.))
                    .overflow_hidden()
                    .text_size(px(FONT_UI_SMALL))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.bases_table_summary_background_hover))
                    // `.bases-table-footer .bases-td` drops the column rules;
                    // a group's summary row keeps them.
                    .when(c > 0 && group.is_some(), |d| {
                        d.border_l_1().border_color(theme.tokens.table_border_color)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let at = this.summary_row_anchor(c, group);
                        this.open_summary_page(c, group, at, cx);
                    }));
                cell = if c == last && stretch_last {
                    cell.flex_1().min_w(px(width))
                } else {
                    cell.flex_shrink_0().w(px(width))
                };
                cell = cell.child(
                    div()
                        .absolute()
                        .left(px(30.))
                        .top_0()
                        .h_full()
                        .w(px(14.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(0.)
                        .group_hover("bases-summary-cell", |s| s.opacity(1.))
                        .child(icon("chevrons-up-down", 14., theme.tokens.text_faint)),
                );
                match (set, text) {
                    // `.summary-function-name` and `.summary-content` are
                    // both muted; the `.placeholder` is hidden until the
                    // pointer is over the cell, then faint.
                    (Some(name), Some(value)) => cell
                        .child(
                            div()
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .text_size(px(FONT_UI_SMALLER))
                                .text_color(theme.tokens.text_muted)
                                .child(SharedString::from(name)),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .text_color(theme.tokens.text_muted)
                                .child(SharedString::from(value)),
                        ),
                    _ => cell.child(
                        div()
                            .opacity(0.)
                            .group_hover("bases-summary-cell", |s| s.opacity(1.))
                            .text_color(theme.tokens.text_faint)
                            .child(SharedString::from("Summary")),
                    ),
                }
            }))
            .into_any_element()
    }
}
