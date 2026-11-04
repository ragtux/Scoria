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
//! *Add summary* (probe of 2026-11-04, `bases-leftovers` `s03`) opens an
//! **Edit new summary** page in the same menu: a *Summary name* field, value
//! and placeholder both `new summary`, focused; a *Formula* editor with the
//! placeholder `values.reduce( ... )` and the validity line. The click writes
//! nothing. Once a name and a formula are typed the base holds a top-level
//! `summaries: {name: formula}` and the column's summary is that name, with
//! no button pressed; Escape goes back to the list, where the new summary is
//! listed among the built-ins by name. [`BasesView::write_new_summary`]
//! writes when the formula parses, which is the one point the run's bytes
//! were read after; whether Obsidian writes a half-typed formula is unseen.
//!
//! **Unprobed**: whether the page's search field takes focus on open; it does
//! here, like the properties panel's.

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

/// What *Add summary* names a new summary until it is renamed: the field's
/// value and its placeholder alike.
pub const NEW_SUMMARY_NAME: &str = "new summary";

/// The Add summary page's formula placeholder, as observed.
const FORMULA_PLACEHOLDER: &str = "values.reduce( ... )";

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

    /// *Add summary*: the Edit new summary page, its name field focused.
    /// Nothing is written yet.
    pub(super) fn add_summary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toolbar.page = Page::NewSummary {
            name: NEW_SUMMARY_NAME.to_string(),
            formula: String::new(),
            written: None,
        };
        self.toolbar.selected = None;
        self.open_field(Field::SummaryName, NEW_SUMMARY_NAME.to_string(), window, cx);
        cx.notify();
    }

    /// The *Summary name* field: kept on the page, and a summary already
    /// written is renamed with it.
    pub(super) fn commit_summary_name(&mut self, text: &str, cx: &mut Context<Self>) {
        let Page::NewSummary { name, .. } = &mut self.toolbar.page else {
            return;
        };
        if name.as_str() == text {
            return;
        }
        *name = text.to_string();
        self.write_new_summary(cx);
    }

    /// The page's *Formula* editor: kept on the page, written once it parses.
    pub(super) fn commit_summary_formula(&mut self, text: &str, cx: &mut Context<Self>) {
        let Page::NewSummary { formula, .. } = &mut self.toolbar.page else {
            return;
        };
        if formula.as_str() == text {
            cx.notify();
            return;
        }
        *formula = text.to_string();
        self.write_new_summary(cx);
    }

    /// Writes the page's summary when it has a name and a formula that
    /// parses: `summaries.<name>: <formula>` at the top of the base (moved
    /// from the name it was written under, if that changed) and the target
    /// column's summary set to it, as the run's file read after typing.
    fn write_new_summary(&mut self, cx: &mut Context<Self>) {
        let Page::NewSummary {
            name,
            formula,
            written,
        } = self.toolbar.page.clone()
        else {
            return;
        };
        let name = name.trim().to_string();
        let ready =
            !name.is_empty() && !formula.trim().is_empty() && scoria_bases::parse(&formula).is_ok();
        let id = self
            .toolbar
            .summary
            .and_then(|t| self.table().and_then(|tb| tb.columns.get(t.column)))
            .map(|c| c.id.clone());
        let (true, Some(id)) = (ready, id) else {
            cx.notify();
            return;
        };
        let ix = self.view_ix();
        let key = name.clone();
        self.edit_base(
            move |base| {
                let summaries = base.summaries.get_or_insert_with(Default::default);
                match written.as_deref() {
                    // A rename keeps the summary's place in the map.
                    Some(old) if old != key => match summaries.get_index_of(old) {
                        Some(at) => {
                            summaries.shift_remove(old);
                            summaries.shift_insert(at, key.clone(), formula);
                        }
                        None => {
                            summaries.insert(key.clone(), formula);
                        }
                    },
                    _ => {
                        summaries.insert(key.clone(), formula);
                    }
                }
                if let Some(view) = base.views.get_mut(ix) {
                    view.summaries.retain(|k, _| !same_property(k, &id));
                    view.summaries.insert(id, key);
                }
            },
            cx,
        );
        if let Page::NewSummary { written, .. } = &mut self.toolbar.page {
            *written = Some(name);
        }
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
        } else if ix == shown.len() {
            self.add_summary(window, cx);
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
        if matches!(self.toolbar.page, Page::NewSummary { .. }) {
            return self.render_new_summary_page(theme, cx);
        }
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
        let add = self.action_row(
            "bases-summary-add",
            0,
            "square-function",
            SharedString::from("Add summary"),
            true,
            theme,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.add_summary(window, cx);
            }),
        );
        let mut actions: Vec<AnyElement> = vec![if selected == Some(shown.len()) {
            div()
                .bg(theme.tokens.background_modifier_hover)
                .rounded(px(4.))
                .child(add)
                .into_any_element()
        } else {
            add
        }];
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

    /// **Edit new summary**: the name, the formula and its validity line.
    fn render_new_summary_page(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Page::NewSummary { name, formula, .. } = self.toolbar.page.clone() else {
            return div().into_any_element();
        };
        let name_value = match &self.toolbar.editing {
            Some(Field::SummaryName) => self.field_text(cx),
            _ => name.clone(),
        };
        let typed = match &self.toolbar.editing {
            Some(Field::Formula) => self.field_text(cx),
            _ => formula,
        };
        let valid = typed.trim().is_empty() || scoria_bases::parse(&typed).is_ok();
        self.toolbar.set_rows(vec![json!({
            "page": "newSummary", "name": name_value, "formula": typed, "valid": valid,
        })]);
        // Observed: the title stays "Edit new summary" while the name is typed.
        let title = "Edit new summary";
        let label = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(ROW_HEIGHT))
            .px(px(2.))
            .py(px(4.))
            .gap(px(4.))
            .child(SharedString::from("Formula"))
            .child(icon("circle-help", 14., theme.tokens.text_faint));
        let editor = div()
            .flex()
            .flex_col()
            .w_full()
            .min_h(px(84.))
            .p(px(6.))
            .rounded(px(5.))
            .bg(theme.tokens.background_modifier_form_field)
            .border_1()
            .border_color(theme.tokens.background_modifier_border)
            .font_family(theme.mono_font.clone())
            .child(self.text_box(Field::Formula, &typed, FORMULA_PLACEHOLDER, theme, cx))
            .relative()
            .when(self.toolbar.editing == Some(Field::Formula), |d| {
                d.child(self.measure(super::toolbar::FORMULA_EDITOR_KEY))
            });
        let (status_icon, status_color, status) = if valid {
            ("circle-check", theme.tokens.text_success, "Valid formula")
        } else {
            ("circle-x", theme.tokens.text_error, "Invalid formula")
        };
        let rows = vec![
            self.input_row(
                Some("Summary name"),
                self.text_box(Field::SummaryName, &name_value, NEW_SUMMARY_NAME, theme, cx),
                theme,
            ),
            div()
                .flex()
                .flex_col()
                .w_full()
                .child(label)
                .child(editor)
                .child(
                    div()
                        .pt(px(6.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .text_color(status_color)
                        .child(icon(status_icon, 14., status_color))
                        .child(SharedString::from(status)),
                )
                .into_any_element(),
        ];
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.page_header(SharedString::from(title), false, theme, cx))
            .child(self.page_form(rows))
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
