//! The dock panes' shared tree: Obsidian's `nav-header` with its
//! `nav-action-button`s, and the `tree-item-self` row every tree pane draws
//! (Tags, All properties, Outline, Bookmarks). One implementation, so a
//! measurement taken on one pane moves them all; the host view is a type
//! parameter, so a pane that is its own entity draws with the same code as
//! the panes the workspace draws itself.

use crate::assets::icon;
use crate::canvas::menu::tooltip;
use crate::text_input::TextInput;
use crate::theme::{Theme, FONT_UI_SMALL, FONT_UI_SMALLER, HEADER_HEIGHT, ICON_SIZE};
use gpui::{div, prelude::*, px, Context, Entity, SharedString, Window};

/// What a click on a row's fold triangle does to the host.
pub type FoldAction<V> = Box<dyn Fn(&mut V, &mut Context<V>) + 'static>;

/// A label wrapped onto as many lines as it needs, the way the Bookmarks
/// pane draws a long title (`white-space: pre-wrap` on its
/// `tree-item-inner`, 2026-11-07 probe) where the other panes cut it.
pub fn wrapped_label(text: impl Into<SharedString>) -> gpui::AnyElement {
    div().child(text.into()).into_any_element()
}

/// A pane's own header button: Obsidian's `nav-action-button`, 28 by
/// 24 with a 4 px radius, `--icon-color` at `--icon-opacity` until the
/// pointer is over it; an `is-active` one sits on the hover wash in
/// `--icon-color-focused` (probe of 2026-11-02, `clickable-icon.active`).
pub fn nav_button(
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

/// Obsidian's `nav-header`: 40 px, 8 px of padding, the buttons
/// centred 2 px apart.
pub fn header() -> gpui::Div {
    div()
        .flex_shrink_0()
        .h(px(HEADER_HEIGHT))
        .p(px(8.))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(2.))
}

/// Obsidian's `tree-item-self`: 25 px tall at a 27 px pitch,
/// `padding: 4px 8px 4px 24px` with 17 px more on the left per level of
/// depth, an 8 px radius, 13 px muted text that brightens under the
/// pointer (or when `active`, the Outline's `is-active` mark), a faint
/// flair at the right edge, a fold triangle or an icon in the gutter
/// 20 px before the text, and an indentation guide for every level.
/// A `wrap` label wraps at 17 px per line; the others are cut with an
/// ellipsis.
#[allow(clippy::too_many_arguments)]
pub fn row<V: 'static>(
    id: (&'static str, usize),
    depth: usize,
    fold: Option<(bool, FoldAction<V>)>,
    icon_name: Option<&'static str>,
    label: gpui::AnyElement,
    flair: Option<SharedString>,
    active: bool,
    ring: bool,
    theme: &Theme,
    cx: &mut Context<V>,
) -> gpui::Stateful<gpui::Div> {
    let text_x = 24. + 17. * depth as f32;
    let gutter = match (fold, icon_name) {
        (Some((collapsed, on_fold)), _) => Some(
            div()
                .id((id.0, id.1 + 1_000_000))
                .absolute()
                .left(px(text_x - 20.))
                .top(px(4.))
                .w(px(16.))
                .h(px(17.))
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.85)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    on_fold(this, cx);
                    cx.stop_propagation();
                }))
                .child(
                    div()
                        .text_color(theme.tokens.collapse_icon_color)
                        .group_hover("tree-row", |s| s.text_color(theme.tokens.icon_color_hover))
                        .child(icon(
                            if collapsed {
                                "chevron-right"
                            } else {
                                "chevron-down"
                            },
                            10.,
                            theme.tokens.collapse_icon_color,
                        )),
                ),
        ),
        (None, Some(name)) => Some(
            div()
                .id((id.0, id.1 + 1_000_000))
                .absolute()
                .left(px(text_x - 20.))
                .top(px(4.))
                .w(px(16.))
                .h(px(17.))
                .flex()
                .items_center()
                .justify_center()
                .opacity(theme.tokens.icon_opacity)
                .child(icon(name, 14., theme.tokens.icon_color)),
        ),
        (None, None) => None,
    };
    div()
        .id(id)
        .group("tree-row")
        .relative()
        .w_full()
        .flex_shrink_0()
        .min_h(px(25.))
        .mb(px(2.))
        .pl(px(text_x))
        .pr(px(8.))
        .py(px(4.))
        .flex()
        .flex_row()
        .items_start()
        .rounded(px(8.))
        .cursor_pointer()
        // A `tree-item-self`: the nav item's colours (probe of
        // 2026-11-02, `pane.tags.*`, `pane.outline.*`).
        .when(active, |d| d.bg(theme.tokens.nav_item_background_active))
        .hover(|s| s.bg(theme.tokens.nav_item_background_hover))
        // The keyboard's row in the pane that has the keyboard:
        // `box-shadow: 0 0 0 2px var(--background-modifier-border-focus)`,
        // a ring outside the box, so nothing moves (2026-11-04 probe).
        // Drawn as a 2 px border 2 px out, with the radius grown to
        // match, since a gpui shadow fills its whole box.
        .when(ring, |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .left(px(-2.))
                    .right(px(-2.))
                    .bottom(px(-2.))
                    .border_2()
                    .border_color(theme.tokens.background_modifier_border_focus)
                    .rounded(px(10.)),
            )
        })
        // Obsidian's `tree-item-children` guides: a hairline 12 px in
        // from each ancestor's text, the row's full pitch tall.
        .children((1..=depth).map(|k| {
            div()
                .absolute()
                .left(px(17. * k as f32 - 5.))
                .top(px(0.))
                .bottom(px(-2.))
                .w(px(1.))
                .bg(theme.tokens.nav_indentation_guide_color)
        }))
        .children(gutter)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(FONT_UI_SMALL))
                .line_height(px(17.))
                .text_color(if active {
                    theme.tokens.nav_item_color_active
                } else {
                    theme.tokens.nav_item_color
                })
                .group_hover("tree-row", |s| {
                    s.text_color(theme.tokens.nav_item_color_hover)
                })
                .child(label),
        )
        .children(flair.map(|count| {
            div()
                .flex_shrink_0()
                .pl(px(4.))
                .pt(px(3.))
                .text_size(px(FONT_UI_SMALLER))
                .line_height(px(12.))
                .text_color(theme.tokens.text_faint)
                .group_hover("tree-row", |s| s.text_color(theme.tokens.text_muted))
                .child(count)
        }))
}

/// A one-line label cut with an ellipsis.
pub fn label(text: impl Into<SharedString>) -> gpui::AnyElement {
    div()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(text.into())
        .into_any_element()
}

/// A pane's filter field: Obsidian's `search-input-container`, its text
/// 30 px in to leave room for the magnifier drawn over it.
pub fn filter_input(cx: &mut Context<TextInput>) -> TextInput {
    let mut input = TextInput::new("Search...", cx);
    input.set_pad_left(px(30.));
    input
}

/// The filter under a pane's header: the field, the magnifier over its
/// left end, and "Clear search" at its right while it holds text.
pub fn filter_field<V: 'static>(
    clear_id: (&'static str, usize),
    input: Entity<TextInput>,
    theme: &Theme,
    cx: &mut Context<V>,
    on_clear: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> gpui::Div {
    let has_text = !input.read(cx).text().is_empty();
    div().flex_shrink_0().px(px(8.)).pb(px(8.)).child(
        div()
            .relative()
            .w_full()
            .h(px(30.))
            .child(input)
            .child(
                div()
                    .absolute()
                    .left(px(8.))
                    .top(px(0.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .child(icon("search", 14., theme.tokens.text_muted)),
            )
            .children(has_text.then(|| {
                div()
                    .id(clear_id)
                    .absolute()
                    .right(px(2.))
                    .top(px(0.))
                    .w(px(28.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .rounded_full()
                    .tooltip(tooltip("Clear search"))
                    .on_click(cx.listener(move |this, _, window, cx| on_clear(this, window, cx)))
                    .child(icon("x", 14., theme.tokens.text_muted))
            })),
    )
}
