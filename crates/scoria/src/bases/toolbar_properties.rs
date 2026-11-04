//! The properties panel and the two pages a row's gear opens.
//!
//! Observed (`2026-11-01-bases-ui.json`, steps `s04-properties-panel` and
//! `s16-fix-formula`): a `Find or create...` field; one row per property —
//! a 16 px checkbox, the type icon (`info` on every `file.*` property), the
//! name, a `chevron-right` gear — the shown columns first in their order,
//! then the hidden `file.*` properties, then hidden formulas
//! (`square-function`), then hidden note properties, every hidden row in
//! `--text-faint`; then a bordered group with **Add formula**
//! (`square-function`) and **Hide all** (`eye-off`). A name typed that no row
//! carries puts `Add "name" property` (`plus`) first.
//!
//! The gear on a property opens **Edit *name***: a **Display name** input
//! and a **Property type** select (Checkbox, Date, Date & time, List, Number,
//! Text). On a formula it opens **Edit *name***: Display name, then
//! **Formula** with a `help-circle` beside the label, the statement's editor,
//! and a status line — *Valid formula* or *Invalid formula* (`x-circle`).
//!
//! The writes: checking appends to `order`, unchecking removes; **Hide all**
//! writes `order: []`; **Add formula** writes `Untitled: ""` under `formulas`
//! and appends `formula.Untitled` to `order`, then opens its page with the
//! name selected; `Add "x" property` appends `x` to `order` and opens the
//! page with only its **Property type** row (the property is new to the
//! vault, so there is no display name to edit); the display name goes under
//! `properties.<id>.displayName`; a property type goes to `types.json`.

use crate::assets::icon;
use crate::bases::toolbar::{kind_of_type, ComboOption, Field, Page, Prop, ROW_HEIGHT};
use crate::bases::view::{BasesEvent, BasesView};
use crate::bases::writes::{add_property, same_property, toggle_property};
use crate::theme::{Theme, ICON_SIZE};
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use scoria_bases::layout;
use scoria_bases::rule::PropertyKind;
use scoria_formats::base::{Base, View};
use scoria_formats::properties::PropertyType;
use serde_json::json;

/// The types the **Property type** select offers, in its order.
const TYPE_OPTIONS: &[PropertyType] = &[
    PropertyType::Checkbox,
    PropertyType::Date,
    PropertyType::DateTime,
    PropertyType::MultiText,
    PropertyType::Number,
    PropertyType::Text,
];

/// One row of the panel.
#[derive(Debug, Clone)]
struct PropRow {
    prop: Prop,
    shown: bool,
}

impl BasesView {
    /// The ids of the columns the showing view resolves to.
    pub(super) fn shown_columns(&self) -> Vec<String> {
        self.table()
            .map(|t| t.columns.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default()
    }

    /// The rows in the panel's order, narrowed by the search field.
    fn property_rows(&self, base: &Base, view: &View, cx: &Context<Self>) -> Vec<PropRow> {
        let props = self.properties_of(base, view);
        let shown = self.shown_columns();
        let mut rows: Vec<PropRow> = Vec::new();
        for id in &shown {
            let prop = props
                .iter()
                .find(|p| same_property(&p.id, id))
                .cloned()
                .unwrap_or_else(|| Prop {
                    id: layout::qualify(id),
                    label: scoria_bases::table::column(base, id).display,
                    kind: PropertyKind::Unknown,
                });
            rows.push(PropRow { prop, shown: true });
        }
        let hidden: Vec<Prop> = props
            .into_iter()
            .filter(|p| !shown.iter().any(|s| same_property(s, &p.id)))
            .collect();
        let mut files: Vec<Prop> = hidden
            .iter()
            .filter(|p| p.id.starts_with("file."))
            .cloned()
            .collect();
        files.sort_by_key(|p| p.id.to_ascii_lowercase());
        let mut formulas: Vec<Prop> = hidden
            .iter()
            .filter(|p| p.id.starts_with("formula."))
            .cloned()
            .collect();
        formulas.sort_by(|a, b| {
            a.label
                .to_ascii_lowercase()
                .cmp(&b.label.to_ascii_lowercase())
        });
        let mut notes: Vec<Prop> = hidden
            .iter()
            .filter(|p| !p.id.starts_with("file.") && !p.id.starts_with("formula."))
            .cloned()
            .collect();
        notes.sort_by_key(|p| p.id.to_ascii_lowercase());
        for prop in files.into_iter().chain(formulas).chain(notes) {
            rows.push(PropRow { prop, shown: false });
        }
        let query = match &self.toolbar.editing {
            Some(Field::Search) => self.field_text(cx).trim().to_ascii_lowercase(),
            _ => String::new(),
        };
        if query.is_empty() {
            return rows;
        }
        rows.into_iter()
            .filter(|r| {
                r.prop.label.to_ascii_lowercase().contains(&query)
                    || r.prop.id.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// The name typed into the field that no row carries, which the
    /// `Add "name" property` row offers.
    fn new_property_name(&self, rows: &[PropRow], cx: &Context<Self>) -> Option<String> {
        let typed = match &self.toolbar.editing {
            Some(Field::Search) => self.field_text(cx).trim().to_string(),
            _ => String::new(),
        };
        if typed.is_empty()
            || rows
                .iter()
                .any(|r| r.prop.label.eq_ignore_ascii_case(&typed))
        {
            return None;
        }
        Some(typed)
    }

    /// How many rows the arrows cycle over: the properties, then the actions.
    pub(super) fn properties_list_len(&self, cx: &Context<Self>) -> usize {
        let Some(base) = self.base() else {
            return 0;
        };
        let view = self.showing_view(base);
        let rows = self.property_rows(base, &view, cx);
        let add = usize::from(self.new_property_name(&rows, cx).is_some());
        rows.len() + add + 2
    }

    /// Enter in the search field: toggles the selected property, or runs
    /// the selected action.
    pub(super) fn properties_activate_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(base) = self.base().cloned() else {
            return;
        };
        let view = self.showing_view(&base);
        let rows = self.property_rows(&base, &view, cx);
        let new_name = self.new_property_name(&rows, cx);
        let selected = self.toolbar.selected.unwrap_or(0);
        if let Some(row) = rows.get(selected) {
            let id = row.prop.id.clone();
            self.toggle_column(&id, cx);
            return;
        }
        let mut actions: Vec<&str> = Vec::new();
        if new_name.is_some() {
            actions.push("add-property");
        }
        actions.push("add-formula");
        actions.push("hide-all");
        match actions.get(selected - rows.len()).copied() {
            Some("add-property") => {
                if let Some(name) = new_name {
                    self.add_new_property(&name, window, cx);
                }
            }
            Some("add-formula") => self.add_formula_from_panel(cx),
            Some("hide-all") => self.hide_all(cx),
            _ => {}
        }
    }

    /// A checkbox: the column shown or hidden.
    fn toggle_column(&mut self, id: &str, cx: &mut Context<Self>) {
        let shown = self.shown_columns();
        let id = id.to_string();
        self.edit_view(move |view| toggle_property(view, &shown, &id), cx);
    }

    /// **Hide all**: `order: []`, the view with no columns.
    fn hide_all(&mut self, cx: &mut Context<Self>) {
        self.edit_view(|view| view.order = Some(Vec::new()), cx);
    }

    /// **Add formula**: `Untitled: ""` under `formulas`, its column shown,
    /// and its page open with the name selected.
    fn add_formula_from_panel(&mut self, cx: &mut Context<Self>) {
        let shown = self.shown_columns();
        let ix = self.view_ix();
        let mut name = String::new();
        self.edit_base(|base| name = layout::add_formula(base, ix, &shown), cx);
        if name.is_empty() {
            return;
        }
        self.toolbar.page = Page::Formula(name.clone());
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.request_field(Field::DisplayName, name);
        cx.notify();
    }

    /// `Add "name" property`: the column appended, and its page opened on
    /// its type.
    fn add_new_property(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.shown_columns();
        let id = layout::qualify(name);
        let write = id.clone();
        self.edit_view(move |view| add_property(view, &shown, &write), cx);
        self.toolbar.page = Page::Property(id);
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.editing = None;
        let _ = window;
        cx.notify();
    }

    /// The panel: its list, or the page a gear opened.
    pub(super) fn render_properties_panel(
        &mut self,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.toolbar.page.clone() {
            Page::Property(id) => return self.render_property_page(base, &id, theme, cx),
            Page::Formula(name) => return self.render_formula_page(base, &name, theme, cx),
            _ => {}
        }
        let rows = self.property_rows(base, view, cx);
        let new_name = self.new_property_name(&rows, cx);
        self.toolbar.set_rows(
            rows.iter()
                .map(|r| json!({ "id": r.prop.id, "label": r.prop.label, "checked": r.shown }))
                .collect(),
        );
        let selected = self.toolbar.selected;
        let list: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(row, r)| {
                let id = r.prop.id.clone();
                let page_id = r.prop.id.clone();
                let is_formula = r.prop.id.starts_with("formula.");
                let color = if r.shown {
                    theme.tokens.text_normal
                } else {
                    theme.tokens.text_faint
                };
                let icon_color = if r.shown {
                    theme.tokens.text_muted
                } else {
                    theme.tokens.text_faint
                };
                div()
                    .id(("bases-property-row", row))
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
                    .text_color(color)
                    .when(selected == Some(row), |d| {
                        d.bg(theme.tokens.background_modifier_hover)
                    })
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_column(&id, cx);
                    }))
                    .child(self.check_box(r.shown, theme))
                    .child(icon(r.prop.panel_icon(), ICON_SIZE, icon_color))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(SharedString::from(r.prop.label.clone())),
                    )
                    .child(
                        div()
                            .id(("bases-property-gear", row))
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.toolbar.page = if is_formula {
                                    Page::Formula(page_id["formula.".len()..].to_string())
                                } else {
                                    Page::Property(page_id.clone())
                                };
                                this.toolbar.combo = None;
                                this.toolbar.selected = None;
                                this.toolbar.editing = None;
                                cx.notify();
                            }))
                            .child(icon("chevron-right", ICON_SIZE, icon_color)),
                    )
                    .into_any_element()
            })
            .collect();
        let mut actions: Vec<AnyElement> = Vec::new();
        let mut action_ix = rows.len();
        let wash = |el: AnyElement, on: bool| {
            if on {
                div()
                    .bg(theme.tokens.background_modifier_hover)
                    .rounded(px(4.))
                    .child(el)
                    .into_any_element()
            } else {
                el
            }
        };
        if let Some(name) = new_name.clone() {
            let label = SharedString::from(format!("Add \"{name}\" property"));
            let add = self.action_row(
                "bases-property-add",
                0,
                "plus",
                label,
                true,
                theme,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.add_new_property(&name, window, cx);
                }),
            );
            actions.push(wash(add, selected == Some(action_ix)));
            action_ix += 1;
        }
        let add_formula = self.action_row(
            "bases-property-add-formula",
            0,
            "square-function",
            SharedString::from("Add formula"),
            true,
            theme,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.add_formula_from_panel(cx);
            }),
        );
        actions.push(wash(add_formula, selected == Some(action_ix)));
        action_ix += 1;
        let hide_all = self.action_row(
            "bases-property-hide-all",
            0,
            "eye-off",
            SharedString::from("Hide all"),
            true,
            theme,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.hide_all(cx);
            }),
        );
        actions.push(wash(hide_all, selected == Some(action_ix)));
        let mut panel = div().flex().flex_col().w_full().child(self.search_box(
            Field::Search,
            "Find or create...",
            theme,
            cx,
        ));
        if !list.is_empty() {
            panel = panel.child(self.group(list, false, theme));
        }
        panel
            .child(self.group(actions, true, theme))
            .into_any_element()
    }

    /// The display name the base gives a property, if any.
    fn display_name_of(&self, base: &Base, id: &str) -> Option<String> {
        base.properties
            .iter()
            .find(|(k, _)| same_property(k, id))
            .and_then(|(_, c)| c.display_name.clone())
    }

    /// **Edit *name***: a property's display name and type.
    fn render_property_page(
        &mut self,
        base: &Base,
        id: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let qualified = layout::qualify(id);
        let last = qualified
            .rsplit('.')
            .next()
            .unwrap_or(&qualified)
            .to_string();
        let note_name = qualified.strip_prefix("note.").map(str::to_string);
        // A property the vault does not carry yet has nothing to display:
        // observed, the page then holds the type row alone under the
        // generic title.
        let known = match &note_name {
            Some(name) => {
                self.vault()
                    .is_some_and(|s| s.property_types().types.contains_key(name))
                    || self.table().is_some_and(|t| {
                        t.rows.iter().any(|r| r.file.properties.contains_key(name))
                    })
            }
            None => true,
        };
        let display = self.display_name_of(base, &qualified);
        let title = if known {
            format!("Edit {}", display.clone().unwrap_or_else(|| last.clone()))
        } else {
            "Edit property".to_string()
        };
        self.toolbar.set_rows(vec![
            json!({ "page": "property", "id": qualified, "title": title }),
        ]);
        let mut rows: Vec<AnyElement> = Vec::new();
        if known {
            let value = match &self.toolbar.editing {
                Some(Field::DisplayName) => self.field_text(cx),
                _ => display.unwrap_or_default(),
            };
            let placeholder: &'static str = Box::leak(last.clone().into_boxed_str());
            rows.push(self.input_row(
                Some("Display name"),
                self.text_box(Field::DisplayName, &value, placeholder, theme, cx),
                theme,
            ));
        }
        if let Some(name) = note_name {
            let kind = self.note_kind(&name);
            let current = TYPE_OPTIONS.iter().position(|t| kind_of_type(*t) == kind);
            let options: Vec<ComboOption> = TYPE_OPTIONS
                .iter()
                .map(|t| ComboOption::plain(t.label()))
                .collect();
            let shown = current
                .map(|i| options[i].label.clone())
                .unwrap_or_else(|| SharedString::from("Text"));
            rows.push(self.input_row(
                Some("Property type"),
                self.select("propertyType", shown, options, current, theme, cx),
                theme,
            ));
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.page_header(SharedString::from(title), false, theme, cx))
            .child(self.page_form(rows))
            .into_any_element()
    }

    /// **Edit *name***: a formula's display name and statement.
    fn render_formula_page(
        &mut self,
        base: &Base,
        name: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = format!("formula.{name}");
        let display = self.display_name_of(base, &id);
        let title = format!(
            "Edit {}",
            display.clone().unwrap_or_else(|| name.to_string())
        );
        let statement = base.formulas.get(name).cloned().unwrap_or_default();
        let typed = match &self.toolbar.editing {
            Some(Field::Formula) => self.field_text(cx),
            _ => statement.clone(),
        };
        let valid = typed.trim().is_empty() || scoria_bases::parse(&typed).is_ok();
        self.toolbar.set_rows(vec![
            json!({ "page": "formula", "name": name, "title": title, "statement": typed, "valid": valid }),
        ]);
        let display_value = match &self.toolbar.editing {
            Some(Field::DisplayName) => self.field_text(cx),
            _ => display.unwrap_or_default(),
        };
        let placeholder: &'static str = Box::leak(name.to_string().into_boxed_str());
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
            .min_h(px(100.))
            .p(px(6.))
            .rounded(px(5.))
            // `.formula-editor`: a form field.
            .bg(theme.tokens.background_modifier_form_field)
            .border_1()
            .border_color(theme.tokens.background_modifier_border)
            .font_family(theme.mono_font.clone())
            .child(self.text_box(Field::Formula, &typed, "x + y", theme, cx))
            .relative()
            .when(self.toolbar.editing == Some(Field::Formula), |d| {
                d.child(self.measure(super::toolbar::FORMULA_EDITOR_KEY))
            });
        // `--text-success` and `--text-error`: the theme's green and red.
        let (status_icon, status_color, status) = if valid {
            ("circle-check", theme.tokens.text_success, "Valid formula")
        } else {
            ("circle-x", theme.tokens.text_error, "Invalid formula")
        };
        let status_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .text_color(status_color)
            .child(icon(status_icon, 14., status_color))
            .child(SharedString::from(status));
        let rows = vec![
            self.input_row(
                Some("Display name"),
                self.text_box(Field::DisplayName, &display_value, placeholder, theme, cx),
                theme,
            ),
            div()
                .flex()
                .flex_col()
                .w_full()
                .child(label)
                .child(editor)
                .child(div().pt(px(6.)).child(status_row))
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

    /// The **Display name** field, on either page.
    pub(super) fn commit_display_name(&mut self, text: &str, cx: &mut Context<Self>) {
        let id = match self.toolbar.page.clone() {
            Page::Property(id) => layout::qualify(&id),
            Page::Formula(name) => format!("formula.{name}"),
            _ => return,
        };
        let now = self.base().and_then(|b| self.display_name_of(b, &id));
        let text = text.trim();
        let next = (!text.is_empty()).then(|| text.to_string());
        if now == next {
            return;
        }
        self.edit_base(
            move |base| layout::set_display_name(base, &id, next.as_deref()),
            cx,
        );
    }

    /// The formula editor: written whenever it parses, kept in the field
    /// otherwise.
    pub(super) fn commit_formula(&mut self, text: &str, cx: &mut Context<Self>) {
        if matches!(self.toolbar.page, Page::NewSummary { .. }) {
            self.commit_summary_formula(text, cx);
            return;
        }
        let Page::Formula(name) = self.toolbar.page.clone() else {
            return;
        };
        let now = self.base().and_then(|b| b.formulas.get(&name).cloned());
        if now.as_deref() == Some(text) {
            cx.notify();
            return;
        }
        if !text.trim().is_empty() && scoria_bases::parse(text).is_err() {
            cx.notify();
            return;
        }
        let statement = text.to_string();
        self.edit_base(move |base| layout::set_formula(base, &name, &statement), cx);
    }

    /// A pick in this panel's comboboxes; answers whether the name was one
    /// of its own.
    pub(super) fn properties_pick(
        &mut self,
        name: &str,
        opt: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        match name {
            "propertyType" => {
                let Page::Property(id) = self.toolbar.page.clone() else {
                    return true;
                };
                let Some(key) = layout::qualify(&id)
                    .strip_prefix("note.")
                    .map(str::to_string)
                else {
                    return true;
                };
                if let Some(ty) = TYPE_OPTIONS.get(opt).copied() {
                    cx.emit(BasesEvent::SetPropertyType { key, ty });
                }
            }
            _ => return false,
        }
        true
    }

    /// The options of this panel's comboboxes, by name.
    pub(super) fn properties_options(&self, name: &str) -> Option<Vec<ComboOption>> {
        match name {
            "propertyType" => Some(
                TYPE_OPTIONS
                    .iter()
                    .map(|t| ComboOption::plain(t.label()))
                    .collect(),
            ),
            _ => None,
        }
    }
}
