//! Editing through the table: a cell typed into writes the **note's**
//! frontmatter, not the base.
//!
//! A base's cell is a property of the file the row stands for, so editing one
//! is a frontmatter write against that note —
//! `scoria_formats::properties::{plan, apply}`, the same planner the property
//! editor uses — and the base's own bytes do not change. Only three of the
//! column sources can be edited at all: a frontmatter property can, a
//! `file.*` property is the filesystem's and a `formula.*` is computed.
//!
//! The column header's own context menu is observed — *Hide column*,
//! *Summarize...*, separator, *Sort A → Z*, *Sort Z → A*, (*Clear sort*, once
//! one exists), separator, *Edit property...* — and so is what two of its rows
//! write: *Sort Z → A* **prepends** `{property, direction: DESC}` to the
//! view's `sort` list and *Hide column* removes the property from `order`.
//! A header drag writes `columnSize` as a px map.
//!
//! **Unprobed**: whether a table cell is editable in place at all in 1.14.5,
//! and what the *Summarize...* submenu offers (the row opens something that is
//! not a `.menu`, so its options were never read).

use serde_json::{json, Value as Json};

/// What an in-progress cell edit keeps.
#[derive(Default)]
pub struct EditState {
    /// The row and column being edited, by index into the table's own rows.
    pub cell: Option<(usize, usize)>,
}

impl EditState {
    pub fn state_json(&self) -> Json {
        json!({ "cell": self.cell.map(|(r, c)| vec![r, c]) })
    }
}
