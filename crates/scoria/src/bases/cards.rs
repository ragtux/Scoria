//! The `cards` layout: a grid of cards, one per row.
//!
//! Its keys are observed (`plugin.getRegistrations()` and the real Configure
//! view page; see [`scoria_bases::layout`] and
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//! `cardSize` (50 to 800 step 10, default 200), `image` (a property id),
//! `imageFit` (`""` Cover or `contain`) and `imageAspectRatio` (0.25 to 2.5
//! step 0.05, default 1). The rows, the columns and the grouping are the
//! table's — every layout draws the same resolved
//! [`Table`](scoria_bases::Table) — so a card shows the same cells a row
//! would, in a box of its own.
//!
//! **Unprobed**: no cards view was ever rendered in the run, so the card's
//! own box — its padding, where the image sits, which cell is its title — is
//! not observed. The keys and their ranges are.

use crate::bases::view::BasesView;
use crate::theme::Theme;
use gpui::{AnyElement, Context, ScrollHandle};
use serde_json::{json, Value as Json};

/// What the cards layout keeps between renders.
#[derive(Default)]
pub struct CardsState {
    /// The grid scrolls as one box; cards are laid out by wrapping rather
    /// than virtualised, which is why this is a scroll handle and not a
    /// [`gpui::ListState`].
    pub scroll: ScrollHandle,
    /// How many cards the grid holds, which is the table's row count once
    /// the limit has been applied.
    pub cards: usize,
}

impl CardsState {
    pub fn state_json(&self) -> Json {
        json!({ "cards": self.cards })
    }
}

impl BasesView {
    pub(super) fn rebuild_cards(&mut self) {
        self.cards.cards = self.table().map_or(0, |t| t.rows.len());
        // The fallback below draws the table, which needs its own body
        // rebuilt; a cards layout that draws cards will not need this.
        self.rebuild_table();
    }

    /// Draws the grid.
    ///
    /// Until this layout is built the table is what a cards view shows,
    /// which is what the read-only first cut did for all four layouts.
    pub(super) fn render_cards(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.render_table(theme, cx)
    }
}
