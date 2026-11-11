//! The Files view: a folder tree over the vault's file list in the left dock.
//!
//! [`FileTree`] is the pure model (ordering, collapse state, visible rows)
//! and [`FileExplorer`] the GPUI entity that draws it with a uniform list, so
//! a 20k-file vault costs one row element per visible line. Names are ordered
//! with [`natural_cmp`], the collation the search pane uses, so two lists of
//! the same files in one window cannot disagree, and the files of each
//! folder follow the leaf's [`SortOrder`]. A right click opens the file menu
//! ([`crate::file_menu`], shared with the Backlinks and Outgoing links
//! panes); the explorer only reports the choice as an [`ExplorerEvent`], the
//! workspace does the work. Above the tree sits
//! Obsidian's `nav-header`: New note, New folder, Change sort order,
//! Auto-reveal current file, and Expand all (Collapse all once a folder is
//! open).
//!
//! The tree takes the keyboard too, under the "Explorer" key context: one
//! row carries the focus ring, Up and Down walk the visible rows, Right
//! opens a folder or steps into it, Left closes one or climbs to the parent,
//! Enter opens a file or toggles a folder, F2 and Delete act on the row,
//! and Escape hands the keyboard back to the main area — what the
//! 2026-10-21 `explorer-keyboard` probe saw Obsidian do.
//!
//! A row renames **in place**, the way Obsidian's does: F2 and the menu's
//! "Rename..." turn the row's own label into a field
//! ([`crate::inline_field`], shared with the view header's title), with the
//! name selected and no dialog anywhere. The explorer does not rename
//! anything itself -- Enter reports the typed name as
//! [`ExplorerEvent::RenameTo`] and the workspace closes the field only if
//! the rename went through.

use crate::assets::icon;
use crate::canvas::menu::tooltip;
use crate::file_menu::{self, Menu, Pick, Target};
use crate::inline_field;
use crate::theme::{Theme, ICON_SIZE};
use gpui::{
    actions, anchored, canvas, deferred, div, point, prelude::*, px, uniform_list, App, Bounds,
    ClickEvent, Context, DragMoveEvent, ElementInputHandler, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, FontWeight, KeyBinding, MouseButton, MouseDownEvent, Pixels, Point,
    ScrollStrategy, SharedString, UTF16Selection, UniformListScrollHandle, Window,
};
use scoria_formats::FileKind;
use scoria_search::natural_cmp;
use scoria_vault::VaultFile;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

/// The font size of a row's extension tag.
const TAG_SIZE: f32 = 10.0;

actions!(
    explorer,
    [
        FocusUp,
        FocusDown,
        FocusLeft,
        FocusRight,
        Activate,
        ToggleFolder,
        LeaveTree,
        RenameFocused,
        DeleteFocused
    ]
);

/// Key bindings for the tree; called once at startup. The context is the
/// explorer's own, beneath the workspace's.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Explorer");
    cx.bind_keys([
        KeyBinding::new("up", FocusUp, ctx),
        KeyBinding::new("down", FocusDown, ctx),
        KeyBinding::new("left", FocusLeft, ctx),
        KeyBinding::new("right", FocusRight, ctx),
        KeyBinding::new("enter", Activate, ctx),
        KeyBinding::new("space", ToggleFolder, ctx),
        KeyBinding::new("escape", LeaveTree, ctx),
        KeyBinding::new("f2", RenameFocused, ctx),
        KeyBinding::new("delete", DeleteFocused, ctx),
    ]);
}

/// The header row: Obsidian's `.nav-header` is 40 px, 8 px of padding
/// around a centred row of 28 by 24 buttons 2 px apart.
const HEADER_HEIGHT: f32 = 40.0;

/// The order the files of each folder are listed in: the six values of the
/// Files leaf's `sortOrder`. Folders keep their name order whichever is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Alphabetical,
    AlphabeticalReverse,
    ByModifiedTime,
    ByModifiedTimeReverse,
    ByCreatedTime,
    ByCreatedTimeReverse,
}

impl SortOrder {
    /// Every order, in the order the menu lists them.
    pub const ALL: [SortOrder; 6] = [
        SortOrder::Alphabetical,
        SortOrder::AlphabeticalReverse,
        SortOrder::ByModifiedTime,
        SortOrder::ByModifiedTimeReverse,
        SortOrder::ByCreatedTime,
        SortOrder::ByCreatedTimeReverse,
    ];

    /// The string `workspace.json` records.
    pub fn as_str(self) -> &'static str {
        match self {
            SortOrder::Alphabetical => "alphabetical",
            SortOrder::AlphabeticalReverse => "alphabeticalReverse",
            SortOrder::ByModifiedTime => "byModifiedTime",
            SortOrder::ByModifiedTimeReverse => "byModifiedTimeReverse",
            SortOrder::ByCreatedTime => "byCreatedTime",
            SortOrder::ByCreatedTimeReverse => "byCreatedTimeReverse",
        }
    }

    pub fn parse(s: &str) -> Option<SortOrder> {
        SortOrder::ALL.into_iter().find(|o| o.as_str() == s)
    }

    /// The menu's label.
    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Alphabetical => "File name (A to Z)",
            SortOrder::AlphabeticalReverse => "File name (Z to A)",
            SortOrder::ByModifiedTime => "Modified time (new to old)",
            SortOrder::ByModifiedTimeReverse => "Modified time (old to new)",
            SortOrder::ByCreatedTime => "Created time (new to old)",
            SortOrder::ByCreatedTimeReverse => "Created time (old to new)",
        }
    }

    /// How two files of one folder compare under this order. Names tie
    /// break every time sort, and the full file name breaks a tie between
    /// two display names, so the order of a rescan is stable.
    fn compare(self, a: &Entry, b: &Entry) -> Ordering {
        let by_name = |a: &Entry, b: &Entry| {
            natural_cmp(display_name(&a.name), display_name(&b.name))
                .then_with(|| natural_cmp(&a.name, &b.name))
        };
        match self {
            SortOrder::Alphabetical => by_name(a, b),
            SortOrder::AlphabeticalReverse => {
                natural_cmp(display_name(&b.name), display_name(&a.name))
                    .then_with(|| natural_cmp(&a.name, &b.name))
            }
            SortOrder::ByModifiedTime => b.mtime.cmp(&a.mtime).then_with(|| by_name(a, b)),
            SortOrder::ByModifiedTimeReverse => a.mtime.cmp(&b.mtime).then_with(|| by_name(a, b)),
            SortOrder::ByCreatedTime => b.ctime.cmp(&a.ctime).then_with(|| by_name(a, b)),
            SortOrder::ByCreatedTimeReverse => a.ctime.cmp(&b.ctime).then_with(|| by_name(a, b)),
        }
    }
}

/// One line of the tree with every folder expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Vault-relative path; folders without a trailing slash.
    pub path: String,
    /// Name as shown: a file loses its extension, which `tag` carries.
    pub name: String,
    /// The extension of a file that is not Markdown, upper-cased at the
    /// row's right end the way Obsidian's `nav-file-tag` shows it.
    pub tag: Option<String>,
    pub depth: usize,
    pub is_folder: bool,
    pub kind: FileKind,
}

/// A row dragged out of the Files view. Obsidian's drag carries an
/// `obsidian://open` url for a file and the bare name for a folder
/// (2026-10-25); inside Scoria the path is what every drop target needs.
/// `action` is the ghost's second line, which the target under the pointer
/// writes ("Move into “Dest”") and clears when it has nothing to say.
#[derive(Clone)]
pub struct FileDrag {
    pub path: String,
    pub folder: bool,
    pub name: SharedString,
    pub action: Rc<RefCell<Option<SharedString>>>,
}

impl FileDrag {
    pub fn set_action(&self, action: Option<SharedString>) {
        *self.action.borrow_mut() = action;
    }
}

/// The ghost under the pointer: Obsidian's `drag-ghost` (2026-11-07
/// `explorer-drag-look`): 85 % black, `6px 8px`, radius 4, a shadow, at most
/// 300 px; the icon (`lucide-file`, `lucide-folder-open` for a folder) at 0.7
/// and the name at 13 px weight 500, then the action at 12 px and 0.7.
///
/// gpui draws a drag's view where the row was grabbed, so the view pads
/// itself by the grab point to sit 5 px below and right of the pointer,
/// where Obsidian's sits (`s03`: the ghost at the pointer plus 5).
pub struct FileGhost(pub FileDrag, pub Point<Pixels>);

impl Render for FileGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let action = self.0.action.borrow().clone();
        let white = gpui::white();
        let mut faded = white;
        faded.a = 0.7;
        let grab = self.1;
        div().pl(grab.x + px(5.)).pt(grab.y + px(5.)).child(
            div()
                .max_w(px(300.))
                .px(px(8.))
                .py(px(6.))
                .rounded(px(4.))
                .bg(gpui::hsla(0., 0., 0., 0.85))
                .shadow_md()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(white)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(div().mr(px(4.)).child(icon(
                            if self.0.folder { "folder-open" } else { "file" },
                            ICON_SIZE,
                            faded,
                        )))
                        .child(self.0.name.clone()),
                )
                .children(action.map(|a| {
                    div()
                        .pt(px(2.))
                        .text_size(px(12.))
                        .text_color(faded)
                        .child(a)
                })),
        )
    }
}

/// The folder a drop over a row lands in: a folder row is its own target,
/// a file row its folder's (`""` at the root), which is what Obsidian
/// washed when a file was held over a file row (`s02`, `s03`).
pub fn drop_folder(row_path: &str, row_is_folder: bool) -> String {
    if row_is_folder {
        row_path.to_string()
    } else {
        row_path
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_string())
            .unwrap_or_default()
    }
}

/// The ghost's action line for a target folder: `Move into “Dest”`, the
/// vault's own name standing for the root (`s03`).
pub fn move_into_label(folder: &str, vault_name: &str) -> String {
    let name = if folder.is_empty() {
        vault_name
    } else {
        folder.rsplit('/').next().unwrap_or(folder)
    };
    format!("Move into \u{201c}{name}\u{201d}")
}

/// Whether dropping `path` in `folder` moves anything: not into the folder
/// it is already in, and a folder not into itself or anything inside it,
/// which Obsidian refused by leaving the tree byte-identical (2026-10-25).
pub fn drop_moves(path: &str, path_is_folder: bool, folder: &str) -> bool {
    let parent = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    if parent == folder {
        return false;
    }
    !(path_is_folder && (folder == path || folder.starts_with(&format!("{path}/"))))
}

/// A file as its folder holds it: what the sort orders need of it.
#[derive(Debug, Clone)]
struct Entry {
    name: String,
    kind: FileKind,
    mtime: u64,
    ctime: u64,
}

#[derive(Debug, Default)]
struct Folder {
    folders: BTreeMap<String, Folder>,
    files: Vec<Entry>,
}

/// Folder tree with per-folder collapse state.
#[derive(Debug, Default)]
pub struct FileTree {
    root: Folder,
    sort: SortOrder,
    all: Vec<Row>,
    collapsed: HashSet<String>,
    visible: Vec<usize>,
}

impl FileTree {
    /// Builds the tree. Files of unknown kind are hidden unless
    /// `show_unsupported` (Obsidian's "Detect all file extensions");
    /// `folders` adds the empty folders the files alone would not show.
    pub fn new<'a>(
        files: impl IntoIterator<Item = &'a VaultFile>,
        folders: impl IntoIterator<Item = &'a String>,
        show_unsupported: bool,
    ) -> FileTree {
        let mut root = Folder::default();
        for path in folders {
            let mut folder = &mut root;
            for part in path.split('/').filter(|p| !p.is_empty()) {
                folder = folder.folders.entry(part.to_string()).or_default();
            }
        }
        for file in files {
            if file.kind == FileKind::Other && !show_unsupported {
                continue;
            }
            let mut parts: Vec<&str> = file.path.split('/').collect();
            let name = parts.pop().unwrap_or_default().to_string();
            let mut folder = &mut root;
            for part in parts {
                folder = folder.folders.entry(part.to_string()).or_default();
            }
            folder.files.push(Entry {
                name,
                kind: file.kind,
                mtime: file.mtime,
                ctime: file.ctime,
            });
        }
        let mut all = Vec::new();
        flatten(&root, "", 0, SortOrder::Alphabetical, &mut all);
        let collapsed = all
            .iter()
            .filter(|r| r.is_folder)
            .map(|r| r.path.clone())
            .collect();
        let mut tree = FileTree {
            root,
            sort: SortOrder::Alphabetical,
            all,
            collapsed,
            visible: Vec::new(),
        };
        tree.recompute();
        tree
    }

    pub fn sort(&self) -> SortOrder {
        self.sort
    }

    /// Re-lists every folder's files in `sort`; folders and their collapse
    /// state stay as they were.
    pub fn set_sort(&mut self, sort: SortOrder) {
        if self.sort == sort {
            return;
        }
        self.sort = sort;
        self.all.clear();
        flatten(&self.root, "", 0, sort, &mut self.all);
        self.recompute();
    }

    fn recompute(&mut self) {
        self.visible.clear();
        let mut hide_below: Option<usize> = None;
        for (ix, row) in self.all.iter().enumerate() {
            if let Some(depth) = hide_below {
                if row.depth > depth {
                    continue;
                }
                hide_below = None;
            }
            self.visible.push(ix);
            if row.is_folder && self.collapsed.contains(&row.path) {
                hide_below = Some(row.depth);
            }
        }
    }

    /// Rows currently shown, in order.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &Row> {
        self.visible.iter().map(|&ix| &self.all[ix])
    }

    pub fn row(&self, visible_ix: usize) -> Option<&Row> {
        self.visible.get(visible_ix).map(|&ix| &self.all[ix])
    }

    pub fn len(&self) -> usize {
        self.visible.len()
    }

    /// Folders currently expanded, to carry over to a rebuilt tree.
    pub fn expanded_folders(&self) -> HashSet<String> {
        self.all
            .iter()
            .filter(|r| r.is_folder && !self.collapsed.contains(&r.path))
            .map(|r| r.path.clone())
            .collect()
    }

    /// Expands the given folders (unknown ones are ignored).
    pub fn expand_folders(&mut self, folders: &HashSet<String>) {
        self.collapsed.retain(|f| !folders.contains(f));
        self.recompute();
    }

    pub fn is_collapsed(&self, folder: &str) -> bool {
        self.collapsed.contains(folder)
    }

    pub fn toggle(&mut self, folder: &str) {
        if !self.collapsed.remove(folder) {
            self.collapsed.insert(folder.to_string());
        }
        self.recompute();
    }

    pub fn collapse_all(&mut self) {
        self.collapsed = self
            .all
            .iter()
            .filter(|r| r.is_folder)
            .map(|r| r.path.clone())
            .collect();
        self.recompute();
    }

    pub fn expand_all(&mut self) {
        self.collapsed.clear();
        self.recompute();
    }

    /// Whether any folder is open: the header's last button is "Collapse
    /// all" then and "Expand all" otherwise, as Obsidian's swaps.
    pub fn any_expanded(&self) -> bool {
        self.all
            .iter()
            .any(|r| r.is_folder && !self.collapsed.contains(&r.path))
    }

    /// Expands every ancestor of `path` and returns its visible row index.
    pub fn reveal(&mut self, path: &str) -> Option<usize> {
        let mut prefix = String::new();
        for part in path.split('/') {
            if !prefix.is_empty() {
                self.collapsed.remove(&prefix);
                prefix.push('/');
            }
            prefix.push_str(part);
        }
        self.recompute();
        self.visible_index(path)
    }

    pub fn visible_index(&self, path: &str) -> Option<usize> {
        self.visible
            .iter()
            .position(|&ix| self.all[ix].path == path)
    }
}

fn flatten(folder: &Folder, prefix: &str, depth: usize, sort: SortOrder, out: &mut Vec<Row>) {
    let mut names: Vec<&String> = folder.folders.keys().collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    for name in names {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        out.push(Row {
            path: path.clone(),
            name: name.clone(),
            depth,
            tag: None,
            is_folder: true,
            kind: FileKind::Other,
        });
        flatten(&folder.folders[name], &path, depth + 1, sort, out);
    }
    let mut files: Vec<&Entry> = folder.files.iter().collect();
    files.sort_by(|a, b| sort.compare(a, b));
    for entry in files {
        let name = &entry.name;
        out.push(Row {
            path: if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            },
            name: stem(name).to_string(),
            tag: file_tag(name),
            depth,
            is_folder: false,
            kind: entry.kind,
        });
    }
}

/// The name a [`FileKind`] goes by outside the app: in the control socket's
/// state dump, and anywhere else a kind has to survive as text.
pub fn kind_name(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Markdown => "markdown",
        FileKind::Canvas => "canvas",
        FileKind::Base => "base",
        FileKind::Image => "image",
        FileKind::Audio => "audio",
        FileKind::Video => "video",
        FileKind::Pdf => "pdf",
        FileKind::Other => "other",
    }
}

/// A file's name without its extension, for the explorer's rows: Obsidian
/// shows every file bare and tags the ones that are not Markdown.
fn stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => stem,
        _ => file_name,
    }
}

/// The row's tag for a file that is not Markdown: its extension in upper
/// case (`CANVAS`, `PNG`); none for a note or a file without one.
fn file_tag(file_name: &str) -> Option<String> {
    match file_name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty() && !ext.is_empty() && !ext.eq_ignore_ascii_case("md") =>
        {
            Some(ext.to_ascii_uppercase())
        }
        _ => None,
    }
}

/// Markdown files show without their extension, everything else with it.
pub fn display_name(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("md") && !stem.is_empty() => stem,
        _ => file_name,
    }
}

pub enum ExplorerEvent {
    Open(String),
    /// The pointer came onto a file's row (`over`) or left it: the row's
    /// bounds anchor the page preview.
    Hover {
        path: String,
        over: bool,
        anchor: Bounds<Pixels>,
        modifiers: gpui::Modifiers,
    },
    /// A row dropped in a folder (`""` for the vault root).
    MoveInto {
        path: String,
        folder: String,
    },
    /// Enter in a row being renamed in place: the name typed into the
    /// label, without the extension the row never showed. The workspace
    /// validates it and renames; the field stays open until it says the
    /// rename went through, which is what Obsidian does with a name the
    /// filesystem refuses (the 2026-10-25 pass typed a colliding name and
    /// an empty one and the editor stayed up with neither a notice nor a
    /// modal).
    RenameTo {
        path: String,
        name: String,
    },
    /// A Ctrl+click: the file goes to a new tab.
    OpenInNewTab(String),
    /// The header's "New note": what the `file-explorer:new-file` command
    /// does, the folder following `newFileLocation`.
    NewNoteFromHeader,
    /// Context-menu actions; paths are vault-relative, folders without a
    /// trailing slash (`""` is the vault root). The header's "New folder"
    /// is a `NewFolder` at the root.
    NewNote {
        folder: String,
    },
    NewFolder {
        parent: String,
    },
    /// "New canvas" and "New base" from a folder row, which take the folder
    /// the click was on rather than the vault root the commands use.
    NewCanvas {
        folder: String,
    },
    NewBase {
        folder: String,
    },
    Rename(String),
    Move(String),
    Duplicate(String),
    Delete(String),
    /// "Bookmark...": a file or folder for the Bookmarks pane.
    Bookmark(String),
    /// "Merge entire file with...": the Note composer's merge of this note.
    MergeWith(String),
    /// "Open to the right": a new tab group beside the active one showing
    /// this file.
    OpenToRight(String),
    /// "Open in new window": this file in a popout window of its own.
    OpenInNewWindow(String),
    /// "Copy path": the vault-relative path onto the clipboard.
    CopyPath(String),
    /// "Open in default app": hand the file to the desktop.
    OpenInDefaultApp(String),
    /// "Show in system explorer": the file or folder selected in the
    /// desktop's file manager.
    ShowInSystemExplorer(String),
    /// "Search in folder": the Search pane scoped to this folder.
    SearchInFolder(String),
    /// Escape in the tree: the keyboard goes back to the main area.
    LeaveFocus,
}

/// The [`ExplorerEvent`] a menu row's pick stands for, on the row the menu
/// was opened on. `None` for a pick the explorer's menus never offer
/// ([`Pick::RevealInNavigation`] is the Backlinks pane's).
fn event_for(pick: Pick, target: &Target) -> Option<ExplorerEvent> {
    use ExplorerEvent as E;
    let path = || target.path().to_string();
    Some(match pick {
        Pick::OpenInNewTab => E::OpenInNewTab(path()),
        Pick::OpenToRight => E::OpenToRight(path()),
        Pick::OpenInNewWindow => E::OpenInNewWindow(path()),
        Pick::Duplicate => E::Duplicate(path()),
        Pick::Move => E::Move(path()),
        Pick::Bookmark => E::Bookmark(path()),
        Pick::MergeWith => E::MergeWith(path()),
        Pick::CopyPath => E::CopyPath(path()),
        Pick::OpenInDefaultApp => E::OpenInDefaultApp(path()),
        Pick::ShowInSystemExplorer => E::ShowInSystemExplorer(path()),
        Pick::Rename => E::Rename(path()),
        Pick::Delete => E::Delete(path()),
        Pick::NewNote => E::NewNote { folder: path() },
        Pick::NewFolder => E::NewFolder { parent: path() },
        Pick::NewCanvas => E::NewCanvas { folder: path() },
        Pick::NewBase => E::NewBase { folder: path() },
        Pick::SearchInFolder => E::SearchInFolder(path()),
        // "Reveal file in navigation" reveals *in* the explorer, the tab
        // rows are the tab header's, the splits are the view header's, and the rest are the editor menu's rows,
        // which act on the editor.
        Pick::RevealInNavigation
        | Pick::CloseTab
        | Pick::CloseOthers
        | Pick::CloseAfter
        | Pick::CloseAll
        | Pick::AddFileProperty
        | Pick::Find
        | Pick::Replace
        | Pick::TogglePin
        | Pick::MoveToNewWindow
        | Pick::ToggleBacklinksInDocument
        | Pick::SplitRight
        | Pick::SplitDown
        | Pick::RenameProperty
        | Pick::DeleteProperty
        | Pick::SetPropertyType(_)
        | Pick::EditBookmark
        | Pick::RemoveBookmark
        | Pick::BookmarkActiveTab
        | Pick::NewBookmarkGroup
        | Pick::AddLink
        | Pick::AddExternalLink
        | Pick::SearchForSelection
        | Pick::Cut
        | Pick::Copy
        | Pick::Paste
        | Pick::PastePlain
        | Pick::SelectAll
        | Pick::ExtractSelection => return None,
    })
}

pub struct FileExplorer {
    tree: FileTree,
    active: Option<String>,
    /// The leaf's `autoReveal`: whether opening a file expands its folders
    /// and scrolls to its row, or only highlights the row if it is on
    /// screen. Off in a fresh vault, as in Obsidian.
    auto_reveal: bool,
    scroll: UniformListScrollHandle,
    menu: Option<Menu>,
    /// The sort menu is open under its header button.
    sort_menu_open: bool,
    /// The keyboard the open menu holds, and where it goes back to.
    menu_focus: file_menu::MenuFocus,
    focus_handle: FocusHandle,
    /// The row the keyboard is on (Obsidian's `has-focus`), by path so it
    /// survives a re-sort; it may be hidden under a collapsed folder, and
    /// then the next key starts over from the open file.
    focused: Option<String>,
    /// The row being renamed in place, if any (Obsidian's
    /// `is-being-renamed`).
    renaming: Option<RowEdit>,
    /// The keyboard an open field holds, and where it goes back to: the
    /// same "focus while open, restore after" helper a menu uses.
    rename_focus: file_menu::MenuFocus,
    /// The vault's name, which the ghost says for a drop at the root.
    vault_name: String,
    /// While a row is dragged: the row, and the folder the pointer is over
    /// (`""` for the root), with where the pointer was when a row claimed
    /// it, so the list's own handler can tell a row's move from its own.
    dragging: Option<String>,
    drop: Option<String>,
    drop_claim: Option<Point<Pixels>>,
}

/// A row whose label is being typed into.
struct RowEdit {
    /// The file or folder the field opened on. The edit is dropped if that
    /// row leaves the tree, so a commit can only ever rename what the row
    /// showed.
    path: String,
    /// The text and the input method's composition in it.
    editing: inline_field::Editing,
    /// Where the label was last painted, which is what a click inside it
    /// is measured against. `None` until the first paint.
    layout: Option<inline_field::Layout>,
}

impl Focusable for FileExplorer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<ExplorerEvent> for FileExplorer {}

impl FileExplorer {
    pub fn new(cx: &mut Context<Self>) -> FileExplorer {
        FileExplorer {
            tree: FileTree::default(),
            active: None,
            auto_reveal: false,
            scroll: UniformListScrollHandle::new(),
            menu: None,
            sort_menu_open: false,
            menu_focus: file_menu::MenuFocus::new(cx),
            focus_handle: cx.focus_handle(),
            focused: None,
            renaming: None,
            rename_focus: file_menu::MenuFocus::new(cx),
            vault_name: String::new(),
            dragging: None,
            drop: None,
            drop_claim: None,
        }
    }

    /// Gives the tree the keyboard, on the row it had or none: what
    /// `file-explorer:open` does.
    pub fn focus_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.end_rename(cx);
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Expands to `path`, puts the focus ring on its row and gives the tree
    /// the keyboard: `file-explorer:reveal-active-file`.
    pub fn reveal_and_focus(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.end_rename(cx);
        self.reveal(path, cx);
        self.focused = Some(path.to_string());
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Turns a row's own label into a field with the whole name selected:
    /// what F2 on a focused row and the menu's "Rename..." both do in
    /// Obsidian (`tools/obsidian-probe/results/2026-10-25-explorer-dnd.json`,
    /// steps `s09` and `s11`). The name is the one the row shows, so a
    /// file's extension is neither selected nor typed over.
    pub fn start_rename(&mut self, path: &str, cx: &mut Context<Self>) {
        self.reveal(path, cx);
        let Some(row) = self
            .tree
            .visible_index(path)
            .and_then(|ix| self.tree.row(ix).cloned())
        else {
            return;
        };
        self.focused = Some(row.path.clone());
        self.renaming = Some(RowEdit {
            path: row.path,
            editing: inline_field::Editing::selected(row.name),
            layout: None,
        });
        cx.notify();
    }

    /// Closes the field, with the keyboard going back to the tree. Called
    /// when the rename went through, and by Escape.
    pub fn end_rename(&mut self, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            cx.notify();
        }
    }

    /// The visible index of the focused row, if it is on a visible row.
    fn focused_ix(&self) -> Option<usize> {
        self.tree.visible_index(self.focused.as_deref()?)
    }

    fn set_focused_ix(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.tree.row(ix) {
            self.focused = Some(row.path.clone());
            self.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Where Up and Down start from when no visible row has the ring: the
    /// open file's row if a folder is not hiding it, else the first row.
    fn start_ix(&self) -> usize {
        self.active
            .as_deref()
            .and_then(|p| self.tree.visible_index(p))
            .unwrap_or(0)
    }

    fn focus_down(&mut self, _: &FocusDown, _: &mut Window, cx: &mut Context<Self>) {
        let next = match self.focused_ix() {
            Some(ix) => (ix + 1).min(self.tree.len().saturating_sub(1)),
            None => self.start_ix(),
        };
        self.set_focused_ix(next, cx);
    }

    fn focus_up(&mut self, _: &FocusUp, _: &mut Window, cx: &mut Context<Self>) {
        let next = match self.focused_ix() {
            Some(ix) => ix.saturating_sub(1),
            None => self.start_ix(),
        };
        self.set_focused_ix(next, cx);
    }

    /// Right opens a collapsed folder, steps into an open one, and does
    /// nothing on a file.
    fn focus_right(&mut self, _: &FocusRight, _: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.focused_ix() else {
            return;
        };
        let Some(row) = self.tree.row(ix).cloned() else {
            return;
        };
        if !row.is_folder {
            return;
        }
        if self.tree.is_collapsed(&row.path) {
            self.tree.toggle(&row.path);
            cx.notify();
        } else if self.tree.row(ix + 1).is_some_and(|r| r.depth > row.depth) {
            self.set_focused_ix(ix + 1, cx);
        }
    }

    /// Left closes an open folder; on a file or a closed folder it climbs
    /// to the parent and closes that, or does nothing at the root.
    fn focus_left(&mut self, _: &FocusLeft, _: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.focused_ix() else {
            return;
        };
        let Some(row) = self.tree.row(ix).cloned() else {
            return;
        };
        if row.is_folder && !self.tree.is_collapsed(&row.path) {
            self.tree.toggle(&row.path);
            cx.notify();
            return;
        }
        let Some((parent, _)) = row.path.rsplit_once('/') else {
            return;
        };
        let parent = parent.to_string();
        if !self.tree.is_collapsed(&parent) {
            self.tree.toggle(&parent);
        }
        if let Some(parent_ix) = self.tree.visible_index(&parent) {
            self.set_focused_ix(parent_ix, cx);
        }
        cx.notify();
    }

    /// Enter opens the file or toggles the folder under the ring.
    fn activate(&mut self, _: &Activate, _: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.focused_ix().and_then(|ix| self.tree.row(ix).cloned()) else {
            return;
        };
        if row.is_folder {
            self.tree.toggle(&row.path);
            cx.notify();
        } else {
            cx.emit(ExplorerEvent::Open(row.path));
        }
    }

    /// Space toggles a folder and leaves a file alone.
    fn toggle_folder(&mut self, _: &ToggleFolder, _: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.focused_ix().and_then(|ix| self.tree.row(ix).cloned()) else {
            return;
        };
        if row.is_folder {
            self.tree.toggle(&row.path);
            cx.notify();
        }
    }

    fn leave_tree(&mut self, _: &LeaveTree, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ExplorerEvent::LeaveFocus);
    }

    fn rename_focused(&mut self, _: &RenameFocused, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.focused_ix().and_then(|ix| self.tree.row(ix).cloned()) {
            cx.emit(ExplorerEvent::Rename(row.path));
        }
    }

    fn delete_focused(&mut self, _: &DeleteFocused, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.focused_ix().and_then(|ix| self.tree.row(ix).cloned()) {
            cx.emit(ExplorerEvent::Delete(row.path));
        }
    }

    /// A rebuilt tree: the folders that were open stay open, the sort order
    /// carries over and the open file keeps its highlight. Nothing is
    /// expanded or scrolled, so a file written elsewhere in the vault does
    /// not move the list.
    pub fn set_tree(&mut self, mut tree: FileTree, cx: &mut Context<Self>) {
        let expanded = self.tree.expanded_folders();
        tree.expand_folders(&expanded);
        tree.set_sort(self.tree.sort());
        self.tree = tree;
        // A row that is no longer in the tree cannot paint the field that
        // is on it, and a field nothing paints would hold the keyboard
        // with nothing on screen.
        let gone = self
            .renaming
            .as_ref()
            .is_some_and(|edit| self.tree.visible_index(&edit.path).is_none());
        if gone {
            self.renaming = None;
        }
        cx.notify();
    }

    pub fn set_vault_name(&mut self, name: String) {
        self.vault_name = name;
    }

    /// The pointer over a row while a row is dragged: the folder it would
    /// land in becomes the target, and the ghost says so.
    fn drag_over_row(
        &mut self,
        ix: usize,
        event: &DragMoveEvent<FileDrag>,
        cx: &mut Context<Self>,
    ) {
        if !event.bounds.contains(&event.event.position) {
            return;
        }
        let Some(row) = self.tree.row(ix) else {
            return;
        };
        let drag = event.drag(cx).clone();
        let folder = drop_folder(&row.path, row.is_folder);
        self.drop_claim = Some(event.event.position);
        self.dragged_over(drag, Some(folder), cx);
    }

    /// The pointer anywhere while a row is dragged: inside the list but on
    /// no row is the vault root (`s04`); outside it, no target at all.
    fn drag_over_list(&mut self, event: &DragMoveEvent<FileDrag>, cx: &mut Context<Self>) {
        if self.drop_claim == Some(event.event.position) {
            return;
        }
        let drag = event.drag(cx).clone();
        let inside = event.bounds.contains(&event.event.position);
        self.dragged_over(drag, inside.then(String::new), cx);
    }

    fn dragged_over(&mut self, drag: FileDrag, folder: Option<String>, cx: &mut Context<Self>) {
        drag.set_action(
            folder
                .as_deref()
                .map(|f| move_into_label(f, &self.vault_name).into()),
        );
        let dragging = Some(drag.path.clone());
        if self.drop != folder || self.dragging != dragging {
            self.drop = folder;
            self.dragging = dragging;
            cx.notify();
        }
        // The ghost reads the action when it is drawn.
        cx.refresh_windows();
    }

    fn dropped(&mut self, drag: &FileDrag, cx: &mut Context<Self>) {
        let target = self.drop.take();
        self.dragging = None;
        self.drop_claim = None;
        if let Some(folder) = target {
            if drop_moves(&drag.path, drag.folder, &folder) {
                cx.emit(ExplorerEvent::MoveInto {
                    path: drag.path.clone(),
                    folder,
                });
            }
        }
        cx.notify();
    }

    /// The leaf's state as `workspace.json` recorded it.
    pub fn set_leaf_state(&mut self, sort: SortOrder, auto_reveal: bool, cx: &mut Context<Self>) {
        self.tree.set_sort(sort);
        self.auto_reveal = auto_reveal;
        cx.notify();
    }

    pub fn sort_order(&self) -> SortOrder {
        self.tree.sort()
    }

    pub fn auto_reveal(&self) -> bool {
        self.auto_reveal
    }

    fn set_sort(&mut self, sort: SortOrder, cx: &mut Context<Self>) {
        self.tree.set_sort(sort);
        self.sort_menu_open = false;
        cx.notify();
    }

    /// "Auto-reveal current file": switching it on reveals the open file
    /// right away, as Obsidian does.
    fn toggle_auto_reveal(&mut self, cx: &mut Context<Self>) {
        self.auto_reveal = !self.auto_reveal;
        if self.auto_reveal {
            if let Some(active) = self.active.clone() {
                self.reveal(&active, cx);
            }
        }
        cx.notify();
    }

    /// "Expand all" opens every folder at every depth; "Collapse all", the
    /// same button once any folder is open, closes them all.
    fn toggle_all(&mut self, cx: &mut Context<Self>) {
        if self.tree.any_expanded() {
            self.tree.collapse_all();
        } else {
            self.tree.expand_all();
        }
        cx.notify();
    }

    /// The file that just became the open one: highlighted, and with
    /// auto-reveal on, also expanded into view.
    pub fn follow(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.auto_reveal {
            self.reveal(path, cx);
        } else {
            self.set_active(path, cx);
        }
    }

    /// Marks `path` as the open file without touching the tree: its row is
    /// highlighted if a folder is not hiding it.
    pub fn set_active(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.active.as_deref() != Some(path) {
            self.active = Some(path.to_string());
            cx.notify();
        }
    }

    /// No file is open: nothing is highlighted.
    pub fn clear_active(&mut self, cx: &mut Context<Self>) {
        if self.active.take().is_some() {
            cx.notify();
        }
    }

    /// Marks `path` as the open file and scrolls it into view.
    pub fn reveal(&mut self, path: &str, cx: &mut Context<Self>) {
        self.active = Some(path.to_string());
        self.show(path, cx);
    }

    /// Scrolls `path` into view without claiming it is the open file.
    ///
    /// A folder is never the open file, so revealing one — a bookmarked
    /// folder, or a folder just created — leaves the highlight on the note
    /// that is open, as Obsidian does.
    pub fn reveal_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        self.show(path, cx);
    }

    fn show(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(ix) = self.tree.reveal(path) {
            self.scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        cx.notify();
    }

    /// What the tree is showing, for the control socket's state dump: the
    /// rows a collapsed folder hides are not in it, because they are not on
    /// screen either.
    pub fn describe(&self, rows: usize) -> Value {
        let total = self.tree.len();
        let shown: Vec<Value> = self
            .tree
            .rows()
            .take(rows)
            .map(|row| {
                json!({
                    "path": row.path,
                    "name": row.name,
                    "depth": row.depth,
                    "folder": row.is_folder,
                    "kind": kind_name(row.kind),
                })
            })
            .collect();
        json!({
            "active": self.active,
            "focused": self.focused,
            "sort_order": self.tree.sort().as_str(),
            "auto_reveal": self.auto_reveal,
            "any_expanded": self.tree.any_expanded(),
            "rows": shown,
            "rows_total": if total > shown.len() { Some(total) } else { None },
            "menu": self.menu.as_ref().map(file_menu::describe),
            "renaming": self.renaming.as_ref().map(|edit| {
                let field = &edit.editing.field;
                json!({
                    "path": edit.path,
                    "text": field.text(),
                    "cursor": field.cursor(),
                    "selection": [field.selection().start, field.selection().end],
                })
            }),
        })
    }

    /// A click on a folder toggles it and puts the keyboard on it; a click
    /// on the open file's row puts the keyboard on that row; a click that
    /// opens a file hands the keyboard to the editor, as Obsidian's does.
    fn click(&mut self, ix: usize, new_tab: bool, window: &mut Window, cx: &mut Context<Self>) {
        // A press on another row is the end of an open rename: the field
        // keeps what it has, the way Escape does.
        self.end_rename(cx);
        let Some(row) = self.tree.row(ix).cloned() else {
            return;
        };
        if row.is_folder {
            self.tree.toggle(&row.path);
            self.focused = Some(row.path);
            window.focus(&self.focus_handle, cx);
            cx.notify();
        } else if new_tab {
            cx.emit(ExplorerEvent::OpenInNewTab(row.path));
        } else if self.active.as_deref() == Some(row.path.as_str()) {
            self.focused = Some(row.path);
            window.focus(&self.focus_handle, cx);
            cx.notify();
        } else {
            self.focused = Some(row.path.clone());
            cx.emit(ExplorerEvent::Open(row.path));
        }
    }

    fn open_menu(&mut self, ix: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        // The menu takes the keyboard, so an open field would be left
        // holding a caret it can no longer be typed into.
        self.end_rename(cx);
        if let Some(row) = self.tree.row(ix).cloned() {
            let target = match row.is_folder {
                true => Target::Folder(row.path),
                false => Target::File(row.path),
            };
            self.menu = Some(Menu {
                position,
                submenu: false,
                anchor: gpui::Anchor::TopLeft,
                target,
            });
            cx.notify();
        }
    }

    /// A right-click on the empty space below the last row: Obsidian's
    /// four-item menu, rooted at the vault.
    fn open_blank_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.menu = Some(Menu {
            position,
            submenu: false,
            anchor: gpui::Anchor::TopLeft,
            target: Target::Blank,
        });
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
        }
    }

    /// The file menu over the row (or the empty space) it was opened on; a
    /// pick becomes the [`ExplorerEvent`] the workspace acts on.
    fn render_menu(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let menu = self.menu.as_ref()?;
        let theme = cx.global::<Theme>().clone();
        let target = menu.target.clone();
        let focus = self.menu_focus.handle.clone();
        // The explorer sits at the window's left, so its submenus open to
        // the right; no window is at hand here and none is needed.
        Some(file_menu::render(
            menu,
            &theme,
            &focus,
            px(f32::MAX),
            cx,
            |this, cx| this.close_menu(cx),
            move |_, pick, _, cx| {
                if let Some(event) = event_for(pick, &target) {
                    cx.emit(event);
                }
            },
            |this, open, cx| {
                if let Some(menu) = this.menu.as_mut() {
                    if menu.submenu != open {
                        menu.submenu = open;
                        cx.notify();
                    }
                }
            },
        ))
    }

    /// A header button: Obsidian's `nav-action-button`, 28 by 24 with a
    /// 4 px radius. Its icon is `--icon-color` at `--icon-opacity` until
    /// the pointer is over it (`--icon-color-hover`, opaque); an
    /// `is-active` one sits on the hover wash in `--icon-color-focused`
    /// (probe of 2026-11-02, `clickable-icon.active`).
    fn header_button(
        id: &'static str,
        icon_name: &'static str,
        label: &'static str,
        active: bool,
        theme: &Theme,
    ) -> gpui::Stateful<gpui::Div> {
        let t = &theme.tokens;
        div()
            .id(id)
            .group(id)
            .flex_shrink_0()
            .w(px(28.))
            .h(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .cursor_pointer()
            .when(active, |d| d.bg(t.background_modifier_hover))
            .hover(|s| s.bg(t.background_modifier_hover))
            .tooltip(tooltip(label))
            .child(
                icon(
                    icon_name,
                    ICON_SIZE,
                    if active {
                        t.icon_color_focused
                    } else {
                        t.icon_color
                    },
                )
                .opacity(if active {
                    t.icon_opacity_active
                } else {
                    t.icon_opacity
                })
                .group_hover(id, |s| {
                    s.opacity(t.icon_opacity_hover)
                        .text_color(t.icon_color_hover)
                }),
            )
    }

    fn render_header(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.global::<Theme>().clone();
        let any_expanded = self.tree.any_expanded();
        let (fold_id, fold_icon, fold_label) = if any_expanded {
            ("explorer-collapse-all", "chevrons-down-up", "Collapse all")
        } else {
            ("explorer-expand-all", "chevrons-up-down", "Expand all")
        };
        let sort_menu = self.render_sort_menu(&theme, cx);
        div()
            .flex_shrink_0()
            .h(px(HEADER_HEIGHT))
            .p(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(2.))
            .child(
                Self::header_button("explorer-new-note", "square-pen", "New note", false, &theme)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ExplorerEvent::NewNoteFromHeader))),
            )
            .child(
                Self::header_button(
                    "explorer-new-folder",
                    "folder-plus",
                    "New folder",
                    false,
                    &theme,
                )
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.emit(ExplorerEvent::NewFolder {
                        parent: String::new(),
                    })
                })),
            )
            .child(
                // The menu hangs off this wrapper, so it opens under the
                // button wherever the dock is.
                div()
                    .relative()
                    .child(
                        Self::header_button(
                            "explorer-sort",
                            "arrow-up-narrow-wide",
                            "Change sort order",
                            false,
                            &theme,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.sort_menu_open = !this.sort_menu_open;
                                cx.notify();
                            },
                        )),
                    )
                    .children(sort_menu),
            )
            .child(
                Self::header_button(
                    "explorer-auto-reveal",
                    "gallery-vertical",
                    "Auto-reveal current file",
                    self.auto_reveal,
                    &theme,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_reveal(cx))),
            )
            .child(
                Self::header_button(fold_id, fold_icon, fold_label, false, &theme)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx))),
            )
    }

    /// "Change sort order": the six orders in three pairs, a check against
    /// the one in force.
    fn render_sort_menu(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.sort_menu_open {
            return None;
        }
        let current = self.tree.sort();
        let focus = self.menu_focus.handle.clone();
        let mut list = file_menu::menu_box(
            div()
                .id("explorer-sort-menu")
                .flex()
                .flex_col()
                .min_w(px(220.))
                .py_1()
                .bg(theme.tokens.menu_background)
                .border_1()
                .border_color(theme.tokens.background_modifier_border)
                .rounded_md()
                .shadow_lg()
                .text_size(px(13.))
                .text_color(theme.tokens.text_normal),
            &focus,
            cx,
            |this, cx| {
                this.sort_menu_open = false;
                cx.notify();
            },
        );
        for (ix, order) in SortOrder::ALL.into_iter().enumerate() {
            if ix > 0 && ix % 2 == 0 {
                list = list.child(
                    div()
                        .my_1()
                        .h(px(1.))
                        .bg(theme.tokens.background_modifier_border),
                );
            }
            let checked = order == current;
            list = list.child(
                div()
                    .id(("explorer-sort-item", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_sort(order, cx)))
                    .child(
                        div()
                            .w(px(16.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .children(checked.then(|| icon("check", 14., theme.tokens.text_muted))),
                    )
                    .child(order.label()),
            );
        }
        Some(
            deferred(
                anchored()
                    .offset(point(px(0.), px(26.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// The label of the row being renamed, painted as a shaped line so the
    /// caret and the selection sit in the running text and a click lands
    /// the caret where the pointer was -- the same painting the view
    /// header's title field uses.
    fn render_rename_field(
        &self,
        ix: usize,
        name: SharedString,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                let explorer = read_entity.read(cx);
                let edit = explorer.renaming.as_ref();
                let text: SharedString = match edit {
                    Some(edit) => edit.editing.field.text().to_string().into(),
                    None => name.clone(),
                };
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
                paint_entity.update(cx, |explorer, _| {
                    if let Some(edit) = explorer.renaming.as_mut() {
                        edit.layout = Some(layout);
                    }
                });
            },
        )
        .size_full();
        // The press is taken here rather than on the row: a row being
        // renamed does not open on a click, and the caret has to land
        // where the pointer was.
        div()
            .id(("rename-field", ix))
            .size_full()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.rename_clicked(event, window, cx);
                }),
            )
            .child(field)
    }

    /// A press inside the open field: the caret goes to the offset the
    /// pointer landed on, and a double press takes the word it landed in.
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

    fn render_rows(
        &mut self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Stateful<gpui::Div>> {
        let theme = cx.global::<Theme>().clone();
        let focused_ix = self.focused_ix();
        range
            .filter_map(|ix| {
                let row = self.tree.row(ix)?;
                let active = self.active.as_deref() == Some(row.path.as_str());
                let focused = focused_ix == Some(ix);
                let marker = row.is_folder.then(|| {
                    if self.tree.is_collapsed(&row.path) {
                        "chevron-right"
                    } else {
                        "chevron-down"
                    }
                });
                let name: SharedString = row.name.clone().into();
                let renaming = self.renaming.as_ref().is_some_and(|e| e.path == row.path);
                let tag: Option<SharedString> = row.tag.clone().map(Into::into);
                // A drag in progress: the dragged row is `is-being-dragged`
                // (the accent fill, white text); the target folder's whole
                // block — its row and the rows inside it — is
                // `is-being-dragged-over`, the accent at a tenth with a 4 px
                // radius, and the folder's own name turns accent
                // (2026-11-07 `explorer-drag-look`). The root target has no
                // wash.
                let dragged = self.dragging.as_deref() == Some(row.path.as_str());
                let in_block = |path: &str| {
                    self.drop.as_deref().is_some_and(|t| {
                        !t.is_empty() && (path == t || path.starts_with(&format!("{t}/")))
                    })
                };
                let washed = in_block(&row.path);
                let block_top = washed && self.drop.as_deref() == Some(row.path.as_str());
                let block_end = washed
                    && !self
                        .tree
                        .row(ix + 1)
                        .is_some_and(|next| in_block(&next.path));
                let drag = FileDrag {
                    path: row.path.clone(),
                    folder: row.is_folder,
                    name: row.name.clone().into(),
                    action: Rc::new(RefCell::new(None)),
                };
                // A file no view can open reads like any other row
                // (`is-unsupported` sets no colour of its own: the probe
                // of 2026-11-02 found `--nav-item-color` on it); only its
                // tag says what it is.
                let _ = FileKind::Other;
                // Obsidian's row: 13 px text in a 25 px row with an 8 px
                // radius, `--nav-item-color` until it is active
                // (`--nav-item-color-active` on `--nav-item-background-active`)
                // or under the pointer (`-hover`).
                let t = &theme.tokens;
                let row_el = div().id(("row", ix));
                // A file's row previews it; a folder's previews nothing.
                let row_el = if row.is_folder {
                    row_el
                } else {
                    let path = row.path.clone();
                    let this = cx.weak_entity();
                    // The row's 2 px ring border is part of its box.
                    crate::hover_preview::hover_row_bordered(
                        row_el,
                        2.,
                        move |over, anchor, modifiers, _, cx| {
                            let path = path.clone();
                            this.update(cx, |_, cx| {
                                cx.emit(ExplorerEvent::Hover {
                                    path,
                                    over,
                                    anchor,
                                    modifiers,
                                })
                            })
                            .ok();
                        },
                    )
                };
                Some(
                    row_el
                        .w_full()
                        .h(px(25.))
                        .mb(px(2.))
                        .flex()
                        .flex_row()
                        .items_center()
                        // The 2 px border is the keyboard's ring (Obsidian's
                        // `has-focus`), transparent on every other row so the
                        // text does not move when the ring arrives.
                        .border_2()
                        .border_color(if focused {
                            t.background_modifier_border_focus
                        } else {
                            gpui::transparent_black()
                        })
                        .pl(px(6. + row.depth as f32 * 14.))
                        .pr(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(if active {
                            t.nav_item_color_active
                        } else {
                            t.nav_item_color
                        })
                        .when(active, |d| d.bg(t.nav_item_background_active))
                        .hover(|s| {
                            s.bg(t.nav_item_background_hover)
                                .text_color(t.nav_item_color_hover)
                        })
                        .relative()
                        .when(block_top, |d| d.text_color(t.text_accent))
                        .when(dragged, |d| {
                            d.bg(t.interactive_accent).text_color(gpui::white())
                        })
                        .when(!renaming, |d| {
                            d.on_drag(drag, |drag, grab, _, cx| {
                                cx.new(|_| FileGhost(drag.clone(), grab))
                            })
                            .on_drag_move(cx.listener(
                                move |this, event: &DragMoveEvent<FileDrag>, _, cx| {
                                    this.drag_over_row(ix, event, cx)
                                },
                            ))
                        })
                        .when(!renaming, |d| {
                            d.on_click(cx.listener(
                                move |this, event: &gpui::ClickEvent, window, cx| {
                                    let modifiers = event.modifiers();
                                    let new_tab = modifiers.control || modifiers.platform;
                                    this.click(ix, new_tab, window, cx)
                                },
                            ))
                        })
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                // The empty space below the rows has its own
                                // right-click; without this the row's menu
                                // would open and then be replaced by that one.
                                cx.stop_propagation();
                                this.open_menu(ix, event.position, cx)
                            }),
                        )
                        // The wash runs through the 2 px gap under every
                        // row of the block but the last, so the block reads
                        // as one box; the row's 2 px ring border is inside it.
                        .when(washed, |d| {
                            let mut wash = t.interactive_accent;
                            wash.a = 0.1;
                            d.child(
                                div()
                                    .absolute()
                                    .top(px(-2.))
                                    .left(px(-2.))
                                    .right(px(-2.))
                                    .bottom(px(if block_end { -2. } else { -4. }))
                                    .bg(wash)
                                    .when(block_top, |d| d.rounded_t(px(4.)))
                                    .when(block_end, |d| d.rounded_b(px(4.))),
                            )
                        })
                        .child(
                            div()
                                .w(px(16.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                // The chevron: `--nav-collapse-icon-color`
                                // (the faint text colour) at `--icon-opacity`.
                                .opacity(t.icon_opacity)
                                .children(marker.map(|name| {
                                    crate::assets::icon(name, 14., t.nav_collapse_icon_color)
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .when(!renaming, |d| d.child(name.clone()))
                                // The field paints itself on a canvas,
                                // which has no size of its own: without a
                                // height to fill it draws nothing at all.
                                .when(renaming, |d| {
                                    d.h_full().child(self.render_rename_field(
                                        ix,
                                        name,
                                        theme.clone(),
                                        cx,
                                    ))
                                }),
                        )
                        .children(tag.map(|tag| {
                            // Obsidian's `nav-file-tag`: small upper-case
                            // text at the row's right end.
                            div()
                                .flex_shrink_0()
                                .ml_2()
                                .text_size(px(TAG_SIZE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.nav_tag_color)
                                .child(tag)
                        })),
                )
            })
            .collect()
    }
}

/// Enter and Escape in a row being renamed. Enter only *reports* the name:
/// the workspace owns the validator and the link rewrite, and closes the
/// field with [`FileExplorer::end_rename`] once the rename has happened.
/// Escape leaves the name as it was and the row keeps the focus ring, which
/// is what the 2026-10-25 pass saw.
impl inline_field::Host for FileExplorer {
    fn editing(&mut self) -> Option<&mut inline_field::Editing> {
        self.renaming.as_mut().map(|edit| &mut edit.editing)
    }

    fn commit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.renaming.as_ref() else {
            return;
        };
        cx.emit(ExplorerEvent::RenameTo {
            path: edit.path.clone(),
            name: edit.editing.field.text().trim().to_string(),
        });
    }

    fn cancel(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.end_rename(cx);
    }
}

/// Typed, composed and pasted text in a row being renamed; the halves are
/// [`inline_field`]'s, shared with the view header's title field. The
/// handler is registered by the label's own element against the field's own
/// focus handle, so it only ever sees text meant for that row.
impl EntityInputHandler for FileExplorer {
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
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.renaming.as_mut() else {
            return;
        };
        inline_field::replace_text_in_range(&mut edit.editing, range_utf16, new_text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.renaming.as_mut() else {
            return;
        };
        inline_field::replace_and_mark_text_in_range(
            &mut edit.editing,
            range_utf16,
            new_text,
            new_selected_range,
        );
        cx.notify();
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

impl Render for FileExplorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.tree.len();
        // An open menu holds the keyboard, so Escape reaches it first; a
        // closed one hands the keyboard back where it was.
        let menu_open = self.menu.is_some() || self.sort_menu_open;
        self.menu_focus.sync(menu_open, window, cx);
        // A row being renamed holds the keyboard under the crate's field
        // context, so Enter, Escape and the editing keys go to the field
        // and the tree's own keys are off while it is open.
        let renaming = self.renaming.is_some();
        self.rename_focus.sync(renaming, window, cx);
        // A drag that ended anywhere but in the list leaves nothing behind.
        if !cx.has_active_drag() && (self.dragging.is_some() || self.drop.is_some()) {
            self.dragging = None;
            self.drop = None;
            self.drop_claim = None;
        }
        let menu = self.render_menu(cx);
        let header = self.render_header(cx);
        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .when(renaming, |d| {
                inline_field::bind_keys(
                    d.key_context("TextInput")
                        .track_focus(&self.rename_focus.handle),
                    cx,
                )
            })
            .when(!renaming, |d| {
                d.key_context("Explorer").track_focus(&self.focus_handle)
            });
        root.on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::activate))
            .on_action(cx.listener(Self::toggle_folder))
            .on_action(cx.listener(Self::leave_tree))
            .on_action(cx.listener(Self::rename_focused))
            .on_action(cx.listener(Self::delete_focused))
            .child(header)
            // The list is wrapped so that the space below the last row
            // takes a right-click of its own: `uniform_list` hands an event
            // outside every row straight through, and a row's own handler
            // stops propagation so only one of the two menus opens.
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.open_blank_menu(event.position, cx)
                        }),
                    )
                    // Rows claim a drag over them first; anywhere else in
                    // the list is the root, and the drop lands wherever the
                    // target last was.
                    .on_drag_move(cx.listener(|this, event: &DragMoveEvent<FileDrag>, _, cx| {
                        this.drag_over_list(event, cx)
                    }))
                    .on_drop(cx.listener(|this, drag: &FileDrag, _, cx| this.dropped(drag, cx)))
                    .child(
                        uniform_list("file-explorer", count, cx.processor(Self::render_rows))
                            .track_scroll(&self.scroll)
                            .size_full()
                            .px_1(),
                    ),
            )
            .children(menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a drop lands and what the ghost says, from the 2026-11-07
    /// holds: a folder row is its own target, a file row its folder's, a
    /// root file the vault's.
    #[test]
    fn a_drop_lands_in_the_folder_under_the_pointer() {
        assert_eq!(drop_folder("Dest", true), "Dest");
        assert_eq!(drop_folder("Inbox/Item.md", false), "Inbox");
        assert_eq!(drop_folder("Other.md", false), "");
        assert_eq!(
            move_into_label("Inbox/Deep", "vault"),
            "Move into \u{201c}Deep\u{201d}"
        );
        assert_eq!(
            move_into_label("", "My vault"),
            "Move into \u{201c}My vault\u{201d}"
        );
    }

    /// The moves Obsidian made and refused on 2026-10-25: into another
    /// folder yes; into the folder it is in, and a folder into itself or
    /// below itself, nothing.
    #[test]
    fn a_drop_moves_only_somewhere_else() {
        assert!(drop_moves("Root.md", false, "Dest"));
        assert!(drop_moves("Inbox/Item.md", false, ""));
        assert!(drop_moves("Deep", true, "Dest"));
        assert!(!drop_moves("Inbox/Item.md", false, "Inbox"));
        assert!(!drop_moves("Root.md", false, ""));
        assert!(!drop_moves("Deep", true, "Deep"));
        assert!(!drop_moves("Deep", true, "Deep/Sub"));
        // A sibling whose name starts the same is not inside.
        assert!(drop_moves("Deep", true, "Deeper"));
    }

    fn files(paths: &[&str]) -> Vec<VaultFile> {
        paths
            .iter()
            .map(|p| VaultFile {
                path: p.to_string(),
                kind: FileKind::from_path(p),
                size: 0,
                mtime: 0,
                ctime: 0,
            })
            .collect()
    }

    /// The orders Obsidian showed on 2026-10-21 for a vault whose files
    /// were created in one order and modified in another (the
    /// `explorer-header` probe): folders stay in name order under every
    /// sort, and only the files of each folder move.
    #[test]
    fn the_six_sort_orders_move_files_but_not_folders() {
        let created = [
            "Alpha.md",
            "beta.md",
            "Gamma.md",
            "n2.md",
            "n10.md",
            "Projects/Plan.md",
            "Projects/Deep/Inner.md",
            "Archive/old.md",
            "Projects/Budget.md",
            "cover.png",
        ];
        let modified = [
            "n10.md",
            "Archive/old.md",
            "Alpha.md",
            "Projects/Budget.md",
            "cover.png",
            "Gamma.md",
            "Projects/Deep/Inner.md",
            "beta.md",
            "Projects/Plan.md",
            "n2.md",
        ];
        let files: Vec<VaultFile> = created
            .iter()
            .enumerate()
            .map(|(ix, p)| VaultFile {
                path: p.to_string(),
                kind: FileKind::from_path(p),
                size: 0,
                mtime: 1 + modified.iter().position(|m| m == p).unwrap() as u64,
                ctime: 1 + ix as u64,
            })
            .collect();
        let empty = vec!["Empty".to_string()];
        let mut tree = FileTree::new(&files, &empty, false);
        tree.expand_all();
        let names = |tree: &FileTree| -> Vec<String> {
            tree.rows()
                .map(|r| {
                    if r.is_folder {
                        format!("{}/", r.name)
                    } else {
                        r.name.clone()
                    }
                })
                .collect()
        };
        let observed: [(SortOrder, &[&str]); 6] = [
            (
                SortOrder::Alphabetical,
                &[
                    "Archive/",
                    "old",
                    "Empty/",
                    "Projects/",
                    "Deep/",
                    "Inner",
                    "Budget",
                    "Plan",
                    "Alpha",
                    "beta",
                    "cover",
                    "Gamma",
                    "n2",
                    "n10",
                ],
            ),
            (
                SortOrder::AlphabeticalReverse,
                &[
                    "Archive/",
                    "old",
                    "Empty/",
                    "Projects/",
                    "Deep/",
                    "Inner",
                    "Plan",
                    "Budget",
                    "n10",
                    "n2",
                    "Gamma",
                    "cover",
                    "beta",
                    "Alpha",
                ],
            ),
            (
                SortOrder::ByModifiedTime,
                &[
                    "Archive/",
                    "old",
                    "Empty/",
                    "Projects/",
                    "Deep/",
                    "Inner",
                    "Plan",
                    "Budget",
                    "n2",
                    "beta",
                    "Gamma",
                    "cover",
                    "Alpha",
                    "n10",
                ],
            ),
            (
                SortOrder::ByModifiedTimeReverse,
                &[
                    "Archive/",
                    "old",
                    "Empty/",
                    "Projects/",
                    "Deep/",
                    "Inner",
                    "Budget",
                    "Plan",
                    "n10",
                    "Alpha",
                    "cover",
                    "Gamma",
                    "beta",
                    "n2",
                ],
            ),
            (
                SortOrder::ByCreatedTime,
                &[
                    "Archive/",
                    "old",
                    "Empty/",
                    "Projects/",
                    "Deep/",
                    "Inner",
                    "Budget",
                    "Plan",
                    "cover",
                    "n10",
                    "n2",
                    "Gamma",
                    "beta",
                    "Alpha",
                ],
            ),
            (
                SortOrder::ByCreatedTimeReverse,
                &[
                    "Archive/",
                    "old",
                    "Empty/",
                    "Projects/",
                    "Deep/",
                    "Inner",
                    "Plan",
                    "Budget",
                    "Alpha",
                    "beta",
                    "Gamma",
                    "n2",
                    "n10",
                    "cover",
                ],
            ),
        ];
        for (order, rows) in observed {
            tree.set_sort(order);
            assert_eq!(names(&tree), rows, "{}", order.as_str());
            assert_eq!(SortOrder::parse(order.as_str()), Some(order));
        }
        // A collapsed folder stays collapsed across a re-sort.
        tree.toggle("Projects");
        tree.set_sort(SortOrder::Alphabetical);
        assert!(tree.is_collapsed("Projects"));
        assert!(tree.any_expanded(), "Archive is still open");
        tree.collapse_all();
        assert!(!tree.any_expanded());
    }

    #[test]
    fn folders_first_then_files_case_insensitively() {
        let files = files(&[
            "zeta.md",
            "Alpha.md",
            "b/inner.md",
            "a/x.png",
            "a/deep/y.md",
            "notes.zip",
        ]);
        let tree = FileTree::new(&files, &[], false);
        let names: Vec<(usize, &str)> = tree.rows().map(|r| (r.depth, r.name.as_str())).collect();
        assert_eq!(names, vec![(0, "a"), (0, "b"), (0, "Alpha"), (0, "zeta")]);
        let empty = vec!["Empty".to_string(), "a/void".to_string()];
        let with_empty = FileTree::new(&files, &empty, false);
        let names: Vec<&str> = with_empty.rows().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "Empty", "Alpha", "zeta"]);
        let shown = FileTree::new(&files, &[], true);
        assert_eq!(
            shown.len(),
            5,
            "notes.zip appears with unsupported files on"
        );
    }

    /// Obsidian collates names numerically, so `n2` comes before `n10`; the
    /// search pane already does and the two panes have to agree.
    #[test]
    fn names_sort_numerically_within_each_group() {
        let files = files(&[
            "n10.md",
            "n2.md",
            "n1.md",
            "n20.md",
            "n3.md",
            "f10/a.md",
            "f2/a.md",
            "Img 10.png",
            "Img 2.png",
        ]);
        let tree = FileTree::new(&files, &[], false);
        let names: Vec<&str> = tree.rows().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["f2", "f10", "Img 2", "Img 10", "n1", "n2", "n3", "n10", "n20"],
            "folders stay ahead of files, and both groups count"
        );
    }

    #[test]
    fn toggle_and_reveal_expand_ancestors() {
        let files = files(&["a/deep/y.md", "a/x.png", "b/inner.md"]);
        let mut tree = FileTree::new(&files, &[], false);
        assert_eq!(tree.len(), 2);
        tree.toggle("a");
        let names: Vec<&str> = tree.rows().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["a", "deep", "x", "b"]);
        tree.toggle("a");
        assert_eq!(tree.reveal("a/deep/y.md"), Some(2));
        let names: Vec<&str> = tree.rows().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["a", "deep", "y", "x", "b"]);
        assert!(tree.rows().nth(2).unwrap().path == "a/deep/y.md");
        tree.collapse_all();
        assert_eq!(tree.len(), 2);
        tree.expand_all();
        assert_eq!(tree.len(), 6);
        let expanded = tree.expanded_folders();
        let mut rebuilt = FileTree::new(&files, &[], false);
        rebuilt.expand_folders(&expanded);
        assert_eq!(rebuilt.len(), 6);
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;

    /// The path an event acts on. The match is exhaustive on purpose: a new
    /// [`ExplorerEvent`] will not compile until it says which row it carries,
    /// which is what keeps a menu from ever acting on the wrong one.
    fn payload(event: &ExplorerEvent) -> Option<&str> {
        match event {
            ExplorerEvent::Open(p)
            | ExplorerEvent::OpenInNewTab(p)
            | ExplorerEvent::Rename(p)
            | ExplorerEvent::Move(p)
            | ExplorerEvent::Duplicate(p)
            | ExplorerEvent::Delete(p)
            | ExplorerEvent::Bookmark(p)
            | ExplorerEvent::MergeWith(p)
            | ExplorerEvent::OpenToRight(p)
            | ExplorerEvent::OpenInNewWindow(p)
            | ExplorerEvent::CopyPath(p)
            | ExplorerEvent::OpenInDefaultApp(p)
            | ExplorerEvent::ShowInSystemExplorer(p)
            | ExplorerEvent::SearchInFolder(p) => Some(p),
            ExplorerEvent::NewNote { folder }
            | ExplorerEvent::NewCanvas { folder }
            | ExplorerEvent::NewBase { folder } => Some(folder),
            ExplorerEvent::NewFolder { parent } => Some(parent),
            ExplorerEvent::RenameTo { path, .. } | ExplorerEvent::MoveInto { path, .. } => {
                Some(path)
            }
            ExplorerEvent::NewNoteFromHeader
            | ExplorerEvent::LeaveFocus
            | ExplorerEvent::Hover { .. } => None,
        }
    }

    /// Every row of the explorer's three menus has an event, and the event
    /// carries the folder or file the menu was opened on -- the vault root
    /// for the empty space -- so a menu can never act on the wrong row.
    #[test]
    fn every_item_carries_the_row_it_was_opened_on() {
        for (target, want) in [
            (Target::Folder("Notes/Deep".into()), "Notes/Deep"),
            (
                Target::File("Notes/Deep/Plan.md".into()),
                "Notes/Deep/Plan.md",
            ),
            (Target::Blank, ""),
        ] {
            for item in file_menu::items(&target) {
                let event = event_for(item.pick, &target)
                    .unwrap_or_else(|| panic!("{} has no event on {target:?}", item.label));
                assert_eq!(
                    payload(&event),
                    Some(want),
                    "{} acts on the wrong row",
                    item.label
                );
            }
        }
    }

    /// The one pick the explorer never offers is the one it has no event
    /// for: "Reveal file in navigation" reveals *in* the explorer.
    #[test]
    fn reveal_in_navigation_is_not_an_explorer_event() {
        assert!(event_for(Pick::RevealInNavigation, &Target::File("Plan.md".into())).is_none());
    }
}
