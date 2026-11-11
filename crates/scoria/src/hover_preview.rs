//! Page preview: the popover a resting pointer raises over an internal
//! link, to [[Specification#Observed page preview, Obsidian 1.15.0]].
//!
//! The pieces that do not need the workspace live here: which sources need
//! Mod, the delays and the box's measurements, what the popover shows for a
//! link, and how it is drawn. The workspace owns the state (a
//! [`HoverPreview`]) and the timers, because a timer has to come back to an
//! entity to act.

use crate::reading::model::{each_embed, EmbedKind, RBlock};
use crate::reading::view::{
    HoverHandler, HoverHost, LinkHandler, ReadingHost, Renderer, ToggleHandler, LINE_HEIGHT,
};
use crate::reading::{build_with, BuildOptions, EmbedSource};
use crate::theme::Theme;
use gpui::{
    anchored, canvas, deferred, div, point, prelude::*, px, AnyElement, App, Bounds, Div,
    Modifiers, Pixels, Point, ScrollHandle, SharedString, Stateful, Window,
};
use scoria_formats::config::{PagePreviewConfig, PropertyTypes};
use scoria_formats::properties::PropertiesInDocument;
use scoria_formats::FileKind;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

/// From the pointer resting on a link (or Mod going down while it rests
/// there) to the popover: 269 to 335 ms over twenty hovers.
pub const SHOW_DELAY: Duration = Duration::from_millis(300);
/// From the pointer leaving both the link and the popover to its removal.
pub const HIDE_DELAY: Duration = Duration::from_millis(300);
/// `--popover-width` and `--popover-height`: the box inside its border.
pub const WIDTH: f32 = 450.;
pub const HEIGHT: f32 = 400.;
/// Between the hovered element and the popover, below it or above it.
pub const GAP: f32 = 10.;
/// How far inside the window's right edge a pushed-in popover stops.
pub const EDGE: f32 = 10.;
/// The reading view's padding inside the popover.
pub const PADDING: f32 = 32.;

/// Where the hovered link is: each is a key of `page-preview.json`, whose
/// value says whether Mod is required there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `preview`, "Reading view": the one source that needs no Mod by default.
    Reading,
    /// `editor`, "Editing view": Live Preview and source mode alike.
    Editor,
    /// `file-explorer`, "Files": a file's row.
    Files,
    /// `tab-header`, "Tab header": a tab showing a file.
    TabHeader,
    /// `bookmarks`, "Bookmarks": a file, heading or block bookmark.
    Bookmarks,
    /// `search`, "Search, Backlinks, and Outgoing links": a Backlinks file
    /// row or an Outgoing links row. The Search pane's own rows raised
    /// nothing in the probe, Mod or not, so they do not report.
    Search,
    /// `outline`, "Outline": a heading's row, which previews its section.
    Outline,
}

impl Source {
    /// Whether a hover from here needs Mod held: the key when it is set,
    /// else the source's `defaultMod` as `hoverLinkSources` records it.
    pub fn requires_mod(self, config: &PagePreviewConfig) -> bool {
        match self {
            Source::Reading => config.preview.unwrap_or(false),
            Source::Editor => config.editor.unwrap_or(true),
            Source::Files => config.file_explorer.unwrap_or(true),
            Source::TabHeader => config.tab_header.unwrap_or(true),
            Source::Bookmarks => config.bookmarks.unwrap_or(true),
            Source::Search => config.search.unwrap_or(true),
            Source::Outline => config.outline.unwrap_or(true),
        }
    }
}

/// Mod: Cmd on macOS (Ctrl raised nothing there), Ctrl elsewhere.
pub fn mod_held(modifiers: &Modifiers) -> bool {
    modifiers.secondary()
}

/// The popover's left edge for an element whose left edge is at
/// `anchor_left`: the same x, pushed left until the box's right edge is
/// [`EDGE`] inside the window, and never off its left side.
pub fn left_edge(anchor_left: f32, viewport_width: f32) -> f32 {
    anchor_left
        .min(viewport_width - EDGE - (WIDTH + 2.))
        .max(0.)
}

/// A vault path (with any `#subpath`) as a link body that resolves to
/// exactly that file: rooted, so a root-level note is not looked up by its
/// basename, which another folder's note of the same name could win.
pub fn exact(path: &str) -> String {
    format!("/{path}")
}

/// A row (a pane's row, a tab) that previews: reports the pointer coming
/// onto it (`true`) and leaving it, with the row's bounds in window
/// coordinates — the anchor, since a row anchors exactly as a link does
/// (the probe's Files, Bookmarks, Backlinks and Outline rows and its tab
/// header all put the popover 10 px under their bottom edge) — and the
/// modifiers held then.
pub fn hover_row(
    el: Stateful<Div>,
    report: impl Fn(bool, Bounds<Pixels>, Modifiers, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    hover_row_bordered(el, 0., report)
}

/// [`hover_row`] for a row with a border of its own (`border` px wide): an
/// absolute child is placed against the padding box, so the measuring
/// child reaches back out over the border to take the row's full box.
pub fn hover_row_bordered(
    el: Stateful<Div>,
    border: f32,
    report: impl Fn(bool, Bounds<Pixels>, Modifiers, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let set = bounds.clone();
    el.relative()
        .child(
            canvas(move |b, _, _| set.set(b), |_, _, _, _| {})
                .absolute()
                .top(px(-border))
                .left(px(-border))
                .right(px(-border))
                .bottom(px(-border)),
        )
        .on_hover(move |over, window, cx| {
            report(*over, bounds.get(), window.modifiers(), window, cx)
        })
}

/// The link under the pointer, whether or not a popover is up for it.
#[derive(Clone, Debug)]
pub struct Pending {
    /// The window the link is in: `None` for the vault window.
    pub window: Option<u64>,
    pub source: Source,
    /// The link body as written.
    pub target: String,
    /// The note it is written in, which it resolves against.
    pub from: String,
    /// The hovered element's first line box, in window coordinates.
    pub anchor: Bounds<Pixels>,
    /// How many popovers the link is inside: 0 for a view or a pane, `n`
    /// for a link in the `n`th popover of the stack, whose own popover
    /// goes up over it as the `n + 1`th.
    pub depth: usize,
}

/// What a popover shows.
pub enum Content {
    /// Rendered blocks: a whole note, a section, a block, or the embed of
    /// any other file. A note's inline title is not among them: Obsidian's
    /// popover holds `.inline-title` but does not show it (its text ran
    /// "Properties, Beta, Beta first paragraph", one "Beta", the heading).
    Blocks { blocks: Vec<RBlock> },
    /// A link to nothing: the line Obsidian's `.file-embed.mod-empty` shows.
    Missing(String),
}

/// A popover that is up.
pub struct Shown {
    pub window: Option<u64>,
    pub anchor: Bounds<Pixels>,
    /// The path its own links resolve against.
    pub from: String,
    pub content: Content,
    /// The note a click turns it into an editor of, and the subpath it
    /// shows: `None` for anything that is not a note (an image, a base, a
    /// missing note), which a click leaves alone.
    pub note: Option<(String, String)>,
    /// Callouts folded open or shut inside it, by title id.
    pub toggled: Rc<RefCell<HashSet<usize>>>,
    pub scroll: ScrollHandle,
    /// Which of its rendered links the pointer is on, as a view's
    /// [`crate::reading::view::HoverHost`] keeps it.
    pub hover_current: Rc<Cell<Option<(usize, usize)>>>,
    /// The pointer is over it, which keeps it (and those under it) up.
    pub over: bool,
    /// Its height inside the border as last painted: the editor a click
    /// makes takes the same box.
    pub height: Rc<Cell<f32>>,
    /// The Live Preview editor a click on its text made (`is-editing`):
    /// it stays when the pointer leaves, until Escape or a click outside.
    pub editing: Option<gpui::Entity<crate::editor::Editor>>,
    /// The workspace's subscription to that editor's events.
    pub editing_subscription: Option<gpui::Subscription>,
}

impl Shown {
    pub fn new(
        window: Option<u64>,
        anchor: Bounds<Pixels>,
        from: String,
        content: Content,
        note: Option<(String, String)>,
        toggled: HashSet<usize>,
    ) -> Shown {
        Shown {
            window,
            anchor,
            from,
            content,
            note,
            toggled: Rc::new(RefCell::new(toggled)),
            scroll: ScrollHandle::default(),
            hover_current: Rc::default(),
            over: false,
            height: Rc::new(Cell::new(0.)),
            editing: None,
            editing_subscription: None,
        }
    }
}

/// The workspace's page-preview state.
#[derive(Default)]
pub struct HoverPreview {
    pub pending: Option<Pending>,
    /// The popovers up, the first raised from a view or a pane and each
    /// later one from a link in the one before it.
    pub shown: Vec<Shown>,
    /// Bumped by every change; a timer acts only if it is unchanged.
    pub generation: u64,
    /// The modifiers last seen, from a move or a modifiers-changed event.
    pub modifiers: Modifiers,
}

impl HoverPreview {
    /// How many popovers stay when the hide timer fires: every one up to
    /// the last that the pointer is over or that is editing, and all of
    /// them while the pointer is on a link (its own popover replaces them
    /// from where it sits when it goes up). Leaving a nested popover and
    /// its parent removes both (observed); one left for its parent keeps
    /// the parent.
    pub fn kept(&self) -> usize {
        if self.pending.is_some() {
            return self.shown.len();
        }
        self.shown
            .iter()
            .rposition(|s| s.over || s.editing.is_some())
            .map_or(0, |ix| ix + 1)
    }
}

/// The note (and the subpath) a popover for `target` would edit, if it
/// shows a note.
pub fn editable_note(
    vault: &dyn EmbedSource,
    target: &str,
    from: &str,
) -> Option<(String, String)> {
    let path = vault.resolve(target, from)?;
    if !FileKind::from_path(&path).is_markdown() {
        return None;
    }
    let subpath = target.find('#').map(|i| &target[i..]).unwrap_or("");
    Some((path, subpath.to_string()))
}

/// One of a popover's handlers, told `T`.
pub type PopoverHandler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// What a popover's element does, handed in by the workspace.
pub struct Handlers {
    /// A link clicked inside it.
    pub on_link: LinkHandler,
    /// The pointer coming onto a link inside it or leaving one: a nested
    /// popover's source.
    pub on_hover_link: HoverHandler,
    /// The pointer coming onto the popover or leaving it.
    pub on_hover: PopoverHandler<bool>,
    /// A click on its text (not on a link), at this window point.
    pub on_edit: PopoverHandler<Point<Pixels>>,
    /// Escape (`true`), or a press outside it (`false`), while it is an
    /// editor.
    pub on_close: PopoverHandler<bool>,
}

/// What hovering `target`, written in `from`, shows.
///
/// A note's whole text is built as the reading view builds it. A link with a subpath, or to anything
/// that is not a note, is built as the embed `![[path#subpath]]` would be,
/// which is what selects a section or a block (Obsidian shows that part
/// alone) and draws an image, a base or a PDF the way the reading view's
/// embed does; a section that is not there falls back to the whole note.
pub fn content(
    vault: &dyn EmbedSource,
    options: BuildOptions,
    target: &str,
    from: &str,
) -> (Content, String) {
    let Some(path) = vault.resolve(target, from) else {
        let name = scoria_markdown::parse_linktext(target).path.to_string();
        let name = name.rsplit('/').next().unwrap_or(&name).to_string();
        return (
            Content::Missing(format!(
                "\u{201c}{name}\u{201d} is not created yet. Click to create."
            )),
            from.to_string(),
        );
    };
    let markdown = FileKind::from_path(&path).is_markdown();
    let subpath = target.find('#').map(|i| &target[i..]).unwrap_or("");
    let whole = |vault: &dyn EmbedSource| {
        let text = vault.read(&path).unwrap_or_default();
        let model = build_with(&path, &text, vault, options);
        Content::Blocks {
            blocks: model.blocks,
        }
    };
    if markdown && subpath.is_empty() {
        return (whole(vault), path.clone());
    }
    let synthetic = format!("![[{path}{subpath}]]");
    let model = build_with(&path, &synthetic, vault, options);
    let mut found: Option<Content> = None;
    let mut missing = false;
    each_embed(&model.blocks, &mut |embed| {
        if found.is_some() || missing {
            return;
        }
        match &embed.kind {
            EmbedKind::Note { blocks, .. } => {
                found = Some(Content::Blocks {
                    blocks: blocks.clone(),
                })
            }
            EmbedKind::Missing => missing = true,
            _ => {}
        }
    });
    let content = match found {
        Some(content) => content,
        None if markdown => whole(vault),
        None => Content::Blocks {
            blocks: model.blocks,
        },
    };
    (content, path.clone())
}

/// The popover's element, drawn over the window it is in. `depth` is its
/// place in the stack: a nested popover is drawn over its parent.
pub fn render(
    shown: &Shown,
    depth: usize,
    theme: &Theme,
    property_types: &PropertyTypes,
    properties: PropertiesInDocument,
    handlers: Handlers,
    window: &mut Window,
) -> AnyElement {
    let t = &theme.tokens;
    let base = theme.base_font_size;
    let body: Vec<AnyElement> = match &shown.content {
        Content::Missing(line) => vec![div()
            .w_full()
            .text_center()
            .text_color(t.text_muted)
            .child(SharedString::from(line.clone()))
            .into_any_element()],
        Content::Blocks { blocks } => {
            let toggled = shown.toggled.borrow();
            let on_toggle: ToggleHandler = Rc::new({
                let toggled = shown.toggled.clone();
                move |id, cx| {
                    let mut toggled = toggled.borrow_mut();
                    if !toggled.remove(&id) {
                        toggled.insert(id);
                    }
                    cx.refresh_windows();
                }
            });
            // Read-only: no task toggles, no pill removal and no menu. Its
            // links raise popovers of their own, on the reading view's
            // rules (observed: no Mod).
            let host = ReadingHost {
                properties,
                copied: None,
                on_copy: Rc::new(|_, _| {}),
                on_task: Rc::new(|_, _| {}),
                on_remove_item: None,
                on_menu: None,
                hover: Some(HoverHost {
                    on_hover: handlers.on_hover_link.clone(),
                    current: shown.hover_current.clone(),
                }),
            };
            let renderer = Renderer::new(
                theme,
                window.text_system().clone(),
                WIDTH - 2. * PADDING,
                &toggled,
                property_types,
                None,
                None,
                handlers.on_link.clone(),
                on_toggle,
            )
            .with_reading_host(&host);
            blocks.iter().map(|b| renderer.block(b, false)).collect()
        }
    };
    let viewport = window.viewport_size();
    let x = left_edge(f32::from(shown.anchor.left()), f32::from(viewport.width));
    // Below the element, 10 px down; where that would leave the window,
    // gpui's anchor switch puts the box's bottom at `position`, which is
    // 10 px over the element's top. The x is settled here, so the switch
    // never flips it sideways.
    let position = point(px(x), shown.anchor.top() - px(GAP));
    let offset = point(px(0.), shown.anchor.size.height + px(2. * GAP));
    let frame = crate::theme::box_shadows(&t.shadow_s);
    // The render's height inside the border, measured while it is a render
    // and frozen once it is an editor, which takes that box.
    let height = shown.height.clone();
    let measure = shown.editing.is_none().then(|| {
        canvas(
            move |b, _, _| height.set(f32::from(b.size.height)),
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    });
    let inner: AnyElement = match &shown.editing {
        // The editor takes the box the render had, inside the border.
        Some(editor) => div()
            .w_full()
            .h(px(shown.height.get().clamp(60., HEIGHT)))
            .child(editor.clone())
            .into_any_element(),
        None => {
            let on_edit = handlers.on_edit.clone();
            let over_link = shown.hover_current.clone();
            let editable = shown.note.is_some();
            div()
                .id("hover-popover-scroll")
                .max_h(px(HEIGHT))
                .overflow_y_scroll()
                .track_scroll(&shown.scroll)
                // A click on the text, not on one of its links, makes the
                // popover an editor with the caret where the click was.
                .when(editable, |d| {
                    d.on_click(move |event, window, cx| {
                        if over_link.get().is_none() {
                            on_edit(event.position(), window, cx)
                        }
                    })
                })
                .child(
                    div()
                        .w_full()
                        .p(px(PADDING))
                        .flex()
                        .flex_col()
                        .children(body),
                )
                .into_any_element()
        }
    };
    let editing = shown.editing.is_some();
    let on_hover = handlers.on_hover.clone();
    let (close_key, close_out) = (handlers.on_close.clone(), handlers.on_close);
    deferred(
        anchored().position(position).offset(offset).child(
            div()
                .id(("hover-popover", depth))
                .occlude()
                .relative()
                .w(px(WIDTH + 2.))
                .max_h(px(HEIGHT + 2.))
                .flex()
                .flex_col()
                .bg(t.background_primary)
                .border_1()
                .border_color(t.background_modifier_border)
                .rounded(px(8.))
                .shadow(frame)
                .overflow_hidden()
                .font_family(theme.text_font.clone())
                .text_size(px(base))
                .line_height(px(base * LINE_HEIGHT))
                .text_color(t.text_normal)
                .on_hover(move |over, window, cx| on_hover(*over, window, cx))
                // An editing popover goes on Escape and on a press outside
                // it, and on nothing else.
                .when(editing, |d| {
                    d.capture_key_down(move |event, window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                            close_key(true, window, cx);
                        }
                    })
                    .on_mouse_down_out(move |_, window, cx| close_out(false, window, cx))
                })
                .children(measure)
                .child(inner),
        ),
    )
    .with_priority(2 + depth)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn only_the_reading_view_previews_without_mod_by_default() {
        let config = PagePreviewConfig::default();
        assert!(!Source::Reading.requires_mod(&config));
        assert!(Source::Editor.requires_mod(&config));
        // A key set to true means "require Mod" (observed with `preview: true`
        // and `editor: false`).
        let flipped = PagePreviewConfig {
            preview: Some(true),
            editor: Some(false),
            ..PagePreviewConfig::default()
        };
        assert!(Source::Reading.requires_mod(&flipped));
        assert!(!Source::Editor.requires_mod(&flipped));
        // Every pane and the tab header needed Mod in the probe (`plain: 0`,
        // `mod: 1` for each), each under its own key.
        for source in [
            Source::Files,
            Source::TabHeader,
            Source::Bookmarks,
            Source::Search,
            Source::Outline,
        ] {
            assert!(source.requires_mod(&config), "{source:?}");
        }
        let off = PagePreviewConfig {
            file_explorer: Some(false),
            tab_header: Some(false),
            bookmarks: Some(false),
            search: Some(false),
            outline: Some(false),
            ..PagePreviewConfig::default()
        };
        for source in [
            Source::Files,
            Source::TabHeader,
            Source::Bookmarks,
            Source::Search,
            Source::Outline,
        ] {
            assert!(!source.requires_mod(&off), "{source:?}");
        }
    }

    #[test]
    fn the_box_keeps_the_anchor_x_until_the_right_edge_pushes_it_in() {
        // The probe's window was 1024 wide: a link at 415 kept its x, and a
        // link at 946 (and the right dock's rows at 737) gave 562.
        assert_eq!(left_edge(415., 1024.), 415.);
        assert_eq!(left_edge(946., 1024.), 562.);
        assert_eq!(left_edge(737., 1024.), 562.);
        assert_eq!(left_edge(5., 300.), 0.);
    }

    #[test]
    fn leaving_a_nested_popover_for_its_parent_keeps_the_parent() {
        let popover = |over: bool| {
            let mut s = Shown::new(
                None,
                Bounds::default(),
                "Beta.md".into(),
                Content::Missing(String::new()),
                None,
                HashSet::new(),
            );
            s.over = over;
            s
        };
        let mut hp = HoverPreview {
            shown: vec![popover(true), popover(false)],
            ..HoverPreview::default()
        };
        // The pointer went back to the parent: the nested one goes.
        assert_eq!(hp.kept(), 1);
        // It left both: both go (observed, ~300 ms).
        hp.shown[0].over = false;
        assert_eq!(hp.kept(), 0);
        // Over the nested one, the parent stays under it.
        hp.shown[1].over = true;
        assert_eq!(hp.kept(), 2);
        // On a link anywhere: nothing goes until that link's popover is up.
        hp.shown[1].over = false;
        hp.pending = Some(Pending {
            window: None,
            source: Source::Reading,
            target: "Gamma".into(),
            from: "Beta.md".into(),
            anchor: Bounds::default(),
            depth: 1,
        });
        assert_eq!(hp.kept(), 2);
    }

    #[test]
    fn only_a_note_is_editable_and_it_keeps_its_subpath() {
        assert_eq!(
            editable_note(&Vault, "Beta#Second", "Alpha.md"),
            Some(("Beta.md".into(), "#Second".into()))
        );
        assert_eq!(
            editable_note(&Vault, "/Beta.md", "Beta.md"),
            None,
            "the test vault does not root paths"
        );
        assert_eq!(editable_note(&Vault, "Pic.png", "Alpha.md"), None);
        assert_eq!(editable_note(&Vault, "Missing", "Alpha.md"), None);
    }

    struct Vault;
    impl EmbedSource for Vault {
        fn resolve(&self, linktext: &str, _: &str) -> Option<String> {
            let name = linktext.split('#').next().unwrap_or(linktext);
            match name {
                "Beta" | "Beta.md" => Some("Beta.md".into()),
                "Pic.png" => Some("Pic.png".into()),
                _ => None,
            }
        }
        fn read(&self, path: &str) -> Option<String> {
            (path == "Beta.md").then(|| {
                "---\nstatus: draft\n---\n# Beta\n\nFirst.\n\n## Second\n\nUnder second.\n\nA block. ^blk\n\n## Third\n\nLast.\n".to_string()
            })
        }
        fn absolute(&self, path: &str) -> PathBuf {
            PathBuf::from("/vault").join(path)
        }
    }

    fn texts(content: &Content) -> String {
        let Content::Blocks { blocks, .. } = content else {
            return String::new();
        };
        format!("{blocks:?}")
    }

    #[test]
    fn a_whole_note_shows_all_of_it_and_a_section_only_itself() {
        let opts = BuildOptions::default();
        let (whole, from) = content(&Vault, opts, "Beta", "Alpha.md");
        assert_eq!(from, "Beta.md");
        assert!(texts(&whole).contains("First."));
        assert!(texts(&whole).contains("Last."));

        let (section, _) = content(&Vault, opts, "Beta#Second", "Alpha.md");
        let s = texts(&section);
        assert!(s.contains("Under second."), "{s}");
        assert!(!s.contains("First."), "{s}");
        assert!(!s.contains("Last."), "{s}");

        let (block, _) = content(&Vault, opts, "Beta#^blk", "Alpha.md");
        let b = texts(&block);
        assert!(b.contains("A block."), "{b}");
        assert!(!b.contains("Under second."), "{b}");

        // A heading that is not there shows the whole note.
        let (fallback, _) = content(&Vault, opts, "Beta#Nowhere", "Alpha.md");
        assert!(texts(&fallback).contains("First."));
    }

    #[test]
    fn a_missing_note_says_so_and_an_image_is_its_embed() {
        let opts = BuildOptions::default();
        let (missing, _) = content(&Vault, opts, "Folder/Missing", "Alpha.md");
        let Content::Missing(line) = missing else {
            panic!()
        };
        assert_eq!(
            line,
            "\u{201c}Missing\u{201d} is not created yet. Click to create."
        );
        let (image, _) = content(&Vault, opts, "Pic.png", "Alpha.md");
        assert!(texts(&image).contains("Image("), "{}", texts(&image));
    }
}
