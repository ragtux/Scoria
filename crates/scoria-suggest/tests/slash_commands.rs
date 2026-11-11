//! Slash commands against what Obsidian 1.15.0 did, over CDP with real key
//! events (`tools/obsidian-probe/slash-commands.mjs`, results in
//! `tools/obsidian-probe/results/2026-11-11-slash-commands.json`). See
//! [[Specification#Observed Slash commands, Obsidian 1.15.0]].
//!
//! Three things are held to the probe: where `/` opens the popup, the rows
//! and their order (`data/slash-rows-1.15.0.json` is the probe's own list:
//! the palette's 150 names, the first 100 rows of the empty query, and the
//! rows of ten queries), and the bytes an accepted row leaves -- the
//! `/query` taken out and the command run on what is left, which is
//! `scoria-text`'s buffer here as it is in the editor.

use scoria_suggest::{rank_commands, slash_accept_edit, slash_rows, slash_trigger, CommandEntry};
use scoria_text::{BlockInsert, EditKind, LinePrefix, LinkSyntax, TextBuffer};
use serde_json::Value;

fn data() -> Value {
    serde_json::from_str(include_str!("data/slash-rows-1.15.0.json")).expect("probe rows")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("an array")
        .iter()
        .map(|s| s.as_str().expect("a string").to_string())
        .collect()
}

/// The palette's commands as the probe listed them, handed over in reverse
/// so that nothing depends on the order they arrive in.
fn commands() -> Vec<CommandEntry> {
    let mut names = strings(&data()["palette"]);
    names.reverse();
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| CommandEntry {
            id: format!("id{i}"),
            name,
            hotkeys: Vec::new(),
        })
        .collect()
}

fn names(commands: &[CommandEntry], query: &str) -> Vec<String> {
    rank_commands(commands, &[], &[], query)
        .into_iter()
        .map(|r| commands[r.index].name.clone())
        .collect()
}

#[test]
fn the_empty_query_lists_the_commands_by_name_as_obsidian_did() {
    let commands = commands();
    let observed = strings(&data()["empty"]);
    assert_eq!(observed.len(), 100);
    let ours: Vec<String> = slash_rows(&commands, &[], &[], "")
        .into_iter()
        .map(|r| r.label)
        .collect();
    assert_eq!(ours, observed);
}

#[test]
fn a_query_keeps_the_commands_obsidian_kept() {
    let commands = commands();
    let data = data();
    for (query, rows) in data["queries"].as_object().expect("queries") {
        // `/e` matched more than the hundred rows shown, so which hundred
        // is the score's choice; its order is not reproduced.
        if query == "e" {
            continue;
        }
        let observed = strings(rows);
        let mut ours = names(&commands, query);
        ours.truncate(100);
        let mut a = ours.clone();
        let mut b = observed.clone();
        a.sort();
        b.sort();
        assert_eq!(a, b, "the rows of /{query}");
    }
}

/// The order within a query is Obsidian's score, which Scoria's fuzzy
/// matcher does not reproduce row for row: these are the queries whose
/// order it does reproduce, the first rows of the rest are listed in
/// differences.md.
#[test]
fn a_query_orders_the_rows_obsidian_ordered() {
    let commands = commands();
    let data = data();
    for query in ["head", "HEAD", "sethead", "h1"] {
        let observed = strings(&data["queries"][query]);
        let ours = names(&commands, query);
        assert_eq!(ours, observed, "the order of /{query}");
    }
}

#[test]
fn pinned_and_recent_commands_come_first() {
    // s05: `pinned: ["editor:insert-table", "editor:toggle-bold"]` put
    // Insert table and Toggle bold above Add alias; with the pins gone the
    // palette's recent Toggle highlight did.
    let commands: Vec<CommandEntry> = [
        ("editor:add-alias", "Add alias"),
        ("editor:toggle-bold", "Toggle bold"),
        ("editor:insert-table", "Insert table"),
        ("editor:toggle-highlight", "Toggle highlight"),
    ]
    .into_iter()
    .map(|(id, name)| CommandEntry {
        id: id.into(),
        name: name.into(),
        hotkeys: Vec::new(),
    })
    .collect();
    let rows = |pinned: &[&str], recent: &[&str]| -> Vec<String> {
        let pinned: Vec<String> = pinned.iter().map(|s| s.to_string()).collect();
        let recent: Vec<String> = recent.iter().map(|s| s.to_string()).collect();
        slash_rows(&commands, &pinned, &recent, "")
            .into_iter()
            .map(|r| r.label)
            .collect()
    };
    assert_eq!(
        rows(&["editor:insert-table", "editor:toggle-bold"], &[]),
        [
            "Insert table",
            "Toggle bold",
            "Add alias",
            "Toggle highlight"
        ]
    );
    assert_eq!(
        rows(&[], &["editor:toggle-highlight"]),
        [
            "Toggle highlight",
            "Add alias",
            "Insert table",
            "Toggle bold"
        ]
    );
    // A query is fuzzy matched; the pin does not lift a row.
    let pinned = vec!["editor:insert-table".to_string()];
    let bold: Vec<String> = slash_rows(&commands, &pinned, &[], "bold")
        .into_iter()
        .map(|r| r.label)
        .collect();
    assert_eq!(bold, ["Toggle bold"]);
}

/// s02: the text before the caret, then `/` typed, then whether the popup
/// opened, in Source mode (Live Preview agreed on every case it could
/// place a caret in).
#[test]
fn where_a_slash_opens_the_popup() {
    let cases: &[(&str, bool)] = &[
        ("", true),
        ("hello ", true),
        ("hello", false),
        ("he", false),
        ("call(", false),
        ("\t", true),
        ("    ", true),
        ("a /", false),
        ("- ", true),
        ("- [ ] ", true),
        ("1. ", true),
        ("> ", true),
        ("# ", true),
        ("> ", true),
        // A fenced block, inline code, frontmatter, a math block and a
        // table cell: nothing suppresses it.
        ("x ", true),
        ("a `b ", true),
        ("key: ", true),
        ("| 1 |  ", true),
        ("see http:", false),
        ("[[Other]] ", true),
        ("%% x ", true),
        ("<!-- x", false),
    ];
    for (before, opens) in cases {
        let line = format!("{before}/");
        assert_eq!(
            slash_trigger(&line, line.len()).is_some(),
            *opens,
            "{line:?}"
        );
    }
    // s06: a space after the slash, or in the query, closes it; Backspace
    // back to the bare `/` keeps it; a second `/` is not one.
    assert!(slash_trigger("/ ", 2).is_none());
    assert!(slash_trigger("/insert table", 13).is_none());
    assert!(slash_trigger("/", 1).is_some());
    assert!(slash_trigger("/a/", 3).is_none());
    // The caret moved back into the query: the query is what is before it.
    assert_eq!(slash_trigger("/bol", 3).map(|t| t.query), Some("bo"));
    assert!(slash_trigger("/bol", 0).is_none());
}

/// A buffer holding `before` with the caret at `caret`, `/query` typed
/// there, the row accepted (the `/query` removed) and then `run` -- the
/// command -- applied to what is left.
fn accept(
    before: &str,
    caret: usize,
    query: &str,
    run: impl FnOnce(&mut TextBuffer),
) -> TextBuffer {
    let mut buf = TextBuffer::from_file(before);
    buf.set_cursor(caret, None);
    buf.insert_all(&format!("/{query}"), EditKind::Typing);
    let cursor = buf.cursor();
    let line = buf.line_text(cursor.line).to_string();
    let t = slash_trigger(&line, cursor.col).expect("the popup is open");
    assert_eq!(t.query, query);
    let (range, caret) = slash_accept_edit(buf.line_range(cursor.line).start, &t);
    let depth = buf.undo_depth();
    buf.edit_many(vec![(range, String::new())], caret, None);
    run(&mut buf);
    buf.merge_undo_from(depth);
    buf
}

/// (line, column) of the caret.
fn caret(buf: &TextBuffer) -> (usize, usize) {
    let c = buf.cursor();
    (c.line, c.col)
}

#[test]
fn an_accepted_row_takes_the_query_out_and_runs_the_command() {
    // Each case: the note, the caret's offset, the query typed after `/`,
    // the command, then Obsidian's bytes and caret (line, column) after
    // Enter. s07 and s09, identical in Live Preview and Source mode.
    type Run = fn(&mut TextBuffer);
    /// The note, the caret, the query, the command, the bytes, the caret.
    type Case = (
        &'static str,
        usize,
        &'static str,
        Run,
        &'static str,
        (usize, usize),
    );
    let cases: &[Case] = &[
        (
            "Hello",
            0,
            "bold",
            |b| {
                b.toggle_wrap("**");
            },
            "**Hello**",
            (0, 2),
        ),
        (
            "Hello ",
            6,
            "bold",
            |b| {
                b.toggle_wrap("**");
            },
            "Hello ****",
            (0, 8),
        ),
        (
            "Hello world",
            6,
            "bold",
            |b| {
                b.toggle_wrap("**");
            },
            "Hello **world**",
            (0, 8),
        ),
        (
            "Hello ",
            6,
            "italic",
            |b| {
                b.toggle_wrap("*");
            },
            "Hello **",
            (0, 7),
        ),
        (
            "word ",
            5,
            "strikethrough",
            |b| {
                b.toggle_wrap("~~");
            },
            "word ~~~~",
            (0, 7),
        ),
        (
            "Intro\n",
            6,
            "table",
            |b| {
                b.insert_table();
            },
            "Intro\n\n|     |     |\n| --- | --- |\n|     |     |\n",
            (2, 1),
        ),
        (
            "Intro ",
            6,
            "table",
            |b| {
                b.insert_table();
            },
            "Intro \n\n|     |     |\n| --- | --- |\n|     |     |\n",
            (2, 1),
        ),
        (
            "",
            0,
            "horizontal",
            |b| {
                b.insert_block(BlockInsert::HorizontalRule);
            },
            "\n---\n",
            (2, 0),
        ),
        (
            "Some ",
            5,
            "codeblock",
            |b| {
                b.insert_block(BlockInsert::CodeBlock);
            },
            "```\nSome \n```",
            (1, 5),
        ),
        (
            "x",
            0,
            "math",
            |b| {
                b.insert_block(BlockInsert::MathBlock);
            },
            "$$\nx\n$$",
            (1, 0),
        ),
        (
            "x ",
            2,
            "footnote",
            |b| {
                b.insert_block(BlockInsert::Footnote);
            },
            "x [^1]\n\n[^1]: \n",
            (0, 6),
        ),
        (
            "Text ",
            5,
            "link",
            |b| {
                b.insert_link(LinkSyntax::Markdown);
            },
            "Text []()",
            (0, 6),
        ),
        (
            "Item",
            0,
            "bullet",
            |b| {
                b.toggle_line_prefix(LinePrefix::Bullet);
            },
            "- Item",
            (0, 2),
        ),
        (
            "one\ntwo",
            4,
            "quote",
            |b| {
                b.toggle_line_prefix(LinePrefix::Quote);
            },
            "one\n> two",
            (1, 2),
        ),
        (
            "- item ",
            7,
            "checkbox",
            |b| {
                b.toggle_checklist();
            },
            "- [ ] item ",
            (0, 11),
        ),
        (
            "Hello",
            0,
            "h2",
            |b| {
                b.set_heading(2);
            },
            "## Hello",
            (0, 3),
        ),
        (
            "## Hello",
            3,
            "h1",
            |b| {
                b.set_heading(1);
            },
            "# Hello",
            (0, 2),
        ),
        (
            "A\nB",
            0,
            "linedown",
            |b| {
                b.swap_lines(false);
            },
            "B\nA",
            (1, 0),
        ),
        // A command that opens a modal (the palette, the quick switcher,
        // Insert template) or a tab writes nothing: the query is gone and
        // the caret is where the `/` was.
        ("Text ", 5, "palette", |_| {}, "Text ", (0, 5)),
        (
            "",
            0,
            "table",
            |b| {
                b.insert_table();
            },
            "\n|     |     |\n| --- | --- |\n|     |     |\n",
            (1, 1),
        ),
        (
            "",
            0,
            "callout",
            |b| {
                b.insert_block(BlockInsert::Callout);
            },
            "\n> [!NOTE] Title\n> Contents\n",
            (1, 8),
        ),
    ];
    for (before, at, query, run, after, at_after) in cases {
        let buf = accept(before, *at, query, *run);
        assert_eq!(buf.text(), *after, "/{query} in {before:?}");
        assert_eq!(
            caret(&buf),
            *at_after,
            "the caret after /{query} in {before:?}"
        );
    }
}

#[test]
fn one_undo_takes_back_the_command_and_the_removal_together() {
    // s09, with 1.2 s between the typing and Enter so the editor could not
    // join them: the first Ctrl+Z gave `Hello /bold` back, the second
    // `Hello `.
    let mut buf = accept("Hello ", 6, "bold", |b| {
        b.toggle_wrap("**");
    });
    assert_eq!(buf.text(), "Hello ****");
    buf.undo();
    assert_eq!(buf.text(), "Hello /bold");
    buf.undo();
    assert_eq!(buf.text(), "Hello ");
    let mut buf = accept("", 0, "table", |b| {
        b.insert_table();
    });
    buf.undo();
    assert_eq!(buf.text(), "/table");
}

#[test]
fn the_caret_inside_the_query_removes_only_what_is_before_it() {
    // s10: `Hello /bold`, the caret moved back two (`/bo|ld`), Enter: the
    // row ran on `Hello ld` with the caret after the space.
    let line = "Hello /bold";
    let t = slash_trigger(line, 9).expect("open");
    let (range, caret) = slash_accept_edit(0, &t);
    let mut text = line.to_string();
    text.replace_range(range, "");
    assert_eq!((text.as_str(), caret), ("Hello ld", 6));
}
