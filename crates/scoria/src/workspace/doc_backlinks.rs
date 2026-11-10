//! Backlinks in document (`backlink:toggle-backlinks-in-document`): the
//! Backlinks pane's rows drawn at the foot of a note, in the editor and in
//! reading view, per leaf.
//!
//! Observed on Obsidian 1.15.0
//! (`tools/obsidian-probe/results/2026-11-09-panes-workspaces-headings.json`,
//! `SCEN=bl` and `SCEN=blsetting`): the command flips the active markdown
//! leaf's view state and writes nothing to `backlink.json`. A fresh leaf
//! with the setting off has no key; turned on, its state gains
//! `"backlinks": true` and `"backlinkOpts"` — the Backlinks pane's own
//! state object, key for key and in its order — after `source`; turned off
//! it keeps `"backlinks": false` and loses `backlinkOpts`. With
//! `backlinkInDocument` on in `backlink.json` a leaf with no key shows the
//! block and is written with both keys from the start, while a leaf that
//! says `false` stays off. Expanding "Unlinked mentions" writes
//! `unlinkedCollapsed: false` into the leaf's entry.
//!
//! The block is the pane's: [`Workspace::render_section_list`] over the
//! same sections, with the leaf's own folds. Of its four nav buttons the two
//! that act on what is already drawn are here — "Collapse results" and
//! "Show more context". "Change sort order" and "Show search filter" are
//! left out: the probe never drove the block's own controls, so where its
//! sort menu opens and how its search field sits are not on record. A
//! leaf's `sortOrder`, `showSearch` and `searchQuery` are still read and
//! written back as they were, and the groups follow the sort order.

use super::*;

/// The Backlinks pane's state object as a fresh leaf's `backlinkOpts`
/// carries it, in Obsidian's key order (observed, `b02-toggle-on`).
const DEFAULT_OPTS: &str = r#"{"collapseAll": false, "extraContext": false, "sortOrder": "alphabetical", "showSearch": false, "searchQuery": "", "backlinkCollapsed": false, "unlinkedCollapsed": true}"#;

fn default_opts() -> Map<String, Value> {
    serde_json::from_str(DEFAULT_OPTS).unwrap_or_default()
}

/// [`Context::listener`] without the context: a handler that updates the
/// entity behind `this`. The sections are drawn from the editor and the
/// reading view as well as from the workspace's own render, and only the
/// latter has a `Context<Workspace>` at hand.
pub(super) fn listen<E: ?Sized, T: 'static>(
    this: &gpui::WeakEntity<T>,
    f: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |event, window, cx| {
        this.update(cx, |entity, cx| f(entity, event, window, cx))
            .ok();
    }
}

/// Whose sections a list draws: a dock pane's, or the backlinks block of the
/// note group `u64` shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SectionsOwner {
    Pane(RightTab),
    Doc(u64),
}

impl SectionsOwner {
    /// The pane whose rows these are: the block draws the Backlinks pane's.
    pub(super) fn kind(self) -> RightTab {
        match self {
            SectionsOwner::Pane(which) => which,
            SectionsOwner::Doc(_) => RightTab::Backlinks,
        }
    }
}

/// One leaf's backlinks block.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct DocBacklinks {
    /// The leaf's `backlinks`: `None` when it has no key, which follows
    /// `backlinkInDocument`.
    shown: Option<bool>,
    /// The leaf's `backlinkOpts` as read or last changed; `None` until it
    /// had one, which reads as the defaults.
    opts: Option<Map<String, Value>>,
    /// Groups folded by hand away from what `collapseAll` says, by
    /// [`Section::group_key`]. Not written: the pane keeps none either.
    toggled: HashSet<String>,
}

impl DocBacklinks {
    /// The block's state from a markdown leaf's inner `state`.
    pub(super) fn read(inner: Option<&Value>) -> DocBacklinks {
        DocBacklinks {
            shown: inner
                .and_then(|s| s.get("backlinks"))
                .and_then(Value::as_bool),
            opts: inner
                .and_then(|s| s.get("backlinkOpts"))
                .and_then(Value::as_object)
                .cloned(),
            toggled: HashSet::new(),
        }
    }

    /// Whether the block shows, given `backlinkInDocument`.
    pub(super) fn is_shown(&self, setting: bool) -> bool {
        self.shown.unwrap_or(setting)
    }

    /// The command: the block flipped from whatever it is now.
    pub(super) fn toggle(&mut self, setting: bool) {
        self.shown = Some(!self.is_shown(setting));
    }

    /// One of `backlinkOpts`' flags, at its default when the leaf has none.
    fn flag(&self, key: &str) -> bool {
        match self.opts.as_ref().and_then(|o| o.get(key)) {
            Some(value) => value.as_bool().unwrap_or(false),
            None => default_opts()
                .get(key)
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    /// Sets a flag in place, the other keys and their order left alone.
    fn set_flag(&mut self, key: &str, on: bool) {
        self.opts
            .get_or_insert_with(default_opts)
            .insert(key.into(), json!(on));
    }

    fn sort(&self) -> SortOrder {
        self.opts
            .as_ref()
            .and_then(|o| o.get("sortOrder"))
            .and_then(Value::as_str)
            .and_then(SortOrder::parse)
            .unwrap_or_default()
    }

    /// Writes the block into a markdown leaf: `"backlinks": true` and the
    /// options while it shows, `"backlinks": false` and no options once
    /// turned off, nothing for a leaf that never said and is not shown.
    pub(super) fn write(&self, leaf: &mut Value, setting: bool) {
        let markdown = leaf
            .get("state")
            .and_then(|s| s.get("type"))
            .and_then(Value::as_str)
            == Some("markdown");
        if !markdown {
            return;
        }
        let Some(inner) = leaf
            .get_mut("state")
            .and_then(|s| s.get_mut("state"))
            .and_then(Value::as_object_mut)
        else {
            return;
        };
        if self.is_shown(setting) {
            inner.insert("backlinks".into(), json!(true));
            let opts = Value::Object(self.opts.clone().unwrap_or_else(default_opts));
            match inner.get_mut("backlinkOpts") {
                Some(slot) => *slot = opts,
                None => {
                    let at = inner
                        .keys()
                        .position(|k| k == "backlinks")
                        .map_or(inner.len(), |i| i + 1);
                    inner.shift_insert(at, "backlinkOpts".into(), opts);
                }
            }
        } else if self.shown == Some(false) {
            inner.insert("backlinks".into(), json!(false));
            inner.shift_remove("backlinkOpts");
        } else {
            inner.shift_remove("backlinks");
            inner.shift_remove("backlinkOpts");
        }
    }
}

/// Gives a group's editor and reading view the block to draw under the
/// note, built from the workspace each time they lay out.
pub(super) fn give_views_the_block(
    ws: gpui::WeakEntity<Workspace>,
    editor: &Entity<Editor>,
    reading: &Entity<ReadingView>,
    cx: &mut App,
) {
    let footer = |host: gpui::EntityId| -> reading::view::FooterBuilder {
        let ws = ws.clone();
        Rc::new(move |width, _window, cx| {
            let entity = ws.upgrade()?;
            let theme = cx.global::<Theme>().clone();
            entity
                .read(cx)
                .backlinks_block(&ws, host, width, &theme, cx)
        })
    };
    let host = editor.entity_id();
    editor.update(cx, |e, _| e.set_footer(Some(footer(host))));
    let host = reading.entity_id();
    reading.update(cx, |r, _| r.set_footer(Some(footer(host))));
}

impl Workspace {
    /// `backlinkInDocument` from `backlink.json`, while the Backlinks core
    /// plugin is on (with it off there is no block to show).
    pub(super) fn backlinks_in_document_setting(&self) -> bool {
        self.state.as_ref().is_some_and(|s| {
            s.core_plugin_enabled("backlink") && s.backlink().backlink_in_document.unwrap_or(false)
        })
    }

    /// Whether the active leaf is a markdown note, which is what the
    /// command's check callback asks.
    pub(super) fn active_note_is_markdown(&self) -> bool {
        self.mode != ViewMode::Graph
            && self
                .current
                .as_deref()
                .is_some_and(|p| FileKind::from_path(p) == FileKind::Markdown)
    }

    /// Whether the active markdown leaf shows the block: the check on the
    /// menus' "Backlinks in document" row. `None` with no markdown leaf
    /// active, where the row is not drawn.
    pub(super) fn active_backlinks_in_document(&self) -> Option<bool> {
        if !self.active_note_is_markdown() {
            return None;
        }
        let setting = self.backlinks_in_document_setting();
        Some(
            self.tabs
                .get(self.active_tab)
                .is_some_and(|t| t.doc_backlinks.is_shown(setting)),
        )
    }

    /// `backlink:toggle-backlinks-in-document`, and the menus' row: the
    /// active markdown leaf's block on or off.
    pub(super) fn toggle_backlinks_in_document(&mut self, cx: &mut Context<Self>) {
        if !self.active_note_is_markdown() {
            return;
        }
        let setting = self.backlinks_in_document_setting();
        self.stash_active_tab();
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.doc_backlinks.toggle(setting);
        }
        cx.notify();
    }

    /// The tab group `id` is showing, by the group's id.
    fn doc_tab(&self, id: u64) -> Option<&Tab> {
        let ix = self.group_ix(id)?;
        if ix == self.active_group {
            return self.tabs.get(self.active_tab);
        }
        let g = &self.groups[ix];
        g.tabs.get(g.active_tab)
    }

    fn doc_tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        let ix = self.group_ix(id)?;
        if ix == self.active_group {
            return self.tabs.get_mut(self.active_tab);
        }
        let g = &mut self.groups[ix];
        let at = g.active_tab;
        g.tabs.get_mut(at)
    }

    /// The group whose editor or reading view is `host`, the note it shows
    /// and that tab's block.
    fn doc_host(&self, host: gpui::EntityId) -> Option<(u64, String, &DocBacklinks)> {
        let ix = self
            .groups
            .iter()
            .position(|g| g.editor.entity_id() == host || g.reading.entity_id() == host)?;
        let g = &self.groups[ix];
        let (tab, path) = if ix == self.active_group {
            (self.tabs.get(self.active_tab)?, self.current.clone()?)
        } else {
            let tab = g.tabs.get(g.active_tab)?;
            (tab, tab.path.clone()?)
        };
        Some((g.id, path, &tab.doc_backlinks))
    }

    /// The block's two sections for `path`: the pane's own when the note is
    /// the open one and the context setting agrees, computed here
    /// otherwise; then put in the leaf's sort order.
    fn doc_sections(&self, path: &str, doc: &DocBacklinks) -> Vec<Section> {
        let Some(state) = self.state.as_ref() else {
            return Vec::new();
        };
        let extra = doc.flag("extraContext");
        let same_context = extra == self.backlink_state.extra_context;
        let open = self.current.as_deref() == Some(path);
        let linked = match self.backlink_sections.first() {
            Some(section) if open && same_context => section.clone(),
            _ => backlinks::linked_mentions(&state.cache(), &state.texts(), path, extra),
        };
        let found = if open {
            self.backlink_sections.get(1).cloned()
        } else {
            self.doc_mentions.get(path).cloned()
        }
        .unwrap_or_else(|| Section::empty(backlinks::UNLINKED));
        let unlinked = if same_context {
            found
        } else {
            backlinks::with_context(&found, &state.texts(), extra)
        };
        let mut sections = vec![linked, unlinked];
        let order = doc.sort();
        let times: HashMap<String, (u64, u64)> = match order {
            SortOrder::Alphabetical | SortOrder::AlphabeticalReverse => HashMap::new(),
            _ => state
                .vault()
                .files()
                .iter()
                .map(|f| (f.path.clone(), (f.mtime, f.ctime)))
                .collect(),
        };
        let stat = |p: &str| times.get(p).copied().unwrap_or((0, 0));
        for section in &mut sections {
            backlinks::sort_groups(section, order, &stat);
        }
        sections
    }

    /// The block under the note `host` shows, `width` wide: the nav
    /// buttons right-aligned under a 12 px top, then the Backlinks pane's
    /// sections over the leaf's folds. `None` when the leaf does not show
    /// it, the note is not Markdown, or the Backlinks plugin is off.
    pub(super) fn backlinks_block(
        &self,
        this: &gpui::WeakEntity<Self>,
        host: gpui::EntityId,
        width: f32,
        theme: &Theme,
        _cx: &App,
    ) -> Option<gpui::AnyElement> {
        let state = self.state.as_ref()?;
        if !state.core_plugin_enabled("backlink") {
            return None;
        }
        let (id, path, doc) = self.doc_host(host)?;
        if FileKind::from_path(&path) != FileKind::Markdown
            || !doc.is_shown(self.backlinks_in_document_setting())
        {
            return None;
        }
        let sections = self.doc_sections(&path, doc);
        let (collapse_all, extra) = (doc.flag("collapseAll"), doc.flag("extraContext"));
        // `div.nav-header` (padding 12px 0 0) → the buttons, right-aligned.
        let header = div()
            .w_full()
            .pt(px(12.))
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(2.))
            .child(
                Self::nav_button(
                    "doc-backlinks-collapse",
                    "list",
                    "Collapse results",
                    collapse_all,
                    theme,
                )
                .on_click(listen(
                    this,
                    move |ws: &mut Self, _: &ClickEvent, _, cx| {
                        ws.doc_toggle_flag(id, "collapseAll", cx)
                    },
                )),
            )
            .child(
                Self::nav_button(
                    "doc-backlinks-context",
                    "move-vertical",
                    "Show more context",
                    extra,
                    theme,
                )
                .on_click(listen(
                    this,
                    move |ws: &mut Self, _: &ClickEvent, _, cx| {
                        ws.doc_toggle_flag(id, "extraContext", cx)
                    },
                )),
            );
        let list =
            self.render_section_list(SectionsOwner::Doc(id), &sections, &path, "", theme, this);
        Some(
            div()
                .id(("doc-backlinks", id as usize))
                .w(px(width))
                .flex()
                .flex_col()
                .font_family(theme.ui_font.clone())
                .text_color(theme.tokens.text_normal)
                .child(header)
                // `div.backlink-pane` sits under the header like the dock
                // pane's list, without the dock's side padding.
                .child(div().pt(px(4.)).child(list))
                .into_any_element(),
        )
    }

    /// One of the block's `backlinkOpts` flags flipped in group `id`'s leaf.
    fn doc_toggle_flag(&mut self, id: u64, key: &str, cx: &mut Context<Self>) {
        if let Some(tab) = self.doc_tab_mut(id) {
            let doc = &mut tab.doc_backlinks;
            let on = !doc.flag(key);
            doc.set_flag(key, on);
            if key == "collapseAll" {
                doc.toggled.clear();
            }
        }
        cx.notify();
    }

    /// Whether a section of `owner`'s list is folded: a pane's own flag, or
    /// the leaf's `backlinkCollapsed` / `unlinkedCollapsed`.
    pub(super) fn owner_section_collapsed(&self, owner: SectionsOwner, ix: usize) -> bool {
        match owner {
            SectionsOwner::Pane(which) => self.section_collapsed(which, ix),
            SectionsOwner::Doc(id) => self.doc_tab(id).is_some_and(|t| {
                t.doc_backlinks.flag(if ix == 0 {
                    "backlinkCollapsed"
                } else {
                    "unlinkedCollapsed"
                })
            }),
        }
    }

    pub(super) fn owner_toggle_section(
        &mut self,
        owner: SectionsOwner,
        ix: usize,
        cx: &mut Context<Self>,
    ) {
        match owner {
            SectionsOwner::Pane(which) => self.toggle_section(which, ix, cx),
            SectionsOwner::Doc(id) => self.doc_toggle_flag(
                id,
                if ix == 0 {
                    "backlinkCollapsed"
                } else {
                    "unlinkedCollapsed"
                },
                cx,
            ),
        }
    }

    /// Whether a group is folded: the leaf's "Collapse results" sets the
    /// default and a click on the group inverts it, as in the pane.
    pub(super) fn owner_group_collapsed(&self, owner: SectionsOwner, key: &str) -> bool {
        match owner {
            SectionsOwner::Pane(which) => self.group_collapsed(which, key),
            SectionsOwner::Doc(id) => self.doc_tab(id).is_some_and(|t| {
                t.doc_backlinks.flag("collapseAll") ^ t.doc_backlinks.toggled.contains(key)
            }),
        }
    }

    pub(super) fn owner_toggle_group(
        &mut self,
        owner: SectionsOwner,
        key: String,
        cx: &mut Context<Self>,
    ) {
        match owner {
            SectionsOwner::Pane(which) => self.toggle_group(which, key, cx),
            SectionsOwner::Doc(id) => {
                if let Some(tab) = self.doc_tab_mut(id) {
                    let toggled = &mut tab.doc_backlinks.toggled;
                    if !toggled.remove(&key) {
                        toggled.insert(key);
                    }
                }
                cx.notify();
            }
        }
    }

    /// The active leaf's block for the control socket: whether it shows,
    /// the leaf's `backlinks` and `backlinkOpts` as they would be written,
    /// and its rows with their folds, as the pane's are reported.
    pub(super) fn backlinks_in_document_json(&self) -> Value {
        let setting = self.backlinks_in_document_setting();
        let id = self.active_group_id();
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return Value::Null;
        };
        let Some(path) = self
            .current
            .clone()
            .filter(|_| self.active_note_is_markdown())
        else {
            return Value::Null;
        };
        let doc = &tab.doc_backlinks;
        let shown = doc.is_shown(setting)
            && self
                .state
                .as_ref()
                .is_some_and(|s| s.core_plugin_enabled("backlink"));
        let mut leaf = json!({"state": {"type": "markdown", "state": {}}});
        doc.write(&mut leaf, setting);
        let owner = SectionsOwner::Doc(id);
        let rows = if shown {
            section_rows_json(
                &self.doc_sections(&path, doc),
                &|ix| self.owner_section_collapsed(owner, ix),
                &|key| self.owner_group_collapsed(owner, key),
            )
        } else {
            Vec::new()
        };
        json!({
            "shown": shown,
            "file": path,
            "leaf": leaf["state"]["state"],
            "rows": rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTS: &str = r#""backlinkOpts": {"collapseAll": false, "extraContext": false, "sortOrder": "alphabetical", "showSearch": false, "searchQuery": "", "backlinkCollapsed": false, "unlinkedCollapsed": true}"#;

    fn markdown_leaf(inner: &str) -> Value {
        serde_json::from_str(&format!(
            r#"{{"id": "7168f5541b949fdd", "type": "leaf", "state": {{"type": "markdown", "state": {inner}, "icon": "lucide-file", "title": "Target"}}}}"#
        ))
        .unwrap()
    }

    fn inner(leaf: &Value) -> String {
        serde_json::to_string(&leaf["state"]["state"]).unwrap()
    }

    fn compact(json: &str) -> String {
        serde_json::to_string(&serde_json::from_str::<Value>(json).unwrap()).unwrap()
    }

    /// The `bl` run: a fresh leaf has no key; the command turns it on with
    /// `backlinks` and the pane's options after `source`, then off with
    /// `backlinks: false` and no options — each exactly as written.
    #[test]
    fn the_toggle_writes_the_leaf_state_obsidian_wrote() {
        let fresh = r#"{"file": "Target.md", "mode": "source", "source": false}"#;
        let mut doc = DocBacklinks::read(Some(&markdown_leaf(fresh)["state"]["state"]));
        let mut leaf = markdown_leaf(fresh);
        doc.write(&mut leaf, false);
        assert_eq!(inner(&leaf), compact(fresh), "a fresh leaf has no key");

        doc.toggle(false);
        doc.write(&mut leaf, false);
        assert_eq!(
            inner(&leaf),
            compact(&format!(
                r#"{{"file": "Target.md", "mode": "source", "source": false, "backlinks": true, {OPTS}}}"#
            ))
        );

        doc.toggle(false);
        doc.write(&mut leaf, false);
        assert_eq!(
            inner(&leaf),
            compact(
                r#"{"file": "Target.md", "mode": "source", "source": false, "backlinks": false}"#
            )
        );
    }

    /// The `blsetting` run: with `backlinkInDocument` on, a leaf with no key
    /// is written on from the start; the command turns it off (`false`, no
    /// options) and back on; a leaf saying `false` stays off.
    #[test]
    fn the_setting_turns_on_every_leaf_that_has_no_key() {
        let fresh = r#"{"file": "Target.md", "mode": "source", "source": false}"#;
        let mut doc = DocBacklinks::default();
        assert!(doc.is_shown(true));
        let mut leaf = markdown_leaf(fresh);
        doc.write(&mut leaf, true);
        assert_eq!(
            inner(&leaf),
            compact(&format!(
                r#"{{"file": "Target.md", "mode": "source", "source": false, "backlinks": true, {OPTS}}}"#
            ))
        );
        doc.toggle(true);
        assert!(!doc.is_shown(true));
        doc.write(&mut leaf, true);
        assert_eq!(
            inner(&leaf),
            compact(
                r#"{"file": "Target.md", "mode": "source", "source": false, "backlinks": false}"#
            )
        );
        doc.toggle(true);
        assert!(doc.is_shown(true));

        let off = DocBacklinks::read(Some(&json!({"backlinks": false})));
        assert!(!off.is_shown(true), "a leaf that says false stays off");
        assert!(!DocBacklinks::default().is_shown(false));
    }

    /// Expanding "Unlinked mentions" writes `unlinkedCollapsed: false` into
    /// the leaf's options, in place (`b03-block-in-modes`).
    #[test]
    fn expanding_unlinked_mentions_writes_unlinked_collapsed_false() {
        let mut doc = DocBacklinks::default();
        doc.toggle(false);
        assert!(
            doc.flag("unlinkedCollapsed"),
            "unlinked mentions start folded"
        );
        assert!(!doc.flag("backlinkCollapsed"), "linked mentions start open");
        doc.set_flag("unlinkedCollapsed", false);
        let mut leaf = markdown_leaf(r#"{"file": "Target.md", "mode": "source", "source": false}"#);
        doc.write(&mut leaf, false);
        let opts = &leaf["state"]["state"]["backlinkOpts"];
        let keys: Vec<&String> = opts.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "collapseAll",
                "extraContext",
                "sortOrder",
                "showSearch",
                "searchQuery",
                "backlinkCollapsed",
                "unlinkedCollapsed"
            ]
        );
        assert_eq!(opts["unlinkedCollapsed"], json!(false));
    }

    /// A leaf read from a real file is written back as it was, unknown keys
    /// in `backlinkOpts` and after it included, through the same
    /// `leaf_json` the layout writer uses.
    #[test]
    fn a_leaf_with_the_block_round_trips_byte_for_byte() {
        let read = r#"{
  "id": "7168f5541b949fdd",
  "type": "leaf",
  "state": {
    "type": "markdown",
    "state": {
      "file": "Target.md",
      "mode": "preview",
      "source": false,
      "backlinks": true,
      "backlinkOpts": {
        "collapseAll": true,
        "extraContext": false,
        "sortOrder": "byModifiedTime",
        "showSearch": true,
        "searchQuery": "rock",
        "backlinkCollapsed": false,
        "unlinkedCollapsed": false,
        "someFutureKey": "kept too"
      },
      "anotherKey": "kept"
    },
    "icon": "lucide-file",
    "title": "Target"
  }
}"#;
        let leaf: Value = serde_json::from_str(read).unwrap();
        let known = |p: &str| p == "Target.md";
        let (_, tab) = tab_from_leaf(&leaf, ViewMode::LivePreview, &known).unwrap();
        assert_eq!(tab.doc_backlinks.sort(), SortOrder::ByModifiedTime);
        assert!(tab.doc_backlinks.flag("collapseAll"));
        for setting in [false, true] {
            let mut written = leaf_json(&tab);
            tab.doc_backlinks.write(&mut written, setting);
            assert_eq!(serde_json::to_string_pretty(&written).unwrap(), read);
        }
    }

    /// A leaf turned on after it had moved to another note, or a tab made
    /// fresh, gains the keys after `source`; a non-markdown leaf is never
    /// touched.
    #[test]
    fn only_markdown_leaves_carry_the_block() {
        let mut doc = DocBacklinks::default();
        doc.toggle(false);
        let mut canvas = json!({"id": "a", "type": "leaf", "state": {"type": "canvas", "state": {"file": "B.canvas"}}});
        let before = canvas.clone();
        doc.write(&mut canvas, true);
        assert_eq!(canvas, before);

        let tab = Tab {
            path: Some("New.md".into()),
            mode: ViewMode::Source,
            history: Vec::new(),
            history_ix: 0,
            leaf: None,
            view_state: None,
            pinned: false,
            doc_backlinks: doc,
        };
        let mut leaf = leaf_json(&tab);
        tab.doc_backlinks.write(&mut leaf, false);
        let keys: Vec<&String> = leaf["state"]["state"].as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            ["file", "mode", "source", "backlinks", "backlinkOpts"]
        );
    }

    /// The command is runnable, belongs to the Backlinks core plugin, and
    /// the "Show backlinks at the bottom of notes" key it follows is
    /// `backlinkInDocument`.
    #[test]
    fn the_command_is_runnable_and_gated_on_the_backlink_plugin() {
        assert!(RUNNABLE.contains(&"backlink:toggle-backlinks-in-document"));
        assert_eq!(
            scoria_commands::plugin_of("backlink:toggle-backlinks-in-document"),
            Some("backlink")
        );
    }
}
