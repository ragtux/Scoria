//! The Footnotes view's rows: a note's footnotes in the order Obsidian's
//! `footnotes` view lists them, read off the one Markdown parser's tree.
//!
//! Observed on Obsidian 1.15.0 (`tools/obsidian-probe/results/2026-11-09-panes-workspaces-headings.json`,
//! `SCEN=fn`): the rows follow the order of first reference in the text,
//! not the order of the definitions; the number shown is the running one,
//! never the label; an inline footnote (`^[...]`) is a row like the others;
//! a definition nothing references comes last, marked as having no
//! occurrences; and a reference with no definition has no row at all.

use scoria_markdown::{inline_runs, parse, walk_blocks, walk_inlines, BlockKind, InlineKind};
use scoria_markdown::{LineIndex, Span};

/// One row of the Footnotes view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteRow {
    /// The number the row shows: 1 to n, in the view's order.
    pub number: usize,
    /// The definition's label (`named` for `[^named]`); `None` for an
    /// inline footnote, which has none.
    pub label: Option<String>,
    /// Whether anything in the text references it. A definition nothing
    /// references is listed last, its number drawn faint
    /// (`mod-no-occurrences`).
    pub referenced: bool,
    /// The footnote's own Markdown: a definition's text with its
    /// `[^label]:` marker taken off and its continuation lines' indent
    /// (up to four spaces, or one tab) removed; an inline footnote's text
    /// between `^[` and `]`.
    pub body: String,
    /// Where the footnote is written: the start of a definition's first
    /// line, or the `^` of an inline footnote.
    pub offset: usize,
    /// The zero-based line `offset` is on.
    pub line: usize,
}

/// The rows of `src`'s Footnotes view, in the order the view lists them.
///
/// Labels match exactly and the first definition of a label wins, as they
/// do in the reading view's own Footnotes section. References written
/// inside a definition are not counted: the probe's note had none, and the
/// reading view numbers only the text's own.
pub fn footnote_rows(src: &str) -> Vec<FootnoteRow> {
    let doc = parse(src);
    let lines = LineIndex::new(src);
    // Every definition, first one per label, in the order written.
    let mut defs: Vec<(&str, Span, String)> = Vec::new();
    walk_blocks(&doc.blocks, &mut |block| {
        if let BlockKind::FootnoteDefinition { label, children } = &block.kind {
            if defs.iter().any(|(l, _, _)| l == label) {
                return;
            }
            let body = match (children.first(), children.last()) {
                (Some(first), Some(last)) => dedent(&src[first.span.start..last.span.end]),
                _ => String::new(),
            };
            defs.push((label, block.span, body));
        }
    });
    let inside_definition = |at: usize| {
        defs.iter()
            .any(|(_, span, _)| span.start <= at && at < span.end)
    };
    // The text's references in source order: a labelled one names a
    // definition, an inline one is its own.
    enum Ref<'a> {
        Label(&'a str),
        Inline(Span),
    }
    let mut refs: Vec<Ref<'_>> = Vec::new();
    for run in inline_runs(&doc.blocks) {
        if run.first().is_some_and(|i| inside_definition(i.span.start)) {
            continue;
        }
        walk_inlines(run, &mut |inline| match &inline.kind {
            InlineKind::FootnoteReference(label) => {
                if let Some((l, _, _)) = defs.iter().find(|(l, _, _)| *l == label.as_ref()) {
                    refs.push(Ref::Label(l));
                }
            }
            InlineKind::InlineFootnote(_) => refs.push(Ref::Inline(inline.span)),
            _ => {}
        });
    }
    let mut rows: Vec<FootnoteRow> = Vec::new();
    let mut listed: Vec<&str> = Vec::new();
    let row = |rows: &Vec<FootnoteRow>, label, referenced, body, offset: usize| FootnoteRow {
        number: rows.len() + 1,
        label,
        referenced,
        body,
        offset,
        line: lines.pos(offset).line as usize,
    };
    for r in refs {
        match r {
            Ref::Label(label) => {
                if listed.contains(&label) {
                    continue;
                }
                listed.push(label);
                let (_, span, body) = defs.iter().find(|(l, _, _)| *l == label).unwrap();
                let next = row(
                    &rows,
                    Some(label.to_string()),
                    true,
                    body.clone(),
                    span.start,
                );
                rows.push(next);
            }
            Ref::Inline(span) => {
                // `^[` … `]`: the text between, as written.
                let body = src
                    .get(span.start + 2..span.end.saturating_sub(1))
                    .unwrap_or_default()
                    .to_string();
                let next = row(&rows, None, true, body, span.start);
                rows.push(next);
            }
        }
    }
    for (label, span, body) in &defs {
        if listed.contains(label) {
            continue;
        }
        let next = row(
            &rows,
            Some(label.to_string()),
            false,
            body.clone(),
            span.start,
        );
        rows.push(next);
    }
    rows
}

/// A definition's text with the indent of its continuation lines taken
/// off: up to four spaces, or one tab, which is what makes a line part of
/// the definition rather than a block of its own.
fn dedent(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            let spaces = line.bytes().take(4).take_while(|b| *b == b' ').count();
            let cut = if spaces == 0 && line.starts_with('\t') {
                1
            } else {
                spaces
            };
            out.push_str(&line[cut..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probe's `Footnotes.md`, byte for byte (`fn` `f05-edit-and-menu`
    /// `fileBytesUnchanged`).
    fn probe_note() -> String {
        let mut s = String::from(
            "# Footnotes\n\nFirst claim.[^1] Second claim.[^named] Inline one.^[An inline note.] \
             Uses one again.[^1] Missing one.[^missing]\n\n",
        );
        for i in 1..=40 {
            s.push_str(&format!("Filler line {i} so the note scrolls.\n\n"));
        }
        s.push_str(
            "[^named]: The named definition,\n    continued on a second line.\n\n\
             [^1]: The first definition.\n\n\
             [^orphan]: A definition nothing references.\n",
        );
        s
    }

    #[test]
    fn rows_follow_first_reference_and_an_orphan_comes_last() {
        let rows = footnote_rows(&probe_note());
        let shown: Vec<(usize, &str, bool)> = rows
            .iter()
            .map(|r| (r.number, r.body.as_str(), r.referenced))
            .collect();
        assert_eq!(
            shown,
            [
                (1, "The first definition.", true),
                (
                    2,
                    "The named definition,\ncontinued on a second line.",
                    true
                ),
                (3, "An inline note.", true),
                (4, "A definition nothing references.", false),
            ]
        );
        let labels: Vec<Option<&str>> = rows.iter().map(|r| r.label.as_deref()).collect();
        assert_eq!(labels, [Some("1"), Some("named"), None, Some("orphan")]);
    }

    /// "Open link" put the caret at column 0 of each definition's first
    /// line: 87 for `[^1]`, 84 for `[^named]`, 89 for `[^orphan]`.
    #[test]
    fn a_definition_row_points_at_its_first_line() {
        let src = probe_note();
        let rows = footnote_rows(&src);
        let lines: Vec<usize> = rows.iter().map(|r| r.line).collect();
        assert_eq!(lines[0], 87);
        assert_eq!(lines[1], 84);
        assert_eq!(lines[3], 89);
        assert!(src[rows[0].offset..].starts_with("[^1]: The first definition."));
        assert!(src[rows[1].offset..].starts_with("[^named]:"));
        // The inline one is written where it is referenced.
        assert_eq!(lines[2], 2);
        assert!(src[rows[2].offset..].starts_with("^[An inline note.]"));
    }

    #[test]
    fn a_reference_without_a_definition_has_no_row() {
        let rows = footnote_rows("Only this.[^missing]\n");
        assert!(rows.is_empty());
        assert!(footnote_rows("# No footnotes\n\nAt all.\n").is_empty());
    }

    #[test]
    fn a_label_is_listed_once_and_the_first_definition_wins() {
        let rows =
            footnote_rows("A[^a] b[^a] c[^b]\n\n[^b]: Bee.\n\n[^a]: First.\n\n[^a]: Second.\n");
        let shown: Vec<(&str, &str)> = rows
            .iter()
            .map(|r| (r.label.as_deref().unwrap(), r.body.as_str()))
            .collect();
        assert_eq!(shown, [("a", "First."), ("b", "Bee.")]);
    }

    #[test]
    fn a_tab_indented_continuation_is_dedented_too() {
        assert_eq!(dedent("one\n\ttwo\n      three"), "one\ntwo\n  three");
    }
}
