//! The local graph as a tab, and the linked leaves it rests on, as Obsidian
//! 1.15.1 makes them (`tools/obsidian-probe/results/2026-11-22-local-graph-pane.json`,
//! see [[Specification#Observed local graph pane, Obsidian 1.15.1]]).
//!
//! **Linked leaves.** A tab may carry a link id, the leaf's `group` in
//! `workspace.json`: tabs sharing one are linked. Here the only use is the
//! local graph's — a `localgraph` tab follows the file of the other tab in
//! its link — but the id is read and written for every tab, so a tab
//! Obsidian linked keeps its link through Scoria, and "Link with tab" and
//! "Open linked view" can be built on it.
//!
//! **The tab.** `graph:open-local` links the active tab (giving it an id if
//! it has none) and splits the main area to the right with a new group
//! holding one `localgraph` tab in that link, active, the keyboard left
//! where it was; every run makes another group. The tab is titled `Graph
//! of <name>` and centres on its linked tab's file. When no other tab is
//! left in its link it is unlinked: its `group` goes, its title becomes
//! `Local graph`, and it follows the active file, as Obsidian's did once
//! its note's leaf closed. Its settings are its leaf's `options`.
//!
//! Like a release-notes tab, a local-graph tab is one with no file and no
//! history whose leaf is a `localgraph` leaf; its view is a [`GraphView`]
//! of its own per leaf id, made when the tab is first synced.

use super::*;
use scoria_formats::local_graph;

/// Whether `tab` is a local graph's.
pub(super) fn is_local_graph(tab: &Tab) -> bool {
    tab.path.is_none()
        && tab.history.is_empty()
        && tab.mode != ViewMode::Graph
        && tab
            .leaf
            .as_ref()
            .and_then(|l| l.pointer("/state/type"))
            .and_then(Value::as_str)
            == Some(local_graph::VIEW_TYPE)
}

/// The leaf id a local-graph tab's view is kept under.
pub(super) fn view_key(tab: &Tab) -> Option<String> {
    is_local_graph(tab)
        .then(|| tab.leaf.as_ref()?.get("id")?.as_str().map(str::to_string))
        .flatten()
}

/// The file a local-graph tab's leaf centres on.
pub(super) fn file_of(tab: &Tab) -> Option<String> {
    tab.leaf
        .as_ref()?
        .pointer("/state/state/file")?
        .as_str()
        .map(str::to_string)
}

/// The tab's title: `Graph of <name>` while linked to a file, `Local graph`
/// otherwise (observed; a canvas gave `Graph of Board`, an image `Graph of
/// pic`).
pub(super) fn title(tab: &Tab) -> String {
    match (&tab.link, file_of(tab)) {
        (Some(_), Some(file)) => format!("Graph of {}", stem(&file)),
        _ => local_graph::UNLINKED_TITLE.to_string(),
    }
}

/// A path's file name without its extension.
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

/// A new local-graph tab in `link`, centred on `file`, on the default
/// options.
pub(super) fn new_tab(link: String, file: Option<&str>, mode: ViewMode) -> Tab {
    let mut tab = Tab {
        leaf: Some(local_graph::new_leaf(
            &leaf_id(),
            file,
            &local_graph::LocalGraphOptions::default(),
        )),
        link: Some(link),
        ..Tab::empty(mode)
    };
    let title = title(&tab);
    if let Some(state) = tab
        .leaf
        .as_mut()
        .and_then(|l| l.get_mut("state"))
        .and_then(Value::as_object_mut)
    {
        state.insert("title".into(), json!(title));
    }
    tab
}

/// Writes a tab's link into its leaf as Obsidian does: `group` after the
/// leaf's other keys, rewritten in place when it is there, gone when the
/// tab has no link.
pub(super) fn set_group(leaf: &mut Value, link: Option<&str>) {
    let Some(object) = leaf.as_object_mut() else {
        return;
    };
    match link {
        Some(link) => {
            object.insert("group".into(), json!(link));
        }
        None => {
            object.shift_remove("group");
        }
    }
}

/// What a local graph follows when nothing links it: the file the main
/// area shows; `None` to keep what it has, which is what it does while a
/// local graph is itself the active tab.
pub(super) enum Follow {
    Keep,
    File(Option<String>),
}

impl Workspace {
    /// The active file as a local graph sees it.
    pub(super) fn local_graph_follow(&self) -> Follow {
        if self.tabs.get(self.active_tab).is_some_and(is_local_graph) && self.current.is_none() {
            return Follow::Keep;
        }
        if self.mode == ViewMode::Graph {
            return Follow::File(None);
        }
        Follow::File(self.current.clone())
    }

    /// Every tab of every group, with its group's index, the active group's
    /// from the live fields.
    fn all_tab_snapshots(&self) -> Vec<(usize, Vec<Tab>)> {
        self.groups
            .iter()
            .enumerate()
            .map(|(ix, g)| {
                if ix == self.active_group {
                    (ix, self.tab_snapshots())
                } else {
                    (ix, g.tabs.clone())
                }
            })
            .collect()
    }

    /// Brings every local-graph tab up to date: the file it centres on (its
    /// linked tab's, or the active file once unlinked), its title, its view
    /// (made the first time) and the options its view holds, written into
    /// its leaf; views of tabs that are gone are dropped.
    pub(super) fn sync_local_graph_tabs(&mut self, cx: &mut Context<Self>) {
        let snapshots = self.all_tab_snapshots();
        let mut sources: HashMap<String, Option<String>> = HashMap::new();
        let mut any = false;
        for (_, tabs) in &snapshots {
            for tab in tabs {
                if is_local_graph(tab) {
                    any = true;
                } else if let Some(link) = &tab.link {
                    sources
                        .entry(link.clone())
                        .or_insert_with(|| tab.path.clone());
                }
            }
        }
        if !any && self.local_graph_tabs.is_empty() {
            return;
        }
        let follow = self.local_graph_follow();
        let mut seen: Vec<String> = Vec::new();
        let active_group = self.active_group;
        for gi in 0..self.groups.len() {
            let count = if gi == active_group {
                self.tabs.len()
            } else {
                self.groups[gi].tabs.len()
            };
            for ti in 0..count {
                let tab = if gi == active_group {
                    self.tabs[ti].clone()
                } else {
                    self.groups[gi].tabs[ti].clone()
                };
                let tab = &tab;
                let Some(key) = view_key(tab) else {
                    continue;
                };
                let link = tab.link.clone();
                let had = file_of(tab);
                let (link, file) = match link {
                    Some(link) if sources.contains_key(&link) => {
                        let file = sources[&link].clone();
                        (Some(link), file)
                    }
                    _ => match &follow {
                        Follow::Keep => (None, had),
                        Follow::File(file) => (None, file.clone()),
                    },
                };
                let view = self.local_graph_tab_view(&key, tab, cx);
                view.update(cx, |g, cx| g.set_current(file.clone(), cx));
                let options = view.read(cx).pane_options();
                let tab = if gi == active_group {
                    &mut self.tabs[ti]
                } else {
                    &mut self.groups[gi].tabs[ti]
                };
                tab.link = link;
                let title = {
                    if let Some(inner) = tab
                        .leaf
                        .as_mut()
                        .and_then(|l| l.pointer_mut("/state/state"))
                        .and_then(Value::as_object_mut)
                    {
                        local_graph::write_state(inner, file.as_deref(), &options);
                    }
                    title(tab)
                };
                if let Some(state) = tab
                    .leaf
                    .as_mut()
                    .and_then(|l| l.get_mut("state"))
                    .and_then(Value::as_object_mut)
                {
                    if state.get("title").and_then(Value::as_str) != Some(title.as_str()) {
                        state.insert("title".into(), json!(title));
                    }
                }
                seen.push(key);
            }
        }
        self.local_graph_tabs.retain(|key, _| seen.contains(key));
    }

    /// The view of the local-graph tab under `key`, made with the leaf's
    /// options the first time.
    fn local_graph_tab_view(
        &mut self,
        key: &str,
        tab: &Tab,
        cx: &mut Context<Self>,
    ) -> Entity<GraphView> {
        if let Some((view, _)) = self.local_graph_tabs.get(key) {
            return view.clone();
        }
        let options =
            local_graph::read_state(tab.leaf.as_ref().and_then(|l| l.pointer("/state/state")));
        let view = cx.new(GraphView::new_pane);
        view.update(cx, |g, cx| {
            if let Some(state) = self.state.clone() {
                g.set_state(state, cx);
            }
            g.set_pane_options(&options, cx);
        });
        let owner = key.to_string();
        let subscription = cx.subscribe(&view, move |this, _, event: &GraphEvent, cx| {
            this.local_graph_tab_event(&owner, event, cx)
        });
        self.local_graph_tabs
            .insert(key.to_string(), (view.clone(), subscription));
        view
    }

    /// A click or a hover in a local-graph tab's view.
    fn local_graph_tab_event(&mut self, key: &str, event: &GraphEvent, cx: &mut Context<Self>) {
        match event {
            GraphEvent::Open(path) => self.open_from_graph_tab(Some(key), path.clone(), cx),
            // A new tab in the graph's own group, where Obsidian's went.
            GraphEvent::OpenInNewTab(path) => self.open_link_in_new_tab(path.clone(), None, cx),
            GraphEvent::Hover(Some(hover)) => {
                let where_ = self.window_of_group(self.groups[self.active_group].id);
                self.row_hover(
                    crate::hover_preview::Source::Graph,
                    where_,
                    crate::hover_preview::exact(&hover.path),
                    hover.path.clone(),
                    true,
                    hover.anchor,
                    hover.modifiers,
                    cx,
                )
            }
            GraphEvent::Hover(None) => self.source_left(crate::hover_preview::Source::Graph, cx),
        }
    }

    /// A node clicked in a local graph, a tab's (`key`) or the dock pane's:
    /// the file opens in the tab the graph is linked to (Obsidian's linked
    /// pane put Gamma in its note's leaf, `s07`), else in the main area's
    /// last tab that is not a local graph, never over the graph itself.
    pub(super) fn open_from_graph_tab(
        &mut self,
        key: Option<&str>,
        path: String,
        cx: &mut Context<Self>,
    ) {
        let snapshots = self.all_tab_snapshots();
        let link = key.and_then(|key| {
            snapshots
                .iter()
                .flat_map(|(_, tabs)| tabs.iter())
                .find(|t| view_key(t).as_deref() == Some(key))
                .and_then(|t| t.link.clone())
        });
        let linked = link.and_then(|link| {
            snapshots.iter().find_map(|(gi, tabs)| {
                tabs.iter()
                    .position(|t| !is_local_graph(t) && t.link.as_deref() == Some(link.as_str()))
                    .map(|ti| (*gi, ti))
            })
        });
        let target = linked.or_else(|| {
            // The group used last whose shown tab is not a local graph.
            self.group_mru.iter().find_map(|id| {
                let gi = self.group_ix(*id)?;
                let (_, tabs) = snapshots.iter().find(|(g, _)| *g == gi)?;
                let active = if gi == self.active_group {
                    self.active_tab
                } else {
                    self.groups[gi].active_tab
                };
                tabs.get(active)
                    .filter(|t| !is_local_graph(t))
                    .map(|_| (gi, active))
            })
        });
        let Some((gi, ti)) = target else {
            self.open_link_in_new_tab(path, None, cx);
            return;
        };
        let id = self.groups[gi].id;
        self.activate_group(id, cx);
        self.activate_tab(ti, cx);
        if self.mode == ViewMode::Graph {
            self.mode = self.mode_before_graph;
        }
        self.open_file(path, None, cx);
        self.request_focus_main(cx);
    }

    /// `graph:open-local`: the active tab linked (an id made for it if it
    /// had none) and a new group to its right holding a local graph in the
    /// same link, active; the keyboard stays where it was (`s01`, `s02`).
    pub(super) fn open_linked_local_graph(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.current.clone() else {
            return;
        };
        let link = match self.tabs.get_mut(self.active_tab) {
            Some(tab) => tab.link.get_or_insert_with(leaf_id).clone(),
            None => return,
        };
        let focused = window.focused(cx);
        self.flush_save(cx);
        self.stash_active_group();
        let from = self.active_group_id();
        let mode = self.default_mode();
        let tab = new_tab(link, Some(&file), mode);
        let id = self.new_group(mode, window, cx);
        self.place_beside(from, id, SplitDirection::Vertical, true);
        let Some(ix) = self.group_ix(id) else {
            return;
        };
        self.groups[ix].tabs = vec![tab];
        self.groups[ix].active_tab = 0;
        self.load_group(ix, cx);
        self.sync_local_graph_tabs(cx);
        if let Some(handle) = focused {
            window.focus(&handle, cx);
            self.refocus_main = false;
        }
        cx.notify();
    }

    /// What a group shows when its tab is a local graph's.
    pub(super) fn local_graph_tab_element(&self, tab: &Tab) -> Option<gpui::AnyElement> {
        let key = view_key(tab)?;
        let (view, _) = self.local_graph_tabs.get(&key)?;
        Some(view.clone().into_any_element())
    }

    /// The local-graph tabs for the control socket's state dump.
    pub(super) fn describe_local_graph_tabs(&self, cx: &App) -> Value {
        let tabs: Vec<Value> = self
            .all_tab_snapshots()
            .iter()
            .flat_map(|(gi, tabs)| tabs.iter().map(move |t| (*gi, t)))
            .filter_map(|(gi, tab)| {
                let key = view_key(tab)?;
                let mut value = self
                    .local_graph_tabs
                    .get(&key)
                    .map(|(view, _)| view.read(cx).describe())
                    .unwrap_or(Value::Null);
                if let Some(object) = value.as_object_mut() {
                    object.insert("id".into(), json!(key));
                    object.insert("group".into(), json!(self.groups[gi].id));
                    object.insert("link".into(), json!(tab.link));
                    object.insert("file".into(), json!(file_of(tab)));
                    object.insert("title".into(), json!(title(tab)));
                }
                Some(value)
            })
            .collect();
        Value::Array(tabs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_tab_is_obsidians_linked_leaf() {
        let tab = new_tab(
            "e071386aa5650ab0".into(),
            Some("Alpha.md"),
            ViewMode::LivePreview,
        );
        assert!(is_local_graph(&tab));
        assert_eq!(title(&tab), "Graph of Alpha");
        let mut leaf = leaf_json(&tab);
        assert_eq!(leaf.pointer("/state/type"), Some(&json!("localgraph")));
        assert_eq!(leaf.pointer("/state/title"), Some(&json!("Graph of Alpha")));
        assert_eq!(leaf.pointer("/state/state/file"), Some(&json!("Alpha.md")));
        let keys: Vec<&str> = leaf
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["id", "type", "state", "group"]);
        assert_eq!(leaf["group"], json!("e071386aa5650ab0"));
        set_group(&mut leaf, None);
        assert!(leaf.get("group").is_none());
    }

    #[test]
    fn titles_follow_the_link_and_the_file() {
        let mut tab = new_tab("g".into(), Some("Board.canvas"), ViewMode::Reading);
        assert_eq!(title(&tab), "Graph of Board");
        tab.link = None;
        assert_eq!(title(&tab), "Local graph");
        assert_eq!(stem("dir/pic.png"), "pic");
        assert_eq!(stem(".hidden"), ".hidden");
    }

    #[test]
    fn an_obsidian_leaf_restores_as_a_linked_local_graph_and_writes_back_the_same() {
        let text = r#"{"id": "c99c59cfbd7bc651", "type": "leaf", "state": {"type": "localgraph",
            "state": {"file": "Alpha.md", "options": {"collapse-filter": true, "search": "",
            "localJumps": 1, "localBacklinks": true, "localForelinks": true,
            "localInterlinks": false, "showTags": false, "showAttachments": false,
            "hideUnresolved": false, "collapse-color-groups": true, "colorGroups": [],
            "collapse-display": true, "showArrow": false, "textFadeMultiplier": 0,
            "nodeSizeMultiplier": 1, "lineSizeMultiplier": 1, "collapse-forces": true,
            "centerStrength": 0.518713248970312, "repelStrength": 10, "linkStrength": 1,
            "linkDistance": 250, "scale": 1, "close": false, "scoriaUnknown": 1}},
            "icon": "lucide-git-fork", "title": "Graph of Alpha"},
            "group": "e071386aa5650ab0", "scoriaLeaf": true}"#;
        let leaf: Value = serde_json::from_str(text).unwrap();
        let (id, tab) = tab_from_leaf(&leaf, ViewMode::LivePreview, &|_| true).unwrap();
        assert_eq!(id, "c99c59cfbd7bc651");
        assert!(is_local_graph(&tab));
        assert_eq!(tab.link.as_deref(), Some("e071386aa5650ab0"));
        assert_eq!(file_of(&tab).as_deref(), Some("Alpha.md"));
        assert_eq!(
            serde_json::to_string(&leaf_json(&tab)).unwrap(),
            serde_json::to_string(&leaf).unwrap()
        );
    }

    #[test]
    fn a_note_leafs_group_is_read_and_written() {
        let text = r#"{"id": "bfc50330929fe9ad", "type": "leaf", "state": {"type": "markdown",
            "state": {"file": "Alpha.md", "mode": "source", "source": false},
            "icon": "lucide-file", "title": "Alpha"}, "group": "e071386aa5650ab0"}"#;
        let leaf: Value = serde_json::from_str(text).unwrap();
        let (_, tab) = tab_from_leaf(&leaf, ViewMode::LivePreview, &|_| true).unwrap();
        assert!(!is_local_graph(&tab));
        assert_eq!(tab.link.as_deref(), Some("e071386aa5650ab0"));
        assert_eq!(
            serde_json::to_string(&leaf_json(&tab)).unwrap(),
            serde_json::to_string(&leaf).unwrap()
        );
        // A tab moved on to another file gets a new leaf, and keeps its link.
        let mut moved = tab.clone();
        moved.path = Some("Beta.md".into());
        assert_eq!(leaf_json(&moved)["group"], json!("e071386aa5650ab0"));
    }
}
