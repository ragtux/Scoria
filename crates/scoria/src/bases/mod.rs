//! The Bases view: draws a `.base` file.
//!
//! [`view`] is the shell — the parsed file, the showing view, the table it
//! resolved to, the error states and the writes — and each layout draws
//! itself: [`table`], [`cards`], [`list`] and [`kanban`], the four layouts
//! Obsidian registers. [`toolbar`] is the chrome above them, [`edit`] is a
//! cell typed into, [`commands`] is what the `bases:*` commands do to the
//! showing view, and [`embed`] is the registry of the live views behind
//! every `![[x.base]]` inside a note.
//!
//! [`summary`] is the column summaries: the footer and group rows of the
//! table, and the Summarize page that sets them.
//!
//! [`writes`] is what a control writes into a view — `sort`, `groupBy`,
//! `order` and `limit` — in one place, because the toolbar's panels and the
//! column header's own menu both write them and must write the same bytes.
//!
//! [`rows`] is the bridge: it turns the vault's own index and cache into the
//! `FileData` values `scoria-bases` evaluates against, so nothing below this
//! module knows what a vault is. [`cells`] is what one cell draws, shared by
//! every layout. The toolbar's panels are [`toolbar_views`],
//! [`toolbar_properties`], [`toolbar_filter`] and [`toolbar_lists`] (sort,
//! group, results), with [`toolbar_picks`] routing their comboboxes.
//!
//! **Writing.** Obsidian writes a base on a toolbar change and canonicalizes
//! it on close, and never writes one that failed to parse; `scoria_formats::base`
//! serializes those bytes and [`view::BasesView::file_contents`] is what the
//! workspace's save path asks for.

pub mod cards;
pub mod cells;
pub mod commands;
pub mod edit;
pub mod embed;
pub mod kanban;
pub mod list;
pub mod rows;
pub mod summary;
pub mod table;
pub mod toolbar;
pub mod toolbar_filter;
pub mod toolbar_lists;
pub mod toolbar_picks;
pub mod toolbar_properties;
pub mod toolbar_views;
pub mod view;
pub mod writes;

pub use embed::EmbeddedBase;
pub use view::{BasesEvent, BasesView};
