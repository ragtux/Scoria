//! The Footnotes pane (`footnotes:open`, the Footnotes view core plugin):
//! the open note's footnotes in the right dock, as Obsidian 1.15.0 lists
//! them (`tools/obsidian-probe/results/2026-11-09-panes-workspaces-headings.json`,
//! `SCEN=fn`).
//!
//! The rows come from [`scoria_cache::footnote_rows`], over the one parser;
//! each row's text is drawn as the reading view draws a note, from a model
//! built off the footnote's own Markdown. The pane has no header buttons
//! and no row menu, as observed. A row is read-only: Obsidian opens an
//! editor inside the row on a click, but the one edit the probe made also
//! rewrote another definition's indent, seen once, so there is nothing safe
//! to copy yet. "Open link" (shown under the pointer) puts the caret at the
//! start of the definition's first line in an editing mode, and scrolls the
//! reading view to its end, where its Footnotes section is; the flash
//! Obsidian gives the definition is not drawn, and an inline footnote's row
//! has no Open link, that case not having been run.

use super::*;
use scoria_cache::FootnoteRow;
use std::hash::{Hash, Hasher};

/// The text Obsidian's pane shows with no note open, and over a note with
/// no footnotes: the same `div.pane-empty` in both cases.
pub(super) const EMPTY: &str = "No footnotes found.";

/// A note's footnote rows, each with the model its text is drawn from.
type Rows = Rc<Vec<(FootnoteRow, reading::model::Model)>>;

/// The pane's own state.
#[derive(Default)]
pub(super) struct FootnotesPane {
    /// Whether the right dock has a `footnotes` leaf: `workspace.json` had
    /// one, or `footnotes:open` added it this session.
    pub(super) open: bool,
    /// The rows and their models for the last text drawn, by note and a
    /// hash of its text, so a frame that changed nothing parses nothing.
    cache: Option<(String, u64, Rows)>,
}

/// Adds a Footnotes leaf to a dock that has none, as `footnotes:open` does:
/// last in the dock's first tab group, after whatever is there, with the
/// state, icon and title Obsidian 1.15.0 wrote (`{"type": "footnotes",
/// "state": {}, "icon": "lucide-file-signature", "title": "Footnotes"}`),
/// and, when it is the tab showing, that group's `currentTab` pointing at it
/// (the probe's `currentTab: 5`). `false` when the dock had one already or
/// has no tab group to put it in.
pub(super) fn push_footnotes_leaf(dock: &mut Value, current: bool) -> bool {
    if leaf_of_kind(dock, "footnotes").is_some() {
        return false;
    }
    fn first_tabs(node: &mut Value) -> Option<&mut Value> {
        if node.get("type").and_then(Value::as_str) == Some("tabs") {
            return Some(node);
        }
        node.get_mut("children")?
            .as_array_mut()?
            .iter_mut()
            .find_map(first_tabs)
    }
    let Some(group) = first_tabs(dock).and_then(Value::as_object_mut) else {
        return false;
    };
    let Some(children) = group.get_mut("children").and_then(Value::as_array_mut) else {
        return false;
    };
    children.push(json!({
        "id": leaf_id(),
        "type": "leaf",
        "state": {
            "type": "footnotes",
            "state": {},
            "icon": "lucide-file-signature",
            "title": "Footnotes",
        },
    }));
    let ix = children.len() - 1;
    if current {
        group.insert("currentTab".into(), json!(ix));
    }
    true
}

impl Workspace {
    /// The open note's footnote rows and their models, built again only
    /// when the note or its text changed.
    fn footnote_rows(&mut self, cx: &App) -> Option<Rows> {
        let (path, text) = self.file_properties_note(cx)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some((p, h, rows)) = &self.footnotes.cache {
            if *p == path && *h == hash {
                return Some(rows.clone());
            }
        }
        let state = self.state.clone()?;
        let opts = state.build_options();
        let rows: Vec<(FootnoteRow, reading::model::Model)> = scoria_cache::footnote_rows(&text)
            .into_iter()
            .map(|row| {
                let model = reading::build_with(&path, &row.body, state.as_ref(), opts);
                (row, model)
            })
            .collect();
        let rows = Rc::new(rows);
        self.footnotes.cache = Some((path, hash, rows.clone()));
        Some(rows)
    }

    /// "Open link" on a row: the definition's first line, column 0, in the
    /// note's editor, scrolled into view; the keyboard stays in the pane,
    /// as observed. In reading view the note scrolls to its end, where the
    /// Footnotes section is.
    fn open_footnote(&mut self, path: String, offset: usize, cx: &mut Context<Self>) {
        if self.current.as_deref() != Some(path.as_str()) {
            return;
        }
        if self.mode == ViewMode::Reading {
            self.reading.update(cx, |r, cx| r.scroll_to_end(cx));
        } else if self.editor.read(cx).name.as_ref() == path.as_str() {
            self.editor.update(cx, |e, cx| e.reveal_offset(offset, cx));
        }
        cx.notify();
    }

    /// The Footnotes pane: one row per footnote, its number at the left
    /// (faint for a definition nothing references) and its text drawn as
    /// the reading view would, Obsidian's "No footnotes found." with no
    /// note or none in it.
    pub(super) fn render_footnotes_pane(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let rows = self.footnote_rows(cx).filter(|rows| !rows.is_empty());
        let path = self.current.clone().unwrap_or_default();
        // `.view-content`: 12px 12px 32px.
        let pane = div()
            .id("footnotes")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .pt(px(12.))
            .px(px(12.))
            .pb(px(32.));
        let Some(rows) = rows else {
            // `div.pane-empty`, as the Outline and File properties draw it.
            return pane
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
        // The row's text is a rendered embed at 13.125 px (observed), so
        // the renderer draws at that size.
        const SIZE: f32 = 13.125;
        let mut small = theme.clone();
        small.base_font_size = SIZE;
        let types = self
            .state
            .as_ref()
            .map(|s| s.property_types().clone())
            .unwrap_or_default();
        let no_folds = HashSet::new();
        let this = cx.entity().downgrade();
        let on_link: reading::view::LinkHandler = Rc::new(move |target, _, cx| {
            let _ = this.update(cx, |this, cx| this.follow_link(target, cx));
        });
        let on_toggle: reading::view::ToggleHandler = Rc::new(|_, _| {});
        // The pane's width less its padding, the number column and the
        // Open link button's room.
        let available = (self.right_width as f32 - 24. - 12. - 15. - 28.).max(100.);
        let renderer = reading::view::Renderer::new(
            &small,
            window.text_system().clone(),
            available,
            &no_folds,
            &types,
            None,
            None,
            on_link,
            on_toggle,
        );
        let mut list = div().w_full().flex().flex_col();
        for (ix, (row, model)) in rows.iter().enumerate() {
            let body = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .text_size(px(SIZE))
                .line_height(px(20.))
                .text_color(theme.tokens.text_normal)
                .children(model.blocks.iter().map(|b| renderer.block(b, true)));
            // `div.markdown-embed-link`: 24×24, 4 px padding, opacity
            // .85, `lucide-maximize-2`, showing under the pointer.
            let open_link = row.label.is_some().then(|| {
                let offset = row.offset;
                let path = path.clone();
                div()
                    .id(("footnote-open", ix))
                    .absolute()
                    .top(px(4.))
                    .right(px(6.))
                    .size(px(24.))
                    .p(px(4.))
                    .rounded(px(4.))
                    .opacity(0.85)
                    .invisible()
                    .group_hover("footnote-row", |s| s.visible())
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .cursor_pointer()
                    .tooltip(tooltip("Open link"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_footnote(path.clone(), offset, cx);
                    }))
                    .child(icon("maximize-2", 16., theme.tokens.icon_color))
            });
            list = list.child(
                // `div.footnote`: a flex row padded 6 px, the id 4 px from
                // the text.
                div()
                    .id(("footnote-row", ix))
                    .group("footnote-row")
                    .relative()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(4.))
                    .p(px(6.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(SIZE))
                            .line_height(px(20.))
                            .text_color(if row.referenced {
                                theme.tokens.text_muted
                            } else {
                                // `mod-no-occurrences`.
                                theme.tokens.text_faint
                            })
                            .child(SharedString::from(row.number.to_string())),
                    )
                    .child(body)
                    .children(open_link),
            );
        }
        pane.child(list).into_any_element()
    }

    /// The pane for the control socket: whether its leaf exists, the note,
    /// and each row's number, label, whether it is referenced, its text and
    /// the line Open link goes to.
    pub(super) fn footnotes_json(&self, cx: &App) -> Value {
        let note = self.file_properties_note(cx);
        let rows: Vec<Value> = note
            .as_ref()
            .map(|(_, text)| scoria_cache::footnote_rows(text))
            .unwrap_or_default()
            .iter()
            .map(|r| {
                json!({
                    "number": r.number,
                    "label": r.label,
                    "referenced": r.referenced,
                    "text": r.body,
                    "line": r.line,
                })
            })
            .collect();
        json!({
            "open": self.footnotes.open,
            "file": note.map(|(path, _)| path),
            "empty": rows.is_empty().then_some(EMPTY),
            "rows": rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `footnotes:open` in a right dock with the five default panes
    /// appends the leaf Obsidian 1.15.0 wrote: sixth, with that state,
    /// icon and title in that key order, and the group's `currentTab` on
    /// it. A second run adds nothing.
    #[test]
    fn footnotes_leaf_is_appended_in_obsidians_shape_and_made_current() {
        let mut right = default_dock_json("right", None, 0);
        assert!(push_footnotes_leaf(&mut right, true));
        assert!(!push_footnotes_leaf(&mut right, true));
        let group = &right["children"][0];
        let leaves = group["children"].as_array().unwrap();
        assert_eq!(leaves.len(), 6);
        assert_eq!(group["currentTab"], 5);
        let leaf = &leaves[5];
        assert_eq!(leaf["id"].as_str().unwrap().len(), 16);
        let mut without_id = leaf.clone();
        without_id.as_object_mut().unwrap().shift_remove("id");
        assert_eq!(
            serde_json::to_string(&without_id).unwrap(),
            r#"{"type":"leaf","state":{"type":"footnotes","state":{},"icon":"lucide-file-signature","title":"Footnotes"}}"#
        );
        assert_eq!(
            dock_current_view(&json!({"right": right}), "right").as_deref(),
            Some("footnotes")
        );
    }

    /// A leaf pushed while another pane is showing leaves `currentTab`
    /// alone, and goes after a File properties leaf already there.
    #[test]
    fn footnotes_leaf_goes_last_and_leaves_another_current_tab_alone() {
        let mut right = default_dock_json("right", Some("A.md"), 1);
        push_file_properties_leaf(&mut right, Some("A.md"));
        assert!(push_footnotes_leaf(&mut right, false));
        let group = &right["children"][0];
        assert_eq!(group["currentTab"], 1);
        let kinds: Vec<&str> = group["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["state"]["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            [
                "backlink",
                "outgoing-link",
                "tag",
                "all-properties",
                "outline",
                "file-properties",
                "footnotes"
            ]
        );
        // A dock with no tab group has nowhere to put it.
        let mut empty = json!({"type": "split", "children": []});
        assert!(!push_footnotes_leaf(&mut empty, true));
    }

    /// `footnotes:open` is runnable, and belongs to the `footnotes` core
    /// plugin, so turning the plugin off takes it away.
    #[test]
    fn footnotes_command_is_runnable_and_gated_on_its_plugin() {
        assert!(RUNNABLE.contains(&"footnotes:open"));
        assert_eq!(
            scoria_commands::plugin_of("footnotes:open"),
            Some("footnotes")
        );
    }

    #[test]
    fn the_empty_state_is_obsidians_text() {
        assert_eq!(EMPTY, "No footnotes found.");
    }
}
