//! What a paste or a drop writes into a note, apart from the editor that
//! receives it: copied rich text (`text/html`) as Markdown ([`html`]), the
//! names pasted and dropped files are saved under ([`attachment`]), and the
//! clipboard bytes gpui does not read, decoded ([`clipboard`]).
//!
//! No gpui: the app reads the clipboard and the drop, saves the files and
//! edits the buffer; everything decided here is a pure function with its
//! tests beside it.

pub mod attachment;
pub mod clipboard;
pub mod html;

pub use attachment::{joined_embeds, saved_name, split_name, Civil, Incoming};
pub use html::to_markdown;
