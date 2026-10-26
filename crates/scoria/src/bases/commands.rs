//! The `bases:*` commands, as far as they are the view's own work.
//!
//! Observed (`tools/obsidian-probe/results/2026-10-25-bases.json`; the table
//! is in
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//!
//! | Command | Palette name | Effect |
//! | --- | --- | --- |
//! | `bases:new-file` | Bases: Create new base | a **zero-byte** `Untitled.base`, opened |
//! | `bases:add-view` | Bases: Add view | appends a view named `View 2` and opens its Configure view page |
//! | `bases:insert` | Bases: Insert new base | another zero-byte base, and `![[Untitled 1.base]]` on a line of its own at the cursor |
//! | `bases:copy-table` | Bases: Copy table to clipboard | the visible columns as TSV, and a "Copied to your clipboard" notice |
//! | `bases:change-view` | Bases: Switch view... | not probed |
//! | `bases:add-item` | Bases: Add item | not probed |
//!
//! `bases:new-file` is the workspace's own (it creates a file and opens it);
//! the rest need the showing view, which is what lives here.
