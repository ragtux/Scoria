//! `![[Board.canvas]]` in a note, and a canvas file card holding another
//! canvas: the canvas's minimap under its name, which is what Obsidian
//! draws there — not a live canvas
//! ([[Specification#Observed canvas embeds, audio and video, Obsidian 1.15.0]]).
//!
//! Observed (`tools/obsidian-probe/results/2026-11-15-embeds-media.json`,
//! `s01`..`s05`): `.internal-embed.canvas-embed`, a block the note's full
//! width with no border, padding or margin; an `.embed-title` row 32 px
//! tall (24 px line, 8 px under it) holding the `layout-dashboard` icon at
//! 16 px in the muted colour, a 4 px gap and the file's name without its
//! extension at the body size in weight 600; then `svg.canvas-minimap`, 100 %
//! wide, `max-height: 400px`, 4 px of padding, and the 6 px an inline SVG
//! leaves under its baseline. The `|400`, `|400x300` and `|x300` hints change
//! nothing. Nothing in it moves: the wheel scrolls the note, nothing pans,
//! zooms or selects, the pointer stays the arrow, and a click anywhere on
//! it — title or picture, reading view or Live Preview — opens the canvas
//! (Ctrl+click in a new tab). An empty or unreadable canvas is the title
//! alone; a missing one is "“Missing.canvas” is not created yet. Click to
//! create." A canvas file card of a canvas shows the same picture, without
//! the title (the card's label names it).
//!
//! The paint: an uncoloured card is filled with `--background-modifier-border`
//! at 0.65 and outlined in it, a coloured one in its colour at 0.5; an edge is
//! `#c0c0c0` in both themes (not `--canvas-color`, which is `#7e7e7e` on the
//! dark theme), or its own colour.

use std::rc::Rc;

use gpui::{
    canvas, div, fill, point, px, AnyElement, App, Bounds, Corners, Edges, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, PathBuilder, Pixels, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use scoria_canvas::minimap::{
    picture_height, Card, Minimap, CARD_RADIUS, CARD_STROKE, COLORED_FILL_OPACITY, FILL_OPACITY,
};

use crate::theme::Theme;

/// The picture's padding and its tallest, in pixels.
pub const PADDING: f32 = 4.0;
pub const MAX_HEIGHT: f32 = 400.0;
/// The title row: a 24 px line and 8 px under it.
pub const TITLE_HEIGHT: f32 = 32.0;
/// What an inline SVG leaves under its baseline at the body size.
pub const DESCENT: f32 = 6.0;
/// An edge with no colour of its own.
const EDGE: u32 = 0xc0c0c0;

/// A `.canvas` embedded in a note: read and reduced to its minimap when the
/// note is built.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasEmbed {
    /// The vault path, which a click opens.
    pub path: String,
    /// The name shown: the file's, without `.canvas`.
    pub name: String,
    /// `None` for a canvas with no cards or one that does not parse.
    pub minimap: Option<Minimap>,
}

impl CanvasEmbed {
    /// The embed of the canvas at `path`, whose text is `text` (`None` when
    /// it could not be read).
    pub fn new(path: &str, text: Option<&str>) -> CanvasEmbed {
        let file = path.rsplit('/').next().unwrap_or(path);
        let name = file
            .strip_suffix(".canvas")
            .filter(|s| !s.is_empty())
            .unwrap_or(file)
            .to_string();
        let minimap = text
            .and_then(|t| scoria_formats::canvas::Canvas::from_json(t).ok())
            .and_then(|c| Minimap::of(&c));
        CanvasEmbed {
            path: path.to_string(),
            name,
            minimap,
        }
    }

    /// The embed's height in a note `width` pixels wide.
    #[cfg(test)]
    pub fn height(&self, width: f32) -> f32 {
        TITLE_HEIGHT
            + self
                .minimap
                .as_ref()
                .map(|m| {
                    picture_height(&m.view, width as f64, PADDING as f64, MAX_HEIGHT as f64) as f32
                        + DESCENT
                })
                .unwrap_or(0.0)
    }
}

/// What a click on the embed does: open this vault path.
pub type OpenPath = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The embed in a note `available` pixels wide.
pub fn render(embed: &CanvasEmbed, theme: &Theme, available: f32, open: OpenPath) -> AnyElement {
    let t = theme;
    let base = t.base_font_size;
    let path = embed.path.clone();
    let mut el = div()
        .id(SharedString::from(format!("canvas-embed:{}", embed.path)))
        .w_full()
        .flex()
        .flex_col()
        .on_click(move |_, window, cx| open(&path, window, cx))
        .child(
            div()
                .h(px(TITLE_HEIGHT))
                .pb(px(8.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(crate::assets::icon(
                    "layout-dashboard",
                    16.,
                    t.tokens.text_muted,
                ))
                .child(
                    div()
                        .text_size(px(base))
                        .line_height(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.tokens.text_normal)
                        .child(SharedString::from(embed.name.clone())),
                ),
        );
    if let Some(m) = &embed.minimap {
        let h = picture_height(&m.view, available as f64, PADDING as f64, MAX_HEIGHT as f64) as f32;
        el = el
            .child(picture(m.clone(), theme, h))
            .child(div().h(px(DESCENT)));
    }
    el.into_any_element()
}

/// The picture alone, `height` pixels tall and as wide as it is given: the
/// embed's, and a canvas card's (which fills the card).
pub fn picture(minimap: Minimap, theme: &Theme, height: f32) -> impl IntoElement {
    let plain = theme.tokens.background_modifier_border;
    let presets = theme.canvas.clone();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            paint(&minimap, bounds, PADDING, plain, &presets, window);
        },
    )
    .w_full()
    .h(px(height))
}

/// A canvas card on another canvas: the picture filling the card, scaled
/// with the zoom (`scale` pixels per canvas unit).
pub fn card(minimap: Minimap, theme: &Theme, scale: f32) -> AnyElement {
    let plain = theme.tokens.background_modifier_border;
    let presets = theme.canvas.clone();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            paint(&minimap, bounds, PADDING * scale, plain, &presets, window);
        },
    )
    .size_full()
    .into_any_element()
}

fn color_of(color: Option<&str>, presets: &crate::theme::CanvasColors) -> Option<Hsla> {
    use scoria_canvas::Color;
    match Color::parse(color) {
        Color::Default => None,
        Color::Preset(n) => presets.preset(n),
        Color::Hex(v) => Some(gpui::rgb(v).into()),
    }
}

fn paint(
    m: &Minimap,
    bounds: Bounds<Pixels>,
    padding: f32,
    plain: Hsla,
    presets: &crate::theme::CanvasColors,
    window: &mut Window,
) {
    let w = (f32::from(bounds.size.width) - 2.0 * padding).max(0.0) as f64;
    let h = (f32::from(bounds.size.height) - 2.0 * padding).max(0.0) as f64;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (scale, offset) = m.fit(w, h);
    let origin = bounds.origin;
    let to_screen = |x: f64, y: f64| {
        point(
            origin.x + px(padding + (offset.x + (x - m.view.x) * scale) as f32),
            origin.y + px(padding + (offset.y + (y - m.view.y) * scale) as f32),
        )
    };
    let card = |c: &Card, window: &mut Window| {
        let (line, alpha) = match color_of(c.color.as_deref(), presets) {
            Some(col) => (col, COLORED_FILL_OPACITY),
            None => (plain, FILL_OPACITY),
        };
        // The stroke is centred on the card's edge: the quad is the card
        // grown by half of it, and its border the whole of it, inside.
        let half = CARD_STROKE / 2.0;
        let r = &c.rect;
        let a = to_screen(r.x - half, r.y - half);
        let b = to_screen(r.x + r.width + half, r.y + r.height + half);
        let radius = (CARD_RADIUS.min(r.width / 2.0).min(r.height / 2.0) + half) * scale;
        let mut body = line;
        body.a *= alpha;
        let mut quad = fill(Bounds::from_corners(a, b), body);
        quad.corner_radii = Corners::all(px(radius as f32));
        quad.border_widths = Edges::all(px((CARD_STROKE * scale) as f32));
        quad.border_color = line;
        window.paint_quad(quad);
    };
    for g in &m.groups {
        card(g, window);
    }
    let width = px((m.edge_width * scale).max(0.5) as f32);
    for e in &m.edges {
        let color = color_of(e.color.as_deref(), presets).unwrap_or_else(|| gpui::rgb(EDGE).into());
        let mut builder = PathBuilder::stroke(width);
        builder.move_to(to_screen(e.from.x, e.from.y));
        builder.cubic_bezier_to(
            to_screen(e.to.x, e.to.y),
            to_screen(e.c1.x, e.c1.y),
            to_screen(e.c2.x, e.c2.y),
        );
        if let Ok(built) = builder.build() {
            window.paint_path(built, color);
        }
    }
    for c in &m.cards {
        card(c, window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = r#"{"nodes":[
        {"id":"g1","type":"group","x":-60,"y":-60,"width":760,"height":420},
        {"id":"t1","type":"text","x":0,"y":0,"width":250,"height":60,"text":"A"}],"edges":[]}"#;

    #[test]
    fn the_name_drops_the_extension_and_the_height_follows_the_width() {
        let e = CanvasEmbed::new("Boards/Board.canvas", Some(BOARD));
        assert_eq!(e.name, "Board");
        // Observed at 700 px: 32 + 400 + 6.
        assert_eq!(e.height(700.0), 438.0);
        let wide = CanvasEmbed::new(
            "Wide.canvas",
            Some(
                r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":3000,"height":100,"text":"W"}],"edges":[]}"#,
            ),
        );
        // Observed: 129.9.
        assert!(
            (wide.height(700.0) - 129.9).abs() < 0.1,
            "{}",
            wide.height(700.0)
        );
    }

    #[test]
    fn an_empty_or_broken_canvas_is_the_title_alone() {
        for text in [
            Some(""),
            Some("{not json"),
            Some(r#"{"nodes":[],"edges":[]}"#),
            None,
        ] {
            let e = CanvasEmbed::new("Empty.canvas", text);
            assert!(e.minimap.is_none());
            assert_eq!(e.height(700.0), TITLE_HEIGHT);
        }
    }
}
