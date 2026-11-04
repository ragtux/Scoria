//! The Bases toolbar: the bar above a view and the panels its buttons open.
//!
//! Everything drawn here is **observed** on Obsidian 1.14.5
//! (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`, steps
//! `s02-toolbar-per-layout`, `s13-keyboard` and `s14-styles`; the earlier
//! `2026-10-25-bases.json` step `s05` already held the same bar). The bar is a
//! 40 px `.bases-header` with a 29 px row of eight `text-icon-button`s in it:
//!
//! | Item | Icon | Label |
//! | --- | --- | --- |
//! | the view switcher | the layout's own | the view's name, then `chevrons-up-down` |
//! | results | `text-align-start` | *N results* |
//! | sort | `arrow-up-down` | *Sort* |
//! | group | `stretch-horizontal` | *Group* |
//! | filter | `list-filter` | *Filter* |
//! | properties | `list` | *Properties* |
//! | search | `search` | *Search* |
//! | new | `plus` | *New* |
//!
//! The first two sit at the left and the rest at the right. A button is
//! 25 px tall with 4 px of padding, a 5 px radius, a 13 px label in
//! `--text-muted`, the hover wash under the pointer, and the accent colour
//! (`is-active`) while its control holds a value — a sort, a filter of the
//! view's own, a group. **None of them has a tooltip.** The count badges
//! exist in the DOM but `--bases-toolbar-badge-display` is `none`, so they are
//! not drawn.
//!
//! Each of the five panel buttons opens a `.menu.bases-toolbar-menu` hung
//! 2 px under the button's left edge: 228 px wide (the filter panel 520), the
//! secondary background, a 1 px border, an 8 px radius and the menu shadow,
//! scrolling inside itself when taller than the window. The sixth button
//! shows the search row under the bar and the seventh creates a note.
//!
//! **Keyboard and focus** (`s13-keyboard`): Escape closes whichever panel is
//! open and puts focus back on the view; the views and properties panels
//! focus their search field on opening, where Down and Up move the selected
//! row and Enter activates it; the other panels leave focus on the bar.
//! Clicking an open panel's button closes it. A combobox opens its own
//! `suggestion-container` under its trigger with a search field of its own,
//! and a press outside that list closes the list alone.
//!
//! The panels themselves live beside this module — [`super::toolbar_views`],
//! [`super::toolbar_properties`], [`super::toolbar_filter`] and
//! [`super::toolbar_lists`] (sort, group, results) — and share the widgets
//! at the bottom of this file. Every write goes through
//! [`BasesView::edit_view`] or [`BasesView::edit_base`], so the bytes are
//! `Base::to_yaml`'s.

use crate::assets::icon;
use crate::bases::view::BasesView;
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER, ICON_SIZE};
use gpui::{
    actions, anchored, canvas, deferred, div, prelude::*, px, Anchor, AnyElement, App, Bounds,
    ClickEvent, Context, Entity, Focusable, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, SharedString, Subscription, Window,
};
use scoria_bases::layout;
use scoria_bases::rule::PropertyKind;
use scoria_formats::base::{Base, View, DEFAULT_VIEW_NAME, DEFAULT_VIEW_TYPE};
use scoria_formats::properties::PropertyType;
use serde_json::{json, Value as Json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

actions!(
    bases,
    [
        ClosePanel,
        SelectUp,
        SelectDown,
        Activate,
        CursorLeft,
        CursorRight,
        CursorNext,
        CursorPrev,
        DeleteCell
    ]
);

/// Escape in the view: closes the open panel, combobox or menu, one layer per
/// press, the way Obsidian's own does. Up, Down and Enter walk and pick in a
/// list that has no search field of its own (a `<select>`'s), where focus
/// stays on the view; a list with a field takes them through the field.
///
/// With nothing open, the same keys and Tab, Shift+Tab, Left, Right, Delete
/// and Backspace drive the table's own cursor (`edit`).
pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Bases");
    cx.bind_keys([
        KeyBinding::new("escape", ClosePanel, ctx),
        KeyBinding::new("up", SelectUp, ctx),
        KeyBinding::new("down", SelectDown, ctx),
        KeyBinding::new("enter", Activate, ctx),
        KeyBinding::new("left", CursorLeft, ctx),
        KeyBinding::new("right", CursorRight, ctx),
        KeyBinding::new("tab", CursorNext, ctx),
        KeyBinding::new("shift-tab", CursorPrev, ctx),
        KeyBinding::new("delete", DeleteCell, ctx),
        KeyBinding::new("backspace", DeleteCell, ctx),
    ]);
}

/// `.bases-header`: 40 px, with the 29 px bar centred in it and a 1 px
/// bottom border.
pub const HEADER_HEIGHT: f32 = 40.;
const BAR_HEIGHT: f32 = 29.;
/// A `text-icon-button`: 25 px tall, 4 px of padding, 2 px of margin.
const BUTTON_HEIGHT: f32 = 25.;
/// The panels' width; the filter panel has its own.
pub(super) const PANEL_WIDTH: f32 = 228.;
pub(super) const FILTER_PANEL_WIDTH: f32 = 520.;
/// A combobox's `suggestion-container`.
pub(super) const COMBO_WIDTH: f32 = 228.;
/// A row of a panel list: 25 px, padded 4 / 4 / 4 / 6.
pub(super) const ROW_HEIGHT: f32 = 25.;
/// An input, a combobox or a select inside a panel: 28 px.
pub(super) const INPUT_HEIGHT: f32 = 28.;

/// Which panel, if any, is open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Panel {
    #[default]
    None,
    Views,
    Results,
    Sort,
    Group,
    Filter,
    Properties,
    /// The Summarize page, opened from a column header or a summary cell
    /// rather than from a button of the bar.
    Summary,
}

impl Panel {
    /// The name the control socket reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Panel::None => "none",
            Panel::Views => "views",
            Panel::Results => "results",
            Panel::Sort => "sort",
            Panel::Group => "group",
            Panel::Filter => "filter",
            Panel::Properties => "properties",
            Panel::Summary => "summary",
        }
    }
}

/// The page a panel is on: its list, or one of the pages a row's gear opens
/// in place of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    List,
    /// The views panel's **Configure view** page for the view at this index.
    Configure(usize),
    /// The properties panel's **Edit *name*** page for a property, by id.
    Property(String),
    /// The properties panel's **Edit *name*** page for a formula, by name.
    Formula(String),
    /// The Summarize page's **Add summary**: the name and formula being
    /// typed, and the name already written to the base's own `summaries`,
    /// if one is (nothing is written until the formula parses).
    NewSummary {
        name: String,
        formula: String,
        written: Option<String>,
    },
}

/// Which of the filter panel's two sections a node belongs to: the base's
/// own `filters` ("All views") or the showing view's ("This view").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    AllViews,
    ThisView,
}

impl Section {
    /// Obsidian's own heading for the section.
    pub fn heading(self) -> &'static str {
        match self {
            Section::AllViews => "All views",
            Section::ThisView => "This view",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Section::AllViews => "all",
            Section::ThisView => "view",
        }
    }

    fn ix(self) -> usize {
        match self {
            Section::AllViews => 0,
            Section::ThisView => 1,
        }
    }
}

/// Which text control the one shared [`TextInput`] is standing in for.
///
/// One field serves the whole toolbar: only one panel is open at a time and
/// inside it only one field is typed into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    /// The search field at the head of the views and properties panels.
    Search,
    /// The search field of an open combobox's list.
    ComboSearch,
    /// The results panel's "Limit number of results" number input.
    Limit,
    /// The Configure view page's name input.
    ViewName,
    /// The list layout's "Property separator".
    Separator,
    /// An Edit page's "Display name".
    DisplayName,
    /// A formula page's statement, and the Add summary page's formula.
    Formula,
    /// The Add summary page's "Summary name".
    SummaryName,
    /// A filter rule in advanced mode: its statement, verbatim. The path is
    /// the node's path within its section.
    Rule(Section, Vec<usize>),
    /// A filter rule's value input.
    RuleValue(Section, Vec<usize>),
    /// The group panel's "Add group" name.
    GroupAdd,
    /// The toolbar's own search row, under the bar.
    TableSearch,
}

impl Field {
    pub fn as_str(&self) -> &'static str {
        match self {
            Field::Search => "search",
            Field::ComboSearch => "combo",
            Field::Limit => "limit",
            Field::ViewName => "name",
            Field::Separator => "separator",
            Field::DisplayName => "displayName",
            Field::Formula => "formula",
            Field::Rule(..) => "rule",
            Field::RuleValue(..) => "value",
            Field::GroupAdd => "groupAdd",
            Field::TableSearch => "tableSearch",
            Field::SummaryName => "summaryName",
        }
    }

    /// The element id a field's box is drawn under: its name, and for a
    /// filter row's field the row too, since every row has one and two
    /// boxes sharing an id lose each other's clicks.
    pub fn element_ix(&self) -> usize {
        let row = match self {
            Field::Rule(section, path) | Field::RuleValue(section, path) => {
                super::toolbar_filter::node_ix(*section, path)
            }
            _ => 0,
        };
        hash_key(self.as_str(), row)
    }
}

/// An open combobox: the name its owner gave it and the entry it belongs to
/// (0 where there is only one of it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub name: &'static str,
    pub ix: usize,
}

/// What the toolbar keeps between renders.
pub struct ToolbarState {
    pub panel: Panel,
    pub page: Page,
    pub combo: Option<Combo>,
    /// The keyboard-selected row of the open list, when it has one.
    pub selected: Option<usize>,
    /// Whether the Configure view page's own `more-vertical` menu is open.
    pub header_menu: bool,
    /// A sort row added and not yet given a property: drawn, not written.
    pub pending_sort: bool,
    /// A filter row added and not yet given a property, by section and the
    /// path of the group it will land in.
    pub pending_rule: Option<(Section, Vec<usize>)>,
    /// The filter rows showing their statement rather than a property,
    /// operator and value.
    pub advanced: Vec<(Section, Vec<usize>)>,
    /// Which of the filter panel's sections are folded. "All views" starts
    /// folded, which is how the panel first opened in the run.
    pub collapsed: [bool; 2],
    /// The search row under the bar and its text; `None` while hidden.
    pub search: Option<String>,
    /// A grip being dragged: the list it belongs to and the row's index.
    pub drag: Option<(&'static str, usize)>,
    /// Where a page opened from somewhere other than its button hangs --
    /// the column header's *Edit property...* and *Summarize...* open the
    /// properties panel's page under the header -- which outranks the
    /// button's box until the panel closes.
    pub pinned_at: Option<Point<Pixels>>,
    /// Which column and rows the Summarize page is about while it is open.
    pub summary: Option<super::summary::SummaryTarget>,
    /// Where the open panel was asked for, when no button box is known.
    at: Point<Pixels>,
    /// The shared text field, made the first time a panel wants one.
    field: Option<Entity<TextInput>>,
    /// What [`ToolbarState::field`] is standing in for, if anything.
    pub editing: Option<Field>,
    /// A field asked for from somewhere without a window — a command, a
    /// page change — which the next render opens.
    pending_field: Option<(Field, String)>,
    /// Escape closed the formula editor's completion list; the next
    /// keystroke opens it again.
    completions_closed: bool,
    /// What the field last held, so the control socket can read it without
    /// the app context a [`TextInput`] needs.
    field_value: String,
    /// The boxes of the bar's buttons, every combobox trigger and every open
    /// list, as painted last frame. A panel hangs under its button's box, a
    /// list under its trigger's, and a press inside any of them is not a
    /// press "outside" the panel. The bar's own boxes live for the view;
    /// a panel's are forgotten when the panel or its list closes, so a box
    /// that is no longer drawn cannot swallow a press.
    anchors: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// How many rows the showing view resolves to with its `limit` dropped.
    total: Option<usize>,
    /// What the open panel drew, for the control socket.
    pub(super) rows: Vec<Json>,
    subscriptions: Vec<Subscription>,
}

impl Default for ToolbarState {
    fn default() -> ToolbarState {
        ToolbarState {
            panel: Panel::None,
            page: Page::List,
            pinned_at: None,
            summary: None,
            combo: None,
            selected: None,
            header_menu: false,
            pending_sort: false,
            pending_rule: None,
            advanced: Vec::new(),
            collapsed: [true, false],
            search: None,
            drag: None,
            at: Point::default(),
            field: None,
            editing: None,
            pending_field: None,
            completions_closed: false,
            field_value: String::new(),
            anchors: Rc::new(RefCell::new(HashMap::new())),
            total: None,
            rows: Vec::new(),
            subscriptions: Vec::new(),
        }
    }
}

impl ToolbarState {
    pub fn state_json(&self) -> Json {
        let page = match &self.page {
            Page::List => json!("list"),
            Page::Configure(ix) => json!({ "configure": ix }),
            Page::Property(id) => json!({ "property": id }),
            Page::Formula(name) => json!({ "formula": name }),
            Page::NewSummary {
                name,
                formula,
                written,
            } => {
                json!({ "newSummary": { "name": name, "formula": formula, "written": written } })
            }
        };
        json!({
            "panel": self.panel.as_str(),
            "page": page,
            "configuring": match self.page { Page::Configure(ix) => Some(ix), _ => None },
            "combo": self.combo.map(|c| json!({ "name": c.name, "ix": c.ix })),
            "selected": self.selected,
            "header_menu": self.header_menu,
            "pending_sort": self.pending_sort,
            "pending_rule": self.pending_rule.as_ref().map(|(s, p)| json!({ "section": s.as_str(), "path": p })),
            "advanced": self.advanced.len(),
            "drag": self.drag.map(|(l, i)| json!({ "list": l, "ix": i })),
            "field": self.editing.as_ref().map(Field::as_str),
            "field_value": self.field_value,
            "search": self.search,
            "total": self.total,
            "rows": self.rows,
        })
    }

    pub(super) fn record(&mut self, row: Json) {
        self.rows.push(row);
    }

    pub(super) fn set_rows(&mut self, rows: Vec<Json>) {
        self.rows = rows;
    }
}

/// A stable number for an anchor or element key.
pub(super) fn key(name: &str, ix: usize) -> u64 {
    name.bytes()
        .fold((ix as u64).wrapping_mul(0x9E37_79B9), |h, b| {
            h.wrapping_mul(1_099_511_628_211).wrapping_add(b as u64)
        })
}

/// The same number cut down to an element id.
pub(super) fn hash_key(name: &str, ix: usize) -> usize {
    (key(name, ix) % 1_000_000_007) as usize
}

// ---------------------------------------------------------------------------
// The bar
// ---------------------------------------------------------------------------

/// One button of the bar.
struct BarItem {
    id: &'static str,
    icon: &'static str,
    label: SharedString,
    active: bool,
    open: bool,
    panel: Option<Panel>,
    aux: Option<&'static str>,
}

impl BasesView {
    /// The toolbar above the layout: the bar, the search row when it is
    /// showing, and whichever panel is open.
    pub(super) fn render_toolbar(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let base = self.base()?.clone();
        let view = self.showing_view(&base);
        if let Some((which, text)) = self.toolbar.pending_field.take() {
            self.open_field(which, text, window, cx);
        }
        let count = self.table().map_or(0, |t| t.rows.len());
        let icon_name = layout::kind_icon(&view.kind).unwrap_or("file-question");
        let open = self.toolbar.panel;
        let items = [
            BarItem {
                id: "views",
                icon: icon_name,
                label: SharedString::from(view.name.clone()),
                active: false,
                open: open == Panel::Views,
                panel: Some(Panel::Views),
                aux: Some("chevrons-up-down"),
            },
            BarItem {
                id: "results",
                icon: "text-align-start",
                label: SharedString::from(format!(
                    "{count} result{}",
                    if count == 1 { "" } else { "s" }
                )),
                active: false,
                open: open == Panel::Results,
                panel: Some(Panel::Results),
                aux: None,
            },
        ];
        let right = [
            BarItem {
                id: "sort",
                icon: "arrow-up-down",
                label: SharedString::from("Sort"),
                active: !view.sort.is_empty(),
                open: open == Panel::Sort,
                panel: Some(Panel::Sort),
                aux: None,
            },
            BarItem {
                id: "group",
                icon: "stretch-horizontal",
                label: SharedString::from("Group"),
                active: view.group_by.is_some(),
                open: open == Panel::Group,
                panel: Some(Panel::Group),
                aux: None,
            },
            BarItem {
                id: "filter",
                icon: "list-filter",
                label: SharedString::from("Filter"),
                active: view.filters.is_some(),
                open: open == Panel::Filter,
                panel: Some(Panel::Filter),
                aux: None,
            },
            BarItem {
                id: "properties",
                icon: "list",
                label: SharedString::from("Properties"),
                active: false,
                open: open == Panel::Properties,
                panel: Some(Panel::Properties),
                aux: None,
            },
            BarItem {
                id: "search",
                icon: "search",
                label: SharedString::from("Search"),
                active: self.toolbar.search.is_some(),
                open: false,
                panel: None,
                aux: None,
            },
            BarItem {
                id: "new",
                icon: "plus",
                label: SharedString::from("New"),
                active: false,
                open: false,
                panel: None,
                aux: None,
            },
        ];
        let bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(BAR_HEIGHT))
            .children(
                items
                    .into_iter()
                    .map(|item| self.render_bar_item(item, theme, cx)),
            )
            .child(div().flex_1())
            .children(
                right
                    .into_iter()
                    .map(|item| self.render_bar_item(item, theme, cx)),
            );
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_shrink_0()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .px(px(2.))
            // `.bases-header`: ruled in `--bases-table-border-color`.
            .border_b_1()
            .border_color(theme.tokens.bases_table_border_color)
            .child(bar);
        let search_row = self.render_search_row(theme, cx);
        let panel = self.render_panel(&base, &view, theme, window, cx);
        Some(
            div()
                .flex()
                .flex_col()
                .flex_shrink_0()
                .w_full()
                .child(header)
                .children(search_row)
                .children(panel)
                .into_any_element(),
        )
    }

    /// One `text-icon-button` of the bar: muted, the views button in the
    /// normal colour, an `is-active` one (a sort, a group, a filter, the
    /// search showing) `--icon-color-active`, and the one whose menu is
    /// open (`has-active-menu`) the normal colour on the hover wash. The
    /// views button's chevrons stay muted (probe of 2026-11-04).
    fn render_bar_item(&self, item: BarItem, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let color = if item.open || item.id == "views" {
            theme.tokens.text_normal
        } else if item.active {
            theme.tokens.icon_color_active
        } else {
            theme.tokens.text_muted
        };
        let id = item.id;
        let anchor = item.panel.map(|p| key("bases-toolbar-button", p as usize));
        div()
            .id(("bases-toolbar-item", hash_key(id, 0)))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .flex_shrink_0()
            .h(px(BUTTON_HEIGHT))
            .m(px(2.))
            .p(px(4.))
            .gap(px(4.))
            .rounded(px(5.))
            .cursor_pointer()
            .text_size(px(FONT_UI_SMALL))
            .text_color(color)
            .when(item.open, |d| d.bg(theme.tokens.background_modifier_hover))
            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            .children(anchor.map(|k| self.measure(k)))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                match item.panel {
                    Some(panel) => this.toggle_panel(panel, event.position(), window, cx),
                    None if id == "search" => this.toggle_search(window, cx),
                    None => this.add_item(cx),
                }
            }))
            .child(icon(item.icon, ICON_SIZE, color))
            .child(div().whitespace_nowrap().child(item.label))
            .children(item.aux.map(|n| icon(n, 14., theme.tokens.text_muted)))
            .into_any_element()
    }

    /// Records the box of the element this is a child of, under `k`, so a
    /// later frame can hang something under it or tell a press inside it
    /// from one outside. The parent must be `relative()`.
    pub(super) fn measure(&self, k: u64) -> AnyElement {
        let anchors = self.toolbar.anchors.clone();
        canvas(
            move |bounds, _, _| {
                anchors.borrow_mut().insert(k, bounds);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }

    pub(super) fn anchor(&self, k: u64) -> Option<Bounds<Pixels>> {
        self.toolbar.anchors.borrow().get(&k).copied()
    }

    /// The formula editor's completions for what it holds now: the word they
    /// replace and the rows, empty while Escape has closed the list.
    pub(super) fn formula_completions(
        &self,
        cx: &Context<Self>,
    ) -> (std::ops::Range<usize>, Vec<super::complete::Completion>) {
        if self.toolbar.editing != Some(Field::Formula) || self.toolbar.completions_closed {
            return (0..0, Vec::new());
        }
        let text = self.field_text(cx);
        let formulas: Vec<String> = self
            .base()
            .map(|b| b.formulas.keys().cloned().collect())
            .unwrap_or_default();
        let scope = super::complete::Scope {
            properties: &self.property_kinds,
            formulas: &formulas,
            summary: matches!(self.toolbar.page, Page::NewSummary { .. }),
        };
        super::complete::completions(&text, &scope)
    }

    /// Puts completion `ix` in place of the word being typed; answers
    /// whether there was one.
    pub(super) fn accept_completion(&mut self, ix: usize, cx: &mut Context<Self>) -> bool {
        let (range, list) = self.formula_completions(cx);
        let Some(pick) = list.get(ix) else {
            return false;
        };
        let next = super::complete::accept(&self.field_text(cx), range, pick);
        if let Some(input) = self.toolbar.field.clone() {
            input.update(cx, |input, cx| input.set_text(next, cx));
        }
        // The accepted name is whole, so the list has nothing left to offer
        // until the next keystroke.
        self.toolbar.completions_closed = true;
        self.toolbar.selected = None;
        cx.notify();
        true
    }

    /// `.cm-tooltip-autocomplete`: the completion rows under the editor,
    /// each its type icon and its name.
    pub(super) fn render_formula_completions(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (_, list) = self.formula_completions(cx);
        let editor = self.anchor(FORMULA_EDITOR_KEY).filter(|_| !list.is_empty());
        let Some(editor) = editor else {
            self.forget_anchor(FORMULA_LIST_KEY);
            return None;
        };
        let at = Point::new(
            editor.origin.x,
            editor.origin.y + editor.size.height + px(3.),
        );
        let selected = self.toolbar.selected;
        let rows = list.into_iter().enumerate().map(|(ix, c)| {
            div()
                .id(("bases-formula-completion", ix))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(29.))
                .px(px(8.))
                .rounded(px(4.))
                .cursor_pointer()
                .when(selected == Some(ix), |d| {
                    d.bg(theme.tokens.background_modifier_hover)
                })
                .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.accept_completion(ix, cx);
                }))
                .child(icon(c.icon, ICON_SIZE, theme.tokens.text_muted))
                .child(div().whitespace_nowrap().child(SharedString::from(c.label)))
        });
        let list = div()
            .id("bases-formula-completions")
            .relative()
            .child(self.measure(FORMULA_LIST_KEY))
            .flex()
            .flex_col()
            .min_w(px(160.))
            .max_h(px(300.))
            .overflow_y_scroll()
            .p(px(6.))
            .bg(theme.tokens.suggestion_background)
            .border_1()
            .border_color(theme.tokens.background_modifier_border)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .occlude()
            .children(rows);
        Some(
            deferred(
                anchored()
                    .position(at)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    /// Drops the box recorded under `k`, for something no longer drawn.
    pub(super) fn forget_anchor(&self, k: u64) {
        self.toolbar.anchors.borrow_mut().remove(&k);
    }

    /// Forgets every box but the bar's buttons: what a panel drew is gone
    /// with it.
    fn forget_panel_anchors(&self) {
        let keep: Vec<u64> = [
            Panel::Views,
            Panel::Results,
            Panel::Sort,
            Panel::Group,
            Panel::Filter,
            Panel::Properties,
        ]
        .iter()
        .map(|p| key("bases-toolbar-button", *p as usize))
        .collect();
        self.toolbar
            .anchors
            .borrow_mut()
            .retain(|k, _| keep.contains(k));
    }

    // ----- open, close, focus -------------------------------------------------

    /// Opens `panel`, or closes it when it was already open.
    fn toggle_panel(
        &mut self,
        panel: Panel,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.toolbar.panel == panel {
            self.close_panel(window, cx);
            return;
        }
        self.forget_panel_anchors();
        self.toolbar.panel = panel;
        self.toolbar.page = Page::List;
        self.toolbar.at = at;
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.header_menu = false;
        self.toolbar.pending_sort = false;
        self.toolbar.pending_rule = None;
        self.toolbar.editing = None;
        self.toolbar.total = None;
        self.toolbar.rows.clear();
        match panel {
            // Observed: these two open with their search field focused; the
            // others leave focus where it was.
            Panel::Views | Panel::Properties => {
                self.open_field(Field::Search, String::new(), window, cx)
            }
            Panel::Results => {
                let total = self.unlimited_rows();
                self.toolbar.total = Some(total);
                self.focus_view(window, cx);
            }
            _ => self.focus_view(window, cx),
        }
        cx.notify();
    }

    /// Closes the open panel and puts focus back on the view.
    /// Opens a panel from somewhere other than its button -- the kanban's
    /// *Edit groups...* opens the Group panel -- under the button's box.
    pub(super) fn show_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        self.forget_panel_anchors();
        self.toolbar.panel = panel;
        self.toolbar.page = Page::List;
        self.toolbar.pinned_at = None;
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.header_menu = false;
        self.toolbar.pending_sort = false;
        self.toolbar.pending_rule = None;
        self.toolbar.editing = None;
        self.toolbar.total = None;
        self.toolbar.rows.clear();
        cx.notify();
    }

    pub(super) fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.forget_panel_anchors();
        self.toolbar.panel = Panel::None;
        self.toolbar.page = Page::List;
        self.toolbar.pinned_at = None;
        self.toolbar.summary = None;
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.header_menu = false;
        self.toolbar.pending_sort = false;
        self.toolbar.pending_rule = None;
        self.toolbar.editing = None;
        self.toolbar.drag = None;
        self.toolbar.field_value.clear();
        self.toolbar.rows.clear();
        self.focus_view(window, cx);
        cx.notify();
    }

    /// Escape: one layer at a time — the menu, then the list, then the panel,
    /// then the search row; with nothing open the key goes on to the shell.
    pub(super) fn close_panel_action(
        &mut self,
        _: &ClosePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.menu.is_some() {
            // The column header's own menu, one layer like the rest.
            self.editing.menu = None;
        } else if self.kanban.menu.is_some() {
            self.close_kanban_menu(cx);
        } else if self.toolbar.header_menu {
            self.toolbar.header_menu = false;
        } else if self.toolbar.combo.is_some() {
            self.close_combo(window, cx);
        } else if self.toolbar.panel != Panel::None {
            self.close_panel(window, cx);
        } else if self.toolbar.search.is_some() {
            self.toolbar.search = None;
            self.toolbar.editing = None;
            self.resolve_search();
        } else {
            cx.propagate();
            return;
        }
        cx.notify();
    }

    /// Up and Down with a searchless list open: the selection moves; with
    /// nothing of the kind open the key goes on to the shell.
    pub(super) fn select_up_action(
        &mut self,
        _: &SelectUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.toolbar.combo.is_some() && self.toolbar.editing != Some(Field::ComboSearch) {
            self.move_selection(-1, cx);
        } else if self.cursor_takes_keys() {
            self.move_cursor(-1, 0, cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn select_down_action(
        &mut self,
        _: &SelectDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.toolbar.combo.is_some() && self.toolbar.editing != Some(Field::ComboSearch) {
            self.move_selection(1, cx);
        } else if self.cursor_takes_keys() {
            self.move_cursor(1, 0, cx);
        } else {
            cx.propagate();
        }
    }

    /// Enter with a searchless list open: picks the selected row.
    pub(super) fn activate_action(
        &mut self,
        _: &Activate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.toolbar.combo.is_some() && self.toolbar.editing != Some(Field::ComboSearch) {
            self.combo_pick_selected(window, cx);
        } else if self.cursor_takes_keys() {
            self.open_focused_cell(window, cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn cursor_left_action(
        &mut self,
        _: &CursorLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.cursor_takes_keys() {
            self.move_cursor(0, -1, cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn cursor_right_action(
        &mut self,
        _: &CursorRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.cursor_takes_keys() {
            self.move_cursor(0, 1, cx);
        } else {
            cx.propagate();
        }
    }

    /// Tab: the next cell, committing an open field first, wrapping to the
    /// next row's first cell.
    pub(super) fn cursor_next_action(
        &mut self,
        _: &CursorNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.cell.is_some() {
            self.commit_cell_edit(cx);
            self.focus_view(window, cx);
            self.step_cursor(1, cx);
        } else if self.cursor_takes_keys() {
            self.step_cursor(1, cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn cursor_prev_action(
        &mut self,
        _: &CursorPrev,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.cell.is_some() {
            self.commit_cell_edit(cx);
            self.focus_view(window, cx);
            self.step_cursor(-1, cx);
        } else if self.cursor_takes_keys() {
            self.step_cursor(-1, cx);
        } else {
            cx.propagate();
        }
    }

    /// Delete or Backspace on the cursor: the editor opens without clearing
    /// (observed, `s25` `deleteOnFocused`: the field opened and the file
    /// did not change).
    pub(super) fn delete_cell_action(
        &mut self,
        _: &DeleteCell,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.cursor_takes_keys() {
            self.open_focused_cell(window, cx);
        } else {
            cx.propagate();
        }
    }

    /// Whether a key goes to the table's cursor: a cell is focused and no
    /// panel, list, menu or field is open over it.
    pub(super) fn cursor_takes_keys(&self) -> bool {
        self.table_state.focused.is_some()
            && self.editing.cell.is_none()
            && self.editing.menu.is_none()
            && self.toolbar.panel == Panel::None
            && self.toolbar.combo.is_none()
            && self.toolbar.editing.is_none()
            && matches!(self.layout(), scoria_bases::Layout::Table(_))
    }

    /// A press that landed outside the open panel. One inside the bar's
    /// buttons or a combobox's trigger is left to that control's own click;
    /// anywhere else closes the outermost open layer.
    pub(super) fn press_out(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .toolbar
            .anchors
            .borrow()
            .values()
            .any(|b| b.contains(&at))
        {
            return;
        }
        if self.toolbar.header_menu {
            self.toolbar.header_menu = false;
            cx.notify();
            return;
        }
        if self.toolbar.combo.is_some() {
            self.close_combo(window, cx);
            return;
        }
        self.close_panel(window, cx);
    }

    pub(super) fn focus_view(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// Opens the views panel on `ix`'s **Configure view** page, which is what
    /// `bases:add-view` and the row's gear do.
    pub(super) fn configure_view(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.toolbar.panel = Panel::Views;
        self.toolbar.page = Page::Configure(ix);
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.header_menu = false;
        let name = self
            .base()
            .and_then(|b| b.views.get(ix))
            .map(|v| v.name.clone())
            .unwrap_or_else(|| DEFAULT_VIEW_NAME.to_string());
        // Observed: the page opens with the name field focused and selected.
        self.request_field(Field::ViewName, name);
        cx.notify();
    }

    /// Asks the next render to open the shared field on `which`, for a
    /// caller that has no window to focus it with.
    pub(super) fn request_field(&mut self, which: Field, text: String) {
        self.toolbar.pending_field = Some((which, text));
    }

    /// The view whose keys the toolbar shows. An empty base has none, and
    /// what it shows is the default view `edit_view` would materialize.
    pub(super) fn showing_view(&self, base: &Base) -> View {
        base.views
            .get(self.view_ix())
            .cloned()
            .unwrap_or_else(default_view)
    }

    /// How many rows the showing view resolves to with no `limit`, which is
    /// the *n* of *Show all*: the resolved table carries only the limited
    /// rows, so the whole count has to be built for it.
    pub(super) fn unlimited_rows(&self) -> usize {
        let Some(base) = self.base() else {
            return 0;
        };
        if self.table().is_some_and(|t| !t.limited) {
            return self.table().map_or(0, |t| t.rows.len());
        }
        let (Some(state), Some(path)) = (self.vault(), self.path()) else {
            return self.table().map_or(0, |t| t.rows.len());
        };
        let mut probe = base.clone();
        let ix = self.view_ix();
        if let Some(view) = probe.views.get_mut(ix) {
            view.limit = None;
        }
        let env = super::rows::env_for(state, path);
        scoria_bases::table::build(&probe, ix, &env).rows.len()
    }

    // ----- the search row -----------------------------------------------------

    /// The *Search* button: shows the row under the bar with its field
    /// focused, or hides it again.
    fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.toolbar.search.is_some() {
            self.toolbar.search = None;
            if self.toolbar.editing == Some(Field::TableSearch) {
                self.toolbar.editing = None;
            }
            self.resolve_search();
            self.focus_view(window, cx);
        } else {
            self.toolbar.search = Some(String::new());
            self.open_field(Field::TableSearch, String::new(), window, cx);
        }
        cx.notify();
    }

    /// `.bases-search-row`: the *Find...* field and *Showing N* beside it.
    fn render_search_row(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = self.toolbar.search.clone()?;
        let showing = self.table().map_or(0, |t| t.rows.len());
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .flex_shrink_0()
                .w_full()
                .gap(px(8.))
                .px(px(8.))
                .py(px(4.))
                // `.bases-search-row`: ruled like the header.
                .border_b_1()
                .border_color(theme.tokens.bases_table_border_color)
                .child(div().flex_1().child(self.text_box(
                    Field::TableSearch,
                    &text,
                    "Find...",
                    theme,
                    cx,
                )))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(FONT_UI_SMALL))
                        .text_color(theme.tokens.text_muted)
                        .child(SharedString::from(format!("Showing {showing}"))),
                )
                .into_any_element(),
        )
    }

    // ----- the shared text field ---------------------------------------------

    /// Puts the shared field on `which`, with `text` in it, and focuses it.
    ///
    /// Nothing is written by installing the text: [`BasesView::commit_field`]
    /// compares before it writes, so the change event `set_text` raises for
    /// the value already in the file is a no-op.
    pub(super) fn open_field(
        &mut self,
        which: Field,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_field(window, cx);
        self.toolbar.editing = Some(which.clone());
        self.toolbar.field_value = text.clone();
        self.toolbar.selected = None;
        if let Some(input) = self.toolbar.field.clone() {
            // Observed: the properties panel's and the Summarize page's
            // search reads "Find or create...", the views panel's "Search...".
            let placeholder = match (&which, self.toolbar.panel) {
                (Field::Search, Panel::Properties | Panel::Summary) => "Find or create...",
                _ => placeholder_of(&which),
            };
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder);
                input.set_height(px(INPUT_HEIGHT));
                input.set_pad_left(px(match which {
                    Field::Search | Field::ComboSearch => 30.,
                    _ => 8.,
                }));
                input.set_text(text, cx);
                input.select_all(cx);
            });
            let handle = input.focus_handle(cx);
            window.focus(&handle, cx);
        }
        // A value field's suggestions hang under a box known only once the
        // field has painted: forget the last one, and ask for that frame.
        if which == Field::Formula {
            self.toolbar.completions_closed = false;
            self.toolbar
                .anchors
                .borrow_mut()
                .remove(&FORMULA_EDITOR_KEY);
            let entity = cx.entity();
            window.on_next_frame(move |_, cx| entity.update(cx, |_, cx| cx.notify()));
        }
        if matches!(which, Field::RuleValue(..)) {
            self.toolbar
                .anchors
                .borrow_mut()
                .remove(&super::toolbar_filter::RULE_VALUE_KEY);
            let entity = cx.entity();
            window.on_next_frame(move |_, cx| entity.update(cx, |_, cx| cx.notify()));
        }
    }

    fn ensure_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.toolbar.field.is_some() {
            return;
        }
        let input = cx.new(|cx| TextInput::new("", cx));
        self.toolbar.subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |this: &mut BasesView,
             input: &Entity<TextInput>,
             event: &InputEvent,
             window: &mut Window,
             cx: &mut Context<BasesView>| {
                let text = input.read(cx).text().to_string();
                match event {
                    InputEvent::Changed => this.commit_field(&text, window, cx),
                    InputEvent::Submit => this.submit_field(&text, window, cx),
                    InputEvent::Cancel => this.cancel_field(window, cx),
                    InputEvent::Up => this.move_selection(-1, cx),
                    InputEvent::Down => this.move_selection(1, cx),
                }
            },
        )];
        self.toolbar.field = Some(input);
    }

    pub(super) fn field_text(&self, cx: &Context<Self>) -> String {
        self.toolbar
            .field
            .as_ref()
            .map(|f| f.read(cx).text().to_string())
            .unwrap_or_default()
    }

    /// What the shared field writes, by what it is standing in for.
    ///
    /// Every arm compares before it writes: the field raises a change event
    /// for the text it is opened with too, and a base must not be written
    /// because a panel was opened.
    fn commit_field(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.toolbar.field_value = text.to_string();
        let Some(which) = self.toolbar.editing.clone() else {
            return;
        };
        match which {
            // The search fields only narrow their lists.
            Field::Search | Field::ComboSearch => {
                self.toolbar.selected = None;
                cx.notify();
            }
            Field::TableSearch => {
                self.toolbar.search = Some(text.to_string());
                self.resolve_search();
                cx.notify();
            }
            Field::Limit => self.commit_limit(text, cx),
            Field::ViewName => self.commit_view_name(text, cx),
            Field::Separator => self.commit_separator(text, cx),
            Field::DisplayName => self.commit_display_name(text, cx),
            Field::Formula => {
                self.toolbar.completions_closed = false;
                self.toolbar.selected = None;
                self.commit_formula(text, cx)
            }
            Field::SummaryName => self.commit_summary_name(text, cx),
            Field::Rule(section, path) => self.commit_rule_text(section, &path, text, cx),
            Field::RuleValue(section, path) => self.commit_rule_value(section, &path, text, cx),
            Field::GroupAdd => cx.notify(),
        }
        let _ = window;
    }

    /// Enter in the field: activates the selected row of a list, or settles
    /// the field it is in.
    fn submit_field(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(which) = self.toolbar.editing.clone() else {
            return;
        };
        // Enter on an arrowed-to completion takes it.
        if which == Field::Formula {
            if let Some(ix) = self.toolbar.selected {
                if self.accept_completion(ix, cx) {
                    return;
                }
            }
        }
        // Enter on an arrowed-to value suggestion takes it.
        if matches!(which, Field::RuleValue(..)) {
            let pick = self
                .toolbar
                .selected
                .and_then(|ix| self.rule_value_suggestions(cx).get(ix).cloned());
            if let Some(pick) = pick {
                self.pick_rule_value(&pick, window, cx);
                return;
            }
        }
        match which {
            Field::Search => self.activate_selected(window, cx),
            Field::ComboSearch => self.combo_pick_selected(window, cx),
            Field::GroupAdd => self.commit_group_add(text, cx),
            Field::RuleValue(section, path) => {
                // A list row takes the typed item as a pill on Enter; the
                // other rows were written as they were typed.
                self.submit_rule_value(section, &path, text, cx);
                self.toolbar.editing = None;
                self.focus_view(window, cx);
                cx.notify();
            }
            Field::Rule(..) => {
                self.commit_field(text, window, cx);
                self.toolbar.editing = None;
                self.focus_view(window, cx);
                cx.notify();
            }
            Field::TableSearch => {}
            _ => {
                self.commit_field(text, window, cx);
                self.toolbar.editing = None;
                self.focus_view(window, cx);
                cx.notify();
            }
        }
    }

    /// Escape in the field. Observed: in a panel's search field it closes
    /// the panel; in the search row it clears and hides the row; in a list's
    /// search field it closes the list.
    fn cancel_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Observed (`bases-leftovers` `s03`): Escape closes the editor's
        // completion list before it does anything else.
        if self.toolbar.editing == Some(Field::Formula)
            && !self.formula_completions(cx).1.is_empty()
        {
            self.toolbar.completions_closed = true;
            self.toolbar.selected = None;
            cx.notify();
            return;
        }
        match self.toolbar.editing.clone() {
            Some(Field::TableSearch) => {
                self.toolbar.search = None;
                self.toolbar.editing = None;
                self.resolve_search();
                self.focus_view(window, cx);
            }
            Some(Field::ComboSearch) => self.close_combo(window, cx),
            Some(Field::Rule(..)) | Some(Field::RuleValue(..)) => {
                // The value typed so far is already in the file; Escape only
                // leaves the field.
                self.toolbar.editing = None;
                self.focus_view(window, cx);
            }
            // Observed (`bases-leftovers` `s03`): Escape on the Add summary
            // page goes back to the Summarize list, the summary kept.
            Some(_) if matches!(self.toolbar.page, Page::NewSummary { .. }) => {
                self.page_back(window, cx);
                return;
            }
            Some(_) if self.toolbar.header_menu => self.toolbar.header_menu = false,
            Some(_) => self.close_panel(window, cx),
            None if self.table_state.focused.is_some() => self.table_state.focused = None,
            None => {}
        }
        cx.notify();
    }

    /// Down and Up in a list's search field.
    fn move_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let count = self.list_len(cx);
        if count == 0 {
            return;
        }
        let next = match self.toolbar.selected {
            None if delta > 0 => 0,
            None => count - 1,
            Some(ix) => {
                let next = ix as i64 + delta as i64;
                next.rem_euclid(count as i64) as usize
            }
        };
        self.toolbar.selected = Some(next);
        cx.notify();
    }

    /// How many rows the open list has, which is what the arrows cycle
    /// over.
    fn list_len(&self, cx: &Context<Self>) -> usize {
        if self.toolbar.combo.is_some() {
            return self.combo_list_len(cx);
        }
        if matches!(self.toolbar.editing, Some(Field::RuleValue(..))) {
            return self.rule_value_suggestions(cx).len();
        }
        if self.toolbar.editing == Some(Field::Formula) {
            return self.formula_completions(cx).1.len();
        }
        match (self.toolbar.panel, &self.toolbar.page) {
            (Panel::Views, Page::List) => self.views_list_len(cx),
            (Panel::Properties, Page::List) => self.properties_list_len(cx),
            (Panel::Summary, Page::List) => self.summary_list_len(cx),
            _ => 0,
        }
    }

    /// Enter in a list's search field.
    fn activate_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.toolbar.panel, &self.toolbar.page) {
            (Panel::Views, Page::List) => self.views_activate_selected(window, cx),
            (Panel::Properties, Page::List) => self.properties_activate_selected(window, cx),
            (Panel::Summary, Page::List) => self.summary_activate_selected(window, cx),
            _ => {}
        }
    }

    // ----- the panel box ------------------------------------------------------

    /// The open panel, hung under its button.
    fn render_panel(
        &mut self,
        base: &Base,
        view: &View,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.toolbar.rows.clear();
        let panel = self.toolbar.panel;
        let width = match panel {
            Panel::None => return None,
            Panel::Filter => FILTER_PANEL_WIDTH,
            Panel::Sort | Panel::Group => 240.,
            _ => PANEL_WIDTH,
        };
        let at = self.toolbar.pinned_at.unwrap_or_else(|| {
            self.anchor(key("bases-toolbar-button", panel as usize))
                .map(|b| Point::new(b.origin.x, b.origin.y + b.size.height + px(2.)))
                .unwrap_or(self.toolbar.at)
        });
        // A panel is as tall as the window allows, not as the room under its
        // anchor does: one hung from a summary row near the bottom slides up
        // to fit (`snap_to_window` below), which is what Obsidian's menu did
        // (`bases-leftovers` `s03`: moved to y 10 from a footer at y 403).
        let tallest = (window.viewport_size().height - px(16.)).max(px(120.));
        let body = match panel {
            Panel::None => return None,
            Panel::Views => self.render_views_panel(base, theme, cx),
            Panel::Results => self.render_results_panel(view, theme, cx),
            Panel::Sort => self.render_sort_panel(base, view, theme, cx),
            Panel::Group => self.render_group_panel(base, view, theme, cx),
            Panel::Filter => self.render_filter_panel(base, view, theme, cx),
            Panel::Properties => self.render_properties_panel(base, view, theme, cx),
            Panel::Summary => self.render_summary_panel(base, view, theme, cx),
        };
        let completions = self.render_formula_completions(theme, cx);
        let boxed = div()
            .id(("bases-toolbar-panel", panel as usize))
            .flex()
            .flex_col()
            .w(px(width))
            .max_h(tallest)
            .overflow_y_scroll()
            // Every panel is a `.menu.bases-toolbar-menu`.
            .bg(theme.tokens.menu_background)
            .border_1()
            .border_color(theme.tokens.menu_border_color)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                this.press_out(event.position, window, cx)
            }))
            .child(body)
            .children(completions);
        Some(
            deferred(
                anchored()
                    .position(at)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(boxed),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    // ----- the widgets --------------------------------------------------------

    /// `.search-input-container`: the field with a magnifier at its left,
    /// under a hairline; `which` says which list it narrows.
    pub(super) fn search_box(
        &self,
        which: Field,
        placeholder: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = match &self.toolbar.editing {
            Some(w) if *w == which => self.field_text(cx),
            _ => String::new(),
        };
        div()
            .relative()
            .mt(px(4.))
            .pb(px(2.))
            .border_b_1()
            .border_color(theme.tokens.background_modifier_border)
            .child(self.text_box(which, &text, placeholder, theme, cx))
            // `.bases-toolbar-menu-container .search-input-container` sets
            // `--search-icon-color` to the faint colour.
            .child(div().absolute().left(px(8.)).top(px(6.)).child(icon(
                "search",
                ICON_SIZE,
                theme.tokens.text_faint,
            )))
            .into_any_element()
    }

    /// A text input: the shared [`TextInput`] when it is standing in for
    /// `which`, and the value as a box that opens it otherwise.
    pub(super) fn text_box(
        &self,
        which: Field,
        value: &str,
        placeholder: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.toolbar.editing.as_ref() == Some(&which) {
            if let Some(input) = self.toolbar.field.clone() {
                return div()
                    .id(("bases-toolbar-field", which.element_ix()))
                    .w_full()
                    // A click inside the open field is not a click away from
                    // the panel, which is what the root would read it as.
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(input)
                    .into_any_element();
            }
        }
        let text = value.to_string();
        let shown = if text.is_empty() {
            placeholder.to_string()
        } else {
            text.clone()
        };
        let pad_left = match which {
            Field::Search | Field::ComboSearch => 30.,
            _ => 8.,
        };
        div()
            .id(("bases-toolbar-field-shut", which.element_ix()))
            .w_full()
            .h(px(INPUT_HEIGHT))
            .pl(px(pad_left))
            .pr(px(8.))
            .flex()
            .items_center()
            .rounded_md()
            // A form field; the table's own search row
            // (`.bases-search-row`) makes the field transparent and
            // borderless.
            .when(which != Field::TableSearch, |d| {
                d.bg(theme.tokens.background_modifier_form_field)
                    .border_1()
                    .border_color(theme.tokens.background_modifier_border)
            })
            .cursor_text()
            .text_size(px(FONT_UI_SMALL))
            .text_color(if text.is_empty() {
                theme.tokens.text_faint
            } else {
                theme.tokens.text_normal
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

    /// `.bases-toolbar-menu-container-header`: the back button with the
    /// page's title, then the page's own `more-vertical` button when it has
    /// a menu, then the close button.
    pub(super) fn page_header(
        &self,
        title: SharedString,
        menu: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .p(px(4.))
            .child(
                div()
                    .id("bases-page-back")
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .min_w(px(0.))
                    .h(px(ROW_HEIGHT))
                    .p(px(4.))
                    .gap(px(4.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.page_back(window, cx);
                    }))
                    .child(icon("chevron-left", ICON_SIZE, theme.tokens.text_normal))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(title),
                    ),
            )
            .when(menu, |d| {
                d.child(
                    div()
                        .id("bases-page-menu")
                        .relative()
                        .w(px(28.))
                        .h(px(ROW_HEIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .when(self.toolbar.header_menu, |d| {
                            d.bg(theme.tokens.background_modifier_hover)
                        })
                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                        .child(self.measure(key("bases-page-menu", 0)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.toolbar.header_menu = !this.toolbar.header_menu;
                            cx.notify();
                        }))
                        .child(icon(
                            "ellipsis-vertical",
                            ICON_SIZE,
                            theme.tokens.text_muted,
                        )),
                )
            })
            .child(
                div()
                    .id("bases-page-close")
                    .w(px(24.))
                    .h(px(ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_panel(window, cx);
                    }))
                    .child(icon("x", ICON_SIZE, theme.tokens.text_muted)),
            )
            .into_any_element()
    }

    /// The back button of a page: the list again.
    fn page_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.forget_panel_anchors();
        self.toolbar.page = Page::List;
        self.toolbar.combo = None;
        self.toolbar.header_menu = false;
        self.toolbar.selected = None;
        match self.toolbar.panel {
            // Observed: leaving the Add summary page lands on the Summarize
            // list, its search field ready, as the other lists' do.
            Panel::Views | Panel::Properties | Panel::Summary => {
                self.open_field(Field::Search, String::new(), window, cx)
            }
            _ => {
                self.toolbar.editing = None;
                self.focus_view(window, cx);
            }
        }
        cx.notify();
    }

    /// `.input-row`: a label over its control, the shape every row of a
    /// page takes. A row with no label is the control alone.
    pub(super) fn input_row(
        &self,
        label: Option<&'static str>,
        control: AnyElement,
        theme: &Theme,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .children(label.map(|label| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(ROW_HEIGHT))
                    .px(px(2.))
                    .py(px(4.))
                    .gap(px(2.))
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.tokens.text_normal)
                    .child(SharedString::from(label))
            }))
            .child(div().w_full().child(control))
            .into_any_element()
    }

    /// The form of a page: rows 8 px apart under `4px 8px 8px` of padding.
    pub(super) fn page_form(&self, rows: Vec<AnyElement>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .pt(px(4.))
            .px(px(8.))
            .pb(px(8.))
            .gap(px(8.))
            .children(rows)
            .into_any_element()
    }

    /// `.suggestion-group`: a padded run of rows; `bordered` puts the
    /// hairline above it that the actions group carries.
    pub(super) fn group(&self, rows: Vec<AnyElement>, bordered: bool, theme: &Theme) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .p(px(6.))
            .when(bordered, |d| {
                d.border_t_1()
                    .border_color(theme.tokens.background_modifier_border)
            })
            .children(rows)
            .into_any_element()
    }

    /// `.bases-toolbar-section-header`: a 13 px medium heading padded
    /// `8px 12px`, with an optional fold triangle before it and a count
    /// flair after it.
    pub(super) fn section_header(
        &self,
        title: &'static str,
        flair: Option<usize>,
        fold: Option<(bool, Section)>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut row = div()
            .id(("bases-section-header", hash_key(title, 0)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px(px(12.))
            .py(px(8.))
            .gap(px(4.))
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .font_weight(gpui::FontWeight::MEDIUM);
        if let Some((collapsed, section)) = fold {
            row = row
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toolbar.collapsed[section.ix()] = !this.toolbar.collapsed[section.ix()];
                    cx.notify();
                }))
                .child(icon(
                    if collapsed {
                        "chevron-right"
                    } else {
                        "chevron-down"
                    },
                    14.,
                    theme.tokens.nav_collapse_icon_color,
                ));
        }
        row = row.child(SharedString::from(title));
        if let Some(n) = flair {
            row = row.child(
                div()
                    .ml(px(4.))
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(theme.tokens.background_modifier_hover)
                    .text_size(px(10.))
                    .text_color(theme.tokens.text_muted)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(SharedString::from(n.to_string())),
            );
        }
        row.into_any_element()
    }

    /// `.bases-toolbar-menu-action`: a row of an actions group — an icon and
    /// a label, 25 px, padded `4px 4px 4px 6px`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn action_row(
        &self,
        prefix: &'static str,
        ix: usize,
        icon_name: &'static str,
        label: SharedString,
        enabled: bool,
        theme: &Theme,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        let color = if enabled {
            theme.tokens.text_normal
        } else {
            theme.tokens.text_faint
        };
        let icon_color = if enabled {
            theme.tokens.text_muted
        } else {
            theme.tokens.text_faint
        };
        div()
            .id((prefix, ix))
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
            .text_color(color)
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            })
            .when(enabled, |d| d.on_click(on_click))
            .child(icon(icon_name, ICON_SIZE, icon_color))
            .child(div().whitespace_nowrap().child(label))
            .into_any_element()
    }

    /// `.clickable-icon`: a 28 by 24 icon button.
    pub(super) fn clickable_icon(
        &self,
        prefix: &'static str,
        ix: usize,
        icon_name: &'static str,
        color: gpui::Hsla,
        theme: &Theme,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> AnyElement {
        div()
            .id((prefix, ix))
            .w(px(28.))
            .h(px(24.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            .on_click(on_click)
            .child(icon(icon_name, ICON_SIZE, color))
            .into_any_element()
    }

    /// The `grip-vertical` handle of a reorderable row: a press starts a
    /// drag of row `ix` of `list`, and the rows under the pointer take it.
    pub(super) fn grip(
        &self,
        list: &'static str,
        ix: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(("bases-grip", hash_key(list, ix)))
            .size(px(16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .cursor_grab()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toolbar.drag = Some((list, ix));
                    cx.notify();
                }),
            )
            .child(icon("grip-vertical", ICON_SIZE, theme.tokens.text_faint))
            .into_any_element()
    }

    /// Wraps a reorderable row: while a grip of `list` is held, the pointer
    /// entering this row moves the dragged row here, and letting go ends the
    /// drag.
    pub(super) fn drop_target<T: InteractiveElement + IntoElement>(
        &self,
        list: &'static str,
        ix: usize,
        row: T,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        row.on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
            let Some((from_list, from)) = this.toolbar.drag else {
                return;
            };
            if from_list != list || from == ix || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            this.toolbar.drag = Some((list, ix));
            this.reorder(list, from, ix, cx);
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                if this.toolbar.drag.take().is_some() {
                    cx.notify();
                }
            }),
        )
        .into_any_element()
    }

    /// A row of `list` moved from `from` to `to`.
    fn reorder(&mut self, list: &'static str, from: usize, to: usize, cx: &mut Context<Self>) {
        match list {
            "sort" => self.edit_view(
                move |view| {
                    if from < view.sort.len() && to < view.sort.len() {
                        let entry = view.sort.remove(from);
                        view.sort.insert(to, entry);
                    }
                },
                cx,
            ),
            "groups" => self.reorder_group(from, to, cx),
            _ => {}
        }
    }

    /// A 16 px checkbox: the accent when checked, with a white check.
    pub(super) fn check_box(&self, checked: bool, theme: &Theme) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .size(px(16.))
            .rounded(px(4.))
            .border_1()
            .border_color(if checked {
                theme.tokens.checkbox_color
            } else {
                theme.tokens.checkbox_border_color
            })
            .when(checked, |d| {
                d.bg(theme.tokens.checkbox_color).child(icon(
                    "check",
                    12.,
                    theme.tokens.checkbox_marker_color,
                ))
            })
    }

    /// `input[type=range]`: a 4 px track with the accent up to the value and
    /// a 16 px thumb, the value's text before it. A press or a drag along the
    /// track picks the step under the pointer.
    pub(super) fn slider(
        &self,
        name: &'static str,
        value: f64,
        range: (f64, f64, f64),
        decimals: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (min, max, _) = range;
        let snapped = layout::snap(value, range);
        let fraction = ((snapped - min) / (max - min)).clamp(0., 1.) as f32;
        let k = key("bases-slider", hash_key(name, 0));
        let pick = move |this: &mut BasesView, x: Pixels, cx: &mut Context<BasesView>| {
            let Some(bounds) = this.anchor(k) else {
                return;
            };
            let width = bounds.size.width.as_f32().max(1.);
            let f = ((x - bounds.origin.x).as_f32() / width).clamp(0., 1.) as f64;
            let picked = layout::snap(min + f * (max - min), range);
            this.slider_pick(name, picked, cx);
        };
        let pick_move = pick;
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(INPUT_HEIGHT))
            .gap(px(8.))
            .child(
                div()
                    .flex_shrink_0()
                    .min_w(px(28.))
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(theme.tokens.text_normal)
                    .child(SharedString::from(format!("{snapped:.decimals$}"))),
            )
            .child(
                div()
                    .id(("bases-slider", hash_key(name, 0)))
                    .relative()
                    .flex_1()
                    .h(px(16.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(self.measure(k))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.toolbar.drag = Some((name, usize::MAX));
                            pick(this, event.position.x, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        if this.toolbar.drag == Some((name, usize::MAX))
                            && event.pressed_button == Some(MouseButton::Left)
                        {
                            pick_move(this, event.position.x, cx);
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if this.toolbar.drag == Some((name, usize::MAX)) {
                                this.toolbar.drag = None;
                                cx.notify();
                            }
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(6.))
                            .h(px(4.))
                            .rounded(px(2.))
                            .bg(theme.tokens.slider_track_background),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(6.))
                            .h(px(4.))
                            .w(gpui::relative(fraction))
                            .rounded(px(2.))
                            .bg(theme.tokens.slider_fill_background),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left(gpui::relative(fraction))
                            .ml(px(-8.))
                            .size(px(16.))
                            .rounded_full()
                            .bg(gpui::white())
                            .shadow_sm(),
                    ),
            )
            .into_any_element()
    }

    /// Every slider's write, by the key it writes.
    fn slider_pick(&mut self, name: &'static str, value: f64, cx: &mut Context<Self>) {
        let now = self.base().map(|b| self.showing_view(b));
        let Some(view) = now else {
            return;
        };
        let unchanged = match (name, layout::Layout::of(&view)) {
            ("cardSize", layout::Layout::Cards(c)) => c.card_size == value,
            ("imageAspectRatio", layout::Layout::Cards(c)) => c.image_aspect_ratio == value,
            ("imageAspectRatio", layout::Layout::Kanban(k)) => k.image_aspect_ratio == value,
            ("columnWidth", layout::Layout::Kanban(k)) => k.column_width == value,
            _ => false,
        };
        if unchanged {
            return;
        }
        match name {
            "cardSize" => self.edit_view(move |view| layout::set_card_size(view, value), cx),
            "columnWidth" => self.edit_view(move |view| layout::set_column_width(view, value), cx),
            "imageAspectRatio" => {
                self.edit_view(move |view| layout::set_image_aspect_ratio(view, value), cx)
            }
            _ => {}
        }
    }

    // ----- comboboxes ---------------------------------------------------------

    /// A `.combobox-button` and, while it is open, its
    /// `suggestion-container`: a search field over rows of icon, label and
    /// aux text, the picked one marked with a check.
    pub(super) fn combobox(
        &self,
        spec: ComboSpec,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let combo = Combo {
            name: spec.name,
            ix: spec.ix,
        };
        let open = self.toolbar.combo == Some(combo);
        let searchable = spec.searchable;
        let k = key("bases-combo", hash_key(spec.name, spec.ix));
        let label_color = if spec.shown.is_some() {
            theme.tokens.text_normal
        } else {
            theme.tokens.text_faint
        };
        let mut trigger = div()
            .id(("bases-combobox", hash_key(spec.name, spec.ix)))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .h(px(INPUT_HEIGHT))
            .min_w(px(0.))
            .pl(px(8.))
            .pr(px(6.))
            .gap(px(4.))
            .cursor_pointer()
            .text_size(px(FONT_UI_SMALL))
            .text_color(label_color)
            .whitespace_nowrap()
            .overflow_hidden()
            .child(self.measure(k))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                if this.toolbar.combo == Some(combo) {
                    this.close_combo(window, cx);
                } else {
                    this.open_combo(combo, searchable, window, cx);
                }
            }));
        if spec.flat {
            trigger = trigger.when(open, |d| d.bg(theme.tokens.background_modifier_hover));
        } else {
            // `.combobox-button`: a form field.
            trigger = trigger
                .w_full()
                .rounded(px(5.))
                .bg(theme.tokens.background_modifier_form_field)
                .border_1()
                .border_color(theme.tokens.background_modifier_border);
        }
        if let Some(name) = spec.icon {
            trigger = trigger.child(icon(name, ICON_SIZE, theme.tokens.text_muted));
        }
        trigger = trigger.child(
            div()
                .min_w(px(0.))
                .when(!spec.flat, |d| d.flex_1())
                .overflow_hidden()
                .text_ellipsis()
                .child(
                    spec.shown
                        .clone()
                        .unwrap_or_else(|| SharedString::from(spec.placeholder)),
                ),
        );
        if spec.clearable && spec.shown.is_some() {
            let (name, ix) = (spec.name, spec.ix);
            trigger = trigger.child(
                div()
                    .id(("bases-combobox-clear", hash_key(spec.name, spec.ix)))
                    .flex_shrink_0()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.combo_clear(name, ix, window, cx);
                    }))
                    .child(icon("x", 14., theme.tokens.text_muted)),
            );
        }
        trigger = trigger.child(icon("chevrons-up-down", 14., theme.tokens.text_muted));
        let mut wrap = div()
            .relative()
            .min_w(px(0.))
            .when(!spec.flat, |d| d.w_full())
            .child(trigger);
        if open {
            wrap = wrap.child(self.render_combo_list(&spec, k, theme, cx));
        }
        wrap.into_any_element()
    }

    /// The open list of a combobox, hung under its trigger.
    fn render_combo_list(
        &self,
        spec: &ComboSpec,
        trigger: u64,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let at = self
            .anchor(trigger)
            .map(|b| Point::new(b.origin.x, b.origin.y + b.size.height + px(4.)))
            .unwrap_or_default();
        let query = match &self.toolbar.editing {
            Some(Field::ComboSearch) => self.field_text(cx).trim().to_ascii_lowercase(),
            _ => String::new(),
        };
        let shown: Vec<(usize, &ComboOption)> = spec
            .options
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                query.is_empty()
                    || o.label.to_ascii_lowercase().contains(&query)
                    || o.aux
                        .as_ref()
                        .is_some_and(|a| a.to_ascii_lowercase().contains(&query))
            })
            .collect();
        let selected_row = self.toolbar.selected;
        let list_key = key("bases-combo-list", hash_key(spec.name, spec.ix));
        let (name, ix) = (spec.name, spec.ix);
        let mut list = div()
            .id(("bases-combobox-list", hash_key(spec.name, spec.ix)))
            .relative()
            .flex()
            .flex_col()
            .w(px(COMBO_WIDTH))
            .max_h(px(300.))
            // The list is a `.suggestion-container.combobox`.
            .bg(theme.tokens.suggestion_background)
            .border_1()
            .border_color(theme.tokens.background_modifier_border)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_normal)
            .occlude()
            .child(self.measure(list_key));
        if spec.searchable {
            list = list.child(div().px(px(0.)).pt(px(2.)).child(self.search_box(
                Field::ComboSearch,
                "Search...",
                theme,
                cx,
            )));
        }
        let rows: Vec<AnyElement> = shown
            .iter()
            .enumerate()
            .map(|(row, (opt, option))| {
                let opt = *opt;
                let is_pick = spec.selected == Some(opt);
                let is_selected = selected_row == Some(row);
                div()
                    .id((
                        "bases-combobox-option",
                        hash_key(spec.name, spec.ix) * 512 + opt,
                    ))
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(29.))
                    .px(px(8.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(is_selected, |d| {
                        d.bg(theme.tokens.background_modifier_hover)
                    })
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.combo_pick(name, ix, opt, window, cx);
                    }))
                    .children(
                        option
                            .icon
                            .map(|n| icon(n, ICON_SIZE, theme.tokens.text_muted)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(option.label.clone()),
                    )
                    .children(option.aux.clone().map(|aux| {
                        div()
                            .flex_shrink_0()
                            .text_size(px(FONT_UI_SMALLER))
                            .text_color(theme.tokens.text_faint)
                            .child(aux)
                    }))
                    .when(is_pick, |d| {
                        d.child(icon("check", 14., theme.tokens.text_muted))
                    })
                    .into_any_element()
            })
            .collect();
        list = list.child(
            div()
                .id(("bases-combobox-rows", hash_key(spec.name, spec.ix)))
                .flex()
                .flex_col()
                .p(px(6.))
                .overflow_y_scroll()
                .children(rows),
        );
        deferred(
            anchored()
                .position(at)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.))
                .child(list),
        )
        .with_priority(2)
        .into_any_element()
    }

    /// Opens a combobox's list. A searchable one takes focus into its
    /// field; a searchless one leaves focus on the view, where the arrows
    /// and Enter are the view's own actions.
    pub(super) fn open_combo(
        &mut self,
        combo: Combo,
        searchable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toolbar.combo = Some(combo);
        self.toolbar.header_menu = false;
        self.toolbar.selected = None;
        if searchable {
            self.open_field(Field::ComboSearch, String::new(), window, cx);
        } else {
            if self.toolbar.editing == Some(Field::ComboSearch) {
                self.toolbar.editing = None;
            }
            self.focus_view(window, cx);
        }
        cx.notify();
    }

    pub(super) fn close_combo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.forget_panel_anchors();
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        if self.toolbar.editing == Some(Field::ComboSearch) {
            self.toolbar.editing = None;
        }
        // A list that opened for a row still waiting on its property closes
        // with the row: a pending entry is never written.
        self.toolbar.pending_sort = false;
        self.toolbar.pending_rule = None;
        self.focus_view(window, cx);
        cx.notify();
    }

    /// The options of the open combobox, as the owner panel built them.
    fn combo_options(&self, cx: &Context<Self>) -> Vec<ComboOption> {
        let Some(combo) = self.toolbar.combo else {
            return Vec::new();
        };
        let Some(base) = self.base().cloned() else {
            return Vec::new();
        };
        let view = self.showing_view(&base);
        self.options_of(combo, &base, &view, cx)
    }

    fn combo_shown_indices(&self, cx: &Context<Self>) -> Vec<usize> {
        let query = match &self.toolbar.editing {
            Some(Field::ComboSearch) => self.field_text(cx).trim().to_ascii_lowercase(),
            _ => String::new(),
        };
        self.combo_options(cx)
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                query.is_empty()
                    || o.label.to_ascii_lowercase().contains(&query)
                    || o.aux
                        .as_ref()
                        .is_some_and(|a| a.to_ascii_lowercase().contains(&query))
            })
            .map(|(ix, _)| ix)
            .collect()
    }

    fn combo_list_len(&self, cx: &Context<Self>) -> usize {
        self.combo_shown_indices(cx).len()
    }

    /// Enter in a list's search field: the selected row, else the first.
    fn combo_pick_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(combo) = self.toolbar.combo else {
            return;
        };
        let shown = self.combo_shown_indices(cx);
        let row = self.toolbar.selected.unwrap_or(0);
        if let Some(opt) = shown.get(row).copied() {
            self.combo_pick(combo.name, combo.ix, opt, window, cx);
        }
    }

    /// A pick in a combobox's list, routed to the panel that owns it.
    fn combo_pick(
        &mut self,
        name: &'static str,
        ix: usize,
        opt: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        if self.toolbar.editing == Some(Field::ComboSearch) {
            self.toolbar.editing = None;
        }
        self.focus_view(window, cx);
        self.pick(name, ix, opt, window, cx);
        cx.notify();
    }

    /// The clear button of a clearable combobox.
    fn combo_clear(
        &mut self,
        name: &'static str,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear(name, ix, window, cx);
        cx.notify();
    }

    // ----- the property set ---------------------------------------------------

    /// Every property the view knows, with its label, kind and icon.
    ///
    /// The `file.*` set is in the order the sort list offers it (observed);
    /// then the base's formulas in file order; then the note properties in
    /// `types.json`'s order followed by the rest as the rows carry them;
    /// then anything the view's `order` names that none of those reached.
    pub(super) fn properties_of(&self, base: &Base, view: &View) -> Vec<Prop> {
        let mut out: Vec<Prop> = Vec::new();
        fn push(out: &mut Vec<Prop>, p: Prop) {
            if !out
                .iter()
                .any(|seen| super::writes::same_property(&seen.id, &p.id))
            {
                out.push(p);
            }
        }
        for (id, kind) in FILE_PROPERTIES {
            push(&mut out, self.prop(base, id, *kind));
        }
        for name in base.formulas.keys() {
            push(
                &mut out,
                self.prop(base, &format!("formula.{name}"), PropertyKind::Formula),
            );
        }
        let mut note_names: Vec<String> = Vec::new();
        if let Some(state) = self.vault() {
            for name in state.property_types().types.keys() {
                note_names.push(name.clone());
            }
        }
        for (name, _) in &self.property_kinds {
            if !note_names.iter().any(|n| n == name) {
                note_names.push(name.clone());
            }
        }
        for name in note_names {
            let kind = self.note_kind(&name);
            push(&mut out, self.prop(base, &format!("note.{name}"), kind));
        }
        for id in view.order.iter().flatten() {
            let qualified = layout::qualify(id);
            let kind = match qualified.strip_prefix("note.") {
                Some(name) => self.note_kind(name),
                None => PropertyKind::Unknown,
            };
            push(&mut out, self.prop(base, &qualified, kind));
        }
        out
    }

    fn prop(&self, base: &Base, id: &str, kind: PropertyKind) -> Prop {
        let display = scoria_bases::table::column(base, id).display;
        Prop {
            id: id.to_string(),
            label: display,
            kind,
        }
    }

    /// A note property's kind: `types.json` first, then what its values
    /// are, the way Obsidian infers a type for a property nobody assigned.
    pub(super) fn note_kind(&self, name: &str) -> PropertyKind {
        if let Some(state) = self.vault() {
            if let Some(ty) = state
                .property_types()
                .types
                .get(name)
                .and_then(|t| PropertyType::parse(t))
            {
                return kind_of_type(ty);
            }
        }
        match name {
            "tags" => return PropertyKind::Tags,
            "aliases" | "cssclasses" => return PropertyKind::List,
            _ => {}
        }
        self.property_kinds
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, k)| *k)
            .unwrap_or(PropertyKind::Text)
    }

    /// The kind of any property id the panels name.
    pub(super) fn kind_of(&self, base: &Base, view: &View, id: &str) -> PropertyKind {
        let qualified = layout::qualify(id);
        if qualified == "file" {
            return PropertyKind::File;
        }
        self.properties_of(base, view)
            .into_iter()
            .find(|p| super::writes::same_property(&p.id, &qualified))
            .map(|p| p.kind)
            .unwrap_or(PropertyKind::Unknown)
    }

    /// The id of the `n`th property of the pickers, which every one of them
    /// is built from.
    pub(super) fn property_id(&self, n: usize) -> Option<String> {
        let base = self.base()?;
        let view = self.showing_view(base);
        self.properties_of(base, &view).get(n).map(|p| p.id.clone())
    }

    /// The property rows of a picker: one option per property, labelled and
    /// iconed, with the id as the aux text where it differs from the label.
    pub(super) fn property_options(&self, props: &[Prop]) -> Vec<ComboOption> {
        props
            .iter()
            .map(|p| ComboOption {
                icon: Some(p.icon()),
                label: SharedString::from(p.label.clone()),
                aux: (p.label != p.id && !p.id.starts_with("note.") || p.id.starts_with("file."))
                    .then(|| SharedString::from(p.id.clone())),
            })
            .collect()
    }
}

/// One property as the panels list it.
#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub id: String,
    pub label: String,
    pub kind: PropertyKind,
}

impl Prop {
    /// The icon the sort and filter lists put before the property: by its
    /// kind, with `file.*` properties by what they hold.
    pub fn icon(&self) -> &'static str {
        match self.id.as_str() {
            "file.ctime" | "file.mtime" => "clock",
            "file.size" => "binary",
            "file.links" | "file.embeds" => "list",
            "file.tags" => "tags",
            id if id.starts_with("file.") => "text",
            "note.aliases" => "forward",
            _ => kind_icon(self.kind),
        }
    }

    /// The icon the properties panel puts before the property, where every
    /// `file.*` property carries `info`.
    pub fn panel_icon(&self) -> &'static str {
        if self.id.starts_with("file.") {
            "info"
        } else {
            self.icon()
        }
    }
}

/// The `lucide-*` icon of a property kind.
pub fn kind_icon(kind: PropertyKind) -> &'static str {
    match kind {
        PropertyKind::File => "file",
        PropertyKind::Text => "text",
        PropertyKind::Number => "binary",
        PropertyKind::Checkbox => "square-check",
        PropertyKind::Date => "calendar",
        PropertyKind::DateTime => "clock",
        PropertyKind::List => "list",
        PropertyKind::Tags => "tags",
        PropertyKind::Formula => "square-function",
        PropertyKind::Unknown => "text",
    }
}

/// A `types.json` type as a kind.
pub fn kind_of_type(ty: PropertyType) -> PropertyKind {
    match ty {
        PropertyType::Text => PropertyKind::Text,
        PropertyType::MultiText | PropertyType::Aliases => PropertyKind::List,
        PropertyType::Number => PropertyKind::Number,
        PropertyType::Checkbox => PropertyKind::Checkbox,
        PropertyType::Date => PropertyKind::Date,
        PropertyType::DateTime => PropertyKind::DateTime,
        PropertyType::Tags => PropertyKind::Tags,
    }
}

/// The twelve `file.*` properties, in the order the sort list offers them,
/// with the kind each one sorts and filters as.
pub const FILE_PROPERTIES: &[(&str, PropertyKind)] = &[
    ("file.name", PropertyKind::Text),
    ("file.basename", PropertyKind::Text),
    ("file.fullname", PropertyKind::Text),
    ("file.path", PropertyKind::Text),
    ("file.folder", PropertyKind::Text),
    ("file.ext", PropertyKind::Text),
    ("file.ctime", PropertyKind::DateTime),
    ("file.mtime", PropertyKind::DateTime),
    ("file.size", PropertyKind::Number),
    ("file.links", PropertyKind::List),
    ("file.embeds", PropertyKind::List),
    ("file.tags", PropertyKind::Tags),
];

/// One option of a combobox's list.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboOption {
    pub icon: Option<&'static str>,
    pub label: SharedString,
    pub aux: Option<SharedString>,
}

impl ComboOption {
    pub fn plain(label: impl Into<SharedString>) -> ComboOption {
        ComboOption {
            icon: None,
            label: label.into(),
            aux: None,
        }
    }
}

/// How a combobox is drawn.
pub struct ComboSpec {
    pub name: &'static str,
    pub ix: usize,
    /// The icon before the shown value.
    pub icon: Option<&'static str>,
    /// The shown value; `None` draws the placeholder faint.
    pub shown: Option<SharedString>,
    pub placeholder: &'static str,
    pub options: Vec<ComboOption>,
    pub selected: Option<usize>,
    /// Draws the clear button after the value.
    pub clearable: bool,
    /// Whether the list opens with a search field.
    pub searchable: bool,
    /// Inside a filter row's expression box: no box of its own.
    pub flat: bool,
}

impl ComboSpec {
    pub fn new(name: &'static str, ix: usize) -> ComboSpec {
        ComboSpec {
            name,
            ix,
            icon: None,
            shown: None,
            placeholder: "",
            options: Vec::new(),
            selected: None,
            clearable: false,
            searchable: true,
            flat: false,
        }
    }
}

/// The placeholder a field shows while empty.
fn placeholder_of(which: &Field) -> &'static str {
    match which {
        Field::Search => "Search...",
        Field::ComboSearch => "Search...",
        Field::Limit => "e.g. 10",
        Field::ViewName => "View name",
        Field::TableSearch => "Find...",
        Field::Formula => "x + y",
        Field::SummaryName => super::summary::NEW_SUMMARY_NAME,
        _ => "",
    }
}

/// Where the open formula editor was last drawn, and its completion list.
pub(super) const FORMULA_EDITOR_KEY: u64 = 0x464f_524d_554c_4145;
const FORMULA_LIST_KEY: u64 = 0x464f_524d_554c_4c53;

/// The view an empty base shows, which is what `edit_view` materializes for
/// a write.
pub(super) fn default_view() -> View {
    View {
        kind: DEFAULT_VIEW_TYPE.to_string(),
        name: DEFAULT_VIEW_NAME.to_string(),
        ..View::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_property_takes_obsidians_own_label_and_icon() {
        let base = Base::from_yaml("views:\n  - type: table\n    name: V\n").unwrap();
        let labels: Vec<String> = FILE_PROPERTIES
            .iter()
            .map(|(id, _)| scoria_bases::table::column(&base, id).display)
            .collect();
        assert!(labels.contains(&"file base name".to_string()));
        assert!(labels.contains(&"created time".to_string()));
        assert_eq!(FILE_PROPERTIES.len(), 12);
        let p = Prop {
            id: "file.ctime".into(),
            label: "created time".into(),
            kind: PropertyKind::DateTime,
        };
        assert_eq!(p.icon(), "clock");
        assert_eq!(p.panel_icon(), "info");
    }

    #[test]
    fn the_keys_are_stable_and_distinct() {
        assert_eq!(key("a", 1), key("a", 1));
        assert_ne!(key("a", 1), key("a", 2));
        assert_ne!(key("sort", 0), key("group", 0));
    }
}
