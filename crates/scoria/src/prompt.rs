//! A small modal for file operations: a title, the lines under it, an
//! optional text field, an optional list of suggestions the field filters
//! (the folder picker of "Move file to..."), an error line, and a row of
//! buttons. Enter submits, Escape cancels; Up and Down walk the
//! suggestions.
//!
//! The button row is `[secondary] [Cancel] [confirm]` unless the spec names
//! its own: Obsidian's "Update links" modal has three buttons in its own
//! order and no Cancel at all, so `PromptSpec::buttons` lets a prompt state
//! the row it was observed to have.
//!
//! A spec with [`PromptSpec::rows`] is drawn as Obsidian's settings-style
//! modal instead (`modal mod-lg`, the bookmark modals of 2026-11-07): 560 px,
//! a close button, then one `setting-item` per row — a name at the left and
//! its control at the right, a rule between rows — where the text field is
//! one row's control, a value that cannot change is a disabled field, and a
//! choice is a dropdown.

use crate::text_input::{InputEvent, TextInput};
use crate::theme::Theme;
use gpui::{
    actions, anchored, deferred, div, prelude::*, px, uniform_list, App, Context, Entity,
    EventEmitter, FocusHandle, Focusable, FontWeight, HighlightStyle, KeyBinding, ScrollStrategy,
    SharedString, StyledText, Subscription, UniformListScrollHandle, Window,
};
use scoria_search::fuzzy_match;
use serde_json::{json, Value};
use std::ops::Range;

actions!(prompt, [Confirm, Dismiss]);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Prompt");
    cx.bind_keys([
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);
}

pub enum PromptEvent {
    /// Enter, or the primary button: the field's text, or the chosen
    /// suggestion when there is a list.
    Submit(String),
    /// The secondary button.
    Secondary,
    /// A third button beside those two: "Always update" on the rename
    /// prompt, which writes a setting before it updates the links.
    Tertiary,
    Cancel,
}

/// What a button in [`PromptSpec::buttons`] does when it is clicked -- the
/// same four things the default row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptAction {
    /// What Enter and the primary button do.
    Submit,
    Secondary,
    Tertiary,
    Cancel,
}

/// One button of a prompt's own row.
#[derive(Debug, Clone)]
pub struct PromptButton {
    pub label: SharedString,
    pub action: PromptAction,
    /// `button.mod-cta`: the accented one. Enter does what it does.
    pub cta: bool,
    /// `button.mod-destructive.mod-secondary`: the error colour on a tenth
    /// of itself, pushed to the row's left end ("Remove" in "Edit
    /// bookmark").
    pub danger: bool,
}

/// One `setting-item` of a settings-style prompt.
#[derive(Debug, Clone)]
pub enum PromptRow {
    /// The prompt's text field, as this row's control.
    Input { name: SharedString },
    /// A value shown in a disabled field ("Path" in "Edit bookmark").
    Fixed {
        name: SharedString,
        value: SharedString,
    },
    /// A dropdown; `selected` is an index into `options`, or none for a
    /// dropdown that shows nothing (an item at the top level, whose group
    /// is not among the options).
    Choice {
        name: SharedString,
        options: Vec<SharedString>,
        selected: Option<usize>,
    },
}

impl PromptRow {
    fn name(&self) -> &SharedString {
        match self {
            PromptRow::Input { name }
            | PromptRow::Fixed { name, .. }
            | PromptRow::Choice { name, .. } => name,
        }
    }
}

impl PromptButton {
    pub fn new(label: impl Into<SharedString>, action: PromptAction) -> PromptButton {
        PromptButton {
            label: label.into(),
            action,
            cta: false,
            danger: false,
        }
    }

    pub fn danger(mut self) -> PromptButton {
        self.danger = true;
        self
    }

    pub fn cta(mut self) -> PromptButton {
        self.cta = true;
        self
    }
}

/// What the prompt asks for.
#[derive(Debug, Clone, Default)]
pub struct PromptSpec {
    pub title: SharedString,
    /// The lines under the title, one element each.
    pub detail: Vec<SharedString>,
    /// `Some(initial text)` shows the text field.
    pub input: Option<String>,
    pub placeholder: SharedString,
    /// Candidates the field filters; Enter submits the selected one.
    pub suggestions: Vec<String>,
    pub confirm: SharedString,
    pub secondary: Option<SharedString>,
    /// The whole button row, in order, for a prompt whose row is not
    /// `[secondary] [Cancel] [confirm]`. Empty means that default row, and
    /// `confirm` and `secondary` are then what label it.
    pub buttons: Vec<PromptButton>,
    /// Settings-style rows; empty for the plain prompt.
    pub rows: Vec<PromptRow>,
}

pub struct Prompt {
    focus_handle: FocusHandle,
    input: Entity<TextInput>,
    spec: PromptSpec,
    error: Option<SharedString>,
    filtered: Vec<(usize, Vec<Range<usize>>)>,
    selected: usize,
    scroll: UniformListScrollHandle,
    /// The dropdown row's choice, and whether its list is open.
    choice: Option<usize>,
    choice_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PromptEvent> for Prompt {}

impl Focusable for Prompt {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.spec.input.is_some() {
            self.input.focus_handle(cx)
        } else {
            self.focus_handle.clone()
        }
    }
}

impl Prompt {
    pub fn new(cx: &mut Context<Self>) -> Prompt {
        let input = cx.new(|cx| TextInput::new("", cx));
        let subscriptions =
            vec![
                cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
                    InputEvent::Changed => this.refresh(cx),
                    InputEvent::Submit => this.submit(cx),
                    InputEvent::Cancel => cx.emit(PromptEvent::Cancel),
                    InputEvent::Up => this.select(this.selected.saturating_sub(1), cx),
                    InputEvent::Down => this.select(this.selected + 1, cx),
                }),
            ];
        Prompt {
            focus_handle: cx.focus_handle(),
            input,
            spec: PromptSpec::default(),
            error: None,
            filtered: Vec::new(),
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            choice: None,
            choice_open: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn open(&mut self, spec: PromptSpec, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let initial = spec.input.clone().unwrap_or_default();
        self.choice = spec.rows.iter().find_map(|r| match r {
            PromptRow::Choice { selected, .. } => *selected,
            _ => None,
        });
        self.choice_open = false;
        self.spec = spec;
        self.input.update(cx, |i, cx| {
            i.set_placeholder(self.spec.placeholder.clone());
            let prefilled = !initial.is_empty();
            i.set_text(initial, cx);
            // Rename opens on the current name, and Obsidian's inline rename
            // selects the whole basename so the first keystroke replaces it.
            if prefilled {
                i.select_all(cx);
            }
        });
        window.focus(&self.focus_handle(cx), cx);
        self.refresh(cx);
    }

    /// What the dropdown row holds now, as an index into its options.
    pub fn choice(&self) -> Option<usize> {
        self.choice
    }

    /// Keeps the prompt open and shows why the operation failed.
    pub fn set_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.error = Some(message.into());
        cx.notify();
    }

    /// What the prompt is asking, for the control socket's state dump: the
    /// field's text, the suggestions it filtered down to, the error line a
    /// failed operation left on screen, and the buttons in the order they
    /// are painted -- which is the only way a driver can tell "Always
    /// update" from "Just once" without aiming at pixels.
    pub fn describe(&self, rows: usize, cx: &App) -> Value {
        let total = self.filtered.len();
        let shown: Vec<&str> = self
            .filtered
            .iter()
            .take(rows)
            .filter_map(|(ix, _)| self.spec.suggestions.get(*ix).map(String::as_str))
            .collect();
        let row = self.buttons();
        let buttons: Vec<&str> = row.iter().map(|b| b.label.as_ref()).collect();
        json!({
            "query": self.input.read(cx).text(),
            "selected": self.selected,
            "rows": shown,
            "rows_total": if total > shown.len() { Some(total) } else { None },
            "error": self.error,
            "buttons": buttons,
            "fields": self.spec.rows.iter().map(|r| r.name().to_string()).collect::<Vec<_>>(),
            "choice": self.choice,
        })
    }

    /// The button row: the spec's own, or the default
    /// `[secondary] [Cancel] [confirm]`.
    fn buttons(&self) -> Vec<PromptButton> {
        if !self.spec.buttons.is_empty() {
            return self.spec.buttons.clone();
        }
        let mut row = Vec::new();
        if let Some(label) = self.spec.secondary.clone() {
            row.push(PromptButton::new(label, PromptAction::Secondary));
        }
        row.push(PromptButton::new("Cancel", PromptAction::Cancel));
        row.push(PromptButton::new(self.spec.confirm.clone(), PromptAction::Submit).cta());
        row
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().trim().to_string();
        let mut scored: Vec<(f64, usize, Vec<Range<usize>>)> = Vec::new();
        for (ix, candidate) in self.spec.suggestions.iter().enumerate() {
            if query.is_empty() {
                scored.push((-(ix as f64), ix, Vec::new()));
            } else if let Some(m) = fuzzy_match(&query, candidate) {
                scored.push((m.score, ix, m.ranges));
            }
        }
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        self.filtered = scored.into_iter().map(|(_, ix, r)| (ix, r)).collect();
        self.selected = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = ix.min(self.filtered.len() - 1);
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Center);
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let value = match self.filtered.get(self.selected) {
            Some((ix, _)) if !self.spec.suggestions.is_empty() => {
                self.spec.suggestions[*ix].clone()
            }
            _ => self.input.read(cx).text().to_string(),
        };
        cx.emit(PromptEvent::Submit(value));
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        self.submit(cx);
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PromptEvent::Cancel);
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Stateful<gpui::Div>> {
        let theme = cx.global::<Theme>().clone();
        range
            .filter_map(|ix| {
                let (candidate_ix, ranges) = self.filtered.get(ix)?;
                let label = self.spec.suggestions.get(*candidate_ix)?.clone();
                let selected = ix == self.selected;
                let highlights: Vec<_> = ranges
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
                Some(
                    div()
                        .id(("prompt-row", ix))
                        .w_full()
                        .h(px(30.))
                        .px_3()
                        .flex()
                        .items_center()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_size(px(13.))
                        .when(selected, |d| d.bg(theme.tokens.background_modifier_hover))
                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = ix;
                            this.submit(cx);
                        }))
                        .child(
                            div()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(
                                    StyledText::new(SharedString::from(label))
                                        .with_highlights(highlights),
                                ),
                        ),
                )
            })
            .collect()
    }
}

impl Prompt {
    /// The rows of a settings-style prompt, each a name at the left and its
    /// control at the right, a rule above every row but the first
    /// (`setting-item`, 2026-11-07 `s11b`: 16 px of padding either side of
    /// each rule, the name at 13 px, every control 30 px tall).
    fn render_setting_rows(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let t = &theme.tokens;
        let last = self.spec.rows.len().saturating_sub(1);
        let rows = self.spec.rows.iter().enumerate().map(|(ix, row)| {
            let control = match row {
                // The two text controls are 348 px, as measured: the
                // modal's width is fixed, so the row's split is too.
                PromptRow::Input { .. } => div()
                    .w(px(348.))
                    .child(self.input.clone())
                    .into_any_element(),
                // A disabled `input`: the field's box, its border the plain
                // one, the value in the muted colour.
                PromptRow::Fixed { value, .. } => div()
                    .w(px(348.))
                    .h(px(30.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .bg(t.background_modifier_form_field)
                    .border_1()
                    .border_color(t.background_modifier_border)
                    .text_size(px(13.))
                    .text_color(t.text_muted)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(value.clone())
                    .into_any_element(),
                PromptRow::Choice { options, .. } => {
                    self.render_choice(options, theme, cx).into_any_element()
                }
            };
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(12.))
                .when(ix > 0, |d| {
                    d.pt(px(16.))
                        .border_t_1()
                        .border_color(t.background_modifier_border)
                })
                .when(ix < last, |d| d.pb(px(16.)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mr(px(4.))
                        .text_size(px(13.))
                        .line_height(px(17.))
                        .text_color(t.text_normal)
                        .child(row.name().clone()),
                )
                .child(div().flex().flex_row().justify_end().child(control))
        });
        div().flex().flex_col().children(rows)
    }

    /// Obsidian's `select.dropdown`: `--interactive-normal`, 30 px, the
    /// choice's text and the up-down chevrons at its right; a click lists
    /// the options under it.
    fn render_choice(
        &self,
        options: &[SharedString],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let t = &theme.tokens;
        let label = self
            .choice
            .and_then(|ix| options.get(ix))
            .map(|o| SharedString::from(o.trim_start_matches('\u{a0}').to_string()))
            .unwrap_or_default();
        let list = self.choice_open.then(|| {
            deferred(
                // Under the button, 2 px clear of it.
                anchored()
                    .position_mode(gpui::AnchoredPositionMode::Local)
                    .position(gpui::point(px(0.), px(2.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("prompt-choice-list")
                            .occlude()
                            .min_w(px(140.))
                            .py_1()
                            .flex()
                            .flex_col()
                            .bg(t.menu_background)
                            .border_1()
                            .border_color(t.menu_border_color)
                            .rounded_md()
                            .shadow_lg()
                            .text_size(px(13.))
                            .text_color(t.text_normal)
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.choice_open = false;
                                cx.notify();
                            }))
                            .children(options.iter().enumerate().map(|(ix, option)| {
                                div()
                                    .id(("prompt-choice", ix))
                                    .px_3()
                                    .py_1()
                                    .whitespace_nowrap()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(t.background_modifier_hover))
                                    .when(self.choice == Some(ix), |d| {
                                        d.bg(t.background_modifier_hover)
                                    })
                                    // The list is drawn deferred, outside
                                    // the prompt's box, so a pick hands the
                                    // keyboard back for Enter and Escape.
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.choice = Some(ix);
                                        this.choice_open = false;
                                        window.focus(&this.focus_handle(cx), cx);
                                        cx.notify();
                                    }))
                                    .child(option.clone())
                            })),
                    ),
            )
            .with_priority(2)
        });
        div()
            .child(
                div()
                    .id("prompt-choice-button")
                    .relative()
                    .min_w(px(140.))
                    .h(px(30.))
                    .pl(px(10.))
                    .pr(px(31.))
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .bg(t.interactive_normal)
                    .hover(|s| s.bg(t.interactive_hover))
                    .text_size(px(13.))
                    .text_color(t.text_normal)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.choice_open = !this.choice_open;
                        cx.notify();
                    }))
                    .child(label)
                    .child(
                        div()
                            .absolute()
                            .right(px(9.))
                            .top(px(0.))
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .child(crate::assets::icon("chevrons-up-down", 14., t.text_muted)),
                    ),
            )
            // After the button in the flow, so its origin is the button's
            // bottom-left corner.
            .children(list)
    }
}

impl Render for Prompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let count = self.filtered.len();
        let rows = (count.min(10) as f32) * 30.;
        // The id and the click listener belong on the painted button rather
        // than on a bare wrapper around it: gpui hit-tests the element that
        // registered the listener, and the wrapper's box is not the box the
        // user aims at.
        let settings = !self.spec.rows.is_empty();
        let button =
            |ix: usize, label: SharedString, primary: bool, danger: bool, theme: &Theme| {
                let mut wash = theme.tokens.text_error;
                wash.a = 0.1;
                let mut wash_hover = theme.tokens.text_error;
                wash_hover.a = 0.2;
                div()
                    .id(("prompt-button", ix))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_size(px(13.))
                    // `button.mod-cta`: `--text-on-accent` on `--interactive-accent`;
                    // a plain button `--text-normal` on `--interactive-normal`,
                    // `--interactive-hover` under the pointer.
                    .when(primary, |d| {
                        d.bg(theme.tokens.interactive_accent)
                            .text_color(theme.tokens.text_on_accent)
                            .hover(|s| s.bg(theme.tokens.interactive_accent_hover))
                    })
                    .when(danger, |d| {
                        d.bg(wash)
                            .text_color(theme.tokens.text_error)
                            .hover(|s| s.bg(wash_hover))
                    })
                    .when(!primary && !danger, |d| {
                        d.bg(theme.tokens.interactive_normal)
                            .text_color(theme.tokens.text_normal)
                            .hover(|s| s.bg(theme.tokens.interactive_hover))
                    })
                    .child(label)
            };
        div()
            .id("prompt")
            .key_context("Prompt")
            // gpui focuses a tracked handle on every mouse-down inside the
            // element, so this has to be the same handle `Focusable` hands
            // out: tracking the box's own handle while the field held the
            // keyboard meant a click anywhere in the prompt — on the field
            // itself, on a button — took the keyboard away from the field and
            // left the prompt typeable only after Escape and a fresh F2.
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .relative()
            .w(px(if settings { 560. } else { 480. }))
            .flex()
            .flex_col()
            .when(!settings, |d| d.gap_2())
            .bg(theme.tokens.prompt_background)
            .border_1()
            .border_color(theme.tokens.prompt_border_color)
            .rounded_lg()
            .shadow_lg()
            .p_4()
            // A press outside dismisses the prompt — unless the dropdown's list is
            // up, which hangs outside the box: that press is the list's.
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if !this.choice_open {
                    cx.emit(PromptEvent::Cancel)
                }
            }))
            .when(settings, |d| {
                // `modal-close-button`: 24 px, 7 px from the top, 13 from
                // the right.
                d.child(
                    div()
                        .id("prompt-close")
                        .absolute()
                        .top(px(7.))
                        .right(px(13.))
                        .w(px(24.))
                        .h(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PromptEvent::Cancel)))
                        .child(crate::assets::icon("x", 16., theme.tokens.text_muted)),
                )
            })
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.text_normal)
                    .when(settings, |d| d.mb(px(11.)))
                    .child(self.spec.title.clone()),
            )
            .when(settings, |d| d.child(self.render_setting_rows(&theme, cx)))
            .when(!settings && !self.spec.detail.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_size(px(13.))
                        .text_color(theme.tokens.text_muted)
                        .children(self.spec.detail.clone()),
                )
            })
            .when(!settings && self.spec.input.is_some(), |d| {
                d.child(self.input.clone())
            })
            .when(count > 0, |d| {
                d.child(
                    uniform_list("prompt-list", count, cx.processor(Self::render_rows))
                        .track_scroll(&self.scroll)
                        .h(px(rows)),
                )
            })
            .children(self.error.clone().map(|e| {
                div()
                    .text_size(px(12.))
                    .text_color(theme.tokens.text_error)
                    .child(e)
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .when(settings, |d| d.mt(px(22.)))
                    .when(!settings, |d| d.mt_1())
                    .children({
                        // A danger button stands alone at the left, the
                        // rest at the right.
                        let row = self.buttons();
                        let split = row.iter().filter(|b| b.danger).count();
                        let mut els: Vec<gpui::AnyElement> = Vec::new();
                        for (
                            ix,
                            PromptButton {
                                label,
                                action,
                                cta,
                                danger,
                            },
                        ) in row.into_iter().enumerate()
                        {
                            if split > 0 && ix == split {
                                els.push(div().flex_1().into_any_element());
                            }
                            els.push(
                                button(ix, label, cta, danger, &theme)
                                    .on_click(cx.listener(move |this, _, _, cx| match action {
                                        PromptAction::Submit => this.submit(cx),
                                        PromptAction::Secondary => cx.emit(PromptEvent::Secondary),
                                        PromptAction::Tertiary => cx.emit(PromptEvent::Tertiary),
                                        PromptAction::Cancel => cx.emit(PromptEvent::Cancel),
                                    }))
                                    .into_any_element(),
                            );
                        }
                        els
                    }),
            )
    }
}
