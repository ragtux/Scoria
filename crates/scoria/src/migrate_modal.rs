//! The Format converter's "Frontmatter migration" modal
//! (`app:migrate-frontmatter`), as Obsidian 1.15.0 drew it for the
//! 2026-11-09 probe ([[Specification#Format converter: Frontmatter migration]]):
//! the title, a close button, the two paragraphs and Cancel / Start
//! conversion; then, once the run is over, "Finished!", the three counters
//! (Failed hidden at 0) and Go back / Done. While a run is going Stop is
//! shown in place of the other two: the probe's 28 notes were finished
//! before the first look, so that state's look is Scoria's.
//!
//! The conversion itself is [`scoria_formats::migrate`]; the files are
//! written by the workspace, which hands the counts back here.

use crate::theme::{Theme, FONT_UI_SMALL};
use gpui::{
    actions, div, prelude::*, px, App, Context, EventEmitter, FocusHandle, Focusable, FontWeight,
    KeyBinding, Window,
};
use serde_json::{json, Value};

actions!(migrate_frontmatter, [Dismiss]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "escape",
        Dismiss,
        Some("MigrateFrontmatter"),
    )]);
}

pub const TITLE: &str = "Frontmatter migration";
pub const INTRO: &str = "Migrate frontmatter tags, aliases, and cssclasses to the strict format required by Obsidian v1.9+";
pub const WARNING: &str = "Warning: the converter will convert all the files in your vault, not just the current file. Your files will be overwritten. Back up all your files before attempting conversion.";

/// The run's counters, as the modal shows them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Every Markdown file looked at.
    pub processed: usize,
    /// The files written.
    pub modified: usize,
    /// The files whose block would not parse, or that could not be read
    /// or written.
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The two paragraphs and the buttons to start.
    Start,
    /// A run going: the counters so far and Stop.
    Running(Counts),
    /// "Finished!" and the counters.
    Finished(Counts),
}

pub enum MigrateEvent {
    /// Start conversion.
    Start,
    /// Stop, during a run.
    Stop,
    /// Done, after a run: the notice and its Undo follow.
    Done,
    /// Cancel, the close button or Escape.
    Close,
}

pub struct MigrateModal {
    focus_handle: FocusHandle,
    pub stage: Stage,
}

impl EventEmitter<MigrateEvent> for MigrateModal {}

impl Focusable for MigrateModal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl MigrateModal {
    pub fn new(cx: &mut Context<Self>) -> MigrateModal {
        MigrateModal {
            focus_handle: cx.focus_handle(),
            stage: Stage::Start,
        }
    }

    pub fn set_stage(&mut self, stage: Stage, cx: &mut Context<Self>) {
        self.stage = stage;
        cx.notify();
    }

    pub fn describe(&self) -> Value {
        match self.stage {
            Stage::Start => json!({
                "title": TITLE,
                "text": [INTRO, WARNING],
                "buttons": ["Cancel", "Start conversion"],
            }),
            Stage::Running(c) | Stage::Finished(c) => {
                let finished = matches!(self.stage, Stage::Finished(_));
                let mut counters = vec![
                    json!({"label": "Processed files", "value": c.processed}),
                    json!({"label": "Modified files", "value": c.modified}),
                ];
                if c.failed > 0 {
                    counters.push(json!({"label": "Failed", "value": c.failed}));
                }
                json!({
                    "title": TITLE,
                    "text": if finished { vec!["Finished!"] } else { vec![] },
                    "counters": counters,
                    "buttons": if finished { vec!["Go back", "Done"] } else { vec!["Stop"] },
                })
            }
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        // A run in progress is stopped first; Escape after it closes.
        match self.stage {
            Stage::Running(_) => cx.emit(MigrateEvent::Stop),
            Stage::Finished(_) => cx.emit(MigrateEvent::Done),
            Stage::Start => cx.emit(MigrateEvent::Close),
        }
    }
}

impl Render for MigrateModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let t = &theme.tokens;
        // `mod-cta` accented, `mod-warning` red, the rest plain.
        let button = |id: &'static str, label: &'static str, kind: u8| {
            div()
                .id(id)
                .h(px(30.))
                .px(px(14.))
                .flex()
                .items_center()
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(FONT_UI_SMALL))
                .map(|d| match kind {
                    1 => d.bg(t.interactive_accent).text_color(t.text_on_accent),
                    2 => d
                        .bg(t.background_modifier_error)
                        .text_color(t.text_on_accent),
                    _ => d.bg(t.background_modifier_hover).text_color(t.text_normal),
                })
                .child(label)
        };
        let counter = |label: &'static str, value: usize, color: gpui::Hsla| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .py(px(4.))
                .text_size(px(FONT_UI_SMALL))
                .child(
                    div()
                        .min_w(px(36.))
                        .px(px(6.))
                        .rounded(px(4.))
                        .bg(color)
                        .text_color(t.text_on_accent)
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex()
                        .justify_center()
                        .child(value.to_string()),
                )
                .child(div().text_color(t.text_normal).child(label))
        };
        let buttons = div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(8.))
            .pt(px(12.));
        let content = match self.stage {
            Stage::Start => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .text_size(px(FONT_UI_SMALL))
                .child(div().text_color(t.text_muted).child(INTRO))
                .child(div().text_color(t.text_error).child(WARNING))
                .child(
                    buttons
                        .child(
                            button("migrate-cancel", "Cancel", 0)
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(MigrateEvent::Close))),
                        )
                        .child(
                            button("migrate-start", "Start conversion", 1)
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(MigrateEvent::Start))),
                        ),
                ),
            Stage::Running(c) | Stage::Finished(c) => {
                let finished = matches!(self.stage, Stage::Finished(_));
                div()
                    .flex()
                    .flex_col()
                    .when(finished, |d| {
                        d.child(
                            div()
                                .flex()
                                .justify_center()
                                .pb(px(8.))
                                .text_color(t.text_normal)
                                .child("Finished!"),
                        )
                    })
                    .child(counter("Processed files", c.processed, t.color_green))
                    .child(counter("Modified files", c.modified, t.interactive_accent))
                    .when(c.failed > 0, |d| {
                        d.child(counter("Failed", c.failed, t.color_red))
                    })
                    .child(if finished {
                        buttons
                            .child(button("migrate-back", "Go back", 0).on_click(
                                cx.listener(|this, _, _, cx| this.set_stage(Stage::Start, cx)),
                            ))
                            .child(
                                button("migrate-done", "Done", 1).on_click(
                                    cx.listener(|_, _, _, cx| cx.emit(MigrateEvent::Done)),
                                ),
                            )
                    } else {
                        buttons.child(
                            button("migrate-stop", "Stop", 2)
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(MigrateEvent::Stop))),
                        )
                    })
            }
        };
        div()
            .id("migrate-frontmatter")
            .key_context("MigrateFrontmatter")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::dismiss))
            .w(px(480.))
            .flex()
            .flex_col()
            .bg(t.background_secondary)
            .border_1()
            .border_color(t.background_modifier_border)
            .rounded_lg()
            .shadow_lg()
            .p_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .pb(px(12.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text_normal)
                            .child(TITLE),
                    )
                    .child(
                        div()
                            .id("migrate-close")
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(t.background_modifier_hover))
                            .on_click(cx.listener(|this, _, _, cx| match this.stage {
                                Stage::Running(_) => cx.emit(MigrateEvent::Stop),
                                Stage::Finished(_) => cx.emit(MigrateEvent::Done),
                                Stage::Start => cx.emit(MigrateEvent::Close),
                            }))
                            .child(crate::assets::icon("x", 16., t.text_muted)),
                    ),
            )
            .child(content)
    }
}
