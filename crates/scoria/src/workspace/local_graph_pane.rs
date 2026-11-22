//! The Local graph pane: a `localgraph` leaf in the right dock, as Obsidian
//! 1.15.1 keeps one there (`tools/obsidian-probe/results/2026-11-22-local-graph-pane.json`,
//! see [[Specification#Observed local graph pane, Obsidian 1.15.1]]).
//!
//! The pane is a [`GraphView`] of its own, always local, centred on the
//! main area's active file and on nothing over an empty tab or the global
//! graph. Its Depth, direction toggles and every other setting are its
//! leaf's `options`, read when the layout is and written back into the
//! leaf with it ([`scoria_formats::local_graph`]); `graph.json` is never
//! touched. A click on a node opens the file in the main area's active
//! tab, Mod+click in a new tab.
//!
//! `graph:open-local` makes the leaf (last in the dock's first tab group)
//! or reveals it, and expands the dock. That is where Scoria departs from
//! Obsidian on purpose: Obsidian's command splits the main area with a
//! linked tab, and its sidebar leaf is the one that tab becomes when
//! dragged into the dock; Scoria has neither linked tabs nor drags into a
//! dock, so the command makes the sidebar leaf directly.

use super::*;
use scoria_formats::local_graph;
#[cfg(test)]
use scoria_formats::local_graph::LocalGraphOptions;

impl Workspace {
    /// The file the pane centres on: the main area's open file, none while
    /// the global graph is on screen (Obsidian's pane emptied when `Ctrl+G`
    /// replaced the note, `s06`).
    pub(super) fn local_graph_file(&self) -> Option<String> {
        if self.mode == ViewMode::Graph {
            return None;
        }
        self.current.clone()
    }

    /// `graph:open-local`: the pane made or revealed, the dock expanded.
    /// The keyboard stays where it was, as Obsidian left it in the note.
    pub(super) fn open_local_graph_pane(&mut self, cx: &mut Context<Self>) {
        self.right_visible = true;
        self.local_graph_open = true;
        self.sync_local_graph(cx);
        self.select_right_tab(RightTab::LocalGraph, cx);
    }

    /// Hands the pane the file it centres on; a no-op when it has it.
    pub(super) fn sync_local_graph(&mut self, cx: &mut Context<Self>) {
        let file = self.local_graph_file();
        self.local_graph.update(cx, |g, cx| g.set_current(file, cx));
    }

    /// The pane's leaf options from `workspace.json`'s right dock, when it
    /// has a `localgraph` leaf.
    pub(super) fn restore_local_graph(&mut self, layout: &Value, cx: &mut Context<Self>) {
        let leaf = layout
            .get("right")
            .and_then(|right| leaf_of_kind(right, local_graph::VIEW_TYPE));
        self.local_graph_open = leaf.is_some();
        if let Some(leaf) = leaf {
            let options = local_graph::read_state(leaf.get("state").and_then(|s| s.get("state")));
            self.local_graph
                .update(cx, |g, cx| g.set_pane_options(&options, cx));
        }
    }

    /// The pane's leaf in the right dock as the layout is written: made at
    /// the end of the first tab group when it is new, its `file` and
    /// `options` rewritten in place otherwise, every other key kept.
    pub(super) fn write_local_graph_leaf(&self, right: &mut Value, cx: &App) {
        if !self.local_graph_open {
            file_properties_pane::remove_leaf(right, local_graph::VIEW_TYPE);
            return;
        }
        let options = self.local_graph.read(cx).pane_options();
        let file = self.local_graph_file();
        if leaf_of_kind(right, local_graph::VIEW_TYPE).is_none() {
            let leaf = local_graph::new_leaf(&leaf_id(), file.as_deref(), &options);
            if push_dock_leaf(right, leaf) && self.right_tab == RightTab::LocalGraph {
                file_properties_pane::point_current_tab_at(right, local_graph::VIEW_TYPE);
            }
            return;
        }
        if let Some(state) = leaf_of_kind_mut(right, local_graph::VIEW_TYPE)
            .and_then(|leaf| leaf.get_mut("state"))
            .and_then(Value::as_object_mut)
        {
            let inner = state
                .entry("state")
                .or_insert_with(|| Value::Object(Map::new()));
            if !inner.is_object() {
                *inner = Value::Object(Map::new());
            }
            if let Some(inner) = inner.as_object_mut() {
                local_graph::write_state(inner, file.as_deref(), &options);
            }
        }
    }

    /// A node clicked in the pane: its file in the main area's active tab,
    /// leaving the global graph if that is what the tab showed, the
    /// keyboard with it (`s07`).
    pub(super) fn open_from_local_graph(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode == ViewMode::Graph {
            let back = self.mode_before_graph;
            self.set_mode(back, window, cx);
        }
        self.open_file(path, None, cx);
        self.focus_main(window, cx);
    }

    /// The pane: the graph, or Obsidian's empty canvas with its controls
    /// when there is no file to centre on.
    pub(super) fn render_local_graph_pane(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.sync_local_graph(cx);
        div()
            .id("local-graph-pane")
            .size_full()
            .child(self.local_graph.clone())
            .into_any_element()
    }

    /// The pane for the control socket's state dump.
    pub(super) fn describe_local_graph(&self, cx: &App) -> Value {
        let mut value = self.local_graph.read(cx).describe();
        if let Some(object) = value.as_object_mut() {
            object.insert("open".into(), json!(self.local_graph_open));
            object.insert("file".into(), json!(self.local_graph_file()));
        }
        value
    }
}

/// Appends `leaf` to a dock's first tab group, where Obsidian put the
/// dragged tab (`right/split[0]/tabs[5]`, after Outline). `false` when the
/// dock has no tab group.
pub(super) fn push_dock_leaf(dock: &mut Value, leaf: Value) -> bool {
    fn first_tabs(node: &mut Value) -> Option<&mut Value> {
        if node.get("type").and_then(Value::as_str) == Some("tabs") {
            return Some(node);
        }
        node.get_mut("children")?
            .as_array_mut()?
            .iter_mut()
            .find_map(first_tabs)
    }
    let Some(children) = first_tabs(dock)
        .and_then(|group| group.get_mut("children"))
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    children.push(leaf);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_leaf_goes_last_in_the_first_tab_group() {
        let mut dock = json!({
            "id": "r", "type": "split",
            "children": [{"id": "t", "type": "tabs", "currentTab": 4,
                "children": [{"id": "a", "type": "leaf", "state": {"type": "backlink"}},
                             {"id": "b", "type": "leaf", "state": {"type": "outline"}}]}],
            "direction": "horizontal", "width": 300
        });
        let leaf = local_graph::new_leaf("n", Some("Alpha.md"), &LocalGraphOptions::default());
        assert!(push_dock_leaf(&mut dock, leaf));
        let kids = dock
            .pointer("/children/0/children")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(kids.len(), 3);
        assert_eq!(kids[2].pointer("/state/type"), Some(&json!("localgraph")));
        assert_eq!(kids[2].pointer("/state/title"), Some(&json!("Local graph")));
        file_properties_pane::point_current_tab_at(&mut dock, "localgraph");
        assert_eq!(dock.pointer("/children/0/currentTab"), Some(&json!(2)));
        assert!(file_properties_pane::remove_leaf(&mut dock, "localgraph"));
        assert_eq!(dock.pointer("/children/0/currentTab"), Some(&json!(1)));
    }
}
