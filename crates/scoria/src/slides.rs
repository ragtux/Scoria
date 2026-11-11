//! Slides, `slides:start` ("Slides: Start presentation"): the active note
//! cut at its top-level thematic breaks and shown a slide at a time over
//! the whole window.
//!
//! Built to [[Specification#Observed Slides, Obsidian 1.15.0 (installer
//! build 1.13.7)]]. The note is the reading view's model — the one parser —
//! cut by [`cut`], and each slide is drawn by the reading view's renderer
//! with the slide's own metrics ([`BlockStyle::SLIDE`]) on a 960 × 700
//! stage scaled to the window ([`stage_scale`]). Nothing is written.
//!
//! Scoria's own: the keyboard is the presentation's as soon as it opens
//! (Obsidian's slide keys act only once the slide has been clicked), the
//! text is set in the interface font (Obsidian's stack starts at "Source
//! Sans Pro", which Scoria does not carry), a link on a slide does nothing,
//! and `o` and `?` are not bound (they showed nothing in Obsidian either).

use crate::assets::icon;
use crate::canvas::cards::Scaled;
use crate::reading::model::{Model, RBlock};
use crate::reading::view::{BlockStyle, LinkHandler, Renderer, ToggleHandler};
use crate::theme::Theme;
use gpui::{
    actions, div, prelude::*, px, relative, rgb, App, Context, EventEmitter, FocusHandle,
    Focusable, Hsla, KeyBinding, MouseButton, Window,
};
use scoria_formats::config::PropertyTypes;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// The stage every slide is laid out on before it is scaled
/// (`--slide-width`, `--slide-height`).
pub const STAGE_WIDTH: f32 = 960.;
pub const STAGE_HEIGHT: f32 = 700.;
/// The body size on the stage (`--r-main-font-size`) and its line height.
pub const MAIN_FONT_SIZE: f32 = 42.;
pub const MAIN_LINE_HEIGHT: f32 = 1.3;
/// `--r-link-color`: links, the arrows and the progress bar.
const LINK_COLOR: u32 = 0x42affa;

actions!(
    slides,
    [Next, Previous, First, Last, Close, Fullscreen, Pause]
);

/// The presentation's keys, as observed: next on ArrowRight, Space,
/// PageDown, ArrowDown, `n` and `l`; previous on ArrowLeft, PageUp, ArrowUp,
/// `p` and `h`; Home and End; `f` fullscreen; `b` and `.` black the screen;
/// Escape closes.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Slides");
    cx.bind_keys([
        KeyBinding::new("right", Next, ctx),
        KeyBinding::new("space", Next, ctx),
        KeyBinding::new("pagedown", Next, ctx),
        KeyBinding::new("down", Next, ctx),
        KeyBinding::new("n", Next, ctx),
        KeyBinding::new("l", Next, ctx),
        KeyBinding::new("left", Previous, ctx),
        KeyBinding::new("pageup", Previous, ctx),
        KeyBinding::new("up", Previous, ctx),
        KeyBinding::new("p", Previous, ctx),
        KeyBinding::new("h", Previous, ctx),
        KeyBinding::new("home", First, ctx),
        KeyBinding::new("end", Last, ctx),
        KeyBinding::new("f", Fullscreen, ctx),
        KeyBinding::new("b", Pause, ctx),
        KeyBinding::new(".", Pause, ctx),
        KeyBinding::new("escape", Close, ctx),
    ]);
}

/// The note's blocks cut into slides: a new slide at every top-level
/// thematic break, the break itself not drawn, and a slide with nothing in
/// it dropped. A break inside a quote or a list is a child of that block
/// and does not cut; a setext underline is a heading, not a break. The
/// properties block counts as something, so a slide holding only the
/// frontmatter is kept and shows blank (observed: a note that is only
/// frontmatter is one blank slide; an empty note has none).
pub fn cut(model: &Model) -> Vec<Vec<RBlock>> {
    let mut slides = vec![Vec::new()];
    for block in &model.blocks {
        match block {
            RBlock::ThematicBreak => slides.push(Vec::new()),
            other => slides.last_mut().expect("one slide").push(other.clone()),
        }
    }
    slides.retain(|s| !s.is_empty());
    slides
}

/// `--slide-scale`: the stage scaled to fit 96 % of the window either way.
pub fn stage_scale(width: f32, height: f32) -> f32 {
    (width * 0.96 / STAGE_WIDTH).min(height * 0.96 / STAGE_HEIGHT)
}

/// Where the presentation is: the slides, the one on screen and whether
/// the screen is blacked out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deck {
    pub slides: Vec<Vec<RBlock>>,
    pub index: usize,
    pub paused: bool,
}

impl Deck {
    pub fn new(slides: Vec<Vec<RBlock>>) -> Deck {
        Deck {
            slides,
            index: 0,
            paused: false,
        }
    }

    /// One on, stopping at the last (no wrap).
    pub fn next(&mut self) {
        if self.has_next() {
            self.index += 1;
        }
    }

    /// One back, stopping at the first.
    pub fn previous(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn first(&mut self) {
        self.index = 0;
    }

    pub fn last(&mut self) {
        self.index = self.slides.len().saturating_sub(1);
    }

    /// Whether the left arrow shows.
    pub fn has_previous(&self) -> bool {
        self.index > 0
    }

    /// Whether the right arrow shows.
    pub fn has_next(&self) -> bool {
        self.index + 1 < self.slides.len()
    }

    /// The progress bar's share of the width: the slides behind over all
    /// but one.
    pub fn progress(&self) -> f32 {
        match self.slides.len() {
            0 | 1 => 0.,
            n => self.index as f32 / (n - 1) as f32,
        }
    }
}

pub enum SlidesEvent {
    Close,
}

/// The presentation, drawn over the whole window while it is up.
pub struct SlidesView {
    focus_handle: FocusHandle,
    deck: Deck,
    /// The note it was started on, for the control socket.
    path: String,
    /// Callouts folded open or shut on a slide, by text id.
    toggled: Rc<RefCell<HashSet<usize>>>,
}

impl EventEmitter<SlidesEvent> for SlidesView {}

impl Focusable for SlidesView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SlidesView {
    pub fn new(path: String, model: &Model, cx: &mut Context<Self>) -> SlidesView {
        SlidesView {
            focus_handle: cx.focus_handle(),
            deck: Deck::new(cut(model)),
            path,
            toggled: Rc::default(),
        }
    }

    /// The control socket's view of it.
    pub fn describe(&self) -> Value {
        json!({
            "path": self.path,
            "slide": self.deck.index,
            "slides": self.deck.slides.len(),
            "paused": self.deck.paused,
        })
    }

    fn go(&mut self, f: impl FnOnce(&mut Deck), cx: &mut Context<Self>) {
        // A blacked-out screen takes the next key back to the slide.
        if self.deck.paused {
            self.deck.paused = false;
        } else {
            f(&mut self.deck);
        }
        cx.notify();
    }

    fn on_next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        self.go(Deck::next, cx);
    }

    fn on_previous(&mut self, _: &Previous, _: &mut Window, cx: &mut Context<Self>) {
        self.go(Deck::previous, cx);
    }

    fn on_first(&mut self, _: &First, _: &mut Window, cx: &mut Context<Self>) {
        self.go(Deck::first, cx);
    }

    fn on_last(&mut self, _: &Last, _: &mut Window, cx: &mut Context<Self>) {
        self.go(Deck::last, cx);
    }

    fn on_pause(&mut self, _: &Pause, _: &mut Window, cx: &mut Context<Self>) {
        self.deck.paused = !self.deck.paused;
        cx.notify();
    }

    fn on_fullscreen(&mut self, _: &Fullscreen, window: &mut Window, _: &mut Context<Self>) {
        window.toggle_fullscreen();
    }

    fn on_close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(SlidesEvent::Close);
    }

    /// The slide on screen, set on the stage at `scale`.
    fn render_slide(
        &self,
        blocks: &[RBlock],
        scale: f32,
        theme: &Theme,
        window: &mut Window,
    ) -> gpui::AnyElement {
        // Every size in the renderer is a multiple of the base, so a slide
        // theme with the stage's body size times the scale scales it all.
        let mut slide = theme.clone();
        slide.base_font_size = MAIN_FONT_SIZE * scale;
        slide.text_font = theme.ui_font.clone();
        let (text, _) = slide_colors(theme.dark);
        slide.tokens.text_normal = text;
        slide.tokens.link_color = rgb(LINK_COLOR).into();
        let base = slide.base_font_size;
        let types = PropertyTypes::default();
        let toggled = self.toggled.borrow().clone();
        // Whether a link on a slide can be followed was not observed.
        let on_link: LinkHandler = Rc::new(|_, _, _| {});
        let on_toggle: ToggleHandler = {
            let toggled = self.toggled.clone();
            Rc::new(move |id, cx| {
                let mut toggled = toggled.borrow_mut();
                if !toggled.remove(&id) {
                    toggled.insert(id);
                }
                cx.refresh_windows();
            })
        };
        let renderer = Renderer::new(
            &slide,
            window.text_system().clone(),
            STAGE_WIDTH * scale,
            &toggled,
            &types,
            None,
            None,
            on_link,
            on_toggle,
        )
        .with_style(BlockStyle::SLIDE);
        // The frontmatter rides in the first slide, hidden.
        let elements: Vec<gpui::AnyElement> = blocks
            .iter()
            .filter(|b| !matches!(b, RBlock::Properties(_)))
            .map(|b| renderer.block(b, false))
            .collect();
        Scaled::new(
            // The renderer's rem spacings (a list's indent, a code block's
            // padding) follow the slide's body size.
            px(base),
            div()
                .w(px(STAGE_WIDTH * scale))
                .h(px(STAGE_HEIGHT * scale))
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .text_center()
                .font_family(slide.text_font.clone())
                .text_size(px(base))
                .line_height(px(base * MAIN_LINE_HEIGHT))
                .text_color(text)
                .children(elements.into_iter().map(|e| {
                    // Each block centred on the stage, a list or a table
                    // at its own width the way reveal sets them inline.
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .justify_center()
                        .child(div().max_w_full().child(e))
                })),
        )
        .into_any_element()
    }
}

/// The body text and the ground: white on `#191919` in the dark theme,
/// `#222` on white in the light one.
fn slide_colors(dark: bool) -> (Hsla, Hsla) {
    if dark {
        (rgb(0xffffff).into(), rgb(0x191919).into())
    } else {
        (rgb(0x222222).into(), rgb(0xffffff).into())
    }
}

/// One of reveal's arrows ("previous slide", "next slide"): `#42affa`,
/// 36 × 30, shown only where it leads. The up and down arrows never lead
/// anywhere on a deck with no vertical slides and are not drawn.
fn arrow(
    id: &'static str,
    glyph: &'static str,
    on: bool,
    cx: &mut Context<SlidesView>,
    go: fn(&mut Deck),
) -> gpui::AnyElement {
    div()
        .id(id)
        .w(px(36.))
        .h(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(!on, |d| d.invisible())
        .child(icon(glyph, 26., rgb(LINK_COLOR).into()))
        .on_click(cx.listener(move |this, _, _, cx| this.go(go, cx)))
        .into_any_element()
}

impl Render for SlidesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let scale = stage_scale(w, h);
        let (_, ground) = slide_colors(theme.dark);
        let link: Hsla = rgb(LINK_COLOR).into();
        let slide = self
            .deck
            .slides
            .get(self.deck.index)
            .map(|blocks| self.render_slide(blocks, scale, &theme, window));
        let (prev, next, progress) = (
            self.deck.has_previous(),
            self.deck.has_next(),
            self.deck.progress(),
        );
        div()
            .id("slides-container")
            .key_context("Slides")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_next))
            .on_action(cx.listener(Self::on_previous))
            .on_action(cx.listener(Self::on_first))
            .on_action(cx.listener(Self::on_last))
            .on_action(cx.listener(Self::on_pause))
            .on_action(cx.listener(Self::on_fullscreen))
            .on_action(cx.listener(Self::on_close))
            // A click on the slide takes the keyboard back and moves nothing.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| window.focus(&this.focus_handle, cx)),
            )
            .occlude()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .bg(ground)
            .flex()
            .items_center()
            .justify_center()
            .children(slide)
            // `div.slides-close-btn`: `lucide-x-square` at 16 px in `#666`,
            // 8 px in from the top right.
            .child(
                div()
                    .id("slides-close")
                    .absolute()
                    .top(px(8.))
                    .right(px(8.))
                    .cursor_pointer()
                    .child(icon("x-square", 16., rgb(0x666666).into()))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SlidesEvent::Close))),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12.))
                    .bottom(px(12.))
                    .flex()
                    .flex_row()
                    .gap(px(4.))
                    .child(arrow(
                        "slides-previous",
                        "chevron-left",
                        prev,
                        cx,
                        Deck::previous,
                    ))
                    .child(arrow("slides-next", "chevron-right", next, cx, Deck::next)),
            )
            // The 3 px progress bar along the bottom edge.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .h(px(3.))
                    .w(relative(progress))
                    .bg(link),
            )
            // `b` and `.`: the pause overlay, black over everything.
            .when(self.deck.paused, |d| {
                d.child(div().absolute().inset_0().bg(rgb(0x000000)))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading::model::build;
    use crate::reading::EmbedSource;
    use std::path::PathBuf;

    struct NoVault;

    impl EmbedSource for NoVault {
        fn resolve(&self, _: &str, _: &str) -> Option<String> {
            None
        }
        fn read(&self, _: &str) -> Option<String> {
            None
        }
        fn absolute(&self, path: &str) -> PathBuf {
            PathBuf::from(path)
        }
    }

    fn slides(text: &str) -> Vec<Vec<RBlock>> {
        cut(&build("Deck.md", text, &NoVault))
    }

    /// What each slide starts with, for the assertions: a heading's text,
    /// `p` for a paragraph, `props` for the hidden frontmatter, and so on.
    fn heads(text: &str) -> Vec<Vec<String>> {
        slides(text)
            .iter()
            .map(|s| {
                s.iter()
                    .map(|b| match b {
                        RBlock::Heading { plain, level, .. } => format!("h{level} {plain}"),
                        RBlock::Paragraph(_) => "p".into(),
                        RBlock::Properties(_) => "props".into(),
                        RBlock::Code { .. } => "code".into(),
                        RBlock::Quote(_) => "quote".into(),
                        RBlock::List { .. } => "list".into(),
                        RBlock::Table { .. } => "table".into(),
                        other => format!("{other:?}").chars().take(12).collect(),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn every_spelling_of_a_top_level_break_cuts() {
        for rule in ["---", "***", "___", "- - -", "-----", "  ---"] {
            let text = format!("One\n\n{rule}\n\nTwo\n");
            assert_eq!(heads(&text), [["p"], ["p"]], "{rule:?}");
        }
    }

    #[test]
    fn a_break_in_a_fence_or_a_quote_does_not_cut() {
        assert_eq!(heads("One\n\n```\n---\n```\n\nTwo\n"), [["p", "code", "p"]]);
        assert_eq!(
            heads("One\n\n> a\n> ---\n> b\n\nTwo\n"),
            [["p", "quote", "p"]]
        );
    }

    #[test]
    fn a_setext_underline_is_a_heading_not_a_cut() {
        assert_eq!(
            heads("# Fourth\n\nText right under.\n---\n\nA heading\n---\n"),
            [["h1 Fourth", "h2 Text right under.", "h2 A heading"]]
        );
    }

    #[test]
    fn a_trailing_break_leaves_no_empty_slide_and_an_empty_note_none() {
        assert_eq!(heads("One\n\n---\n\nTwo\n\n---\n"), [["p"], ["p"]]);
        assert!(slides("").is_empty());
        assert_eq!(heads("One\n\n---\n\n---\n\nTwo\n"), [["p"], ["p"]]);
    }

    #[test]
    fn the_frontmatter_rides_in_the_first_slide_and_alone_is_a_blank_one() {
        assert_eq!(
            heads("---\nkey: value\n---\n# First\n\nText\n\n---\n\n## Second\n"),
            [vec!["props", "h1 First", "p"], vec!["h2 Second"]]
        );
        assert_eq!(
            heads("---\nkey: value\n---\n---\n\nAfter\n"),
            [["props"], ["p"]]
        );
        assert_eq!(heads("---\nkey: value\n---\n"), [["props"]]);
    }

    /// The probe's deck: frontmatter, four slides.
    #[test]
    fn the_probes_deck_is_four_slides() {
        let text = "---\ntags: [deck]\n---\n# First slide\n\nIntro paragraph.\n\n---\n\n## Second slide\n\n- one\n- two\n\n---\n\nSome code:\n\n```js\nlet x = 1;\n```\n\n---\n\n# Fourth\n\nText right under.\n---\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
        assert_eq!(
            heads(text),
            [
                vec!["props", "h1 First slide", "p"],
                vec!["h2 Second slide", "list"],
                vec!["p", "code"],
                vec!["h1 Fourth", "h2 Text right under.", "table"],
            ]
        );
    }

    /// 1596 × 973 gave 1.3344: the width is the tighter side there.
    #[test]
    fn the_stage_scales_to_the_tighter_side() {
        assert!((stage_scale(1596., 973.) - 1.3344).abs() < 1e-4);
        assert!((stage_scale(960., 2000.) - 0.96).abs() < 1e-6);
        assert!((stage_scale(4000., 700.) - 0.96).abs() < 1e-6);
    }

    #[test]
    fn the_deck_stops_at_either_end_and_jumps_home_and_end() {
        let mut deck = Deck::new(vec![vec![], vec![], vec![]]);
        assert!(!deck.has_previous() && deck.has_next());
        deck.previous();
        assert_eq!(deck.index, 0);
        deck.next();
        deck.next();
        deck.next();
        assert_eq!(deck.index, 2);
        assert!(deck.has_previous() && !deck.has_next());
        assert_eq!(deck.progress(), 1.0);
        deck.first();
        assert_eq!((deck.index, deck.progress()), (0, 0.0));
        deck.last();
        assert_eq!(deck.index, 2);
        let mut empty = Deck::new(vec![]);
        empty.next();
        empty.last();
        assert_eq!((empty.index, empty.progress()), (0, 0.0));
    }

    #[test]
    fn a_slides_metrics_are_reveals_on_the_stage() {
        let s = BlockStyle::SLIDE;
        let h1 = s.headings[0];
        assert_eq!(MAIN_FONT_SIZE * h1.0, 105.);
        assert_eq!((h1.1, h1.2), (600., 1.2));
        assert_eq!(s.headings[1].0, 1.6);
        assert!((MAIN_FONT_SIZE * s.gap - 20.).abs() < 1e-4);
        assert!((MAIN_FONT_SIZE * s.code_size - 23.1).abs() < 1e-4);
        let reading = BlockStyle::READING;
        assert_eq!(
            (s.uppercase_headings, reading.uppercase_headings),
            (true, false)
        );
        assert_eq!(
            BlockStyle::READING.headings[0],
            crate::theme::heading_style(1)
        );
    }

    #[test]
    fn capitals_keep_every_byte_range() {
        use crate::reading::model::RichText;
        let text = RichText {
            id: 0,
            text: "straße émigré".into(),
            runs: vec![],
            links: vec![],
        };
        let up = crate::reading::view::uppercased(&text);
        assert_eq!(up.text, "STRAßE ÉMIGRÉ");
        assert_eq!(up.text.len(), text.text.len());
    }
}
