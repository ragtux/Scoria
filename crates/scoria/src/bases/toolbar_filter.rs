//! The filter panel: the base's own `filters` under **All views** and the
//! showing view's under **This view**, as rows of property, operator and
//! value.
//!
//! Observed (`2026-11-01-bases-ui.json`, steps `s05-filters` and
//! `s17-fix-filters`): a 520 px panel of two collapsible sections, each
//! headed with a fold triangle, its name and a count flair, *All views*
//! starting folded. A section holds a `.filter-group`: a header select —
//! **All the following are true** (`and`), **Any of the following are true**
//! (`or`), **None of the following are true** (`not`) — then one row per
//! statement: the word *where* on the first and the group's conjunction on
//! the rest, an expression box holding the property combobox (with its type
//! icon), the operator combobox and the value — a text, a number, a
//! checkbox, a date, or pills for a list — then **Advanced filter**
//! (`code-xml`, which swaps the row to its statement and itself to
//! **Simple filter**, `mouse-pointer-click`) and **Remove filter**
//! (`trash-2`). A nested group is a row of its own with the group's select
//! and a `trash-2` in its header. Under a group: **Add filter** and **Add
//! filter group** (`plus`).
//!
//! The operator lists per property kind and the statement each row writes
//! are [`scoria_bases::rule`]'s; a statement that is not one of those
//! shapes is drawn in advanced mode, verbatim, and written back verbatim.
//!
//! An absent `filters` and a bare top-level statement are both shown as an
//! `and` group, so every section has one group at its root; the first edit
//! of a bare statement therefore writes it as `and: [..]`. What Obsidian
//! writes for a single rule is unobserved.

use crate::assets::icon;
use crate::bases::toolbar::{
    hash_key, kind_icon, ComboOption, ComboSpec, Field, Section, FILTER_PANEL_WIDTH, INPUT_HEIGHT,
};
use crate::bases::view::BasesView;
use crate::bases::writes::same_property;
use crate::theme::{Theme, FONT_UI_SMALL, ICON_SIZE};
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use scoria_bases::layout;
use scoria_bases::rule::{
    new_rule, operators_for, parse_rule, Operator, PropertyKind, Rule, RuleValue, ValueKind,
};
use scoria_formats::base::{Base, Filter, View};
use serde_json::json;

/// The conjunction select's rows, in Obsidian's order and wording.
pub const CONJUNCTIONS: &[(&str, &str)] = &[
    ("and", "All the following are true"),
    ("or", "Any of the following are true"),
    ("not", "None of the following are true"),
];

/// A stable index for a node, which the comboboxes and fields of its row
/// are keyed by.
pub(super) fn node_ix(section: Section, path: &[usize]) -> usize {
    let mut h = hash_key(section.as_str(), 7);
    for p in path {
        h = hash_key("p", h.wrapping_mul(31).wrapping_add(*p + 1));
    }
    h
}

impl BasesView {
    /// The filter tree of a section, as the panel edits it.
    fn section_tree(&self, section: Section) -> Filter {
        let Some(base) = self.base() else {
            return Filter::And(Vec::new());
        };
        match section {
            Section::AllViews => group_tree(base.filters.as_ref()),
            Section::ThisView => group_tree(self.showing_view(base).filters.as_ref()),
        }
    }

    /// Routes a filter write to the section it belongs to.
    fn edit_filter<F>(&mut self, section: Section, edit: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&mut Filter),
    {
        match section {
            Section::AllViews => self.edit_base(
                move |base| {
                    let mut tree = group_tree(base.filters.as_ref());
                    edit(&mut tree);
                    base.filters = written(tree);
                },
                cx,
            ),
            Section::ThisView => self.edit_view(
                move |view| {
                    let mut tree = group_tree(view.filters.as_ref());
                    edit(&mut tree);
                    view.filters = written(tree);
                },
                cx,
            ),
        }
    }

    /// The property rows a rule's combobox offers: `file`, then every
    /// formula and note property — with their ids beside, so a pick can be
    /// written.
    fn rule_property_options(&self, base: &Base, view: &View) -> (Vec<String>, Vec<ComboOption>) {
        self.image_options(base, view)
    }

    /// The kind of the property a statement names.
    fn kind_lookup(&self, base: &Base, view: &View) -> impl Fn(&str) -> PropertyKind + '_ {
        let props = self.properties_of(base, view);
        move |id: &str| {
            let qualified = layout::qualify(id);
            if qualified == "file" {
                return PropertyKind::File;
            }
            props
                .iter()
                .find(|p| same_property(&p.id, &qualified))
                .map(|p| p.kind)
                .unwrap_or(PropertyKind::Unknown)
        }
    }

    pub(super) fn render_filter_panel(
        &mut self,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.toolbar.set_rows(Vec::new());
        let mut panel = div().flex().flex_col().w(px(FILTER_PANEL_WIDTH));
        for section in [Section::AllViews, Section::ThisView] {
            let tree = self.section_tree(section);
            let count = count_rules(&tree);
            let collapsed = self.toolbar.collapsed[match section {
                Section::AllViews => 0,
                Section::ThisView => 1,
            }];
            let mut block = div()
                .flex()
                .flex_col()
                .w_full()
                .p(px(2.))
                .border_b_1()
                .border_color(theme.border)
                .child(self.section_header(
                    section.heading(),
                    Some(count),
                    Some((collapsed, section)),
                    theme,
                    cx,
                ));
            if !collapsed {
                block = block.child(div().p(px(8.)).child(self.render_group(
                    section,
                    &tree,
                    Vec::new(),
                    base,
                    view,
                    theme,
                    cx,
                )));
            }
            panel = panel.child(block);
        }
        panel.into_any_element()
    }

    /// `.filter-group`: its conjunction select, its rows, and the two add
    /// buttons under them.
    #[allow(clippy::too_many_arguments)]
    fn render_group(
        &mut self,
        section: Section,
        group: &Filter,
        path: Vec<usize>,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let conj = conjunction_of(group);
        let conj_ix = CONJUNCTIONS
            .iter()
            .position(|(c, _)| *c == conj)
            .unwrap_or(0);
        let ix = node_ix(section, &path);
        self.toolbar.record(json!({
            "section": section.as_str(),
            "path": path,
            "conjunction": conj,
        }));
        let options: Vec<ComboOption> = CONJUNCTIONS
            .iter()
            .map(|(_, label)| ComboOption::plain(*label))
            .collect();
        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .pb(px(2.))
            .child(div().w(px(226.)).child(self.combobox(
                ComboSpec {
                    shown: Some(options[conj_ix].label.clone()),
                    options,
                    selected: Some(conj_ix),
                    searchable: false,
                    ..ComboSpec::new("conjunction", ix)
                },
                theme,
                cx,
            )));
        if !path.is_empty() {
            let remove_path = path.clone();
            header = header.child(self.clickable_icon(
                "bases-group-remove",
                ix,
                "trash-2",
                theme.text_muted,
                theme,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let p = remove_path.clone();
                    this.edit_filter(section, move |tree| remove_at(tree, &p), cx);
                }),
            ));
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, item) in items_of(group).into_iter().flatten().enumerate() {
            let mut child = path.clone();
            child.push(i);
            let word = if i == 0 { "where" } else { conj };
            match item {
                Filter::Statement(s) => {
                    rows.push(self.render_rule(section, child, word, s, base, view, theme, cx))
                }
                nested => {
                    let inner = self.render_group(section, nested, child, base, view, theme, cx);
                    rows.push(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .w_full()
                            .gap(px(8.))
                            .child(self.conjunction_word(word, theme))
                            .child(div().flex_1().min_w(px(0.)).child(inner))
                            .into_any_element(),
                    );
                }
            }
        }
        if self.toolbar.pending_rule.as_ref() == Some(&(section, path.clone())) {
            let n = items_of(group).map_or(0, Vec::len);
            let word = if n == 0 { "where" } else { conj };
            rows.push(self.render_pending_rule(section, &path, word, base, view, theme, cx));
        }
        let add_path = path.clone();
        let add_group_path = path.clone();
        let actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .child(self.text_icon_button(
                "bases-filter-add",
                ix,
                "plus",
                "Add filter",
                theme,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.toolbar.pending_rule = Some((section, add_path.clone()));
                    let combo_ix = node_ix(section, &add_path) ^ 0x5555;
                    this.open_combo(
                        crate::bases::toolbar::Combo {
                            name: "ruleNewProperty",
                            ix: combo_ix,
                        },
                        true,
                        window,
                        cx,
                    );
                }),
            ))
            .child(self.text_icon_button(
                "bases-filter-add-group",
                ix,
                "plus",
                "Add filter group",
                theme,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let p = add_group_path.clone();
                    this.edit_filter(section, move |tree| add_group(tree, &p), cx);
                }),
            ));
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(6.))
            .child(header)
            .child(div().flex().flex_col().w_full().gap(px(6.)).children(rows))
            .child(actions)
            .into_any_element()
    }

    /// The *where* / *and* / *or* word at the head of a row.
    fn conjunction_word(&self, word: &str, theme: &Theme) -> AnyElement {
        div()
            .w(px(72.))
            .flex_shrink_0()
            .h(px(INPUT_HEIGHT))
            .flex()
            .items_center()
            .justify_end()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.text)
            .child(SharedString::from(word.to_string()))
            .into_any_element()
    }

    /// A `text-icon-button` of a group's footer.
    fn text_icon_button(
        &self,
        prefix: &'static str,
        ix: usize,
        icon_name: &'static str,
        label: &'static str,
        theme: &Theme,
        on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        div()
            .id((prefix, ix))
            .flex()
            .flex_row()
            .items_center()
            .h(px(25.))
            .p(px(4.))
            .gap(px(4.))
            .rounded(px(5.))
            .cursor_pointer()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.text_muted)
            .hover(|s| s.bg(theme.bg_alt))
            .on_click(on_click)
            .child(icon(icon_name, ICON_SIZE, theme.text_muted))
            .child(SharedString::from(label))
            .into_any_element()
    }

    /// One `.filter-row`.
    #[allow(clippy::too_many_arguments)]
    fn render_rule(
        &mut self,
        section: Section,
        path: Vec<usize>,
        word: &str,
        statement: &str,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ix = node_ix(section, &path);
        let parsed = {
            let kind_of = self.kind_lookup(base, view);
            parse_rule(statement, &kind_of)
        };
        let advanced = parsed.is_none() || self.toolbar.advanced.contains(&(section, path.clone()));
        self.toolbar.record(json!({
            "section": section.as_str(),
            "path": path,
            "rule": statement,
            "advanced": advanced,
            "property": parsed.as_ref().map(|r| r.property.clone()),
            "operator": parsed.as_ref().map(|r| r.operator.label()),
        }));
        let body: AnyElement = match (&parsed, advanced) {
            (Some(rule), false) => {
                self.render_simple_rule(section, &path, rule, base, view, theme, cx)
            }
            _ => {
                let text = match &self.toolbar.editing {
                    Some(Field::Rule(s, p)) if *s == section && *p == path => self.field_text(cx),
                    _ => statement.to_string(),
                };
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(self.text_box(Field::Rule(section, path.clone()), &text, "", theme, cx))
                    .into_any_element()
            }
        };
        let toggle_path = path.clone();
        let can_toggle = parsed.is_some();
        let remove_path = path.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(INPUT_HEIGHT))
            .gap(px(8.))
            .child(self.conjunction_word(word, theme))
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .p(px(2.))
                    .child(self.clickable_icon(
                        "bases-rule-mode",
                        ix,
                        if advanced {
                            "mouse-pointer-click"
                        } else {
                            "code-xml"
                        },
                        if can_toggle {
                            theme.text_muted
                        } else {
                            theme.text_faint
                        },
                        theme,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            if !can_toggle {
                                return;
                            }
                            let k = (section, toggle_path.clone());
                            if let Some(pos) = this.toolbar.advanced.iter().position(|a| *a == k) {
                                this.toolbar.advanced.remove(pos);
                            } else {
                                this.toolbar.advanced.push(k);
                            }
                            if matches!(
                                this.toolbar.editing,
                                Some(Field::Rule(..)) | Some(Field::RuleValue(..))
                            ) {
                                this.toolbar.editing = None;
                                let _ = window;
                            }
                            cx.notify();
                        }),
                    ))
                    .child(self.clickable_icon(
                        "bases-rule-remove",
                        ix,
                        "trash-2",
                        theme.text_muted,
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            let p = remove_path.clone();
                            this.toolbar.editing = None;
                            this.edit_filter(section, move |tree| remove_at(tree, &p), cx);
                        }),
                    )),
            )
            .into_any_element()
    }

    /// `.filter-expression`: the property, operator and value of a rule.
    #[allow(clippy::too_many_arguments)]
    fn render_simple_rule(
        &mut self,
        section: Section,
        path: &[usize],
        rule: &Rule,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ix = node_ix(section, path);
        let (ids, options) = self.rule_property_options(base, view);
        let selected = ids.iter().position(|id| {
            same_property(id, &rule.property) || (id == "file" && rule.property == "file")
        });
        let kind = self.kind_lookup(base, view)(&rule.property);
        let shown = selected
            .map(|i| options[i].label.clone())
            .unwrap_or_else(|| SharedString::from(rule.property.clone()));
        let icon_name = selected
            .and_then(|i| options[i].icon)
            .unwrap_or_else(|| kind_icon(kind));
        let operators = operators_for(kind);
        let op_ix = operators.iter().position(|o| *o == rule.operator);
        let op_options: Vec<ComboOption> = operators
            .iter()
            .map(|o| ComboOption::plain(o.label()))
            .collect();
        let mut expr = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_1()
            .min_w(px(0.))
            .h(px(INPUT_HEIGHT))
            .rounded(px(6.))
            .bg(theme.bg_alt)
            .border_1()
            .border_color(theme.tab_divider)
            .overflow_hidden()
            .child(self.combobox(
                ComboSpec {
                    icon: Some(icon_name),
                    shown: Some(shown),
                    options,
                    selected,
                    flat: true,
                    ..ComboSpec::new("ruleProperty", ix)
                },
                theme,
                cx,
            ))
            .child(
                div()
                    .h_full()
                    .border_l_1()
                    .border_color(theme.border)
                    .child(self.combobox(
                        ComboSpec {
                            shown: Some(SharedString::from(rule.operator.label())),
                            options: op_options,
                            selected: op_ix,
                            searchable: false,
                            flat: true,
                            ..ComboSpec::new("ruleOperator", ix)
                        },
                        theme,
                        cx,
                    )),
            );
        let value_kind = rule.operator.value_kind_for(kind);
        let value_field = Field::RuleValue(section, path.to_vec());
        match value_kind {
            ValueKind::None => {}
            ValueKind::Checkbox => {
                let on = matches!(rule.value, RuleValue::Bool(true));
                let flip_path = path.to_vec();
                expr = expr.child(
                    div()
                        .id(("bases-rule-check", ix))
                        .px(px(8.))
                        .h_full()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            let p = flip_path.clone();
                            this.set_rule_value(section, &p, RuleValue::Bool(!on), cx);
                        }))
                        .child(self.check_box(on, theme)),
                );
            }
            ValueKind::List => {
                let items: Vec<String> = match &rule.value {
                    RuleValue::List(items) => items.clone(),
                    RuleValue::Text(t) if !t.is_empty() => vec![t.clone()],
                    _ => Vec::new(),
                };
                let typed = match &self.toolbar.editing {
                    Some(Field::RuleValue(s, p)) if *s == section && p.as_slice() == path => {
                        self.field_text(cx)
                    }
                    _ => String::new(),
                };
                let mut pills = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .px(px(8.))
                    .gap(px(4.))
                    .border_l_1()
                    .border_color(theme.border);
                for (i, item) in items.iter().enumerate() {
                    let remove_path = path.to_vec();
                    let remaining: Vec<String> = items
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, s)| s.clone())
                        .collect();
                    pills = pills.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .px(px(6.))
                            .h(px(17.))
                            .rounded(px(4.))
                            .bg(theme.bg_active)
                            .text_size(px(FONT_UI_SMALL))
                            .child(SharedString::from(item.clone()))
                            .child(
                                div()
                                    .id(("bases-rule-pill", ix * 64 + i))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        let p = remove_path.clone();
                                        this.set_rule_value(
                                            section,
                                            &p,
                                            RuleValue::List(remaining.clone()),
                                            cx,
                                        );
                                    }))
                                    .child(icon("x", 12., theme.text_muted)),
                            ),
                    );
                }
                pills = pills.child(div().flex_1().min_w(px(40.)).child(self.flat_text(
                    value_field,
                    &typed,
                    "",
                    theme,
                    cx,
                )));
                expr = expr.child(pills);
            }
            _ => {
                let text = match &rule.value {
                    RuleValue::Text(t) | RuleValue::Number(t) => t.clone(),
                    RuleValue::Bool(b) => b.to_string(),
                    RuleValue::List(items) => items.join(", "),
                    RuleValue::None => String::new(),
                };
                let typed = match &self.toolbar.editing {
                    Some(Field::RuleValue(s, p)) if *s == section && p.as_slice() == path => {
                        self.field_text(cx)
                    }
                    _ => text,
                };
                expr = expr.child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .h_full()
                        .border_l_1()
                        .border_color(theme.border)
                        .child(self.flat_text(value_field, &typed, "Empty", theme, cx)),
                );
            }
        }
        expr.into_any_element()
    }

    /// A value input inside the expression box: the shared field with no
    /// box of its own.
    fn flat_text(
        &self,
        which: Field,
        value: &str,
        placeholder: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.toolbar.editing.as_ref() == Some(&which) {
            return self.text_box(which, value, placeholder, theme, cx);
        }
        let text = value.to_string();
        let shown = if text.is_empty() {
            placeholder.to_string()
        } else {
            text.clone()
        };
        div()
            .id(("bases-rule-value", hash_key(which.as_str(), 0)))
            .w_full()
            .h_full()
            .px(px(8.))
            .flex()
            .items_center()
            .cursor_text()
            .text_size(px(FONT_UI_SMALL))
            .text_color(if text.is_empty() {
                theme.text_faint
            } else {
                theme.text
            })
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
            .child(SharedString::from(shown))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.open_field(which.clone(), text.clone(), window, cx);
            }))
            .into_any_element()
    }

    /// The row **Add filter** opened: its property combobox, open, and
    /// nothing written until a property is picked.
    #[allow(clippy::too_many_arguments)]
    fn render_pending_rule(
        &mut self,
        section: Section,
        path: &[usize],
        word: &str,
        base: &Base,
        view: &View,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ix = node_ix(section, path) ^ 0x5555;
        let (_, options) = self.rule_property_options(base, view);
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(INPUT_HEIGHT))
            .gap(px(8.))
            .child(self.conjunction_word(word, theme))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .min_w(px(0.))
                    .h(px(INPUT_HEIGHT))
                    .rounded(px(6.))
                    .bg(theme.bg_alt)
                    .border_1()
                    .border_color(theme.tab_divider)
                    .child(self.combobox(
                        ComboSpec {
                            placeholder: "Property",
                            options,
                            flat: true,
                            ..ComboSpec::new("ruleNewProperty", ix)
                        },
                        theme,
                        cx,
                    )),
            )
            .child(div().p(px(2.)).child(self.clickable_icon(
                "bases-rule-remove",
                ix,
                "trash-2",
                theme.text_muted,
                theme,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_combo(window, cx);
                }),
            )))
            .into_any_element()
    }

    // ----- what the rows write ----------------------------------------------

    /// The rule at `path` of `section`, parsed, with the kind of its
    /// property.
    fn rule_at(&self, section: Section, path: &[usize]) -> Option<(Rule, PropertyKind)> {
        let base = self.base()?;
        let view = self.showing_view(base);
        let tree = self.section_tree(section);
        let node = at(&tree, path)?;
        let Filter::Statement(s) = node else {
            return None;
        };
        let kind_of = self.kind_lookup(base, &view);
        let rule = parse_rule(s, &kind_of)?;
        let kind = kind_of(&rule.property);
        Some((rule, kind))
    }

    fn write_rule(&mut self, section: Section, path: &[usize], rule: Rule, cx: &mut Context<Self>) {
        let p = path.to_vec();
        let statement = rule.statement();
        self.edit_filter(section, move |tree| set_rule(tree, &p, &statement), cx);
    }

    fn set_rule_value(
        &mut self,
        section: Section,
        path: &[usize],
        value: RuleValue,
        cx: &mut Context<Self>,
    ) {
        let Some((mut rule, _)) = self.rule_at(section, path) else {
            return;
        };
        rule.value = value;
        self.write_rule(section, path, rule, cx);
    }

    /// An advanced row's field: the statement, verbatim.
    pub(super) fn commit_rule_text(
        &mut self,
        section: Section,
        path: &[usize],
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let tree = self.section_tree(section);
        match at(&tree, path) {
            Some(Filter::Statement(now)) if now == text => return,
            Some(Filter::Statement(_)) => {}
            _ => return,
        }
        let p = path.to_vec();
        let text = text.to_string();
        self.edit_filter(section, move |tree| set_rule(tree, &p, &text), cx);
    }

    /// A simple row's value field, by the kind of value the operator takes.
    /// On a list the field adds a pill on Enter rather than on every
    /// keystroke, which is why a list row's text is only committed from
    /// [`BasesView::submit_field`].
    pub(super) fn commit_rule_value(
        &mut self,
        section: Section,
        path: &[usize],
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some((rule, kind)) = self.rule_at(section, path) else {
            return;
        };
        let value = match rule.operator.value_kind_for(kind) {
            ValueKind::None => RuleValue::None,
            ValueKind::Checkbox => RuleValue::Bool(text.trim() == "true"),
            ValueKind::Number => RuleValue::Number(text.trim().to_string()),
            ValueKind::List => {
                // Enter adds the typed item; typing alone changes nothing.
                if self.toolbar.editing.as_ref() == Some(&Field::RuleValue(section, path.to_vec()))
                {
                    cx.notify();
                    return;
                }
                let mut items = match rule.value.clone() {
                    RuleValue::List(items) => items,
                    RuleValue::Text(t) if !t.is_empty() => vec![t],
                    _ => Vec::new(),
                };
                let item = text.trim().to_string();
                if !item.is_empty() && !items.contains(&item) {
                    items.push(item);
                }
                RuleValue::List(items)
            }
            _ => RuleValue::Text(text.to_string()),
        };
        if rule.value == value {
            return;
        }
        let mut next = rule;
        next.value = value;
        self.write_rule(section, path, next, cx);
    }

    /// Enter in a list row's field: the typed item becomes a pill.
    pub(super) fn submit_rule_value(
        &mut self,
        section: Section,
        path: &[usize],
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some((rule, kind)) = self.rule_at(section, path) else {
            return;
        };
        if rule.operator.value_kind_for(kind) != ValueKind::List {
            return;
        }
        let mut items = match rule.value.clone() {
            RuleValue::List(items) => items,
            RuleValue::Text(t) if !t.is_empty() => vec![t],
            _ => Vec::new(),
        };
        let item = text.trim().to_string();
        if item.is_empty() || items.contains(&item) {
            return;
        }
        items.push(item);
        let mut next = rule;
        next.value = RuleValue::List(items);
        self.write_rule(section, path, next, cx);
    }

    /// A pick in one of this panel's comboboxes; answers whether the name
    /// was one of its own.
    pub(super) fn filter_pick(
        &mut self,
        name: &str,
        ix: usize,
        opt: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        match name {
            "conjunction" => {
                let Some((section, path)) = self.node_by_ix(ix) else {
                    return true;
                };
                if let Some((conj, _)) = CONJUNCTIONS.get(opt) {
                    let conj = *conj;
                    self.edit_filter(section, move |tree| set_conjunction(tree, &path, conj), cx);
                }
            }
            "ruleProperty" => {
                let Some((section, path)) = self.node_by_ix(ix) else {
                    return true;
                };
                let Some(base) = self.base().cloned() else {
                    return true;
                };
                let view = self.showing_view(&base);
                let (ids, _) = self.rule_property_options(&base, &view);
                let Some(id) = ids.get(opt).cloned() else {
                    return true;
                };
                let kind = self.kind_lookup(&base, &view)(&id);
                let property = if id == "file" {
                    "file".to_string()
                } else {
                    id.strip_prefix("note.").map(str::to_string).unwrap_or(id)
                };
                let Some((old, _)) = self.rule_at(section, &path) else {
                    return true;
                };
                // The operator survives when the new kind offers it, and
                // the value when the operator still takes that kind of value.
                let mut rule = new_rule(&property, kind);
                if operators_for(kind).contains(&old.operator) {
                    rule.operator = old.operator;
                    if old.operator.value_kind_for(kind) == rule.operator.value_kind_for(kind) {
                        rule.value = old.value;
                    }
                }
                self.write_rule(section, &path, rule, cx);
            }
            "ruleOperator" => {
                let Some((section, path)) = self.node_by_ix(ix) else {
                    return true;
                };
                let Some((mut rule, kind)) = self.rule_at(section, &path) else {
                    return true;
                };
                let Some(op) = operators_for(kind).get(opt).copied() else {
                    return true;
                };
                let keep = op.value_kind_for(kind) == rule.operator.value_kind_for(kind);
                rule.operator = op;
                if !keep {
                    rule.value = RuleValue::None;
                }
                self.write_rule(section, &path, rule, cx);
            }
            "ruleNewProperty" => {
                let Some((section, group_path)) = self.toolbar.pending_rule.take() else {
                    return true;
                };
                let Some(base) = self.base().cloned() else {
                    return true;
                };
                let view = self.showing_view(&base);
                let (ids, _) = self.rule_property_options(&base, &view);
                let Some(id) = ids.get(opt).cloned() else {
                    return true;
                };
                let kind = self.kind_lookup(&base, &view)(&id);
                let property = if id == "file" {
                    "file".to_string()
                } else {
                    id.strip_prefix("note.").map(str::to_string).unwrap_or(id)
                };
                let statement = new_rule(&property, kind).statement();
                self.edit_filter(
                    section,
                    move |tree| add_rule(tree, &group_path, &statement),
                    cx,
                );
            }
            _ => return false,
        }
        true
    }

    /// The options of this panel's comboboxes, by name.
    pub(super) fn filter_options(
        &self,
        name: &str,
        ix: usize,
        base: &Base,
        view: &View,
    ) -> Option<Vec<ComboOption>> {
        Some(match name {
            "conjunction" => CONJUNCTIONS
                .iter()
                .map(|(_, label)| ComboOption::plain(*label))
                .collect(),
            "ruleProperty" | "ruleNewProperty" => self.rule_property_options(base, view).1,
            "ruleOperator" => {
                let (section, path) = self.node_by_ix(ix)?;
                let (_, kind) = self.rule_at(section, &path)?;
                operators_for(kind)
                    .iter()
                    .map(|o: &Operator| ComboOption::plain(o.label()))
                    .collect()
            }
            _ => return None,
        })
    }

    /// The node a combobox's index names, found by walking both sections.
    fn node_by_ix(&self, ix: usize) -> Option<(Section, Vec<usize>)> {
        for section in [Section::AllViews, Section::ThisView] {
            let tree = self.section_tree(section);
            let mut found = None;
            walk(&tree, Vec::new(), &mut |path| {
                if found.is_none() && node_ix(section, path) == ix {
                    found = Some(path.to_vec());
                }
            });
            if let Some(path) = found {
                return Some((section, path));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// The filter tree
// ---------------------------------------------------------------------------

/// The tree the panel edits: an absent `filters` and a bare statement are
/// both read as an `and` group, so every section has one group at its root
/// and a conjunction select to go with it.
pub fn group_tree(filter: Option<&Filter>) -> Filter {
    match filter {
        None => Filter::And(Vec::new()),
        Some(Filter::Statement(s)) => Filter::And(vec![Filter::Statement(s.clone())]),
        Some(other) => other.clone(),
    }
}

/// What a group tree is written back as: an empty root group is no
/// `filters` key at all, which is the shape the file had before the panel
/// touched it.
pub fn written(tree: Filter) -> Option<Filter> {
    let empty = match &tree {
        Filter::And(items) | Filter::Or(items) | Filter::Not(items) => items.is_empty(),
        Filter::Statement(_) => false,
    };
    if empty {
        None
    } else {
        Some(tree)
    }
}

pub fn conjunction_of(filter: &Filter) -> &'static str {
    match filter {
        Filter::And(_) => "and",
        Filter::Or(_) => "or",
        Filter::Not(_) => "not",
        Filter::Statement(_) => "and",
    }
}

fn items_of(filter: &Filter) -> Option<&Vec<Filter>> {
    match filter {
        Filter::And(items) | Filter::Or(items) | Filter::Not(items) => Some(items),
        Filter::Statement(_) => None,
    }
}

fn items_mut(filter: &mut Filter) -> Option<&mut Vec<Filter>> {
    match filter {
        Filter::And(items) | Filter::Or(items) | Filter::Not(items) => Some(items),
        Filter::Statement(_) => None,
    }
}

fn at<'f>(tree: &'f Filter, path: &[usize]) -> Option<&'f Filter> {
    match path.split_first() {
        None => Some(tree),
        Some((head, rest)) => at(items_of(tree)?.get(*head)?, rest),
    }
}

fn at_mut<'f>(tree: &'f mut Filter, path: &[usize]) -> Option<&'f mut Filter> {
    match path.split_first() {
        None => Some(tree),
        Some((head, rest)) => at_mut(items_mut(tree)?.get_mut(*head)?, rest),
    }
}

/// Visits every node's path.
fn walk(tree: &Filter, path: Vec<usize>, f: &mut dyn FnMut(&[usize])) {
    f(&path);
    for (i, item) in items_of(tree).into_iter().flatten().enumerate() {
        let mut child = path.clone();
        child.push(i);
        walk(item, child, f);
    }
}

/// How many statements a tree holds, which is the section's flair.
pub fn count_rules(tree: &Filter) -> usize {
    match tree {
        Filter::Statement(_) => 1,
        group => items_of(group).into_iter().flatten().map(count_rules).sum(),
    }
}

/// Rewraps the group at `path` under another conjunction, keeping its rules.
pub fn set_conjunction(tree: &mut Filter, path: &[usize], conjunction: &str) {
    let Some(node) = at_mut(tree, path) else {
        return;
    };
    let items = match items_mut(node) {
        Some(items) => std::mem::take(items),
        None => return,
    };
    *node = match conjunction {
        "or" => Filter::Or(items),
        "not" => Filter::Not(items),
        _ => Filter::And(items),
    };
}

/// Replaces the rule at `path` with `text`, verbatim.
pub fn set_rule(tree: &mut Filter, path: &[usize], text: &str) {
    if let Some(node) = at_mut(tree, path) {
        *node = Filter::Statement(text.to_string());
    }
}

/// Appends a rule to the group at `path`. An empty statement writes nothing.
pub fn add_rule(tree: &mut Filter, path: &[usize], text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(items) = at_mut(tree, path).and_then(items_mut) {
        items.push(Filter::Statement(text.to_string()));
    }
}

/// Appends an empty `and` group to the group at `path`.
pub fn add_group(tree: &mut Filter, path: &[usize]) {
    if let Some(items) = at_mut(tree, path).and_then(items_mut) {
        items.push(Filter::And(Vec::new()));
    }
}

/// Removes whatever is at `path` from its parent group.
pub fn remove_at(tree: &mut Filter, path: &[usize]) {
    let Some((last, parent)) = path.split_last() else {
        return;
    };
    if let Some(items) = at_mut(tree, parent).and_then(items_mut) {
        if *last < items.len() {
            items.remove(*last);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_formats::base::Base;

    fn base_of(yaml: &str) -> Base {
        Base::from_yaml(yaml).expect("parses")
    }

    #[test]
    fn an_absent_filter_reads_as_an_empty_and_group_and_writes_back_as_nothing() {
        let tree = group_tree(None);
        assert_eq!(tree, Filter::And(Vec::new()));
        assert_eq!(written(tree), None);
    }

    #[test]
    fn the_conjunction_select_rewraps_a_group_and_keeps_its_rules() {
        let mut base = base_of(
            "filters:\n  and:\n    - status != \"done\"\n    - not:\n        - file.ext == \"md\"\n",
        );
        let mut tree = group_tree(base.filters.as_ref());
        set_conjunction(&mut tree, &[], "or");
        base.filters = written(tree);
        assert_eq!(
            base.to_yaml(),
            "filters:\n  or:\n    - status != \"done\"\n    - not:\n        - file.ext == \"md\"\n"
        );
    }

    #[test]
    fn a_nested_group_is_reached_by_its_path_and_counted_by_its_rules() {
        let mut base = base_of("filters:\n  and:\n    - a\n    - not:\n        - b\n");
        let tree = group_tree(base.filters.as_ref());
        assert_eq!(count_rules(&tree), 2);
        let mut paths = Vec::new();
        walk(&tree, Vec::new(), &mut |p| paths.push(p.to_vec()));
        assert_eq!(paths, vec![vec![], vec![0], vec![1], vec![1, 0]]);
        let mut tree = group_tree(base.filters.as_ref());
        set_conjunction(&mut tree, &[1], "or");
        base.filters = written(tree);
        assert_eq!(
            base.to_yaml(),
            "filters:\n  and:\n    - a\n    - or:\n        - b\n"
        );
    }

    #[test]
    fn a_rules_statement_is_written_verbatim() {
        let mut base = base_of("filters:\n  and:\n    - a\n");
        let mut tree = group_tree(base.filters.as_ref());
        set_rule(&mut tree, &[0], "type + \" \"  + file.name");
        base.filters = written(tree);
        assert_eq!(
            base.to_yaml(),
            "filters:\n  and:\n    - type + \" \"  + file.name\n"
        );
    }

    #[test]
    fn the_last_rule_of_a_group_takes_the_filters_key_with_it() {
        let mut base = base_of("filters:\n  and:\n    - a\n");
        let mut tree = group_tree(base.filters.as_ref());
        remove_at(&mut tree, &[0]);
        base.filters = written(tree);
        assert_eq!(base.to_yaml(), "");
    }

    #[test]
    fn a_new_rule_and_a_new_group_land_at_the_end_of_their_group() {
        let mut base = base_of("filters:\n  and:\n    - a\n");
        let mut tree = group_tree(base.filters.as_ref());
        add_rule(&mut tree, &[], "");
        add_rule(&mut tree, &[], "status == \"todo\"");
        add_group(&mut tree, &[]);
        base.filters = written(tree);
        assert_eq!(
            base.to_yaml(),
            "filters:\n  and:\n    - a\n    - status == \"todo\"\n    - and: []\n"
        );
    }

    #[test]
    fn a_this_view_filter_lands_on_the_view_and_an_all_views_one_on_the_base() {
        let mut base = base_of("views:\n  - type: table\n    name: V\n");
        let mut tree = group_tree(base.filters.as_ref());
        add_rule(&mut tree, &[], "file.ext == \"md\"");
        base.filters = written(tree);
        let mut tree = group_tree(base.views[0].filters.as_ref());
        add_rule(&mut tree, &[], "status != \"done\"");
        base.views[0].filters = written(tree);
        assert_eq!(
            base.to_yaml(),
            "filters:\n  and:\n    - file.ext == \"md\"\nviews:\n  - type: table\n    name: V\n    filters:\n      and:\n        - status != \"done\"\n"
        );
    }

    #[test]
    fn node_indices_tell_sections_and_paths_apart() {
        assert_ne!(
            node_ix(Section::AllViews, &[]),
            node_ix(Section::ThisView, &[])
        );
        assert_ne!(
            node_ix(Section::AllViews, &[0]),
            node_ix(Section::AllViews, &[1])
        );
        assert_ne!(
            node_ix(Section::AllViews, &[0, 1]),
            node_ix(Section::AllViews, &[1, 0])
        );
    }
}
