//! The autocompletion popup: the rows the editor is offering, and where
//! they are drawn.
//!
//! What triggers it, what it offers and what accepting a row writes all
//! live in `scoria-suggest`, free of gpui and tested against Obsidian's own
//! rows. This module is the window onto that: a list anchored under the
//! cursor, the selected row, and the mouse.
//!
//! The slash popup (the Slash commands core plugin) is the same popup over
//! command rows, drawn to the measurements of Obsidian 1.15.0's: rows of
//! 31.5 px at 15 px text with their hotkeys as pills, a box at most 500 px
//! wide and 300 px tall, 5 px under the line, and a selection that wraps.

use crate::editor::Editor;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, Context, FontWeight, HighlightStyle, Pixels, SharedString, StyledText,
};
use scoria_suggest::{Accept, Item, Suggestion};
use std::ops::Range;

/// Height of one row.
const ROW: f32 = 26.0;
/// Rows drawn before the list starts scrolling with the selection; also
/// what the page keys move the selection by.
pub(crate) const VISIBLE: usize = 12;
const WIDTH: f32 = 380.0;

/// The slash popup's row: 31.5 px and a 1 px gap (Obsidian 1.15.0's
/// `.suggestion-item`, `padding: 6px 12px`, `margin-bottom: 1px`).
const SLASH_ROW: f32 = 32.5;
/// `.suggestion-container`: `max-height: 300px`, `max-width: 500px`, the
/// list padded 6 px, 5 px under the caret's line.
const SLASH_MAX_HEIGHT: f32 = 300.0;
const SLASH_MAX_WIDTH: f32 = 500.0;
const SLASH_PAD: f32 = 6.0;
const SLASH_GAP: f32 = 5.0;
/// Rows the slash box shows (the ninth cut by its edge), and how far
/// PageDown moved Obsidian's selection: 0 to 8.
const SLASH_VISIBLE: usize = 9;
const SLASH_PAGE: usize = 8;

/// The popup as the editor holds it.
pub struct SuggestState {
    /// The line the query is on, and the byte columns accepting it replaces.
    pub line: usize,
    pub range: Range<usize>,
    pub items: Vec<Suggestion>,
    pub selected: usize,
    /// A slash popup: command rows, whose selection wraps at either end.
    pub slash: bool,
}

impl SuggestState {
    /// Moves the selection by `delta`, clamped rather than wrapped, the way
    /// Obsidian's list moves.
    pub fn step(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() - 1;
        let next = self.selected as isize + delta;
        // Obsidian's slash list wraps a single step past either end (Up on
        // the first row selected the hundredth); a page stops at the end.
        self.selected = if self.slash && delta.abs() == 1 && !(0..=last as isize).contains(&next) {
            if next < 0 {
                last
            } else {
                0
            }
        } else {
            next.clamp(0, last as isize) as usize
        };
    }

    /// How far the page keys move the selection.
    pub fn page(&self) -> usize {
        if self.slash {
            SLASH_PAGE
        } else {
            VISIBLE
        }
    }

    fn visible(&self) -> usize {
        if self.slash {
            SLASH_VISIBLE
        } else {
            VISIBLE
        }
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        self.selected = self.items.len().saturating_sub(1);
    }

    /// The window of rows drawn, which follows the selection.
    fn window(&self) -> Range<usize> {
        let visible = self.visible();
        if self.items.len() <= visible {
            return 0..self.items.len();
        }
        let half = visible / 2;
        let start = self
            .selected
            .saturating_sub(half)
            .min(self.items.len() - visible);
        start..start + visible
    }

    fn height(&self) -> Pixels {
        px(self.window().len() as f32 * ROW + 8.)
    }
}

/// Where the popup sits: under the cursor, or above it when there is no
/// room below.
pub struct Anchor {
    /// The cursor's left edge, relative to the editor's own bounds.
    pub x: Pixels,
    /// The top of the cursor's line.
    pub top: Pixels,
    /// The bottom of the cursor's line.
    pub bottom: Pixels,
    /// The editor's own size, for keeping the popup inside it.
    pub width: Pixels,
    pub height: Pixels,
}

/// Draws the popup for `state` at `anchor`.
pub fn render(
    state: &SuggestState,
    anchor: &Anchor,
    theme: &Theme,
    cx: &mut Context<Editor>,
) -> gpui::AnyElement {
    if state.slash {
        return render_slash(state, anchor, theme, cx).into_any_element();
    }
    let height = state.height();
    // Below the line unless it would fall out of the view.
    let below = anchor.bottom + height <= anchor.height;
    let top = if below {
        anchor.bottom
    } else {
        (anchor.top - height).max(px(0.))
    };
    let left = anchor.x.min((anchor.width - px(WIDTH)).max(px(0.)));
    let window = state.window();
    let rows = window.map(|ix| {
        let item = &state.items[ix];
        let selected = ix == state.selected;
        let highlights: Vec<(Range<usize>, HighlightStyle)> = item
            .ranges
            .iter()
            .map(|r| {
                (
                    r.clone(),
                    HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..HighlightStyle::default()
                    },
                )
            })
            .collect();
        let label =
            StyledText::new(SharedString::from(item.label.clone())).with_highlights(highlights);
        let muted = matches!(item.item, Item::Literal);
        div()
            .id(("suggest-row", ix))
            .w_full()
            .h(px(ROW))
            .px_2()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded_sm()
            .cursor_pointer()
            .when(selected, |d| d.bg(theme.tokens.background_modifier_hover))
            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.accept_suggestion_at(ix, Accept::Whole, window, cx);
                }),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(if muted {
                        theme.tokens.text_muted
                    } else {
                        theme.tokens.text_normal
                    })
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(label),
            )
            .children(item.detail.clone().map(|d| {
                div()
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(theme.tokens.text_muted)
                    .child(SharedString::from(d))
            }))
    });
    div()
        .absolute()
        .left(left)
        .top(top)
        .w(px(WIDTH))
        .flex()
        .flex_col()
        .p_1()
        .bg(theme.tokens.suggestion_background)
        .border_1()
        .border_color(theme.tokens.background_modifier_border)
        .rounded_md()
        .shadow_lg()
        .children(rows)
        .into_any_element()
}

/// A rough width for a slash row, since Obsidian's box is as wide as its
/// widest row up to 500 px: the label at 15 px and each hotkey pill at
/// 12 px, measured off the probe's screenshots (`Add file property` with
/// `Ctrl + ;` is 436 px wide in a 1600 px window).
fn slash_row_width(item: &Suggestion) -> f32 {
    let label = item.label.chars().count() as f32 * 7.1;
    let hotkeys = match &item.item {
        Item::Command { hotkeys, .. } => hotkeys
            .iter()
            .map(|h| h.chars().count() as f32 * 6.2 + 12.0 + 6.0)
            .sum::<f32>(),
        _ => 0.0,
    };
    label + if hotkeys > 0.0 { hotkeys + 16.0 } else { 0.0 }
}

/// The slash popup: command names, their matched characters bold, and
/// each command's hotkeys as pills at the right.
fn render_slash(
    state: &SuggestState,
    anchor: &Anchor,
    theme: &Theme,
    cx: &mut Context<Editor>,
) -> impl IntoElement {
    let window = state.window();
    let rows_height = state.items.len() as f32 * SLASH_ROW + 2.0 * SLASH_PAD;
    let height = px(rows_height.min(SLASH_MAX_HEIGHT) + 2.0);
    let widest = state
        .items
        .iter()
        .map(slash_row_width)
        .fold(0.0_f32, f32::max);
    // The rows' padding, the list's and a scrollbar's room.
    let width = px((widest + 24.0 + 2.0 * SLASH_PAD + 14.0).min(SLASH_MAX_WIDTH));
    let below = anchor.bottom + px(SLASH_GAP) + height <= anchor.height;
    let top = if below {
        anchor.bottom + px(SLASH_GAP)
    } else {
        (anchor.top - px(SLASH_GAP) - height).max(px(0.))
    };
    let left = anchor.x.min((anchor.width - width).max(px(0.)));
    let rows = window.map(|ix| {
        let item = &state.items[ix];
        let selected = ix == state.selected;
        let highlights: Vec<(Range<usize>, HighlightStyle)> = item
            .ranges
            .iter()
            .map(|r| {
                (
                    r.clone(),
                    HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..HighlightStyle::default()
                    },
                )
            })
            .collect();
        let label =
            StyledText::new(SharedString::from(item.label.clone())).with_highlights(highlights);
        let hotkeys = match &item.item {
            Item::Command { hotkeys, .. } => hotkeys.clone(),
            _ => Vec::new(),
        };
        div()
            .id(("slash-row", ix))
            .w_full()
            .h(px(SLASH_ROW - 1.0))
            .mb(px(1.))
            .px(px(12.))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_4()
            .rounded(px(4.))
            .cursor_pointer()
            .when(selected, |d| d.bg(theme.tokens.background_modifier_hover))
            .hover(|s| s.bg(theme.tokens.background_modifier_hover))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.accept_suggestion_at(ix, Accept::Whole, window, cx);
                }),
            )
            .child(
                div()
                    .min_w_0()
                    .text_size(px(15.))
                    .text_color(theme.tokens.text_normal)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_shrink_0()
                    .gap(px(6.))
                    .children(hotkeys.into_iter().map(|h| {
                        // `kbd.suggestion-hotkey`: 12 px, `2px 6px`, a
                        // 24 px radius, on `--background-modifier-hover`.
                        div()
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(24.))
                            .bg(theme.tokens.background_modifier_hover)
                            .text_size(px(12.))
                            .text_color(theme.tokens.text_normal)
                            .whitespace_nowrap()
                            .child(SharedString::from(h))
                    })),
            )
    });
    div()
        .id("slash-popup")
        .absolute()
        .left(left)
        .top(top)
        .w(width)
        .h(height)
        .overflow_hidden()
        .flex()
        .flex_col()
        .p(px(SLASH_PAD))
        .bg(theme.tokens.suggestion_background)
        .border_1()
        .border_color(theme.tokens.background_modifier_border)
        .rounded(px(8.))
        .shadow_lg()
        .children(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(slash: bool, n: usize) -> SuggestState {
        SuggestState {
            line: 0,
            range: 0..1,
            items: (0..n)
                .map(|i| Suggestion {
                    item: Item::Command {
                        id: format!("id{i}"),
                        hotkeys: Vec::new(),
                    },
                    label: format!("Row {i}"),
                    detail: None,
                    ranges: Vec::new(),
                })
                .collect(),
            selected: 0,
            slash,
        }
    }

    /// The slash-commands probe (2026-11-11, s06): Up on the first of a
    /// hundred rows selected the hundredth, Down there the first again,
    /// and PageDown moved from the first to the ninth.
    #[test]
    fn the_slash_list_wraps_and_pages_by_eight() {
        let mut s = state(true, 100);
        s.step(-1);
        assert_eq!(s.selected, 99);
        s.step(1);
        assert_eq!(s.selected, 0);
        s.step(s.page() as isize);
        assert_eq!(s.selected, 8);
        // A page does not wrap.
        s.selected = 98;
        s.step(s.page() as isize);
        assert_eq!(s.selected, 99);
        assert_eq!(s.window().len(), 9);
    }

    /// The link and tag popup keeps its clamped selection.
    #[test]
    fn the_link_list_clamps() {
        let mut s = state(false, 20);
        s.step(-1);
        assert_eq!(s.selected, 0);
        s.select_last();
        s.step(1);
        assert_eq!(s.selected, 19);
        assert_eq!(s.page(), VISIBLE);
    }
}
