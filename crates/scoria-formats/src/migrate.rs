//! The Format converter's "Frontmatter migration" (`app:migrate-frontmatter`):
//! `tags`, `aliases` and `cssclasses` brought to the list form Obsidian 1.9
//! requires, as Obsidian 1.15.0 converted the 45 notes of the 2026-11-09 probe
//! ([[Specification#Format converter: Frontmatter migration]]).
//!
//! Unlike every other properties write in Scoria, which is surgical
//! ([`crate::properties::plan`]), a note the migration changes has its whole
//! YAML block re-serialised, because that is what Obsidian writes: comments
//! go, a flow list becomes a block list, `'single'` loses its quotes, blank
//! lines inside the block go, and in a CRLF note the inner lines come out with
//! `\n` while both fence lines keep their `\r\n`. A note it does not change is
//! not written at all, so nothing else ever reaches this serialiser.
//!
//! The rules, for each of `tag`/`tags`, `alias`/`aliases` and
//! `cssclass`/`cssclasses` (keys matched ignoring case):
//!
//! - the plural key present: its value converted when it is a string;
//! - else the singular present: renamed to the plural in place, and its value
//!   converted when it is a string (a list or an empty value is renamed only);
//! - both present: the singular left alone, the plural converted;
//! - a string is split on spaces and commas (aliases: on commas only), each
//!   piece trimmed and empty pieces dropped, `#` kept; a string with no pieces
//!   becomes `[]`; numbers, booleans, null and lists are left as they are.
//!
//! Not converted (and so not written): a note with no frontmatter, one whose
//! opening fence follows a BOM, and one whose block is not a YAML mapping.

use crate::properties::{format_number, render_key, render_scalar};
use serde_yaml::{Mapping, Value};

/// What the migration does to one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migration {
    /// Nothing to convert: the file is left as it is.
    Unchanged,
    /// The note's new text.
    Changed(String),
    /// The block would not parse as YAML: counted under "Failed" and left.
    Failed,
}

/// The three key pairs, singular first, and whether a string value splits on
/// spaces as well as commas.
const PAIRS: [(&str, &str, bool); 3] = [
    ("tag", "tags", true),
    ("alias", "aliases", false),
    ("cssclass", "cssclasses", true),
];

/// Converts one note's text.
pub fn migrate_frontmatter(text: &str) -> Migration {
    // Obsidian's converter skipped a note whose fence follows a BOM, though
    // its properties view reads that block.
    let Some((yaml_start, yaml_end)) = fences(text) else {
        return Migration::Unchanged;
    };
    let yaml = &text[yaml_start..yaml_end];
    let parsed: Value = match serde_yaml::from_str(yaml) {
        Ok(v) => v,
        Err(_) => return Migration::Failed,
    };
    let Value::Mapping(map) = parsed else {
        return Migration::Unchanged;
    };
    let mut entries: Vec<(Value, Value)> = map.into_iter().collect();
    let mut changed = false;
    for (singular, plural, spaces) in PAIRS {
        let find = |entries: &[(Value, Value)], name: &str| {
            entries
                .iter()
                .position(|(k, _)| matches!(k, Value::String(s) if s.eq_ignore_ascii_case(name)))
        };
        let ix = match (find(&entries, plural), find(&entries, singular)) {
            (Some(ix), _) => ix,
            (None, Some(ix)) => {
                entries[ix].0 = Value::String(plural.to_string());
                changed = true;
                ix
            }
            (None, None) => continue,
        };
        if let Value::String(s) = &entries[ix].1 {
            entries[ix].1 =
                Value::Sequence(split(s, spaces).into_iter().map(Value::String).collect());
            changed = true;
        }
    }
    if !changed {
        return Migration::Unchanged;
    }
    let mut block = String::new();
    for (key, value) in &entries {
        match render_pair(key, value, 0) {
            Some(lines) => block.push_str(&lines),
            // A tagged value or a complex key: nothing Obsidian's serialiser
            // was seen to write, so the note is left rather than guessed at.
            None => return Migration::Failed,
        }
    }
    Migration::Changed(format!(
        "{}{}{}",
        &text[..yaml_start],
        block,
        &text[yaml_end..]
    ))
}

/// The YAML between the fences: from after the opening `---` line to the
/// start of the closing one. `None` without a block, or with a BOM first.
fn fences(text: &str) -> Option<(usize, usize)> {
    let first_end = text.find('\n')?;
    if text[..first_end].trim_end_matches('\r') != "---" {
        return None;
    }
    let start = first_end + 1;
    let mut pos = start;
    loop {
        let line_end = text[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(text.len());
        if text[pos..line_end].trim_end_matches('\r') == "---" {
            return Some((start, pos));
        }
        if line_end >= text.len() {
            return None;
        }
        pos = line_end + 1;
    }
}

fn split(s: &str, spaces: bool) -> Vec<String> {
    s.split(|c: char| c == ',' || (spaces && c.is_whitespace()))
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// `key: value` at `indent` spaces, each line ending in `\n`.
fn render_pair(key: &Value, value: &Value, indent: usize) -> Option<String> {
    let pad = " ".repeat(indent);
    let key = match key {
        Value::String(s) => render_key(s),
        Value::Number(_) | Value::Bool(_) => scalar(key)?,
        _ => return None,
    };
    let head = format!("{pad}{key}:");
    Some(match value {
        Value::Null => format!("{head}\n"),
        Value::Sequence(items) if items.is_empty() => format!("{head} []\n"),
        Value::Mapping(map) if map.is_empty() => format!("{head} {{}}\n"),
        Value::Sequence(items) => format!("{head}\n{}", render_items(items, indent + 2)?),
        Value::Mapping(map) => format!("{head}\n{}", render_map(map, indent + 2)?),
        Value::String(s) if s.contains('\n') => format!("{head} {}", block_scalar(s, indent + 2)),
        _ => format!("{head} {}\n", scalar(value)?),
    })
}

fn render_map(map: &Mapping, indent: usize) -> Option<String> {
    let mut out = String::new();
    for (k, v) in map {
        out.push_str(&render_pair(k, v, indent)?);
    }
    Some(out)
}

/// A block sequence: `- item` at `indent` spaces, a mapping item's first
/// entry on the dash's line and the rest under it.
fn render_items(items: &[Value], indent: usize) -> Option<String> {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    for item in items {
        match item {
            Value::Null => out.push_str(&format!("{pad}- null\n")),
            Value::Sequence(inner) if inner.is_empty() => out.push_str(&format!("{pad}- []\n")),
            Value::Mapping(inner) if inner.is_empty() => out.push_str(&format!("{pad}- {{}}\n")),
            Value::Sequence(inner) => {
                let nested = render_items(inner, indent + 2)?;
                out.push_str(&format!("{pad}- {}", &nested[indent + 2..]));
            }
            Value::Mapping(inner) => {
                let nested = render_map(inner, indent + 2)?;
                out.push_str(&format!("{pad}- {}", &nested[indent + 2..]));
            }
            Value::String(s) if s.contains('\n') => {
                out.push_str(&format!("{pad}- {}", block_scalar(s, indent + 2)))
            }
            _ => out.push_str(&format!("{pad}- {}\n", scalar(item)?)),
        }
    }
    Some(out)
}

/// A one-line scalar as Obsidian writes it.
fn scalar(value: &Value) -> Option<String> {
    Some(match value {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64()?;
                if f.is_nan() {
                    ".nan".to_string()
                } else if f.is_infinite() {
                    if f > 0.0 { ".inf" } else { "-.inf" }.to_string()
                } else {
                    format_number(f)
                }
            }
        }
        Value::String(s) => render_scalar(s),
        _ => return None,
    })
}

/// A string with a newline: `|-` when it does not end in one, `|` when it
/// ends in exactly one, `|+` when more; the lines at `indent`.
fn block_scalar(s: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let body = s.trim_end_matches('\n');
    let chomp = match s.len() - body.len() {
        0 => "|-",
        1 => "|",
        _ => "|+",
    };
    let mut out = format!("{chomp}\n");
    let mut lines: Vec<&str> = body.split('\n').collect();
    if chomp == "|+" {
        lines.extend(std::iter::repeat_n("", s.len() - body.len() - 1));
    }
    for line in lines {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&pad);
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Obsidian 1.15.0's own output for every note of the 2026-11-09 probe
    /// (`tools/obsidian-probe/results/2026-11-09-attachments-app-info.json`,
    /// step `s04-migrate-frontmatter` of its three runs): the note's name,
    /// its bytes before, its bytes after. Equal bytes mean the file was not
    /// written.
    const OBSERVED: &[(&str, &str, &str)] = &[
        (
            "m01 tags comma",
            "---\ntags: a, b\n---\nbody\n",
            "---\ntags:\n  - a\n  - b\n---\nbody\n",
        ),
        (
            "m02 tags spaces",
            "---\ntags: \"#a #b\"\n---\nbody\n",
            "---\ntags:\n  - \"#a\"\n  - \"#b\"\n---\nbody\n",
        ),
        (
            "m03 tag singular",
            "---\ntag: x\n---\nbody\n",
            "---\ntags:\n  - x\n---\nbody\n",
        ),
        (
            "m04 tags flow",
            "---\ntags: [a, b]\n---\nbody\n",
            "---\ntags: [a, b]\n---\nbody\n",
        ),
        (
            "m05 tags block",
            "---\ntags:\n  - a\n  - b\n---\nbody\n",
            "---\ntags:\n  - a\n  - b\n---\nbody\n",
        ),
        (
            "m06 aliases scalar",
            "---\naliases: foo\n---\nbody\n",
            "---\naliases:\n  - foo\n---\nbody\n",
        ),
        (
            "m07 alias singular",
            "---\nalias: foo, bar\n---\nbody\n",
            "---\naliases:\n  - foo\n  - bar\n---\nbody\n",
        ),
        (
            "m08 cssclass",
            "---\ncssclass: wide\n---\nbody\n",
            "---\ncssclasses:\n  - wide\n---\nbody\n",
        ),
        (
            "m09 cssclasses string",
            "---\ncssclasses: wide narrow\n---\nbody\n",
            "---\ncssclasses:\n  - wide\n  - narrow\n---\nbody\n",
        ),
        (
            "m10 tags hash",
            "---\ntags: \"#a\"\n---\nbody\n",
            "---\ntags:\n  - \"#a\"\n---\nbody\n",
        ),
        ("m11 no frontmatter", "tags: a\nbody\n", "tags: a\nbody\n"),
        (
            "m12 crlf",
            "---\r\ntitle: T\r\ntags: a\r\nother: 1\r\n---\r\nbody\r\n",
            "---\r\ntitle: T\ntags:\n  - a\nother: 1\n---\r\nbody\r\n",
        ),
        (
            "m13 tags empty",
            "---\ntags:\n---\nbody\n",
            "---\ntags:\n---\nbody\n",
        ),
        (
            "m14 tags number",
            "---\ntags: 123\n---\nbody\n",
            "---\ntags: 123\n---\nbody\n",
        ),
        (
            "m15 both tag and tags",
            "---\ntag: x\ntags: [y]\n---\nbody\n",
            "---\ntag: x\ntags: [y]\n---\nbody\n",
        ),
        (
            "m16 order kept",
            "---\nz: 1\ntags: a\ny: 2\naliases: b\nx: 3\n---\nbody\n",
            "---\nz: 1\ntags:\n  - a\ny: 2\naliases:\n  - b\nx: 3\n---\nbody\n",
        ),
        (
            "Sub/m17 nested",
            "---\ntags: n1 n2\n---\nbody\n",
            "---\ntags:\n  - n1\n  - n2\n---\nbody\n",
        ),
        (
            "m18 tags one word",
            "---\ntags: solo\n---\nbody\n",
            "---\ntags:\n  - solo\n---\nbody\n",
        ),
        (
            "m19 aliases list",
            "---\naliases:\n  - one\n  - two\n---\nbody\n",
            "---\naliases:\n  - one\n  - two\n---\nbody\n",
        ),
        (
            "m20 tags quoted comma",
            "---\ntags: 'a, b'\n---\nbody\n",
            "---\ntags:\n  - a\n  - b\n---\nbody\n",
        ),
        (
            "m21 alias and aliases",
            "---\nalias: x\naliases: y\n---\nbody\n",
            "---\nalias: x\naliases:\n  - y\n---\nbody\n",
        ),
        (
            "m22 tags flow hash",
            "---\ntags: [\"#a\", b]\n---\nbody\n",
            "---\ntags: [\"#a\", b]\n---\nbody\n",
        ),
        (
            "m23 cssclass list",
            "---\ncssclass:\n  - wide\n---\nbody\n",
            "---\ncssclasses:\n  - wide\n---\nbody\n",
        ),
        (
            "m24 tags comma no space",
            "---\ntags: a,b,c\n---\nbody\n",
            "---\ntags:\n  - a\n  - b\n  - c\n---\nbody\n",
        ),
        (
            "m25 bom",
            "\u{feff}---\ntags: a\n---\nbody\n",
            "\u{feff}---\ntags: a\n---\nbody\n",
        ),
        (
            "m26 comment",
            "---\ntags: a # note\nk: v # keep\n---\nbody\n",
            "---\ntags:\n  - a\nk: v\n---\nbody\n",
        ),
        (
            "m27 tags nested",
            "---\ntags: a/b c/d\n---\nbody\n",
            "---\ntags:\n  - a/b\n  - c/d\n---\nbody\n",
        ),
        (
            "m28 other keys only",
            "---\ntitle: plain\n---\nbody\n",
            "---\ntitle: plain\n---\nbody\n",
        ),
        (
            "n01 aliases spaces",
            "---\naliases: New York\n---\nbody\n",
            "---\naliases:\n  - New York\n---\nbody\n",
        ),
        (
            "n02 aliases comma spaces",
            "---\naliases: New York, LA\n---\nbody\n",
            "---\naliases:\n  - New York\n  - LA\n---\nbody\n",
        ),
        (
            "n03 tags comma and spaces",
            "---\ntags: a b, c\n---\nbody\n",
            "---\ntags:\n  - a\n  - b\n  - c\n---\nbody\n",
        ),
        (
            "n04 cssclasses comma",
            "---\ncssclasses: wide, narrow\n---\nbody\n",
            "---\ncssclasses:\n  - wide\n  - narrow\n---\nbody\n",
        ),
        (
            "n05 tags duplicate",
            "---\ntags: a a\n---\nbody\n",
            "---\ntags:\n  - a\n  - a\n---\nbody\n",
        ),
        (
            "n06 tags trailing comma",
            "---\ntags: a,\n---\nbody\n",
            "---\ntags:\n  - a\n---\nbody\n",
        ),
        (
            "n07 tags with hash unquoted list",
            "---\ntags:\n  - \"#a\"\n---\nbody\n",
            "---\ntags:\n  - \"#a\"\n---\nbody\n",
        ),
        (
            "n08 Tags capital",
            "---\nTags: a b\n---\nbody\n",
            "---\nTags:\n  - a\n  - b\n---\nbody\n",
        ),
        (
            "n09 tags bool",
            "---\ntags: true\n---\nbody\n",
            "---\ntags: true\n---\nbody\n",
        ),
        (
            "n10 alias list",
            "---\nalias:\n  - x\n---\nbody\n",
            "---\naliases:\n  - x\n---\nbody\n",
        ),
        (
            "n11 cssclass and cssclasses",
            "---\ncssclass: a\ncssclasses: b\n---\nbody\n",
            "---\ncssclass: a\ncssclasses:\n  - b\n---\nbody\n",
        ),
        (
            "n12 nested other",
            "---\nmeta:\n  k: v\ntags: a\n---\nbody\n",
            "---\nmeta:\n  k: v\ntags:\n  - a\n---\nbody\n",
        ),
        (
            "n13 flow other",
            "---\nlist: [1, 2]\ntags: a\n---\nbody\n",
            "---\nlist:\n  - 1\n  - 2\ntags:\n  - a\n---\nbody\n",
        ),
        (
            "n14 date and quotes",
            "---\nd: 2024-01-02\ns: 'single'\ntags: a\n---\nbody\n",
            "---\nd: 2024-01-02\ns: single\ntags:\n  - a\n---\nbody\n",
        ),
        (
            "n15 blank line",
            "---\na: 1\n\ntags: a\n---\nbody\n",
            "---\na: 1\ntags:\n  - a\n---\nbody\n",
        ),
        (
            "n16 tag empty",
            "---\ntag:\n---\nbody\n",
            "---\ntags:\n---\nbody\n",
        ),
        (
            "n17 aliases empty string",
            "---\naliases: \"\"\n---\nbody\n",
            "---\naliases: []\n---\nbody\n",
        ),
    ];

    #[test]
    fn matches_every_observed_note_byte_for_byte() {
        assert_eq!(OBSERVED.len(), 45);
        for (name, before, after) in OBSERVED {
            let got = match migrate_frontmatter(before) {
                Migration::Changed(text) => {
                    assert_ne!(&text, before, "{name}: a change that changes nothing");
                    text
                }
                Migration::Unchanged => before.to_string(),
                Migration::Failed => panic!("{name}: failed"),
            };
            assert_eq!(&got, after, "{name}");
        }
    }

    /// A second run over converted notes changes none (Modified files 0).
    #[test]
    fn a_second_run_changes_nothing() {
        for (name, _, after) in OBSERVED {
            assert_eq!(migrate_frontmatter(after), Migration::Unchanged, "{name}");
        }
    }

    #[test]
    fn an_unparsable_block_fails_and_is_left() {
        assert_eq!(
            migrate_frontmatter("---\ntags: a\ntags: b\n---\n"),
            Migration::Failed
        );
        assert_eq!(
            migrate_frontmatter("---\ntags: [a\n---\n"),
            Migration::Failed
        );
    }

    #[test]
    fn an_unclosed_block_is_not_frontmatter() {
        assert_eq!(migrate_frontmatter("---\ntags: a\n"), Migration::Unchanged);
    }

    /// The serialiser's shapes beyond the probe's: nested lists and maps, a
    /// list of maps, block scalars, quoting.
    #[test]
    fn re_serialises_the_other_shapes() {
        let before = concat!(
            "---\n",
            "tags: a\n",
            "people:\n  - name: Ann\n    age: 3\n  - name: Bo\n",
            "grid: [[1, 2], [3]]\n",
            "deep:\n  inner:\n    list: [x, \"#y\"]\n",
            "text: |\n  one\n  two\n",
            "keep: |-\n  one\n  two\n",
            "n: 1.5\n",
            "q: \"42\"\n",
            "e: {}\n",
            "---\n",
            "body\n",
        );
        let Migration::Changed(after) = migrate_frontmatter(before) else {
            panic!("not changed");
        };
        assert_eq!(
            after,
            concat!(
                "---\n",
                "tags:\n  - a\n",
                "people:\n  - name: Ann\n    age: 3\n  - name: Bo\n",
                "grid:\n  - - 1\n    - 2\n  - - 3\n",
                "deep:\n  inner:\n    list:\n      - x\n      - \"#y\"\n",
                "text: |\n  one\n  two\n",
                "keep: |-\n  one\n  two\n",
                "n: 1.5\n",
                "q: \"42\"\n",
                "e: {}\n",
                "---\n",
                "body\n",
            )
        );
    }

    #[test]
    fn a_singular_key_in_another_case_is_renamed() {
        assert_eq!(
            migrate_frontmatter("---\nAlias: a, b\n---\n"),
            Migration::Changed("---\naliases:\n  - a\n  - b\n---\n".into())
        );
    }
}
