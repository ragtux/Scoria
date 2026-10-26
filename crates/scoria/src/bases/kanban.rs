//! The `kanban` layout: a column per `groupBy` value, cards inside it.
//!
//! Its keys are observed (see [`scoria_bases::layout`] and
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//! `columnWidth` (200 to 500 step 10, default 280), `image` (a property id),
//! `imageFit` (`""` Cover or `contain`) and `imageAspectRatio` (0.25 to 2.5
//! step 0.05, default **0.5** — the one key whose default differs from the
//! cards layout's).
//!
//! A kanban column is a group: the engine's own `groupBy`, `groupOrder` and
//! direction decide which columns there are and in what order, and the
//! no-value group leads under an ascending `groupBy`. A view with no
//! `groupBy` therefore has one column holding every row, which is the shape
//! [`Table::groups`](scoria_bases::Table) already answers for one.
//!
//! **Unprobed**: no kanban view was ever rendered in the run, so the column's
//! own header, whether a card drags between columns, and what such a drag
//! writes are all unobserved.

use crate::bases::view::BasesView;
use crate::theme::Theme;
use gpui::{AnyElement, Context, ScrollHandle};
use serde_json::{json, Value as Json};

/// What the kanban layout keeps between renders.
#[derive(Default)]
pub struct KanbanState {
    /// The board scrolls sideways through its columns.
    pub scroll: ScrollHandle,
    /// How many columns the board holds.
    pub columns: usize,
}

impl KanbanState {
    pub fn state_json(&self) -> Json {
        json!({ "columns": self.columns })
    }
}

impl BasesView {
    pub(super) fn rebuild_kanban(&mut self) {
        self.kanban.columns = self.table().map_or(0, |t| t.groups.len());
        // The fallback below draws the table, which needs its own body
        // rebuilt; a kanban layout that draws columns will not need this.
        self.rebuild_table();
    }

    /// Draws the board.
    ///
    /// Until this layout is built the table is what a kanban view shows,
    /// which is what the read-only first cut did for all four layouts.
    pub(super) fn render_kanban(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.render_table(theme, cx)
    }
}
