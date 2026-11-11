//! The file context menu: the list Obsidian opens on a right click over
//! anything that stands for a file or folder, drawn once here for every view
//! that raises it.
//!
//! The Files view raises it on its rows and on the empty space below them;
//! the Backlinks and Outgoing links panes raise it on every row that stands
//! for a file. The lists differ by what was clicked ([`Target`]), so the
//! model is a [`Target`] and the [`Item`]s it offers, each in Obsidian's
//! `data-section`, with a separator drawn wherever the section changes.
//! The views keep their own state and their own handlers: a menu here reports
//! a [`Pick`] and the path it was opened on, and nothing else. The look was
//! captured against Obsidian's on 2026-10-28 and is the one the Files view
//! had; a change here moves every menu at once, which is the point.
//!
//! The editor raises it too, on a right click in the text
//! ([`Target::Editor`]): Obsidian's editor menu is the same list widget,
//! with the clipboard rows and — over a wikilink — the whole file menu
//! wrapped around them
//! ([[Specification#Observed editor context menu and view header actions, Obsidian 1.14.5 (installer build 1.13.7)]]).
//! Three of its rows open **submenus** — Format, Paragraph and Insert, and
//! "Copy path" has one of its own — and the probe never opened them, so
//! what they hold is unobserved. A menu here lists only what it can run and
//! never draws a dead row, so those rows are left out rather than guessed,
//! even where Scoria has the commands behind them; they await a probe that
//! opens each submenu and reads it. The reading view has no text menu at
//! all: a right click there raises the file menu on a link and nothing
//! anywhere else, which is what Obsidian does.
//!
//! "Show in system explorer" is also here, as the command lines that select
//! a file in the desktop's file manager, so the one platform table has a test
//! beside it.

use crate::theme::Theme;
use gpui::{
    actions, anchored, deferred, div, prelude::*, px, relative, Anchor, AnyElement, App, Context,
    FocusHandle, KeyBinding, Pixels, Point, SharedString, Stateful, Window,
};
use serde_json::{json, Value};
use std::cell::Cell;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;

actions!(menu, [CloseMenu]);

/// Escape closes an open menu: observed, and what every probe run relies
/// on to take one menu down before raising the next. The binding lives in
/// the `Menu` key context, which every menu drawn through [`menu_box`]
/// wears while it holds the keyboard.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseMenu, Some("Menu"))]);
}

/// The keyboard a host hands to its menus: the focus handle the open menu
/// tracks, and where focus was before, so closing the menu -- by Escape,
/// a pick or a press outside -- puts it back.
///
/// The host calls [`MenuFocus::sync`] from its render with whether a menu
/// is open: the first render with one open takes focus (remembering the
/// previous handle), the first render with none restores it. Routing it
/// through the render means every way a menu closes restores focus the
/// same way, with no window needed at the close site.
pub struct MenuFocus {
    pub handle: FocusHandle,
    restore: Option<FocusHandle>,
}

impl MenuFocus {
    pub fn new(cx: &mut App) -> MenuFocus {
        MenuFocus {
            handle: cx.focus_handle(),
            restore: None,
        }
    }

    pub fn sync(&mut self, open: bool, window: &mut Window, cx: &mut App) {
        match (open, self.restore.is_some()) {
            (true, false) => {
                self.restore = Some(window.focused(cx).unwrap_or_else(|| self.handle.clone()));
                window.focus(&self.handle, cx);
            }
            (false, true) => {
                if let Some(previous) = self.restore.take() {
                    if self.handle.is_focused(window) {
                        window.focus(&previous, cx);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Makes a list box a menu: it tracks `focus` under the `Menu` key context,
/// so Escape runs `on_close`, and a press outside it runs `on_close` too.
/// `occlude` keeps a click on a row from reaching what is under it.
pub fn menu_box<V: 'static>(
    list: Stateful<gpui::Div>,
    focus: &FocusHandle,
    cx: &mut Context<V>,
    on_close: impl Fn(&mut V, &mut Context<V>) + 'static,
) -> Stateful<gpui::Div> {
    menu_box_with_submenu(list, focus, cx, on_close, Rc::new(Cell::new(false)))
}

thread_local! {
    /// Whether the pointer rests on an open submenu. It outlives a frame
    /// because gpui reports a hover only when it *changes*: a flag made
    /// afresh in each render would read `false` again after the re-render
    /// the submenu's own opening causes, and never see the hover again.
    static OVER_SUBMENU: Rc<Cell<bool>> = Rc::new(Cell::new(false));
}

/// [`menu_box`] for a menu with a submenu drawn beside the list.
/// gpui decides "outside" from the list's own bounds, and a submenu stands
/// beside them, so a press on one of its rows would close the menu on the
/// way down, before the click that picks the row ever lands. `over_submenu`
/// is set while the pointer rests on the submenu, and a press then is not
/// outside.
pub fn menu_box_with_submenu<V: 'static>(
    list: Stateful<gpui::Div>,
    focus: &FocusHandle,
    cx: &mut Context<V>,
    on_close: impl Fn(&mut V, &mut Context<V>) + 'static,
    over_submenu: Rc<Cell<bool>>,
) -> Stateful<gpui::Div> {
    let on_close = Rc::new(on_close);
    list.track_focus(focus)
        .key_context("Menu")
        .occlude()
        .on_action(cx.listener({
            let on_close = on_close.clone();
            move |this, _: &CloseMenu, _, cx| on_close(this, cx)
        }))
        .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
            if !over_submenu.get() {
                on_close(this, cx)
            }
        }))
}

/// Obsidian's `data-section` on a menu item. Items are grouped by it in the
/// order below, and a separator is drawn wherever the section changes — which
/// is how the captured menus get their separators without any being written
/// down one at a time ([[Specification#The file explorer's context menus]]).
/// [`Section::Danger`] takes **two**: Obsidian emits an empty section before
/// it, and both explorer menus were captured with the two consecutive
/// separators that leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// `close`: the tab header menu's Close, Close others and Close all.
    Close,
    /// `pane`: the tab header menu's Pin.
    Pane,
    /// `find`: a markdown tab's Find... and Replace....
    Find,
    /// The folder row's and the empty space's creation items.
    ActionPrimary,
    /// The file row's "Open in..." items.
    Open,
    /// `selection-link`: the editor menu's two link-making rows, which
    /// Obsidian draws whether or not anything is selected.
    SelectionLink,
    /// `selection`: the editor menu's rows built from what is selected,
    /// and the Format / Edit link / Edit tag row that sits with them.
    Selection,
    /// `clipboard`: the editor menu's Cut, Copy, Paste, Paste as plain text
    /// and Select all.
    Clipboard,
    Action,
    /// Obsidian leaves "Copy path" with no section at all.
    Unsectioned,
    System,
    Danger,
    /// The All properties row's "Delete property": one separator before it,
    /// not the danger pair.
    Remove,
}

/// What a clicked row asks for. The path it acts on is the menu's
/// [`Target`], so a menu can never act on the wrong row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    // ----- the tab header's own rows ------------------------------------------
    /// "Close": the right-clicked tab, pinned or not.
    CloseTab,
    /// "Close others": every other tab of its group but the pinned ones.
    CloseOthers,
    /// "Close tabs after": the tabs to its right but the pinned ones.
    CloseAfter,
    /// "Close all": every tab of its group but the pinned ones.
    CloseAll,
    /// "Pin" / "Unpin": `workspace:toggle-pin` on the tab.
    TogglePin,
    /// "Move to new window": `workspace:move-to-new-window` on the tab.
    MoveToNewWindow,
    /// "Backlinks in document": `backlink:toggle-backlinks-in-document` on
    /// the active markdown leaf.
    ToggleBacklinksInDocument,
    /// A markdown tab's "Add file property", "Find..." and "Replace...":
    /// `markdown:add-metadata-property`, `editor:open-search` and
    /// `editor:open-search-replace`.
    AddFileProperty,
    Find,
    Replace,
    OpenInNewTab,
    OpenToRight,
    /// "Open in new window": the file in a popout window of its own
    /// (`workspace:open-in-new-window` on the view header's menu, where
    /// it copies the leaf; a fresh tab on the row's file everywhere else).
    OpenInNewWindow,
    /// The view header's "Split right" and "Split down":
    /// `workspace:split-vertical` and `split-horizontal` on the leaf's
    /// own group.
    SplitRight,
    SplitDown,
    /// "Make a copy".
    Duplicate,
    Move,
    Bookmark,
    /// "Merge entire file with...": `note-composer:merge-file` on the
    /// menu's file.
    MergeWith,
    /// The editor menu's "Extract current selection...":
    /// `note-composer:split-file`.
    ExtractSelection,
    CopyPath,
    OpenInDefaultApp,
    ShowInSystemExplorer,
    /// "Reveal file in navigation": the Files view scrolled to the row.
    RevealInNavigation,
    Rename,
    Delete,
    /// The creation items, in the target folder.
    NewNote,
    NewFolder,
    NewCanvas,
    NewBase,
    SearchInFolder,
    // ----- the editor menu's rows ------------------------------------------
    /// "Add link": the wikilink shape around the selection
    /// (`editor:insert-wikilink`). Obsidian's own label for that command is
    /// "Add internal link", and the row wears `lucide-link`, so the pairing
    /// is read off the icon and the label rather than observed directly.
    AddLink,
    /// "Add external link": the Markdown link shape (`editor:insert-link`),
    /// the row Obsidian wears `lucide-external-link` on.
    AddExternalLink,
    /// Search for “<selection>”: the Search pane, the selected text as its
    /// query, the way a folder row's "Search in folder" opens it.
    SearchForSelection,
    Cut,
    Copy,
    Paste,
    /// "Paste as plain text". Scoria's clipboard is read as text and pasted
    /// verbatim, with no HTML or Markdown conversion on the way in, so this
    /// is the same paste — which is what the row promises.
    PastePlain,
    SelectAll,
    // ----- the All properties row's menu ------------------------------------
    /// "Rename": the key renamed in every note that carries it.
    RenameProperty,
    /// "Delete property": the key removed from every note that carries it.
    DeleteProperty,
    /// "Property type" ▸ one of the six: `types.json`.
    SetPropertyType(scoria_formats::properties::PropertyType),
    // ----- the Bookmarks pane's rows ------------------------------------------
    /// "Edit...": the "Edit bookmark" modal on the row's item.
    EditBookmark,
    /// "Remove": the item out of `bookmarks.json`, the note left alone.
    RemoveBookmark,
    /// A group's "Bookmark the active tab...", into that group.
    BookmarkActiveTab,
    /// "New group": in the group right-clicked, or at the top level from
    /// the pane's empty space.
    NewBookmarkGroup,
}

/// One row of a context menu: what Obsidian draws, and what a click reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: SharedString,
    pub icon: &'static str,
    pub section: Section,
    pub pick: Pick,
    /// Drawn greyed and inert, the way Obsidian draws Cut and Copy with
    /// nothing selected (`is-disabled` on the row).
    pub disabled: bool,
    /// `is-warning`: the label in the warning colour.
    pub warning: bool,
    /// `mod-checked`: a check at the row's right.
    pub checked: bool,
    /// `has-submenu`: the rows a hover opens beside this one.
    pub submenu: Vec<Item>,
}

/// What was right-clicked. Paths are vault-relative, folders without a
/// trailing slash (`""` is the vault root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A file row of the Files view.
    File(String),
    /// A folder row of the Files view; `""` is the vault root.
    Folder(String),
    /// The empty space below the Files view's last row, which Obsidian gives
    /// the four `action-primary` items alone, rooted at the vault.
    Blank,
    /// A row standing for a file somewhere other than the Files view: a
    /// Links row or a mentions group of the Backlinks and Outgoing links
    /// panes. Obsidian's menu there is the file menu without "Make a copy",
    /// "Delete" and the version history, with Rename moved up beside Move
    /// and "Reveal file in navigation" added at the end
    /// ([[Specification#Observed outgoing links pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]).
    Linked(String),
    /// A right click in the editor's text, in either editing mode: the
    /// probe raised the same rows in Live Preview and in Source mode.
    Editor(EditorClick),
    /// The view header's **More options** over a base leaf
    /// (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`, `s02`
    /// `moreOptions`): the file menu with the leaf's two splits in place
    /// of the open-in rows, Rename beside Move and Bookmark, and "Delete
    /// file" alone in the danger pair. The path is the leaf's file.
    View(String),
    /// **More options** over a markdown leaf: [`Target::View`]'s rows under
    /// a first row, "Backlinks in document", checked while the leaf shows
    /// the block (`tools/obsidian-probe/results/2026-11-09-panes-workspaces-headings.json`,
    /// `bl` `moreMenu`). Its two view-mode rows are left out, as the tab
    /// header's are.
    Note { path: String, backlinks: bool },
    /// A right click on a Bases row's name, a card or a list line's link
    /// (`s24` `linkMenu`, 2026-11-01): the file menu with the two open-in
    /// rows, Rename beside Move and Bookmark, and "Delete file".
    Row(String),
    /// A right click on an All properties row
    /// ([[Specification#Observed all properties pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]):
    /// Rename, Property type ▸, a separator, Delete property. `ty` is the
    /// type the key has now, which the submenu checks.
    Property {
        name: String,
        ty: Option<scoria_formats::properties::PropertyType>,
    },
    /// A row of the Bookmarks pane, or the pane's empty space when `kind`
    /// is [`BookmarkKind::Blank`]
    /// (`tools/obsidian-probe/results/2026-11-07-bookmarks-pane.json`,
    /// `s06`). `path` is the file or folder a `file` or `folder` item
    /// points at, which "Reveal file in navigation" acts on.
    Bookmark { kind: BookmarkKind, path: String },
    /// A right click on a tab header
    /// (`tools/obsidian-probe/results/2026-11-09-tab-header-menu.json`):
    /// the close rows, Pin, the leaf's window and split rows, then the
    /// view header's file rows. A right click does not make the tab
    /// active, and an inactive tab's menu stops after Move to new window.
    /// `group` and `ix` name the tab; `path` is its file, empty for a tab
    /// with none; `tabs` is how many its group holds, since Close others
    /// and Close all are drawn only beside another tab, and Close tabs
    /// after only with one to its right; `pinned` names the Pin row.
    /// `backlinks` is whether the active markdown tab shows its backlinks
    /// block, `None` for any other tab, which has no such row.
    Tab {
        group: u64,
        ix: usize,
        path: String,
        tabs: usize,
        pinned: bool,
        active: bool,
        backlinks: Option<bool>,
    },
}

/// What a Bookmarks pane row stands for, as far as its menu is concerned:
/// a heading, a block and an attachment are all `file` items and take the
/// file's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookmarkKind {
    File,
    Folder,
    Search,
    Url,
    Graph,
    Group,
    Blank,
}

/// A right click in the editor: what it landed on, and the selection as it
/// stood when it arrived.
///
/// The two selection fields are not one field twice. Obsidian built the
/// Search row from the selection **as it was at the click** and left Cut and
/// Copy to the selection that **survived** it: a right click outside a
/// selection moves the caret, which takes the selection away, and the menu
/// it raised still carried "Search for “…”" while Cut and Copy were drawn
/// disabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorClick {
    pub spot: Spot,
    /// The text selected when the click arrived, empty with no selection.
    /// It is what names the Search row.
    pub selection: String,
    /// Whether a selection survived the click: a click inside one keeps it,
    /// a click anywhere else collapses it. Cut and Copy follow this.
    pub selected: bool,
}

/// What a right click in the editor landed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spot {
    /// Plain text — and an external Markdown link, and a task checkbox in
    /// Live Preview, both of which raised the plain menu too.
    Text,
    /// A tag. Obsidian puts "Edit tag" where the plain menu's Format row
    /// sits; both are left out here (Format is a submenu, Edit tag has no
    /// command behind it), so a tag's rows are the plain menu's. The name
    /// rides along for the probe-shaped row to come.
    Tag(String),
    /// A wikilink that resolves to this vault file: the file menu's rows
    /// wrap the text rows.
    Link(String),
}

impl Target {
    /// The path every item of the menu acts on; the vault root for
    /// [`Target::Blank`], and nothing for an editor click that is not on a
    /// link to a file.
    pub fn path(&self) -> &str {
        match self {
            Target::File(p)
            | Target::Folder(p)
            | Target::Linked(p)
            | Target::View(p)
            | Target::Row(p) => p,
            Target::Bookmark { path, .. }
            | Target::Tab { path, .. }
            | Target::Note { path, .. } => path,
            Target::Blank | Target::Property { .. } => "",
            Target::Editor(click) => match &click.spot {
                Spot::Link(path) => path,
                Spot::Text | Spot::Tag(_) => "",
            },
        }
    }
}

/// Where a right click in the editor leaves the caret: `None` to keep the
/// selection as it is, `Some(at)` to collapse the caret onto the click.
///
/// Observed: with the caret at the document start a right click on a word
/// moved it to that word; a right click *inside* a selection kept the
/// selection; a right click *outside* one moved the caret to the click.
/// A click exactly on either end of the selection was not probed — it is
/// taken as inside, the way a pointer landing on the edge of the painted
/// highlight reads (OBSERVE).
pub fn right_click_caret(selection: &Range<usize>, at: usize) -> Option<usize> {
    if !selection.is_empty() && selection.start <= at && at <= selection.end {
        None
    } else {
        Some(at)
    }
}

/// The menu for a target, in Obsidian's order.
///
/// One of the captured items is left out rather than drawn dead, because
/// Scoria has no command behind it: **Open version history** (no file
/// history). **Merge entire file with...** was a second until the Note
/// composer landed (2026-11-09), and is drawn on notes only. Listing a row
/// that cannot run is what the command palette refuses to do, and the menu
/// refuses it too; the gaps are entries in `docs/scoria/UI parity
/// backlog.md`. Dropping the only item of a section drops its separator
/// with it, so the `view` section's separator is absent here where the
/// capture has it. **Open in new window** was a third until the popout
/// windows landed (2026-11-09); it wears `lucide-picture-in-picture-2`
/// in every capture.
///
/// Obsidian's file row has no creation items at all — Scoria's menu used to
/// offer "New note" and "New folder" on a file, which is the one thing here
/// that was wrong rather than missing.
pub fn items(target: &Target) -> Vec<Item> {
    use Pick as P;
    use Section as S;
    match target {
        Target::File(_) => vec![
            item("Open in new tab", "file-plus", S::Open, P::OpenInNewTab),
            item(
                "Open to the right",
                "separator-vertical",
                S::Open,
                P::OpenToRight,
            ),
            item(
                "Open in new window",
                "picture-in-picture-2",
                S::Open,
                P::OpenInNewWindow,
            ),
            item("Make a copy", "files", S::Action, P::Duplicate),
            item("Move file to...", "folder-tree", S::Action, P::Move),
            item("Bookmark...", "bookmark", S::Action, P::Bookmark),
        ]
        .into_iter()
        .chain(merge_row(target.path()))
        .chain([
            item("Copy path", "clipboard", S::Unsectioned, P::CopyPath),
            item(
                "Open in default app",
                "arrow-up-right",
                S::System,
                P::OpenInDefaultApp,
            ),
            item(
                "Show in system explorer",
                "arrow-up-right",
                S::System,
                P::ShowInSystemExplorer,
            ),
            item("Rename...", "pen-line", S::Danger, P::Rename),
            item("Delete", "trash-2", S::Danger, P::Delete),
        ])
        .collect(),
        Target::Folder(_) => vec![
            // Obsidian draws `lucide-edit` here, which is the current
            // `square-pen`; `lucide-edit-3` on Rename is `pen-line`. Both
            // renames come from Lucide's own alias data, not a guess.
            item("New note", "square-pen", S::ActionPrimary, P::NewNote),
            item("New folder", "folder-open", S::ActionPrimary, P::NewFolder),
            item(
                "New canvas",
                "layout-dashboard",
                S::ActionPrimary,
                P::NewCanvas,
            ),
            item("New base", "layout-list", S::ActionPrimary, P::NewBase),
            item("Make a copy", "files", S::Action, P::Duplicate),
            item("Move folder to...", "folder-tree", S::Action, P::Move),
            item(
                "Search in folder",
                "folder-search",
                S::Action,
                P::SearchInFolder,
            ),
            item("Bookmark...", "bookmark", S::Action, P::Bookmark),
            item("Copy path", "clipboard", S::Unsectioned, P::CopyPath),
            item(
                "Show in system explorer",
                "arrow-up-right",
                S::System,
                P::ShowInSystemExplorer,
            ),
            item("Rename...", "pen-line", S::Danger, P::Rename),
            item("Delete", "trash-2", S::Danger, P::Delete),
        ],
        Target::Blank => vec![
            item("New note", "square-pen", S::ActionPrimary, P::NewNote),
            item("New folder", "folder-open", S::ActionPrimary, P::NewFolder),
            item(
                "New canvas",
                "layout-dashboard",
                S::ActionPrimary,
                P::NewCanvas,
            ),
            item("New base", "layout-list", S::ActionPrimary, P::NewBase),
        ],
        // The 2026-10-22 probe read this menu's labels and separators off a
        // Links row but not its icons, so these are the explorer's for the
        // same labels. "Reveal file in navigation" has no explorer twin;
        // `folder-open` is a guess marked OBSERVE until a probe reads it.
        Target::Linked(_) => vec![
            item("Open in new tab", "file-plus", S::Open, P::OpenInNewTab),
            item(
                "Open to the right",
                "separator-vertical",
                S::Open,
                P::OpenToRight,
            ),
            item(
                "Open in new window",
                "picture-in-picture-2",
                S::Open,
                P::OpenInNewWindow,
            ),
            item("Rename...", "pen-line", S::Action, P::Rename),
            item("Move file to...", "folder-tree", S::Action, P::Move),
            item("Bookmark...", "bookmark", S::Action, P::Bookmark),
            item("Copy path", "clipboard", S::Unsectioned, P::CopyPath),
            item(
                "Open in default app",
                "arrow-up-right",
                S::System,
                P::OpenInDefaultApp,
            ),
            item(
                "Show in system explorer",
                "arrow-up-right",
                S::System,
                P::ShowInSystemExplorer,
            ),
            item(
                "Reveal file in navigation",
                "folder-open",
                S::System,
                P::RevealInNavigation,
            ),
        ],
        Target::Editor(click) => editor_items(click),
        // Observed through a CDP right click on a property row: Rename
        // (`lucide-edit-3`, which is `pen-line`), Property type (`lucide-menu`,
        // a submenu of the six types), a separator, Delete property
        // (`is-warning`, `lucide-trash-2`). On `tags`, `aliases` and
        // `cssclasses` Rename and Delete property are `is-disabled`
        // (2026-11-04 probe); Property type stays live.
        Target::Bookmark { kind, .. } => {
            // Observed per kind (2026-11-07, `s06`): the open rows on what
            // opens in a leaf, Rename and Edit... on every item, Reveal on
            // what is in the Files view, Remove in the danger pair. "Open in
            // new window" rides with them since the popouts landed, and a
            // group takes the header's two actions in place of Edit....
            use BookmarkKind as K;
            let open = [
                item("Open in new tab", "file-plus", S::Open, P::OpenInNewTab),
                item(
                    "Open to the right",
                    "separator-vertical",
                    S::Open,
                    P::OpenToRight,
                ),
                item(
                    "Open in new window",
                    "picture-in-picture-2",
                    S::Open,
                    P::OpenInNewWindow,
                ),
            ];
            let rename = item("Rename", "pencil", S::Action, P::Rename);
            let edit = item("Edit...", "wrench", S::Action, P::EditBookmark);
            let reveal = item(
                "Reveal file in navigation",
                "folder-open",
                S::System,
                P::RevealInNavigation,
            );
            let remove = item("Remove", "trash-2", S::Danger, P::RemoveBookmark);
            match kind {
                K::File => [&open[..], &[rename, edit, reveal, remove]].concat(),
                K::Folder => vec![rename, edit, reveal, remove],
                K::Search | K::Url => vec![rename, edit, remove],
                K::Graph => [&open[..], &[rename, edit, remove]].concat(),
                K::Group => [
                    &open[..],
                    &[
                        rename,
                        item(
                            "Bookmark the active tab...",
                            "bookmark-plus",
                            S::Action,
                            P::BookmarkActiveTab,
                        ),
                        item("New group", "folder-plus", S::Action, P::NewBookmarkGroup),
                        remove,
                    ],
                ]
                .concat(),
                K::Blank => vec![item(
                    "New group",
                    "folder-plus",
                    S::ActionPrimary,
                    P::NewBookmarkGroup,
                )],
            }
        }
        Target::Property { name, ty } => {
            let reserved = ["tags", "aliases", "cssclasses"]
                .iter()
                .any(|k| name.eq_ignore_ascii_case(k));
            use scoria_formats::properties::PropertyType as T;
            let types = [
                ("Checkbox", "square-check", T::Checkbox),
                ("Date", "calendar", T::Date),
                ("Date & time", "clock", T::DateTime),
                ("List", "list", T::MultiText),
                ("Number", "binary", T::Number),
                ("Text", "text", T::Text),
            ];
            let mut type_row = item(
                "Property type",
                "menu",
                S::Action,
                P::SetPropertyType(T::Text),
            );
            type_row.submenu = types
                .iter()
                .map(|(label, icon, t)| Item {
                    checked: *ty == Some(*t),
                    ..item(label, icon, S::Action, P::SetPropertyType(*t))
                })
                .collect();
            vec![
                Item {
                    disabled: reserved,
                    ..item("Rename", "pen-line", S::Action, P::RenameProperty)
                },
                type_row,
                Item {
                    warning: true,
                    disabled: reserved,
                    ..item("Delete property", "trash-2", S::Remove, P::DeleteProperty)
                },
            ]
        }
        // Observed on a base leaf; "Open version history" and the "Open
        // linked view" submenu are left out, as the file row's are, and
        // dropping the two `view` rows drops their separator. "Copy path" is a submenu there (as Obsidian URL / from
        // vault folder / from system root) and one row here, like the file
        // row's. Obsidian draws `lucide-edit-3` on Rename, which is
        // `pen-line`.
        // The Bases row's menu is the view header's with the open-in rows
        // in place of the splits; the same three rows are left out.
        Target::Row(_) => vec![
            item("Open in new tab", "file-plus", S::Open, P::OpenInNewTab),
            item(
                "Open to the right",
                "separator-vertical",
                S::Open,
                P::OpenToRight,
            ),
            item(
                "Open in new window",
                "picture-in-picture-2",
                S::Open,
                P::OpenInNewWindow,
            ),
            item("Rename...", "pen-line", S::Action, P::Rename),
            item("Move file to...", "folder-tree", S::Action, P::Move),
            item("Bookmark...", "bookmark", S::Action, P::Bookmark),
            item("Copy path", "clipboard", S::Unsectioned, P::CopyPath),
            item(
                "Open in default app",
                "arrow-up-right",
                S::System,
                P::OpenInDefaultApp,
            ),
            item(
                "Show in system explorer",
                "arrow-up-right",
                S::System,
                P::ShowInSystemExplorer,
            ),
            item(
                "Reveal file in navigation",
                "folder-open",
                S::System,
                P::RevealInNavigation,
            ),
            item("Delete file", "trash-2", S::Danger, P::Delete),
        ],
        Target::Tab {
            ix,
            path,
            tabs,
            pinned,
            active,
            backlinks,
            ..
        } => tab_items(path, *ix, *tabs, *pinned, *active, *backlinks),
        Target::Note { path, backlinks } => {
            let mut out = vec![backlinks_row(*backlinks)];
            out.extend(items(&Target::View(path.clone())));
            out
        }
        Target::View(_) => vec![
            item("Split right", "separator-vertical", S::Open, P::SplitRight),
            item("Split down", "separator-horizontal", S::Open, P::SplitDown),
            item(
                "Open in new window",
                "picture-in-picture-2",
                S::Open,
                P::OpenInNewWindow,
            ),
            item("Rename...", "pen-line", S::Action, P::Rename),
            item("Move file to...", "folder-tree", S::Action, P::Move),
            item("Bookmark...", "bookmark", S::Action, P::Bookmark),
        ]
        .into_iter()
        .chain(merge_row(target.path()))
        .chain([
            item("Copy path", "clipboard", S::Unsectioned, P::CopyPath),
            item(
                "Open in default app",
                "arrow-up-right",
                S::System,
                P::OpenInDefaultApp,
            ),
            item(
                "Show in system explorer",
                "arrow-up-right",
                S::System,
                P::ShowInSystemExplorer,
            ),
            item(
                "Reveal file in navigation",
                "folder-open",
                S::System,
                P::RevealInNavigation,
            ),
            item("Delete file", "trash-2", S::Danger, P::Delete),
        ])
        .collect(),
    }
}

/// "Merge entire file with..." (`lucide-git-merge`, `action`, after
/// Bookmark...), on a note only: an image card's menu has none, and the
/// command it runs wants a Markdown note.
fn merge_row(path: &str) -> Option<Item> {
    path.to_ascii_lowercase().ends_with(".md").then(|| {
        item(
            "Merge entire file with...",
            "git-merge",
            Section::Action,
            Pick::MergeWith,
        )
    })
}

/// The tab header's menu, as the 2026-11-09 probe drew it. Left out for
/// want of anything behind them: **Link with tab...** (no linked panes,
/// and the probe saw no trace of what it starts), a markdown tab's two
/// view-mode rows (their labels in the other modes were not captured),
/// **Export to PDF...**, a canvas's **Export as image**, **Open version
/// history** and the **Open linked view** submenu; Copy path keeps the one
/// flat row the other menus give it.
fn tab_items(
    path: &str,
    ix: usize,
    tabs: usize,
    pinned: bool,
    active: bool,
    backlinks: Option<bool>,
) -> Vec<Item> {
    use Pick as P;
    use Section as S;
    let mut out = vec![item("Close", "x", S::Close, P::CloseTab)];
    if tabs > 1 {
        out.push(item("Close others", "x", S::Close, P::CloseOthers));
        if ix + 1 < tabs {
            out.push(item("Close tabs after", "x", S::Close, P::CloseAfter));
        }
        out.push(item("Close all", "x", S::Close, P::CloseAll));
    }
    out.push(item(
        if pinned { "Unpin" } else { "Pin" },
        "pin",
        S::Pane,
        P::TogglePin,
    ));
    // After Pin (and Obsidian's "Link with tab..."), on the active markdown
    // tab only: the inactive tab's menu in the probe had no such row.
    if let Some(on) = backlinks.filter(|_| active) {
        out.push(backlinks_row(on));
    }
    out.push(item(
        "Move to new window",
        "picture-in-picture",
        S::Open,
        P::MoveToNewWindow,
    ));
    if !active {
        return out;
    }
    out.push(item(
        "Split right",
        "separator-vertical",
        S::Open,
        P::SplitRight,
    ));
    out.push(item(
        "Split down",
        "separator-horizontal",
        S::Open,
        P::SplitDown,
    ));
    if path.is_empty() {
        return out;
    }
    let markdown = path.to_ascii_lowercase().ends_with(".md");
    for row in items(&Target::View(path.to_string())) {
        if row.pick == P::SplitRight || row.pick == P::SplitDown {
            continue;
        }
        // Add file property follows Merge entire file with..., which a
        // markdown tab always has.
        let after_bookmark = row.pick == P::MergeWith;
        let before_copy = row.pick == P::CopyPath;
        if markdown && before_copy {
            out.push(item("Find...", "file-search", S::Find, P::Find));
            out.push(item("Replace...", "file-search", S::Find, P::Replace));
        }
        out.push(row);
        if markdown && after_bookmark {
            out.push(item(
                "Add file property",
                "circle-plus",
                S::Action,
                P::AddFileProperty,
            ));
        }
    }
    out
}

/// "Backlinks in document", `links-coming-in`, in the `pane` section; its
/// label never changes, and the check says whether the block shows.
fn backlinks_row(on: bool) -> Item {
    Item {
        checked: on,
        ..item(
            "Backlinks in document",
            "links-coming-in",
            Section::Pane,
            Pick::ToggleBacklinksInDocument,
        )
    }
}

/// One row, enabled.
fn item(label: &'static str, icon: &'static str, section: Section, pick: Pick) -> Item {
    Item {
        warning: false,
        checked: false,
        submenu: Vec::new(),
        label: SharedString::new_static(label),
        icon,
        section,
        pick,
        disabled: false,
    }
}

/// The editor menu for a click, in Obsidian's order.
///
/// Four of the captured rows open submenus nobody has read — **Format**,
/// **Paragraph**, **Insert** and, on a wikilink, **Copy path** — so the
/// three text ones are left out whole and Copy path keeps the one flat row
/// the rest of Scoria's menus give it. Three more are left out for want of
/// a command: **Edit link** and **Edit tag** (no inline rename of a link or
/// a tag) and
/// **Start presentation** (no presentation mode). Dropping the only row of a section drops its
/// separator with it, which is why the pair the capture has between Start
/// presentation and the system rows is one separator here.
fn editor_items(click: &EditorClick) -> Vec<Item> {
    use Pick as P;
    use Section as S;
    let on_link = matches!(click.spot, Spot::Link(_));
    let mut out = Vec::new();
    if on_link {
        out.push(item(
            "Open in new tab",
            "file-plus",
            S::Open,
            P::OpenInNewTab,
        ));
        out.push(item(
            "Open to the right",
            "separator-vertical",
            S::Open,
            P::OpenToRight,
        ));
        out.push(item(
            "Open in new window",
            "picture-in-picture-2",
            S::Open,
            P::OpenInNewWindow,
        ));
    }
    out.push(item("Add link", "link", S::SelectionLink, P::AddLink));
    out.push(item(
        "Add external link",
        "external-link",
        S::SelectionLink,
        P::AddExternalLink,
    ));
    if !click.selection.is_empty() {
        // Obsidian quotes the selected text in curly quotes.
        out.push(Item {
            label: format!("Search for \u{201c}{}\u{201d}", click.selection).into(),
            icon: "search",
            section: S::Selection,
            pick: P::SearchForSelection,
            disabled: false,
            warning: false,
            checked: false,
            submenu: Vec::new(),
        });
        out.push(item(
            "Extract current selection...",
            "git-branch-plus",
            S::Selection,
            P::ExtractSelection,
        ));
    }
    // `lucide-box-select` is Lucide's deprecated alias of `square-dashed`,
    // from its own alias data, the way `lucide-edit-3` is `pen-line`.
    for (label, icon, pick, needs_selection) in [
        ("Cut", "scissors", P::Cut, true),
        ("Copy", "copy", P::Copy, true),
        ("Paste", "clipboard-check", P::Paste, false),
        (
            "Paste as plain text",
            "clipboard-type",
            P::PastePlain,
            false,
        ),
        ("Select all", "square-dashed", P::SelectAll, false),
    ] {
        out.push(Item {
            disabled: needs_selection && !click.selected,
            ..item(label, icon, S::Clipboard, pick)
        });
    }
    if on_link {
        out.push(item("Copy path", "clipboard", S::Unsectioned, P::CopyPath));
        out.push(item("Rename...", "pen-line", S::Action, P::Rename));
        out.push(item("Move file to...", "folder-tree", S::Action, P::Move));
        out.push(item("Bookmark...", "bookmark", S::Action, P::Bookmark));
        out.push(item(
            "Open in default app",
            "arrow-up-right",
            S::System,
            P::OpenInDefaultApp,
        ));
        out.push(item(
            "Show in system explorer",
            "arrow-up-right",
            S::System,
            P::ShowInSystemExplorer,
        ));
        out.push(item(
            "Reveal file in navigation",
            "folder-open",
            S::System,
            P::RevealInNavigation,
        ));
    }
    out
}

/// How many separators sit above an item given the section above it: one
/// wherever the section changes, and two above [`Section::Danger`] for the
/// empty section Obsidian puts there.
pub fn separators_above(previous: Option<Section>, section: Section) -> usize {
    match previous {
        None => 0,
        Some(p) if p == section => 0,
        Some(_) if section == Section::Danger => 2,
        Some(_) => 1,
    }
}

/// The menu as a reader sees it: a `---` per separator, then each item's
/// label. Comparing this against the spec's own tables is what the tests
/// do, and what the control socket reports.
pub fn drawn(target: &Target) -> Vec<String> {
    let mut out = Vec::new();
    let mut previous = None;
    for item in items(target) {
        for _ in 0..separators_above(previous, item.section) {
            out.push("---".to_string());
        }
        previous = Some(item.section);
        out.push(item.label.to_string());
    }
    out
}

/// An open menu: where it is and what it was opened on.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    pub position: Point<Pixels>,
    /// Whether a row's submenu is showing beside it: the pointer rests on
    /// the row, or did last.
    pub submenu: bool,
    /// Which corner of the list sits at `position`: the top-left for a
    /// right click, the top-right for a menu hung under a button at the
    /// window's right edge, the way the view header's **More options**
    /// opens right-aligned under its button.
    pub anchor: Anchor,
    pub target: Target,
}

/// The open menu as the control socket sees it: what it was opened on and
/// the rows a reader would see, `"---"` per separator. Without this a
/// harness can right-click but cannot read back what came up, which is what
/// kept the menus untestable from outside the process.
pub fn describe(menu: &Menu) -> Value {
    let target = match &menu.target {
        Target::File(_) => "file",
        Target::Folder(_) => "folder",
        Target::Blank => "blank",
        Target::Linked(_) => "linked",
        Target::Editor(click) => match click.spot {
            Spot::Text => "editor",
            Spot::Tag(_) => "editor-tag",
            Spot::Link(_) => "editor-link",
        },
        Target::View(_) | Target::Note { .. } => "view",
        Target::Row(_) => "row",
        Target::Property { .. } => "property",
        Target::Bookmark { .. } => "bookmark",
        Target::Tab { .. } => "tab",
    };
    let items = items(&menu.target);
    let disabled: Vec<String> = items
        .iter()
        .filter(|i| i.disabled)
        .map(|i| i.label.to_string())
        .collect();
    let mut value = json!({
        "target": target,
        "path": menu.target.path(),
        "items": drawn(&menu.target),
        "disabled": disabled,
        "submenu": menu.submenu,
    });
    // What the editor's menu was built from, which no other target has.
    if let Target::Editor(click) = &menu.target {
        value["selection"] = json!(click.selection);
        value["selected"] = json!(click.selected);
        if let Spot::Tag(tag) = &click.spot {
            value["tag"] = json!(tag);
        }
    }
    value
}

/// The menu drawn at its position over everything else: a press outside it
/// runs `on_close`; a click on a row runs `on_close` and then `on_pick` with
/// the row's [`Pick`]. The view owns the [`Menu`] and clears it in
/// `on_close`; this only draws.
#[allow(clippy::too_many_arguments)]
pub fn render<V: 'static>(
    menu: &Menu,
    theme: &Theme,
    focus: &FocusHandle,
    viewport_width: Pixels,
    cx: &mut Context<V>,
    on_close: impl Fn(&mut V, &mut Context<V>) + 'static,
    on_pick: impl Fn(&mut V, Pick, &mut Window, &mut Context<V>) + 'static,
    on_submenu: impl Fn(&mut V, bool, &mut Context<V>) + 'static,
) -> AnyElement {
    let on_close = Rc::new(on_close);
    let on_pick = Rc::new(on_pick);
    let on_submenu = Rc::new(on_submenu);
    let submenu_open = menu.submenu;
    // A submenu opens beside its row on the right, or on the left when the
    // menu stands near the window's right edge and there is no room.
    let flip_submenu = menu.position.x + px(180. + 140.) > viewport_width;
    let over_submenu = OVER_SUBMENU.with(Rc::clone);
    if !menu.submenu {
        // A menu closed under the pointer never hears the hover end.
        over_submenu.set(false);
    }
    let mut list = menu_box_with_submenu(
        // Obsidian's `.menu`: `--menu-background` inside a
        // `--menu-border-color` line, rows in `--text-normal` at the small
        // UI size, icons `--text-muted`, a warning row `--text-error`, the
        // hover wash `--background-modifier-hover` (probe of 2026-11-02).
        div()
            .id("file-menu")
            .flex()
            .flex_col()
            .min_w(px(180.))
            .py_1()
            .bg(theme.tokens.menu_background)
            .border_1()
            .border_color(theme.tokens.menu_border_color)
            .rounded_md()
            .shadow_lg()
            .text_size(px(13.))
            .text_color(theme.tokens.text_normal),
        focus,
        cx,
        {
            let on_close = on_close.clone();
            move |this, cx| on_close(this, cx)
        },
        over_submenu.clone(),
    );
    let mut previous: Option<Section> = None;
    for (ix, item) in items(&menu.target).into_iter().enumerate() {
        // Obsidian emits two separators before the danger pair but draws
        // only the first: the second is `display: none` (its rect is zero
        // in every capture, 2026-11-07 `s06`). The model keeps both, so
        // [`drawn`] still matches the DOM; the eye sees one line.
        for n in 0..separators_above(previous, item.section).min(1) {
            list = list.child(
                div()
                    .id(("file-menu-separator", ix * 2 + n))
                    .my_1()
                    .h(px(1.))
                    .w_full()
                    .bg(theme.tokens.background_modifier_border),
            );
        }
        previous = Some(item.section);
        let pick = item.pick;
        let on_close_row = on_close.clone();
        let on_pick_row = on_pick.clone();
        let has_submenu = !item.submenu.is_empty();
        let on_hover = on_submenu.clone();
        // Obsidian's `is-disabled` row: greyed, no hover, and a click on it
        // closes nothing and does nothing.
        let faded = theme.tokens.text_faint;
        let color = if item.warning {
            theme.tokens.text_error
        } else {
            theme.tokens.text_normal
        };
        let mut row = div()
            .id(("file-menu-item", ix))
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .text_color(color)
            .when(item.disabled, |d| d.text_color(faded))
            .when(!item.disabled, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered {
                            on_hover(this, has_submenu, cx);
                        }
                    }))
                    .when(!has_submenu, |d| {
                        d.on_click(cx.listener(move |this, _, window, cx| {
                            on_close_row(this, cx);
                            on_pick_row(this, pick, window, cx);
                        }))
                    })
            })
            .child(crate::assets::icon(
                item.icon,
                16.,
                if item.disabled {
                    faded
                } else {
                    theme.tokens.text_muted
                },
            ))
            .child(div().flex_1().child(item.label.clone()))
            .when(item.checked, |d| {
                d.child(crate::assets::icon("check", 14., theme.tokens.text_muted))
            })
            .when(has_submenu, |d| {
                d.child(crate::assets::icon(
                    "chevron-right",
                    14.,
                    theme.tokens.text_muted,
                ))
            });
        // The submenu stands beside its row, open while the pointer rested
        // on that row last; its rows pick like the menu's own.
        if has_submenu && submenu_open {
            let mut sub = div()
                .id(("file-submenu", ix))
                .absolute()
                .when(!flip_submenu, |d| d.left(relative(1.)))
                .when(flip_submenu, |d| d.right(relative(1.)))
                .top(px(-6.))
                .flex()
                .flex_col()
                .min_w(px(126.))
                .py_1()
                .bg(theme.tokens.menu_background)
                .border_1()
                .border_color(theme.tokens.menu_border_color)
                .rounded_md()
                .shadow_lg()
                .occlude()
                .on_hover({
                    let over_submenu = over_submenu.clone();
                    move |hovered: &bool, _, _| over_submenu.set(*hovered)
                });
            for (sx, subitem) in item.submenu.iter().enumerate() {
                let pick = subitem.pick;
                let on_close = on_close.clone();
                let on_pick = on_pick.clone();
                sub = sub.child(
                    div()
                        .id(("file-submenu-item", ix * 64 + sx))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .text_color(theme.tokens.text_normal)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            on_close(this, cx);
                            on_pick(this, pick, window, cx);
                        }))
                        .child(crate::assets::icon(
                            subitem.icon,
                            16.,
                            theme.tokens.text_muted,
                        ))
                        .child(div().flex_1().child(subitem.label.clone()))
                        .when(subitem.checked, |d| {
                            d.child(crate::assets::icon("check", 14., theme.tokens.text_muted))
                        }),
                );
            }
            row = row.child(sub);
        }
        list = list.child(row);
    }
    deferred(
        anchored()
            .position(menu.position)
            .anchor(menu.anchor)
            .snap_to_window_with_margin(px(8.))
            .child(list),
    )
    .with_priority(1)
    .into_any_element()
}

// ----- "Show in system explorer" --------------------------------------------

/// A `file://` URL for an absolute path, every byte outside RFC 3986's
/// unreserved set and the `/` percent-encoded — spaces, `#`, `?`, `%` and
/// anything non-ASCII included — so a file manager reads the name back
/// exactly, which `ShowItems` needs since it selects the file by it. Only
/// the Linux call wants one, so a Windows path's drive and backslashes are
/// not special-cased here.
pub fn file_url(absolute: &Path) -> String {
    let text = absolute.to_string_lossy();
    let mut url = String::from("file://");
    for &byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                url.push(byte as char)
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

/// One command line that selects a file in the desktop's file manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reveal {
    pub program: &'static str,
    pub args: Vec<String>,
}

/// The command lines that select `absolute` in the desktop's file manager,
/// in the order to try them: each but the last must spawn and exit 0 or the
/// next is tried; the last is spawned and let go, the way "Open in default
/// app" is, because the handler it starts outlives this process.
///
/// Linux asks the session bus's `org.freedesktop.FileManager1.ShowItems`
/// through `dbus-send`, which every mainstream file manager answers, and
/// falls back to `xdg-open` on the containing folder where nothing does.
/// macOS has `open -R`; Windows has `explorer /select,<path>`, one argument
/// with the comma attached. No crate is added for this.
pub fn reveal_commands(absolute: &Path) -> Vec<Reveal> {
    if cfg!(target_os = "macos") {
        vec![Reveal {
            program: "open",
            args: vec!["-R".into(), absolute.display().to_string()],
        }]
    } else if cfg!(target_os = "windows") {
        vec![Reveal {
            program: "explorer",
            args: vec![format!("/select,{}", absolute.display())],
        }]
    } else {
        linux_reveal_commands(absolute)
    }
}

fn linux_reveal_commands(absolute: &Path) -> Vec<Reveal> {
    let parent: PathBuf = absolute
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| absolute.to_path_buf());
    vec![
        Reveal {
            program: "dbus-send",
            args: vec![
                "--session".into(),
                "--print-reply".into(),
                "--dest=org.freedesktop.FileManager1".into(),
                "/org/freedesktop/FileManager1".into(),
                "org.freedesktop.FileManager1.ShowItems".into(),
                format!("array:string:{}", file_url(absolute)),
                "string:".into(),
            ],
        },
        Reveal {
            program: "xdg-open",
            args: vec![parent.display().to_string()],
        },
    ]
}

/// Runs [`reveal_commands`] in order and reports the error of the last,
/// if it could not be spawned either. Blocking: the D-Bus call waits for the
/// file manager's reply, so callers run this off the UI thread.
pub fn show_in_system_explorer(absolute: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let commands = reveal_commands(absolute);
    let last = commands.len().saturating_sub(1);
    let mut err = String::new();
    for (ix, reveal) in commands.into_iter().enumerate() {
        let mut command = Command::new(reveal.program);
        command
            .args(&reveal.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if ix == last {
            return command
                .spawn()
                .map(drop)
                .map_err(|e| format!("{}: {e}", reveal.program));
        }
        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => err = format!("{} exited with {status}", reveal.program),
            Err(e) => err = format!("{}: {e}", reveal.program),
        }
    }
    Err(err)
}

/// Opens `dir` itself in the desktop's file manager, which is what the
/// Appearance tab's "Open themes folder" and "Open snippets folder" do.
///
/// Unlike [`show_in_system_explorer`] nothing is selected, so the plain
/// platform opener is the whole command: `xdg-open` on Linux, `open` on
/// macOS and `explorer` on Windows. Blocking in the same way, so callers
/// run it off the UI thread.
pub fn open_folder(dir: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    Command::new(program)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("{program}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    /// [[Specification#The file explorer's context menus]], the file-row
    /// table, minus the one item Scoria has no command for -- Open version
    /// history. Dropping it empties the `view` section, so the separator the
    /// capture has around it is gone too; every other separator is where
    /// Obsidian put it, including the pair above Rename.
    #[test]
    fn the_file_rows_menu_is_obsidians_order_without_the_rows_it_cannot_run() {
        assert_eq!(
            drawn(&Target::File("Notes/Plan.md".into())),
            [
                "Open in new tab",
                "Open to the right",
                "Open in new window",
                "---",
                "Make a copy",
                "Move file to...",
                "Bookmark...",
                "Merge entire file with...",
                "---",
                "Copy path",
                "---",
                "Open in default app",
                "Show in system explorer",
                "---",
                "---",
                "Rename...",
                "Delete",
            ]
        );
    }

    /// The folder-row table, whole. A folder gets no "Open in default app":
    /// the capture has that item on the file row only, and "Show in system
    /// explorer" is its whole `system` section.
    #[test]
    fn the_folder_rows_menu_is_obsidians_order() {
        assert_eq!(
            drawn(&Target::Folder("Notes".into())),
            [
                "New note",
                "New folder",
                "New canvas",
                "New base",
                "---",
                "Make a copy",
                "Move folder to...",
                "Search in folder",
                "Bookmark...",
                "---",
                "Copy path",
                "---",
                "Show in system explorer",
                "---",
                "---",
                "Rename...",
                "Delete",
            ]
        );
    }

    /// "The empty space below the last row gives only the four
    /// `action-primary` items" -- so no separators at all, and every one of
    /// them rooted at the vault.
    #[test]
    fn the_empty_space_offers_the_four_creation_items_at_the_root() {
        assert_eq!(
            drawn(&Target::Blank),
            ["New note", "New folder", "New canvas", "New base"]
        );
        assert_eq!(Target::Blank.path(), "");
    }

    /// The 2026-10-22 probe's `s13-context-menu`, read off a Links row of
    /// the Outgoing links pane (`results/2026-10-22-outgoing-links.json`),
    /// whole: Rename sits with Move and Bookmark under one separator rather
    /// than in a `danger` pair, there is no Make a copy and no Delete, and
    /// "Reveal file in navigation" closes the list.
    #[test]
    fn a_linked_rows_menu_is_the_probes_list() {
        assert_eq!(
            drawn(&Target::Linked("Notes/Plan.md".into())),
            [
                "Open in new tab",
                "Open to the right",
                "Open in new window",
                "---",
                "Rename...",
                "Move file to...",
                "Bookmark...",
                "---",
                "Copy path",
                "---",
                "Open in default app",
                "Show in system explorer",
                "Reveal file in navigation",
            ]
        );
    }

    /// The view header's More options over a base leaf (`s02` `moreOptions`,
    /// 2026-11-01), minus the two `view` rows and their separator: the
    /// splits and the window row, Rename with Move and Bookmark, Copy
    /// path, the three system rows, and "Delete file" after the danger
    /// pair.
    #[test]
    fn a_base_leafs_more_options_is_the_probes_list_without_the_rows_it_cannot_run() {
        assert_eq!(
            drawn(&Target::View("Shelf/Rocks.base".into())),
            [
                "Split right",
                "Split down",
                "Open in new window",
                "---",
                "Rename...",
                "Move file to...",
                "Bookmark...",
                "---",
                "Copy path",
                "---",
                "Open in default app",
                "Show in system explorer",
                "Reveal file in navigation",
                "---",
                "---",
                "Delete file",
            ]
        );
        assert_eq!(
            Target::View("Shelf/Rocks.base".into()).path(),
            "Shelf/Rocks.base"
        );
    }

    fn tab(path: &str, ix: usize, tabs: usize, pinned: bool, active: bool) -> Target {
        Target::Tab {
            group: 1,
            ix,
            path: path.into(),
            tabs,
            pinned,
            active,
            backlinks: None,
        }
    }

    /// "Backlinks in document" on an active markdown tab sits after Pin in
    /// the `pane` section, checked while the block shows (the
    /// 2026-11-09 panes probe, `bl` `tabMenu`); an inactive tab has none.
    #[test]
    fn the_tab_menu_has_backlinks_in_document_after_pin() {
        let on = |backlinks, active| Target::Tab {
            group: 1,
            ix: 0,
            path: "Target.md".into(),
            tabs: 1,
            pinned: false,
            active,
            backlinks,
        };
        let rows = items(&on(Some(true), true));
        let labels: Vec<&str> = rows.iter().take(4).map(|i| i.label.as_ref()).collect();
        assert_eq!(
            labels,
            [
                "Close",
                "Pin",
                "Backlinks in document",
                "Move to new window"
            ]
        );
        let row = &rows[2];
        assert!(row.checked);
        assert_eq!(row.icon, "links-coming-in");
        assert_eq!(row.section, Section::Pane);
        assert_eq!(row.pick, Pick::ToggleBacklinksInDocument);
        assert!(!items(&on(Some(false), true))[2].checked);
        assert!(!drawn(&on(Some(true), false)).contains(&"Backlinks in document".to_string()));
    }

    /// More options over a markdown leaf: "Backlinks in document" first,
    /// then the view header's rows (`bl` `moreMenu`).
    #[test]
    fn more_options_on_a_note_starts_with_backlinks_in_document() {
        let target = Target::Note {
            path: "Target.md".into(),
            backlinks: false,
        };
        let shown = drawn(&target);
        assert_eq!(
            &shown[..4],
            ["Backlinks in document", "---", "Split right", "Split down"]
        );
        assert!(!items(&target)[0].checked);
        assert_eq!(target.path(), "Target.md");
        let checked = Target::Note {
            path: "Target.md".into(),
            backlinks: true,
        };
        assert!(items(&checked)[0].checked);
    }

    /// The tab header's menu over the middle of three markdown tabs, the
    /// active one (`2026-11-09-tab-header-menu.json`, cases 1 and 2), minus
    /// the rows `tab_items` names as left out — which takes the `view`
    /// section's separator with them.
    #[test]
    fn a_markdown_tabs_menu_is_the_probes_list_without_the_rows_it_cannot_run() {
        let target = tab("Notes/Alpha.md", 1, 3, false, true);
        assert_eq!(
            drawn(&target),
            [
                "Close",
                "Close others",
                "Close tabs after",
                "Close all",
                "---",
                "Pin",
                "---",
                "Move to new window",
                "Split right",
                "Split down",
                "Open in new window",
                "---",
                "Rename...",
                "Move file to...",
                "Bookmark...",
                "Merge entire file with...",
                "Add file property",
                "---",
                "Find...",
                "Replace...",
                "---",
                "Copy path",
                "---",
                "Open in default app",
                "Show in system explorer",
                "Reveal file in navigation",
                "---",
                "---",
                "Delete file",
            ]
        );
        assert_eq!(target.path(), "Notes/Alpha.md");
    }

    /// The last of two tabs has no Close tabs after; a canvas has no
    /// markdown rows; a pinned tab's row reads Unpin.
    #[test]
    fn a_last_canvas_tab_has_no_close_after_and_no_markdown_rows() {
        assert_eq!(
            drawn(&tab("Board.canvas", 1, 2, true, true)),
            [
                "Close",
                "Close others",
                "Close all",
                "---",
                "Unpin",
                "---",
                "Move to new window",
                "Split right",
                "Split down",
                "Open in new window",
                "---",
                "Rename...",
                "Move file to...",
                "Bookmark...",
                "---",
                "Copy path",
                "---",
                "Open in default app",
                "Show in system explorer",
                "Reveal file in navigation",
                "---",
                "---",
                "Delete file",
            ]
        );
    }

    /// An inactive tab's menu stops after Move to new window, and an empty
    /// tab's after the splits (cases 2 and 4).
    #[test]
    fn an_inactive_tab_and_an_empty_tab_have_the_short_menus() {
        assert_eq!(
            drawn(&tab("Alpha.md", 0, 3, false, false)),
            [
                "Close",
                "Close others",
                "Close tabs after",
                "Close all",
                "---",
                "Pin",
                "---",
                "Move to new window",
            ]
        );
        assert_eq!(
            drawn(&tab("", 0, 1, false, true)),
            [
                "Close",
                "---",
                "Pin",
                "---",
                "Move to new window",
                "Split right",
                "Split down",
            ]
        );
    }

    /// A Bases row's name cell, card or list link (`s24` `linkMenu`), minus
    /// the `view` rows and their separator.
    #[test]
    fn a_bases_rows_menu_is_the_links_list_without_the_rows_it_cannot_run() {
        assert_eq!(
            drawn(&Target::Row("Rocks/Basalt.md".into())),
            [
                "Open in new tab",
                "Open to the right",
                "Open in new window",
                "---",
                "Rename...",
                "Move file to...",
                "Bookmark...",
                "---",
                "Copy path",
                "---",
                "Open in default app",
                "Show in system explorer",
                "Reveal file in navigation",
                "---",
                "---",
                "Delete file",
            ]
        );
    }

    /// The All properties row's menu: Rename, Property type with its six
    /// types (the current one checked), a separator, Delete property.
    #[test]
    fn a_property_rows_menu_is_rename_type_and_delete() {
        use scoria_formats::properties::PropertyType;
        let target = Target::Property {
            name: "status".into(),
            ty: Some(PropertyType::Number),
        };
        assert_eq!(
            drawn(&target),
            ["Rename", "Property type", "---", "Delete property"]
        );
        let items = items(&target);
        let types: Vec<(&str, bool)> = items[1]
            .submenu
            .iter()
            .map(|i| (i.label.as_ref(), i.checked))
            .collect();
        assert_eq!(
            types,
            [
                ("Checkbox", false),
                ("Date", false),
                ("Date & time", false),
                ("List", false),
                ("Number", true),
                ("Text", false),
            ]
        );
        assert!(items[2].warning);
        assert!(!items[0].disabled && !items[2].disabled);
        assert_eq!(target.path(), "");
    }

    /// The three keys Obsidian defines keep their menu, but Rename and
    /// Delete property are disabled on them.
    #[test]
    fn the_reserved_keys_cannot_be_renamed_or_deleted() {
        for name in ["tags", "aliases", "cssclasses"] {
            let items = items(&Target::Property {
                name: name.into(),
                ty: None,
            });
            let disabled: Vec<bool> = items.iter().map(|i| i.disabled).collect();
            assert_eq!(disabled, [true, false, true], "{name}");
        }
    }

    /// The editor click the plain menu came from: no selection, plain text.
    fn plain_click() -> Target {
        Target::Editor(EditorClick {
            spot: Spot::Text,
            selection: String::new(),
            selected: false,
        })
    }

    /// The rows of the editor's clipboard section, which every editor menu
    /// carries, with the labels Obsidian drew disabled marked.
    fn clipboard_rows(target: &Target) -> Vec<(String, bool)> {
        items(target)
            .into_iter()
            .filter(|i| i.section == Section::Clipboard)
            .map(|i| (i.label.to_string(), i.disabled))
            .collect()
    }

    /// The plain menu of `s01-editor-menu-live`, minus the three submenu
    /// rows nobody has opened: the two link rows, one separator where
    /// Format's section was, then the clipboard five.
    #[test]
    fn the_editor_menu_is_obsidians_plain_list_without_the_submenus() {
        assert_eq!(
            drawn(&plain_click()),
            [
                "Add link",
                "Add external link",
                "---",
                "Cut",
                "Copy",
                "Paste",
                "Paste as plain text",
                "Select all",
            ]
        );
    }

    /// "Cut and Copy are drawn disabled with no selection" -- and enabled
    /// with one; Paste, Paste as plain text and Select all never are.
    #[test]
    fn cut_and_copy_are_disabled_only_while_nothing_is_selected() {
        assert_eq!(
            clipboard_rows(&plain_click()),
            [
                ("Cut".to_string(), true),
                ("Copy".to_string(), true),
                ("Paste".to_string(), false),
                ("Paste as plain text".to_string(), false),
                ("Select all".to_string(), false),
            ]
        );
        let live = Target::Editor(EditorClick {
            spot: Spot::Text,
            selection: "paragraph with".into(),
            selected: true,
        });
        assert_eq!(
            clipboard_rows(&live)
                .into_iter()
                .filter(|(_, d)| *d)
                .count(),
            0,
            "with a selection no clipboard row is disabled"
        );
    }

    /// With a selection the `selection` section opens with Obsidian's
    /// quoted Search row and Extract current selection....
    #[test]
    fn a_selection_adds_the_quoted_search_row() {
        let target = Target::Editor(EditorClick {
            spot: Spot::Text,
            selection: "paragraph with".into(),
            selected: true,
        });
        assert_eq!(
            drawn(&target),
            [
                "Add link",
                "Add external link",
                "---",
                "Search for \u{201c}paragraph with\u{201d}",
                "Extract current selection...",
                "---",
                "Cut",
                "Copy",
                "Paste",
                "Paste as plain text",
                "Select all",
            ]
        );
    }

    /// A right click outside a selection: the Search row is built from the
    /// selection as it was, and Cut and Copy from the one that did not
    /// survive -- both at once, which is what the probe read back.
    #[test]
    fn a_click_outside_the_selection_keeps_the_search_row_and_disables_cut() {
        let target = Target::Editor(EditorClick {
            spot: Spot::Text,
            selection: "paragraph with".into(),
            selected: false,
        });
        assert!(drawn(&target).contains(&"Search for \u{201c}paragraph with\u{201d}".to_string()));
        assert_eq!(
            clipboard_rows(&target)[..2],
            [("Cut".to_string(), true), ("Copy".to_string(), true)]
        );
    }

    /// "On a tag the Format row is replaced by Edit tag; the rest is the
    /// plain menu." Edit tag has no command behind it and Format is a
    /// submenu, so both are out and a tag's rows are the plain menu's --
    /// the one place the tag's name still reaches is [`describe`].
    #[test]
    fn a_tag_draws_the_plain_menu_until_edit_tag_has_a_command() {
        let tag = Target::Editor(EditorClick {
            spot: Spot::Tag("project".into()),
            selection: String::new(),
            selected: false,
        });
        assert_eq!(drawn(&tag), drawn(&plain_click()));
        assert_eq!(tag.path(), "", "a tag names no file");
    }

    /// `s01-editor-menu-live`'s `onWikilink`, minus Edit link, Paragraph,
    /// Insert and Start presentation: the three Open rows above the text
    /// rows, then Copy path, the action three and the
    /// system three. Dropping Start presentation empties the `view`
    /// section, so the capture's two consecutive separators are one here.
    #[test]
    fn a_wikilink_wraps_the_text_rows_in_the_file_menu() {
        let target = Target::Editor(EditorClick {
            spot: Spot::Link("Notes/Plan.md".into()),
            selection: String::new(),
            selected: false,
        });
        assert_eq!(
            drawn(&target),
            [
                "Open in new tab",
                "Open to the right",
                "Open in new window",
                "---",
                "Add link",
                "Add external link",
                "---",
                "Cut",
                "Copy",
                "Paste",
                "Paste as plain text",
                "Select all",
                "---",
                "Copy path",
                "---",
                "Rename...",
                "Move file to...",
                "Bookmark...",
                "---",
                "Open in default app",
                "Show in system explorer",
                "Reveal file in navigation",
            ]
        );
        assert_eq!(target.path(), "Notes/Plan.md");
    }

    /// The caret rule: a point inside the selection keeps it, a point
    /// outside moves the caret there, and with nothing selected every point
    /// moves it -- which is the right click that placed the caret on the
    /// word it landed on.
    #[test]
    fn a_right_click_keeps_a_selection_it_lands_in_and_moves_the_caret_otherwise() {
        let selection = 6..20;
        assert_eq!(right_click_caret(&selection, 12), None);
        assert_eq!(right_click_caret(&selection, 6), None, "its head");
        assert_eq!(right_click_caret(&selection, 20), None, "its tail");
        assert_eq!(right_click_caret(&selection, 5), Some(5));
        assert_eq!(right_click_caret(&selection, 35), Some(35));
        // No selection: the caret goes to the click wherever that is, the
        // click's own offset included.
        assert_eq!(right_click_caret(&(0..0), 30), Some(30));
        assert_eq!(right_click_caret(&(30..30), 30), Some(30));
    }

    /// Obsidian's file row has no creation items, and a row away from the
    /// Files view none either.
    #[test]
    fn a_file_row_offers_nothing_that_creates_a_file() {
        for target in [
            Target::File("Plan.md".into()),
            Target::Linked("Plan.md".into()),
        ] {
            for item in items(&target) {
                assert!(
                    !item.label.starts_with("New "),
                    "{target:?} offered {}",
                    item.label
                );
            }
        }
    }

    /// Only the Files view can be revealed in, and only its rows can delete
    /// or copy: the picks a view has no handler for are never offered to it.
    #[test]
    fn each_target_offers_only_the_picks_its_view_handles() {
        let picks = |t: &Target| items(t).iter().map(|i| i.pick).collect::<Vec<_>>();
        for target in [
            Target::File("Plan.md".into()),
            Target::Folder("Notes".into()),
            Target::Blank,
        ] {
            assert!(!picks(&target).contains(&Pick::RevealInNavigation));
        }
        let linked = picks(&Target::Linked("Plan.md".into()));
        for pick in [
            Pick::Delete,
            Pick::Duplicate,
            Pick::NewNote,
            Pick::NewFolder,
            Pick::NewCanvas,
            Pick::NewBase,
            Pick::SearchInFolder,
        ] {
            assert!(!linked.contains(&pick), "a linked row offered {pick:?}");
        }
    }

    /// The path is the target's: the one place every item of a menu reads it
    /// from.
    #[test]
    fn every_target_carries_the_row_it_was_opened_on() {
        assert_eq!(
            Target::File("Notes/Deep/Plan.md".into()).path(),
            "Notes/Deep/Plan.md"
        );
        assert_eq!(Target::Folder("Notes/Deep".into()).path(), "Notes/Deep");
        assert_eq!(
            Target::Linked("Notes/Plan.md".into()).path(),
            "Notes/Plan.md"
        );
    }

    /// A separator where the section changes, two above `danger`, none at the
    /// top, and none between two items of one section.
    #[test]
    fn a_separator_falls_where_the_section_changes() {
        assert_eq!(separators_above(None, Section::Open), 0);
        assert_eq!(separators_above(Some(Section::Open), Section::Open), 0);
        assert_eq!(separators_above(Some(Section::Open), Section::Action), 1);
        assert_eq!(
            separators_above(Some(Section::Action), Section::Unsectioned),
            1
        );
        assert_eq!(separators_above(Some(Section::System), Section::Danger), 2);
        assert_eq!(separators_above(Some(Section::Danger), Section::Danger), 0);
    }

    /// The socket's view of a menu: the kind, the path and the drawn rows.
    #[test]
    fn describe_reports_the_target_and_the_drawn_rows() {
        let menu = Menu {
            position: Point::new(px(10.), px(20.)),
            submenu: false,
            anchor: Anchor::TopLeft,
            target: Target::Linked("Notes/Plan.md".into()),
        };
        let value = describe(&menu);
        assert_eq!(value["target"], "linked");
        assert_eq!(value["path"], "Notes/Plan.md");
        assert_eq!(value["items"][0], "Open in new tab");
        assert_eq!(value["items"][2], "Open in new window");
        assert_eq!(value["items"][3], "---");
    }

    /// The icon of every item in every menu is bundled and is the name
    /// Obsidian leaves in its DOM, or Lucide's current spelling of it where
    /// Obsidian's is an old alias: `lucide-edit` is `square-pen` and
    /// `lucide-edit-3` is `pen-line`, both from Lucide's own alias data.
    #[test]
    fn every_menu_icon_is_bundled() {
        let targets = [
            Target::File("Plan.md".into()),
            Target::Folder("Notes".into()),
            Target::Blank,
            Target::Linked("Plan.md".into()),
            plain_click(),
            Target::Editor(EditorClick {
                spot: Spot::Link("Plan.md".into()),
                selection: "some words".into(),
                selected: true,
            }),
        ];
        for target in targets {
            for item in items(&target) {
                let path = format!("icons/{}.svg", item.icon);
                assert!(
                    crate::assets::Assets.load(&path).unwrap().is_some(),
                    "{} wants {path}, which is not bundled",
                    item.label
                );
            }
        }
    }

    /// Spaces, the characters a URL gives meaning to, and non-ASCII bytes
    /// are percent-encoded; letters, digits, `-._~` and the slashes pass.
    #[test]
    fn a_file_url_escapes_what_a_file_manager_would_misread() {
        assert_eq!(
            file_url(Path::new("/home/me/My Vault/Notes/Plan.md")),
            "file:///home/me/My%20Vault/Notes/Plan.md"
        );
        assert_eq!(
            file_url(Path::new("/v/Café #1/100%?.md")),
            "file:///v/Caf%C3%A9%20%231/100%25%3F.md"
        );
        assert_eq!(
            file_url(Path::new("/v/a-b_c.d~e/f")),
            "file:///v/a-b_c.d~e/f"
        );
    }

    /// The D-Bus call is the first thing tried on Linux and `xdg-open` on
    /// the containing folder the last: the ShowItems argument carries the
    /// file's URL as one `array:string:` and an empty startup id.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_asks_file_manager1_first_and_opens_the_folder_last() {
        let commands = reveal_commands(Path::new("/home/me/My Vault/Notes/Plan.md"));
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].program, "dbus-send");
        assert_eq!(
            commands[0].args,
            [
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.FileManager1",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
                "array:string:file:///home/me/My%20Vault/Notes/Plan.md",
                "string:",
            ]
        );
        assert_eq!(commands[1].program, "xdg-open");
        assert_eq!(commands[1].args, ["/home/me/My Vault/Notes"]);
    }

    /// `open -R` on macOS; `explorer /select,<path>` on Windows, the comma
    /// attached because `explorer` splits its one argument on it.
    #[test]
    fn the_other_desktops_get_their_one_command() {
        let path = Path::new("/v/Plan.md");
        let commands = reveal_commands(path);
        if cfg!(target_os = "macos") {
            assert_eq!(
                commands,
                [Reveal {
                    program: "open",
                    args: vec!["-R".into(), "/v/Plan.md".into()]
                }]
            );
        } else if cfg!(target_os = "windows") {
            assert_eq!(commands.len(), 1);
            assert_eq!(commands[0].program, "explorer");
            assert!(commands[0].args[0].starts_with("/select,"));
        } else {
            assert_eq!(commands.last().unwrap().program, "xdg-open");
        }
    }

    /// Every Bookmarks pane menu as the 2026-11-07 probe captured it
    /// (`s06`, `s13`), the window row included since the popouts landed.
    #[test]
    fn a_bookmark_row_menu_is_the_one_obsidian_draws_for_its_kind() {
        let menu = |kind| {
            drawn(&Target::Bookmark {
                kind,
                path: String::new(),
            })
        };
        let open = [
            "Open in new tab",
            "Open to the right",
            "Open in new window",
            "---",
        ];
        let file = [
            "Rename",
            "Edit...",
            "---",
            "Reveal file in navigation",
            "---",
            "---",
            "Remove",
        ];
        assert_eq!(menu(BookmarkKind::File), [&open[..], &file].concat());
        assert_eq!(menu(BookmarkKind::Folder), file);
        let plain = ["Rename", "Edit...", "---", "---", "Remove"];
        assert_eq!(menu(BookmarkKind::Search), plain);
        assert_eq!(menu(BookmarkKind::Url), plain);
        assert_eq!(menu(BookmarkKind::Graph), [&open[..], &plain].concat());
        assert_eq!(
            menu(BookmarkKind::Group),
            [
                &open[..],
                &[
                    "Rename",
                    "Bookmark the active tab...",
                    "New group",
                    "---",
                    "---",
                    "Remove"
                ]
            ]
            .concat()
        );
        assert_eq!(menu(BookmarkKind::Blank), ["New group"]);
    }
}
