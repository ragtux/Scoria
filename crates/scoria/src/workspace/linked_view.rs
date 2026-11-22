//! "Open linked view": the Backlinks, Outgoing links, File properties and
//! Outline views as leaves of the main area, centred on one note, as
//! Obsidian 1.15.1 makes them (`tools/obsidian-probe/open-linked-view.mjs`,
//! results `tools/obsidian-probe/results/2026-11-22-open-linked-view.json`,
//! see [[Specification#Observed Open linked view, Obsidian 1.15.1]]).
//!
//! **The submenu.** The view header's More options and the tab's menu carry
//! "Open linked view" ([`file_menu::linked_view_row`]): on a note Open local
//! graph, Open backlinks, Open outgoing links, Open file properties and Open
//! outline; on a canvas, a PDF, an image or a base Open backlinks alone. A
//! row links the note's tab (giving it a link id if it has none, the same
//! id [`super::local_graph_tab`] uses) and splits the main area beside it
//! with a new group holding the view in that link, active: Backlinks,
//! Outgoing links and File properties **below** the note, Outline (and the
//! local graph) to its **right**. Every run makes another group, put
//! directly beside the note. The keyboard stays where it was, but for File
//! properties, which takes it.
//!
//! **The commands.** `backlink:open-backlinks`,
//! `outgoing-links:open-for-current` and `outline:open-for-current` make the
//! same view **unlinked**, in a new group right of the active one.
//!
//! **Following.** A linked view follows its note's tab, whatever that tab
//! opens (a canvas gives `Backlinks for Board`); once no note tab is left
//! in its link it is unlinked, titled plainly (`Backlinks`), and follows
//! the active file, dropping its `file` key while no file is active. That
//! one-way rule is the local graph's, and both go through
//! [`Workspace::link_sources`] and [`Workspace::follower_follow`].
//!
//! **The leaf.** Its `state` is Obsidian's object for the view, key for key
//! (`file` first): a view keeps its own collapse, sort, search and folds
//! there, apart from the dock's, and takes them along when its note moves
//! on. Scoria holds that object as the tab's [`DocBacklinks`] options, so
//! the Backlinks view runs on the Backlinks-in-document block's own
//! handlers ([`SectionsOwner::Doc`] of its group), and writes it back into
//! the leaf with every unknown key kept. A row clicked in a view opens in
//! its note's tab, which becomes active (an outline row moves that note's
//! cursor).

use super::*;
use crate::file_menu::LinkedRow;

/// The views a linked leaf can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkedKind {
    Backlinks,
    Outgoing,
    FileProperties,
    Outline,
}

impl LinkedKind {
    pub(super) const ALL: [LinkedKind; 4] = [
        LinkedKind::Backlinks,
        LinkedKind::Outgoing,
        LinkedKind::FileProperties,
        LinkedKind::Outline,
    ];

    /// The leaf's `state.type`.
    pub(super) fn view_type(self) -> &'static str {
        match self {
            LinkedKind::Backlinks => "backlink",
            LinkedKind::Outgoing => "outgoing-link",
            LinkedKind::FileProperties => "file-properties",
            LinkedKind::Outline => "outline",
        }
    }

    pub(super) fn from_view_type(view: &str) -> Option<LinkedKind> {
        LinkedKind::ALL.into_iter().find(|k| k.view_type() == view)
    }

    /// The submenu row that opens it; the local graph's row is
    /// [`super::local_graph_tab`]'s.
    pub(super) fn from_row(row: LinkedRow) -> Option<LinkedKind> {
        match row {
            LinkedRow::LocalGraph => None,
            LinkedRow::Backlinks => Some(LinkedKind::Backlinks),
            LinkedRow::OutgoingLinks => Some(LinkedKind::Outgoing),
            LinkedRow::FileProperties => Some(LinkedKind::FileProperties),
            LinkedRow::Outline => Some(LinkedKind::Outline),
        }
    }

    /// The leaf's `icon`.
    fn icon(self) -> &'static str {
        match self {
            LinkedKind::Backlinks => "links-coming-in",
            LinkedKind::Outgoing => "links-going-out",
            LinkedKind::FileProperties => "lucide-info",
            LinkedKind::Outline => "lucide-list",
        }
    }

    /// The title of an unlinked leaf (`Backlinks`), and the view header's.
    pub(super) fn name(self) -> &'static str {
        match self {
            LinkedKind::Backlinks => "Backlinks",
            LinkedKind::Outgoing => "Outgoing links",
            LinkedKind::FileProperties => "File properties",
            LinkedKind::Outline => "Outline",
        }
    }

    /// A linked leaf's title over the file `name` (its basename without
    /// the extension): `Backlinks for Alpha`, `Outgoing links from Alpha`,
    /// `File properties for Alpha`, `Outline of Alpha` (observed).
    fn title_of(self, name: &str) -> String {
        let joint = match self {
            LinkedKind::Backlinks | LinkedKind::FileProperties => "for",
            LinkedKind::Outgoing => "from",
            LinkedKind::Outline => "of",
        };
        format!("{} {joint} {name}", self.name())
    }

    /// A fresh leaf's `state` after its `file`, in Obsidian's key order
    /// (observed, `s02`).
    fn default_state(self) -> &'static str {
        match self {
            LinkedKind::Backlinks => {
                r#"{"collapseAll": false, "extraContext": false, "sortOrder": "alphabetical", "showSearch": false, "searchQuery": "", "backlinkCollapsed": false, "unlinkedCollapsed": true}"#
            }
            LinkedKind::Outgoing => r#"{"linksCollapsed": false, "unlinkedCollapsed": true}"#,
            LinkedKind::FileProperties => "{}",
            LinkedKind::Outline => {
                r#"{"followCursor": false, "showSearch": false, "searchQuery": ""}"#
            }
        }
    }

    /// Where a row of the submenu puts the view: below the note, or to its
    /// right for the Outline (observed, `s02`).
    fn direction(self) -> SplitDirection {
        match self {
            LinkedKind::Outline => SplitDirection::Vertical,
            _ => SplitDirection::Horizontal,
        }
    }
}

/// Which linked view `tab` is, if it is one: a tab with no file and no
/// history whose leaf is one of the four views.
pub(super) fn kind_of(tab: &Tab) -> Option<LinkedKind> {
    if tab.path.is_some() || !tab.history.is_empty() || tab.mode == ViewMode::Graph {
        return None;
    }
    tab.leaf
        .as_ref()?
        .pointer("/state/type")
        .and_then(Value::as_str)
        .and_then(LinkedKind::from_view_type)
}

/// Whether `tab` shows another tab's file rather than one of its own: a
/// local graph or a linked view. A link's note is its tab that is not one.
pub(super) fn is_follower(tab: &Tab) -> bool {
    local_graph_tab::is_local_graph(tab) || kind_of(tab).is_some()
}

/// The file a linked view is centred on.
pub(super) fn file_of(tab: &Tab) -> Option<String> {
    tab.doc_backlinks
        .opts()
        .and_then(|o| o.get("file"))
        .or_else(|| tab.leaf.as_ref()?.pointer("/state/state/file"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The tab's title: `Backlinks for <name>` while linked to a file, the
/// view's plain name otherwise.
pub(super) fn title(tab: &Tab) -> String {
    let Some(kind) = kind_of(tab) else {
        return String::new();
    };
    match (&tab.link, file_of(tab)) {
        (Some(_), Some(file)) => kind.title_of(stem(&file)),
        _ => kind.name().to_string(),
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

/// The view's state object with `file` set: in place when it is there,
/// first otherwise, and taken out with no file (as Obsidian's unlinked
/// Backlinks leaf did over an empty tab, `s07`).
fn with_file(mut state: Map<String, Value>, file: Option<&str>) -> Map<String, Value> {
    match file {
        Some(file) => match state.get_mut("file") {
            Some(slot) => *slot = json!(file),
            None => {
                state.shift_insert(0, "file".into(), json!(file));
            }
        },
        None => {
            state.shift_remove("file");
        }
    }
    state
}

/// A new view tab of `kind` on `file`, linked by `link` or unlinked.
pub(super) fn new_tab(
    kind: LinkedKind,
    link: Option<String>,
    file: Option<&str>,
    mode: ViewMode,
) -> Tab {
    let state: Map<String, Value> = serde_json::from_str(kind.default_state()).unwrap_or_default();
    let state = with_file(state, file);
    let mut tab = Tab {
        leaf: Some(json!({
            "id": leaf_id(),
            "type": "leaf",
            "state": {
                "type": kind.view_type(),
                "state": Value::Object(state.clone()),
                "icon": kind.icon(),
                "title": "",
            },
        })),
        link,
        doc_backlinks: DocBacklinks::for_view(state),
        ..Tab::empty(mode)
    };
    let title = title(&tab);
    if let Some(slot) = tab
        .leaf
        .as_mut()
        .and_then(|l| l.pointer_mut("/state/title"))
    {
        *slot = json!(title);
    }
    tab
}

/// The tab a view's `workspace.json` leaf restores as: the leaf kept
/// whole, its `state` held as the view's options.
pub(super) fn restore(leaf: &Value, mode: ViewMode) -> Tab {
    let state = leaf
        .pointer("/state/state")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Tab {
        leaf: Some(leaf.clone()),
        doc_backlinks: DocBacklinks::for_view(state),
        ..Tab::empty(mode)
    }
}

/// The session-only state of the views of the main area, by leaf id.
#[derive(Default)]
pub(super) struct LinkedViews {
    /// The unlinked mentions found in a view's file when it is not the
    /// open note (whose are the dock's): what Outgoing links lists. The
    /// incoming ones go into the workspace's `doc_mentions`.
    outgoing: HashMap<String, Section>,
    /// Paths whose mentions are being looked for, and the ones found.
    pending: HashSet<String>,
    found: HashSet<String>,
    /// An Outline view's folds, by leaf id; not written (the dock's are
    /// not either).
    folds: HashMap<String, HashSet<usize>>,
    /// A File properties view's own view and keyboard, by leaf id.
    properties: HashMap<String, (Entity<LinkedPropertiesView>, FocusHandle)>,
}

impl LinkedViews {
    /// Mentions go stale when the vault changes: looked for again.
    pub(super) fn forget_mentions(&mut self, doc_mentions: &mut HashMap<String, Section>) {
        for path in self.found.drain() {
            doc_mentions.remove(&path);
        }
        self.outgoing.clear();
        self.pending.clear();
    }
}

/// A File properties view's body: drawn by the workspace, which needs the
/// window to lay the rows out.
pub(super) struct LinkedPropertiesView {
    ws: gpui::WeakEntity<Workspace>,
    key: String,
}

impl Render for LinkedPropertiesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let key = self.key.clone();
        self.ws
            .update(cx, |ws, cx| ws.linked_properties_element(&key, window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

/// The leaf id a view tab is kept under.
fn view_key(tab: &Tab) -> Option<String> {
    kind_of(tab)?;
    tab.leaf.as_ref()?.get("id")?.as_str().map(str::to_string)
}

impl Workspace {
    /// Every tab of every group with its group's index, the active group's
    /// from the live fields.
    pub(super) fn every_tab(&self) -> Vec<(usize, Vec<Tab>)> {
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

    /// The file each link's note shows, by link id: the first tab in the
    /// link that is not a follower and shows a file, the one file
    /// [`super::linked_tabs`] keeps every note tab of the link on (an empty
    /// tab or the global graph gives none). What a local graph and a linked
    /// view both follow; a link with no note tab left has no entry.
    pub(super) fn link_sources(&self) -> HashMap<String, Option<String>> {
        let mut sources: HashMap<String, Option<String>> = HashMap::new();
        for (_, tabs) in self.every_tab() {
            for tab in tabs {
                if is_follower(&tab) {
                    continue;
                }
                if let Some(link) = &tab.link {
                    let source = sources.entry(link.clone()).or_insert(None);
                    if source.is_none() && tab.mode != ViewMode::Graph {
                        source.clone_from(&tab.path);
                    }
                }
            }
        }
        sources
    }

    /// What an unlinked follower follows: the active file, none under the
    /// global graph, and what it has while a follower is the active tab.
    pub(super) fn follower_follow(&self) -> local_graph_tab::Follow {
        if self.tabs.get(self.active_tab).is_some_and(is_follower) && self.current.is_none() {
            return local_graph_tab::Follow::Keep;
        }
        if self.mode == ViewMode::Graph {
            return local_graph_tab::Follow::File(None);
        }
        local_graph_tab::Follow::File(self.current.clone())
    }

    /// Whether a local graph or a linked view is the active tab: the dock's
    /// Outline and File properties leaves then keep the file they had.
    pub(super) fn follower_is_active(&self) -> bool {
        self.current.is_none() && self.tabs.get(self.active_tab).is_some_and(is_follower)
    }

    /// The note tab of `link`, by group and tab index; else the group used
    /// last whose shown tab is not a follower. Where a click in a follower
    /// opens its file.
    pub(super) fn follower_target(&self, link: Option<&str>) -> Option<(usize, usize)> {
        let snapshots = self.every_tab();
        let linked = link.and_then(|link| {
            snapshots.iter().find_map(|(gi, tabs)| {
                tabs.iter()
                    .position(|t| !is_follower(t) && t.link.as_deref() == Some(link))
                    .map(|ti| (*gi, ti))
            })
        });
        linked.or_else(|| {
            self.group_mru.iter().find_map(|id| {
                let gi = self.group_ix(*id)?;
                let (_, tabs) = snapshots.iter().find(|(g, _)| *g == gi)?;
                let active = if gi == self.active_group {
                    self.active_tab
                } else {
                    self.groups[gi].active_tab
                };
                tabs.get(active)
                    .filter(|t| !is_follower(t))
                    .map(|_| (gi, active))
            })
        })
    }

    /// Makes the tab a follower's click should open in the active one;
    /// `false` when there is none.
    fn activate_follower_target(&mut self, link: Option<&str>, cx: &mut Context<Self>) -> bool {
        let Some((gi, ti)) = self.follower_target(link) else {
            return false;
        };
        let id = self.groups[gi].id;
        self.activate_group(id, cx);
        self.activate_tab(ti, cx);
        if self.mode == ViewMode::Graph {
            self.mode = self.mode_before_graph;
        }
        true
    }

    /// A row of a Backlinks or Outgoing links list clicked: when the list
    /// is a view leaf's, its note's tab is made active first, so the file
    /// opens there (observed, `s07`) rather than over the view.
    pub(super) fn linked_view_row_target(&mut self, owner: SectionsOwner, cx: &mut Context<Self>) {
        let (SectionsOwner::Doc(id) | SectionsOwner::Outgoing(id)) = owner else {
            return;
        };
        let Some(tab) = self.doc_tab(id).filter(|t| kind_of(t).is_some()) else {
            return;
        };
        let link = tab.link.clone();
        self.activate_follower_target(link.as_deref(), cx);
        self.request_focus_main(cx);
    }

    /// A row of the "Open linked view" submenu on the active tab.
    pub(super) fn open_linked_row(
        &mut self,
        row: LinkedRow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match LinkedKind::from_row(row) {
            Some(kind) => self.open_linked_view(kind, true, window, cx),
            None => self.open_linked_local_graph(window, cx),
        }
    }

    /// A view of `kind` on the active file in a new group: linked to the
    /// active tab and beside it (the submenu), or unlinked and to the right
    /// (the commands). The new group is active; the keyboard stays where
    /// it was, but for File properties, which takes it.
    pub(super) fn open_linked_view(
        &mut self,
        kind: LinkedKind,
        linked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(file) = self.current.clone() else {
            return;
        };
        let link = if linked {
            match self.tabs.get_mut(self.active_tab) {
                Some(tab) => Some(tab.link.get_or_insert_with(leaf_id).clone()),
                None => return,
            }
        } else {
            None
        };
        let focused = window.focused(cx);
        self.flush_save(cx);
        self.stash_active_group();
        let from = self.active_group_id();
        let mode = self.default_mode();
        let tab = new_tab(kind, link, Some(&file), mode);
        let key = view_key(&tab);
        let direction = if linked {
            kind.direction()
        } else {
            SplitDirection::Vertical
        };
        let id = self.new_group(mode, window, cx);
        self.place_beside(from, id, direction, true);
        let Some(ix) = self.group_ix(id) else {
            return;
        };
        self.groups[ix].tabs = vec![tab];
        self.groups[ix].active_tab = 0;
        self.load_group(ix, cx);
        self.sync_linked_views(cx);
        let pane_focus = (kind == LinkedKind::FileProperties)
            .then(|| key.and_then(|k| self.linked_views.properties.get(&k).map(|(_, f)| f.clone())))
            .flatten();
        match (pane_focus, focused) {
            (Some(handle), _) => {
                window.focus(&handle, cx);
                self.refocus_main = false;
            }
            (None, Some(handle)) => {
                window.focus(&handle, cx);
                self.refocus_main = false;
            }
            (None, None) => {}
        }
        cx.notify();
    }

    /// Brings every view tab up to date: the file it is centred on (its
    /// note's, or the active file once unlinked), its link, title and
    /// leaf; asks for the mentions its file needs; drops what tabs that
    /// are gone kept.
    pub(super) fn sync_linked_views(&mut self, cx: &mut Context<Self>) {
        let any = self.tabs.iter().any(|t| kind_of(t).is_some())
            || self
                .groups
                .iter()
                .any(|g| g.tabs.iter().any(|t| kind_of(t).is_some()));
        if !any && self.linked_views.properties.is_empty() && self.linked_views.folds.is_empty() {
            return;
        }
        let sources = self.link_sources();
        let follow = self.follower_follow();
        let active_group = self.active_group;
        let mut seen: Vec<String> = Vec::new();
        let mut properties: Vec<String> = Vec::new();
        let mut wanted: Vec<(LinkedKind, String)> = Vec::new();
        for gi in 0..self.groups.len() {
            let count = if gi == active_group {
                self.tabs.len()
            } else {
                self.groups[gi].tabs.len()
            };
            for ti in 0..count {
                let tab = if gi == active_group {
                    &mut self.tabs[ti]
                } else {
                    &mut self.groups[gi].tabs[ti]
                };
                let Some(kind) = kind_of(tab) else {
                    continue;
                };
                let had = file_of(tab);
                let (link, file) = match tab.link.clone() {
                    Some(link) if sources.contains_key(&link) => {
                        let file = sources[&link].clone();
                        (Some(link), file)
                    }
                    _ => match &follow {
                        local_graph_tab::Follow::Keep => (None, had),
                        local_graph_tab::Follow::File(file) => (None, file.clone()),
                    },
                };
                tab.link = link;
                let state = with_file(
                    tab.doc_backlinks.opts().cloned().unwrap_or_default(),
                    file.as_deref(),
                );
                tab.doc_backlinks.set_view_state(state.clone());
                let title = title(tab);
                if let Some(leaf) = tab.leaf.as_mut() {
                    if leaf.pointer("/state/state") != Some(&Value::Object(state.clone())) {
                        if let Some(slot) = leaf.pointer_mut("/state/state") {
                            *slot = Value::Object(state);
                        }
                    }
                    if leaf.pointer("/state/title").and_then(Value::as_str) != Some(title.as_str())
                    {
                        if let Some(object) = leaf.get_mut("state").and_then(Value::as_object_mut) {
                            object.insert("title".into(), json!(title));
                        }
                    }
                }
                if let Some(key) = view_key(tab) {
                    if kind == LinkedKind::FileProperties {
                        properties.push(key.clone());
                    }
                    seen.push(key);
                }
                if let Some(file) = file {
                    wanted.push((kind, file));
                }
            }
        }
        // A Backlinks or Outline view's filter field is its group's: it
        // shows the query of the view it shows.
        for gi in 0..self.groups.len() {
            let tab = if gi == active_group {
                self.tabs.get(self.active_tab)
            } else {
                self.groups[gi].tabs.get(self.groups[gi].active_tab)
            };
            let Some(query) = tab
                .filter(|t| kind_of(t).is_some())
                .map(|t| t.doc_backlinks.query().to_string())
            else {
                continue;
            };
            let group = &mut self.groups[gi];
            if group.doc_filter_synced != query {
                group.doc_filter_synced = query.clone();
                let input = group.doc_filter.clone();
                input.update(cx, |input, cx| input.set_text(query, cx));
            }
        }
        self.linked_views.folds.retain(|key, _| seen.contains(key));
        self.linked_views
            .properties
            .retain(|key, _| seen.contains(key));
        // A File properties view's body and keyboard, made the first time.
        for key in &properties {
            if !self.linked_views.properties.contains_key(key) {
                let ws = cx.weak_entity();
                let view = cx.new(|_| LinkedPropertiesView {
                    ws,
                    key: key.clone(),
                });
                let focus = cx.focus_handle();
                self.linked_views
                    .properties
                    .insert(key.clone(), (view, focus));
            }
        }
        for (kind, file) in wanted {
            if matches!(kind, LinkedKind::Backlinks | LinkedKind::Outgoing)
                && self.current.as_deref() != Some(file.as_str())
            {
                self.request_linked_mentions(&file, cx);
            }
        }
    }

    /// Looks for the unlinked mentions of and in `path` on the background
    /// executor, once, for a view whose file is not the open note.
    fn request_linked_mentions(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.linked_views.found.contains(path) || self.linked_views.pending.contains(path) {
            return;
        }
        let Some(state) = self.state.clone() else {
            return;
        };
        self.linked_views.pending.insert(path.to_string());
        let finder = self.name_finder.clone();
        let extra = self.backlink_state.extra_context;
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            let rows = cx
                .background_executor()
                .spawn(async move {
                    let finder = finder.or_else(|| {
                        let names = state.names();
                        let refs: Vec<Name<'_>> = names
                            .iter()
                            .map(|(text, path)| Name { text, path })
                            .collect();
                        Finder::new(&refs).map(Arc::new)
                    });
                    mention_sections(&state, &path, finder.as_deref(), extra)
                })
                .await;
            this.update(cx, |ws, cx| {
                if !ws.linked_views.pending.remove(&rows.path) {
                    return;
                }
                ws.linked_views.found.insert(rows.path.clone());
                ws.doc_mentions.insert(rows.path.clone(), rows.incoming);
                ws.linked_views.outgoing.insert(rows.path, rows.outgoing);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The Outgoing links view's two sections for `path`: the dock's when
    /// it is the open note, built here otherwise.
    fn linked_outgoing_sections(&self, path: &str) -> Vec<Section> {
        if self.current.as_deref() == Some(path) && self.outgoing_sections.len() == 2 {
            return self.outgoing_sections.clone();
        }
        let Some(state) = self.state.as_ref() else {
            return Vec::new();
        };
        let links = backlinks::outgoing_links(&state.cache(), path);
        let unlinked = self
            .linked_views
            .outgoing
            .get(path)
            .cloned()
            .unwrap_or_else(|| Section::empty(backlinks::UNLINKED));
        vec![links, unlinked]
    }

    /// The headings of `path` as the Outline lists them.
    fn linked_headings(&self, path: &str) -> Vec<panes::Heading> {
        self.state
            .as_ref()
            .and_then(|s| s.cache().file(path).map(outline_headings))
            .unwrap_or_default()
    }

    /// What a group shows when its tab is a view leaf.
    pub(super) fn linked_view_element(
        &self,
        view: &GroupView,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let tab = view.tabs.get(view.active_tab)?;
        let kind = kind_of(tab)?;
        let id = view.id;
        let file = file_of(tab);
        let element = match kind {
            LinkedKind::Backlinks => self.linked_backlinks_element(id, file, tab, theme, cx),
            LinkedKind::Outgoing => self.linked_outgoing_element(id, file, theme, cx),
            LinkedKind::Outline => self.linked_outline_element(id, file, tab, theme, cx),
            LinkedKind::FileProperties => {
                let key = view_key(tab)?;
                let (body, _) = self.linked_views.properties.get(&key)?;
                body.clone().into_any_element()
            }
        };
        Some(
            div()
                .size_full()
                .font_family(theme.ui_font.clone())
                .text_color(theme.tokens.text_normal)
                .child(element)
                .into_any_element(),
        )
    }

    /// The Backlinks view: the dock pane's four nav buttons, its filter
    /// field (the group's), then the sections, all over the leaf's own
    /// options through the backlinks block's handlers.
    fn linked_backlinks_element(
        &self,
        id: u64,
        file: Option<String>,
        tab: &Tab,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let this = cx.entity().downgrade();
        let doc = &tab.doc_backlinks;
        let (collapse_all, extra, show_search) = (
            doc.flag("collapseAll"),
            doc.flag("extraContext"),
            doc.flag("showSearch"),
        );
        let query = doc.query().to_string();
        let sort_menu = self.doc_sort_menu_element(id, doc.sort(), theme, &this);
        let header = Self::pane_header()
            .child(
                Self::nav_button(
                    "linked-backlinks-collapse",
                    "list",
                    "Collapse results",
                    collapse_all,
                    theme,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.doc_toggle_flag(id, "collapseAll", cx)),
                ),
            )
            .child(
                Self::nav_button(
                    "linked-backlinks-context",
                    "move-vertical",
                    "Show more context",
                    extra,
                    theme,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.doc_toggle_flag(id, "extraContext", cx)),
                ),
            )
            .child(
                div()
                    .relative()
                    .child(
                        Self::nav_button(
                            "linked-backlinks-sort",
                            "arrow-up-narrow-wide",
                            "Change sort order",
                            false,
                            theme,
                        )
                        .on_click(cx.listener(
                            move |this, event: &ClickEvent, _, cx| {
                                this.sort_menu_at = event.position();
                                this.doc_sort_menu = match this.doc_sort_menu {
                                    Some(open) if open == id => None,
                                    _ => Some(id),
                                };
                                cx.notify();
                            },
                        )),
                    )
                    .children(sort_menu),
            )
            .child(
                Self::nav_button(
                    "linked-backlinks-search",
                    "search",
                    "Show search filter",
                    show_search,
                    theme,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.doc_toggle_search(id, window, cx)),
                ),
            );
        let filter = show_search.then(|| self.group_ix(id)).flatten().map(|ix| {
            tree::filter_field(
                ("linked-filter-clear", id as usize),
                self.groups[ix].doc_filter.clone(),
                theme,
                cx,
                move |this: &mut Self, window, cx| this.doc_clear_filter(id, window, cx),
            )
        });
        let sections = file
            .as_deref()
            .map(|path| self.doc_sections(path, doc))
            .unwrap_or_default();
        let list = self.render_section_list(
            SectionsOwner::Doc(id),
            &sections,
            file.as_deref().unwrap_or(""),
            &query,
            theme,
            &this,
        );
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .children(filter)
            .child(
                div()
                    .id(("linked-backlinks", id as usize))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(12.))
                    .pt(px(4.))
                    .pb(px(32.))
                    .child(list),
            )
            .into_any_element()
    }

    /// The Outgoing links view: no header of its own, Links then Unlinked
    /// mentions, over the leaf's `linksCollapsed` and `unlinkedCollapsed`.
    fn linked_outgoing_element(
        &self,
        id: u64,
        file: Option<String>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let this = cx.entity().downgrade();
        let sections = file
            .as_deref()
            .map(|path| self.linked_outgoing_sections(path))
            .unwrap_or_default();
        let list = self.render_section_list(
            SectionsOwner::Outgoing(id),
            &sections,
            file.as_deref().unwrap_or(""),
            "",
            theme,
            &this,
        );
        div()
            .id(("linked-outgoing", id as usize))
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .px(px(12.))
            .pt(px(12.))
            .pb(px(32.))
            .child(list)
            .into_any_element()
    }

    /// The Outline view: the dock pane's three nav buttons over the leaf's
    /// `showSearch` and `followCursor`, the group's filter field (the
    /// leaf's `searchQuery`), then the note's headings with folds of the
    /// leaf's own.
    fn linked_outline_element(
        &self,
        id: u64,
        file: Option<String>,
        tab: &Tab,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let doc = &tab.doc_backlinks;
        let key = view_key(tab).unwrap_or_default();
        let headings = file
            .as_deref()
            .map(|p| self.linked_headings(p))
            .unwrap_or_default();
        let folds = self
            .linked_views
            .folds
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let top = outline_top_parents_of(&headings);
        let all_folded = !top.is_empty() && top.iter().all(|ix| folds.contains(ix));
        let show_search = doc.flag("showSearch");
        let follow = doc.flag("followCursor");
        let fold_key = key.clone();
        let header = Self::pane_header()
            .child(
                Self::nav_button(
                    "linked-outline-search",
                    "search",
                    "Show search filter",
                    show_search,
                    theme,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.doc_toggle_search(id, window, cx)),
                ),
            )
            .child(
                Self::nav_button(
                    "linked-outline-follow",
                    "gallery-vertical",
                    "Auto-scroll to current section",
                    follow,
                    theme,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.doc_toggle_flag(id, "followCursor", cx)),
                ),
            )
            .child(
                if all_folded {
                    Self::nav_button(
                        "linked-outline-fold",
                        "chevrons-up-down",
                        "Expand all",
                        false,
                        theme,
                    )
                } else {
                    Self::nav_button(
                        "linked-outline-fold",
                        "chevrons-down-up",
                        "Collapse all",
                        false,
                        theme,
                    )
                }
                .on_click(cx.listener(move |this, _, _, cx| {
                    let folds = this.linked_views.folds.entry(fold_key.clone()).or_default();
                    if all_folded {
                        folds.clear();
                    } else {
                        folds.extend(top.iter().copied());
                    }
                    cx.notify();
                })),
            );
        let filter = show_search.then(|| self.group_ix(id)).flatten().map(|ix| {
            tree::filter_field(
                ("linked-filter-clear", id as usize),
                self.groups[ix].doc_filter.clone(),
                theme,
                cx,
                move |this: &mut Self, window, cx| this.doc_clear_filter(id, window, cx),
            )
        });
        let query = doc.query().to_string();
        let body: gpui::AnyElement = if headings.is_empty() {
            div()
                .flex_1()
                .px(px(12.))
                .child(
                    div()
                        .my(px(8.))
                        .px(px(16.))
                        .py(px(24.))
                        .text_size(px(FONT_UI_SMALL))
                        .text_color(theme.tokens.text_faint)
                        .text_center()
                        .child("No headings found."),
                )
                .into_any_element()
        } else {
            let rows = panes::outline_rows(&headings, &folds, &query, None);
            let mut list = div()
                .id(("linked-outline", id as usize))
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(12.))
                .pt(px(4.))
                .pb(px(32.));
            for row in rows {
                let offset = row.offset;
                let fold = row.has_children.then(|| {
                    let ix = row.ix;
                    let key = key.clone();
                    let on_fold: FoldAction = Box::new(move |this, cx| {
                        let folds = this.linked_views.folds.entry(key.clone()).or_default();
                        if !folds.remove(&ix) {
                            folds.insert(ix);
                        }
                        cx.notify();
                    });
                    (row.collapsed, on_fold)
                });
                let highlights: Vec<(Range<usize>, HighlightStyle)> = row
                    .highlights
                    .iter()
                    .filter(|r| r.end <= row.text.len())
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
                let label = StyledText::new(SharedString::from(row.text.clone()))
                    .with_highlights(highlights)
                    .into_any_element();
                list = list.child(
                    Self::tree_row(
                        ("linked-outline-row", row.ix),
                        row.depth,
                        fold,
                        None,
                        label,
                        None,
                        false,
                        false,
                        theme,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.linked_outline_open(id, offset, window, cx)
                    })),
                );
            }
            list.into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .children(filter)
            .child(body)
            .into_any_element()
    }

    /// A heading clicked in the Outline view of group `id`: its note's tab
    /// made active and scrolled there, the caret on the heading and the
    /// keyboard in the note (observed, `s07`).
    fn linked_outline_open(
        &mut self,
        id: u64,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.doc_tab(id).filter(|t| kind_of(t).is_some()) else {
            return;
        };
        let (link, file) = (tab.link.clone(), file_of(tab));
        if !self.activate_follower_target(link.as_deref(), cx) {
            return;
        }
        if file.is_some() && self.current != file {
            if let Some(file) = file {
                self.open_file(file, None, cx);
            }
        }
        self.editor.update(cx, |e, cx| {
            e.set_selection_range(offset, offset, window, cx);
        });
        self.reveal_heading(offset, window, cx);
    }

    /// The body of the File properties view under `key`: the note's rows,
    /// editable through the editor of the group showing that note.
    pub(super) fn linked_properties_element(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some((_, focus)) = self.linked_views.properties.get(key).cloned() else {
            return div().into_any_element();
        };
        let tab = self
            .every_tab()
            .into_iter()
            .flat_map(|(_, tabs)| tabs)
            .find(|t| view_key(t).as_deref() == Some(key));
        let file = tab.as_ref().and_then(file_of);
        let link = tab.as_ref().and_then(|t| t.link.clone());
        // The editor holding the note: its group's, while it shows it.
        let editor = self
            .follower_target(link.as_deref())
            .map(|(gi, _)| self.groups[gi].editor.clone())
            .unwrap_or_else(|| self.editor.clone());
        let note = file
            .clone()
            .filter(|p| FileKind::from_path(p) == FileKind::Markdown);
        let holds = note
            .as_deref()
            .is_some_and(|p| editor.read(cx).name.as_ref() == p);
        let note = note.and_then(|path| {
            if holds {
                let text = Arc::new(editor.read(cx).text().to_string());
                return Some((path, text));
            }
            let text = self.state.as_ref()?.texts().get(&path).cloned()?;
            Some((path, text))
        });
        let theme = cx.global::<Theme>().clone();
        let available = (f32::from(window.viewport_size().width) / 2. - 24.).max(160.);
        self.properties_element(
            format!("linked-properties-{key}").into(),
            focus,
            note,
            editor,
            holds,
            available,
            &theme,
            window,
            cx,
        )
    }

    /// A note's buffer edited from a linked File properties view while its
    /// group is not the active one: written now, as Obsidian wrote a
    /// property edit from a pane on Enter.
    pub(super) fn write_linked_edit(&mut self, editor: &Entity<Editor>, cx: &mut Context<Self>) {
        let dirty = editor.read(cx).is_dirty();
        let name = editor.read(cx).name.to_string();
        let shown = self.groups.iter().any(|g| {
            g.editor.entity_id() == editor.entity_id()
                && g.tabs.get(g.active_tab).and_then(|t| t.path.as_deref()) == Some(name.as_str())
        });
        if dirty && shown {
            self.write_buffer(editor, cx);
        }
    }

    /// The view tabs for the control socket's state dump: each one's leaf
    /// id, group, link, view type, file, title and state as it would be
    /// written, and what it lists.
    pub(super) fn describe_linked_views(&self, cx: &App) -> Value {
        let mut out = Vec::new();
        for (gi, tabs) in self.every_tab() {
            let group = self.groups[gi].id;
            for tab in tabs {
                let Some(kind) = kind_of(&tab) else {
                    continue;
                };
                let file = file_of(&tab);
                let rows: Value = match (kind, file.as_deref()) {
                    (LinkedKind::Backlinks, Some(path)) => Value::Array(section_rows_json(
                        &self.doc_sections(path, &tab.doc_backlinks),
                        &|ix| {
                            tab.doc_backlinks.flag(if ix == 0 {
                                "backlinkCollapsed"
                            } else {
                                "unlinkedCollapsed"
                            })
                        },
                        &|key| {
                            tab.doc_backlinks.flag("collapseAll")
                                ^ tab.doc_backlinks.toggled.contains(key)
                        },
                    )),
                    (LinkedKind::Outgoing, Some(path)) => Value::Array(section_rows_json(
                        &self.linked_outgoing_sections(path),
                        &|ix| {
                            tab.doc_backlinks.flag(if ix == 0 {
                                "linksCollapsed"
                            } else {
                                "unlinkedCollapsed"
                            })
                        },
                        &|key| tab.doc_backlinks.toggled.contains(key),
                    )),
                    (LinkedKind::Outline, Some(path)) => {
                        let key = view_key(&tab).unwrap_or_default();
                        let folds = self
                            .linked_views
                            .folds
                            .get(&key)
                            .cloned()
                            .unwrap_or_default();
                        let headings = self.linked_headings(path);
                        let rows =
                            panes::outline_rows(&headings, &folds, tab.doc_backlinks.query(), None);
                        rows.iter()
                            .map(|r| json!({"text": r.text, "depth": r.depth, "collapsed": r.collapsed}))
                            .collect()
                    }
                    (LinkedKind::FileProperties, Some(path)) => self
                        .state
                        .as_ref()
                        .and_then(|s| s.texts().get(path).cloned())
                        .and_then(|text| scoria_cache::note_properties(&text))
                        .map(|b| b.entries.iter().map(|e| json!(e.key)).collect())
                        .unwrap_or(Value::Array(Vec::new())),
                    _ => Value::Array(Vec::new()),
                };
                out.push(json!({
                    "id": view_key(&tab),
                    "group": group,
                    "link": tab.link,
                    "type": kind.view_type(),
                    "file": file,
                    "title": title(&tab),
                    "state": tab.leaf.as_ref().and_then(|l| l.pointer("/state/state")).cloned(),
                    "rows": rows,
                }));
            }
        }
        let _ = cx;
        Value::Array(out)
    }
}

/// The top-level headings with children: what Collapse all folds.
fn outline_top_parents_of(headings: &[panes::Heading]) -> Vec<usize> {
    let depths = panes::outline_depths(headings);
    (0..depths.len())
        .filter(|&ix| depths[ix] == 0 && depths.get(ix + 1).is_some_and(|&d| d > depths[ix]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &Value) -> Vec<String> {
        value.as_object().unwrap().keys().cloned().collect()
    }

    /// Each row's leaf, as Obsidian wrote it on Alpha (`s02`): the state's
    /// keys in order, the icon, the linked title and the `group` last.
    #[test]
    fn new_leaves_are_obsidians() {
        let cases = [
            (
                LinkedKind::Backlinks,
                r#"{"type":"backlink","state":{"file":"Alpha.md","collapseAll":false,"extraContext":false,"sortOrder":"alphabetical","showSearch":false,"searchQuery":"","backlinkCollapsed":false,"unlinkedCollapsed":true},"icon":"links-coming-in","title":"Backlinks for Alpha"}"#,
            ),
            (
                LinkedKind::Outgoing,
                r#"{"type":"outgoing-link","state":{"file":"Alpha.md","linksCollapsed":false,"unlinkedCollapsed":true},"icon":"links-going-out","title":"Outgoing links from Alpha"}"#,
            ),
            (
                LinkedKind::FileProperties,
                r#"{"type":"file-properties","state":{"file":"Alpha.md"},"icon":"lucide-info","title":"File properties for Alpha"}"#,
            ),
            (
                LinkedKind::Outline,
                r#"{"type":"outline","state":{"file":"Alpha.md","followCursor":false,"showSearch":false,"searchQuery":""},"icon":"lucide-list","title":"Outline of Alpha"}"#,
            ),
        ];
        for (kind, state) in cases {
            let tab = new_tab(
                kind,
                Some("g1".into()),
                Some("Alpha.md"),
                ViewMode::LivePreview,
            );
            assert_eq!(kind_of(&tab), Some(kind));
            assert!(is_follower(&tab));
            let leaf = leaf_json(&tab);
            assert_eq!(keys(&leaf), ["id", "type", "state", "group"]);
            assert_eq!(serde_json::to_string(&leaf["state"]).unwrap(), state);
            assert_eq!(leaf["group"], json!("g1"));
        }
    }

    /// Titles: linked over a file, plain once unlinked (`s03`).
    #[test]
    fn titles_follow_the_link() {
        let mut tab = new_tab(
            LinkedKind::Backlinks,
            Some("g".into()),
            Some("Board.canvas"),
            ViewMode::Reading,
        );
        assert_eq!(title(&tab), "Backlinks for Board");
        tab.link = None;
        assert_eq!(title(&tab), "Backlinks");
        let tab = new_tab(
            LinkedKind::Outline,
            None,
            Some("dir/pic.png"),
            ViewMode::Reading,
        );
        assert_eq!(title(&tab), "Outline");
        assert_eq!(
            LinkedKind::Outgoing.title_of("doc"),
            "Outgoing links from doc"
        );
    }

    /// The `file` key moves in place, comes first when missing and goes
    /// with no file, the other keys left as they were.
    #[test]
    fn the_file_key_is_kept_in_place() {
        let state: Map<String, Value> =
            serde_json::from_str(r#"{"file":"A.md","linksCollapsed":true,"x":1}"#).unwrap();
        let moved = with_file(state, Some("B.md"));
        assert_eq!(
            serde_json::to_string(&moved).unwrap(),
            r#"{"file":"B.md","linksCollapsed":true,"x":1}"#
        );
        let gone = with_file(moved, None);
        assert_eq!(
            serde_json::to_string(&gone).unwrap(),
            r#"{"linksCollapsed":true,"x":1}"#
        );
        let back = with_file(gone, Some("C.md"));
        assert_eq!(
            serde_json::to_string(&back).unwrap(),
            r#"{"file":"C.md","linksCollapsed":true,"x":1}"#
        );
    }

    /// The probe's edited Backlinks leaf (`s04`), with keys of its own
    /// added, restores linked and writes back byte for byte.
    #[test]
    fn an_obsidian_leaf_round_trips() {
        let text = r#"{
  "id": "d542b9961e168c1c",
  "type": "leaf",
  "state": {
    "type": "backlink",
    "state": {
      "file": "Beta.md",
      "collapseAll": true,
      "extraContext": true,
      "sortOrder": "alphabeticalReverse",
      "showSearch": true,
      "searchQuery": "Gam",
      "backlinkCollapsed": true,
      "unlinkedCollapsed": false,
      "scoriaUnknown": "kept"
    },
    "icon": "links-coming-in",
    "title": "Backlinks for Beta"
  },
  "group": "d2d7e09e347e9ed6",
  "scoriaLeaf": true
}"#;
        let leaf: Value = serde_json::from_str(text).unwrap();
        let (id, tab) = tab_from_leaf(&leaf, ViewMode::LivePreview, &|_| true).unwrap();
        assert_eq!(id, "d542b9961e168c1c");
        assert_eq!(kind_of(&tab), Some(LinkedKind::Backlinks));
        assert_eq!(tab.link.as_deref(), Some("d2d7e09e347e9ed6"));
        assert_eq!(file_of(&tab).as_deref(), Some("Beta.md"));
        assert!(tab.doc_backlinks.flag("collapseAll"));
        assert_eq!(tab.doc_backlinks.query(), "Gam");
        assert_eq!(tab.doc_backlinks.sort(), SortOrder::AlphabeticalReverse);
        assert_eq!(
            serde_json::to_string_pretty(&leaf_json(&tab)).unwrap(),
            text
        );
    }

    /// An unlinked leaf (the commands', or one whose note closed) has no
    /// `group` and a plain title (`s03`, `s05`).
    #[test]
    fn an_unlinked_leaf_has_no_group() {
        let text = r#"{"id": "87c2f698ea644010", "type": "leaf", "state": {"type": "outline",
            "state": {"file": "Delta.md", "followCursor": false, "showSearch": false,
            "searchQuery": ""}, "icon": "lucide-list", "title": "Outline"}}"#;
        let leaf: Value = serde_json::from_str(text).unwrap();
        let (_, tab) = tab_from_leaf(&leaf, ViewMode::LivePreview, &|_| true).unwrap();
        assert_eq!(kind_of(&tab), Some(LinkedKind::Outline));
        assert!(tab.link.is_none());
        assert_eq!(title(&tab), "Outline");
        assert_eq!(
            serde_json::to_string(&leaf_json(&tab)).unwrap(),
            serde_json::to_string(&leaf).unwrap()
        );
    }

    /// The submenu rows map onto the views; the local graph's is its own.
    #[test]
    fn rows_name_their_views() {
        assert_eq!(LinkedKind::from_row(LinkedRow::LocalGraph), None);
        assert_eq!(
            LinkedKind::from_row(LinkedRow::Outline),
            Some(LinkedKind::Outline)
        );
        assert_eq!(LinkedKind::Outline.direction(), SplitDirection::Vertical);
        assert_eq!(
            LinkedKind::Backlinks.direction(),
            SplitDirection::Horizontal
        );
        assert_eq!(LinkedKind::Outgoing.direction(), SplitDirection::Horizontal);
        assert_eq!(
            LinkedKind::FileProperties.direction(),
            SplitDirection::Horizontal
        );
    }
}
