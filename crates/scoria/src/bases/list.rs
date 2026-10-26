//! The `list` layout: one line per row, its values joined.
//!
//! Its keys are observed (see [`scoria_bases::layout`] and
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//! `markers` (`bullet`, `number` or `none`, default `bullet`), `separator`
//! (any string, default `", "`) and the "Indent properties" toggle, whose own
//! key is **unprobed** — the toggle produced no write and the registration
//! names it only as a minified constant.
//!
//! The rows, the columns and the grouping are the table's: every layout draws
//! the same resolved [`Table`](scoria_bases::Table), so a line shows the same
//! cells a row would, joined by `separator`.
//!
//! **Unprobed**: no list view was ever rendered in the run, so the line's own
//! box — where the marker sits, which cell leads — is not observed.

use crate::bases::view::BasesView;
use crate::theme::Theme;
use gpui::{AnyElement, Context, ScrollHandle};
use serde_json::{json, Value as Json};

/// What the list layout keeps between renders.
#[derive(Default)]
pub struct ListLayoutState {
    pub scroll: ScrollHandle,
    /// How many lines the body holds.
    pub lines: usize,
}

impl ListLayoutState {
    pub fn state_json(&self) -> Json {
        json!({ "lines": self.lines })
    }
}

impl BasesView {
    pub(super) fn rebuild_list(&mut self) {
        self.list.lines = self.table().map_or(0, |t| t.rows.len());
        // The fallback below draws the table, which needs its own body
        // rebuilt; a list layout that draws lines will not need this.
        self.rebuild_table();
    }

    /// Draws the lines.
    ///
    /// Until this layout is built the table is what a list view shows,
    /// which is what the read-only first cut did for all four layouts.
    pub(super) fn render_list(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.render_table(theme, cx)
    }
}
