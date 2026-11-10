//! What the binary carries with it: the icons the shell draws, the logo it
//! signs the window with, and the interface font it falls back to.
//!
//! The icons are Lucide's (ISC, `assets/icons/LICENSE`), fetched from the
//! Lucide repository rather than from any Obsidian bundle; Obsidian names
//! the Lucide icon each control uses in its DOM (`svg.lucide-search`), and
//! that name is what [`icon`] takes. Two of the names Obsidian uses are its
//! own drawings rather than Lucide's — `links-coming-in` and
//! `links-going-out` on the right dock's Backlinks and Outgoing links tabs —
//! so Scoria draws its own under the same names: Lucide's chain at the top
//! left with a horizontal arrow beneath it, pointing in or out.
//! `bookmark-filled` is Lucide's `bookmark` with `fill="currentColor"`: the
//! view header's "Bookmarked" action is that icon filled and stroked in the
//! accent (2026-11-07), and gpui paints an SVG's fill only where it has one. The font is Inter (SIL OFL,
//! `assets/fonts/LICENSE-Inter.txt`), the family at the head of Obsidian's
//! own interface and text font stacks, in the static faces the pinned gpui
//! can pick weights from (a variable font renders every weight as regular).
//! Beside it rides one face of Rubik (SIL OFL, `assets/fonts/LICENSE-Rubik.txt`),
//! the Black weight as a static instance of the variable file, chosen for the
//! product name under the mark: as heavy as the Gibson Anya set the wordmark
//! in, with rounder corners and a blockier set. It signs that name and nothing else.
//! The logo is Anya's; see `docs/img/marks/` for the master.

use anyhow::Result;
use gpui::{img, px, svg, AssetSource, Hsla, Img, SharedString, Styled, Svg};
use std::borrow::Cow;

/// The asset source handed to gpui: every path `svg().path(..)` and
/// `img(..)` can name.
pub struct Assets;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $((concat!("icons/", $name, ".svg"), include_bytes!(concat!("../assets/icons/", $name, ".svg")))),*
        ];
    };
}

icons!(
    "align-left",
    "archive",
    "arrow-down",
    "arrow-down-left",
    "arrow-left",
    "arrow-left-right",
    "arrow-right",
    "arrow-up",
    "arrow-up-narrow-wide",
    "arrow-up-right",
    "book-open",
    "bug",
    "binary",
    "bookmark",
    "calendar",
    "case-sensitive",
    "circle-check",
    "circle-help",
    "circle-plus",
    "clipboard",
    "clipboard-check",
    "clipboard-list",
    "clipboard-type",
    "clock",
    "check",
    "chevron-down",
    "chevron-right",
    "chevron-up",
    "chevrons-down-up",
    "chevrons-up-down",
    "code",
    "code-xml",
    "copy",
    "ellipsis",
    "ellipsis-vertical",
    "external-link",
    "file-plus",
    "file-question",
    "file-search",
    "file",
    "file-text",
    "files",
    "flame",
    "flask-conical",
    "folder-closed",
    "folder-open",
    "folder-search",
    "folder-tree",
    "folder-plus",
    "forward",
    "gallery-vertical",
    "git-fork",
    "heading",
    "image",
    "info",
    "layers",
    "kanban-square",
    "layout-dashboard",
    "layout-grid",
    "layout-list",
    "layout-template",
    "link",
    "links-coming-in",
    "links-going-out",
    "list",
    "list-start",
    "maximize",
    "menu",
    "minus",
    "move-horizontal",
    "move-vertical",
    "palette",
    "panel-left",
    "panel-right",
    "panels-top-left",
    "pen-line",
    "pencil",
    "picture-in-picture",
    "picture-in-picture-2",
    "pin",
    "plus",
    "quote",
    "rectangle-vertical",
    "redo-2",
    "refresh-cw",
    "rotate-ccw",
    "scissors",
    "search",
    "separator-vertical",
    "settings",
    "sliders-horizontal",
    "sticky-note",
    "square",
    "square-check",
    "square-dashed",
    "square-pen",
    "table",
    "tags",
    "terminal",
    "text",
    "trash-2",
    "triangle-alert",
    "type",
    "undo-2",
    "x",
    "zap",
    "zoom-in",
    "zoom-out",
    "text-align-start",
    "arrow-up-down",
    "stretch-horizontal",
    "list-filter",
    "grip-vertical",
    "eye-off",
    "square-function",
    "file-down",
    "chevron-left",
    "layers-2",
    "mouse-pointer-click",
    "calendar-range",
    "calculator",
    "group",
    "eraser",
    "arrow-down-wide-narrow",
    "circle-x",
    "circle-check-big",
    "bookmark-plus",
    "folder",
    "globe-2",
    "toy-brick",
    "wrench",
    "bookmark-filled",
    "grip-horizontal",
);

/// The logo, the one bundled asset that is not a monochrome icon. It is
/// loaded through [`img`] rather than [`svg`] because gpui draws an `svg()`
/// as a single-colour mask and rasterizes an `img()` through resvg, which is
/// what keeps the mark's five colours apart.
const LOGO: (&str, &[u8]) = ("logo.svg", include_bytes!("../assets/logo.svg"));

/// The mark's width over its height. Setting both axes from it keeps the
/// aspect ratio off the layout's hands. The `viewBox` it comes from is
/// 603.3067 x 430.8733, which is finer than an `f32` holds, so the ratio
/// is written out rather than divided here.
const LOGO_ASPECT: f32 = 1.400_195_2;

/// Every path [`Assets`] answers to.
fn bundled() -> impl Iterator<Item = &'static (&'static str, &'static [u8])> {
    ICONS.iter().chain(std::iter::once(&LOGO))
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(bundled()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(bundled()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}

/// Inter's static faces and Rubik's Black, registered with the text system
/// at startup so the families resolve on a machine that does not have them
/// installed.
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Rubik-Black.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Italic.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Medium.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-BoldItalic.ttf")),
    ]
}

/// The interface family [`fonts`] registers.
pub const FONT_FAMILY: &str = "Inter";

/// The display family [`fonts`] registers, in its Black weight only: the
/// product name under the mark is set in it, the way Anya's wordmark is.
pub const DISPLAY_FONT_FAMILY: &str = "Rubik";

/// A Lucide icon by name, `size` pixels square, painted in `color`.
/// Obsidian's icons are 16 px with a 2 px stroke on a 24-unit grid, which is
/// Lucide's own drawing, so a 16 px box here is the same picture.
pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// The Lucide icon Obsidian draws in a callout's title row, by canonical
/// callout type (see [`scoria_markdown::canonical_callout_type`]). The
/// pairing was read from the rendered `.callout-icon svg`'s own class in
/// Obsidian 1.14.5; the table is in `docs/scoria/Specification.md` under
/// "The lucide icon and colour of every built-in callout type". Two of the
/// names Obsidian leaves there are Lucide's older ones, and the drawing is
/// bundled under its current name instead: `check-circle-2` is
/// `circle-check`, `alert-triangle` is `triangle-alert`. A type Obsidian
/// does not know keeps its own name in the DOM but takes `note`'s pencil,
/// which it already has by the time it arrives here — the canonical type of
/// anything unrecognized is `note`.
pub fn callout_icon(kind: &str) -> &'static str {
    match kind {
        "abstract" => "clipboard-list",
        "info" => "info",
        "todo" => "circle-check",
        "tip" => "flame",
        "success" => "check",
        "question" => "circle-help",
        "warning" => "triangle-alert",
        "failure" => "x",
        "danger" => "zap",
        "bug" => "bug",
        "example" => "list",
        "quote" => "quote",
        _ => "pencil",
    }
}

/// The Scoria mark, `width` pixels wide and in its own colours.
pub fn logo(width: f32) -> Img {
    img(LOGO.0)
        .w(px(width))
        .h(px(width / LOGO_ASPECT))
        .flex_shrink_0()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_icon_loads_and_is_an_svg() {
        for (path, _) in ICONS {
            let bytes = Assets.load(path).unwrap().expect(path);
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(text.contains("<svg"), "{path} is not an svg");
            assert!(
                text.contains("viewBox=\"0 0 24 24\""),
                "{path} is not on Lucide's 24-unit grid"
            );
        }
    }

    #[test]
    fn every_callout_type_draws_the_icon_obsidian_draws() {
        // The whole of the probe's table, aliases and all, plus a type
        // Obsidian does not know: it falls back to note's pencil.
        let table = [
            ("note", "pencil"),
            ("abstract", "clipboard-list"),
            ("summary", "clipboard-list"),
            ("tldr", "clipboard-list"),
            ("info", "info"),
            ("todo", "circle-check"),
            ("tip", "flame"),
            ("hint", "flame"),
            ("important", "flame"),
            ("success", "check"),
            ("check", "check"),
            ("done", "check"),
            ("question", "circle-help"),
            ("help", "circle-help"),
            ("faq", "circle-help"),
            ("warning", "triangle-alert"),
            ("caution", "triangle-alert"),
            ("attention", "triangle-alert"),
            ("failure", "x"),
            ("fail", "x"),
            ("missing", "x"),
            ("danger", "zap"),
            ("error", "zap"),
            ("bug", "bug"),
            ("example", "list"),
            ("quote", "quote"),
            ("cite", "quote"),
            ("nosuchtype", "pencil"),
        ];
        for (written, want) in table {
            let name = callout_icon(scoria_markdown::canonical_callout_type(written));
            assert_eq!(name, want, "[!{written}]");
            let path = format!("icons/{name}.svg");
            assert!(
                Assets.load(&path).unwrap().is_some(),
                "{path} is not bundled"
            );
        }
        // The fold chevron, drawn in both states.
        for name in ["chevron-down", "chevron-right"] {
            assert!(Assets.load(&format!("icons/{name}.svg")).unwrap().is_some());
        }
    }

    #[test]
    fn the_logo_loads_and_keeps_its_own_colours() {
        let bytes = Assets.load(LOGO.0).unwrap().expect("logo.svg");
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("<svg"), "the logo is not an svg");
        // Five fills and no others, which is the whole reason it goes through
        // img() instead of svg(): the two near-blacks are the outline and the
        // vesicles, and neither is #000, so a mark drawn as a single-colour
        // mask would lose the drawing.
        for colour in ["#ee3300", "#ff5721", "#ff6f21", "#231f20", "#221f1f"] {
            assert!(text.contains(colour), "the logo lost {colour}");
        }
        // Flat on purpose: nothing is stroked and nothing is shaded, so the
        // mark is the same drawing at 16 px as at 1024.
        assert!(!text.contains("linearGradient"), "the logo grew a gradient");
        assert!(!text.contains("stroke="), "the logo grew a stroke");
        assert!(Assets.list("").unwrap().iter().any(|p| p == LOGO.0));
    }

    #[test]
    fn listing_filters_by_prefix() {
        assert!(Assets.list("icons/").unwrap().len() == ICONS.len());
        assert!(Assets.list("fonts/").unwrap().is_empty());
        assert!(Assets.load("icons/no-such-icon.svg").unwrap().is_none());
    }

    #[test]
    fn the_fonts_are_truetype() {
        for bytes in fonts() {
            // A TrueType face starts with the 0x00010000 sfnt version.
            assert_eq!(&bytes[..4], &[0, 1, 0, 0]);
        }
    }
}
