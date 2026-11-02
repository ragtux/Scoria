//! The `bases:*` commands, as far as they are the view's own work.
//!
//! Observed (`tools/obsidian-probe/results/2026-10-25-bases.json`; the table
//! is in
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]):
//!
//! | Command | Palette name | Effect |
//! | --- | --- | --- |
//! | `bases:new-file` | Bases: Create new base | a **zero-byte** `Untitled.base`, opened |
//! | `bases:add-view` | Bases: Add view | appends a view named `View` and opens its Configure view page |
//! | `bases:insert` | Bases: Insert new base | another zero-byte base, and `![[Untitled 1.base]]` on a line of its own at the cursor |
//! | `bases:copy-table` | Bases: Copy table to clipboard | the visible columns as TSV, and a "Copied to your clipboard" notice |
//! | `bases:change-view` | Bases: Switch view... | not probed |
//! | `bases:add-item` | Bases: Add item | not probed |
//!
//! `bases:new-file` is the workspace's own (it creates a file and opens it);
//! the rest need the showing view, which is what lives here. Each of these is
//! half a command: the view answers what only it knows — the next view's
//! name, the TSV of the showing table, where a new item would go — and the
//! workspace does the file, the clipboard and the Notice.
//!
//! A command that needs a base does **nothing at all** when none is showing:
//! the file that failed to parse has no base to append a view to, and
//! Obsidian's own check callbacks keep such a command off the palette rather
//! than letting it run and fail.

use crate::bases::cells::table_tsv;
use crate::bases::toolbar::Section;
use crate::bases::view::{BasesEvent, BasesView};
use gpui::Context;
use scoria_bases::rule::{parse_rule, Operator, RuleValue};
use scoria_formats::base::{Base, Filter, View, DEFAULT_VIEW_NAME, DEFAULT_VIEW_TYPE};

/// Where `bases:add-item` would put the note it creates, which is the base's
/// own `newItemFolder` and `newItemTemplate` — two keys the published schema
/// never mentions and both of which round-tripped from the probe's fixture.
///
/// Both are optional, and an absent folder is the workspace's to fall back
/// on: the view knows nothing about the vault's default location for a new
/// note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    pub folder: Option<String>,
    pub template: Option<String>,
    /// The frontmatter the note starts with, as text — `---` lines included
    /// — or empty when the view implies none.
    ///
    /// Observed (2026-11-01, the toolbar's *New*): the note's frontmatter is
    /// prefilled from the filters — `tags: [rock]` for `file.hasTag("rock")`,
    /// `rating: 1` for `rating >= 1`, `status: done` for an equality — and
    /// carries an empty key for every other note property the view shows.
    pub frontmatter: String,
}

impl BasesView {
    /// `bases:add-view`: appends a view to the base and shows it.
    ///
    /// Observed (2026-11-01): every added view is named `View` — the fifth
    /// of a base included — and Obsidian opens its Configure view page,
    /// which is the toolbar's views panel, so the command shows the new view
    /// and then opens that page on it.
    ///
    /// Answers the new view's index, or `None` when there is no base to
    /// append to: a file that did not parse is never written, so there is
    /// nothing a view could be added to either.
    pub fn add_view(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        self.base()?;
        self.edit_base(append_view, cx);
        let ix = self.base()?.views.len().checked_sub(1)?;
        self.show_view(ix, cx);
        self.configure_view(ix, cx);
        Some(ix)
    }

    /// The names of the views `bases:change-view` chooses between, in file
    /// order.
    ///
    /// A base whose file names no views has one default view all the same —
    /// the parsed query of a zero-byte file is one `table` named `View` — so
    /// that is what this answers for it. A file that did not parse has no
    /// views at all.
    pub fn view_names(&self) -> Vec<String> {
        let Some(base) = self.base() else {
            return Vec::new();
        };
        if base.views.is_empty() {
            return vec![DEFAULT_VIEW_NAME.to_string()];
        }
        base.views.iter().map(|v| v.name.clone()).collect()
    }

    /// `bases:change-view`: shows the view the chooser picked.
    ///
    /// **Unprobed**: the command was never driven, so neither the chooser nor
    /// what it looks like is observed; this is [`BasesView::show_view`], the
    /// same switch the views panel makes.
    pub fn switch_view(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.show_view(ix, cx);
    }

    /// `bases:copy-table`: the showing view's visible columns as TSV, or
    /// `None` when no base is showing.
    ///
    /// Observed: one header row of **display names**, tab separators, no
    /// trailing newline (`status\tPrice per unit\ntodo\t0.02\ntodo\t0.06`),
    /// and the rows of a grouped view in group order with no group headings
    /// — which is [`Table::rows`] as it stands. The results panel's own
    /// *Copy to clipboard* row produces the same bytes, so it shares this.
    ///
    /// Putting it on the clipboard and announcing it is the workspace's: the
    /// notice depends on where the keyboard is, which the view cannot see.
    pub fn table_tsv(&self) -> Option<String> {
        self.table().map(table_tsv)
    }

    /// `bases:add-item`: the base's own `newItemFolder` and
    /// `newItemTemplate`, which is what a new item would be created from.
    ///
    /// **Unprobed**: the command was never driven. These two keys are what
    /// the file carries for it, so they are the reading; creating the note is
    /// the workspace's half, and nothing here writes the `.base`.
    pub fn new_item(&self) -> Option<NewItem> {
        let base = self.base()?;
        Some(NewItem {
            folder: base.new_item_folder.clone(),
            template: base.new_item_template.clone(),
            frontmatter: self.new_item_frontmatter(base),
        })
    }

    /// The toolbar's *New*: the workspace creates the note.
    pub(super) fn add_item(&mut self, cx: &mut Context<Self>) {
        cx.emit(BasesEvent::AddItem);
    }

    /// The frontmatter a new item starts with; see [`NewItem::frontmatter`].
    fn new_item_frontmatter(&self, base: &Base) -> String {
        let view = self.view().cloned().unwrap_or_default();
        let mut lines: Vec<(String, String)> = Vec::new();
        let mut tags: Vec<String> = Vec::new();
        let kind_of = |id: &str| self.kind_of(base, &view, id);
        for section in [Section::AllViews, Section::ThisView] {
            let filters = match section {
                Section::AllViews => base.filters.as_ref(),
                Section::ThisView => view.filters.as_ref(),
            };
            let statements: Vec<&str> = match filters {
                None => Vec::new(),
                Some(Filter::Statement(s)) => vec![s.as_str()],
                Some(Filter::And(items)) => items
                    .iter()
                    .filter_map(|f| match f {
                        Filter::Statement(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect(),
                Some(_) => Vec::new(),
            };
            for statement in statements {
                let Some(rule) = parse_rule(statement, &kind_of) else {
                    continue;
                };
                match (rule.operator, &rule.value) {
                    (Operator::HasTag, RuleValue::List(items)) => {
                        tags.extend(items.iter().cloned())
                    }
                    (Operator::HasTag, RuleValue::Text(t)) if !t.is_empty() => tags.push(t.clone()),
                    (
                        Operator::Is
                        | Operator::Eq
                        | Operator::Ge
                        | Operator::Le
                        | Operator::CheckboxIs,
                        value,
                    ) if rule.property != "file" => {
                        let key = rule.property.trim_start_matches("note.").to_string();
                        let text = match value {
                            RuleValue::Text(t) => yaml_scalar(t),
                            RuleValue::Number(n) => n.clone(),
                            RuleValue::Bool(b) => b.to_string(),
                            RuleValue::List(items) => items.join(", "),
                            RuleValue::None => continue,
                        };
                        if !lines.iter().any(|(k, _)| k == &key) {
                            lines.push((key, text));
                        }
                    }
                    _ => {}
                }
            }
        }
        for id in view.order.iter().flatten() {
            let qualified = scoria_bases::layout::qualify(id);
            let Some(name) = qualified.strip_prefix("note.") else {
                continue;
            };
            if name == "tags" || lines.iter().any(|(k, _)| k == name) {
                continue;
            }
            lines.push((name.to_string(), String::new()));
        }
        if lines.is_empty() && tags.is_empty() {
            return String::new();
        }
        let mut out = String::from("---\n");
        if !tags.is_empty() {
            out.push_str("tags:\n");
            for t in tags {
                out.push_str(&format!("  - {t}\n"));
            }
        }
        for (k, v) in lines {
            if v.is_empty() {
                out.push_str(&format!("{k}:\n"));
            } else {
                out.push_str(&format!("{k}: {v}\n"));
            }
        }
        out.push_str("---\n");
        out
    }
}

/// A frontmatter scalar as Obsidian would write a plain string: bare when
/// it reads back as that string, quoted otherwise.
fn yaml_scalar(text: &str) -> String {
    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(serde_yaml::Value::String(s)) if s == text && !text.contains(':') => text.to_string(),
        _ => format!("{text:?}"),
    }
}

/// What `bases:add-view` does to the base: appends a [`DEFAULT_VIEW_TYPE`]
/// view named [`DEFAULT_VIEW_NAME`].
///
/// **Observed** (2026-11-01, `s15-fix-views`): the fifth view of a base was
/// written as `- type: table` / `name: View`, the same name as the first —
/// Obsidian neither numbers nor dedupes.
///
/// A base whose file names no views is given its default view first, the way
/// [`BasesView::edit_view`] materializes one, because that view exists in
/// Obsidian's parsed query already and the write that follows has to carry
/// it.
pub(super) fn append_view(base: &mut Base) {
    if base.views.is_empty() {
        base.views.push(View {
            kind: DEFAULT_VIEW_TYPE.to_string(),
            name: DEFAULT_VIEW_NAME.to_string(),
            ..View::default()
        });
    }
    base.views.push(View {
        kind: DEFAULT_VIEW_TYPE.to_string(),
        name: DEFAULT_VIEW_NAME.to_string(),
        ..View::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_bases::{Cell, CellKind, Column, ColumnSource, FileData, Group, Row, Table, Value};
    use std::sync::Arc;

    fn column(id: &str, display: &str) -> Column {
        Column {
            id: id.to_string(),
            display: display.to_string(),
            source: ColumnSource::Property(id.to_string()),
        }
    }

    fn cell(text: &str) -> Cell {
        Cell {
            value: Value::Str(text.to_string()),
            error: None,
            kind: CellKind::Text,
            text: text.to_string(),
        }
    }

    fn row(path: &str, cells: &[&str]) -> Row {
        Row {
            file: Arc::new(FileData {
                path: path.to_string(),
                name: path.to_string(),
                basename: path.trim_end_matches(".md").to_string(),
                ..FileData::default()
            }),
            cells: cells.iter().map(|c| cell(c)).collect(),
            image: None,
        }
    }

    /// The bytes the probe read off the clipboard, for a two-column table of
    /// two rows.
    #[test]
    fn the_tsv_is_the_display_names_and_a_line_a_row_with_no_trailing_newline() {
        let rows = vec![
            row("a.md", &["todo", "0.02"]),
            row("b.md", &["todo", "0.06"]),
        ];
        let table = Table {
            columns: vec![column("status", "status"), column("ppu", "Price per unit")],
            rows: rows.clone(),
            groups: vec![Group { key: None, rows }],
            all_groups: Vec::new(),
            limited: false,
            unknown_kind: None,
        };
        assert_eq!(
            table_tsv(&table),
            "status\tPrice per unit\ntodo\t0.02\ntodo\t0.06"
        );
    }

    /// A view with no rows is still a header row, and still no newline after
    /// it.
    #[test]
    fn a_table_with_no_rows_is_its_header_alone() {
        let table = Table {
            columns: vec![column("status", "status")],
            rows: Vec::new(),
            groups: Vec::new(),
            all_groups: Vec::new(),
            limited: false,
            unknown_kind: None,
        };
        assert_eq!(table_tsv(&table), "status");
    }

    #[test]
    fn add_view_appends_a_view_named_view_and_writes_obsidians_bytes() {
        let mut base = Base::from_yaml("views:\n  - type: table\n    name: View\n").unwrap();
        append_view(&mut base);
        assert_eq!(base.views.len(), 2);
        assert_eq!(base.views[1].name, "View");
        assert_eq!(base.views[1].kind, "table");
        assert_eq!(
            base.to_yaml(),
            "views:\n  - type: table\n    name: View\n  - type: table\n    name: View\n"
        );
    }

    /// The default view of a zero-byte base is in Obsidian's parsed query but
    /// not in the file, so appending to one has to write both views.
    #[test]
    fn add_view_on_an_empty_base_writes_the_default_view_too() {
        let mut base = Base::from_yaml("").unwrap();
        append_view(&mut base);
        assert_eq!(
            base.to_yaml(),
            "views:\n  - type: table\n    name: View\n  - type: table\n    name: View\n"
        );
    }

    #[test]
    fn a_frontmatter_scalar_is_bare_when_it_reads_back_as_itself() {
        assert_eq!(yaml_scalar("done"), "done");
        assert_eq!(yaml_scalar("true"), "\"true\"");
        assert_eq!(yaml_scalar("a: b"), "\"a: b\"");
    }
}
