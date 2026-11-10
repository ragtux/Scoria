//! The Workspaces core plugin: named layouts in `workspaces.json`.
//!
//! Four commands, as Obsidian 1.15.0 has them (see
//! [[Specification#Observed Workspaces plugin, Obsidian 1.15.0]]):
//! `workspaces:open-modal` and the ribbon's "Manage workspace layouts"
//! open the manage modal; `workspaces:save` opens it too when no layout is
//! active, and otherwise saves over the active one and says so;
//! `workspaces:load` and `workspaces:save-and-load` open a suggester over
//! the saved names, the second saving over the active layout before it
//! loads the pick.
//!
//! A save is `workspace.json` as this session would write it, less
//! `lastOpenFiles`, plus the time ([`WorkspaceLayout::from_workspace`]).
//! A load closes every group and popout and restores the saved layout
//! through the very code that restores `workspace.json` at launch; the
//! session's `lastOpenFiles` carries on, with the files the load closed
//! moved to its front. `workspace.json` itself is not written by a load:
//! Obsidian rewrote it a second later on its ordinary layout save, and
//! Scoria's ordinary layout save is the one on the way out.

use super::*;
use gpui::{Div, ElementId, EventEmitter};
use scoria_formats::workspaces::{WorkspaceLayout, Workspaces};

/// The placeholder of the manage modal's name field.
const SAVE_PLACEHOLDER: &str = "Save current workspace layout as...";
/// The load suggester's placeholder, and what it says with nothing saved.
const LOAD_PLACEHOLDER: &str = "Type layout name...";
const LOAD_EMPTY: &str = "No saved layout found.";

/// What a save with a layout active posts, curly quotes and all.
fn saved_notice(name: &str) -> String {
    format!("Successfully saved layout \u{201c}{name}\u{201d}")
}

/// `mtime` as Obsidian writes it: local time with its offset, to the
/// second (`2026-11-09T20:05:44-06:00`).
fn mtime_of(at: chrono::DateTime<chrono::Local>) -> String {
    at.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// The row's "Modified …" line: the time since `mtime` in the words
/// Obsidian's dates use everywhere ("a few seconds ago", "3 minutes ago"),
/// the same humanizing the Bases `relative()` function was matched to.
/// Only "a few seconds ago" was seen in this modal.
fn modified_label(mtime: Option<&str>, now: chrono::DateTime<chrono::Local>) -> String {
    let at = match mtime {
        None => now.naive_local(),
        Some(text) => match chrono::DateTime::parse_from_rfc3339(text) {
            Ok(at) => at.with_timezone(&chrono::Local).naive_local(),
            Err(_) => return "Modified Invalid date".into(),
        },
    };
    let date = scoria_bases::value::DateValue { at, has_time: true };
    format!(
        "Modified {}",
        scoria_bases::funcs::time::relative(&date, now.naive_local())
    )
}

/// `lastOpenFiles` after a load: the session's list, with each file the
/// load closed moved to the front in the order its tab stood, so the last
/// tab closed comes first. In the probe, loading "One" (A) over A and B
/// turned `["A.md"]` into `["B.md", "A.md"]`, and loading it again (C)
/// over A and B turned `["A.md", "C.md", "B.md"]` into
/// `["B.md", "A.md", "C.md"]`.
fn recent_after_load(mut recent: Vec<String>, before: &[String], after: &[String]) -> Vec<String> {
    for path in before.iter().filter(|p| !after.contains(p)) {
        recent.retain(|p| p != path);
        recent.insert(0, path.clone());
    }
    recent.truncate(LAST_OPEN_FILES_KEPT);
    recent
}

/// One saved layout as the manage modal lists it.
#[derive(Debug, Clone, PartialEq)]
struct LayoutRow {
    name: String,
    active: bool,
    modified: String,
}

fn layout_rows(file: &Workspaces, now: chrono::DateTime<chrono::Local>) -> Vec<LayoutRow> {
    file.workspaces
        .iter()
        .map(|(name, layout)| LayoutRow {
            name: name.clone(),
            active: file.active.as_deref() == Some(name.as_str()),
            modified: modified_label(layout.mtime.as_deref(), now),
        })
        .collect()
}

pub enum LayoutsEvent {
    /// Save, or Enter in the field: the name typed.
    Save(String),
    Load(String),
    Delete(String),
    Close,
}

/// "Manage workspace layouts": the name field and Save, then one row per
/// layout with Load and "Delete layout". There is no rename, and a delete
/// asks nothing.
pub struct LayoutsModal {
    input: Entity<TextInput>,
    rows: Vec<LayoutRow>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<LayoutsEvent> for LayoutsModal {}

impl Focusable for LayoutsModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl LayoutsModal {
    fn new(cx: &mut Context<Self>) -> LayoutsModal {
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx);
            input.set_placeholder(SAVE_PLACEHOLDER);
            input.set_height(px(30.));
            input.set_pad_left(px(8.));
            input.set_pad_right(px(8.));
            input
        });
        let subscriptions = vec![cx.subscribe(
            &input,
            |this: &mut LayoutsModal, _, event: &InputEvent, cx| match event {
                InputEvent::Submit => this.submit(cx),
                InputEvent::Cancel => cx.emit(LayoutsEvent::Close),
                _ => {}
            },
        )];
        LayoutsModal {
            input,
            rows: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// The rows anew, after a save or a delete; the field is cleared, as
    /// Obsidian's is after a save.
    fn set_rows(&mut self, rows: Vec<LayoutRow>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.input.update(cx, |i, cx| i.set_text("", cx));
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.input.read(cx).text().to_string();
        // What an empty name does was not observed; nothing is saved.
        if !name.is_empty() {
            cx.emit(LayoutsEvent::Save(name));
        }
    }

    /// For the control socket: the field, and each row as it reads.
    fn describe(&self, cx: &App) -> Value {
        json!({
            "query": self.input.read(cx).text(),
            "rows": self.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            "layouts": self.rows.iter().map(|r| json!({
                "name": r.name,
                "active": r.active,
                "modified": r.modified,
            })).collect::<Vec<_>>(),
        })
    }

    /// A `button.list-item-part`: 13 px on `--interactive-normal`, radius 5,
    /// padding 4 × 12, 30 tall (2026-11-09, `w05` styles).
    fn button(id: impl Into<ElementId>, label: &'static str, theme: &Theme) -> gpui::Stateful<Div> {
        div()
            .id(id)
            .flex_none()
            .h(px(30.))
            .px(px(12.))
            .flex()
            .items_center()
            .rounded(px(5.))
            .cursor_pointer()
            .bg(theme.tokens.interactive_normal)
            .hover(|s| s.bg(theme.tokens.interactive_hover))
            .text_size(px(13.))
            .text_color(theme.tokens.text_normal)
            .child(label)
    }
}

impl Render for LayoutsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>().clone();
        let rows: Vec<gpui::AnyElement> = self
            .rows
            .iter()
            .enumerate()
            .map(|(ix, row)| {
                let (load, delete) = (row.name.clone(), row.name.clone());
                div()
                    .id(("workspaces-row", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .min_h(px(35.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .text_color(theme.tokens.text_normal)
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(SharedString::from(row.name.clone())),
                                    )
                                    // `span.flair.mod-pop`: 10 px capitals,
                                    // white on the accent, radius 4.
                                    .when(row.active, |d| {
                                        d.child(
                                            div()
                                                .flex_none()
                                                .px(px(4.))
                                                .py(px(2.))
                                                .rounded(px(4.))
                                                .bg(theme.tokens.interactive_accent)
                                                .text_size(px(10.))
                                                .line_height(px(14.))
                                                .text_color(theme.tokens.text_on_accent)
                                                .child("ACTIVE"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.tokens.text_faint)
                                    .child(SharedString::from(row.modified.clone())),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Self::button(("workspaces-load", ix), "Load", &theme).on_click(
                                    cx.listener(move |_, _, _, cx| {
                                        cx.emit(LayoutsEvent::Load(load.clone()))
                                    }),
                                ),
                            )
                            .child(
                                div()
                                    .id(("workspaces-delete", ix))
                                    .flex_none()
                                    .w(px(24.))
                                    .h(px(30.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                                    .tooltip(tooltip("Delete layout"))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.emit(LayoutsEvent::Delete(delete.clone()))
                                    }))
                                    .child(crate::assets::icon("x", 16., theme.tokens.icon_color)),
                            ),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .id("workspaces-modal")
            .relative()
            .w(px(560.))
            .flex()
            .flex_col()
            .p(px(16.))
            .bg(theme.tokens.modal_background)
            .border_1()
            .border_color(theme.tokens.modal_border_color)
            .rounded(px(12.))
            .shadow_lg()
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(LayoutsEvent::Close)))
            .child(
                div()
                    .id("workspaces-close")
                    .absolute()
                    .top(px(12.))
                    .right(px(12.))
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(LayoutsEvent::Close)))
                    .child(crate::assets::icon("x", 16., theme.tokens.icon_color)),
            )
            .child(
                div()
                    .h(px(20.))
                    .mb(px(18.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.tokens.text_normal)
                    .child("Manage workspace layouts"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().min_w_0().child(self.input.clone()))
                    .child(
                        Self::button("workspaces-save", "Save", &theme)
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
            .when(!rows.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(28.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .children(rows),
                )
            })
    }
}

impl Workspace {
    /// `workspaces.json`, empty when the vault has none yet. A file that
    /// will not parse is reported and `None` comes back: nothing may be
    /// written over it.
    fn read_workspaces(&mut self, cx: &mut Context<Self>) -> Option<Workspaces> {
        let state = self.state.clone()?;
        let read = state.vault().workspaces();
        match read {
            Ok(file) => Some(file.unwrap_or_default()),
            Err(err) => {
                self.report(format!("workspaces.json could not be read: {err}"), cx);
                None
            }
        }
    }

    fn write_workspaces(&mut self, file: &Workspaces, cx: &mut Context<Self>) -> bool {
        let Some(state) = self.state.clone() else {
            return false;
        };
        let written = state.vault().write_workspaces(file);
        match written {
            Ok(()) => true,
            Err(err) => {
                self.report(format!("workspaces.json could not be written: {err}"), cx);
                false
            }
        }
    }

    /// `workspaces:open-modal`, and the ribbon's "Manage workspace layouts".
    pub(super) fn open_layouts_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.read_workspaces(cx) else {
            return;
        };
        self.close_editor_suggest(cx);
        let modal = match self.layouts_modal.clone() {
            Some(modal) => modal,
            None => {
                let modal = cx.new(LayoutsModal::new);
                let subscription = cx.subscribe_in(
                    &modal,
                    window,
                    |this, _, event: &LayoutsEvent, window, cx| match event {
                        LayoutsEvent::Save(name) => {
                            this.save_named_layout(name, cx);
                            this.refresh_layouts_modal(cx);
                        }
                        LayoutsEvent::Load(name) => {
                            this.close_modal(window, cx);
                            this.load_named_layout(name.clone(), cx);
                        }
                        LayoutsEvent::Delete(name) => {
                            this.delete_named_layout(name, cx);
                            this.refresh_layouts_modal(cx);
                        }
                        LayoutsEvent::Close => this.close_modal(window, cx),
                    },
                );
                self._subscriptions.push(subscription);
                self.layouts_modal = Some(modal.clone());
                modal
            }
        };
        let rows = layout_rows(&file, chrono::Local::now());
        modal.update(cx, |m, cx| m.set_rows(rows, cx));
        self.modal = Modal::Layouts;
        window.focus(&modal.focus_handle(cx), cx);
        cx.notify();
    }

    fn refresh_layouts_modal(&mut self, cx: &mut Context<Self>) {
        let (Some(modal), Some(file)) = (self.layouts_modal.clone(), self.read_workspaces(cx))
        else {
            return;
        };
        let rows = layout_rows(&file, chrono::Local::now());
        modal.update(cx, |m, cx| m.set_rows(rows, cx));
    }

    /// The manage modal for the socket's state dump.
    pub(super) fn describe_layouts_modal(&self, cx: &App) -> Option<Value> {
        self.layouts_modal.as_ref().map(|m| m.read(cx).describe(cx))
    }

    /// `workspaces:save`: with no layout active, the manage modal; with
    /// one, a save over it and the notice. A deleted layout still counts
    /// as active, as it does to Obsidian's plugin, so this saves it again.
    pub(super) fn save_active_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.read_workspaces(cx) else {
            return;
        };
        match file.active {
            Some(name) => {
                if self.save_named_layout(&name, cx) {
                    self.report(saved_notice(&name), cx);
                }
            }
            None => self.open_layouts_modal(window, cx),
        }
    }

    /// `workspaces:load` and `workspaces:save-and-load`: the saved names in
    /// their order, the active one flaired.
    pub(super) fn open_layout_chooser(
        &mut self,
        save_first: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(file) = self.read_workspaces(cx) else {
            return;
        };
        let rows: Vec<ChooserRow> = file
            .workspaces
            .keys()
            .map(|name| ChooserRow {
                label: name.clone(),
                value: name.clone(),
                flair: (file.active.as_deref() == Some(name.as_str())).then(|| "Active".into()),
            })
            .collect();
        self.close_editor_suggest(cx);
        self.chooser_purpose = ChooserPurpose::Layout { save_first };
        self.modal = Modal::Chooser;
        self.chooser.update(cx, |c, cx| {
            c.open(rows, LOAD_PLACEHOLDER, LOAD_EMPTY, window, cx)
        });
        cx.notify();
    }

    /// A pick in the load suggester; `save_first` saves over the active
    /// layout before loading (observed: its `mtime` moved), silently.
    pub(super) fn layout_chosen(&mut self, name: String, save_first: bool, cx: &mut Context<Self>) {
        if save_first {
            let active = self.read_workspaces(cx).and_then(|f| f.active);
            if let Some(active) = active {
                if !self.save_named_layout(&active, cx) {
                    return;
                }
            }
        }
        self.load_named_layout(name, cx);
    }

    /// Saves the layout in hand as `name` and makes it active.
    fn save_named_layout(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        let Some(mut file) = self.read_workspaces(cx) else {
            return false;
        };
        let layout =
            WorkspaceLayout::from_workspace(&self.layout_json(cx), mtime_of(chrono::Local::now()));
        file.save(name, layout);
        self.write_workspaces(&file, cx)
    }

    fn delete_named_layout(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(mut file) = self.read_workspaces(cx) else {
            return;
        };
        if file.delete(name) {
            self.write_workspaces(&file, cx);
        }
    }

    /// Loads layout `name` once this update is over, in the vault window:
    /// the groups it makes subscribe to that window, which outlives any
    /// popout the load closes, the one the request came from included.
    fn load_named_layout(&mut self, name: String, cx: &mut Context<Self>) {
        let this = cx.entity();
        let main = self.main_window;
        cx.defer(move |cx| {
            main.update(cx, |_, window, cx| {
                this.update(cx, |ws, cx| ws.apply_named_layout(&name, window, cx))
            })
            .ok();
        });
    }

    /// Replaces the layout in place: every group and popout closes, the
    /// saved layout is restored the way `workspace.json` is at launch, and
    /// `workspaces.json` records it as active.
    fn apply_named_layout(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        let Some(mut file) = self.read_workspaces(cx) else {
            return;
        };
        let Some(layout) = file.workspaces.get(name).map(WorkspaceLayout::to_workspace) else {
            return;
        };
        self.flush_save(cx);
        if let Some(path) = self.current.clone() {
            self.save_folds(&path, cx);
        }
        let before = self.open_paths();
        let recent = std::mem::take(&mut self.last_open_files);
        let ids: Vec<u64> = self.groups.iter().map(|g| g.id).collect();
        for id in ids {
            self.close_group(id, false, cx);
        }
        self.apply_layout(layout, &state, window, cx);
        // The layout has no `lastOpenFiles`, so `self.layout` lacks the key
        // `layout_json` writes and the next layout save writes the file.
        let after = self.open_paths();
        self.last_open_files = recent_after_load(recent, &before, &after);
        file.set_active(name);
        self.write_workspaces(&file, cx);
        // The handle that had the keyboard may be a closed group's view,
        // which the render's refocus does not count as parked: the
        // keyboard goes to the restored active group here.
        self.focus_main(window, cx);
        cx.notify();
    }

    /// Every file open in a tab, the main area's groups in layout order
    /// and then each popout's, each file once.
    fn open_paths(&self) -> Vec<String> {
        let mut order = Vec::new();
        self.layout_tree.groups(&mut order);
        for w in &self.windows {
            w.tree.groups(&mut order);
        }
        let mut paths: Vec<String> = Vec::new();
        for id in order {
            let Some(ix) = self.group_ix(id) else {
                continue;
            };
            let tabs = if ix == self.active_group {
                self.tab_snapshots()
            } else {
                self.groups[ix].tabs.clone()
            };
            for path in tabs.into_iter().filter_map(|t| t.path) {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn the_four_workspaces_commands_are_runnable_and_belong_to_the_plugin() {
        let known = scoria_commands::known_ids();
        for id in [
            "workspaces:open-modal",
            "workspaces:save",
            "workspaces:load",
            "workspaces:save-and-load",
        ] {
            assert!(RUNNABLE.contains(&id), "{id} runnable");
            assert!(known.contains(&id), "{id} is Obsidian's");
            assert_eq!(scoria_commands::plugin_of(id), Some("workspaces"), "{id}");
        }
    }

    /// A `main` node read the way a load reads it and written the way the
    /// next layout save writes it.
    fn main_json(spec: &LayoutSpec) -> Value {
        match spec {
            LayoutSpec::Group(group) => tabs_node_json(
                group.node.clone(),
                group.tabs.iter().map(|(_, t)| leaf_json(t)).collect(),
                group.active_tab,
            ),
            LayoutSpec::Split {
                direction,
                children,
                node,
            } => split_node_json(
                node.clone(),
                children.iter().map(main_json).collect(),
                *direction,
            ),
        }
    }

    /// Every layout Obsidian saved in the probe — one tab, a split of two —
    /// comes through a load and the save after it with its `main` as it
    /// was, leaf ids, icons and titles included.
    #[test]
    fn a_saved_layouts_main_area_restores_and_writes_back_unchanged() {
        for text in [
            include_str!("../../../scoria-formats/tests/workspaces/w03-workspaces.json"),
            include_str!("../../../scoria-formats/tests/workspaces/w08-workspaces.json"),
        ] {
            let file = Workspaces::parse(text).unwrap();
            for (name, layout) in &file.workspaces {
                let restored = layout.to_workspace();
                let spec = read_main(&restored["main"], ViewMode::LivePreview, &|_| true).unwrap();
                assert_eq!(main_json(&spec), restored["main"], "{name}");
            }
        }
    }

    #[test]
    fn mtime_is_local_time_with_its_offset_to_the_second() {
        let offset = chrono::FixedOffset::west_opt(6 * 3600).unwrap();
        let at = offset
            .with_ymd_and_hms(2026, 11, 9, 20, 5, 44)
            .unwrap()
            .with_timezone(&chrono::Local);
        let text = mtime_of(at);
        // Whatever the machine's zone, it reads back as the same instant,
        // in Obsidian's shape: no fraction, a `±hh:mm` offset.
        let back = chrono::DateTime::parse_from_rfc3339(&text).unwrap();
        assert_eq!(back, at);
        assert_eq!(text.len(), "2026-11-09T20:05:44-06:00".len());
        assert!(matches!(&text[19..20], "+" | "-"), "{text}");
        assert_eq!(&text[22..23], ":");
    }

    #[test]
    fn the_save_notice_quotes_the_name_with_curly_quotes() {
        assert_eq!(
            saved_notice("One"),
            "Successfully saved layout \u{201c}One\u{201d}"
        );
    }

    #[test]
    fn a_layout_saved_seconds_ago_reads_modified_a_few_seconds_ago() {
        let now = chrono::Local::now();
        let saved = mtime_of(now - chrono::Duration::seconds(5));
        assert_eq!(
            modified_label(Some(&saved), now),
            "Modified a few seconds ago"
        );
        let older = mtime_of(now - chrono::Duration::minutes(3));
        assert_eq!(modified_label(Some(&older), now), "Modified 3 minutes ago");
    }

    /// The probe's two loads: the files a load closed go to the front of
    /// `lastOpenFiles`, the last tab first.
    #[test]
    fn a_load_moves_the_files_it_closed_to_the_front_of_last_open_files() {
        let s = |v: &[&str]| v.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        assert_eq!(
            recent_after_load(s(&["A.md"]), &s(&["A.md", "B.md"]), &s(&["A.md"])),
            s(&["B.md", "A.md"])
        );
        assert_eq!(
            recent_after_load(
                s(&["A.md", "C.md", "B.md"]),
                &s(&["A.md", "B.md"]),
                &s(&["C.md"])
            ),
            s(&["B.md", "A.md", "C.md"])
        );
    }

    /// The modal's rows in insertion order, "Active" only on the layout
    /// `active` names; after the active one is deleted, none.
    #[test]
    fn manage_rows_flair_the_active_layout_and_none_once_it_is_deleted() {
        let mut file = Workspaces::default();
        let now = chrono::Local::now();
        let layout = || WorkspaceLayout::from_workspace(&json!({"main": {}}), mtime_of(now));
        file.save("One", layout());
        file.save("Two", layout());
        file.save("Three", layout());
        file.set_active("One");
        let rows = layout_rows(&file, now);
        let names: Vec<(&str, bool)> = rows.iter().map(|r| (r.name.as_str(), r.active)).collect();
        assert_eq!(names, [("One", true), ("Two", false), ("Three", false)]);
        assert_eq!(rows[0].modified, "Modified a few seconds ago");
        file.delete("Two");
        file.delete("One");
        let rows = layout_rows(&file, now);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].active);
        assert_eq!(file.active.as_deref(), Some("One"));
    }
}
