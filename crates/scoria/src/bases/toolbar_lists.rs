//! The sort, group and results panels.
//!
//! Observed (`2026-11-01-bases-ui.json`, step `s06-sort-and-group`, and
//! `s26-kanban-rest` for `groupOrder`):
//!
//! - **Sort**: a section headed *Sort by*, one `base-toolbar-sort-item` per
//!   entry — a `grip-vertical` handle, the property combobox with its type
//!   icon, the direction combobox (*A → Z* / *Z → A*, or *0 → 1* / *1 → 0*
//!   for a number), a `trash-2` — then an actions group with **Add sort**
//!   (`plus`), which appends an empty row with its property list open; the
//!   file is written when a property is picked.
//! - **Group**: headed *Group by*, one row of property, direction (*A → Z*
//!   / *Z → A* / *Manual*) and `trash-2`, with no grip. *Manual* is the state
//!   in which `groupOrder` is present: the kanban's *Hide column* wrote
//!   `groupOrder: [todo, doing]` and the direction combobox then read
//!   *Manual* while the file kept `direction: ASC`. Under it the
//!   `bases-group-list` section lists every group with a grip and a checkbox,
//!   then **Add group** and **Hide all**.
//! - **Results**: *Limit number of results* over a number input with the
//!   placeholder `e.g. 10`, then **Show all** (`rotate-ccw`, faint until a
//!   limit is set), **Copy to clipboard** (`copy`) and **Export CSV...**
//!   (`file-down`). The limit is written as a bare number.
//!
//! **Hide all** in the group list is drawn faint and inert: an empty
//! `groupOrder` is not representable in the model (the writer drops an empty
//! list), and what Obsidian writes for it was never observed.

use crate::bases::cells::table_tsv;
use crate::bases::toolbar::{ComboOption, ComboSpec, Field, Prop, INPUT_HEIGHT, ROW_HEIGHT};
use crate::bases::view::{BasesEvent, BasesView};
use crate::bases::writes::{
    parse_limit, remove_sort, same_property, set_group_direction, set_group_property,
    set_sort_direction, set_sort_property, ASC, DESC,
};
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, ClipboardItem, Context, SharedString, Window};
use scoria_bases::layout;
use scoria_bases::rule::PropertyKind;
use scoria_formats::base::{Base, View};
use serde_json::json;

/// The direction labels: Obsidian's *A → Z* / *Z → A*, and *0 → 1* / *1 →
/// 0* on a number.
pub(super) fn direction_options(kind: PropertyKind) -> Vec<ComboOption> {
    let (up, down) = match kind {
        PropertyKind::Number => ("0 → 1", "1 → 0"),
        _ => ("A → Z", "Z → A"),
    };
    vec![ComboOption::plain(up), ComboOption::plain(down)]
}

/// Which of the two directions a direction string is; an absent one is
/// ascending.
pub(super) fn direction_ix(direction: Option<&str>) -> usize {
    match direction {
        Some(d) if d.eq_ignore_ascii_case("desc") => 1,
        _ => 0,
    }
}

impl BasesView {
    // ----- sort ---------------------------------------------------------------

    pub(super) fn render_sort_panel(
        &mut self,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let props = self.properties_of(base, view);
        let options = self.property_options(&props);
        self.toolbar.set_rows(
            view.sort
                .iter()
                .map(|s| json!({ "property": s.property, "direction": s.direction }))
                .collect(),
        );
        let mut rows: Vec<AnyElement> = Vec::new();
        for (ix, entry) in view.sort.iter().enumerate() {
            let selected = props
                .iter()
                .position(|p| same_property(&p.id, &entry.property));
            let prop = selected.and_then(|i| props.get(i));
            let kind = prop.map_or(PropertyKind::Unknown, |p| p.kind);
            let shown = prop
                .map(|p| SharedString::from(p.label.clone()))
                .unwrap_or_else(|| SharedString::from(entry.property.clone()));
            let down = direction_ix(entry.direction.as_deref());
            let directions = direction_options(kind);
            let row = div()
                .id(("bases-sort-row", ix))
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(INPUT_HEIGHT))
                .gap(px(4.))
                .px(px(4.))
                .my(px(3.))
                .child(self.grip("sort", ix, theme, cx))
                .child(div().flex_1().min_w(px(0.)).child(self.combobox(
                    ComboSpec {
                        icon: prop.map(Prop::icon),
                        shown: Some(shown),
                        placeholder: "Property",
                        options: options.clone(),
                        selected,
                        ..ComboSpec::new("sortProperty", ix)
                    },
                    theme,
                    cx,
                )))
                .child(div().w(px(70.)).child(self.combobox(
                    ComboSpec {
                        shown: Some(directions[down].label.clone()),
                        options: directions,
                        selected: Some(down),
                        searchable: false,
                        ..ComboSpec::new("sortDirection", ix)
                    },
                    theme,
                    cx,
                )))
                .child(self.clickable_icon(
                    "bases-sort-trash",
                    ix,
                    "trash-2",
                    theme.tokens.text_muted,
                    theme,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.edit_view(move |view| remove_sort(view, ix), cx);
                    }),
                ));
            rows.push(self.drop_target("sort", ix, row, cx));
        }
        if self.toolbar.pending_sort {
            let ix = view.sort.len();
            let directions = direction_options(PropertyKind::Unknown);
            rows.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(INPUT_HEIGHT))
                    .gap(px(4.))
                    .px(px(4.))
                    .my(px(3.))
                    .child(self.grip("sort", ix, theme, cx))
                    .child(div().flex_1().min_w(px(0.)).child(self.combobox(
                        ComboSpec {
                            placeholder: "Property",
                            options: options.clone(),
                            ..ComboSpec::new("sortNew", 0)
                        },
                        theme,
                        cx,
                    )))
                    .child(div().w(px(70.)).child(self.combobox(
                        ComboSpec {
                            shown: Some(directions[0].label.clone()),
                            options: directions,
                            selected: Some(0),
                            searchable: false,
                            ..ComboSpec::new("sortNewDirection", 0)
                        },
                        theme,
                        cx,
                    )))
                    .child(self.clickable_icon(
                        "bases-sort-trash",
                        ix,
                        "trash-2",
                        theme.tokens.text_muted,
                        theme,
                        cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_combo(window, cx);
                        }),
                    ))
                    .into_any_element(),
            );
        }
        let add = self.action_row(
            "bases-sort-add",
            0,
            "plus",
            SharedString::from("Add sort"),
            true,
            theme,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.toolbar.pending_sort = true;
                this.open_combo(
                    crate::bases::toolbar::Combo {
                        name: "sortNew",
                        ix: 0,
                    },
                    true,
                    window,
                    cx,
                );
            }),
        );
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(2.))
                    .border_b_1()
                    .border_color(theme.tokens.background_modifier_border)
                    .child(self.section_header("Sort by", None, None, theme, cx))
                    .children(rows),
            )
            .child(self.group(vec![add], false, theme))
            .into_any_element()
    }

    // ----- group --------------------------------------------------------------

    pub(super) fn render_group_panel(
        &mut self,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let props = self.properties_of(base, view);
        let options = self.property_options(&props);
        self.toolbar.set_rows(
            view.group_by
                .iter()
                .map(|g| json!({ "property": g.property, "direction": g.direction, "manual": layout::is_manual_group(view) }))
                .collect(),
        );
        let selected = view
            .group_by
            .as_ref()
            .and_then(|g| props.iter().position(|p| same_property(&p.id, &g.property)));
        let prop = selected.and_then(|i| props.get(i));
        let kind = prop.map_or(PropertyKind::Unknown, |p| p.kind);
        let shown = match (&view.group_by, prop) {
            (Some(_), Some(p)) => Some(SharedString::from(p.label.clone())),
            (Some(g), None) => Some(SharedString::from(g.property.clone())),
            (None, _) => None,
        };
        let manual = layout::is_manual_group(view);
        let mut directions = direction_options(kind);
        directions.push(ComboOption::plain("Manual"));
        let dir = if manual {
            2
        } else {
            direction_ix(view.group_by.as_ref().and_then(|g| g.direction.as_deref()))
        };
        let grouped = view.group_by.is_some();
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(INPUT_HEIGHT))
            .gap(px(4.))
            .px(px(4.))
            .my(px(3.))
            .child(div().flex_1().min_w(px(0.)).child(self.combobox(
                ComboSpec {
                    icon: prop.map(Prop::icon),
                    shown,
                    placeholder: "Property",
                    options,
                    selected,
                    ..ComboSpec::new("groupProperty", 0)
                },
                theme,
                cx,
            )))
            .child(div().w(px(74.)).child(self.combobox(
                ComboSpec {
                    shown: Some(directions[dir].label.clone()),
                    options: directions,
                    selected: Some(dir),
                    searchable: false,
                    ..ComboSpec::new("groupDirection", 0)
                },
                theme,
                cx,
            )))
            .child(self.clickable_icon(
                "bases-group-trash",
                0,
                "trash-2",
                if grouped {
                    theme.tokens.text_muted
                } else {
                    theme.tokens.text_faint
                },
                theme,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.edit_view(
                        |view| {
                            view.group_by = None;
                            layout::set_group_order(view, None);
                        },
                        cx,
                    );
                }),
            ));
        let mut panel = div().flex().flex_col().w_full().child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .p(px(2.))
                .child(self.section_header("Group by", None, None, theme, cx))
                .child(row),
        );
        if manual {
            panel = panel.child(self.render_group_list(view, theme, cx));
        }
        panel.into_any_element()
    }

    /// `.bases-group-list`: every group the rows produce, in the manual
    /// order, each with a grip and a checkbox that hides it.
    fn render_group_list(
        &mut self,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let keys = self.manual_group_keys(view);
        let all = self
            .table()
            .map(|t| t.all_groups.clone())
            .unwrap_or_default();
        let hidden: Vec<Option<String>> =
            all.iter().filter(|k| !keys.contains(k)).cloned().collect();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut listed: Vec<(Option<String>, bool)> =
            keys.iter().map(|k| (k.clone(), true)).collect();
        listed.extend(hidden.into_iter().map(|k| (k, false)));
        for (ix, (k, visible)) in listed.iter().enumerate() {
            let label = k.clone().unwrap_or_else(|| "None".to_string());
            let key_for = k.clone();
            self.toolbar
                .record(json!({ "group": k, "visible": visible }));
            let row = div()
                .id(("bases-group-row", ix))
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
                .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_group_visible(key_for.clone(), cx);
                }))
                .when(*visible, |d| d.child(self.grip("groups", ix, theme, cx)))
                .when(!*visible, |d| d.child(div().size(px(16.))))
                .child(self.check_box(*visible, theme))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(if *visible {
                            theme.tokens.text_normal
                        } else {
                            theme.tokens.text_faint
                        })
                        .child(SharedString::from(label)),
                );
            rows.push(if *visible {
                self.drop_target("groups", ix, row, cx)
            } else {
                row.into_any_element()
            });
        }
        let mut actions: Vec<AnyElement> = Vec::new();
        if self.toolbar.editing == Some(Field::GroupAdd) {
            let text = self.field_text(cx);
            actions.push(
                div()
                    .px(px(4.))
                    .pb(px(4.))
                    .child(self.text_box(Field::GroupAdd, &text, "", theme, cx))
                    .into_any_element(),
            );
        }
        actions.push(self.action_row(
            "bases-group-add",
            0,
            "plus",
            SharedString::from("Add group"),
            true,
            theme,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_field(Field::GroupAdd, String::new(), window, cx);
                cx.notify();
            }),
        ));
        actions.push(self.action_row(
            "bases-group-hide-all",
            0,
            "eye-off",
            SharedString::from("Hide all"),
            false,
            theme,
            |_, _, _| {},
        ));
        div()
            .flex()
            .flex_col()
            .w_full()
            .border_t_1()
            .border_color(theme.tokens.background_modifier_border)
            .child(self.group(rows, false, theme))
            .child(self.group(actions, true, theme))
            .into_any_element()
    }

    /// The visible groups in their manual order: `groupOrder` as the file
    /// carries it, with a null for the no-value group.
    fn manual_group_keys(&self, view: &View) -> Vec<Option<String>> {
        view.group_order
            .iter()
            .map(|v| match v {
                serde_yaml::Value::Null => None,
                serde_yaml::Value::String(s) => Some(s.clone()),
                other => Some(
                    serde_yaml::to_string(other)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                ),
            })
            .collect()
    }

    /// A checkbox of the group list: a hidden group goes back to the end of
    /// the order, a shown one leaves it.
    fn toggle_group_visible(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        let Some(base) = self.base() else {
            return;
        };
        let view = self.showing_view(base);
        let mut keys = self.manual_group_keys(&view);
        if let Some(pos) = keys.iter().position(|k| *k == key) {
            keys.remove(pos);
        } else {
            keys.push(key);
        }
        self.edit_view(move |view| layout::set_group_order(view, Some(keys)), cx);
    }

    /// A grip of the group list dropped: the manual order follows.
    pub(super) fn reorder_group(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(base) = self.base() else {
            return;
        };
        let view = self.showing_view(base);
        let mut keys = self.manual_group_keys(&view);
        if from < keys.len() && to < keys.len() {
            let k = keys.remove(from);
            keys.insert(to, k);
            self.edit_view(move |view| layout::set_group_order(view, Some(keys)), cx);
        }
    }

    /// Enter in the **Add group** field: a new group at the end of the
    /// manual order.
    pub(super) fn commit_group_add(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        self.toolbar.editing = None;
        if text.is_empty() {
            cx.notify();
            return;
        }
        let Some(base) = self.base() else {
            return;
        };
        let view = self.showing_view(base);
        let mut keys = self.manual_group_keys(&view);
        if !keys.iter().any(|k| k.as_deref() == Some(text.as_str())) {
            keys.push(Some(text));
        }
        self.edit_view(move |view| layout::set_group_order(view, Some(keys)), cx);
    }

    // ----- results ------------------------------------------------------------

    pub(super) fn render_results_panel(
        &mut self,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let limit = view.limit;
        self.toolbar.set_rows(vec![
            json!({ "control": "Limit number of results", "value": limit }),
            json!({ "row": "Show all", "enabled": limit.is_some() }),
            json!({ "row": "Copy to clipboard" }),
            json!({ "row": "Export CSV..." }),
        ]);
        let text = match &self.toolbar.editing {
            Some(Field::Limit) => self.field_text(cx),
            _ => limit.map(|n| n.to_string()).unwrap_or_default(),
        };
        let field = self.text_box(Field::Limit, &text, "e.g. 10", theme, cx);
        let actions = vec![
            self.action_row(
                "bases-results-show-all",
                0,
                "rotate-ccw",
                SharedString::from("Show all"),
                limit.is_some(),
                theme,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    if this.view().and_then(|v| v.limit).is_none() {
                        return;
                    }
                    this.edit_view(|view| layout::set_limit(view, None), cx);
                    if this.toolbar.editing == Some(Field::Limit) {
                        this.toolbar.editing = None;
                    }
                    cx.notify();
                }),
            ),
            self.action_row(
                "bases-results-copy",
                0,
                "copy",
                SharedString::from("Copy to clipboard"),
                true,
                theme,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.copy_table(window, cx);
                }),
            ),
            self.action_row(
                "bases-results-export",
                0,
                "file-down",
                SharedString::from("Export CSV..."),
                true,
                theme,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.export_csv(window, cx);
                }),
            ),
        ];
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.page_form(vec![self.input_row(
                Some("Limit number of results"),
                field,
                theme,
            )]))
            .child(self.group(actions, true, theme))
            .into_any_element()
    }

    /// The limit field: `limit` as a bare number, or no key for an empty or
    /// unreadable field.
    pub(super) fn commit_limit(&mut self, text: &str, cx: &mut Context<Self>) {
        let limit = parse_limit(text);
        if self.view().and_then(|v| v.limit) == limit {
            return;
        }
        self.edit_view(move |view| layout::set_limit(view, limit), cx);
    }

    /// The visible columns as TSV, on the clipboard, with Obsidian's own
    /// notice — the same bytes `bases:copy-table` produces.
    fn copy_table(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(table) = self.table() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(table_tsv(table)));
        self.report("Copied to your clipboard", cx);
        self.close_panel(window, cx);
    }

    /// **Export CSV...**: the workspace asks where to put it.
    fn export_csv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(table) = self.table() else {
            return;
        };
        let csv = scoria_bases::table_csv(table);
        let stem = self
            .path()
            .and_then(|p| p.rsplit('/').next())
            .and_then(|n| n.strip_suffix(".base"))
            .unwrap_or("base")
            .to_string();
        let name = match self.view() {
            Some(view) => format!("{stem} - {}.csv", view.name),
            None => format!("{stem}.csv"),
        };
        cx.emit(BasesEvent::ExportCsv { name, csv });
        self.close_panel(window, cx);
    }

    /// A pick in one of these panels' comboboxes; answers whether the name
    /// was one of theirs.
    pub(super) fn lists_pick(
        &mut self,
        name: &str,
        ix: usize,
        opt: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        match name {
            // Observed: **Add sort** appends; it is the header menu's *Sort
            // A → Z* that prepends.
            "sortNew" => {
                self.toolbar.pending_sort = false;
                if let Some(id) = self.property_id(opt) {
                    self.edit_view(
                        move |view| {
                            view.sort.push(scoria_formats::base::Sort {
                                property: layout::qualify(&id),
                                direction: Some(ASC.to_string()),
                            })
                        },
                        cx,
                    );
                }
            }
            "sortNewDirection" => {}
            "sortProperty" => {
                if let Some(id) = self.property_id(opt) {
                    self.edit_view(
                        move |view| set_sort_property(view, ix, &layout::qualify(&id)),
                        cx,
                    );
                }
            }
            "sortDirection" => {
                let dir = if opt == 1 { DESC } else { ASC };
                self.edit_view(move |view| set_sort_direction(view, ix, dir), cx);
            }
            "groupProperty" => {
                if let Some(id) = self.property_id(opt) {
                    self.edit_view(
                        move |view| set_group_property(view, &layout::qualify(&id)),
                        cx,
                    );
                }
            }
            "groupDirection" => {
                let all = self
                    .table()
                    .map(|t| t.all_groups.clone())
                    .unwrap_or_default();
                match opt {
                    // Manual: the order as it stands becomes the file's.
                    2 => self.edit_view(move |view| layout::set_group_order(view, Some(all)), cx),
                    _ => {
                        let dir = if opt == 1 { DESC } else { ASC };
                        self.edit_view(
                            move |view| {
                                layout::set_group_order(view, None);
                                set_group_direction(view, dir);
                            },
                            cx,
                        );
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// The options of these panels' comboboxes, by name.
    pub(super) fn lists_options(
        &self,
        name: &str,
        ix: usize,
        base: &Base,
        view: &View,
    ) -> Option<Vec<ComboOption>> {
        let props = self.properties_of(base, view);
        Some(match name {
            "sortNew" | "sortProperty" | "groupProperty" => self.property_options(&props),
            "sortNewDirection" => direction_options(PropertyKind::Unknown),
            "sortDirection" => {
                let kind = view
                    .sort
                    .get(ix)
                    .map(|s| self.kind_of(base, view, &s.property))
                    .unwrap_or(PropertyKind::Unknown);
                direction_options(kind)
            }
            "groupDirection" => {
                let kind = view
                    .group_by
                    .as_ref()
                    .map(|g| self.kind_of(base, view, &g.property))
                    .unwrap_or(PropertyKind::Unknown);
                let mut d = direction_options(kind);
                d.push(ComboOption::plain("Manual"));
                d
            }
            _ => return None,
        })
    }
}
