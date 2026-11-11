//! The Note composer's modal: the file prompt merging and extracting share,
//! the merge's confirmation, and the "Update links" question a merge asks
//! on the way to deleting its source.
//!
//! Obsidian 1.15.0's, as the 2026-11-09 probe read it: a `.prompt` whose
//! input says `Select file to merge into...`, recent notes while it is
//! empty (`No recent files found. Type to search...` with none), notes by
//! fuzzy name once typed (`No files found.`), and two instructions under
//! the list — `ctrl ↵` "to create new", then `shift ↵` "to merge at top"
//! for a merge or "to move to top" for an extraction. An extraction adds a
//! row for a name no note has, `Enter to create` at its right. The
//! confirmation is `Merge file`, its sentence, a `Don't ask again`
//! checkbox, and `Merge` (focused) before `Cancel`.
//!
//! The view only reports what was chosen; [`super::note_composer`] does
//! the work.

use crate::prompt::{Prompt, PromptEvent, PromptSpec};
use crate::text_input::{InputEvent, TextInput};
use crate::theme::Theme;
use gpui::{
    actions, div, prelude::*, px, uniform_list, App, Context, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, HighlightStyle, KeyBinding, ScrollStrategy, SharedString, StyledText,
    Subscription, UniformListScrollHandle, Window,
};
use scoria_composer::Placement;
use scoria_search::fuzzy_match;
use serde_json::{json, Value};
use std::ops::Range;

actions!(
    note_composer,
    [AtTop, CreateNew, ConfirmMerge, CancelMerge, ToggleDontAsk]
);

/// The rows a prompt never shows more of. Obsidian's limit went unread; the
/// quick switcher's is the one Scoria already draws.
const MAX_ROWS: usize = 50;

/// Shift+Enter and Ctrl+Enter in the prompt, Enter, Escape and Space in the
/// confirmation. The prompt's bindings sit in a context of their own above
/// the text field, which binds neither, so they win over the workspace's
/// hotkeys; plain Enter stays the field's Submit.
pub fn bind_keys(cx: &mut App) {
    let prompt = Some("NoteComposer");
    let confirm = Some("NoteComposerConfirm");
    cx.bind_keys([
        KeyBinding::new("shift-enter", AtTop, prompt),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-enter"
            } else {
                "ctrl-enter"
            },
            CreateNew,
            prompt,
        ),
        KeyBinding::new("enter", ConfirmMerge, confirm),
        KeyBinding::new("escape", CancelMerge, confirm),
        KeyBinding::new("space", ToggleDontAsk, confirm),
    ]);
}

/// Which command opened the prompt: the second instruction differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Merge,
    Extract,
}

/// One note the prompt can offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The vault path.
    pub path: String,
    /// What the row reads and the query matches: the path without `.md`
    /// (`Sub/s-folder`), as Obsidian's rows read.
    pub label: String,
}

/// What a row stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Note(String),
    /// An extraction's "Enter to create" row: the name typed.
    Create(String),
}

pub enum ComposerEvent {
    /// Enter or Shift+Enter on a row.
    Chosen(Choice, Placement),
    /// Ctrl+Enter with at least one row listed: a new note named from the
    /// query.
    CreateNew(String),
    /// The confirmation's Merge, with the checkbox as it stood.
    Confirmed {
        dont_ask: bool,
    },
    /// The Update links question: Always update, Just once, Do not update.
    Links(LinksAnswer),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinksAnswer {
    Always,
    Once,
    DoNot,
}

struct Match {
    choice: Choice,
    label: String,
    ranges: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Stage {
    Pick,
    Confirm { text: String, dont_ask: bool },
    Links,
}

pub struct ComposerPrompt {
    kind: Kind,
    stage: Stage,
    input: Entity<TextInput>,
    focus: FocusHandle,
    candidates: Vec<Candidate>,
    recent: Vec<Candidate>,
    matches: Vec<Match>,
    selected: usize,
    scroll: UniformListScrollHandle,
    links: Entity<Prompt>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ComposerEvent> for ComposerPrompt {}

impl Focusable for ComposerPrompt {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.stage {
            Stage::Pick => self.input.focus_handle(cx),
            Stage::Confirm { .. } => self.focus.clone(),
            Stage::Links => self.links.focus_handle(cx),
        }
    }
}

impl ComposerPrompt {
    pub fn new(cx: &mut Context<Self>) -> ComposerPrompt {
        let input = cx.new(|cx| TextInput::new(scoria_composer::PLACEHOLDER, cx));
        let links = cx.new(Prompt::new);
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Changed => this.refresh(cx),
                InputEvent::Submit => this.choose(Placement::Append, cx),
                InputEvent::Cancel => cx.emit(ComposerEvent::Close),
                InputEvent::Up => this.select(this.selected.saturating_sub(1), cx),
                InputEvent::Down => this.select(this.selected + 1, cx),
            }),
            cx.subscribe(&links, |_, _, event: &PromptEvent, cx| {
                cx.emit(ComposerEvent::Links(match event {
                    PromptEvent::Tertiary => LinksAnswer::Always,
                    PromptEvent::Submit(_) => LinksAnswer::Once,
                    // Escape is "Do not update", as on the rename prompt:
                    // by then the merge has been asked for.
                    PromptEvent::Secondary | PromptEvent::Cancel => LinksAnswer::DoNot,
                }))
            }),
        ];
        ComposerPrompt {
            kind: Kind::Merge,
            stage: Stage::Pick,
            input,
            focus: cx.focus_handle(),
            candidates: Vec::new(),
            recent: Vec::new(),
            matches: Vec::new(),
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            links,
            _subscriptions: subscriptions,
        }
    }

    /// Shows the prompt over `candidates`, `recent` first while the query is
    /// empty, the field holding `query` (an extraction of a heading offers
    /// the heading's text).
    pub fn open(
        &mut self,
        kind: Kind,
        candidates: Vec<Candidate>,
        recent: Vec<Candidate>,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        self.stage = Stage::Pick;
        self.candidates = candidates;
        self.recent = recent;
        self.input.update(cx, |i, cx| {
            i.set_placeholder(scoria_composer::PLACEHOLDER);
            i.set_text(query, cx);
            if !query.is_empty() {
                i.select_all(cx);
            }
        });
        window.focus(&self.input.focus_handle(cx), cx);
        self.refresh(cx);
    }

    /// The confirmation, in place of the prompt.
    pub fn confirm(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.stage = Stage::Confirm {
            text,
            dont_ask: false,
        };
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// The Update links question, in place of the confirmation.
    pub fn ask_links(&mut self, spec: PromptSpec, window: &mut Window, cx: &mut Context<Self>) {
        self.stage = Stage::Links;
        self.links.update(cx, |p, cx| p.open(spec, window, cx));
        cx.notify();
    }

    pub fn describe(&self, rows: usize, cx: &App) -> Value {
        let shown: Vec<String> = self
            .matches
            .iter()
            .take(rows)
            .map(|m| match &m.choice {
                Choice::Create(_) => format!("{} ({})", m.label, scoria_composer::ENTER_TO_CREATE),
                Choice::Note(_) => m.label.clone(),
            })
            .collect();
        let stage = match &self.stage {
            Stage::Pick => json!("pick"),
            Stage::Confirm { text, dont_ask } => json!({"confirm": text, "dont_ask": dont_ask}),
            Stage::Links => json!("links"),
        };
        json!({
            "query": self.input.read(cx).text(),
            "selected": self.selected,
            "rows": shown,
            "stage": stage,
            "error": if self.matches.is_empty() { Some(self.empty_text(cx)) } else { None },
        })
    }

    fn empty_text(&self, cx: &App) -> &'static str {
        if self.input.read(cx).text().trim().is_empty() {
            scoria_composer::NO_RECENT
        } else {
            scoria_composer::NO_FILES
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().trim().to_string();
        self.matches = if query.is_empty() {
            self.recent
                .iter()
                .take(MAX_ROWS)
                .map(|c| Match {
                    choice: Choice::Note(c.path.clone()),
                    label: c.label.clone(),
                    ranges: Vec::new(),
                })
                .collect()
        } else {
            let mut scored: Vec<(f64, Match)> = self
                .candidates
                .iter()
                .filter_map(|c| {
                    let m = fuzzy_match(&query, &c.label)?;
                    Some((
                        m.score,
                        Match {
                            choice: Choice::Note(c.path.clone()),
                            label: c.label.clone(),
                            ranges: m.ranges,
                        },
                    ))
                })
                .collect();
            scored.sort_by(|a, b| {
                b.0.partial_cmp(&a.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.1.label.cmp(&b.1.label))
            });
            let mut matches: Vec<Match> =
                scored.into_iter().map(|(_, m)| m).take(MAX_ROWS).collect();
            // An extraction offers to create a note no note's name already
            // is; the matches come first and Enter takes the first of them
            // (`Top` picked `h-top`).
            let taken = self.candidates.iter().any(|c| {
                c.label.eq_ignore_ascii_case(&query)
                    || scoria_cache::basename(&c.path).eq_ignore_ascii_case(&query)
            });
            if self.kind == Kind::Extract && !taken {
                matches.push(Match {
                    choice: Choice::Create(query.clone()),
                    label: query,
                    ranges: Vec::new(),
                });
            }
            matches
        };
        self.selected = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.matches.is_empty() {
            return;
        }
        self.selected = ix.min(self.matches.len() - 1);
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Center);
        cx.notify();
    }

    fn choose(&mut self, place: Placement, cx: &mut Context<Self>) {
        // Enter on nothing listed does nothing, and the prompt stays.
        if let Some(m) = self.matches.get(self.selected) {
            cx.emit(ComposerEvent::Chosen(m.choice.clone(), place));
        }
    }

    /// Ctrl+Enter makes a note named from the query, but only while some
    /// row is listed: with "No files found." the key is taken and nothing
    /// happens (2026-11-09, s19c). Only the merge's was pressed; the
    /// extraction's instruction is drawn and the key left alone.
    fn create_new(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().trim().to_string();
        if self.kind == Kind::Merge && !query.is_empty() && !self.matches.is_empty() {
            cx.emit(ComposerEvent::CreateNew(query));
        }
    }

    fn at_top(&mut self, _: &AtTop, _: &mut Window, cx: &mut Context<Self>) {
        self.choose(Placement::Prepend, cx);
    }

    fn create_new_action(&mut self, _: &CreateNew, _: &mut Window, cx: &mut Context<Self>) {
        self.create_new(cx);
    }

    fn confirm_merge(&mut self, _: &ConfirmMerge, _: &mut Window, cx: &mut Context<Self>) {
        if let Stage::Confirm { dont_ask, .. } = self.stage {
            cx.emit(ComposerEvent::Confirmed { dont_ask });
        }
    }

    fn cancel_merge(&mut self, _: &CancelMerge, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ComposerEvent::Close);
    }

    fn toggle_dont_ask(&mut self, _: &ToggleDontAsk, _: &mut Window, cx: &mut Context<Self>) {
        if let Stage::Confirm { dont_ask, .. } = &mut self.stage {
            *dont_ask = !*dont_ask;
            cx.notify();
        }
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
                let m = self.matches.get(ix)?;
                let selected = ix == self.selected;
                let highlights: Vec<_> = m
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
                let label = StyledText::new(SharedString::from(m.label.clone()))
                    .with_highlights(highlights);
                let create = matches!(m.choice, Choice::Create(_));
                Some(
                    div()
                        .id(("composer-row", ix))
                        .w_full()
                        .h(px(34.))
                        .px_3()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .when(selected, |d| d.bg(theme.tokens.background_modifier_hover))
                        .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = ix;
                            this.choose(Placement::Append, cx);
                        }))
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(theme.tokens.text_normal)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(label),
                        )
                        .when(create, |d| {
                            d.child(
                                div()
                                    .flex_none()
                                    .text_size(px(12.))
                                    .text_color(theme.tokens.text_muted)
                                    .child(scoria_composer::ENTER_TO_CREATE),
                            )
                        }),
                )
            })
            .collect()
    }

    fn render_pick(&mut self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let count = self.matches.len();
        let rows = (count.min(12) as f32) * 34.;
        let instructions = match self.kind {
            Kind::Merge => scoria_composer::MERGE_INSTRUCTIONS,
            Kind::Extract => scoria_composer::EXTRACT_INSTRUCTIONS,
        };
        let empty = self.empty_text(cx);
        div()
            .id("note-composer")
            .key_context("NoteComposer")
            .on_action(cx.listener(Self::at_top))
            .on_action(cx.listener(Self::create_new_action))
            .w(px(620.))
            .flex()
            .flex_col()
            .bg(theme.tokens.prompt_background)
            .border_1()
            .border_color(theme.tokens.prompt_border_color)
            .rounded_lg()
            .shadow_lg()
            .p_2()
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(ComposerEvent::Close)))
            .child(self.input.clone())
            .when(count > 0, |d| {
                d.child(
                    uniform_list("composer-list", count, cx.processor(Self::render_rows))
                        .track_scroll(&self.scroll)
                        .h(px(rows))
                        .mt_2(),
                )
            })
            .when(count == 0, |d| {
                d.child(
                    div()
                        .p_3()
                        .text_size(px(12.))
                        .text_color(theme.tokens.text_muted)
                        .child(empty),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .px_3()
                    .pt_2()
                    .text_size(px(12.))
                    .children(instructions.iter().map(|(keys, text)| {
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .child(
                                div()
                                    .text_color(theme.tokens.text_muted)
                                    .font_weight(FontWeight::BOLD)
                                    .child(*keys),
                            )
                            .child(div().text_color(theme.tokens.text_muted).child(*text))
                    })),
            )
            .into_any_element()
    }

    fn render_confirm(
        &mut self,
        text: String,
        dont_ask: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let button = |id: &'static str, label: &'static str, cta: bool| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(13.))
                // `mod-destructive mod-cta`: the error colour as the accent.
                .when(cta, |d| {
                    d.bg(theme.tokens.text_error)
                        .text_color(theme.tokens.text_on_accent)
                })
                .when(!cta, |d| {
                    d.bg(theme.tokens.interactive_normal)
                        .text_color(theme.tokens.text_normal)
                        .hover(|s| s.bg(theme.tokens.interactive_hover))
                })
                .child(label)
        };
        div()
            .id("note-composer-confirm")
            .key_context("NoteComposerConfirm")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::confirm_merge))
            .on_action(cx.listener(Self::cancel_merge))
            .on_action(cx.listener(Self::toggle_dont_ask))
            .w(px(480.))
            .flex()
            .flex_col()
            .gap_2()
            .bg(theme.tokens.prompt_background)
            .border_1()
            .border_color(theme.tokens.prompt_border_color)
            .rounded_lg()
            .shadow_lg()
            .p_4()
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(ComposerEvent::Close)))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.text_normal)
                    .child(scoria_composer::CONFIRM_TITLE),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme.tokens.text_muted)
                    .child(text),
            )
            .child(
                div()
                    .id("note-composer-dont-ask")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(theme.tokens.text_normal)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_dont_ask(&ToggleDontAsk, window, cx)
                    }))
                    .child(
                        div()
                            .w(px(14.))
                            .h(px(14.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(theme.tokens.text_muted)
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(dont_ask, |d| {
                                d.bg(theme.tokens.interactive_accent)
                                    .child(crate::assets::icon(
                                        "check",
                                        12.,
                                        theme.tokens.text_on_accent,
                                    ))
                            }),
                    )
                    .child(scoria_composer::DONT_ASK_AGAIN),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .mt_1()
                    .child(
                        button("note-composer-merge", scoria_composer::MERGE, true).on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.confirm_merge(&ConfirmMerge, window, cx)
                            }),
                        ),
                    )
                    .child(
                        button("note-composer-cancel", scoria_composer::CANCEL, false)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ComposerEvent::Close))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ComposerPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        match self.stage.clone() {
            Stage::Pick => self.render_pick(&theme, cx),
            Stage::Confirm { text, dont_ask } => self.render_confirm(text, dont_ask, &theme, cx),
            Stage::Links => self.links.clone().into_any_element(),
        }
    }
}
