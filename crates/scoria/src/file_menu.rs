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
//! "Show in system explorer" is also here, as the command lines that select
//! a file in the desktop's file manager, so the one platform table has a test
//! beside it.

use crate::theme::Theme;
use gpui::{anchored, deferred, div, prelude::*, px, AnyElement, Context, Pixels, Point, Window};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Obsidian's `data-section` on a menu item. Items are grouped by it in the
/// order below, and a separator is drawn wherever the section changes — which
/// is how the captured menus get their separators without any being written
/// down one at a time ([[Specification#The file explorer's context menus]]).
/// [`Section::Danger`] takes **two**: Obsidian emits an empty section before
/// it, and both explorer menus were captured with the two consecutive
/// separators that leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// The folder row's and the empty space's creation items.
    ActionPrimary,
    /// The file row's "Open in..." items.
    Open,
    Action,
    /// Obsidian leaves "Copy path" with no section at all.
    Unsectioned,
    System,
    Danger,
}

/// What a clicked row asks for. The path it acts on is the menu's
/// [`Target`], so a menu can never act on the wrong row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    OpenInNewTab,
    OpenToRight,
    /// "Make a copy".
    Duplicate,
    Move,
    Bookmark,
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
}

/// One row of a context menu: what Obsidian draws, and what a click reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    pub label: &'static str,
    pub icon: &'static str,
    pub section: Section,
    pub pick: Pick,
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
}

impl Target {
    /// The path every item of the menu acts on; the vault root for
    /// [`Target::Blank`].
    pub fn path(&self) -> &str {
        match self {
            Target::File(p) | Target::Folder(p) | Target::Linked(p) => p,
            Target::Blank => "",
        }
    }
}

/// The menu for a target, in Obsidian's order.
///
/// Three of the captured items are left out rather than drawn dead, because
/// Scoria has no command behind them: **Open in new window** (no second
/// window), **Merge entire file with...** (no merge) and **Open version
/// history** (no file history). Listing a row that cannot run is what
/// the command palette refuses to do, and the menu refuses it too; the gaps
/// are entries in `docs/scoria/UI parity backlog.md`. Dropping the only item
/// of a section drops its separator with it, so the `view` section's
/// separator is absent here where the capture has it.
///
/// Obsidian's file row has no creation items at all — Scoria's menu used to
/// offer "New note" and "New folder" on a file, which is the one thing here
/// that was wrong rather than missing.
pub fn items(target: &Target) -> Vec<Item> {
    use Pick as P;
    use Section as S;
    let item = |label, icon, section, pick| Item {
        label,
        icon,
        section,
        pick,
    };
    match target {
        Target::File(_) => vec![
            item("Open in new tab", "file-plus", S::Open, P::OpenInNewTab),
            item(
                "Open to the right",
                "separator-vertical",
                S::Open,
                P::OpenToRight,
            ),
            item("Make a copy", "files", S::Action, P::Duplicate),
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
            item("Rename...", "pen-line", S::Danger, P::Rename),
            item("Delete", "trash-2", S::Danger, P::Delete),
        ],
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
    }
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
    };
    json!({
        "target": target,
        "path": menu.target.path(),
        "items": drawn(&menu.target),
    })
}

/// The menu drawn at its position over everything else: a press outside it
/// runs `on_close`; a click on a row runs `on_close` and then `on_pick` with
/// the row's [`Pick`]. The view owns the [`Menu`] and clears it in
/// `on_close`; this only draws.
pub fn render<V: 'static>(
    menu: &Menu,
    theme: &Theme,
    cx: &mut Context<V>,
    on_close: impl Fn(&mut V, &mut Context<V>) + 'static,
    on_pick: impl Fn(&mut V, Pick, &mut Window, &mut Context<V>) + 'static,
) -> AnyElement {
    let on_close = Rc::new(on_close);
    let on_pick = Rc::new(on_pick);
    let mut list = div()
        .id("file-menu")
        .flex()
        .flex_col()
        .min_w(px(180.))
        .py_1()
        .bg(theme.bg_secondary)
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .shadow_lg()
        .text_size(px(13.))
        .text_color(theme.text)
        .occlude()
        .on_mouse_down_out(cx.listener({
            let on_close = on_close.clone();
            move |this, _, _, cx| on_close(this, cx)
        }));
    let mut previous: Option<Section> = None;
    for (ix, item) in items(&menu.target).into_iter().enumerate() {
        for n in 0..separators_above(previous, item.section) {
            list = list.child(
                div()
                    .id(("file-menu-separator", ix * 2 + n))
                    .my_1()
                    .h(px(1.))
                    .w_full()
                    .bg(theme.border),
            );
        }
        previous = Some(item.section);
        let pick = item.pick;
        let on_close = on_close.clone();
        let on_pick = on_pick.clone();
        list = list.child(
            div()
                .id(("file-menu-item", ix))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|s| s.bg(theme.bg_alt))
                .on_click(cx.listener(move |this, _, window, cx| {
                    on_close(this, cx);
                    on_pick(this, pick, window, cx);
                }))
                .child(crate::assets::icon(item.icon, 16., theme.text_muted))
                .child(item.label),
        );
    }
    deferred(
        anchored()
            .position(menu.position)
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    /// [[Specification#The file explorer's context menus]], the file-row
    /// table, minus the three items Scoria has no command for -- Open in new
    /// window, Merge entire file with... and Open version history. Dropping
    /// Open version history empties the `view` section, so the separator the
    /// capture has around it is gone too; every other separator is where
    /// Obsidian put it, including the pair above Rename.
    #[test]
    fn the_file_rows_menu_is_obsidians_order_without_the_rows_it_cannot_run() {
        assert_eq!(
            drawn(&Target::File("Notes/Plan.md".into())),
            [
                "Open in new tab",
                "Open to the right",
                "---",
                "Make a copy",
                "Move file to...",
                "Bookmark...",
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
    /// minus "Open in new window": Rename sits with Move and Bookmark under
    /// one separator rather than in a `danger` pair, there is no Make a copy
    /// and no Delete, and "Reveal file in navigation" closes the list.
    #[test]
    fn a_linked_rows_menu_is_the_probes_list_without_open_in_new_window() {
        assert_eq!(
            drawn(&Target::Linked("Notes/Plan.md".into())),
            [
                "Open in new tab",
                "Open to the right",
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
            target: Target::Linked("Notes/Plan.md".into()),
        };
        let value = describe(&menu);
        assert_eq!(value["target"], "linked");
        assert_eq!(value["path"], "Notes/Plan.md");
        assert_eq!(value["items"][0], "Open in new tab");
        assert_eq!(value["items"][2], "---");
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
}
