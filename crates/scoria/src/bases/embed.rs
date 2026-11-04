//! An embedded base: `![[x.base]]` inside a note.
//!
//! **Observed** (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`,
//! `s12-embed` and `s22-fix-embed`, in the reading view and in Live
//! Preview alike): an embedded base is the **whole view** — the toolbar
//! with its live count and badges, every panel, the layout the view names,
//! the cell editors and *New* — working in place and writing the `.base`
//! file, inside a box with no title, no border, no padding and a
//! transparent ground (`border: 0px`, `radius: 0px`, `padding: 0px`,
//! `bg: rgba(0, 0, 0, 0)`). The box is as tall as its content and never
//! scrolls on its own (`maxHeight: none`, `overflowY: visible`, a table of
//! 8 rows measured 553 px, the same base's cards 1978 px); a table wider
//! than the note is clipped at the note's edge (`overflow: hidden`, a 1055
//! px table in a 604 px embed). `![[x.base#Cards]]` shows the view of that
//! name; a plain `![[x.base]]` shows the first. Live Preview adds **Edit
//! this block** (`lucide-code-2`, 28 by 24, top-right with 8 px above and
//! 4 px to the right), which puts the caret on the embed's line; the reading
//! view adds nothing. A right click inside the embed in Live Preview raises
//! the editor's own menu; in the reading view it raises nothing.
//!
//! So an embed is a live [`BasesView`] — the same entity a leaf shows,
//! in its embedded mode — and this module is the **registry** of those
//! views. The reading model carries only what names one
//! ([`EmbeddedBase`]: the path, the view the subpath picks, and which
//! occurrence of the same embed in the host this is); a host (a reading
//! view or an editor) hands its model's embeds to [`EmbedViews::sync`]
//! whenever the model changes, which makes a view for each new one, keeps
//! the ones that are still there — so a keystroke in Live Preview does not
//! lose an open panel — and drops the rest. The workspace owns the
//! registry, subscribes to every view it makes (that is what
//! [`EmbedViews::on_create`] is for) and saves the ones that edit their
//! file, exactly as it does the leaf's.
//!
//! What the hosts draw is the entity itself; a host that has no registry
//! (a canvas card drawing a note) draws [`render_fallback`], the
//! note-embed box with the base's name, which opens the base when clicked.

use crate::bases::BasesView;
use crate::reading::model::{EmbedKind, RBlock, Segment};
use crate::theme::Theme;
use crate::workspace::VaultState;
use gpui::{div, prelude::*, px, AnyElement, App, Entity, EntityId, SharedString, Window};
use serde_json::{json, Value as Json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// What the reading model keeps for an embedded base: enough to name the
/// live view that draws it, and nothing of the rows, which are the view's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedBase {
    /// The base's vault path.
    pub path: String,
    /// The `#View` subpath — the view to show, by name — or `None` for the
    /// file's first view.
    pub view: Option<String>,
    /// Which occurrence of this same `path` and `view` in the host this is,
    /// from 0. The same base embedded twice in a note is two live views,
    /// each with its own open panel and scroll.
    pub nth: usize,
}

/// What tells one embed from another in a host: its path and view,
/// compared the way every other vault path is, and its ordinal.
pub type Key = (String, Option<String>, usize);

impl EmbeddedBase {
    pub fn key(&self) -> Key {
        (
            self.path.to_lowercase(),
            self.view.as_ref().map(|v| v.to_lowercase()),
            self.nth,
        )
    }
}

/// What a click on the fallback box opens, by vault path.
pub type OpenPath = Rc<dyn Fn(&str, &mut Window, &mut App)>;

type OnCreate = Rc<dyn Fn(&Entity<BasesView>, &mut App)>;

/// The live views behind every embedded base on screen, shared by the
/// workspace, its reading views and its editors. Cloning shares it.
#[derive(Clone, Default)]
pub struct EmbedViews {
    inner: Rc<RefCell<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// The vault the views resolve against; with none, nothing is made.
    state: Option<Arc<VaultState>>,
    /// Per host, the views it draws, by key.
    hosts: HashMap<EntityId, HashMap<Key, Entity<BasesView>>>,
    /// Runs once for each view made, with the app at hand: the workspace
    /// subscribes to the view's events there.
    on_create: Option<OnCreate>,
}

impl EmbedViews {
    /// The vault the views are made over. `None` (the vault closed) drops
    /// every view.
    pub fn set_state(&self, state: Option<Arc<VaultState>>) {
        let mut inner = self.inner.borrow_mut();
        if state.is_none() {
            inner.hosts.clear();
        }
        inner.state = state;
    }

    pub fn on_create(&self, f: impl Fn(&Entity<BasesView>, &mut App) + 'static) {
        self.inner.borrow_mut().on_create = Some(Rc::new(f));
    }

    /// Host `host`'s model changed: `embeds` are the bases it now draws, in
    /// the order the model lists them. A view the host still draws is
    /// kept; a new one is made and loaded on the view its subpath names;
    /// one no longer drawn is dropped.
    ///
    /// A base that cannot be read makes no view, and the host draws its
    /// missing-file line, which is what the model decided on the same
    /// read.
    pub fn sync(&self, host: EntityId, embeds: &[EmbeddedBase], cx: &mut App) {
        let (state, mut old, on_create) = {
            let mut inner = self.inner.borrow_mut();
            (
                inner.state.clone(),
                inner.hosts.remove(&host).unwrap_or_default(),
                inner.on_create.clone(),
            )
        };
        let Some(state) = state else {
            return;
        };
        let mut new: HashMap<Key, Entity<BasesView>> = HashMap::new();
        for embed in embeds {
            let key = embed.key();
            if new.contains_key(&key) {
                continue;
            }
            let view = match old.remove(&key) {
                Some(view) => view,
                None => {
                    let Ok(text) = std::fs::read_to_string(state.vault().absolute(&embed.path))
                    else {
                        continue;
                    };
                    let view = cx.new(|cx| {
                        let mut view = BasesView::new(cx);
                        view.set_embedded();
                        view
                    });
                    view.update(cx, |view, cx| {
                        view.load(&embed.path, &text, state.clone(), cx);
                        if let Some(name) = embed.view.as_deref() {
                            view.show_view_named(name, cx);
                        }
                    });
                    if let Some(on_create) = on_create.as_ref() {
                        on_create(&view, cx);
                    }
                    view
                }
            };
            new.insert(key, view);
        }
        self.inner.borrow_mut().hosts.insert(host, new);
        // `old` drops here with whatever the host no longer draws.
    }

    /// The view host `host` draws for `embed`, if one was made.
    pub fn get(&self, host: EntityId, embed: &EmbeddedBase) -> Option<Entity<BasesView>> {
        self.inner
            .borrow()
            .hosts
            .get(&host)
            .and_then(|views| views.get(&embed.key()))
            .cloned()
    }

    /// Every view in every host.
    pub fn all(&self) -> Vec<Entity<BasesView>> {
        self.inner
            .borrow()
            .hosts
            .values()
            .flat_map(|views| views.values().cloned())
            .collect()
    }

    /// Every view over the base at `path`, whichever host draws it.
    pub fn of_path(&self, path: &str) -> Vec<Entity<BasesView>> {
        self.inner
            .borrow()
            .hosts
            .values()
            .flat_map(|views| {
                views
                    .iter()
                    .filter(|((p, _, _), _)| p.eq_ignore_ascii_case(path))
                    .map(|(_, view)| view.clone())
            })
            .collect()
    }

    /// How many views there are, over every host.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner
            .borrow()
            .hosts
            .values()
            .map(|views| views.len())
            .sum()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The embedded bases of a model's blocks, in document order — what a host
/// hands to [`EmbedViews::sync`].
pub fn embeds_in(blocks: &[RBlock]) -> Vec<EmbeddedBase> {
    embeds_in_each(blocks.iter())
}

/// The same over blocks held one by one, the way Live Preview holds its
/// widgets.
pub fn embeds_in_each<'a>(blocks: impl Iterator<Item = &'a RBlock>) -> Vec<EmbeddedBase> {
    let mut out = Vec::new();
    for block in blocks {
        walk(std::slice::from_ref(block), &mut |base| {
            out.push(base.clone())
        });
    }
    out
}

/// Whether a Live Preview widget's block is an embedded base, which earns
/// the **Edit this block** button over it.
pub fn is_base_embed(block: &RBlock) -> bool {
    let mut found = false;
    walk(std::slice::from_ref(block), &mut |_| found = true);
    found
        && matches!(block, RBlock::Paragraph(segments)
            if segments.len() == 1 && matches!(&segments[0], Segment::Embed(_)))
}

fn walk(blocks: &[RBlock], f: &mut dyn FnMut(&EmbeddedBase)) {
    for block in blocks {
        match block {
            RBlock::Paragraph(segments) => {
                for segment in segments {
                    if let Segment::Embed(embed) = segment {
                        if let EmbedKind::Base(base) = &embed.kind {
                            f(base);
                        }
                        if let EmbedKind::Note { blocks, .. } = &embed.kind {
                            walk(blocks, f);
                        }
                    }
                }
            }
            RBlock::Quote(children) | RBlock::Callout { children, .. } => walk(children, f),
            RBlock::List { items, .. } => {
                for item in items {
                    walk(&item.children, f);
                }
            }
            RBlock::Footnotes(notes) => {
                for (_, body) in notes {
                    walk(body, f);
                }
            }
            _ => {}
        }
    }
}

/// Every embedded base of `blocks` as the control socket reports it: what
/// the model named, and — through `live` — what the view behind it shows.
/// Without this a harness could not tell `![[x.base]]` drew anything
/// without a screenshot.
pub fn describe_all(blocks: &[RBlock], live: &dyn Fn(&EmbeddedBase) -> Option<Json>) -> Vec<Json> {
    let mut out = Vec::new();
    walk(blocks, &mut |base| {
        let mut value = json!({
            "path": base.path,
            "view": base.view,
            "nth": base.nth,
            "live": false,
        });
        if let Some(state) = live(base) {
            value["live"] = json!(true);
            value["state"] = state;
        }
        out.push(value);
    });
    out
}

/// What a host with no registry draws: the note-embed box with the base's
/// name, opening the base on a click. The canvas's cards draw their notes
/// this way; the two hosts that show a note at full size draw the view
/// itself.
pub fn render_fallback(base: &EmbeddedBase, theme: &Theme, open: OpenPath) -> AnyElement {
    let name = base
        .path
        .rsplit('/')
        .next()
        .unwrap_or(&base.path)
        .trim_end_matches(".base")
        .to_string();
    let title = match base.view.as_deref() {
        Some(view) => format!("{name} > {view}"),
        None => name,
    };
    let path = base.path.clone();
    div()
        .w_full()
        .my(px(8.))
        .border_1()
        .border_color(theme.tokens.bases_embed_border_color)
        .rounded_md()
        .px_4()
        .py_3()
        .child(
            div()
                .id(("base-embed-title", base.nth))
                .text_size(px(theme.base_font_size * 0.75))
                .text_color(theme.tokens.text_muted)
                .cursor_pointer()
                .hover(|s| s.text_color(theme.tokens.link_color))
                .on_click(move |_, window, cx| open(&path, window, cx))
                .child(SharedString::from(format!("↗ {title}"))),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_compares_the_path_and_view_case_insensitively_and_keeps_the_ordinal() {
        let a = EmbeddedBase {
            path: "Shelf/Rocks.base".into(),
            view: Some("Cards".into()),
            nth: 1,
        };
        let b = EmbeddedBase {
            path: "shelf/rocks.BASE".into(),
            view: Some("cards".into()),
            nth: 1,
        };
        assert_eq!(a.key(), b.key());
        let c = EmbeddedBase {
            nth: 0,
            ..a.clone()
        };
        assert_ne!(a.key(), c.key(), "the same embed twice is two views");
        let d = EmbeddedBase {
            view: None,
            ..a.clone()
        };
        assert_ne!(a.key(), d.key(), "a subpath picks another view");
    }

    #[test]
    fn a_registry_with_no_vault_makes_nothing() {
        let views = EmbedViews::default();
        assert!(views.is_empty());
        assert!(views.of_path("Rocks.base").is_empty());
        assert!(views.all().is_empty());
    }
}
