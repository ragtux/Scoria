//! File recovery's "Open local history" modal (`file-recovery:open`), laid
//! out as Obsidian 1.15.0 drew it for the 2026-11-09 probe
//! ([[Specification#Observed File recovery, Obsidian 1.15.0 (installer build 1.13.7)]]):
//! a sidebar with a search field ("Choose a file...", prefilled with the
//! active file's name) over the chosen file's snapshots, each its date over
//! its size; on the right the file's name, a "Show changes" toggle, Copy and
//! Restore, a close button, and the snapshot's text — or, with the toggle
//! on, its lines against the file as it is now. With no file chosen the
//! right side reads "Please select a file on the left to view snapshots."
//!
//! The snapshots come from Scoria's own store
//! ([`scoria_vault::recovery`]); the workspace writes a restore.

use crate::text_input::{InputEvent, TextInput};
use crate::theme::{Theme, FONT_UI_SMALL};
use gpui::{
    div, prelude::*, px, relative, App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, SharedString, Subscription, Window,
};
use scoria_vault::recovery::{diff_lines, DiffLine, Snapshot};
use serde_json::{json, Value};
use std::sync::Arc;

pub const NO_FILE: &str = "Please select a file on the left to view snapshots.";

/// A path's snapshots, newest first.
pub type ReadSnapshots = Arc<dyn Fn(&str) -> Vec<Snapshot>>;
/// A file's text now.
pub type ReadCurrent = Arc<dyn Fn(&str) -> Option<String>>;

/// The paths with snapshots, and how to read them.
pub struct Source {
    pub paths: Vec<String>,
    pub snapshots: ReadSnapshots,
    pub current: ReadCurrent,
}

pub enum RecoveryEvent {
    /// Restore: the snapshot to write over its file.
    Restore(Snapshot),
    /// Copy: the snapshot's text.
    Copy(String),
    Close,
}

pub struct RecoveryModal {
    input: Entity<TextInput>,
    source: Source,
    /// The file whose snapshots are listed.
    chosen: Option<String>,
    snapshots: Vec<Snapshot>,
    selected: Option<usize>,
    /// The files with snapshots that match the search, while none is chosen.
    suggestions: Vec<String>,
    show_changes: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RecoveryEvent> for RecoveryModal {}

impl Focusable for RecoveryModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

/// A file's name as the list and the title show it: the last segment,
/// without `.md`.
pub fn display_name(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

/// The row's date, moment's `llll`: "Tue, Nov 10, 2026 12:22 AM".
pub fn snapshot_date(ts: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(ts).single() {
        Some(t) => t.format("%a, %b %-d, %Y %-I:%M %p").to_string(),
        None => String::new(),
    }
}

/// The row's size: "13 B" as observed; larger ones in KB and MB of 1024.
pub fn snapshot_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// The files `query` picks out of `paths`: the one it names exactly (its
/// path, or its name without `.md`, ignoring case), else every one whose
/// path contains it.
pub fn match_files(paths: &[String], query: &str) -> (Option<String>, Vec<String>) {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (None, paths.to_vec());
    }
    if let Some(exact) = paths
        .iter()
        .find(|p| p.to_lowercase() == q || display_name(p).to_lowercase() == q)
    {
        return (Some(exact.clone()), Vec::new());
    }
    let hits = paths
        .iter()
        .filter(|p| p.to_lowercase().contains(&q))
        .cloned()
        .collect();
    (None, hits)
}

impl RecoveryModal {
    pub fn new(source: Source, query: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut i = TextInput::new("Choose a file...", cx);
            i.set_text(query, cx);
            i
        });
        let subscriptions =
            vec![
                cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
                    InputEvent::Changed => this.refresh(cx),
                    InputEvent::Submit => {
                        if this.chosen.is_none() {
                            if let Some(first) = this.suggestions.first().cloned() {
                                this.choose(first, cx);
                            }
                        }
                    }
                    InputEvent::Cancel => cx.emit(RecoveryEvent::Close),
                    InputEvent::Up => this.step(-1, cx),
                    InputEvent::Down => this.step(1, cx),
                }),
            ];
        window.focus(&input.focus_handle(cx), cx);
        let mut modal = RecoveryModal {
            input,
            source,
            chosen: None,
            snapshots: Vec::new(),
            selected: None,
            suggestions: Vec::new(),
            show_changes: false,
            _subscriptions: subscriptions,
        };
        modal.refresh(cx);
        modal
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().to_string();
        let (exact, hits) = match_files(&self.source.paths, &query);
        match exact {
            Some(path) => self.choose_quiet(path),
            None => {
                self.chosen = None;
                self.snapshots.clear();
                self.selected = None;
                self.suggestions = hits;
            }
        }
        cx.notify();
    }

    fn choose_quiet(&mut self, path: String) {
        if self.chosen.as_deref() == Some(path.as_str()) {
            return;
        }
        self.snapshots = (self.source.snapshots)(&path);
        self.selected = (!self.snapshots.is_empty()).then_some(0);
        self.suggestions.clear();
        self.chosen = Some(path);
    }

    fn choose(&mut self, path: String, cx: &mut Context<Self>) {
        let name = display_name(&path).to_string();
        self.choose_quiet(path);
        // The field shows the chosen file's name, which keeps it chosen.
        self.input.update(cx, |i, cx| i.set_text(name, cx));
        cx.notify();
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.snapshots.is_empty() {
            return;
        }
        let at = self.selected.unwrap_or(0) as isize + by;
        self.selected = Some(at.clamp(0, self.snapshots.len() as isize - 1) as usize);
        cx.notify();
    }

    fn current(&self) -> Option<&Snapshot> {
        self.selected.and_then(|i| self.snapshots.get(i))
    }

    pub fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.snapshots.len() {
            self.selected = Some(ix);
            cx.notify();
        }
    }

    pub fn toggle_changes(&mut self, cx: &mut Context<Self>) {
        self.show_changes = !self.show_changes;
        cx.notify();
    }

    pub fn describe(&self, cx: &App) -> Value {
        json!({
            "title": self.chosen.as_deref().map(display_name).unwrap_or("File recovery"),
            "query": self.input.read(cx).text(),
            "file": self.chosen,
            "files": self.suggestions,
            "snapshots": self.snapshots.iter().map(|s| json!({
                "date": snapshot_date(s.ts),
                "size": snapshot_size(s.data.len()),
            })).collect::<Vec<_>>(),
            "selected": self.selected,
            "text": self.current().map(|s| s.data.clone()),
            "show_changes": self.show_changes,
            "buttons": ["Copy", "Restore"],
        })
    }
}

impl Render for RecoveryModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let t = &theme.tokens;
        let mut list = div()
            .id("file-recovery-list")
            .flex()
            .flex_col()
            .gap(px(2.))
            .flex_1()
            .overflow_y_scroll();
        if self.chosen.is_some() {
            for (ix, s) in self.snapshots.iter().enumerate() {
                let on = self.selected == Some(ix);
                list = list.child(
                    div()
                        .id(("file-recovery-version", ix))
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(t.interactive_accent))
                        .when(!on, |d| d.hover(|s| s.bg(t.background_modifier_hover)))
                        .on_click(cx.listener(move |this, _, _, cx| this.select(ix, cx)))
                        .child(
                            div()
                                .text_size(px(FONT_UI_SMALL))
                                .text_color(if on { t.text_on_accent } else { t.text_normal })
                                .child(snapshot_date(s.ts)),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(if on { t.text_on_accent } else { t.text_muted })
                                .child(snapshot_size(s.data.len())),
                        ),
                );
            }
        } else {
            for (ix, path) in self.suggestions.iter().enumerate() {
                let path = path.clone();
                list = list.child(
                    div()
                        .id(("file-recovery-file", ix))
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.background_modifier_hover))
                        .text_size(px(FONT_UI_SMALL))
                        .text_color(t.text_normal)
                        .child(path.clone())
                        .on_click(cx.listener(move |this, _, _, cx| this.choose(path.clone(), cx))),
                );
            }
        }
        let sidebar = div()
            .w(px(250.))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(8.))
            .bg(t.background_secondary)
            .border_r_1()
            .border_color(t.background_modifier_border)
            .child(
                div()
                    .rounded(px(5.))
                    .border_1()
                    .border_color(t.background_modifier_border)
                    .bg(t.background_modifier_form_field)
                    .child(self.input.clone()),
            )
            .child(list);

        let title: SharedString = self
            .chosen
            .as_deref()
            .map(display_name)
            .unwrap_or("File recovery")
            .to_string()
            .into();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(FONT_UI_SMALL))
                .bg(t.interactive_normal)
                .hover(|s| s.bg(t.interactive_hover))
                .text_color(t.text_normal)
                .child(label)
        };
        let on = self.show_changes;
        let toggle = div()
            .id("file-recovery-show-changes")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.toggle_changes(cx)))
            .child(
                div()
                    .text_size(px(FONT_UI_SMALL))
                    .text_color(t.text_normal)
                    .child("Show changes"),
            )
            .child(
                div()
                    .w(px(34.))
                    .h(px(18.))
                    .rounded(px(9.))
                    .p(px(2.))
                    .flex()
                    .when(on, |d| d.justify_end().bg(t.interactive_accent))
                    .when(!on, |d| d.bg(t.background_modifier_border))
                    .child(div().size(px(14.)).rounded(px(7.)).bg(gpui::white())),
            );
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .px(px(16.))
            .h(px(46.))
            .border_b_1()
            .border_color(t.background_modifier_border)
            .child(
                div()
                    .flex_1()
                    .text_color(t.text_normal)
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(toggle)
            .child(
                button("file-recovery-copy", "Copy").on_click(cx.listener(|this, _, _, cx| {
                    if let Some(s) = this.current() {
                        cx.emit(RecoveryEvent::Copy(s.data.clone()));
                    }
                })),
            )
            .child(
                button("file-recovery-restore", "Restore").on_click(cx.listener(
                    |this, _, _, cx| {
                        if let Some(s) = this.current().cloned() {
                            cx.emit(RecoveryEvent::Restore(s));
                        }
                    },
                )),
            )
            .child(
                div()
                    .id("file-recovery-close")
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.background_modifier_hover))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(RecoveryEvent::Close)))
                    .child(crate::assets::icon("x", 16., t.text_muted)),
            );
        let body = div()
            .id("file-recovery-text")
            .flex_1()
            .overflow_y_scroll()
            .px(px(24.))
            .py(px(16.))
            .text_size(px(14.))
            .line_height(relative(1.5));
        let body = match self.current() {
            None => body.text_color(t.text_muted).child(NO_FILE),
            Some(s) if self.show_changes => {
                let now = self
                    .chosen
                    .as_deref()
                    .and_then(|p| (self.source.current)(p))
                    .unwrap_or_default();
                body.children(diff_lines(&s.data, &now).into_iter().map(|line| {
                    let (text, bg, color) = match line {
                        DiffLine::Same(l) => (l, None, t.text_normal),
                        DiffLine::Removed(l) => {
                            (l, Some(t.background_modifier_error), t.text_normal)
                        }
                        DiffLine::Added(l) => {
                            (l, Some(t.background_modifier_success), t.text_normal)
                        }
                    };
                    div()
                        .min_h(px(21.))
                        .px(px(4.))
                        .text_color(color)
                        .when_some(bg, |d, bg| d.bg(bg.opacity(0.35)))
                        .child(text)
                }))
            }
            Some(s) => body.text_color(t.text_normal).children(
                s.data
                    .split('\n')
                    .map(|l| div().min_h(px(21.)).child(l.to_string())),
            ),
        };
        div()
            .id("file-recovery")
            .key_context("FileRecovery")
            .w(px(1100.))
            .max_w(relative(0.92))
            .h(px(640.))
            .flex()
            .flex_row()
            .overflow_hidden()
            .bg(t.background_primary)
            .border_1()
            .border_color(t.background_modifier_border)
            .rounded_lg()
            .shadow_lg()
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_dates_and_sizes() {
        assert_eq!(display_name("Sub/Note.md"), "Note");
        assert_eq!(display_name("Board.canvas"), "Board.canvas");
        assert_eq!(snapshot_size(13), "13 B");
        assert_eq!(snapshot_size(2048), "2.0 KB");
        // moment's `llll` shape, in local time.
        let date = snapshot_date(1794291740648);
        assert!(date.contains(", 2026 "), "{date}");
        assert!(date.ends_with(" AM") || date.ends_with(" PM"), "{date}");
    }

    #[test]
    fn the_search_chooses_a_file_by_name_or_lists_matches() {
        let paths = vec![
            "Note.md".to_string(),
            "Sub/Notes.md".to_string(),
            "Board.canvas".to_string(),
        ];
        assert_eq!(
            match_files(&paths, "note"),
            (Some("Note.md".into()), vec![])
        );
        assert_eq!(
            match_files(&paths, "Board.canvas"),
            (Some("Board.canvas".into()), vec![])
        );
        assert_eq!(
            match_files(&paths, "not"),
            (None, vec!["Note.md".into(), "Sub/Notes.md".into()])
        );
        assert_eq!(match_files(&paths, "").1.len(), 3);
    }
}
