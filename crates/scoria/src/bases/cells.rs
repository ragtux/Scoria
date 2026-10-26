//! What one cell of a base draws, shared by all four layouts.
//!
//! A cell's *value* is `scoria_bases::cell`'s business — null and a boolean
//! are empty, a List joins with a bare `,`, a Link is its text — and this
//! module is only the drawing: the text a cell shows, whether it shows as a
//! link, the disabled checkbox a boolean draws as, and the image a cards or
//! kanban row names.
//!
//! Every layout draws the same values, so none of this belongs to the table:
//! a card shows the same cell the table's row does, in a different box.

use std::path::PathBuf;

use crate::reading::EmbedSource;
use crate::theme::Theme;
use gpui::{div, prelude::*, px};
use scoria_bases::{Cell, Column, ColumnSource, Value};

/// Padding inside a cell and a header, left and right. **Unprobed**: the
/// headers of `Layout.base` abut exactly (344 + 92 = 436, 436 + 88 = 524), so
/// the padding is inside the box, but its size was never read. 8 px is the
/// shell's own (`0 8px` on a dock tab header).
pub const CELL_PAD: f32 = 8.;

/// What a cell draws.
///
/// [`Cell::text`] already carries an evaluation refusal as its content, which
/// is what Obsidian renders for one; this prefers the error anyway so a cell
/// whose text was never filled in still says why.
pub fn cell_text(cell: &Cell) -> String {
    match (&cell.error, cell.text.is_empty()) {
        (Some(e), true) => e.clone(),
        _ => cell.text.clone(),
    }
}

/// Whether a cell draws as an internal link.
///
/// The one link the run rendered is the `file.name` column's — a
/// `span.internal-link` inside a `markdown-rendered` cell where every other
/// cell is bare text — and a Link value reaches a cell as its display text, so
/// both are drawn in the link colour. **Unprobed**: whether `file.path` and
/// the other file columns are links too.
pub fn renders_as_link(column: &Column, cell: &Cell) -> bool {
    matches!(cell.value, Value::Link(_) | Value::File(_))
        || matches!(&column.source, ColumnSource::File(f) if f.eq_ignore_ascii_case("name"))
}

/// Obsidian's `<input disabled type="checkbox">`: the box the reading view's
/// property rows draw, dimmed, because a base's own cell is read-only until
/// something edits it.
pub fn checkbox(checked: bool, theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .opacity(0.5)
        .w(px(theme.base_font_size))
        .h(px(theme.base_font_size))
        .border_1()
        .border_color(theme.border)
        .rounded_sm()
        .when(checked, |d| {
            d.bg(theme.accent)
                .text_color(theme.bg)
                .child(crate::assets::icon(
                    "check",
                    theme.base_font_size - 4.,
                    theme.bg,
                ))
        })
}

/// Where a row's image comes from: a file in the vault, or a URL the layout's
/// property named directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRef {
    File(PathBuf),
    Url(String),
}

/// The image a cards or kanban row draws, from the cell the view's `image`
/// property answered.
///
/// Four shapes reach here, and all four are shapes a frontmatter property
/// really takes: a wikilink to an attachment (`cover: "[[rock.png]]"`, which
/// the language answers as a Link), a vault path as a plain string, an
/// external URL, and a list of any of those — of which the first is taken,
/// the way an embed takes one target.
///
/// **Unprobed**: no `image` key was ever rendered in the run, so which of
/// these Obsidian accepts is a reading of the property picker (it offers any
/// property) rather than an observation.
pub fn image_ref(cell: &Cell, source_path: &str, vault: &dyn EmbedSource) -> Option<ImageRef> {
    fn of(value: &Value, source_path: &str, vault: &dyn EmbedSource) -> Option<ImageRef> {
        match value {
            Value::Link(link) => resolve(&link.path, source_path, vault),
            Value::File(file) => resolve(&file.path, source_path, vault),
            Value::Str(text) => resolve(text, source_path, vault),
            Value::List(items) => items.iter().find_map(|v| of(v, source_path, vault)),
            _ => None,
        }
    }
    fn resolve(text: &str, source_path: &str, vault: &dyn EmbedSource) -> Option<ImageRef> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(ImageRef::Url(text.to_string()));
        }
        // A wikilink's own body may have reached the property as text.
        let body = text
            .strip_prefix("[[")
            .and_then(|t| t.strip_suffix("]]"))
            .unwrap_or(text);
        let path = vault.resolve(body, source_path)?;
        Some(ImageRef::File(vault.absolute(&path)))
    }
    of(&cell.value, source_path, vault)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_bases::{CellKind, LinkValue};
    use std::path::Path;

    struct Vault;

    impl EmbedSource for Vault {
        fn resolve(&self, linktext: &str, _source: &str) -> Option<String> {
            (linktext == "rock.png" || linktext == "img/rock.png")
                .then(|| "img/rock.png".to_string())
        }
        fn read(&self, _path: &str) -> Option<String> {
            None
        }
        fn absolute(&self, path: &str) -> PathBuf {
            Path::new("/vault").join(path)
        }
    }

    fn cell(value: Value) -> Cell {
        Cell {
            value,
            error: None,
            kind: CellKind::Text,
            text: String::new(),
        }
    }

    #[test]
    fn an_images_text_is_resolved_against_the_vault() {
        let got = image_ref(&cell(Value::Str("rock.png".into())), "Note.md", &Vault);
        assert_eq!(
            got,
            Some(ImageRef::File(PathBuf::from("/vault/img/rock.png")))
        );
    }

    #[test]
    fn a_wikilink_reaches_it_as_a_link_or_as_its_own_body() {
        let link = Value::Link(LinkValue {
            path: "rock.png".into(),
            display: None,
        });
        assert!(matches!(
            image_ref(&cell(link), "Note.md", &Vault),
            Some(ImageRef::File(_))
        ));
        assert!(matches!(
            image_ref(&cell(Value::Str("[[rock.png]]".into())), "Note.md", &Vault),
            Some(ImageRef::File(_))
        ));
    }

    #[test]
    fn a_url_is_taken_as_written() {
        let got = image_ref(
            &cell(Value::Str("https://example.com/a.png".into())),
            "Note.md",
            &Vault,
        );
        assert_eq!(
            got,
            Some(ImageRef::Url("https://example.com/a.png".to_string()))
        );
    }

    #[test]
    fn a_list_takes_its_first_resolvable_target() {
        let list = Value::List(vec![
            Value::Str("nosuch.png".into()),
            Value::Str("rock.png".into()),
        ]);
        assert_eq!(
            image_ref(&cell(list), "Note.md", &Vault),
            Some(ImageRef::File(PathBuf::from("/vault/img/rock.png")))
        );
    }

    #[test]
    fn nothing_that_names_no_file_draws_an_image() {
        assert_eq!(image_ref(&cell(Value::Null), "Note.md", &Vault), None);
        assert_eq!(image_ref(&cell(Value::Bool(true)), "Note.md", &Vault), None);
        assert_eq!(
            image_ref(&cell(Value::Str("   ".into())), "Note.md", &Vault),
            None
        );
    }

    #[test]
    fn an_error_is_what_a_cell_with_no_text_draws() {
        let mut c = cell(Value::Null);
        c.error = Some("Cannot find function \"x\"".to_string());
        assert_eq!(cell_text(&c), "Cannot find function \"x\"");
        c.text = "something".to_string();
        assert_eq!(cell_text(&c), "something");
    }
}
