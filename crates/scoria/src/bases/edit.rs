//! Editing **through** the table: a cell typed into writes the **note's**
//! frontmatter, not the base.
//!
//! A base's cell is a property of the file the row stands for, so editing one
//! is a frontmatter write against that note —
//! [`scoria_formats::properties::plan`] and [`apply`], the same planner the
//! property editor uses, over the note's own bytes, so its line endings, its
//! BOM and every key the edit did not touch keep their bytes — and the
//! `.base`'s own bytes do not change. Only one of the three column sources
//! can be edited at all: a frontmatter property can, a `file.*` property is
//! the filesystem's and a `formula.*` is computed, so both refuse
//! ([`editable_key`] is the whole rule).
//!
//! Three writes land in the **base** instead, and those go through
//! [`BasesView::edit_view`], which re-resolves the table and schedules the
//! save. Two of them are observed
//! (`tools/obsidian-probe/results/2026-10-25-bases.json`; the prose is in
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//!
//! - the column header's own context menu is *Hide column*, *Summarize...*,
//!   separator, *Sort A → Z*, *Sort Z → A*, (*Clear sort*, once one exists),
//!   separator, *Edit property...* — [`header_menu_rows`];
//! - *Sort Z → A* **prepends** `{property, direction: DESC}` to the view's
//!   `sort` list — [`prepend_sort`];
//! - *Hide column* **removes the property from `order`** — [`hide_column`];
//! - a header drag writes `columnSize` as a px map keyed by the id `order`
//!   carries (`columnSize:` / `  file.name: 232`, and the header then
//!   measures 232), with 88 px the floor — [`BasesView::resize_column`]
//!   through [`scoria_bases::layout::set_column_size`].
//!
//! **Unprobed**, and all of it is therefore this build's reading:
//!
//! - *whether a table cell is editable in place at all in 1.14.5*. The one
//!   cell control the run captured is a **disabled** checkbox
//!   (`<input disabled type="checkbox">`), which is evidence against it. This
//!   build allows it and keeps the interaction conventional: a click on a
//!   frontmatter cell opens the field, Enter commits, Escape cancels, a press
//!   outside commits — which is what the property editor does — and a click
//!   on a cell that cannot be edited opens the note, as the read-only first
//!   cut did for every cell.
//! - *what Summarize... offers*: the row opens something that is not a
//!   `.menu`, so its options were never read. The row is drawn and inert
//!   rather than given an invented submenu.
//! - *Edit property...*: nothing in the shell edits a property's name or type
//!   from a view, so this row is drawn and inert too.
//! - whether *Sort A → Z* prepends as its mirror does, whether either row
//!   drops an entry the list already had for the same property, and whether
//!   *Clear sort* clears this column's entries or the whole list. The
//!   readings are below, each on its own function.

use crate::bases::cells::{cell_text, row_of};
use crate::bases::table::HEADER_ROW_KEY;
use crate::bases::table::{ColumnResize, MIN_COLUMN_WIDTH};
use crate::bases::view::BasesView;
use crate::bases::writes::{
    clear_sort, hide_column, prepend_sort, set_group_property, sort_direction, ASC, DESC,
};
use crate::text_input::{InputEvent, TextInput};
use crate::theme::{Theme, FONT_UI_SMALL};
use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, Context, Entity, Focusable, Pixels, Point,
    SharedString, Subscription, Window,
};
use scoria_bases::{Column, ColumnSource};
use scoria_formats::config::PropertyTypes;
use scoria_formats::properties::{apply, new_block, plan, type_of, Change, PropertyType, Value};
use serde_json::{json, Value as Json};

/// A menu row whose label is this draws a separator line instead, as the
/// canvas's own right-click list does.
pub const SEPARATOR: &str = "---";

/// The `is-warning` colour of *Clear sort* and *Delete*: Obsidian's own
/// `--text-error` was not read off the run, so this is the red the rest of
/// the shell uses for a warning.
const WARNING: gpui::Hsla = gpui::Hsla {
    h: 0.985,
    s: 0.80,
    l: 0.60,
    a: 1.0,
};

// ---------------------------------------------------------------------------
// The column header's context menu
// ---------------------------------------------------------------------------

/// One row of the column header's context menu: the id a pick dispatches on,
/// the label and icon Obsidian draws, and the marks it wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuRow {
    pub id: &'static str,
    pub label: &'static str,
    /// The row's Lucide icon; empty for a separator.
    pub icon: &'static str,
    /// `mod-checked`: a check at the row's right.
    pub checked: bool,
    /// `is-warning`: the label in the warning colour.
    pub warning: bool,
    /// `has-submenu`: a chevron at the right, and a second list on hover.
    pub submenu: bool,
}

impl MenuRow {
    const fn plain(id: &'static str, label: &'static str, icon: &'static str) -> MenuRow {
        MenuRow {
            id,
            label,
            icon,
            checked: false,
            warning: false,
            submenu: false,
        }
    }

    const fn separator() -> MenuRow {
        MenuRow::plain("separator", SEPARATOR, "")
    }
}

/// What kind of column a header menu belongs to, which decides its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderKind {
    /// `file.name`: no *Group by this property* and no *Property type*.
    FileName,
    /// Any other `file.*` property.
    File,
    /// A frontmatter property: the whole menu.
    Property,
    /// A `formula.*` column: *Edit formula...* in place of *Edit property...*,
    /// no *Property type*.
    Formula,
}

/// The menu a right-click on a column header opens, in Obsidian's own order
/// and with its own labels and icons — **observed in full**
/// (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`, `s08`
/// `rightClick` and `nameHeaderMenu`, `s24` `formulaHeaderMenu` and
/// `headerMenu:*`): *Hide column* (`eye-off`), *Summarize...*
/// (`calculator`), *Group by this property* (`group`) | *Sort A → Z*
/// (`arrow-down-wide-narrow`, `mod-checked` while the column sorts that way),
/// *Sort Z → A* (`arrow-up-narrow-wide`), *Clear sort* (`eraser`,
/// `is-warning`, only while sorted) | | *Edit property...* (`edit-3`, which is
/// `pen-line`) | *Property type* ▸ (`info`). The `file name` header has no
/// *Group by* and no *Property type*; a formula header has *Edit formula...*
/// and no *Property type*. The two separators before *Edit property...* are
/// Obsidian's own empty section.
///
/// `sort` is the direction the column sorts in, when it does.
pub fn header_menu_rows(kind: HeaderKind, sort: Option<&str>) -> Vec<MenuRow> {
    let mut rows = vec![
        MenuRow::plain("hide-column", "Hide column", "eye-off"),
        MenuRow::plain("summarize", "Summarize...", "calculator"),
    ];
    if kind != HeaderKind::FileName {
        rows.push(MenuRow::plain(
            "group-by",
            "Group by this property",
            "group",
        ));
    }
    rows.push(MenuRow::separator());
    rows.push(MenuRow {
        checked: sort == Some(ASC),
        ..MenuRow::plain("sort-asc", "Sort A → Z", "arrow-down-wide-narrow")
    });
    rows.push(MenuRow {
        checked: sort == Some(DESC),
        ..MenuRow::plain("sort-desc", "Sort Z → A", "arrow-up-narrow-wide")
    });
    if sort.is_some() {
        rows.push(MenuRow {
            warning: true,
            ..MenuRow::plain("clear-sort", "Clear sort", "eraser")
        });
    }
    rows.push(MenuRow::separator());
    rows.push(MenuRow::separator());
    if kind == HeaderKind::Formula {
        rows.push(MenuRow::plain(
            "edit-formula",
            "Edit formula...",
            "pen-line",
        ));
    } else {
        rows.push(MenuRow::plain(
            "edit-property",
            "Edit property...",
            "pen-line",
        ));
    }
    if kind == HeaderKind::Property {
        rows.push(MenuRow::separator());
        rows.push(MenuRow {
            submenu: true,
            ..MenuRow::plain("property-type", "Property type", "info")
        });
    }
    rows
}

/// The *Property type* submenu's rows, observed (`s08` `submenus`): the six
/// types in this order, the column's current one checked.
pub const TYPE_ROWS: [(&str, &str, PropertyType); 6] = [
    ("Checkbox", "square-check", PropertyType::Checkbox),
    ("Date", "calendar", PropertyType::Date),
    ("Date & time", "clock", PropertyType::DateTime),
    ("List", "list", PropertyType::MultiText),
    ("Number", "binary", PropertyType::Number),
    ("Text", "text", PropertyType::Text),
];

/// The open header menu: which column it belongs to, that column's id as
/// `order` carries it, where it was opened and the rows it drew.
///
/// The rows are kept rather than recomputed so the control socket reports
/// exactly what is on screen, *Clear sort* included.
pub struct HeaderMenu {
    pub column: usize,
    pub id: String,
    pub position: Point<Pixels>,
    pub rows: Vec<MenuRow>,
    /// The frontmatter key *Property type* writes for, and the type it has
    /// now; `None` on a header with no such row.
    pub type_key: Option<(String, PropertyType)>,
    /// Whether the *Property type* list is showing beside its row.
    pub submenu: bool,
}

// ---------------------------------------------------------------------------
// What the menu writes into the view
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// What a cell edit writes into the note
// ---------------------------------------------------------------------------

/// The frontmatter key a column names, when the column is one a cell edit can
/// write.
///
/// This is the whole rule for what refuses to be edited: a
/// [`ColumnSource::Property`] is a key of the note's own properties block, a
/// [`ColumnSource::File`] is the filesystem's (`file.name` is the file's name,
/// not a property of it) and a [`ColumnSource::Formula`] is computed from the
/// others. Neither of the last two has bytes in the note to write.
pub fn editable_key(column: &Column) -> Option<&str> {
    match &column.source {
        ColumnSource::Property(name) => Some(name.as_str()),
        ColumnSource::File(_) | ColumnSource::Formula(_) => None,
    }
}

/// The note's bytes with `key` set to what was typed, or `None` when nothing
/// would change.
///
/// This is the whole write, and it is the property editor's own path: the
/// block [`scoria_cache::note_properties`] found is planned over the note's
/// own text, so the CRLF a note is written with, a UTF-8 BOM before the
/// opening fence, a comment, a flow list and every key the edit did not name
/// all keep their bytes. The value is read as the key's own type —
/// `types.json`, then the reserved keys, then inference — so a `number`
/// column commits a bare number and a `multitext` column commits a block
/// sequence.
///
/// A note with **no** block gets one, above everything and with no blank line
/// after it, which is what Obsidian does when a property is added to a note
/// without frontmatter; the BOM stays first and an empty value writes nothing
/// at all rather than a block holding one blank key.
pub fn write_property(text: &str, key: &str, typed: &str, types: &PropertyTypes) -> Option<String> {
    if let Some(props) = scoria_cache::note_properties(text) {
        let value = Value::parse_as(typed, props.type_of(key, types));
        let edits = plan(
            text,
            &props,
            &[Change::Set {
                key: key.to_string(),
                value,
            }],
        )?;
        return Some(apply(text, &edits));
    }
    let value = Value::parse_as(typed, type_of(key, &[], types));
    if value == Value::Empty {
        return None;
    }
    // A BOM is the one thing that may stand before the opening fence.
    let at = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block = new_block(key, &value, newline);
    let mut out = String::with_capacity(text.len() + block.len());
    out.push_str(&text[..at]);
    out.push_str(&block);
    out.push_str(&text[at..]);
    Some(out)
}

/// The width a header drag leaves a column at: the pointer, measured from
/// the column's own left edge, floored at [`MIN_COLUMN_WIDTH`].
///
/// Read off the pointer rather than accumulated from deltas, so a drag that
/// outran a frame cannot leave the edge behind the cursor. **Observed**: the
/// drag writes `columnSize` and "the header then measures 232", and 88 px is
/// the floor.
pub fn resized_width(column_left: f32, pointer_x: f32) -> f32 {
    (pointer_x - column_left).max(MIN_COLUMN_WIDTH)
}

// ---------------------------------------------------------------------------
// The state
// ---------------------------------------------------------------------------

/// The cell a field is open over: where it is drawn, and what it writes.
///
/// The line and column place the field; the path and the key are what the
/// write needs, and they are kept beside the indexes because a rebuild — the
/// watcher re-resolving the table under the field — can move a row to another
/// line. [`BasesView::rebuild_table`] drops the edit when the line it opened
/// on no longer stands for the same note.
pub struct CellEdit {
    /// Index into [`crate::bases::table::TableState::lines`].
    pub line: usize,
    /// Index into the table's columns.
    pub column: usize,
    /// The note the row stands for.
    pub path: String,
    /// The frontmatter key the column names.
    pub key: String,
}

/// What editing through the table keeps between renders.
///
/// The field is built on the first edit rather than in the constructor: the
/// shell resets this state with [`Default`], which has no `cx` to make an
/// entity with, and a base is usually read without one ever being opened.
#[derive(Default)]
pub struct EditState {
    pub cell: Option<CellEdit>,
    pub menu: Option<HeaderMenu>,
    pub field: Option<Entity<TextInput>>,
    /// Keeps the field's Submit and Cancel coming; dropped with the state.
    pub subscription: Option<Subscription>,
}

impl EditState {
    pub fn state_json(&self) -> Json {
        json!({
            "cell": self.cell.as_ref().map(|cell| json!({
                "line": cell.line,
                "column": cell.column,
                "path": cell.path,
                "key": cell.key,
            })),
            "menu": self.menu.as_ref().map(|menu| json!({
                "column": menu.column,
                "id": menu.id,
                "x": f32::from(menu.position.x),
                "y": f32::from(menu.position.y),
                "rows": menu.rows.iter().map(|row| json!({
                    "id": row.id,
                    "label": row.label,
                    "icon": row.icon,
                    "checked": row.checked,
                    "warning": row.warning,
                    "submenu": row.submenu,
                })).collect::<Vec<_>>(),
                "submenu": menu.submenu,
                "type": menu.type_key.as_ref().map(|(k, t)| json!({"key": k, "type": t.as_str()})),
            })),
        })
    }
}

/// What [`gpui::StatefulInteractiveElement::on_drag`] carries while a header
/// grip is being dragged: which column's edge is moving.
///
/// It draws nothing. Obsidian's own drag may show a guide line; the run never
/// captured one, and moving the header's own edge is what this build does
/// instead.
#[derive(Clone)]
pub struct ColumnDrag {
    pub column: usize,
}

/// What a drag of the header itself carries: which column is being moved.
/// Observed: a header is draggable, the body wears `is-grabbing`, and a
/// `table-drag-target mod-col` line (0 wide, the table's full height) marks
/// where it would land.
#[derive(Clone)]
pub struct HeaderMove {
    pub column: usize,
}

impl gpui::Render for HeaderMove {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

impl gpui::Render for ColumnDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

// ---------------------------------------------------------------------------
// The view's own methods
// ---------------------------------------------------------------------------

impl BasesView {
    // ----- the header menu ---------------------------------------------------

    /// Opens the column header's context menu at the pointer.
    pub(super) fn open_header_menu(
        &mut self,
        column: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        // Opening a menu is the last moment an open field can be written.
        self.commit_cell_edit(cx);
        let Some(col) = self.table().and_then(|t| t.columns.get(column)).cloned() else {
            return;
        };
        let id = col.id.clone();
        let kind = match &col.source {
            ColumnSource::File(f) if f.eq_ignore_ascii_case("name") => HeaderKind::FileName,
            ColumnSource::File(_) => HeaderKind::File,
            ColumnSource::Formula(_) => HeaderKind::Formula,
            ColumnSource::Property(_) => HeaderKind::Property,
        };
        let sort = self.view().and_then(|v| sort_direction(v, &id));
        let type_key = match &col.source {
            ColumnSource::Property(name) => {
                let key = name.strip_prefix("note.").unwrap_or(name).to_string();
                let ty = self.property_type_of(&key);
                Some((key, ty))
            }
            _ => None,
        };
        self.editing.menu = Some(HeaderMenu {
            column,
            id,
            position,
            rows: header_menu_rows(kind, sort.as_deref()),
            type_key,
            submenu: false,
        });
        cx.notify();
    }

    /// The type a frontmatter key has: `types.json` first, then what its
    /// values infer, which is what the *Property type* submenu checks.
    fn property_type_of(&self, key: &str) -> PropertyType {
        use scoria_bases::rule::PropertyKind;
        match self.note_kind(key) {
            PropertyKind::Number => PropertyType::Number,
            PropertyKind::Checkbox => PropertyType::Checkbox,
            PropertyKind::Date => PropertyType::Date,
            PropertyKind::DateTime => PropertyType::DateTime,
            PropertyKind::List => PropertyType::MultiText,
            PropertyKind::Tags => PropertyType::Tags,
            _ => PropertyType::Text,
        }
    }

    pub(super) fn close_header_menu(&mut self, cx: &mut Context<Self>) {
        if self.editing.menu.take().is_some() {
            cx.notify();
        }
    }

    /// Where a page opened from a column header hangs: under the header
    /// cell, which is where Obsidian put the *Edit* and *Summarize* pages
    /// (observed at y 152 under a header ending at 148).
    pub(super) fn header_page_anchor(&self, column: usize) -> Option<Point<Pixels>> {
        let row = self.anchor(HEADER_ROW_KEY)?;
        let before: f32 = self.table_state.widths.iter().take(column).sum();
        Some(Point::new(
            row.origin.x + px(before),
            row.origin.y + row.size.height + px(4.),
        ))
    }

    /// Opens the properties panel on `page`, hung under column `column`'s
    /// header rather than under the toolbar's button.
    pub(super) fn open_page_under_header(
        &mut self,
        column: usize,
        page: super::toolbar::Page,
        cx: &mut Context<Self>,
    ) {
        self.toolbar.pinned_at = self.header_page_anchor(column);
        self.toolbar.panel = super::toolbar::Panel::Properties;
        self.toolbar.page = page;
        self.toolbar.combo = None;
        self.toolbar.selected = None;
        self.toolbar.editing = None;
        cx.notify();
    }

    /// What a menu row writes. Every one of these goes through
    /// [`BasesView::edit_view`], which re-resolves the table from the parsed
    /// base and schedules the save; the `.base` is never serialized here.
    pub(super) fn header_menu_pick(&mut self, row: &'static str, cx: &mut Context<Self>) {
        let Some(menu) = self.editing.menu.take() else {
            return;
        };
        let id = menu.id;
        let column = menu.column;
        match row {
            "hide-column" => {
                // A view with no `order` shows the columns its rows imply,
                // which is exactly the table's own column list.
                let implied: Vec<String> = self
                    .table()
                    .map(|t| t.columns.iter().map(|c| c.id.clone()).collect())
                    .unwrap_or_default();
                self.edit_view(move |view| hide_column(view, &id, &implied), cx);
            }
            "sort-asc" => self.edit_view(move |view| prepend_sort(view, &id, ASC), cx),
            "sort-desc" => self.edit_view(move |view| prepend_sort(view, &id, DESC), cx),
            "clear-sort" => self.edit_view(move |view| clear_sort(view, &id), cx),
            // Observed (`s18` `applied:Group by this property`): the file
            // gained `groupBy: {property: due, direction: ASC}` for the
            // header clicked, the id spelled as `order` spells it.
            "group-by" => self.edit_view(move |view| set_group_property(view, &id), cx),
            "edit-property" => {
                let page = super::toolbar::Page::Property(scoria_bases::layout::qualify(&id));
                self.open_page_under_header(column, page, cx);
            }
            "edit-formula" => {
                let name = id.strip_prefix("formula.").unwrap_or(&id).to_string();
                self.open_page_under_header(column, super::toolbar::Page::Formula(name), cx);
            }
            "summarize" => self.summarize_from_header(column, cx),
            _ => {}
        }
        cx.notify();
    }

    /// The *Property type* submenu's pick: `types.json` is the workspace's
    /// to write, so the view only asks.
    fn header_menu_pick_type(&mut self, ty: PropertyType, cx: &mut Context<Self>) {
        let Some(menu) = self.editing.menu.take() else {
            return;
        };
        if let Some((key, _)) = menu.type_key {
            cx.emit(super::view::BasesEvent::SetPropertyType { key, ty });
        }
        cx.notify();
    }

    /// The menu itself, over everything else: Obsidian's `.menu`, 25 px rows
    /// of a 16 px icon and the label, a separator per section change, a
    /// check on a `mod-checked` row and the warning colour on `is-warning`,
    /// and the *Property type* list beside its row while the pointer rests
    /// on it. A press outside closes it.
    pub(super) fn render_header_menu(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.editing.menu.as_ref()?;
        let mut list = div()
            .id("bases-header-menu")
            .relative()
            .flex()
            .flex_col()
            .min_w(px(192.))
            .py(px(6.))
            .bg(theme.bg_secondary)
            .border_1()
            .border_color(theme.tab_divider)
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.text)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_header_menu(cx)));
        let mut type_row_top = px(0.);
        let mut y = px(6.);
        for (ix, row) in menu.rows.iter().enumerate() {
            if row.label == SEPARATOR {
                list = list.child(div().my(px(6.)).h(px(1.)).bg(theme.border));
                y += px(13.);
                continue;
            }
            if row.submenu {
                type_row_top = y;
            }
            let id = row.id;
            let submenu = row.submenu;
            let color = if row.warning { WARNING } else { theme.text };
            list = list.child(
                div()
                    .id(("bases-header-item", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(25.))
                    .mx(px(6.))
                    .px(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(color)
                    .hover(|s| s.bg(theme.bg_alt))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if let Some(menu) = this.editing.menu.as_mut() {
                            if *hovered && menu.submenu != submenu {
                                menu.submenu = submenu;
                                cx.notify();
                            }
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if submenu {
                            if let Some(menu) = this.editing.menu.as_mut() {
                                menu.submenu = true;
                                cx.notify();
                            }
                        } else {
                            this.header_menu_pick(id, cx);
                        }
                    }))
                    .child(crate::assets::icon(row.icon, 16., theme.text_muted))
                    .child(div().flex_1().child(SharedString::from(row.label)))
                    .when(row.checked, |d| {
                        d.child(crate::assets::icon("check", 14., theme.text_muted))
                    })
                    .when(submenu, |d| {
                        d.child(crate::assets::icon("chevron-right", 14., theme.text_muted))
                    }),
            );
            y += px(25.);
        }
        if menu.submenu {
            if let Some((_, current)) = menu.type_key.as_ref() {
                let current = *current;
                let mut sub = div()
                    .id("bases-header-type-menu")
                    .absolute()
                    .left(px(188.))
                    .top(type_row_top)
                    .flex()
                    .flex_col()
                    .min_w(px(126.))
                    .py(px(6.))
                    .bg(theme.bg_secondary)
                    .border_1()
                    .border_color(theme.tab_divider)
                    .rounded(px(8.))
                    .shadow_lg()
                    .occlude();
                for (ix, (label, icon_name, ty)) in TYPE_ROWS.iter().enumerate() {
                    let ty = *ty;
                    sub =
                        sub.child(
                            div()
                                .id(("bases-header-type", ix))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .h(px(25.))
                                .mx(px(6.))
                                .px(px(8.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.bg_alt))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.header_menu_pick_type(ty, cx)
                                }))
                                .child(crate::assets::icon(icon_name, 16., theme.text_muted))
                                .child(div().flex_1().child(SharedString::from(*label)))
                                .when(ty == current, |d| {
                                    d.child(crate::assets::icon("check", 14., theme.text_muted))
                                }),
                        );
                }
                list = list.child(sub);
            }
        }
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    // ----- the header drag ---------------------------------------------------

    /// A header dragged onto another column: `order` is rewritten.
    pub(super) fn drop_column(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.table_state.drop_at = None;
        let shown: Vec<String> = self
            .table()
            .map(|t| t.columns.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default();
        if from == to || from + 1 == to {
            cx.notify();
            return;
        }
        self.edit_view(
            move |view| super::writes::move_column(view, from, to, &shown),
            cx,
        );
    }

    /// Where a dragged header would land, from the pointer's x against the
    /// column edges: the slot whose left edge is nearest.
    pub(super) fn header_drag_over(
        &mut self,
        pointer_x: Pixels,
        table_left: Pixels,
        cx: &mut Context<Self>,
    ) {
        let x = f32::from(pointer_x) - f32::from(table_left);
        let mut edge = 0.;
        let mut slot = 0;
        for (ix, w) in self.table_state.widths.iter().enumerate() {
            if x < edge + w / 2. {
                slot = ix;
                break;
            }
            edge += w;
            slot = ix + 1;
        }
        if self.table_state.drop_at != Some(slot) {
            self.table_state.drop_at = Some(slot);
            cx.notify();
        }
    }

    /// A header grip moved: the column's right edge follows the pointer.
    ///
    /// `table_left` is the left edge of the table itself, which the drag event
    /// carries as its own element's bounds; the column's left edge is that
    /// plus the widths before it, so the width is read off the pointer rather
    /// than accumulated from deltas and cannot drift. 88 px is the floor,
    /// observed: the fixture's shortest label came out 88 px wide.
    ///
    /// Nothing is written while the pointer moves. `columnSize` is one write
    /// at the end of the drag, which is both what the run saw ("lands on disk
    /// within about a second") and what keeps a 20 000-row table from being
    /// re-evaluated on every mouse move.
    pub(super) fn resize_column(
        &mut self,
        column: usize,
        pointer_x: Pixels,
        table_left: Pixels,
        cx: &mut Context<Self>,
    ) {
        let before: f32 = self.table_state.widths.iter().take(column).sum();
        let width = resized_width(f32::from(table_left) + before, f32::from(pointer_x));
        let Some(slot) = self.table_state.widths.get_mut(column) else {
            return;
        };
        if (*slot - width).abs() < 0.5 {
            return;
        }
        *slot = width;
        self.table_state.resize = Some(ColumnResize { column, width });
        cx.notify();
    }

    /// The drag ended: `columnSize` is written as an integer px map keyed by
    /// the id `order` carries.
    pub(super) fn finish_column_resize(&mut self, cx: &mut Context<Self>) {
        let Some(resize) = self.table_state.resize.take() else {
            return;
        };
        let Some(id) = self
            .table()
            .and_then(|t| t.columns.get(resize.column))
            .map(|c| c.id.clone())
        else {
            return;
        };
        let width = resize.width as f64;
        self.edit_view(
            move |view| scoria_bases::layout::set_column_size(view, &id, width),
            cx,
        );
    }

    // ----- the cell field ----------------------------------------------------

    /// The field, built on the first edit and kept for the rest of the
    /// session.
    fn cell_field(&mut self, cx: &mut Context<Self>) -> Entity<TextInput> {
        if let Some(field) = self.editing.field.clone() {
            return field;
        }
        let field = cx.new(|cx| TextInput::new("", cx));
        // Enter and Escape reach the field as its own Submit and Cancel, so
        // nothing here has to race an action: the field is focused and the
        // bindings are its key context's.
        self.editing.subscription = Some(cx.subscribe(
            &field,
            |this, _, event: &InputEvent, cx| match event {
                InputEvent::Submit => this.commit_cell_edit(cx),
                InputEvent::Cancel => this.cancel_cell_edit(cx),
                _ => {}
            },
        ));
        self.editing.field = Some(field.clone());
        field
    }

    /// Opens the field over one cell, with the property's own value in it.
    ///
    /// The value shown is read from the note rather than from the cell: a
    /// cell's text is what the table *renders* (a list joins with a bare
    /// `,`), and what a field has to commit is what the properties block
    /// holds, which is [`scoria_formats::properties::Value::display`] — the
    /// same text the property editor puts in its own field. The cell's text is
    /// the fallback for a note that cannot be read.
    pub(super) fn start_cell_edit(
        &mut self,
        line: usize,
        column: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Opening another cell is the last moment this one can be written.
        self.commit_cell_edit(cx);
        self.editing.menu = None;
        let Some(found) = self.cell_target(line, column) else {
            return;
        };
        let (path, key, fallback) = found;
        let shown = self
            .note_text(&path)
            .and_then(|text| {
                scoria_cache::note_properties(&text)
                    .and_then(|props| props.get(&key).map(|p| p.value.display()))
            })
            .unwrap_or(fallback);
        let field = self.cell_field(cx);
        field.update(cx, |input, cx| input.set_text(shown, cx));
        window.focus(&field.focus_handle(cx), cx);
        self.editing.cell = Some(CellEdit {
            line,
            column,
            path,
            key,
        });
        cx.notify();
    }

    /// The note, the key and the cell's own text for one cell, when that cell
    /// is one a frontmatter write can reach.
    fn cell_target(&self, line: usize, column: usize) -> Option<(String, String, String)> {
        let table = self.table()?;
        let row = row_of(table, *self.table_state.lines.get(line)?)?;
        let key = editable_key(table.columns.get(column)?)?.to_string();
        let text = row.cells.get(column).map(cell_text).unwrap_or_default();
        Some((row.file.path.clone(), key, text))
    }

    /// Writes what was typed. Enter, a press outside the field and opening
    /// another cell all come here.
    pub(super) fn commit_cell_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.editing.cell.take() else {
            return;
        };
        let typed = match self.editing.field.as_ref() {
            Some(field) => field.read(cx).text().to_string(),
            None => return,
        };
        self.write_cell(&edit.path, &edit.key, &typed, cx);
        cx.notify();
    }

    /// Escape: the note is not written and the field closes.
    pub(super) fn cancel_cell_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing.cell.take().is_some() {
            cx.notify();
        }
    }

    /// A click on an editable checkbox cell flips the property.
    ///
    /// **Unprobed**, and against the one observation: Obsidian's own box is
    /// drawn `disabled`. A boolean has no text to type, so a field over one
    /// would be a field holding `true`; flipping it in place is the
    /// conventional reading, and the box is still drawn exactly as the run
    /// captured it.
    pub(super) fn toggle_checkbox_cell(
        &mut self,
        line: usize,
        column: usize,
        cx: &mut Context<Self>,
    ) {
        self.commit_cell_edit(cx);
        let Some((path, key, _)) = self.cell_target(line, column) else {
            return;
        };
        let checked = self
            .table()
            .and_then(|t| row_of(t, *self.table_state.lines.get(line)?))
            .and_then(|row| row.cells.get(column))
            .is_some_and(|cell| matches!(cell.value, scoria_bases::Value::Bool(true)));
        self.write_cell(&path, &key, if checked { "false" } else { "true" }, cx);
        cx.notify();
    }

    /// The note's bytes as they are on disk, for filling the field with the
    /// value the note really holds. A read, not a write.
    fn note_text(&self, path: &str) -> Option<String> {
        let state = self.vault()?;
        let absolute = state.vault().write_path(path).ok()?;
        std::fs::read_to_string(absolute).ok()
    }

    /// The frontmatter write itself, which the **workspace** performs.
    ///
    /// A base's row is another file, and no view in this shell writes a file
    /// it does not hold: the note may be open in a buffer with unsaved edits,
    /// and a write behind that buffer's back would be thrown away by its own
    /// next save. So the cell reports the change it wants —
    /// [`BasesEvent::SetProperty`] — and the workspace reads the note, plans
    /// the change over its own bytes through [`write_property`], guards the
    /// buffer and writes it.
    ///
    /// The row is not patched afterwards either. A base is a view over the
    /// whole vault, so the write lands as a vault change and the table is
    /// resolved again from the index.
    fn write_cell(&mut self, path: &str, key: &str, typed: &str, cx: &mut Context<Self>) {
        self.set_property(path, key, typed, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_formats::base::Base;
    use scoria_formats::base::View;

    fn view_of(yaml: &str) -> View {
        Base::from_yaml(yaml)
            .expect("the fixture parses")
            .views
            .remove(0)
    }

    fn ids(view: &View) -> Vec<String> {
        view.order.clone().unwrap_or_default()
    }

    fn sorts(view: &View) -> Vec<(String, Option<String>)> {
        view.sort
            .iter()
            .map(|s| (s.property.clone(), s.direction.clone()))
            .collect()
    }

    // ----- the menu ----------------------------------------------------------

    #[test]
    fn the_header_menu_is_the_observed_one_for_each_kind_of_column() {
        let labels = |kind, sort| -> Vec<&'static str> {
            header_menu_rows(kind, sort)
                .iter()
                .map(|r| r.label)
                .collect()
        };
        // `s08` `rightClick`: a frontmatter property, sorted A -> Z.
        assert_eq!(
            labels(HeaderKind::Property, Some(ASC)),
            vec![
                "Hide column",
                "Summarize...",
                "Group by this property",
                SEPARATOR,
                "Sort A → Z",
                "Sort Z → A",
                "Clear sort",
                SEPARATOR,
                SEPARATOR,
                "Edit property...",
                SEPARATOR,
                "Property type",
            ]
        );
        // `s08` `nameHeaderMenu`: `file name` has no Group by and no type.
        assert_eq!(
            labels(HeaderKind::FileName, Some(ASC)),
            vec![
                "Hide column",
                "Summarize...",
                SEPARATOR,
                "Sort A → Z",
                "Sort Z → A",
                "Clear sort",
                SEPARATOR,
                SEPARATOR,
                "Edit property...",
            ]
        );
        // `s24` `formulaHeaderMenu`: unsorted, Edit formula, no type.
        assert_eq!(
            labels(HeaderKind::Formula, None),
            vec![
                "Hide column",
                "Summarize...",
                "Group by this property",
                SEPARATOR,
                "Sort A → Z",
                "Sort Z → A",
                SEPARATOR,
                SEPARATOR,
                "Edit formula...",
            ]
        );
    }

    #[test]
    fn the_sort_rows_check_the_direction_and_clear_sort_warns() {
        let rows = header_menu_rows(HeaderKind::Property, Some(DESC));
        let by = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert!(!by("sort-asc").checked);
        assert!(by("sort-desc").checked);
        assert!(by("clear-sort").warning);
        assert!(by("property-type").submenu);
        assert_eq!(by("hide-column").icon, "eye-off");
        assert_eq!(by("group-by").icon, "group");
        assert_eq!(by("edit-property").icon, "pen-line");
        let unsorted = header_menu_rows(HeaderKind::File, None);
        assert!(unsorted.iter().all(|r| !r.checked && !r.warning));
        assert!(unsorted
            .iter()
            .all(|r| r.id != "clear-sort" && r.id != "property-type"));
        assert_eq!(
            TYPE_ROWS.iter().map(|(l, _, _)| *l).collect::<Vec<_>>(),
            ["Checkbox", "Date", "Date & time", "List", "Number", "Text"]
        );
    }

    // ----- what the menu writes ---------------------------------------------

    #[test]
    fn sort_z_to_a_prepends_rather_than_appends() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    sort:\n      - property: file.name\n        direction: ASC\n",
        );
        prepend_sort(&mut view, "status", DESC);
        assert_eq!(
            sorts(&view),
            vec![
                ("status".to_string(), Some("DESC".to_string())),
                ("file.name".to_string(), Some("ASC".to_string())),
            ]
        );
    }

    #[test]
    fn sort_a_to_z_prepends_too_and_writes_asc() {
        let mut view = view_of("views:\n  - type: table\n    name: V\n");
        prepend_sort(&mut view, "status", ASC);
        assert_eq!(
            sorts(&view),
            vec![("status".to_string(), Some("ASC".to_string()))]
        );
    }

    #[test]
    fn sorting_the_same_property_twice_leaves_one_entry() {
        let mut view = view_of("views:\n  - type: table\n    name: V\n");
        prepend_sort(&mut view, "status", ASC);
        // The qualified spelling is the same property as the bare one.
        prepend_sort(&mut view, "note.status", DESC);
        assert_eq!(
            sorts(&view),
            vec![("note.status".to_string(), Some("DESC".to_string()))]
        );
    }

    #[test]
    fn clear_sort_drops_this_columns_entries_only() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    sort:\n      - property: status\n        direction: DESC\n      - property: file.name\n        direction: ASC\n",
        );
        assert!(crate::bases::writes::is_sorted(&view, "note.status"));
        clear_sort(&mut view, "note.status");
        assert!(!crate::bases::writes::is_sorted(&view, "status"));
        assert_eq!(
            sorts(&view),
            vec![("file.name".to_string(), Some("ASC".to_string()))]
        );
    }

    #[test]
    fn hide_column_drops_exactly_one_id_from_order() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    order:\n      - file.name\n      - status\n      - formula.ppu\n",
        );
        hide_column(&mut view, "note.status", &[]);
        assert_eq!(ids(&view), vec!["file.name", "formula.ppu"]);
    }

    #[test]
    fn hiding_a_column_of_an_implied_order_materializes_the_rest() {
        let mut view = view_of("views:\n  - type: table\n    name: V\n");
        let implied = vec![
            "file.name".to_string(),
            "status".to_string(),
            "density".to_string(),
        ];
        hide_column(&mut view, "status", &implied);
        assert_eq!(ids(&view), vec!["file.name", "density"]);
    }

    #[test]
    fn hiding_the_last_column_writes_an_empty_order() {
        // Observed (2026-11-01, the properties panel's *Hide all*): an
        // explicit `order: []` is the view with no columns, which an absent
        // key -- every implied column -- is not.
        let mut view =
            view_of("views:\n  - type: table\n    name: V\n    order:\n      - status\n");
        hide_column(&mut view, "status", &[]);
        assert_eq!(view.order, Some(Vec::new()));
        let mut implied_view = view_of("views:\n  - type: table\n    name: V\n");
        hide_column(&mut implied_view, "status", &["status".to_string()]);
        assert_eq!(implied_view.order, Some(Vec::new()));
    }

    // ----- what a cell edit writes ------------------------------------------

    fn column(id: &str) -> Column {
        Column {
            id: id.to_string(),
            display: id.to_string(),
            source: scoria_bases::table::source_of(id),
        }
    }

    #[test]
    fn only_a_frontmatter_property_can_be_edited() {
        assert_eq!(editable_key(&column("status")), Some("status"));
        assert_eq!(editable_key(&column("note.status")), Some("status"));
        assert_eq!(editable_key(&column("file.name")), None);
        assert_eq!(editable_key(&column("formula.ppu")), None);
    }

    // ----- what a header drag writes ----------------------------------------

    #[test]
    fn a_resize_writes_an_integer_px_map_keyed_by_the_id_order_carries() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    order:\n      - file.name\n      - status\n",
        );
        scoria_bases::layout::set_column_size(&mut view, "file.name", 232.4);
        let layout = scoria_bases::TableLayout::of(&view);
        assert_eq!(layout.column_size.get("file.name"), Some(&232.));
        assert_eq!(layout.width_of("file.name"), Some(232.));
        // And the bytes are the observed ones: the key is a mapping of the
        // bare id to an integer, after every documented key of the view.
        let base = Base::from_yaml("views:\n  - type: table\n    name: V\n").expect("parses");
        let mut written = base.clone();
        scoria_bases::layout::set_column_size(&mut written.views[0], "file.name", 232.);
        assert!(
            written
                .to_yaml()
                .contains("columnSize:\n      file.name: 232"),
            "{}",
            written.to_yaml()
        );
    }

    #[test]
    fn a_drag_measures_from_the_columns_own_left_edge_and_floors_at_eighty_eight() {
        assert_eq!(resized_width(344., 576.), 232.);
        assert_eq!(resized_width(344., 400.), MIN_COLUMN_WIDTH);
        assert_eq!(resized_width(344., 0.), MIN_COLUMN_WIDTH);
    }

    #[test]
    fn a_cell_edit_plans_only_the_entry_it_changes() {
        let types = PropertyTypes::default();
        let text = "---\ntitle: Hi   # kept\nstatus: todo\n---\n# Body\n";
        let out = write_property(text, "status", "done", &types).expect("one edit");
        assert_eq!(out, "---\ntitle: Hi   # kept\nstatus: done\n---\n# Body\n");
    }

    #[test]
    fn an_unchanged_value_writes_nothing() {
        let types = PropertyTypes::default();
        let text = "---\nstatus: todo\n---\nbody\n";
        assert_eq!(write_property(text, "status", "todo", &types), None);
    }

    #[test]
    fn a_crlf_note_with_a_bom_keeps_both() {
        let types = PropertyTypes::default();
        let text = "\u{feff}---\r\ntitle: Hi\r\nstatus: todo\r\n---\r\nbody\r\n";
        let out = write_property(text, "status", "done", &types).expect("one edit");
        assert_eq!(
            out,
            "\u{feff}---\r\ntitle: Hi\r\nstatus: done\r\n---\r\nbody\r\n"
        );
        assert!(out.starts_with('\u{feff}'), "the BOM is still first");
        assert!(!out.contains("\n\n"), "no line ending was rewritten");
    }

    #[test]
    fn a_new_key_goes_after_the_last_entry() {
        let types = PropertyTypes::default();
        let text = "---\ntitle: Hi\n---\nbody\n";
        let out = write_property(text, "status", "todo", &types).expect("one edit");
        assert_eq!(out, "---\ntitle: Hi\nstatus: todo\n---\nbody\n");
    }

    #[test]
    fn a_note_with_no_block_gets_one_above_everything() {
        let types = PropertyTypes::default();
        let out = write_property("# Body\n", "status", "todo", &types).expect("a new block");
        assert_eq!(out, "---\nstatus: todo\n---\n# Body\n");
        // And the BOM of a note that has one still comes first.
        let out = write_property("\u{feff}# Body\r\n", "status", "todo", &types).expect("a block");
        assert_eq!(out, "\u{feff}---\r\nstatus: todo\r\n---\r\n# Body\r\n");
    }

    #[test]
    fn an_empty_value_never_invents_a_block() {
        let types = PropertyTypes::default();
        assert_eq!(write_property("# Body\n", "status", "", &types), None);
    }

    #[test]
    fn the_keys_own_type_decides_how_the_value_is_written() {
        let mut types = PropertyTypes::default();
        types
            .types
            .insert("count".to_string(), "number".to_string());
        types.types.insert("tags".to_string(), "tags".to_string());
        let text = "---\ncount: 1\ntags: [a]\n---\nbody\n";
        let out = write_property(text, "count", "42", &types).expect("one edit");
        assert!(out.contains("count: 42"), "a number is written bare: {out}");
        let out = write_property(text, "tags", "a, b", &types).expect("one edit");
        assert!(
            out.contains("tags:\n  - a\n  - b"),
            "a list is a block sequence: {out}"
        );
    }

    #[test]
    fn a_checkbox_commits_true_and_false() {
        let types = PropertyTypes::default();
        let text = "---\ndone: false\n---\nbody\n";
        let out = write_property(text, "done", "true", &types).expect("one edit");
        assert_eq!(out, "---\ndone: true\n---\nbody\n");
    }
}
