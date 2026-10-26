//! The Bases toolbar: the view switcher and the five panels its controls
//! open.
//!
//! Observed (`tools/obsidian-probe/results/2026-10-25-bases.json`; the prose
//! is in
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//! the toolbar does **not** open menus of rows. Each control opens a
//! `.menu.bases-toolbar-menu` whose body is a panel —
//!
//! - **sort** and **group**: `.combobox-button` pairs (property, direction)
//!   with a `lucide-trash-2` button;
//! - **filter**: a `<select class="conjunction">` (`and`/`or`/`not`) per
//!   group, then a property combobox, an operator combobox, a value input and
//!   a `filer-expression-mode-toggle` (`lucide-code-xml`) per rule, in two
//!   sections, "All views" and "This view";
//! - **properties**: a checkbox list of every property with "Add *name*
//!   property", "Add formula" and "Hide all";
//! - **results**: a "Limit number of results" number input and the rows
//!   *Show all (n)*, *Copy to clipboard* and *Export CSV...*.
//!
//! The views menu lists one row per view, and the row's own icon opens that
//! view's settings (the Configure view page), whose Layout combobox offers
//! exactly `table`, `cards`, `list` and `kanban`.
//!
//! What the panels write is [`BasesView::edit_view`] and
//! [`BasesView::edit_base`]'s business: *Sort Z → A* **prepends**
//! `{property, direction: DESC}` to the view's `sort`, *Hide column* removes
//! the property from `order`, and the limit input writes `limit` as a bare
//! number.

use crate::bases::view::BasesView;
use crate::theme::Theme;
use gpui::{AnyElement, Context};
use serde_json::{json, Value as Json};

/// Which panel, if any, is open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    None,
    Views,
    Filter,
    Properties,
    Sort,
    Group,
    Results,
}

impl Panel {
    /// The name the control socket reports, which is also the name the
    /// toolbar's own buttons go by.
    pub fn as_str(self) -> &'static str {
        match self {
            Panel::None => "none",
            Panel::Views => "views",
            Panel::Filter => "filter",
            Panel::Properties => "properties",
            Panel::Sort => "sort",
            Panel::Group => "group",
            Panel::Results => "results",
        }
    }
}

/// What the toolbar keeps between renders.
#[derive(Default)]
pub struct ToolbarState {
    pub panel: Panel,
}

impl ToolbarState {
    pub fn state_json(&self) -> Json {
        json!({ "panel": self.panel.as_str() })
    }
}

impl BasesView {
    /// The toolbar above the layout, or nothing while it is unbuilt — the
    /// read-only first cut drew the rows with no chrome at all.
    pub(super) fn render_toolbar(
        &mut self,
        _theme: &Theme,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        None
    }
}
