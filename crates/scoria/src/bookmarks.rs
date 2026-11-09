//! The Bookmarks pane: `.obsidian/bookmarks.json` as a tree in the left
//! dock, drawn to Obsidian 1.15.0's pane as the 2026-11-07 probe measured it
//! (`tools/obsidian-probe/results/2026-11-07-bookmarks-pane.json`, the
//! screenshots beside it, and
//! [[Specification#Observed bookmarks pane, Obsidian 1.15.0]]).
//!
//! The header is the dock panes' own `nav-header` (Bookmark the active
//! tab..., New group, Collapse all / Expand all, Show search filter) and
//! every row is the shared `tree-item-self` ([`crate::tree::row`]) with a
//! Lucide icon per kind in its gutter, a group's fold triangle in its place,
//! and the title wrapped rather than cut. A bookmark whose file is not in the
//! vault is not drawn at all. A row renames in place, drags to a new place
//! or into a group, and opens the menu its kind has; the filter keeps the
//! rows whose title has the query in it, with the groups above them.
//!
//! The pane only shows and asks: the workspace owns `bookmarks.json`, so
//! every change comes out as a [`BookmarkEvent`] and comes back through
//! [`BookmarksPane::set_items`].

use crate::explorer::FileDrag;
use crate::file_menu::{self, BookmarkKind, Pick, Target};
use crate::inline_field;
use crate::text_input::{InputEvent, TextInput};
use crate::theme::Theme;
use crate::tree;
use crate::workspace::{TreeActivate, TreeDown, TreeLeft, TreeRight, TreeUp};
use gpui::{
    canvas, div, prelude::*, px, Anchor, App, Bounds, Context, DragMoveEvent, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, MouseButton, MouseDownEvent,
    Pixels, Point, SharedString, Subscription, UTF16Selection, Window,
};
use scoria_formats::bookmarks::Bookmark;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::ops::Range;

pub enum BookmarkEvent {
    /// A file bookmark: the note, scrolled to its `#Heading` or `#^block`;
    /// in a new tab for Ctrl, the middle button and "Open in new tab".
    Open {
        path: String,
        subpath: Option<String>,
        new_tab: bool,
    },
    /// "Open to the right" on a file bookmark: the note in a new split.
    OpenToRight(String),
    /// A search bookmark: the Search pane with this query.
    Search(String),
    Graph,
    /// A folder bookmark: shown in the Files pane.
    Folder(String),
    Url(String),
    /// "Reveal file in navigation".
    Reveal(String),
    /// "Bookmark the active tab...", at the top level or into a group.
    AddCurrent {
        into: Vec<usize>,
    },
    /// "New group": at the top level or inside a group. The workspace adds
    /// it and hands the row back to [`BookmarksPane::start_rename`].
    NewGroup {
        into: Vec<usize>,
    },
    /// "Edit...": the "Edit bookmark" modal on this item.
    Edit(Vec<usize>),
    /// Remove the item at this index path (indexes into groups).
    Remove(Vec<usize>),
    /// A title typed on the row.
    SetTitle {
        at: Vec<usize>,
        title: String,
    },
    /// A drop: the item at `from` to the insertion path `to`, read against
    /// the list before the move.
    Move {
        from: Vec<usize>,
        to: Vec<usize>,
    },
    /// A row of the Files view dropped here: a new `file` (or `folder`)
    /// item at the insertion path `to` (2026-11-07 probe, `s16`).
    AddPath {
        path: String,
        folder: bool,
        to: Vec<usize>,
    },
    /// The filter was shown, hidden or typed into: `workspace.json` holds
    /// it as the leaf's `showSearch` and `searchQuery`.
    LeafState,
}

/// One visible row: the item, where it sits, and how deep.
#[derive(Debug, Clone)]
pub struct Row {
    pub at: Vec<usize>,
    /// Stable across moves, unlike `at`: the `ctime`s down to the item and
    /// its title, which is what a fold and the keyboard's ring follow.
    pub key: String,
    pub depth: usize,
    pub item: Bookmark,
    pub collapsed: bool,
    pub has_children: bool,
}

/// The rows the pane draws, in order.
///
/// A `file` item whose path is in `missing` is skipped: Obsidian keeps its
/// row in the DOM with `display: none` (2026-11-07, `s02`, `Missing.md`).
/// With an empty `query`, a folded group hides its members. With one, a row
/// is kept when its title holds the query, ignoring case, and a group when
/// any member is kept — the filter showed `Group one` over `Beta` for
/// `beta`, and `Group one` › `Nested group` › `Delta` for `del` (`s09`);
/// folds are not consulted while filtering, which the probe did not test.
pub fn visible_rows(
    items: &[Bookmark],
    collapsed: &HashSet<String>,
    query: &str,
    missing: &HashSet<String>,
) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    #[allow(clippy::too_many_arguments)]
    fn walk(
        items: &[Bookmark],
        at: &mut Vec<usize>,
        parent_key: &str,
        depth: usize,
        collapsed: &HashSet<String>,
        query: &str,
        missing: &HashSet<String>,
        out: &mut Vec<Row>,
    ) -> bool {
        let mut any = false;
        for (ix, item) in items.iter().enumerate() {
            if item.kind() == "file" && item.path().is_some_and(|p| missing.contains(p)) {
                continue;
            }
            at.push(ix);
            let key = format!("{parent_key}/{}:{}", item.ctime(), item.display_title());
            let is_group = item.kind() == "group";
            let members = if is_group { item.items() } else { Vec::new() };
            let folded = query.is_empty() && collapsed.contains(&key);
            let row_ix = out.len();
            out.push(Row {
                at: at.clone(),
                key: key.clone(),
                depth,
                item: item.clone(),
                collapsed: folded,
                has_children: !members.is_empty(),
            });
            let mut kept_member = false;
            if is_group && !folded {
                kept_member = walk(
                    &members,
                    at,
                    &key,
                    depth + 1,
                    collapsed,
                    query,
                    missing,
                    out,
                );
            }
            let matches = query.is_empty()
                || item.display_title().to_lowercase().contains(query)
                || kept_member;
            if !matches {
                out.truncate(row_ix);
            }
            any |= matches;
            at.pop();
        }
        any
    }
    let mut out = Vec::new();
    walk(
        items,
        &mut Vec::new(),
        "",
        0,
        collapsed,
        &query,
        missing,
        &mut out,
    );
    out
}

/// The Lucide icon Obsidian draws in a row's gutter, by kind (2026-11-07,
/// `s02`): a heading and a block are file items with a subpath.
pub fn kind_icon(item: &Bookmark) -> &'static str {
    match item.kind() {
        "file" => match item.subpath() {
            Some(s) if s.starts_with("#^") => "toy-brick",
            Some(_) => "heading",
            None => "file",
        },
        "folder" => "folder",
        "search" => "search",
        "graph" => "git-fork",
        "url" => "globe-2",
        _ => "file",
    }
}

fn menu_kind(item: &Bookmark) -> BookmarkKind {
    match item.kind() {
        "folder" => BookmarkKind::Folder,
        "search" => BookmarkKind::Search,
        "url" => BookmarkKind::Url,
        "graph" => BookmarkKind::Graph,
        "group" => BookmarkKind::Group,
        _ => BookmarkKind::File,
    }
}

/// Where a drop over a row puts the dragged item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Before,
    After,
    /// Onto a group: its first member (where `Image.png` landed, `s15`).
    Into,
}

/// Which part of a row the pointer is over, `y` from the row's top. A
/// group takes its middle half as "into" and a quarter at either edge as a
/// place beside it; any other row splits at its middle. The probe dropped
/// at the top edge and the centre only; the edges' widths are a choice.
pub fn drop_zone(y: f32, height: f32, group: bool) -> Zone {
    if group {
        if y < height / 4. {
            Zone::Before
        } else if y > height * 3. / 4. {
            Zone::After
        } else {
            Zone::Into
        }
    } else if y < height / 2. {
        Zone::Before
    } else {
        Zone::After
    }
}

/// The insertion path a drop on the row at `at` names.
pub fn insertion_path(at: &[usize], zone: Zone) -> Vec<usize> {
    let mut to = at.to_vec();
    match zone {
        Zone::Before => {}
        Zone::After => {
            if let Some(last) = to.last_mut() {
                *last += 1;
            }
        }
        Zone::Into => to.push(0),
    }
    to
}

/// What a drag carries: the item and its title for the ghost.
#[derive(Debug, Clone)]
pub struct BookmarkDrag {
    at: Vec<usize>,
    key: String,
    title: SharedString,
}

/// The ghost under the pointer: Obsidian's `drag-ghost`, the title in
/// white on `--drag-ghost-background`, 300 px wide (`s14`).
struct Ghost(SharedString, Point<Pixels>);

impl Render for Ghost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let grab = self.1;
        div().pl(grab.x + px(5.)).pt(grab.y + px(5.)).child(
            div()
                .w(px(300.))
                .px(px(8.))
                .py(px(6.))
                .rounded(px(4.))
                .bg(gpui::hsla(0., 0., 0., 0.85))
                .shadow_md()
                .text_size(px(crate::theme::FONT_UI_SMALL))
                .text_color(gpui::white())
                .child(self.0.clone()),
        )
    }
}

/// A row whose title is being typed into.
struct RowEdit {
    key: String,
    at: Vec<usize>,
    /// What the row showed, so an unchanged or emptied title writes nothing.
    was: String,
    editing: inline_field::Editing,
    layout: Option<inline_field::Layout>,
    /// The field has had the keyboard, so losing it is a commit.
    had_focus: bool,
}

pub struct BookmarksPane {
    items: Vec<Bookmark>,
    /// Paths of `file` items that are not in the vault.
    missing: HashSet<String>,
    /// Keys of the groups folded shut.
    collapsed: HashSet<String>,
    /// The open note, whose row is marked.
    active: Option<String>,
    show_search: bool,
    filter: Entity<TextInput>,
    focus_handle: FocusHandle,
    /// The row with the keyboard's ring (Obsidian's `has-focus`).
    focused: Option<String>,
    menu: Option<file_menu::Menu>,
    /// The index path the open menu acts on (the pane's empty space: none).
    menu_at: Vec<usize>,
    /// The row the open menu was raised on, which wears the ring while it
    /// is up (Obsidian's `has-active-menu`, `s06`).
    menu_key: Option<String>,
    menu_focus: file_menu::MenuFocus,
    renaming: Option<RowEdit>,
    rename_focus: file_menu::MenuFocus,
    /// The row being dragged, and the drop the pointer is over.
    dragging: Option<String>,
    drop: Option<(String, Zone)>,
    /// Where the pointer was when a row last claimed the drop, so the
    /// list's own handler can clear a target the pointer has left; and
    /// whether a row has already taken the drop, so the list does not act
    /// on the same release again.
    drop_claim: Option<Point<Pixels>>,
    drop_taken: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<BookmarkEvent> for BookmarksPane {}

impl Focusable for BookmarksPane {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BookmarksPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> BookmarksPane {
        let filter = cx.new(tree::filter_input);
        let subscription = cx.subscribe_in(
            &filter,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Changed => {
                    cx.emit(BookmarkEvent::LeafState);
                    cx.notify();
                }
                // Escape empties the field and hides it (`s09`).
                InputEvent::Cancel => this.hide_search(window, cx),
                InputEvent::Submit | InputEvent::Up | InputEvent::Down => {}
            },
        );
        BookmarksPane {
            items: Vec::new(),
            missing: HashSet::new(),
            collapsed: HashSet::new(),
            active: None,
            show_search: false,
            filter,
            focus_handle: cx.focus_handle(),
            focused: None,
            menu: None,
            menu_at: Vec::new(),
            menu_key: None,
            menu_focus: file_menu::MenuFocus::new(cx),
            renaming: None,
            rename_focus: file_menu::MenuFocus::new(cx),
            dragging: None,
            drop: None,
            drop_claim: None,
            drop_taken: false,
            _subscriptions: vec![subscription],
        }
    }

    /// The items, and which file items point at nothing in the vault.
    pub fn set_items(
        &mut self,
        items: Vec<Bookmark>,
        missing: HashSet<String>,
        cx: &mut Context<Self>,
    ) {
        self.items = items;
        self.missing = missing;
        // A field on a row that has gone would hold the keyboard with
        // nothing on screen.
        if let Some(edit) = &self.renaming {
            if !self.rows(cx).iter().any(|r| r.key == edit.key) {
                self.renaming = None;
            }
        }
        cx.notify();
    }

    pub fn set_active(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        if self.active != path {
            self.active = path;
            cx.notify();
        }
    }

    /// The leaf's state from `workspace.json`.
    pub fn restore(&mut self, state: &Value, cx: &mut Context<Self>) {
        self.show_search = state
            .get("showSearch")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let query = state
            .get("searchQuery")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.filter.update(cx, |f, cx| f.set_text(query, cx));
        cx.notify();
    }

    /// The leaf's state for `workspace.json`.
    pub fn leaf_state(&self, cx: &App) -> Vec<(&'static str, Value)> {
        vec![
            ("showSearch", json!(self.show_search)),
            ("searchQuery", json!(self.filter.read(cx).text())),
        ]
    }

    /// Opens a field on the row at `at` with its title selected: the
    /// menu's Rename, and a group just made by "New group" (`s12`).
    pub fn start_rename(&mut self, at: &[usize], cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let Some(row) = rows.iter().find(|r| r.at == at) else {
            return;
        };
        let title = row.item.display_title();
        self.focused = Some(row.key.clone());
        self.renaming = Some(RowEdit {
            key: row.key.clone(),
            at: row.at.clone(),
            was: title.clone(),
            editing: inline_field::Editing::selected(title),
            layout: None,
            had_focus: false,
        });
        cx.notify();
    }

    fn query(&self, cx: &App) -> String {
        if self.show_search {
            self.filter.read(cx).text().to_string()
        } else {
            String::new()
        }
    }

    fn rows(&self, cx: &App) -> Vec<Row> {
        visible_rows(&self.items, &self.collapsed, &self.query(cx), &self.missing)
    }

    /// Every group's key, folded or not.
    fn group_keys(&self) -> Vec<String> {
        visible_rows(&self.items, &HashSet::new(), "", &self.missing)
            .into_iter()
            .filter(|r| r.item.kind() == "group")
            .map(|r| r.key)
            .collect()
    }

    /// The header's fold button reads "Expand all" once every group is
    /// folded, and "Collapse all" otherwise (`s07`).
    fn all_collapsed(&self) -> bool {
        let groups = self.group_keys();
        !groups.is_empty() && groups.iter().all(|k| self.collapsed.contains(k))
    }

    fn toggle_all(&mut self, cx: &mut Context<Self>) {
        if self.all_collapsed() {
            self.collapsed.clear();
        } else {
            self.collapsed = self.group_keys().into_iter().collect();
        }
        cx.notify();
    }

    fn toggle_fold(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.collapsed.remove(key) {
            self.collapsed.insert(key.to_string());
        }
        cx.notify();
    }

    fn toggle_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_search {
            self.hide_search(window, cx);
        } else {
            self.show_search = true;
            let handle = self.filter.focus_handle(cx);
            window.focus(&handle, cx);
            cx.emit(BookmarkEvent::LeafState);
            cx.notify();
        }
    }

    fn hide_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_search = false;
        self.filter.update(cx, |f, cx| f.set_text("", cx));
        window.focus(&self.focus_handle, cx);
        cx.emit(BookmarkEvent::LeafState);
        cx.notify();
    }

    fn clear_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |f, cx| f.set_text("", cx));
        let handle = self.filter.focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// A click on a row (Enter on the ringed one): a group folds, anything
    /// else opens, in a new tab when asked.
    fn activate(&mut self, row: &Row, new_tab: bool, cx: &mut Context<Self>) {
        let item = &row.item;
        match item.kind() {
            "group" => self.toggle_fold(&row.key, cx),
            "file" => {
                if let Some(path) = item.path() {
                    cx.emit(BookmarkEvent::Open {
                        path: path.to_string(),
                        subpath: item.subpath().map(str::to_string),
                        new_tab,
                    });
                }
            }
            "search" => cx.emit(BookmarkEvent::Search(
                item.query().unwrap_or("").to_string(),
            )),
            "graph" => cx.emit(BookmarkEvent::Graph),
            "folder" => cx.emit(BookmarkEvent::Folder(item.path().unwrap_or("").to_string())),
            "url" => {
                if let Some(url) = item.url() {
                    cx.emit(BookmarkEvent::Url(url.to_string()));
                }
            }
            _ => {}
        }
    }

    fn tree_key(&mut self, key: crate::panes::NavKey, cx: &mut Context<Self>) {
        use crate::panes::{NavMove, NavRow};
        let rows = self.rows(cx);
        let nav: Vec<NavRow> = rows
            .iter()
            .map(|r| NavRow {
                depth: r.depth,
                has_children: r.has_children,
                collapsed: r.collapsed,
            })
            .collect();
        let focused = self
            .focused
            .as_ref()
            .and_then(|k| rows.iter().position(|r| &r.key == k));
        match crate::panes::tree_key(&nav, focused, key) {
            Some(NavMove::Focus(ix)) => self.focused = Some(rows[ix].key.clone()),
            Some(NavMove::Fold(ix) | NavMove::Unfold(ix)) => {
                let key = rows[ix].key.clone();
                self.toggle_fold(&key, cx);
            }
            None => {}
        }
        cx.notify();
    }

    fn tree_activate(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if let Some(row) = self
            .focused
            .as_ref()
            .and_then(|k| rows.iter().find(|r| &r.key == k))
        {
            self.activate(row, false, cx);
        }
    }

    fn open_menu(&mut self, row: Option<&Row>, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (kind, path, at) = match row {
            Some(row) => (
                menu_kind(&row.item),
                row.item.path().unwrap_or("").to_string(),
                row.at.clone(),
            ),
            None => (BookmarkKind::Blank, String::new(), Vec::new()),
        };
        self.menu_at = at;
        self.menu_key = row.map(|r| r.key.clone());
        self.menu = Some(file_menu::Menu {
            position,
            submenu: false,
            anchor: Anchor::TopLeft,
            target: Target::Bookmark { kind, path },
        });
        cx.notify();
    }

    fn picked(&mut self, pick: Pick, cx: &mut Context<Self>) {
        let at = self.menu_at.clone();
        let item = item_at(&self.items, &at);
        let file = item.as_ref().and_then(|i| {
            (i.kind() == "file").then(|| {
                (
                    i.path().unwrap_or("").to_string(),
                    i.subpath().map(str::to_string),
                )
            })
        });
        match pick {
            Pick::OpenInNewTab => match (&item, file) {
                (_, Some((path, subpath))) => cx.emit(BookmarkEvent::Open {
                    path,
                    subpath,
                    new_tab: true,
                }),
                (Some(i), None) if i.kind() == "graph" => cx.emit(BookmarkEvent::Graph),
                // A group opens every note in it, each in a tab of its own.
                (Some(i), None) if i.kind() == "group" => {
                    for (path, subpath) in group_files(i) {
                        cx.emit(BookmarkEvent::Open {
                            path,
                            subpath,
                            new_tab: true,
                        });
                    }
                }
                _ => {}
            },
            Pick::OpenToRight => match (&item, file) {
                (_, Some((path, _))) => cx.emit(BookmarkEvent::OpenToRight(path)),
                (Some(i), None) if i.kind() == "group" => {
                    for (path, _) in group_files(i) {
                        cx.emit(BookmarkEvent::OpenToRight(path));
                    }
                }
                (Some(i), None) if i.kind() == "graph" => cx.emit(BookmarkEvent::Graph),
                _ => {}
            },
            Pick::Rename => self.start_rename(&at, cx),
            Pick::EditBookmark => cx.emit(BookmarkEvent::Edit(at)),
            Pick::RevealInNavigation => {
                if let Some(path) = item.as_ref().and_then(|i| i.path()) {
                    cx.emit(BookmarkEvent::Reveal(path.to_string()));
                }
            }
            Pick::RemoveBookmark => cx.emit(BookmarkEvent::Remove(at)),
            Pick::BookmarkActiveTab => cx.emit(BookmarkEvent::AddCurrent { into: at }),
            Pick::NewBookmarkGroup => cx.emit(BookmarkEvent::NewGroup { into: at }),
            _ => {}
        }
    }

    /// What the pane shows, for the control socket's state dump.
    pub fn describe(&self, rows: usize, cx: &App) -> Value {
        let all = self.rows(cx);
        let total = all.len();
        let listed: Vec<Value> = all
            .iter()
            .take(rows)
            .map(|r| {
                json!({
                    "at": r.at,
                    "depth": r.depth,
                    "kind": r.item.kind(),
                    "icon": (r.item.kind() != "group").then(|| kind_icon(&r.item)),
                    "title": r.item.display_title(),
                    "path": r.item.path(),
                    "subpath": r.item.subpath(),
                    "query": r.item.query(),
                    "collapsed": (r.item.kind() == "group").then_some(r.collapsed),
                    "active": self.is_active(&r.item),
                    "focused": self.focused.as_ref() == Some(&r.key),
                })
            })
            .collect();
        json!({
            "count": self.items.len(),
            "rows": listed,
            "rows_total": (total > rows).then_some(total),
            "show_search": self.show_search,
            "query": self.filter.read(cx).text(),
            "all_collapsed": self.all_collapsed(),
            "renaming": self.renaming.as_ref().map(|e| json!({
                "at": e.at,
                "text": e.editing.field.text(),
                "field": e.layout.as_ref().map(|l| json!({
                    "x": f32::from(l.bounds.origin.x),
                    "width": f32::from(l.bounds.size.width),
                    "scroll_x": f32::from(l.scroll_x),
                })),
            })),
            "menu": self.menu.as_ref().map(file_menu::describe),
        })
    }

    /// The open note's own row: a `file` item with no subpath (`s05`: the
    /// heading and block rows of the open note stayed unmarked).
    fn is_active(&self, item: &Bookmark) -> bool {
        item.kind() == "file"
            && item.subpath().is_none()
            && item
                .path()
                .is_some_and(|p| self.active.as_deref() == Some(p))
    }

    // ----- the inline title ------------------------------------------------------

    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.renaming.take() else {
            return;
        };
        // An emptied or unchanged title writes nothing (`s11b`).
        let title = edit.editing.field.text().trim().to_string();
        if !title.is_empty() && title != edit.was {
            cx.emit(BookmarkEvent::SetTitle { at: edit.at, title });
        }
        cx.notify();
    }

    /// The open field, laid over the row's label: absolute against the
    /// row from where its text starts (24 px, and 17 more per level) to its
    /// right padding, since a field painted on a canvas has no width of its
    /// own to give the label's box.
    fn render_rename_field(
        &self,
        ix: usize,
        depth: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let read_entity = cx.entity();
        let paint_entity = cx.entity();
        let read_focus = self.rename_focus.handle.clone();
        let paint_focus = self.rename_focus.handle.clone();
        let color = theme.tokens.nav_item_color_active;
        let caret_color = theme.tokens.text_normal;
        let mut selection_color = theme.tokens.interactive_accent;
        selection_color.a = if theme.dark { 0.35 } else { 0.25 };
        let field = canvas(
            move |bounds, window, cx| {
                let pane = read_entity.read(cx);
                let edit = pane.renaming.as_ref();
                let text: SharedString = edit
                    .map(|e| e.editing.field.text().to_string())
                    .unwrap_or_default()
                    .into();
                let scroll_x = edit
                    .and_then(|e| e.layout.as_ref())
                    .map(|l| l.scroll_x)
                    .unwrap_or(px(0.));
                inline_field::shape(
                    text,
                    edit.map(|e| &e.editing),
                    color,
                    read_focus.is_focused(window),
                    bounds,
                    scroll_x,
                    window,
                )
            },
            move |bounds, prepaint: inline_field::Prepaint, window, cx| {
                window.handle_input(
                    &paint_focus,
                    ElementInputHandler::new(bounds, paint_entity.clone()),
                    cx,
                );
                let layout =
                    inline_field::paint(prepaint, bounds, selection_color, caret_color, window, cx);
                paint_entity.update(cx, |pane, _| {
                    if let Some(edit) = pane.renaming.as_mut() {
                        edit.layout = Some(layout);
                    }
                });
            },
        )
        .size_full();
        div()
            .id(("bookmark-rename", ix))
            .absolute()
            .left(px(24. + 17. * depth as f32))
            .right(px(8.))
            .top(px(4.))
            .h(px(17.))
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.rename_clicked(event, window, cx);
                }),
            )
            .child(field)
            .into_any_element()
    }

    fn rename_clicked(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.renaming.as_mut() else {
            return;
        };
        let Some(layout) = edit.layout.clone() else {
            return;
        };
        let Some(at) = layout.offset_for(event.position, edit.editing.field.text()) else {
            return;
        };
        edit.editing
            .field
            .click(at, event.click_count, event.modifiers.shift);
        window.focus(&self.rename_focus.handle, cx);
        cx.notify();
    }

    // ----- drawing ---------------------------------------------------------------

    fn render_header(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let (fold_icon, fold_label) = if self.all_collapsed() {
            ("chevrons-up-down", "Expand all")
        } else {
            ("chevrons-down-up", "Collapse all")
        };
        tree::header()
            .child(
                tree::nav_button(
                    "bookmark-add",
                    "bookmark-plus",
                    "Bookmark the active tab...",
                    false,
                    theme,
                )
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(BookmarkEvent::AddCurrent { into: Vec::new() })
                })),
            )
            .child(
                tree::nav_button("bookmark-group", "folder-plus", "New group", false, theme)
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(BookmarkEvent::NewGroup { into: Vec::new() })
                    })),
            )
            .child(
                tree::nav_button("bookmark-collapse", fold_icon, fold_label, false, theme)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx))),
            )
            .child(
                tree::nav_button(
                    "bookmark-search",
                    "search",
                    "Show search filter",
                    self.show_search,
                    theme,
                )
                .on_click(cx.listener(|this, _, window, cx| this.toggle_search(window, cx))),
            )
    }

    fn render_row(
        &self,
        ix: usize,
        row: Row,
        keyboard: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let t = &theme.tokens;
        let is_group = row.item.kind() == "group";
        let renaming = self.renaming.as_ref().is_some_and(|e| e.key == row.key);
        let dragged = self.dragging.as_ref() == Some(&row.key);
        let drop = self
            .drop
            .as_ref()
            .filter(|(k, _)| *k == row.key)
            .map(|(_, z)| *z);
        let title: SharedString = row.item.display_title().into();
        // While the field is open the label keeps the row's height and the
        // field is laid over it.
        let label = if renaming {
            div().h(px(17.)).into_any_element()
        } else {
            tree::wrapped_label(title.clone())
        };
        let field = renaming.then(|| self.render_rename_field(ix, row.depth, theme, cx));
        let fold = is_group.then(|| {
            let key = row.key.clone();
            let action: tree::FoldAction<Self> =
                Box::new(move |this: &mut Self, cx: &mut Context<Self>| this.toggle_fold(&key, cx));
            (row.collapsed, action)
        });
        let icon_name = (!is_group).then(|| kind_icon(&row.item));
        let menu_row_open = self.menu.is_some() && self.menu_key.as_ref() == Some(&row.key);
        let ring =
            !renaming && (menu_row_open || keyboard && self.focused.as_ref() == Some(&row.key));
        let row = std::rc::Rc::new(row);
        let (click_row, middle_row, menu_row, move_row, drop_row) = (
            row.clone(),
            row.clone(),
            row.clone(),
            row.clone(),
            row.clone(),
        );
        let (file_move_row, file_drop_row) = (row.clone(), row.clone());
        let mut el = tree::row(
            ("bookmark", ix),
            row.depth,
            fold,
            icon_name,
            label,
            None,
            self.is_active(&row.item),
            ring,
            theme,
            cx,
        );
        if dragged {
            // `is-selected is-being-dragged`: the accent fill, white text.
            el = el.bg(t.interactive_accent).text_color(gpui::white());
        }
        if drop == Some(Zone::Into) {
            // `is-being-dragged-over`: the accent at a tenth.
            let mut wash = t.interactive_accent;
            wash.a = 0.1;
            el = el.bg(wash);
        }
        if let Some(field) = field {
            el = el.child(field);
        }
        if renaming {
            // `is-being-renamed`: a 2 px accent ring outside the row.
            el = el.child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .left(px(-2.))
                    .right(px(-2.))
                    .bottom(px(-2.))
                    .border_2()
                    .border_color(t.interactive_accent)
                    .rounded(px(10.)),
            );
        }
        if let Some(zone @ (Zone::Before | Zone::After)) = drop {
            // `drop-indicator`: a 4 px accent bar on the edge, 2 px of
            // border each side.
            let bar = div()
                .absolute()
                .left(px(0.))
                .right(px(-24.))
                .h(px(4.))
                .bg(t.interactive_accent)
                .rounded(px(2.));
            el = el.child(if zone == Zone::Before {
                bar.top(px(-3.))
            } else {
                bar.bottom(px(-5.))
            });
        }
        if !renaming {
            el = el
                .on_click(
                    cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                        let m = event.modifiers();
                        this.focused = Some(click_row.key.clone());
                        window.focus(&this.focus_handle, cx);
                        this.activate(&click_row, m.control || m.platform, cx)
                    }),
                )
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, _, cx| this.activate(&middle_row, true, cx)),
                )
                .on_drag(
                    BookmarkDrag {
                        at: row.at.clone(),
                        key: row.key.clone(),
                        title: title.clone(),
                    },
                    |drag, grab, _, cx| cx.new(|_| Ghost(drag.title.clone(), grab)),
                );
        }
        el.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.open_menu(Some(&menu_row), event.position, cx)
            }),
        )
        .on_drag_move(
            cx.listener(move |this, event: &DragMoveEvent<BookmarkDrag>, _, cx| {
                if !event.bounds.contains(&event.event.position) {
                    return;
                }
                let drag = event.drag(cx).clone();
                this.dragging = Some(drag.key.clone());
                let y = f32::from(event.event.position.y - event.bounds.top());
                let height = f32::from(event.bounds.size.height);
                let zone = drop_zone(y, height, move_row.item.kind() == "group");
                // Nothing drops on itself or inside itself.
                let inside = move_row.at.starts_with(&drag.at);
                let next = (!inside).then(|| (move_row.key.clone(), zone));
                this.drop_claim = Some(event.event.position);
                if this.drop != next {
                    this.drop = next;
                    cx.notify();
                }
            }),
        )
        // A row of the Files view held over this one: the same places a
        // bookmark drag has, and no "Move into" under the ghost.
        .on_drag_move(
            cx.listener(move |this, event: &DragMoveEvent<FileDrag>, _, cx| {
                if !event.bounds.contains(&event.event.position) {
                    return;
                }
                event.drag(cx).set_action(None);
                this.drop_claim = Some(event.event.position);
                let y = f32::from(event.event.position.y - event.bounds.top());
                let height = f32::from(event.bounds.size.height);
                let zone = drop_zone(y, height, file_move_row.item.kind() == "group");
                let next = Some((file_move_row.key.clone(), zone));
                if this.drop != next {
                    this.drop = next;
                    cx.notify();
                }
            }),
        )
        .on_drop(cx.listener(move |this, drag: &FileDrag, _, cx| {
            let zone = this
                .drop
                .take()
                .filter(|(k, _)| *k == file_drop_row.key)
                .map(|(_, z)| z);
            this.drop_taken = true;
            if let Some(zone) = zone {
                cx.emit(BookmarkEvent::AddPath {
                    path: drag.path.clone(),
                    folder: drag.folder,
                    to: insertion_path(&file_drop_row.at, zone),
                });
            }
            cx.notify();
        }))
        .on_drop(cx.listener(move |this, drag: &BookmarkDrag, _, cx| {
            let zone = this
                .drop
                .take()
                .filter(|(k, _)| *k == drop_row.key)
                .map(|(_, z)| z);
            this.dragging = None;
            if let Some(zone) = zone {
                cx.emit(BookmarkEvent::Move {
                    from: drag.at.clone(),
                    to: insertion_path(&drop_row.at, zone),
                });
            }
            cx.notify();
        }))
        .into_any_element()
    }
}

/// The item at an index path into groups.
fn item_at(items: &[Bookmark], at: &[usize]) -> Option<Bookmark> {
    let (first, rest) = at.split_first()?;
    let mut item = items.get(*first)?.clone();
    for ix in rest {
        item = item.items().get(*ix)?.clone();
    }
    Some(item)
}

impl BookmarksPane {
    /// A move no row claimed: the pointer is over the empty space or
    /// outside the pane, so the drop marker goes.
    fn drag_left_rows(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.drop_claim != Some(position) && self.drop.is_some() {
            self.drop = None;
            cx.notify();
        }
    }
}

/// Every file item in a group, members of nested groups included.
fn group_files(group: &Bookmark) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for item in group.items() {
        match item.kind() {
            "file" => {
                if let Some(path) = item.path() {
                    out.push((path.to_string(), item.subpath().map(str::to_string)));
                }
            }
            "group" => out.extend(group_files(&item)),
            _ => {}
        }
    }
    out
}

impl Render for BookmarksPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        self.menu_focus.sync(self.menu.is_some(), window, cx);
        self.rename_focus.sync(self.renaming.is_some(), window, cx);
        // A field that had the keyboard and lost it is committed, which is
        // what a click elsewhere did to Obsidian's (`s11b` `blurRename`).
        let field_focused = self.rename_focus.handle.is_focused(window);
        if let Some(edit) = self.renaming.as_mut() {
            if field_focused {
                edit.had_focus = true;
            } else if edit.had_focus && self.menu.is_none() {
                self.commit_rename(cx);
            }
        }
        // A drag that ended anywhere but on a row leaves nothing behind.
        if !cx.has_active_drag() {
            self.drop_taken = false;
        }
        if !cx.has_active_drag() && (self.dragging.is_some() || self.drop.is_some()) {
            self.dragging = None;
            self.drop = None;
            self.drop_claim = None;
        }
        let keyboard = self.focus_handle.is_focused(window);
        let rows = self.rows(cx);
        let empty = rows.is_empty();
        let list = div()
            .id("bookmarks-list")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            // The first row sits 4 px under the header (`s02`: header
            // 40-80, first row at 84).
            .px(px(12.))
            .pt(px(4.))
            .pb(px(32.))
            .when(empty, |d| {
                // "No bookmarks found", centred in the faint colour (`s20`).
                d.child(
                    div()
                        .w_full()
                        .pt(px(16.))
                        .flex()
                        .justify_center()
                        .text_size(px(crate::theme::FONT_UI_SMALL))
                        .text_color(theme.tokens.text_muted)
                        .child("No bookmarks found"),
                )
            })
            .children(
                rows.into_iter()
                    .enumerate()
                    .map(|(ix, row)| self.render_row(ix, row, keyboard, &theme, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.open_menu(None, event.position, cx)
                }),
            )
            // A Files row dropped on the empty space below the rows goes
            // last. A row's own drop takes the target first, so this only
            // acts when no row is under the pointer.
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<FileDrag>, _, cx| {
                this.drag_left_rows(event.event.position, cx)
            }))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<BookmarkDrag>, _, cx| {
                    this.drag_left_rows(event.event.position, cx)
                }),
            )
            .on_drop(cx.listener(|this, drag: &FileDrag, _, cx| {
                if std::mem::take(&mut this.drop_taken) || this.drop.is_some() {
                    return;
                }
                cx.emit(BookmarkEvent::AddPath {
                    path: drag.path.clone(),
                    folder: drag.folder,
                    to: vec![this.items.len()],
                });
            }));
        let menu = self.menu.as_ref().map(|menu| {
            file_menu::render(
                menu,
                &theme,
                &self.menu_focus.handle,
                window.viewport_size().width,
                cx,
                |this, cx| {
                    this.menu = None;
                    cx.notify();
                },
                |this, pick, _, cx| this.picked(pick, cx),
                |this, open, cx| {
                    if let Some(menu) = this.menu.as_mut() {
                        menu.submenu = open;
                        cx.notify();
                    }
                },
            )
        });
        let rename_keys = inline_field::bind_keys(
            div()
                .key_context("TextInput")
                .track_focus(&self.rename_focus.handle),
            cx,
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .key_context("DockTree")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, _: &TreeUp, _, cx| this.tree_key(crate::panes::NavKey::Up, cx)),
            )
            .on_action(cx.listener(|this, _: &TreeDown, _, cx| {
                this.tree_key(crate::panes::NavKey::Down, cx)
            }))
            .on_action(cx.listener(|this, _: &TreeLeft, _, cx| {
                this.tree_key(crate::panes::NavKey::Left, cx)
            }))
            .on_action(cx.listener(|this, _: &TreeRight, _, cx| {
                this.tree_key(crate::panes::NavKey::Right, cx)
            }))
            .on_action(cx.listener(|this, _: &TreeActivate, _, cx| this.tree_activate(cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.renaming.is_none() {
                        window.focus(&this.focus_handle, cx);
                    }
                }),
            )
            .child(self.render_header(&theme, cx))
            .when(self.show_search, |d| {
                d.child(tree::filter_field(
                    ("bookmark-filter-clear", 0),
                    self.filter.clone(),
                    &theme,
                    cx,
                    |this, window, cx| this.clear_filter(window, cx),
                ))
            })
            .child(
                rename_keys
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(list),
            )
            .children(menu)
    }
}

impl inline_field::Host for BookmarksPane {
    fn editing(&mut self) -> Option<&mut inline_field::Editing> {
        self.renaming.as_mut().map(|edit| &mut edit.editing)
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_rename(cx);
        window.focus(&self.focus_handle, cx);
    }

    /// Escape leaves the title as it was (`s11b` `escapeRename`).
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }
}

/// Typed, composed and pasted text in a row being renamed: the halves are
/// [`inline_field`]'s, as for the Files view's rows.
impl EntityInputHandler for BookmarksPane {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        inline_field::text_for_range(
            &self.renaming.as_ref()?.editing,
            range_utf16,
            adjusted_range,
        )
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(inline_field::selected_text_range(
            &self.renaming.as_ref()?.editing,
        ))
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        inline_field::marked_text_range(&self.renaming.as_ref()?.editing)
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(edit) = self.renaming.as_mut() {
            edit.editing.marked = None;
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.renaming.as_mut() {
            inline_field::replace_text_in_range(&mut edit.editing, range_utf16, text);
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = self.renaming.as_mut() {
            inline_field::replace_and_mark_text_in_range(
                &mut edit.editing,
                range_utf16,
                new_text,
                new_selected_range,
            );
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let edit = self.renaming.as_ref()?;
        Some(inline_field::bounds_for_range(
            &edit.editing,
            edit.layout.as_ref()?,
            range_utf16,
            element_bounds,
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Bookmark> {
        vec![
            Bookmark::file("Alpha.md", None, None, 1),
            Bookmark::file("Alpha.md", Some("#Second heading"), None, 2),
            Bookmark::file("Alpha.md", Some("#^blk1"), None, 3),
            Bookmark::folder("sub", None, 4),
            Bookmark::search("alpha", None, 5),
            Bookmark::group(
                Some("Group one"),
                vec![
                    Bookmark::file("Beta.md", None, None, 6),
                    Bookmark::group(
                        Some("Nested group"),
                        vec![Bookmark::file("sub/Delta.md", None, None, 7)],
                        8,
                    ),
                ],
                9,
            ),
            Bookmark::group(Some("Empty group"), vec![], 10),
            Bookmark::file("Missing.md", None, None, 11),
            Bookmark::file("Image.png", None, None, 12),
        ]
    }

    fn titles(rows: &[Row]) -> Vec<(String, usize)> {
        rows.iter()
            .map(|r| (r.item.display_title(), r.depth))
            .collect()
    }

    fn missing() -> HashSet<String> {
        ["Missing.md".to_string()].into_iter().collect()
    }

    /// The pane at rest: every item in file order, groups' members one
    /// level in, the bookmark of a note that is not there left out, and an
    /// attachment's extension kept (`s02`).
    #[test]
    fn the_rows_are_the_file_in_order_without_missing_notes() {
        let rows = visible_rows(&sample(), &HashSet::new(), "", &missing());
        assert_eq!(
            titles(&rows),
            [
                ("Alpha".into(), 0),
                ("Alpha#Second heading".into(), 0),
                ("Alpha#^blk1".into(), 0),
                ("sub".into(), 0),
                ("alpha".into(), 0),
                ("Group one".into(), 0),
                ("Beta".into(), 1),
                ("Nested group".into(), 1),
                ("Delta".into(), 2),
                ("Empty group".into(), 0),
                ("Image.png".into(), 0),
            ]
        );
        assert_eq!(rows[6].at, [5, 0]);
        assert_eq!(rows[8].at, [5, 1, 0]);
        // A note that is missing keeps its place in the index paths.
        assert_eq!(rows[10].at, [8]);
    }

    #[test]
    fn a_folded_group_hides_its_members() {
        let all = visible_rows(&sample(), &HashSet::new(), "", &missing());
        let folded: HashSet<String> = [all[5].key.clone()].into_iter().collect();
        let rows = visible_rows(&sample(), &folded, "", &missing());
        assert!(rows[5].collapsed);
        assert_eq!(rows[6].item.display_title(), "Empty group");
    }

    /// The filter's four captures (`s09`): `al`, `beta`, `del` and `zzz`.
    #[test]
    fn the_filter_keeps_matches_and_the_groups_above_them() {
        let rows = |q: &str| {
            titles(&visible_rows(&sample(), &HashSet::new(), q, &missing()))
                .into_iter()
                .map(|(t, _)| t)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rows("al"),
            ["Alpha", "Alpha#Second heading", "Alpha#^blk1", "alpha"]
        );
        assert_eq!(rows("BETA"), ["Group one", "Beta"]);
        assert_eq!(rows("del"), ["Group one", "Nested group", "Delta"]);
        assert!(rows("zzz").is_empty());
    }

    #[test]
    fn every_kind_has_obsidians_icon() {
        let icons: Vec<&str> = sample()[..5].iter().map(kind_icon).collect();
        assert_eq!(icons, ["file", "heading", "toy-brick", "folder", "search"]);
        assert_eq!(
            kind_icon(&Bookmark::website("https://x", None, 1)),
            "globe-2"
        );
        assert_eq!(
            kind_icon(&Bookmark::graph("g", serde_json::json!({}), 1)),
            "git-fork"
        );
    }

    #[test]
    fn a_drop_names_the_place_it_lands() {
        assert_eq!(drop_zone(2., 25., false), Zone::Before);
        assert_eq!(drop_zone(20., 25., false), Zone::After);
        assert_eq!(drop_zone(12., 25., true), Zone::Into);
        assert_eq!(drop_zone(2., 25., true), Zone::Before);
        assert_eq!(insertion_path(&[5, 1], Zone::Before), [5, 1]);
        assert_eq!(insertion_path(&[5, 1], Zone::After), [5, 2]);
        assert_eq!(insertion_path(&[5], Zone::Into), [5, 0]);
    }
}
