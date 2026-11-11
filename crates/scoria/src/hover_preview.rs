//! Page preview: the popover a resting pointer raises over an internal
//! link, to [[Specification#Observed page preview, Obsidian 1.15.0]].
//!
//! The pieces that do not need the workspace live here: which sources need
//! Mod, the delays and the box's measurements, what the popover shows for a
//! link, and how it is drawn. The workspace owns the state (a
//! [`HoverPreview`]) and the timers, because a timer has to come back to an
//! entity to act.

use crate::reading::model::{each_embed, EmbedKind, RBlock};
use crate::reading::view::{LinkHandler, ReadingHost, Renderer, ToggleHandler, LINE_HEIGHT};
use crate::reading::{build_with, BuildOptions, EmbedSource};
use crate::theme::Theme;
use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, App, Bounds, Modifiers, Pixels,
    ScrollHandle, SharedString, Window,
};
use scoria_formats::config::{PagePreviewConfig, PropertyTypes};
use scoria_formats::properties::PropertiesInDocument;
use scoria_formats::FileKind;
use std::cell::RefCell;
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
}

impl Source {
    /// Whether a hover from here needs Mod held: the key when it is set,
    /// else the source's `defaultMod` as `hoverLinkSources` records it.
    pub fn requires_mod(self, config: &PagePreviewConfig) -> bool {
        match self {
            Source::Reading => config.preview.unwrap_or(false),
            Source::Editor => config.editor.unwrap_or(true),
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
    /// Callouts folded open or shut inside it, by title id.
    pub toggled: Rc<RefCell<HashSet<usize>>>,
    pub scroll: ScrollHandle,
}

/// The workspace's page-preview state.
#[derive(Default)]
pub struct HoverPreview {
    pub pending: Option<Pending>,
    pub shown: Option<Shown>,
    /// The pointer is over the popover, which keeps it up.
    pub over_popover: bool,
    /// Bumped by every change; a timer acts only if it is unchanged.
    pub generation: u64,
    /// The modifiers last seen, from a move or a modifiers-changed event.
    pub modifiers: Modifiers,
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

/// The popover's element, drawn over the window it is in.
#[allow(clippy::too_many_arguments)]
pub fn render(
    shown: &Shown,
    theme: &Theme,
    property_types: &PropertyTypes,
    properties: PropertiesInDocument,
    on_link: LinkHandler,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
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
            // Read-only: no task toggles, no pill removal, no menu, and no
            // hover of its own until nested popovers land.
            let host = ReadingHost {
                properties,
                copied: None,
                on_copy: Rc::new(|_, _| {}),
                on_task: Rc::new(|_, _| {}),
                on_remove_item: None,
                on_menu: None,
                hover: None,
            };
            let renderer = Renderer::new(
                theme,
                window.text_system().clone(),
                WIDTH - 2. * PADDING,
                &toggled,
                property_types,
                None,
                None,
                on_link,
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
    deferred(
        anchored().position(position).offset(offset).child(
            div()
                .id("hover-popover")
                .occlude()
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
                .on_hover(on_hover)
                .child(
                    div()
                        .id("hover-popover-scroll")
                        .max_h(px(HEIGHT))
                        .overflow_y_scroll()
                        .track_scroll(&shown.scroll)
                        .child(
                            div()
                                .w_full()
                                .p(px(PADDING))
                                .flex()
                                .flex_col()
                                .children(body),
                        ),
                ),
        ),
    )
    .with_priority(2)
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
