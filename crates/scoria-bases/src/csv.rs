//! A resolved view as CSV, for *Export as CSV*.
//!
//! The shape is RFC 4180: a header row of the columns' display names, one
//! row per [`Table::rows`] carrying each cell's rendered text, `\n` line ends
//! and a trailing newline. A field holding a comma, a double quote, a CR or
//! an LF is double-quoted with every `"` doubled; every other field is bare.
//! Groups are flattened in view order, which is what `rows` already is, and a
//! hidden group is therefore not exported.
//!
//! **The dialect is unobserved.** Obsidian's file dialog was never driven in
//! any run, so nothing here is checked against bytes it wrote: the line end,
//! the quoting rule, whether a group header gets a row of its own, and what a
//! checkbox cell exports as (its text is empty, see [`crate::cell`]) are all
//! the plain RFC reading rather than a measurement.

use crate::table::Table;

/// One field, quoted only when RFC 4180 needs it.
fn field(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        let mut out = String::with_capacity(text.len() + 2);
        out.push('"');
        for c in text.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        text.to_string()
    }
}

fn line(fields: impl Iterator<Item = String>, out: &mut String) {
    let mut first = true;
    for f in fields {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&f);
    }
    out.push('\n');
}

/// The table as CSV: a header of display names, then each row's cell texts.
/// A table with no columns exports an empty header line and one empty line
/// per row.
pub fn table_csv(table: &Table) -> String {
    let mut out = String::new();
    line(table.columns.iter().map(|c| field(&c.display)), &mut out);
    for row in &table.rows {
        line(row.cells.iter().map(|c| field(&c.text)), &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{Cell, CellKind};
    use crate::table::{Column, ColumnSource, Group, Row};
    use crate::value::{FileData, Value};
    use std::sync::Arc;

    fn cell(text: &str) -> Cell {
        Cell {
            value: Value::str(text),
            error: None,
            kind: CellKind::Text,
            text: text.to_string(),
        }
    }

    fn column(display: &str) -> Column {
        Column {
            id: display.to_string(),
            display: display.to_string(),
            source: ColumnSource::Property(display.to_string()),
        }
    }

    fn row(texts: &[&str]) -> Row {
        Row {
            file: Arc::new(FileData::default()),
            cells: texts.iter().map(|t| cell(t)).collect(),
            image: None,
        }
    }

    fn table(columns: &[&str], rows: Vec<Row>) -> Table {
        Table {
            columns: columns.iter().map(|c| column(c)).collect(),
            groups: vec![Group {
                key: None,
                rows: rows.clone(),
            }],
            rows,
            all_groups: Vec::new(),
            limited: false,
            unknown_kind: None,
        }
    }

    #[test]
    fn a_header_of_display_names_then_one_line_per_row() {
        let t = table(
            &["file name", "status"],
            vec![row(&["Scoria", "todo"]), row(&["Pumice", ""])],
        );
        assert_eq!(table_csv(&t), "file name,status\nScoria,todo\nPumice,\n");
    }

    #[test]
    fn a_field_is_quoted_only_when_rfc_4180_needs_it() {
        assert_eq!(field("plain text"), "plain text");
        assert_eq!(field("a,b"), "\"a,b\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(field("two\nlines"), "\"two\nlines\"");
        assert_eq!(field("cr\rhere"), "\"cr\rhere\"");
        assert_eq!(field(""), "");
        let t = table(&["a,b", "c"], vec![row(&["1\n2", "\"q\""])]);
        assert_eq!(table_csv(&t), "\"a,b\",c\n\"1\n2\",\"\"\"q\"\"\"\n");
    }

    #[test]
    fn groups_are_flattened_in_view_order() {
        // `rows` is already the groups flattened; the export follows it and
        // never looks at `groups`, so a hidden group exports nothing.
        let mut t = table(&["n"], vec![row(&["todo-1"]), row(&["done-1"])]);
        t.groups = vec![
            Group {
                key: Some("todo".to_string()),
                rows: vec![row(&["todo-1"])],
            },
            Group {
                key: Some("done".to_string()),
                rows: vec![row(&["done-1"])],
            },
        ];
        assert_eq!(table_csv(&t), "n\ntodo-1\ndone-1\n");
    }

    #[test]
    fn an_empty_table_is_one_empty_line() {
        assert_eq!(table_csv(&table(&[], vec![])), "\n");
        assert_eq!(table_csv(&table(&[], vec![row(&[])])), "\n\n");
    }
}
