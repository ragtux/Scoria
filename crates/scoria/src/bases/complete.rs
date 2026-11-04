//! The formula editor's completion list: what the editor offers for the word
//! being typed at the end of a statement.
//!
//! **Observed** (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`,
//! `s16` `formulaSuggestions`): typing `d` after `rating * 2 + (` opened a
//! `cm-tooltip-autocomplete suggestion-container` under the editor listing
//! `date()`, `density`, `done`, `due`, `duration()` -- the globals and the
//! vault's frontmatter names that start with what is typed, in one list
//! ordered by name, each a `suggestion-item mod-complex` with a type icon:
//! `square-function` for a function, and for a property the icon of its
//! type (`binary` a number, `check-square` a checkbox, `calendar` a date).
//! The globals are the sixteen
//! [[Specification#The function table]] lists, which says the completion
//! list is the whole set; after `file.` the list is the File receiver's
//! functions and properties from the same table, and after `this.` it is
//! `file`, `note`, `formula`, `this` and the frontmatter names.
//!
//! **Readings**: that the match is a case-insensitive prefix (the run typed
//! one lowercase letter); that a bare word also offers the four idents
//! `file`, `formula`, `note` and `this` (the `this.` list holds them, the
//! `d` list could not show them); that `formula.` offers the base's formulas
//! and `note.` the frontmatter names; that a summary's editor adds its own
//! ident `values`. Other receivers (`status.`, a call's result) would need
//! the expression's type and offer nothing here. Obsidian's per-row
//! description panel (`cm-completionInfo`) is not drawn.

use scoria_bases::rule::PropertyKind;
use std::ops::Range;

/// The sixteen global functions.
pub const GLOBALS: &[&str] = &[
    "date",
    "duration",
    "escapeHTML",
    "file",
    "html",
    "icon",
    "if",
    "image",
    "link",
    "list",
    "max",
    "min",
    "now",
    "number",
    "random",
    "today",
];

/// The idents that are not frontmatter names.
const IDENTS: &[&str] = &["file", "formula", "note", "this"];

/// The File receiver's functions.
const FILE_FUNCTIONS: &[&str] = &["asLink", "hasLink", "hasProperty", "hasTag", "inFolder"];

/// The File receiver's properties, with the icon of each one's type.
const FILE_PROPERTIES: &[(&str, &str)] = &[
    ("backlinks", "list"),
    ("basename", "text"),
    ("ctime", "clock"),
    ("embeds", "list"),
    ("ext", "text"),
    ("file", "file"),
    ("folder", "text"),
    ("fullname", "text"),
    ("links", "list"),
    ("mtime", "clock"),
    ("name", "text"),
    ("path", "text"),
    ("properties", "list"),
    ("size", "binary"),
    ("tags", "tags"),
];

/// One row of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// What the row shows, `date()` for a function.
    pub label: String,
    /// What accepting it puts in place of the typed word.
    pub insert: String,
    /// The `lucide-*` name of its icon.
    pub icon: &'static str,
}

/// What the editor knows about the base it is in.
pub struct Scope<'a> {
    /// The vault's frontmatter names and their kinds.
    pub properties: &'a [(String, PropertyKind)],
    /// The base's own formula names.
    pub formulas: &'a [String],
    /// A summary formula: `values` is an ident.
    pub summary: bool,
}

/// The icon a property of `kind` wears in the list, the value type tags of
/// [[Specification#Observed Bases expression language]].
pub fn kind_icon(kind: PropertyKind) -> &'static str {
    match kind {
        PropertyKind::Number => "binary",
        PropertyKind::Checkbox => "check-square",
        PropertyKind::Date => "calendar",
        PropertyKind::DateTime => "clock",
        PropertyKind::List => "list",
        PropertyKind::Tags => "tags",
        PropertyKind::File => "file",
        PropertyKind::Formula => "square-function",
        PropertyKind::Text | PropertyKind::Unknown => "text",
    }
}

/// The word being typed at the end of `text` (its byte range) and what the
/// list offers for it, ordered by label. An empty list means nothing to show.
pub fn completions(text: &str, scope: &Scope<'_>) -> (Range<usize>, Vec<Completion>) {
    let word_start = text
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
        .last()
        .map_or(text.len(), |(i, _)| i);
    let word = &text[word_start..];
    let before = &text[..word_start];
    // A digit-led word is a number literal, never a name.
    if word.starts_with(|c: char| c.is_ascii_digit()) {
        return (word_start..text.len(), Vec::new());
    }
    let receiver = before.strip_suffix('.').map(|b| {
        let start = b
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
            .last()
            .map_or(b.len(), |(i, _)| i);
        &b[start..]
    });
    let function = |name: &str| Completion {
        label: format!("{name}()"),
        insert: format!("{name}()"),
        icon: "square-function",
    };
    let named = |name: &str, icon: &'static str| Completion {
        label: name.to_string(),
        insert: name.to_string(),
        icon,
    };
    let frontmatter = || {
        scope
            .properties
            .iter()
            .map(|(name, kind)| named(name, kind_icon(*kind)))
    };
    let mut all: Vec<Completion> = match receiver {
        None => {
            // A bare word needs at least one letter typed.
            if word.is_empty() {
                return (word_start..text.len(), Vec::new());
            }
            let mut out: Vec<Completion> = GLOBALS.iter().map(|g| function(g)).collect();
            out.extend(IDENTS.iter().map(|i| named(i, "list")));
            if scope.summary {
                out.push(named("values", "list"));
            }
            out.extend(frontmatter());
            out
        }
        Some("file") => FILE_FUNCTIONS
            .iter()
            .map(|f| function(f))
            .chain(FILE_PROPERTIES.iter().map(|(p, i)| named(p, i)))
            .collect(),
        Some("this") => IDENTS
            .iter()
            .map(|i| named(i, "list"))
            .chain(frontmatter())
            .collect(),
        Some("note") => frontmatter().collect(),
        Some("formula") => scope
            .formulas
            .iter()
            .map(|f| named(f, "square-function"))
            .collect(),
        Some(_) => Vec::new(),
    };
    let lower = word.to_lowercase();
    all.retain(|c| {
        let name = c.label.trim_end_matches("()").to_lowercase();
        name.starts_with(&lower) && name != lower
    });
    all.sort_by_key(|c| c.label.to_lowercase());
    all.dedup_by(|a, b| a.label == b.label);
    (word_start..text.len(), all)
}

/// `text` with the word at `range` replaced by the accepted completion.
pub fn accept(text: &str, range: Range<usize>, completion: &Completion) -> String {
    format!(
        "{}{}{}",
        &text[..range.start],
        completion.insert,
        &text[range.end..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope<'a>(props: &'a [(String, PropertyKind)], formulas: &'a [String]) -> Scope<'a> {
        Scope {
            properties: props,
            formulas,
            summary: false,
        }
    }

    fn props() -> Vec<(String, PropertyKind)> {
        vec![
            ("density".into(), PropertyKind::Number),
            ("done".into(), PropertyKind::Checkbox),
            ("due".into(), PropertyKind::Date),
            ("status".into(), PropertyKind::Text),
        ]
    }

    #[test]
    fn a_letter_lists_the_globals_and_properties_it_starts_as_observed() {
        let p = props();
        let (range, list) = completions("rating * 2 + (d", &scope(&p, &[]));
        assert_eq!(range, 14..15);
        let labels: Vec<&str> = list.iter().map(|c| c.label.as_str()).collect();
        // The run's own list for `d`, in its order.
        assert_eq!(labels, ["date()", "density", "done", "due", "duration()"]);
        let icons: Vec<&str> = list.iter().map(|c| c.icon).collect();
        assert_eq!(
            icons,
            [
                "square-function",
                "binary",
                "check-square",
                "calendar",
                "square-function"
            ]
        );
    }

    #[test]
    fn nothing_shows_for_no_word_a_number_or_a_word_typed_in_full() {
        let p = props();
        assert!(completions("rating * ", &scope(&p, &[])).1.is_empty());
        assert!(completions("rating * 2", &scope(&p, &[])).1.is_empty());
        assert!(completions("status", &scope(&p, &[])).1.is_empty());
    }

    #[test]
    fn a_receiver_narrows_the_list() {
        let p = props();
        let f = vec!["score".to_string()];
        let s = scope(&p, &f);
        let labels = |t: &str| -> Vec<String> {
            completions(t, &s).1.into_iter().map(|c| c.label).collect()
        };
        assert_eq!(
            labels("file.has"),
            ["hasLink()", "hasProperty()", "hasTag()"]
        );
        assert_eq!(labels("file."), {
            let mut all: Vec<String> = FILE_FUNCTIONS
                .iter()
                .map(|f| format!("{f}()"))
                .chain(FILE_PROPERTIES.iter().map(|(p, _)| p.to_string()))
                .collect();
            all.sort_by_key(|l| l.to_lowercase());
            all
        });
        assert_eq!(labels("formula."), ["score"]);
        assert_eq!(labels("note.d"), ["density", "done", "due"]);
        assert_eq!(labels("this.f"), ["file", "formula"]);
        // A receiver whose type is not known offers nothing.
        assert!(labels("status.").is_empty());
    }

    #[test]
    fn a_summary_editor_knows_values() {
        let p = props();
        let s = Scope {
            properties: &p,
            formulas: &[],
            summary: true,
        };
        let (range, list) = completions("val", &s);
        assert_eq!(list[0].label, "values");
        assert_eq!(accept("val", range, &list[0]), "values");
    }

    #[test]
    fn accepting_a_function_writes_its_parentheses() {
        let p = props();
        let (range, list) = completions("1 + no", &scope(&p, &[]));
        let now = list.iter().find(|c| c.label == "now()").unwrap();
        assert_eq!(accept("1 + no", range, now), "1 + now()");
    }
}
