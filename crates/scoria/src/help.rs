//! The Help window, what the vault profile's `circle-help` button and
//! `app:open-help` open.
//!
//! Obsidian 1.14 answers "Open help" with a window of its own rather than
//! the browser: its mark, name and version over a card of four rows, each
//! an icon, a title, a line of explanation and a button on the right —
//! the help site (an accent "Visit"), its Discord ("Join"), the forum
//! ("Visit") and the sandbox vault ("Open"). Scoria draws the same window
//! with its own four: the manual, the source, the issue tracker and the
//! sandbox vault. Escape or the frame's close button dismisses it, and
//! opening help again while it is up brings it forward rather than making
//! a second one.

use crate::assets::{icon, logo, DISPLAY_FONT_FAMILY};
use crate::theme::{Theme, FONT_UI_MEDIUM, FONT_UI_SMALL};
use crate::workspace::{
    client_frame, frame_window_options, window_title_row, WindowCorners, Workspace,
};
use gpui::{
    actions, div, prelude::*, px, size, AnyWindowHandle, App, Bounds, Context, FocusHandle,
    Focusable, FontWeight, KeyBinding, SharedString, WeakEntity, Window, WindowBounds,
    WindowHandle, WindowOptions,
};

actions!(help, [Close]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Close, Some("Help"))]);
}

/// The window's size: the shape of Obsidian's, as wide as the vault
/// manager's splash column and tall enough for the mark and the name at
/// the sizes the vault manager sets them, and the four rows under them,
/// before the column has to scroll. The frame's margin is inside it.
pub const WINDOW_SIZE: (f32, f32) = (700., 980.);

/// The card under the mark, and the parts of a row in it: the card's
/// width, its side padding, the icon's column, the gap between the parts
/// and the button's width. They are definite on purpose, the way the
/// vault manager's are - the blurb's column is the card less the rest,
/// not what the flex solve leaves it - because gpui lays a text out at
/// the width it was last measured at, and a width that only the solve
/// knows was measured at two lines and painted at three, with the last
/// row running out of the card's bottom.
const CARD_WIDTH: f32 = 600.;
const CARD_PADDING: f32 = 24.;
const ICON_WIDTH: f32 = 64.;
const ROW_GAP: f32 = 16.;
const BUTTON_WIDTH: f32 = 96.;
const BLURB_WIDTH: f32 = CARD_WIDTH - 2. * CARD_PADDING - ICON_WIDTH - 2. * ROW_GAP - BUTTON_WIDTH;

/// Where "Visit" goes for the manual: Obsidian opens its help site, Scoria
/// scoria.cc.
pub const MANUAL_URL: &str = "https://scoria.cc";
pub const SOURCE_URL: &str = "https://github.com/ragtux/scoria";
pub const ISSUES_URL: &str = "https://github.com/ragtux/scoria/issues";

/// What a row's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Go {
    Url(&'static str),
    Sandbox,
}

/// One row of the card.
struct Row {
    icon: &'static str,
    title: &'static str,
    blurb: &'static str,
    button: &'static str,
    /// Drawn in the accent, the one row Obsidian singles out.
    primary: bool,
    go: Go,
}

/// The rows, in Obsidian's order: help first, the sandbox last.
const ROWS: &[Row] = &[
    Row {
        icon: "book-open",
        title: "Manual",
        blurb: "Read the Scoria manual: what the app does with a vault, and what it does not yet.",
        button: "Visit",
        primary: true,
        go: Go::Url(MANUAL_URL),
    },
    Row {
        icon: "code",
        title: "Source code",
        blurb: "Scoria is open source. Read the code, build it yourself, or send a change.",
        button: "Visit",
        primary: false,
        go: Go::Url(SOURCE_URL),
    },
    Row {
        icon: "bug",
        title: "Report an issue",
        blurb: "Found something Obsidian does that Scoria does not? Say which note shows it.",
        button: "Visit",
        primary: false,
        go: Go::Url(ISSUES_URL),
    },
    Row {
        icon: "flask-conical",
        title: "Sandbox vault",
        blurb: "Play around and experiment with various features. The sandbox is a fresh \
                folder in the temporary directory, so nothing you have is touched.",
        button: "Open",
        primary: false,
        go: Go::Sandbox,
    },
];

/// The Help window's root view.
pub struct HelpWindow {
    focus_handle: FocusHandle,
    /// The workspace that opened it, for the sandbox vault.
    workspace: WeakEntity<Workspace>,
    main: AnyWindowHandle,
}

impl HelpWindow {
    /// Opens the window next to `main`, the workspace's own, which is what
    /// the sandbox vault loads into.
    pub fn open(
        workspace: WeakEntity<Workspace>,
        main: AnyWindowHandle,
        cx: &mut App,
    ) -> anyhow::Result<WindowHandle<HelpWindow>> {
        let (w, h) = WINDOW_SIZE;
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // Wide enough for the card.
                window_min_size: Some(size(px(700.), px(480.))),
                // The same frame the main window has, with a title row of
                // its own for the window buttons.
                ..frame_window_options("Help")
            },
            move |window, cx| {
                let view = cx.new(|cx| HelpWindow {
                    focus_handle: cx.focus_handle(),
                    workspace,
                    main,
                });
                let handle = view.read(cx).focus_handle.clone();
                window.focus(&handle, cx);
                view
            },
        )
    }

    fn go(&mut self, go: Go, window: &mut Window, cx: &mut Context<Self>) {
        match go {
            Go::Url(url) => cx.open_url(url),
            Go::Sandbox => {
                let workspace = self.workspace.clone();
                self.main
                    .update(cx, |_, main_window, cx| {
                        workspace
                            .update(cx, |ws, cx| ws.open_sandbox_vault(main_window, cx))
                            .ok();
                        main_window.activate_window();
                    })
                    .ok();
                window.remove_window();
            }
        }
    }

    fn close(&mut self, _: &Close, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }

    fn render_row(
        &self,
        row: &'static Row,
        last: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let go = row.go;
        let (bg, fg, border) = if row.primary {
            (
                theme.tokens.interactive_accent,
                gpui::white(),
                theme.tokens.interactive_accent,
            )
        } else {
            (
                theme.tokens.background_modifier_hover,
                theme.tokens.text_normal,
                theme.tokens.background_modifier_border,
            )
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(ROW_GAP))
            .py(px(18.))
            .when(!last, |d| {
                d.border_b_1()
                    .border_color(theme.tokens.background_modifier_border)
            })
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(ICON_WIDTH))
                    .flex()
                    .justify_center()
                    .child(icon(row.icon, 48., theme.tokens.text_faint)),
            )
            .child(
                div()
                    .w(px(BLURB_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(FONT_UI_MEDIUM))
                            .text_color(theme.tokens.text_normal)
                            .child(row.title),
                    )
                    .child(
                        div()
                            .text_size(px(FONT_UI_SMALL))
                            .text_color(theme.tokens.text_muted)
                            .child(row.blurb),
                    ),
            )
            .child(
                div()
                    .id(row.title)
                    .flex_shrink_0()
                    .px(px(16.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(BUTTON_WIDTH))
                    .rounded(px(6.))
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(fg)
                    .cursor_pointer()
                    .hover(move |s| {
                        if row.primary {
                            s.opacity(0.9)
                        } else {
                            s.bg(theme.tokens.background_modifier_hover)
                        }
                    })
                    .on_click(cx.listener(move |this, _, window, cx| this.go(go, window, cx)))
                    .child(row.button),
            )
    }
}

impl Focusable for HelpWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HelpWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let corners = WindowCorners::of(window);
        // The row is the splash's own ground rather than a bar, as the
        // vault manager's is: the buttons sit in the dark, over nothing.
        let title_row = window_title_row(
            "Help",
            theme.tokens.background_primary,
            &theme,
            window,
            |w, _| w.remove_window(),
        );
        let version: SharedString = concat!("Version ", env!("CARGO_PKG_VERSION")).into();
        // Nothing in the column shrinks: when the window is short the
        // column scrolls, rather than the card closing up on its last row.
        let mut card = div()
            .flex_shrink_0()
            .w(px(CARD_WIDTH))
            .px(px(CARD_PADDING))
            .rounded(px(12.))
            .bg(theme.tokens.background_secondary)
            .flex()
            .flex_col();
        let count = ROWS.len();
        for (ix, row) in ROWS.iter().enumerate() {
            card = card.child(self.render_row(row, ix + 1 == count, &theme, cx));
        }
        // The same splash as the vault manager's: the mark at 264, the
        // name at 104 in the display face, the version, and the card.
        let body = div()
            .id("help-body")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .map(|d| corners.bottom().apply(d))
            // The column is a block inside the scroll box, not the scroll
            // box itself: a text that wraps inside a flex item of a
            // scrolling column is measured at one line and painted at two.
            .child(
                div()
                    .w_full()
                    .min_h_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .px(px(30.))
                    .py(px(24.))
                    // Two spacers rather than `justify_center`: they share
                    // whatever height is spare and shrink to nothing when
                    // there is none, so a short window scrolls to the top
                    // of the mark instead of clipping it behind the centring.
                    .child(div().flex_1())
                    .child(logo(264.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .mt(px(16.))
                            .font_family(DISPLAY_FONT_FAMILY)
                            .text_size(px(104.))
                            .line_height(px(104.))
                            .font_weight(FontWeight::BLACK)
                            .text_color(theme.tokens.text_normal)
                            .child("Scoria"),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .mt(px(4.))
                            .text_size(px(FONT_UI_SMALL))
                            .text_color(theme.tokens.text_muted)
                            .child(version),
                    )
                    .child(div().flex_shrink_0().h(px(32.)))
                    .child(card)
                    .child(div().flex_1()),
            );
        let root = div()
            .id("help-window")
            .key_context("Help")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.tokens.background_primary)
            .map(|d| corners.apply(d))
            .font_family(theme.ui_font.clone())
            .text_color(theme.tokens.text_normal)
            .children(title_row)
            .child(body);
        client_frame(root.into_any_element(), &theme, window)
    }
}
