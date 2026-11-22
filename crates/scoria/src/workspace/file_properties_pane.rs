//! The File properties pane (`properties:open-local`, the right dock's
//! `file-properties` leaf), as Obsidian 1.15.1 runs it
//! (`tools/obsidian-probe/results/2026-11-15-file-properties.json`).
//!
//! The pane edits the open note through the note's own editor: the rows are
//! drawn by the renderer Live Preview's properties block uses, with a
//! [`PropertiesHost`] the editor builds ([`Editor::pane_properties_host`]),
//! and every click, key and typed character goes to the editor's own
//! property handlers. So a key, a value, the type menu and Add property write
//! the bytes Live Preview writes, one undo step each, and the autosave puts
//! them on disk with the note's BOM and line endings. The editor draws a
//! field in one place only: in the pane or in the note's block, whichever
//! opened it.
//!
//! The leaf is made by `properties:open-local` (last in the dock's first tab
//! group, active, the dock expanded, the keyboard in the pane) and only
//! revealed when it exists. Its tab's menu closes it, and so does Mod+W while
//! the pane has the keyboard; the dock then shows the tab before it and the
//! file loses the leaf. With no file open, or a file that is not Markdown,
//! the pane says "No properties found."; a note with no properties shows
//! only "Add property". Links in a value preview under Mod (the
//! `properties` hover source).

use super::*;
use crate::editor::{pane_field_keys, CloseFind, PropertySurface, PANE_CONTEXT};
use crate::reading::view::{HoverHost, ReadingHost};

/// What the pane says with no file, or a file that is not Markdown: the
/// pane's `div.pane-empty`, shown in those cases only.
pub(super) const EMPTY: &str = "No properties found.";

/// The right dock's tabs in the order they are drawn.
pub(super) const DOCK_ORDER: [RightTab; 7] = [
    RightTab::Backlinks,
    RightTab::Outgoing,
    RightTab::Tags,
    RightTab::Properties,
    RightTab::Outline,
    RightTab::FileProperties,
    RightTab::Footnotes,
];

/// The tab the dock shows once `closed`, the tab on screen, is gone: the
/// one before it that is still offered, else the one after, as Obsidian
/// showed Outline when File properties, after it, closed (`s04`, `s06`).
pub(super) fn tab_after_close(
    closed: RightTab,
    offered: impl Fn(RightTab) -> bool,
) -> Option<RightTab> {
    let at = DOCK_ORDER.iter().position(|t| *t == closed)?;
    DOCK_ORDER[..at]
        .iter()
        .rev()
        .chain(DOCK_ORDER[at + 1..].iter())
        .copied()
        .find(|t| *t != closed && offered(*t))
}

/// Takes the `kind` leaf out of a dock's tab group, as Close does: the
/// group's `currentTab` keeps naming the leaf it named, and when that was
/// the one removed it names the leaf before it (Obsidian wrote
/// `currentTab: 4` after closing the leaf at 5). `false` when the dock had
/// no such leaf.
pub(super) fn remove_leaf(dock: &mut Value, kind: &str) -> bool {
    fn walk(node: &mut Value, kind: &str) -> bool {
        let is_tabs = node.get("type").and_then(Value::as_str) == Some("tabs");
        let Some(object) = node.as_object_mut() else {
            return false;
        };
        let Some(children) = object.get_mut("children").and_then(Value::as_array_mut) else {
            return false;
        };
        let at = children.iter().position(|child| {
            child.get("type").and_then(Value::as_str) == Some("leaf")
                && child.pointer("/state/type").and_then(Value::as_str) == Some(kind)
        });
        if let (true, Some(at)) = (is_tabs, at) {
            children.remove(at);
            if let Some(current) = object.get("currentTab").and_then(Value::as_u64) {
                let current = current as usize;
                let now = if current >= at && current > 0 {
                    current - 1
                } else {
                    current
                };
                object.insert("currentTab".into(), json!(now));
            }
            return true;
        }
        children.iter_mut().any(|child| walk(child, kind))
    }
    walk(dock, kind)
}

/// Points the tab group holding the `kind` leaf at it, for a leaf made
/// while its tab is the one on screen (Obsidian's `currentTab: 5` after
/// `properties:open-local`).
pub(super) fn point_current_tab_at(dock: &mut Value, kind: &str) {
    fn walk(node: &mut Value, kind: &str) -> bool {
        let Some(object) = node.as_object_mut() else {
            return false;
        };
        let is_tabs = object.get("type").and_then(Value::as_str) == Some("tabs");
        let Some(children) = object.get_mut("children").and_then(Value::as_array_mut) else {
            return false;
        };
        let at = children
            .iter()
            .position(|child| child.pointer("/state/type").and_then(Value::as_str) == Some(kind));
        if let (true, Some(at)) = (is_tabs, at) {
            object.insert("currentTab".into(), json!(at));
            return true;
        }
        children.iter_mut().any(|child| walk(child, kind))
    }
    walk(dock, kind);
}

impl Workspace {
    /// The note File properties shows and its text: the open note when it
    /// is Markdown, read from the buffer while the editor holds it (so the
    /// rows follow typing) and from the index otherwise.
    pub(super) fn file_properties_note(&self, cx: &App) -> Option<(String, Arc<String>)> {
        let path = self.current.clone()?;
        if FileKind::from_path(&path) != FileKind::Markdown {
            return None;
        }
        let editor = self.editor.read(cx);
        if editor.name.as_ref() == path.as_str() {
            return Some((path, Arc::new(editor.text().to_string())));
        }
        let text = self.state.as_ref()?.texts().get(&path).cloned()?;
        Some((path, text))
    }

    /// Whether the pane can write the note: the editor holds it, which is
    /// what every edit goes through. Otherwise the rows are drawn read-only.
    pub(super) fn file_properties_editable(&self, cx: &App) -> bool {
        self.current.as_deref().is_some_and(|path| {
            FileKind::from_path(path) == FileKind::Markdown
                && self.editor.read(cx).name.as_ref() == path
        })
    }

    /// File properties' rows: the open note's properties block, each key at
    /// the type `types.json` assigns it, in the block's own order.
    pub(super) fn file_property_rows(&self, cx: &App) -> Vec<crate::properties::Row> {
        let Some((_, text)) = self.file_properties_note(cx) else {
            return Vec::new();
        };
        let Some(block) = scoria_cache::note_properties(&text) else {
            return Vec::new();
        };
        let types = self
            .state
            .as_ref()
            .map(|s| s.property_types().clone())
            .unwrap_or_default();
        let mut next_id = crate::properties::PROPERTY_ID_BASE;
        crate::properties::rows(
            &block.entries,
            |key| scoria_formats::properties::type_of(key, &block.entries, &types),
            &mut next_id,
        )
    }

    /// `properties:open-local`: the leaf made, or revealed when it exists,
    /// the dock expanded, and the keyboard in the pane (`s01`, `s02`).
    pub(super) fn open_file_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.right_visible = true;
        self.show_right_tab("file-properties", cx);
        self.focus_file_properties(window, cx);
    }

    /// Puts the keyboard in the pane, which is how Obsidian makes it the
    /// active leaf (its `.metadata-container` takes focus).
    pub(super) fn focus_file_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.file_properties_focus, cx);
        cx.notify();
    }

    /// Close on a sidebar tab's menu, or Mod+W in the pane: the leaf goes,
    /// with any field of it written first, and the dock shows the tab
    /// before it; the keyboard goes back to the note. Its command makes it
    /// again.
    pub(super) fn close_side_leaf(
        &mut self,
        kind: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = match kind {
            "file-properties" => {
                self.leave_file_properties_field(window, cx);
                self.file_properties_open = false;
                RightTab::FileProperties
            }
            "footnotes" => {
                self.footnotes.open = false;
                RightTab::Footnotes
            }
            _ => return,
        };
        if self.right_tab == tab {
            if let Some(next) = tab_after_close(tab, |t| self.pane_enabled(t)) {
                self.select_right_tab(next, cx);
            }
        }
        if self.file_properties_focus.is_focused(window) {
            self.focus_main(window, cx);
        }
        cx.notify();
    }

    /// A right click on a closable sidebar tab: its menu at the click.
    pub(super) fn open_side_tab_menu(
        &mut self,
        kind: &'static str,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.file_menu = Some(file_menu::Menu {
            position,
            submenu: Default::default(),
            anchor: Anchor::TopLeft,
            target: file_menu::Target::SideTab { kind },
        });
        cx.notify();
    }

    /// The pane lost the keyboard: whichever editor holds a field of the
    /// pane writes it.
    pub(super) fn leave_file_properties_field(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editors: Vec<Entity<Editor>> = self.groups.iter().map(|g| g.editor.clone()).collect();
        for editor in std::iter::once(self.editor.clone()).chain(editors) {
            editor.update(cx, |e, cx| e.leave_pane_field(window, cx));
        }
    }

    /// Escape in the pane: the open menu or field goes first; a second
    /// press hands the keyboard back to the note (`s07`).
    fn file_properties_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.update(cx, |e, cx| e.escape_pane_field(cx)) {
            return;
        }
        self.focus_main(window, cx);
    }

    /// An edit the pane, or Add alias's field in the reading view's own
    /// block, made in reading view, where the editor holding the note is not
    /// on screen: the reading view is rebuilt from its buffer, as it is
    /// after a pill removed there, and the note is written.
    pub(super) fn reading_follows_pane_edit(
        &mut self,
        editor: &Entity<Editor>,
        cx: &mut Context<Self>,
    ) {
        if self.mode != ViewMode::Reading || editor.entity_id() != self.editor.entity_id() {
            return;
        }
        let Some(path) = self.current.clone() else {
            return;
        };
        if editor.read(cx).name.as_ref() != path.as_str() {
            return;
        }
        let Some(state) = self.state.clone() else {
            return;
        };
        self.rebuild_reading_from_editor(&path, &state, cx);
        // Obsidian wrote a property edit made from reading view at once, the
        // alias on Enter or on leaving the field within the probe's first
        // poll (2026-11-22, `s01`, `s05`), not on an editor's debounce.
        self.flush_save(cx);
    }

    /// The pane: the open note's rows drawn by the renderer Live Preview's
    /// block uses, editable through the note's editor, with Add property
    /// after them and no "Properties" heading. With no Markdown note open
    /// it says Obsidian's "No properties found.".
    pub(super) fn render_file_properties_pane(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let focus = self.file_properties_focus.clone();
        let note = self.file_properties_note(cx);
        let root = div()
            .id("file-properties")
            .key_context(PANE_CONTEXT)
            .track_focus(&focus)
            .flex()
            .flex_col()
            .size_full()
            .on_action(cx.listener(|this, _: &CloseFind, window, cx| {
                this.file_properties_escape(window, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.file_properties_focus, cx);
                }),
            );
        let Some((path, text)) = note else {
            // Obsidian's `.pane-empty`, as the Outline draws it.
            return root
                .px(px(12.))
                .child(
                    div()
                        .my(px(8.))
                        .px(px(16.))
                        .py(px(24.))
                        .text_size(px(FONT_UI_SMALL))
                        .text_color(theme.tokens.text_faint)
                        .text_center()
                        .child(EMPTY),
                )
                .into_any_element();
        };
        let entries = scoria_cache::note_properties(&text)
            .map(|b| b.entries)
            .unwrap_or_default();
        let types = self
            .state
            .as_ref()
            .map(|s| s.property_types().clone())
            .unwrap_or_default();
        let editable = self.file_properties_editable(cx);
        let editor = self.editor.clone();
        // A click that opens a field takes the keyboard into the pane first,
        // so the keys that follow are the pane's.
        let host = editable.then(|| {
            let mut host = Editor::pane_properties_host(&editor, cx);
            let inner = host.on_action.clone();
            let focus = focus.clone();
            host.on_action = Rc::new(move |action, window, cx| {
                window.focus(&focus, cx);
                inner(action, window, cx);
            });
            host
        });
        let this = cx.entity().downgrade();
        let on_link: reading::view::LinkHandler = Rc::new(move |target, _, cx| {
            let _ = this.update(cx, |this, cx| this.follow_link(target, cx));
        });
        let on_toggle: reading::view::ToggleHandler = Rc::new(|_, _| {});
        // Page preview's `properties` source: Mod needed by default, the
        // link resolved from the note the pane shows (`s11` of 2026-11-11).
        let this = cx.entity().downgrade();
        let from = path.clone();
        let reading_host = ReadingHost {
            properties: PropertiesInDocument::Visible,
            copied: None,
            on_copy: Rc::new(|_, _| {}),
            on_task: Rc::new(|_, _| {}),
            on_remove_item: None,
            on_menu: None,
            hover: Some(HoverHost {
                on_hover: Rc::new(move |hover, _, cx| {
                    let from = from.clone();
                    let _ = this.update(cx, |this, cx| {
                        this.nested_link_hover(
                            crate::hover_preview::Source::Properties,
                            None,
                            from,
                            0,
                            hover,
                            cx,
                        )
                    });
                }),
                current: self.file_properties_hover.clone(),
            }),
        };
        let no_folds = HashSet::new();
        let available = (self.right_width as f32 - 24.).max(100.);
        let renderer = reading::view::Renderer::new(
            theme,
            window.text_system().clone(),
            available,
            &no_folds,
            &types,
            host.as_ref(),
            None,
            on_link,
            on_toggle,
        )
        .with_reading_host(&reading_host);
        let content = renderer.properties(&entries);
        // Typed text reaches the open field through the editor's own input
        // handler, registered against the pane's handle.
        let input = (editable && editor.read(cx).pane_field_open()).then(|| {
            let editor = editor.clone();
            let focus = focus.clone();
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    window.handle_input(&focus, ElementInputHandler::new(bounds, editor), cx);
                },
            )
            .absolute()
            .size_full()
        });
        let root = root
            .relative()
            .overflow_y_scroll()
            .px(px(12.))
            .pt(px(8.))
            .text_color(theme.tokens.text_normal)
            .children(input)
            .child(content);
        pane_field_keys(root, &editor, PropertySurface::Pane).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(dock: &Value) -> Vec<String> {
        dock["children"][0]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["state"]["type"].as_str().unwrap().to_string())
            .collect()
    }

    /// Close on the leaf the dock shows, last in its group: the leaf goes
    /// and `currentTab` names the one before it, as Obsidian wrote (`s04`).
    #[test]
    fn closing_the_shown_leaf_points_the_group_at_the_one_before() {
        let mut right = default_dock_json("right", Some("A.md"), 0);
        push_file_properties_leaf(&mut right, Some("A.md"));
        set_dock_current_tab(&mut right, 5);
        assert!(remove_leaf(&mut right, "file-properties"));
        assert_eq!(
            kinds(&right),
            [
                "backlink",
                "outgoing-link",
                "tag",
                "all-properties",
                "outline"
            ]
        );
        assert_eq!(right["children"][0]["currentTab"], 4);
        assert!(!remove_leaf(&mut right, "file-properties"), "already gone");
    }

    /// A leaf closed before the shown one moves `currentTab` down with the
    /// leaves after it; one closed after it leaves it alone.
    #[test]
    fn closing_another_leaf_keeps_the_shown_one_current() {
        let mut right = default_dock_json("right", Some("A.md"), 4);
        push_file_properties_leaf(&mut right, Some("A.md"));
        footnotes_pane::push_footnotes_leaf(&mut right, true);
        // Footnotes (6) is shown; File properties (5) closes before it.
        assert!(remove_leaf(&mut right, "file-properties"));
        assert_eq!(right["children"][0]["currentTab"], 5);
        assert_eq!(
            dock_current_view(&json!({"right": right.clone()}), "right").as_deref(),
            Some("footnotes")
        );
        // Outline (4) shown; Footnotes after it closes.
        set_dock_current_tab(&mut right, 4);
        assert!(remove_leaf(&mut right, "footnotes"));
        assert_eq!(right["children"][0]["currentTab"], 4);
    }

    /// A leaf in a nested split's tab group is found there too.
    #[test]
    fn a_leaf_in_a_nested_group_is_removed() {
        let mut right = json!({"type": "split", "children": [
            {"type": "tabs", "children": [
                {"type": "leaf", "state": {"type": "backlink", "state": {}}}
            ]},
            {"type": "tabs", "currentTab": 1, "children": [
                {"type": "leaf", "state": {"type": "outline", "state": {}}},
                {"type": "leaf", "state": {"type": "file-properties", "state": {}}}
            ]}
        ]});
        assert!(remove_leaf(&mut right, "file-properties"));
        assert_eq!(right["children"][1]["currentTab"], 0);
        assert_eq!(
            right["children"][1]["children"].as_array().unwrap().len(),
            1
        );
    }

    /// A leaf made again is pointed at, as `properties:open-local` leaves
    /// `currentTab` on its new leaf (`s05`).
    #[test]
    fn a_reopened_leaf_becomes_the_current_tab() {
        let mut right = default_dock_json("right", Some("A.md"), 4);
        push_file_properties_leaf(&mut right, Some("A.md"));
        point_current_tab_at(&mut right, "file-properties");
        assert_eq!(right["children"][0]["currentTab"], 5);
        assert_eq!(
            dock_current_view(&json!({"right": right}), "right").as_deref(),
            Some("file-properties")
        );
    }

    /// The dock shows the tab before the closed one, else the one after,
    /// skipping tabs that are not offered.
    #[test]
    fn the_tab_after_a_close_is_the_one_before_it() {
        let all = |_: RightTab| true;
        assert_eq!(
            tab_after_close(RightTab::FileProperties, all),
            Some(RightTab::Outline)
        );
        let no_outline = |t: RightTab| t != RightTab::Outline;
        assert_eq!(
            tab_after_close(RightTab::FileProperties, no_outline),
            Some(RightTab::Properties)
        );
        let only_footnotes = |t: RightTab| t == RightTab::Footnotes;
        assert_eq!(
            tab_after_close(RightTab::FileProperties, only_footnotes),
            Some(RightTab::Footnotes)
        );
        assert_eq!(tab_after_close(RightTab::FileProperties, |_| false), None);
    }

    #[test]
    fn the_empty_state_is_obsidians_text() {
        assert_eq!(EMPTY, "No properties found.");
    }
}
