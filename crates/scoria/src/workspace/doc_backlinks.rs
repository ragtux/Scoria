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
//! same sections, with the leaf's own folds, and its four nav buttons.
//! The last two were driven on 1.15.0 on 2026-11-16
//! (`tools/obsidian-probe/font-size-backlinks-block.mjs`): "Change sort
//! order" opens the pane's six orders in three pairs, the menu's corner at
//! the pointer, and the pick goes to the leaf's `sortOrder` alone — the
//! dock pane keeps its own and `backlink.json` is not written. "Show
//! search filter" puts a 30 px field on the buttons' row, left of them and
//! `100% - 150px` wide with 12 px under it, and the keyboard in it; what is
//! typed filters both sections as the pane's filter does and is the leaf's
//! `searchQuery`. Escape empties the field and leaves it open with the
//! keyboard in it; the button pressed again hides it, empties the query
//! and hands the keyboard to the note.

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
        self.set_value(key, json!(on));
    }

    /// Sets one of `backlinkOpts`' keys in place.
    fn set_value(&mut self, key: &str, value: Value) {
        self.opts
            .get_or_insert_with(default_opts)
            .insert(key.into(), value);
    }

    /// The leaf's `searchQuery`, empty when it has none.
    pub(super) fn query(&self) -> &str {
        self.opts
            .as_ref()
            .and_then(|o| o.get("searchQuery"))
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// Sets `searchQuery`; `false` when it already held `query`.
    fn set_query(&mut self, query: &str) -> bool {
        if self.query() == query {
            return false;
        }
        self.set_value("searchQuery", json!(query));
        true
    }

    fn set_sort(&mut self, order: SortOrder) {
        self.set_value("sortOrder", json!(order.as_str()));
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
            // The group's one filter field shows the query of the tab it
            // shows now, whichever tab that is.
            entity.update(cx, |ws, cx| ws.sync_doc_filter(host, cx));
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
        let show_search = doc.flag("showSearch");
        let query = doc.query().to_string();
        // The field: on the buttons' row, left of them, `100% - 150px` wide,
        // 30 px tall and 12 px over the list (observed).
        let field = show_search
            .then(|| {
                self.group_ix(id)
                    .map(|ix| self.groups[ix].doc_filter.clone())
            })
            .flatten()
            .map(|input| {
                div()
                    .relative()
                    .flex_shrink_0()
                    .w(px((width - 150.).max(60.)))
                    .h(px(30.))
                    .mb(px(12.))
                    .child(input)
                    .child(
                        div()
                            .absolute()
                            .left(px(8.))
                            .top(px(0.))
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .child(icon("search", 14., theme.tokens.text_muted)),
                    )
                    .children((!query.is_empty()).then(|| {
                        div()
                            .id(("doc-backlinks-clear", id as usize))
                            .absolute()
                            .right(px(2.))
                            .top(px(0.))
                            .w(px(28.))
                            .h(px(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .rounded_full()
                            .tooltip(tooltip("Clear search"))
                            .on_click(listen(
                                this,
                                move |ws: &mut Self, _: &ClickEvent, window, cx| {
                                    ws.doc_clear_filter(id, window, cx)
                                },
                            ))
                            .child(icon("x", 14., theme.tokens.text_muted))
                    }))
            });
        let sort_menu = self.doc_sort_menu_element(id, doc.sort(), theme, this);
        let buttons = div()
            .ml_auto()
            .flex()
            .flex_row()
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
            )
            .child(
                div()
                    .relative()
                    .child(
                        Self::nav_button(
                            "doc-backlinks-sort",
                            "arrow-up-narrow-wide",
                            "Change sort order",
                            false,
                            theme,
                        )
                        .on_click(listen(
                            this,
                            move |ws: &mut Self, event: &ClickEvent, _, cx| {
                                ws.sort_menu_at = event.position();
                                ws.doc_sort_menu = match ws.doc_sort_menu {
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
                    "doc-backlinks-search",
                    "search",
                    "Show search filter",
                    show_search,
                    theme,
                )
                .on_click(listen(
                    this,
                    move |ws: &mut Self, _: &ClickEvent, window, cx| {
                        ws.doc_toggle_search(id, window, cx)
                    },
                )),
            );
        // `div.nav-header` (padding 12px 0 0): the field, then the buttons
        // right-aligned.
        let header = div()
            .w_full()
            .pt(px(12.))
            .flex()
            .flex_row()
            .items_start()
            .children(field)
            .child(buttons);
        let list = self.render_section_list(
            SectionsOwner::Doc(id),
            &sections,
            &path,
            &query,
            theme,
            this,
        );
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

    /// "Change sort order" under the block of group `id`: the pane's six
    /// orders in three pairs, a check against the leaf's, the menu's
    /// top-left corner at the pointer (observed: two pixels off it) and held
    /// inside the window.
    fn doc_sort_menu_element(
        &self,
        id: u64,
        current: SortOrder,
        theme: &Theme,
        this: &gpui::WeakEntity<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.doc_sort_menu != Some(id) {
            return None;
        }
        let close = move |ws: &mut Self, cx: &mut Context<Self>| {
            if ws.doc_sort_menu == Some(id) {
                ws.doc_sort_menu = None;
                cx.notify();
            }
        };
        let mut list = div()
            .id("doc-backlinks-sort-menu")
            .flex()
            .flex_col()
            .min_w(px(190.))
            .py_1()
            .bg(theme.tokens.background_secondary)
            .border_1()
            .border_color(theme.tokens.background_modifier_border)
            .rounded_md()
            .shadow_lg()
            .text_size(px(13.))
            .text_color(theme.tokens.text_normal)
            .track_focus(&self.menu_focus.handle)
            .key_context("Menu")
            .occlude()
            .on_action(listen(
                this,
                move |ws: &mut Self, _: &file_menu::CloseMenu, _, cx| close(ws, cx),
            ))
            .on_mouse_down_out(listen(
                this,
                move |ws: &mut Self, _: &gpui::MouseDownEvent, _, cx| close(ws, cx),
            ));
        for (ix, order) in SortOrder::ALL.into_iter().enumerate() {
            if ix > 0 && ix % 2 == 0 {
                list = list.child(
                    div()
                        .my_1()
                        .h(px(1.))
                        .bg(theme.tokens.background_modifier_border),
                );
            }
            list = list.child(
                div()
                    .id(("doc-backlinks-sort-item", ix))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.tokens.background_modifier_hover))
                    .on_click(listen(this, move |ws: &mut Self, _: &ClickEvent, _, cx| {
                        ws.doc_set_sort(id, order, cx)
                    }))
                    .child(
                        div()
                            .w(px(16.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .children(
                                (order == current)
                                    .then(|| icon("check", 14., theme.tokens.text_muted)),
                            ),
                    )
                    .child(order.label()),
            );
        }
        Some(
            deferred(
                anchored()
                    .position(self.sort_menu_at)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(list),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// A pick from the block's sort menu: the leaf's `sortOrder`, the
    /// dock pane's left as it is.
    fn doc_set_sort(&mut self, id: u64, order: SortOrder, cx: &mut Context<Self>) {
        if let Some(tab) = self.doc_tab_mut(id) {
            tab.doc_backlinks.set_sort(order);
        }
        self.doc_sort_menu = None;
        cx.notify();
    }

    /// "Show search filter" in the block: shown, with the keyboard in the
    /// field; hidden, with the query emptied and the keyboard on the note.
    fn doc_toggle_search(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.doc_tab_mut(id) else {
            return;
        };
        let doc = &mut tab.doc_backlinks;
        let on = !doc.flag("showSearch");
        doc.set_flag("showSearch", on);
        if !on {
            doc.set_query("");
        }
        let Some(input) = self
            .group_ix(id)
            .map(|ix| self.groups[ix].doc_filter.clone())
        else {
            return;
        };
        if on {
            window.focus(&input.focus_handle(cx), cx);
        } else {
            input.update(cx, |input, cx| input.set_text("", cx));
            self.focus_main(window, cx);
        }
        cx.notify();
    }

    /// The field's "Clear search": emptied, the keyboard kept there.
    fn doc_clear_filter(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self
            .group_ix(id)
            .map(|ix| self.groups[ix].doc_filter.clone())
        else {
            return;
        };
        input.update(cx, |input, cx| input.set_text("", cx));
        window.focus(&input.focus_handle(cx), cx);
        cx.notify();
    }

    /// What the field of group `id` says: a keystroke is the shown tab's
    /// `searchQuery`; Escape empties it and keeps the field and the
    /// keyboard where they are (observed).
    pub(super) fn doc_filter_event(&mut self, id: u64, event: &InputEvent, cx: &mut Context<Self>) {
        let Some(input) = self
            .group_ix(id)
            .map(|ix| self.groups[ix].doc_filter.clone())
        else {
            return;
        };
        match event {
            InputEvent::Changed => {
                let text = input.read(cx).text().to_string();
                if let Some(ix) = self.group_ix(id) {
                    self.groups[ix].doc_filter_synced = text.clone();
                }
                let changed = self
                    .doc_tab_mut(id)
                    .is_some_and(|tab| tab.doc_backlinks.set_query(&text));
                if changed {
                    cx.notify();
                }
            }
            InputEvent::Cancel => input.update(cx, |input, cx| input.set_text("", cx)),
            _ => {}
        }
    }

    /// Puts the query of the tab the block under `host` shows into its
    /// group's field, when the tab changed under it: a tab whose query is
    /// not the one the field last agreed on. A keystroke the field has not
    /// yet reported is left alone, since the tab still agrees with the
    /// field's last report.
    fn sync_doc_filter(&mut self, host: gpui::EntityId, cx: &mut Context<Self>) {
        let Some((id, _, doc)) = self.doc_host(host) else {
            return;
        };
        let query = doc.query().to_string();
        let Some(ix) = self.group_ix(id) else {
            return;
        };
        let group = &mut self.groups[ix];
        if group.doc_filter_synced == query {
            return;
        }
        group.doc_filter_synced = query.clone();
        let input = group.doc_filter.clone();
        input.update(cx, |input, cx| input.set_text(query, cx));
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

    /// The 2026-11-16 run: each order picked from the block's menu is the
    /// leaf's `sortOrder` in place; the filter shown and typed into is
    /// `showSearch: true` and the query; hidden again it is
    /// `showSearch: false` with the query emptied. The key order never moves.
    #[test]
    fn the_block_sort_and_filter_write_the_leaf_options_in_place() {
        let mut doc = DocBacklinks::default();
        doc.toggle(false);
        let opts_of = |doc: &DocBacklinks| {
            let mut leaf =
                markdown_leaf(r#"{"file": "Target.md", "mode": "source", "source": false}"#);
            doc.write(&mut leaf, false);
            serde_json::to_string(&leaf["state"]["state"]["backlinkOpts"]).unwrap()
        };
        for (order, written) in [
            (SortOrder::AlphabeticalReverse, "alphabeticalReverse"),
            (SortOrder::ByModifiedTime, "byModifiedTime"),
            (SortOrder::ByModifiedTimeReverse, "byModifiedTimeReverse"),
            (SortOrder::ByCreatedTime, "byCreatedTime"),
            (SortOrder::ByCreatedTimeReverse, "byCreatedTimeReverse"),
            (SortOrder::Alphabetical, "alphabetical"),
        ] {
            doc.set_sort(order);
            assert_eq!(doc.sort(), order);
            assert!(opts_of(&doc).contains(&format!(r#""sortOrder":"{written}""#)));
        }
        doc.set_sort(SortOrder::ByModifiedTime);
        doc.set_flag("showSearch", true);
        assert!(doc.set_query("kiwi"));
        assert!(!doc.set_query("kiwi"), "the same query is no change");
        assert_eq!(
            opts_of(&doc),
            compact(
                r#"{"collapseAll": false, "extraContext": false, "sortOrder": "byModifiedTime", "showSearch": true, "searchQuery": "kiwi", "backlinkCollapsed": false, "unlinkedCollapsed": true}"#
            )
        );
        doc.set_flag("showSearch", false);
        doc.set_query("");
        assert_eq!(doc.query(), "");
        assert_eq!(
            opts_of(&doc),
            compact(
                r#"{"collapseAll": false, "extraContext": false, "sortOrder": "byModifiedTime", "showSearch": false, "searchQuery": "", "backlinkCollapsed": false, "unlinkedCollapsed": true}"#
            )
        );
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
            note_stamp: 0,
            link: None,
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
