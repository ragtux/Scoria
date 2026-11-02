//! The element that draws a `.base`, and the state behind it.
//!
//! This module is the shell: it holds the parsed file, the view that is
//! showing, the table that view resolved to, and the writes the view makes.
//! The drawing itself belongs to a layout —
//! [`table`](crate::bases::table), [`cards`](crate::bases::cards),
//! [`list`](crate::bases::list) and [`kanban`](crate::bases::kanban), the
//! four layouts Obsidian registers — each of which adds its own methods to
//! [`BasesView`] and keeps its own state in one of its fields. The toolbar
//! above them is [`toolbar`](crate::bases::toolbar) and editing a cell is
//! [`edit`](crate::bases::edit).
//!
//! What the shell itself draws is the two states that are not a layout, both
//! observed (`tools/obsidian-probe/results/2026-10-25-bases.json`; the prose
//! is in
//! [[Specification#Observed Bases table view, Obsidian 1.14.5 (installer build 1.13.7)]]):
//!
//! - a **parse error** as `.bases-error`, two paragraphs: the banner
//!   [`PARSE_ERROR_BANNER`] and then the message, with nothing joining them;
//! - an **unknown view type** as the same block with one paragraph,
//!   "Unknown view type: x", and no rows.
//!
//! **Writing.** Obsidian writes a base on a toolbar change and canonicalizes
//! it on close, and never writes one that failed to parse. So does this: a
//! change goes through [`BasesView::edit_view`] or
//! [`BasesView::edit_base`], which re-resolve the table from the parsed file
//! and emit [`BasesEvent::Edited`]; the workspace's save path asks for
//! [`BasesView::file_contents`] — `Base::to_yaml`, Obsidian's own bytes — and
//! hands the written text back to [`BasesView::mark_saved`] so the watcher
//! can tell the view's own write from somebody else's.

use crate::bases::rows;
use crate::theme::{Theme, FONT_UI_SMALL};
use crate::workspace::VaultState;
use gpui::{
    div, prelude::*, px, AnyElement, Context, EventEmitter, FocusHandle, Focusable, Render,
    SharedString, Window,
};
use scoria_bases::{Layout, Table};
use scoria_formats::base::{Base, View, PARSE_ERROR_BANNER};
use serde_json::{json, Value as Json};
use std::sync::Arc;

/// What the view asks the workspace to do; it owns no vault of its own.
pub enum BasesEvent {
    /// A row was activated: open this file.
    Open(String),
    /// The base changed and is dirty: the autosave timer starts, as the
    /// canvas's own edits do.
    Edited,
    /// Something the view has to say, as a Notice — "Copied to your
    /// clipboard" and the rest.
    Report(String),
    /// A cell was typed into: set `key` to `value` in the **note** the row
    /// stands for.
    ///
    /// The view does not write it. A base's row is another file, and no view
    /// in this shell writes a file it does not hold — the workspace does,
    /// because only it can see whether that note is open in a buffer with
    /// unsaved edits, which a write behind its back would throw away.
    SetProperty {
        path: String,
        key: String,
        value: String,
    },
    /// The results panel's *Export CSV...*: the workspace asks where the
    /// file goes and writes these bytes there.
    ExportCsv { name: String, csv: String },
    /// A property page's **Property type** select: `types.json` is the
    /// workspace's to write, and every pane reading it to refresh.
    SetPropertyType {
        key: String,
        ty: scoria_formats::properties::PropertyType,
    },
    /// The toolbar's *New* button: `bases:add-item`, which the workspace
    /// runs.
    AddItem,
    /// A right click on a row's name, a card or a list line's link: the
    /// file menu over that row's file, which the workspace draws.
    FileMenu {
        path: String,
        position: gpui::Point<gpui::Pixels>,
    },
    /// A date cell's *Open daily note* icon: the daily note of that day,
    /// which the workspace opens or creates.
    OpenDaily { date: chrono::NaiveDate },
}

pub struct BasesView {
    focus: FocusHandle,
    path: Option<String>,
    /// The bytes the view last read, so the workspace can tell its own write
    /// coming back through the watcher from somebody else's.
    source: String,
    /// Which of the base's views is showing, by index into `base.views`.
    view_ix: usize,
    /// The file as it parsed, which is both what the layouts read their own
    /// keys off and what a write serializes.
    base: Option<Base>,
    table: Option<Table>,
    /// The showing view's layout and the keys that layout reads.
    layout: Layout,
    /// A file that does not parse shows this instead of a table, under
    /// [`scoria_formats::base::PARSE_ERROR_BANNER`].
    error: Option<String>,
    state: Option<Arc<VaultState>>,
    /// Set when the view changed the base and the bytes on disk are older.
    dirty: bool,
    /// Drawn inside a note (`![[x.base]]`) rather than filling a leaf: as
    /// tall as its content, no ground of its own, nothing scrolled but the
    /// note. See `bases::embed` for the observation.
    embedded: bool,
    /// Every frontmatter property the vault's notes carry, with the kind its
    /// values infer, from the whole vault rather than the filtered rows: a
    /// view that empties still has to know `density` is a number.
    pub(super) property_kinds: Vec<(String, scoria_bases::rule::PropertyKind)>,
    /// Every distinct value each frontmatter property holds across the
    /// vault, as the property editor displays one, sorted without regard
    /// to case: what a cell's suggestion list offers.
    pub(super) property_values: Vec<(String, Vec<String>)>,
    pub(super) table_state: super::table::TableState,
    pub(super) cards: super::cards::CardsState,
    pub(super) list: super::list::ListLayoutState,
    pub(super) kanban: super::kanban::KanbanState,
    pub(super) toolbar: super::toolbar::ToolbarState,
    pub(super) editing: super::edit::EditState,
    /// The suggestions the open cell's list last drew, in order, so Enter
    /// can take the keyboard's pick without the app context the list needs.
    pub(super) editing_shown: Vec<String>,
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
            layout: Layout::default(),
            error: None,
            state: None,
            dirty: false,
            embedded: false,
            property_kinds: Vec::new(),
            property_values: Vec::new(),
            table_state: super::table::TableState::default(),
            cards: super::cards::CardsState::default(),
            list: super::list::ListLayoutState::default(),
            kanban: super::kanban::KanbanState::default(),
            toolbar: super::toolbar::ToolbarState::default(),
            editing: super::edit::EditState::default(),
            editing_shown: Vec::new(),
        }
    }

    /// Shows `path` with the bytes just read from it. An empty file is a base
    /// with one default view, which is what `bases:new-file` writes.
    pub fn load(&mut self, path: &str, text: &str, state: Arc<VaultState>, cx: &mut Context<Self>) {
        self.path = Some(path.to_string());
        self.source = text.to_string();
        self.state = Some(state);
        // A different file starts on its first view, as opening one does,
        // and carries none of the last one's unsaved state.
        self.view_ix = 0;
        self.dirty = false;
        self.toolbar = super::toolbar::ToolbarState::default();
        self.editing = super::edit::EditState::default();
        self.resolve(cx);
    }

    /// The file changed underneath the view.
    ///
    /// Obsidian reloads the live query from the new bytes and keeps the view
    /// the leaf was on (the external rewrite of the fixture came back with its
    /// renamed view already in the query), so the index is kept here too and
    /// only clamped if the rewrite dropped views. An outside edit is never
    /// clobbered: the view takes the new bytes and its own unsaved change
    /// goes with them.
    pub fn reload(&mut self, text: &str, cx: &mut Context<Self>) {
        self.source = text.to_string();
        self.dirty = false;
        self.editing = super::edit::EditState::default();
        self.resolve(cx);
    }

    /// Lets go of the file, as the canvas does when another kind opens.
    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.path = None;
        self.base = None;
        self.table = None;
        self.layout = Layout::default();
        self.error = None;
        self.source.clear();
        self.state = None;
        self.dirty = false;
        self.toolbar = super::toolbar::ToolbarState::default();
        self.editing = super::edit::EditState::default();
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
        // A base the view has edited but not yet saved must not be re-parsed
        // from the bytes on disk: the edit is in `base`, not in `source`.
        if self.dirty {
            self.resolve_table();
            self.rebuild();
            cx.notify();
            return;
        }
        self.resolve(cx);
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Makes this the view behind an embed: see [`BasesView::embedded`].
    pub fn set_embedded(&mut self) {
        self.embedded = true;
    }

    pub(super) fn is_embedded(&self) -> bool {
        self.embedded
    }

    /// Shows the base's view named `name` — what `![[x.base#Name]]` asks
    /// for — compared the way Obsidian compares its view names, exactly
    /// first and then without case. A name no view has leaves the first
    /// view showing (**unprobed**: the run embedded only names that exist).
    pub fn show_view_named(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(base) = self.base.as_ref() else {
            return;
        };
        let ix = base.views.iter().position(|v| v.name == name).or_else(|| {
            base.views
                .iter()
                .position(|v| v.name.eq_ignore_ascii_case(name))
        });
        if let Some(ix) = ix {
            self.show_view(ix, cx);
        }
    }

    /// Whether the bytes on disk are the ones the view read.
    pub fn matches_file(&self, text: &str) -> bool {
        self.source == text
    }

    /// Whether the view holds a change the disk does not.
    pub fn is_dirty(&self) -> bool {
        self.dirty && self.base.is_some()
    }

    /// The bytes a save writes: the parsed base in Obsidian's canonical form.
    /// `None` when there is nothing to write, which includes every file that
    /// failed to parse — a broken base is never written.
    pub fn file_contents(&self) -> Option<String> {
        self.base.as_ref().map(Base::to_yaml)
    }

    /// The save landed: these are now the bytes on disk, so the watcher event
    /// that follows is the view's own write rather than somebody else's.
    pub fn mark_saved(&mut self, text: String) {
        self.source = text;
        self.dirty = false;
    }

    /// Parses [`BasesView::source`] and resolves the showing view against the
    /// vault. A file that does not parse leaves no table at all, which is what
    /// Obsidian does: the query is the error object, and nothing is drawn but
    /// the banner.
    fn resolve(&mut self, cx: &mut Context<Self>) {
        let resolved = rows::resolve(&self.source);
        self.base = resolved.base;
        self.error = resolved.error;
        // A base with no views has one default view, so index 0 is always
        // valid; anything past the end falls back to it.
        if self.view_ix > 0 && self.view_ix >= self.base.as_ref().map_or(0, |b| b.views.len()) {
            self.view_ix = 0;
        }
        self.resolve_table();
        self.rebuild();
        cx.notify();
    }

    /// Resolves the showing view against the vault from the base already
    /// parsed, which is the path an edit takes: the file on disk is the old
    /// one and must not be re-read.
    fn resolve_table(&mut self) {
        let Some(base) = self.base.as_ref() else {
            self.table = None;
            self.layout = Layout::default();
            return;
        };
        let env = match (self.state.as_ref(), self.path.as_deref()) {
            (Some(state), Some(path)) => rows::env_for(state, path),
            _ => scoria_bases::Env {
                files: Vec::new(),
                base_file: None,
                now: chrono::Local::now().naive_local(),
            },
        };
        let (kinds, values) = infer_properties(&env.files);
        self.property_kinds = kinds;
        self.property_values = values;
        let mut table = scoria_bases::table::build(base, self.view_ix, &env);
        if let Some(needle) = self.toolbar.search.as_deref() {
            apply_search(&mut table, needle);
        }
        self.table = Some(table);
        self.layout = match base.views.get(self.view_ix) {
            Some(view) => Layout::of(view),
            // An empty base is one default table of the whole vault.
            None => Layout::default(),
        };
    }

    /// The toolbar's search row changed: the same query, its rows narrowed
    /// again.
    pub(super) fn resolve_search(&mut self) {
        self.resolve_table();
        self.rebuild();
    }

    /// Rebuilds whichever layout is showing. Only one runs: a kanban's
    /// columns cost real work over a 20 000-note vault, and nothing but the
    /// showing layout is drawn.
    fn rebuild(&mut self) {
        match self.layout {
            Layout::Table(_) | Layout::Unknown(_) => self.rebuild_table(),
            Layout::Cards(_) => self.rebuild_cards(),
            Layout::List(_) => self.rebuild_list(),
            Layout::Kanban(_) => self.rebuild_kanban(),
        }
    }

    // ----- what the layouts read ---------------------------------------------

    /// The view whose rows are showing, when the file parsed and has one.
    pub(super) fn view(&self) -> Option<&View> {
        self.base.as_ref()?.views.get(self.view_ix)
    }

    /// The showing view's index into `base.views`.
    pub(super) fn view_ix(&self) -> usize {
        self.view_ix
    }

    pub(super) fn table(&self) -> Option<&Table> {
        self.table.as_ref()
    }

    pub(super) fn base(&self) -> Option<&Base> {
        self.base.as_ref()
    }

    pub(super) fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(super) fn vault(&self) -> Option<&Arc<VaultState>> {
        self.state.as_ref()
    }
    // ----- what the layouts do -----------------------------------------------

    /// Opens the file a row names, which is the workspace's to do.
    pub(super) fn open(&mut self, path: String, cx: &mut Context<Self>) {
        cx.emit(BasesEvent::Open(path));
    }

    /// Raises the file menu over a row's file at `position`.
    pub(super) fn file_menu(
        &mut self,
        path: String,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        cx.emit(BasesEvent::FileMenu { path, position });
    }

    /// The distinct values a frontmatter property holds across the vault.
    pub(super) fn values_of(&self, key: &str) -> &[String] {
        self.property_values
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// Says something as a Notice.
    pub(super) fn report(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        cx.emit(BasesEvent::Report(message.into()));
    }

    /// Asks the workspace to set a property of the note a row stands for,
    /// which is what a cell typed into does. See
    /// [`BasesEvent::SetProperty`] for why the view does not write it
    /// itself.
    pub(super) fn set_property(
        &mut self,
        path: &str,
        key: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        cx.emit(BasesEvent::SetProperty {
            path: path.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    /// Shows another of the base's views, as the views menu and
    /// `bases:change-view` do. Out of range is ignored rather than clamped:
    /// nothing should silently show a view the caller did not name.
    pub(super) fn show_view(&mut self, ix: usize, cx: &mut Context<Self>) {
        let count = self.base.as_ref().map_or(0, |b| b.views.len());
        if ix >= count.max(1) {
            return;
        }
        self.view_ix = ix;
        self.editing = super::edit::EditState::default();
        self.resolve_table();
        self.rebuild();
        cx.notify();
    }

    /// Changes the showing view and writes the base.
    ///
    /// This is the one way a layout or the toolbar changes the file: the
    /// closure edits the [`View`], the table is resolved again from the
    /// parsed base — never from the bytes on disk, which are still the old
    /// ones — and the workspace is told to save.
    pub(super) fn edit_view<F>(&mut self, edit: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&mut View),
    {
        let ix = self.view_ix;
        self.edit_base(
            move |base| {
                // An empty base has no view to edit: the default view is
                // materialized so the change has somewhere to land, which is
                // also what Obsidian's own close writes.
                if base.views.is_empty() {
                    base.views.push(View {
                        kind: scoria_formats::base::DEFAULT_VIEW_TYPE.to_string(),
                        name: scoria_formats::base::DEFAULT_VIEW_NAME.to_string(),
                        ..View::default()
                    });
                }
                if let Some(view) = base.views.get_mut(ix) {
                    edit(view);
                }
            },
            cx,
        );
    }

    /// Changes the base itself — its own filters, formulas, properties or the
    /// list of views — and writes it.
    pub(super) fn edit_base<F>(&mut self, edit: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&mut Base),
    {
        let Some(base) = self.base.as_mut() else {
            return;
        };
        edit(base);
        self.dirty = true;
        self.resolve_table();
        self.rebuild();
        cx.emit(BasesEvent::Edited);
        cx.notify();
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
                    "cells": r.cells.iter().map(super::cells::cell_text).collect::<Vec<_>>(),
                })
            })
            .collect();
        let groups: Vec<Json> = self
            .table
            .iter()
            .flat_map(|t| t.groups.iter())
            .map(|g| json!({ "key": g.key, "rows": g.rows.len() }))
            .collect();
        let views: Vec<Json> = self
            .base
            .iter()
            .flat_map(|b| b.views.iter())
            .map(|v| json!({ "name": v.name, "type": v.kind }))
            .collect();
        json!({
            "path": self.path,
            "error": self.error,
            "view": self.view_ix,
            "view_name": self.view().map(|v| v.name.clone()),
            "views": views,
            "layout": self.layout.kind(),
            "layout_state": self.layout_state_json(),
            "unknown_kind": self.table.as_ref().and_then(|t| t.unknown_kind.clone()),
            "columns": columns,
            "rows": rows,
            "rows_total": self.table.as_ref().map_or(0, |t| t.rows.len()),
            "limited": self.table.as_ref().is_some_and(|t| t.limited),
            "groups": groups,
            "dirty": self.dirty,
            "embedded": self.embedded,
            "toolbar": self.toolbar.state_json(),
            "editing": self.editing.state_json(),
        })
    }

    /// What the showing layout reports for itself.
    fn layout_state_json(&self) -> Json {
        match self.layout {
            Layout::Table(_) | Layout::Unknown(_) => self.table_state.state_json(),
            Layout::Cards(_) => self.cards.state_json(),
            Layout::List(_) => self.list.state_json(),
            Layout::Kanban(_) => self.kanban.state_json(),
        }
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
            .text_color(theme.tokens.text_normal);
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
                    .text_color(theme.tokens.text_muted)
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

    /// A view that resolved to no rows at all, which every layout draws
    /// through here.
    ///
    /// The **wording is observed**: `BadFilter.base` — whose only filter is
    /// `density >>> 1` — "reports 0 results" with its header row still drawn
    /// and the error banner hidden, so a view that empties says how many
    /// results it has rather than that something went wrong. **Unprobed**:
    /// where that count sits and what it looks like. The box is the shell's
    /// own `pane-empty` — 13 px faint, centred, padded `24px 16px` — which is
    /// what the outline pane draws for the same situation.
    pub(super) fn render_empty(&self, theme: &Theme) -> AnyElement {
        div()
            .flex_1()
            .w_full()
            .flex()
            .items_start()
            .justify_center()
            .py(px(24.))
            .px(px(16.))
            .text_size(px(FONT_UI_SMALL))
            .text_color(theme.tokens.text_faint)
            .child(SharedString::from("0 results"))
            .into_any_element()
    }
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

/// Every frontmatter property with the kind its values infer.
type PropertyKinds = Vec<(String, scoria_bases::rule::PropertyKind)>;
/// Every frontmatter property with the distinct values it holds.
type PropertyValues = Vec<(String, Vec<String>)>;

/// Every property name the notes carry, in first-seen order, with the kind
/// its first non-null value infers — what Obsidian does for a property no
/// one assigned a type to in `types.json` — and, per property, the distinct
/// values it holds, sorted without regard to case (observed: a `kinds`
/// cell's suggestions read `ash`, `basalt`, `felsic`, `frothy`, `glass`,
/// `intermediate`). A list property's items are values of their own.
fn infer_properties(files: &[Arc<scoria_bases::FileData>]) -> (PropertyKinds, PropertyValues) {
    use scoria_bases::rule::PropertyKind;
    use scoria_bases::Value;
    let mut kinds: Vec<(String, PropertyKind)> = Vec::new();
    let mut values: Vec<(String, Vec<String>)> = Vec::new();
    for file in files {
        for (name, value) in &file.properties {
            let kind = match value {
                Value::Bool(_) => Some(PropertyKind::Checkbox),
                Value::Number(_) => Some(PropertyKind::Number),
                Value::List(_) | Value::Tags(_) => Some(PropertyKind::List),
                Value::Date(d) if d.has_time => Some(PropertyKind::DateTime),
                Value::Date(_) => Some(PropertyKind::Date),
                Value::Str(_) | Value::Link(_) => Some(PropertyKind::Text),
                _ => None,
            };
            if let Some(kind) = kind {
                match kinds.iter().position(|(n, _)| n == name) {
                    Some(ix) => {
                        if kinds[ix].1 == PropertyKind::Unknown {
                            kinds[ix].1 = kind;
                        }
                    }
                    None => kinds.push((name.clone(), kind)),
                }
            }
            let texts: Vec<String> = match value {
                Value::Null | Value::Bool(_) => Vec::new(),
                Value::List(items) => items.iter().map(|v| v.to_string()).collect(),
                Value::Tags(tags) => tags.clone(),
                other => vec![other.to_string()],
            };
            if texts.is_empty() {
                continue;
            }
            let slot = match values.iter().position(|(n, _)| n == name) {
                Some(ix) => ix,
                None => {
                    values.push((name.clone(), Vec::new()));
                    values.len() - 1
                }
            };
            for text in texts {
                if !text.is_empty() && !values[slot].1.contains(&text) {
                    values[slot].1.push(text);
                }
            }
        }
    }
    for (_, list) in &mut values {
        list.sort_by_key(|v| v.to_lowercase());
    }
    (kinds, values)
}

/// The toolbar's search row: keeps the rows whose displayed cells hold the
/// query, case-insensitively, and drops the groups that empties.
///
/// Observed only as far as the row's own words go — "search for items using
/// their displayed properties" and a *Showing N* count beside the field —
/// so what counts as a match is a reading: any cell's text.
fn apply_search(table: &mut Table, needle: &str) {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return;
    }
    let hit = |row: &scoria_bases::Row| {
        row.cells
            .iter()
            .any(|c| c.text.to_lowercase().contains(&needle))
            || row.file.name.to_lowercase().contains(&needle)
    };
    table.rows.retain(|r| hit(r));
    for group in &mut table.groups {
        group.rows.retain(|r| hit(r));
    }
    table.groups.retain(|g| !g.rows.is_empty());
}

impl Render for BasesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let body: Vec<AnyElement> = if let Some(message) = self.error.clone() {
            vec![self.render_error(true, &message, &theme)]
        } else if let Some(kind) = self.table.as_ref().and_then(|t| t.unknown_kind.clone()) {
            // The file parsed; only the layout is one this build cannot draw.
            vec![self.render_error(false, &format!("Unknown view type: {kind}"), &theme)]
        } else if self.table.is_some() {
            let mut body = Vec::new();
            body.extend(self.render_toolbar(&theme, window, cx));
            body.extend(match self.layout.clone() {
                Layout::Cards(_) => self.render_cards(&theme, cx),
                Layout::List(_) => self.render_list(&theme, cx),
                Layout::Kanban(_) => self.render_kanban(&theme, cx),
                // An unknown type never reaches here; it is the branch above.
                Layout::Table(_) | Layout::Unknown(_) => self.render_table(&theme, cx),
            });
            body
        } else {
            Vec::new()
        };
        let embedded = self.embedded;
        // The suggestion list hangs under the open cell's box, which is
        // known only after the cell has painted once: ask for that frame.
        if self.editing.cell.is_some()
            && self.editing.suggestions
            && !self.editing.frame_asked
            && self.anchor(super::edit::CELL_KEY).is_none()
        {
            self.editing.frame_asked = true;
            let entity = cx.entity();
            window.on_next_frame(move |_, cx| entity.update(cx, |_, cx| cx.notify()));
        }
        div()
            .track_focus(&self.focus)
            .key_context("Bases")
            .on_action(cx.listener(Self::close_panel_action))
            .on_action(cx.listener(Self::select_up_action))
            .on_action(cx.listener(Self::select_down_action))
            .on_action(cx.listener(Self::activate_action))
            .on_action(cx.listener(Self::cursor_left_action))
            .on_action(cx.listener(Self::cursor_right_action))
            .on_action(cx.listener(Self::cursor_next_action))
            .on_action(cx.listener(Self::cursor_prev_action))
            .on_action(cx.listener(Self::delete_cell_action))
            // Typing on the cursor opens the cell with the character: the
            // keys that are not a binding reach here with their `key_char`.
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let keystroke = &event.keystroke;
                if keystroke.modifiers.control
                    || keystroke.modifiers.alt
                    || keystroke.modifiers.platform
                {
                    return;
                }
                let Some(text) = keystroke.key_char.as_deref() else {
                    return;
                };
                if text.chars().any(|c| c.is_control()) || !this.cursor_takes_keys() {
                    return;
                }
                this.type_into_focused(text, window, cx);
                cx.stop_propagation();
            }))
            .flex()
            .flex_col()
            // A leaf's view fills the leaf on its own ground; an embed is
            // as tall as its content on the note's ground, clipped at the
            // note's edge where its table is wider.
            .when(!embedded, |d| {
                d.size_full().bg(theme.tokens.background_primary)
            })
            .when(embedded, |d| d.w_full().overflow_hidden())
            .text_color(theme.tokens.text_normal)
            .children(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_that_does_not_parse_has_a_message_and_no_table() {
        // The parse itself is `scoria-formats`'s; what matters here is that
        // the shell keeps the message and draws no rows for it.
        let resolved = rows::resolve("views:\n  - type: table\n");
        assert!(resolved.error.is_some(), "a view with no name is refused");
        assert!(resolved.base.is_none());
    }

    fn no_vault() -> scoria_bases::Env {
        scoria_bases::Env {
            files: Vec::new(),
            base_file: None,
            now: chrono::Local::now().naive_local(),
        }
    }

    #[test]
    fn an_empty_file_is_one_default_table_of_the_whole_vault() {
        let resolved = rows::resolve("");
        assert!(resolved.error.is_none());
        let base = resolved.base.expect("an empty file parses");
        assert!(base.views.is_empty(), "the default view is not in the file");
        let table = scoria_bases::table::build(&base, 0, &no_vault());
        assert_eq!(table.unknown_kind, None);
    }

    #[test]
    fn an_unknown_type_resolves_to_no_rows_and_names_itself() {
        let resolved = rows::resolve("views:\n  - type: nonsense\n    name: V\n");
        let base = resolved.base.expect("the file parses");
        let table = scoria_bases::table::build(&base, 0, &no_vault());
        assert_eq!(table.unknown_kind.as_deref(), Some("nonsense"));
        assert!(table.rows.is_empty());
    }
}
