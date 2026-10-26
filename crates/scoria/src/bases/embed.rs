//! An embedded base: `![[x.base]]` inside a note.
//!
//! The reading view resolves the embed through [`resolve`] and draws what it
//! answers, which is the same [`Resolved`](crate::bases::rows::Resolved) the
//! view itself draws — one engine, one set of rules, whether a base is open
//! in a leaf or embedded in a note.
//!
//! **Unprobed**: an embedded base was never captured in the run — it is named
//! in the write-behavior section's own unprobed list, along with what
//! `createAndEmbedBase` writes — so the box around it, whether it carries a
//! toolbar, and how tall it is drawn are this build's reading rather than
//! observations.

use crate::bases::rows;
use crate::theme::{Theme, FONT_UI_SMALL};
use crate::workspace::VaultState;
use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};
use scoria_bases::{Layout, Table};

/// A `.base` resolved for an embed: what the first view of the file shows.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddedBase {
    /// The base's own vault path, which is what a click opens.
    pub path: String,
    /// The showing view's name, which is the title an embed draws.
    pub view_name: Option<String>,
    pub layout: Layout,
    pub table: Option<Table>,
    /// The parse failure, if the file has one: an embedded broken base says
    /// the same thing the view does.
    pub error: Option<String>,
}

/// Resolves the base at `path` for an embed, on its **first** view — a leaf
/// remembers which view it was on, an embed has nowhere to remember one.
pub fn resolve(state: &VaultState, path: &str) -> Option<EmbeddedBase> {
    let text = std::fs::read_to_string(state.vault().absolute(path)).ok()?;
    let resolved = rows::resolve(Some(state), Some(path), &text, 0);
    let view = resolved.base.as_ref().and_then(|b| b.views.first());
    Some(EmbeddedBase {
        path: path.to_string(),
        view_name: view.map(|v| v.name.clone()),
        layout: view.map(Layout::of).unwrap_or_default(),
        table: resolved.table,
        error: resolved.error,
    })
}

/// Draws an embedded base inside a note.
///
/// `open` is what a click on its title does: the reading view follows an
/// internal link, which is how every other embed's title opens its target.
///
/// **Unprobed**, as the module note says: the box here is the shell's own
/// embed box — the one a note embed draws — with the base's view name as its
/// title.
pub fn render(
    base: &EmbeddedBase,
    theme: &Theme,
    open: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let title = base.view_name.clone().unwrap_or_else(|| base.path.clone());
    let rows = base.table.as_ref().map_or(0, |t| t.rows.len());
    let body = match (&base.error, &base.table) {
        (Some(message), _) => message.clone(),
        (None, Some(table)) => match &table.unknown_kind {
            Some(kind) => format!("Unknown view type: {kind}"),
            None => format!("{rows} results"),
        },
        (None, None) => String::new(),
    };
    div()
        .my_2()
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .px_4()
        .py_3()
        .flex()
        .flex_col()
        .child(
            div()
                .id(("bases-embed-title", base.path.len()))
                .mb_2()
                .text_size(px(theme.base_font_size * 0.75))
                .text_color(theme.text_muted)
                .cursor_pointer()
                .hover(|s| s.text_color(theme.link))
                .on_click(move |_, window, cx| open(window, cx))
                .child(SharedString::from(format!("\u{2197} {title}"))),
        )
        .child(
            div()
                .text_size(px(FONT_UI_SMALL))
                .text_color(theme.text_faint)
                .child(SharedString::from(body)),
        )
        .into_any_element()
}
