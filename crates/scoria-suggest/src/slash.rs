//! Slash commands: `/` in the editor offers the commands the palette
//! offers, and accepting one takes the `/query` out and runs the command.
//!
//! Observed in Obsidian 1.15.0 and recorded in
//! [[Specification#Observed Slash commands, Obsidian 1.15.0]]: the core
//! plugin `slash-command` (off in a fresh vault, no options, no command
//! of its own) adds one more editor suggester. The trigger is the last `/`
//! before the caret on the caret's line, at the start of the line or after
//! whitespace, with no whitespace between it and the caret. Nothing about
//! the surrounding Markdown suppresses it: a code block, inline code, a
//! table cell and frontmatter all open it. The rows are the palette's list
//! in the palette's order -- pinned, then recent, then by name -- fuzzy
//! matched by name, at most [`LIMIT`] of them, and the popup closes rather
//! than show an empty list.
//!
//! The ordering lives here, free of gpui, and the command palette uses it
//! too, so the two lists cannot drift apart.

use crate::candidates::{Item, Suggestion, LIMIT};
use scoria_search::fuzzy_match;
use std::cmp::Ordering;
use std::ops::Range;

/// A `/query` before the caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashTriggered<'a> {
    /// The `/` through the caret: the bytes accepting a row removes. Text
    /// after the caret stays, even when it continues the word (`/bo|ld`
    /// accepted leaves `ld`).
    pub range: Range<usize>,
    /// What was typed after the `/`.
    pub query: &'a str,
}

/// What `line` asks the slash popup for with the caret at byte column `col`.
///
/// `/` opens it at the start of a line or after whitespace (a space, a tab,
/// the space after `- `, `> ` or `# `), never after a letter or punctuation
/// (`hello/`, `call(/`, `http:/`, `a //`). A space after the `/` closes it,
/// so a query is one word.
pub fn slash_trigger(line: &str, col: usize) -> Option<SlashTriggered<'_>> {
    let col = col.min(line.len());
    let before = line.get(..col)?;
    let slash = before.rfind('/')?;
    let query = &before[slash + 1..];
    if query.chars().any(char::is_whitespace) {
        return None;
    }
    if !before[..slash]
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace)
    {
        return None;
    }
    Some(SlashTriggered {
        range: slash..col,
        query,
    })
}

/// What accepting a row does to the text before the command runs: the
/// `/query` (through the caret, not past it) is removed and the caret put
/// where the `/` was. `line_start` is the caret line's offset in the
/// document; the range is in document offsets.
///
/// Obsidian then runs the command on that text, and one undo takes back
/// the removal and whatever the command wrote together, leaving the
/// `/query` as it was typed.
pub fn slash_accept_edit(line_start: usize, t: &SlashTriggered<'_>) -> (Range<usize>, usize) {
    let range = line_start + t.range.start..line_start + t.range.end;
    let caret = range.start;
    (range, caret)
}

/// A command as the palette and the slash popup list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: String,
    /// The palette name, `Plugin: name` for a plugin's command.
    pub name: String,
    /// The hotkeys as the palette draws them (`Ctrl + B`).
    pub hotkeys: Vec<String>,
}

/// One row of a command list: an index into the entries, and the byte
/// ranges of its name the query matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    pub index: usize,
    pub ranges: Vec<Range<usize>>,
}

/// The order the palette lists commands in, and the slash popup with it.
///
/// An empty query lists the pinned commands in `command-palette.json`'s
/// order, then the commands run recently (most recent first), then every
/// other command by name. A query keeps the commands whose name it fuzzy
/// matches, best score first, a shorter name first between equal scores,
/// and by name after that. Ids in `pinned` or `recent` that are not in
/// `commands` are skipped.
pub fn rank_commands(
    commands: &[CommandEntry],
    pinned: &[String],
    recent: &[String],
    query: &str,
) -> Vec<Ranked> {
    let mut by_name: Vec<usize> = (0..commands.len()).collect();
    by_name.sort_by(|&a, &b| name_order(&commands[a].name, &commands[b].name));
    let query = query.trim();
    if query.is_empty() {
        let mut out: Vec<usize> = Vec::with_capacity(commands.len());
        let index_of = |id: &str| commands.iter().position(|c| c.id == id);
        for id in pinned.iter().chain(recent) {
            if let Some(ix) = index_of(id) {
                if !out.contains(&ix) {
                    out.push(ix);
                }
            }
        }
        let head = out.clone();
        out.extend(by_name.into_iter().filter(|ix| !head.contains(ix)));
        return out
            .into_iter()
            .map(|index| Ranked {
                index,
                ranges: Vec::new(),
            })
            .collect();
    }
    let mut scored: Vec<(f64, Ranked)> = by_name
        .into_iter()
        .filter_map(|index| {
            let m = fuzzy_match(query, &commands[index].name)?;
            Some((
                m.score,
                Ranked {
                    index,
                    ranges: m.ranges,
                },
            ))
        })
        .collect();
    // Stable, so names that tie on score and length stay in name order.
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                commands[a.1.index]
                    .name
                    .len()
                    .cmp(&commands[b.1.index].name.len())
            })
    });
    scored.into_iter().map(|(_, r)| r).collect()
}

/// The rows the slash popup shows for `query`: [`rank_commands`] cut at
/// [`LIMIT`], as suggestions whose item names the command to run. Empty
/// when nothing matches, which closes the popup.
pub fn slash_rows(
    commands: &[CommandEntry],
    pinned: &[String],
    recent: &[String],
    query: &str,
) -> Vec<Suggestion> {
    rank_commands(commands, pinned, recent, query)
        .into_iter()
        .take(LIMIT)
        .map(|r| {
            let c = &commands[r.index];
            Suggestion {
                item: Item::Command {
                    id: c.id.clone(),
                    hotkeys: c.hotkeys.clone(),
                },
                label: c.name.clone(),
                detail: None,
                ranges: r.ranges,
            }
        })
        .collect()
}

/// Command names in the order Obsidian's lists put them: case ignored,
/// whitespace before punctuation before digits before letters (so
/// `Bookmark all tabs...` comes before `Bookmark...`, and `Copy current
/// file path` before `Copy Obsidian URL`), then lower case before upper.
pub fn name_order(a: &str, b: &str) -> Ordering {
    fn class(c: char) -> u8 {
        if c.is_whitespace() {
            0
        } else if c.is_alphabetic() {
            3
        } else if c.is_numeric() {
            2
        } else {
            1
        }
    }
    let key = |s: &str| -> Vec<(u8, char)> {
        s.chars()
            .flat_map(|c| c.to_lowercase().map(move |l| (class(c), l)))
            .collect()
    };
    key(a).cmp(&key(b)).then_with(|| {
        // Lower case first where the letters are the same.
        let lower_first = |s: &str| -> Vec<bool> { s.chars().map(|c| c.is_uppercase()).collect() };
        lower_first(a).cmp(&lower_first(b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(line: &str) -> Option<(&str, &str)> {
        let t = slash_trigger(line, line.len())?;
        Some((&line[t.range.clone()], t.query))
    }

    #[test]
    fn a_slash_opens_at_a_line_start_or_after_whitespace() {
        assert_eq!(at_end("/"), Some(("/", "")));
        assert_eq!(at_end("/head"), Some(("/head", "head")));
        assert_eq!(at_end("hello /"), Some(("/", "")));
        assert_eq!(at_end("\t/"), Some(("/", "")));
        assert_eq!(at_end("    /ta"), Some(("/ta", "ta")));
        assert_eq!(at_end("- /"), Some(("/", "")));
        assert_eq!(at_end("- [ ] /"), Some(("/", "")));
        assert_eq!(at_end("1. /"), Some(("/", "")));
        assert_eq!(at_end("> /"), Some(("/", "")));
        assert_eq!(at_end("# /"), Some(("/", "")));
        assert_eq!(at_end("[[Other]] /"), Some(("/", "")));
        assert_eq!(at_end("%% x /"), Some(("/", "")));
        assert_eq!(at_end("| 1 | /"), Some(("/", "")));
        assert_eq!(at_end("a `b /"), Some(("/", "")));
        assert_eq!(at_end("key: /"), Some(("/", "")));
    }

    #[test]
    fn not_after_a_letter_or_punctuation() {
        assert_eq!(at_end("hello/"), None);
        assert_eq!(at_end("call(/"), None);
        assert_eq!(at_end("see http:/"), None);
        assert_eq!(at_end("a //"), None);
        assert_eq!(at_end("/a/"), None);
        assert_eq!(at_end(""), None);
        assert_eq!(at_end("plain"), None);
    }

    #[test]
    fn a_space_ends_the_query() {
        assert_eq!(at_end("/ "), None);
        assert_eq!(at_end("/insert table"), None);
        assert_eq!(at_end("/toggle bold "), None);
    }

    #[test]
    fn the_query_stops_at_the_caret() {
        // `/abc` with the caret after the `/`: an empty query.
        let t = slash_trigger("/abc", 1).expect("opens");
        assert_eq!((t.range.clone(), t.query), (0..1, ""));
        // `/bo|ld`: only `/bo` is replaced.
        let t = slash_trigger("Hello /bold", 9).expect("opens");
        assert_eq!((t.range.clone(), t.query), (6..9, "bo"));
        // Back past the slash closes it.
        assert_eq!(slash_trigger("/bol", 0), None);
    }

    #[test]
    fn multibyte_text_keeps_byte_columns() {
        let line = "héllo /tâb";
        let t = slash_trigger(line, line.len()).expect("opens");
        assert_eq!((&line[t.range.clone()], t.query), ("/tâb", "tâb"));
        assert_eq!(slash_trigger("é/", 3), None);
    }

    fn entry(id: &str, name: &str) -> CommandEntry {
        CommandEntry {
            id: id.into(),
            name: name.into(),
            hotkeys: Vec::new(),
        }
    }

    #[test]
    fn pinned_then_recent_then_by_name() {
        let commands = vec![
            entry("c", "Toggle highlight"),
            entry("a", "Add alias"),
            entry("t", "Insert table"),
            entry("b", "Toggle bold"),
            entry("x", "Close window"),
        ];
        let names = |pinned: &[String], recent: &[String]| -> Vec<String> {
            rank_commands(&commands, pinned, recent, "")
                .into_iter()
                .map(|r| commands[r.index].name.clone())
                .collect()
        };
        assert_eq!(
            names(&[], &[]),
            [
                "Add alias",
                "Close window",
                "Insert table",
                "Toggle bold",
                "Toggle highlight"
            ]
        );
        // Pinned in the file's order, a recent one after them, an unknown
        // id skipped and a pinned one not repeated.
        assert_eq!(
            names(
                &["t".into(), "b".into(), "gone".into()],
                &["c".into(), "t".into()]
            ),
            [
                "Insert table",
                "Toggle bold",
                "Toggle highlight",
                "Add alias",
                "Close window"
            ]
        );
    }

    #[test]
    fn rows_stop_at_the_limit_and_name_the_command() {
        let commands: Vec<CommandEntry> = (0..150)
            .map(|i| entry(&format!("id{i}"), &format!("Command {i:03}")))
            .collect();
        let rows = slash_rows(&commands, &[], &[], "");
        assert_eq!(rows.len(), LIMIT);
        assert_eq!(rows[0].label, "Command 000");
        assert_eq!(
            rows[0].item,
            Item::Command {
                id: "id0".into(),
                hotkeys: Vec::new()
            }
        );
        assert!(slash_rows(&commands, &[], &[], "xyz").is_empty());
    }
}
