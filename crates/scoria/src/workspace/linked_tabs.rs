//! **Link with tab...** and **Unlink tab**, and what linked tabs do, as
//! Obsidian 1.15.1 does them (`tools/obsidian-probe/link-with-tab.mjs`, see
//! [[Specification#Observed Link with tab, Obsidian 1.15.1]]). Built on the
//! link id [`local_graph_tab`] gave every tab: the leaf's `group`.
//!
//! **The pick.** "Link with tab..." on a tab's menu starts a mode with
//! nothing drawn until the pointer moves: then the group under it is washed
//! below its strip in the accent at half strength, rounded 8 px, the drop
//! overlay a dragged tab shows. A press of any button on a group links the
//! source with the tab that group shows (its strip included: a header
//! names its group, not its own tab), and makes that group active. A press
//! on the source's own leaf or anywhere outside the groups ends the mode
//! with nothing linked; Escape does not end it.
//!
//! **The id.** The source joins the target's link, or both take a new id
//! when the target has none ([`joined_link`]). A tab already linked offers
//! "Unlink tab" in that row instead, so a source is never linked. Unlinking
//! takes the one tab out; a link left with one tab is dissolved
//! ([`lone_links`]), which is also what closing a tab of a pair does.
//!
//! **Following.** When a linked tab opens a file — a link followed, a pick,
//! back or forward — every other tab in its link opens it too, pushing it
//! onto its own history and keeping its own mode ([`followers`]). An empty
//! tab and the global graph are left alone; a local graph follows through
//! its own sync. Linking opens nothing. A restored layout whose linked
//! tabs disagree is settled once, the later tab in the layout winning
//! ([`settled_leader`]), as Obsidian's relaunch did.
//!
//! **Pin** on a linked tab pins (or unpins) every tab in its link.
//!
//! **The header icon.** A linked tab draws `link` in the accent before its
//! close button, 14 px in an 18 px box; under the pointer it becomes
//! `unlink` on a tenth of the accent and every leaf of the link is washed
//! in a quarter of it; a click unlinks the tab.

use super::*;
use gpui::{MouseMoveEvent, Point};

/// Where the pick mode stands, the header icon's hover, and what the next
/// sync has to settle.
#[derive(Default)]
pub(super) struct Linking {
    /// The tab "Link with tab..." was picked on: its group's id and index.
    pick: Option<(u64, usize)>,
    /// The group under the pointer while picking.
    hover: Option<u64>,
    /// Every group's box as last laid out while picking, for the press.
    bounds: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// The tab whose link icon is under the pointer, and its link.
    icon_hover: Option<(u64, usize, String)>,
    /// The active tab moved to another file: its group's id and index.
    moved: Option<(u64, usize)>,
    /// A layout was restored: its links are settled at the next sync.
    reconcile: bool,
}

/// The link a source joins: the target's, or a new id when it has none.
pub(super) fn joined_link(target: Option<&str>, fresh: impl FnOnce() -> String) -> String {
    target.map(str::to_string).unwrap_or_else(fresh)
}

/// The links only one tab holds, which Obsidian dissolves: the tab left
/// behind loses its `group`.
pub(super) fn lone_links<'a>(links: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    for link in links {
        *count.entry(link).or_default() += 1;
    }
    count
        .into_iter()
        .filter(|(_, n)| *n == 1)
        .map(|(l, _)| l.to_string())
        .collect()
}

/// The members of one link that follow `leader`, by index: every other
/// member showing a file (an empty tab does not follow) and not on the
/// leader's file already.
pub(super) fn followers(files: &[Option<&str>], leader: usize) -> Vec<usize> {
    let Some(Some(to)) = files.get(leader) else {
        return Vec::new();
    };
    files
        .iter()
        .enumerate()
        .filter(|(ix, f)| *ix != leader && f.is_some_and(|f| f != *to))
        .map(|(ix, _)| ix)
        .collect()
}

/// The member whose file a restored link settles on, when its members
/// showing a file disagree: the last of them in layout order.
pub(super) fn settled_leader(files: &[Option<&str>]) -> Option<usize> {
    let shown: Vec<(usize, &str)> = files
        .iter()
        .enumerate()
        .filter_map(|(ix, f)| f.map(|f| (ix, f)))
        .collect();
    let (last, file) = *shown.last()?;
    shown.iter().any(|(_, f)| *f != file).then_some(last)
}

impl Workspace {
    /// Every tab in layout order (the main area's groups, then each
    /// popout's), with its group's id and index, the active tab's live
    /// fields folded in.
    fn layout_tabs(&self) -> Vec<(u64, usize, Tab)> {
        let mut order = Vec::new();
        self.layout_tree.groups(&mut order);
        for w in &self.windows {
            w.tree.groups(&mut order);
        }
        let mut out = Vec::new();
        for id in order {
            let Some(gi) = self.group_ix(id) else {
                continue;
            };
            let tabs = if gi == self.active_group {
                self.tab_snapshots()
            } else {
                self.groups[gi].tabs.clone()
            };
            out.extend(tabs.into_iter().enumerate().map(|(ix, t)| (id, ix, t)));
        }
        out
    }

    fn tab_slot(&mut self, group: u64, ix: usize) -> Option<&mut Tab> {
        let gi = self.group_ix(group)?;
        if gi == self.active_group {
            self.tabs.get_mut(ix)
        } else {
            self.groups[gi].tabs.get_mut(ix)
        }
    }

    /// The file a tab follows and leads with: none for an empty tab, a
    /// local graph or the global graph.
    fn linked_file(tab: &Tab) -> Option<&str> {
        if tab.mode == ViewMode::Graph || local_graph_tab::is_local_graph(tab) {
            return None;
        }
        tab.path.as_deref()
    }

    /// Marks the active tab as having moved to another file, for its link
    /// to follow at the next sync: what an open and back or forward call.
    pub(super) fn linked_tab_moved(&mut self) {
        self.linking.moved = Some((self.active_group_id(), self.active_tab));
    }

    /// Marks a restored layout's links for settling.
    pub(super) fn reconcile_links(&mut self) {
        self.linking.reconcile = true;
    }

    /// The frame's link work, before the local graphs sync: links left with
    /// one tab dissolved, a restored layout's links settled, and the tabs
    /// linked to one that moved brought to its file.
    pub(super) fn sync_linked_tabs(&mut self, cx: &mut Context<Self>) {
        let tabs = self.layout_tabs();
        let lone = lone_links(tabs.iter().filter_map(|(_, _, t)| t.link.as_deref()));
        for (group, ix, tab) in &tabs {
            if tab.link.as_ref().is_some_and(|l| lone.contains(l)) {
                if let Some(slot) = self.tab_slot(*group, *ix) {
                    slot.link = None;
                }
            }
        }
        let mut moves: Vec<(u64, usize, String)> = Vec::new();
        let members = |link: &str| -> Vec<(u64, usize, Option<&str>)> {
            tabs.iter()
                .filter(|(_, _, t)| t.link.as_deref() == Some(link) && !lone.contains(link))
                .map(|(g, i, t)| (*g, *i, Self::linked_file(t)))
                .collect()
        };
        if std::mem::take(&mut self.linking.reconcile) {
            let mut seen: HashSet<&str> = HashSet::new();
            for link in tabs.iter().filter_map(|(_, _, t)| t.link.as_deref()) {
                if !seen.insert(link) {
                    continue;
                }
                let m = members(link);
                let files: Vec<Option<&str>> = m.iter().map(|(_, _, f)| *f).collect();
                if let Some(leader) = settled_leader(&files) {
                    let to = files[leader].unwrap_or_default().to_string();
                    for f in followers(&files, leader) {
                        moves.push((m[f].0, m[f].1, to.clone()));
                    }
                }
            }
        }
        if let Some((group, ix)) = self.linking.moved.take() {
            let leader = tabs.iter().find(|(g, i, _)| *g == group && *i == ix);
            if let Some(link) = leader.and_then(|(_, _, t)| t.link.as_deref()) {
                let m = members(link);
                let files: Vec<Option<&str>> = m.iter().map(|(_, _, f)| *f).collect();
                if let Some(at) = m.iter().position(|(g, i, _)| *g == group && *i == ix) {
                    let to = files[at].unwrap_or_default().to_string();
                    for f in followers(&files, at) {
                        moves.push((m[f].0, m[f].1, to.clone()));
                    }
                }
            }
        }
        for (group, ix, path) in moves {
            self.follow_in(group, ix, &path, cx);
        }
    }

    /// Tab `ix` of group `group` opens `path` because a tab linked to it
    /// did: pushed onto its history in its own mode, the keyboard left
    /// where it is, a pinned tab no exception. A tab not on screen moves on
    /// in its slot; one on screen in another group has that group made live
    /// for the open and the active group made live again after.
    fn follow_in(&mut self, group: u64, ix: usize, path: &str, cx: &mut Context<Self>) {
        let Some(gi) = self.group_ix(group) else {
            return;
        };
        let live = gi == self.active_group;
        let shown = if live {
            self.active_tab == ix
        } else {
            self.groups[gi].active_tab == ix
        };
        if live && shown {
            self.follow_live(path, cx);
            return;
        }
        if !shown {
            if let Some(tab) = self.tab_slot(group, ix) {
                tab.history.truncate(tab.history_ix);
                tab.history
                    .push(HistoryEntry::new(path.to_string(), tab.mode));
                tab.history_ix = tab.history.len();
                tab.path = Some(path.to_string());
            }
            return;
        }
        let back = self.active_group_id();
        let refocus = self.refocus_main;
        self.flush_save(cx);
        self.stash_active_group();
        self.load_group(gi, cx);
        self.follow_live(path, cx);
        self.stash_active_group();
        if let Some(ix) = self.group_ix(back) {
            self.load_group(ix, cx);
        }
        self.refocus_main = refocus;
    }

    /// The live tab follows to `path`: what an open does, short of the
    /// pinned tab's new tab, the keyboard and the recent files.
    fn follow_live(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(state) = self.state.clone() else {
            return;
        };
        if self.current.as_deref() == Some(path) {
            return;
        }
        self.flush_save(cx);
        self.record_history_state(cx);
        self.history.truncate(self.history_ix);
        self.history
            .push(HistoryEntry::new(path.to_string(), self.mode));
        self.history_ix = self.history.len();
        self.show_file(path, &state, cx);
        if self.current.as_deref() != Some(path) {
            self.forget_history(path);
        }
        cx.notify();
    }

    // ----- the menu rows -----------------------------------------------------

    /// "Link with tab..." on tab `ix` of group `group`: the pick begins.
    pub(super) fn start_link_pick(&mut self, group: u64, ix: usize, cx: &mut Context<Self>) {
        self.stash_active_group();
        self.linking.pick = Some((group, ix));
        self.linking.hover = None;
        self.linking.bounds.borrow_mut().clear();
        cx.notify();
    }

    /// "Unlink tab", or a click on the header's link icon: the one tab out
    /// of its link; a link left with one tab goes at the next sync.
    pub(super) fn unlink_tab(&mut self, group: u64, ix: usize, cx: &mut Context<Self>) {
        self.stash_active_group();
        if let Some(tab) = self.tab_slot(group, ix) {
            tab.link = None;
        }
        self.linking.icon_hover = None;
        cx.notify();
    }

    /// Pin's row on a linked tab: the pin the tab now has given to every
    /// tab of its link.
    pub(super) fn pin_link_of(&mut self, group: u64, ix: usize) {
        self.stash_active_group();
        let Some(tab) = self.tab_slot(group, ix) else {
            return;
        };
        let (Some(link), pinned) = (tab.link.clone(), tab.pinned) else {
            return;
        };
        for (g, i, t) in self.layout_tabs() {
            if t.link.as_deref() == Some(link.as_str()) {
                if let Some(slot) = self.tab_slot(g, i) {
                    slot.pinned = pinned;
                }
            }
        }
    }

    // ----- the pick ----------------------------------------------------------

    /// The group whose box holds `at`, among the window being drawn's.
    fn link_pick_group_at(&self, at: Point<Pixels>, window: Option<u64>) -> Option<u64> {
        let mut order = Vec::new();
        self.tree(window).groups(&mut order);
        let bounds = self.linking.bounds.borrow();
        order
            .into_iter()
            .find(|g| bounds.get(g).is_some_and(|b| b.contains(&at)))
    }

    fn link_pick_moved(&mut self, at: Point<Pixels>, window: Option<u64>, cx: &mut Context<Self>) {
        let hover = self.link_pick_group_at(at, window);
        if hover != self.linking.hover {
            self.linking.hover = hover;
            cx.notify();
        }
    }

    /// A press while picking: on a group, the source linked with the tab
    /// that group shows and the group made active; on the source itself or
    /// outside the groups, the mode ends with nothing linked.
    fn link_pick_pressed(
        &mut self,
        at: Point<Pixels>,
        window: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let target = self.link_pick_group_at(at, window);
        let Some(source) = self.linking.pick.take() else {
            return;
        };
        self.linking.hover = None;
        cx.notify();
        let Some(group) = target else {
            return;
        };
        self.activate_group(group, cx);
        self.stash_active_group();
        let Some(gi) = self.group_ix(group) else {
            return;
        };
        let ix = self.groups[gi].active_tab;
        if (group, ix) == source {
            return;
        }
        let Some(target_link) = self.tab_slot(group, ix).map(|t| t.link.clone()) else {
            return;
        };
        let link = joined_link(target_link.as_deref(), leaf_id);
        if let Some(t) = self.tab_slot(group, ix) {
            t.link = Some(link.clone());
        }
        if let Some(t) = self.tab_slot(source.0, source.1) {
            t.link = Some(link);
        }
    }

    /// The layer over the whole window while picking: it takes every move
    /// and press, so nothing under it reacts, and finds the group under
    /// the pointer from the boxes the groups recorded.
    pub(super) fn render_link_pick(
        &self,
        window: Option<u64>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.linking.pick?;
        Some(
            deferred(
                div()
                    .id("link-pick")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                        this.link_pick_moved(e.position, window, cx)
                    }))
                    .on_any_mouse_down(cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.link_pick_pressed(e.position, window, cx)
                    })),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// What a group draws over itself for the links: while picking, the box
    /// it records and, under the pointer, the accent overlay below its
    /// strip; with a link icon under the pointer, the wash over a group
    /// showing a tab of that link.
    pub(super) fn render_link_marks(
        &self,
        group: u64,
        shown: Option<&Tab>,
        theme: &Theme,
    ) -> Option<gpui::AnyElement> {
        let accent = theme.tokens.interactive_accent;
        if self.linking.pick.is_some() {
            let bounds = self.linking.bounds.clone();
            let record = canvas(
                move |b, _, _| {
                    bounds.borrow_mut().insert(group, b);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full();
            let over = (self.linking.hover == Some(group)).then(|| {
                div()
                    .absolute()
                    .top(px(HEADER_HEIGHT))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .rounded(px(8.))
                    .bg(accent.opacity(0.5))
            });
            return Some(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(record)
                    .children(over)
                    .into_any_element(),
            );
        }
        let (_, _, link) = self.linking.icon_hover.as_ref()?;
        (shown?.link.as_deref() == Some(link.as_str())).then(|| {
            div()
                .absolute()
                .top(px(HEADER_HEIGHT))
                .left_0()
                .right_0()
                .bottom_0()
                .bg(accent.opacity(0.25))
                .into_any_element()
        })
    }

    /// A linked tab's status icon, before its close button.
    pub(super) fn render_link_icon(
        &self,
        group: u64,
        ix: usize,
        link: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let hovered = self
            .linking
            .icon_hover
            .as_ref()
            .is_some_and(|(g, i, _)| *g == group && *i == ix);
        let t = &theme.tokens;
        div()
            .id(("tab-link", ix))
            .flex_shrink_0()
            .w(px(18.))
            .h(px(18.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .when(hovered, |d| d.bg(t.interactive_accent.opacity(0.1)))
            // Its `aria-label` is "Unlink tab", but no tooltip came up
            // under the pointer (`s04`).
            .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                let next = over.then(|| (group, ix, link.clone()));
                if this.linking.icon_hover != next {
                    this.linking.icon_hover = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.unlink_tab(group, ix, cx)
            }))
            .child(icon(
                if hovered { "unlink" } else { "link" },
                14.,
                t.text_accent,
            ))
            .into_any_element()
    }

    /// The links for the control socket: the pick and the icon's hover.
    pub(super) fn describe_linking(&self) -> Value {
        json!({
            "picking": self.linking.pick.map(|(g, i)| json!({"group": g, "tab": i})),
            "hover": self.linking.hover,
            "icon_hover": self.linking.icon_hover.as_ref().map(|(g, i, l)| json!({"group": g, "tab": i, "link": l})),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_joins_the_targets_link_or_both_take_a_new_one() {
        assert_eq!(
            joined_link(Some("e47f88ecd2cab5de"), || unreachable!()),
            "e47f88ecd2cab5de"
        );
        assert_eq!(joined_link(None, || "fresh".into()), "fresh");
    }

    #[test]
    fn a_link_left_with_one_tab_is_dissolved() {
        let lone = lone_links(["a", "a", "b", "c", "c", "c"]);
        assert_eq!(lone, HashSet::from(["b".to_string()]));
        assert!(lone_links(["a", "a"]).is_empty());
    }

    #[test]
    fn the_others_follow_the_tab_that_moved_but_an_empty_tab_does_not() {
        // Alpha moved to Beta; Gamma follows, the empty tab and a tab on
        // Beta already do not.
        let files = [Some("Beta.md"), Some("Gamma.md"), None, Some("Beta.md")];
        assert_eq!(followers(&files, 0), vec![1]);
        // Either way round.
        let files = [Some("Alpha.md"), Some("Delta.md")];
        assert_eq!(followers(&files, 1), vec![0]);
        // An empty leader leads nowhere.
        assert!(followers(&[None, Some("A.md")], 0).is_empty());
    }

    #[test]
    fn a_restored_link_settles_on_its_last_tab() {
        // Obsidian's relaunch: Alpha before Gamma in the layout, both came
        // back on Gamma, whichever was active.
        assert_eq!(
            settled_leader(&[Some("Alpha.md"), Some("Gamma.md")]),
            Some(1)
        );
        assert_eq!(
            settled_leader(&[Some("Alpha.md"), Some("Gamma.md"), None]),
            Some(1)
        );
        assert_eq!(settled_leader(&[Some("A.md"), None, Some("A.md")]), None);
        assert_eq!(settled_leader(&[None, None]), None);
    }
}
