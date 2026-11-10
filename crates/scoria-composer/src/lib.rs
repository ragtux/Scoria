//! The Note composer core plugin's bytes, free of gpui: what a merge writes
//! into the note it merges into, what an extraction leaves behind and where
//! it puts what it took, and the template both wrap their text in.
//!
//! Every rule here is Obsidian 1.15.0's, read off disk after each step of
//! `tools/obsidian-probe/note-composer-unique.mjs` (results
//! `tools/obsidian-probe/results/2026-11-09-note-composer-unique.json`), and
//! the tests are that probe's own before and after bytes:
//!
//! - **The joint** ([`joint`]): the part that comes first is padded to end in
//!   a blank line — nothing when it already ends in `\n\n`, one `\n` after a
//!   single newline, `\n\n` after none — and the part that follows is added
//!   untouched. The padding is a bare `\n` even in a CRLF note.
//! - **Two frontmatters** ([`merge_notes`]) become one, at the top, whichever
//!   way the bodies are joined: the target's keys keep their place, the
//!   source's new keys follow, a scalar clash takes the source's value and a
//!   list clash the union, written as a block list.
//! - **The template** ([`expand_template`]) fills `{{fromTitle}}`,
//!   `{{newTitle}}`, `{{date:FORMAT}}` and `{{content}}`; one without
//!   `{{content}}` gets the content after `\n\n`.
//! - **A heading's section** ([`heading_section`]) runs from the heading line
//!   to the next heading of the same or a higher level, and one newline of
//!   it stays behind with the link.
//!
//! Links to a merged note are rewritten by the rename planner,
//! [`scoria_cache::MetadataCache::plan_rename_rewrites`], and the link an
//! extraction leaves is [`scoria_suggest::file_link`], the one
//! autocompletion writes: there is no third link writer here.

use scoria_cache::{
    basename, note_properties, CachedMetadata, LinkFormat, LinkResolver, MetadataCache, Rewrite,
};
use scoria_formats::properties::{apply, plan, Change, Properties, Value};
use scoria_templates::Stamp;
use std::ops::Range;

// ------------------------------------------------------------------ strings

/// The prompt's placeholder, for merging and for both extractions alike.
pub const PLACEHOLDER: &str = "Select file to merge into...";
/// The empty prompt on a vault with no recent files.
pub const NO_RECENT: &str = "No recent files found. Type to search...";
/// A query nothing matches.
pub const NO_FILES: &str = "No files found.";
/// The `suggestion-action` on an extraction's row for a note not yet made.
pub const ENTER_TO_CREATE: &str = "Enter to create";
/// The merge prompt's two instructions, in their order.
pub const MERGE_INSTRUCTIONS: [(&str, &str); 2] = [
    ("ctrl \u{21b5}", "to create new"),
    ("shift \u{21b5}", "to merge at top"),
];
/// The extraction prompt's, which differ in the second.
pub const EXTRACT_INSTRUCTIONS: [(&str, &str); 2] = [
    ("ctrl \u{21b5}", "to create new"),
    ("shift \u{21b5}", "to move to top"),
];
/// The confirmation's title, checkbox and two buttons.
pub const CONFIRM_TITLE: &str = "Merge file";
pub const DONT_ASK_AGAIN: &str = "Don't ask again";
pub const MERGE: &str = "Merge";
pub const CANCEL: &str = "Cancel";

/// The confirmation's text, with the basenames in curly quotes.
pub fn confirmation(source: &str, target: &str) -> String {
    let (s, t) = (basename(source), basename(target));
    format!(
        "Are you sure you want to merge \u{201c}{s}\u{201d} into \u{201c}{t}\u{201d}? \u{201c}{s}\u{201d} will be deleted."
    )
}

/// The Notice an extraction raises for a name it will not create, verbatim:
/// the three characters are separated by no-break spaces.
pub const NAME_ERROR: &str =
    "Error: File name cannot contain any of these characters: \\\u{a0}/\u{a0}:";

/// Refuses a name the composer would not create a note under: one with a
/// `\`, a `/` or a `:` in it, the three its Notice names
/// (`## Sierra/Romeo: Quebec? *em* [[Link]]` was refused, nothing changed).
pub fn check_name(name: &str) -> Result<(), &'static str> {
    if name.contains(['\\', '/', ':']) {
        Err(NAME_ERROR)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------- settings

/// `replacementText` in `note-composer.json`: what an extraction leaves in
/// place of the text it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Replacement {
    /// "Link to new file", the default.
    #[default]
    Link,
    /// "Embed new file".
    Embed,
    /// "None": the text is simply cut.
    None,
}

impl Replacement {
    /// The config value; absent and unknown values are the default, `link`.
    pub fn from_config(value: Option<&str>) -> Replacement {
        match value {
            Some("embed") => Replacement::Embed,
            Some("none") => Replacement::None,
            _ => Replacement::Link,
        }
    }
}

// ------------------------------------------------------------------- joint

/// Which way two notes' bodies are joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Enter: the merged text after the target's.
    Append,
    /// Shift+Enter: before it ("to merge at top", "to move to top").
    Prepend,
}

/// `first` padded to end in a blank line, then `second` as it is.
///
/// An empty `first` takes no padding: a merge into the note Ctrl+Enter just
/// created left it the source's bytes exactly (`"k3b src\n"`), and an
/// extraction into a new note its selection exactly (`"PICK ME"`).
pub fn joint(first: &str, second: &str) -> String {
    if first.is_empty() {
        return second.to_string();
    }
    let pad = if first.ends_with("\n\n") {
        ""
    } else if first.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let mut out = String::with_capacity(first.len() + pad.len() + second.len());
    out.push_str(first);
    out.push_str(pad);
    out.push_str(second);
    out
}

/// A note's frontmatter block (the fences and the newline after the closing
/// one) and the body after it, with a byte order mark left off the body of
/// a note that has no block.
fn split_note(text: &str) -> (Option<Properties>, &str) {
    match note_properties(text) {
        Some(props) => {
            let end = props.block.end;
            (Some(props), &text[end..])
        }
        None => (None, text.strip_prefix('\u{feff}').unwrap_or(text)),
    }
}

/// The value of a clashing key that one side or the other holds as a list,
/// as list items.
fn items(value: &Value) -> Vec<String> {
    match value {
        Value::List(items) => items.clone(),
        Value::Empty => Vec::new(),
        other => vec![other.display()],
    }
}

/// `target` (a whole note) with `source`'s frontmatter merged into its own:
/// target keys first and in place, a source key it lacks after them, a
/// scalar clash taking the source's value, a list clash becoming the union
/// (the target's items, then the source's not already there) as a block
/// list. Only the entries that change are rewritten.
fn merge_blocks(target: &str, tp: &Properties, sp: &Properties) -> String {
    let mut changes = Vec::new();
    for entry in &sp.entries {
        let value = match tp.get(&entry.key) {
            Some(existing)
                if matches!(existing.value, Value::List(_))
                    || matches!(entry.value, Value::List(_)) =>
            {
                let mut union = items(&existing.value);
                for item in items(&entry.value) {
                    if !union.contains(&item) {
                        union.push(item);
                    }
                }
                Value::List(union)
            }
            _ => entry.value.clone(),
        };
        changes.push(Change::Set {
            key: entry.key.clone(),
            value,
        });
    }
    match plan(target, tp, &changes) {
        Some(edits) => apply(target, &edits),
        None => target.to_string(),
    }
}

/// The note a merge leaves: `target` and `source` (both whole notes) with
/// one frontmatter at the top and their bodies joined by [`joint`] in the
/// order `place` gives. `wrap` is applied to the source's body before the
/// join — the template, when one is set; the identity otherwise.
pub fn merge_notes(
    target: &str,
    source: &str,
    place: Placement,
    wrap: &dyn Fn(&str) -> String,
) -> String {
    let (tp, _) = split_note(target);
    let (sp, source_body) = split_note(source);
    let source_body = wrap(source_body);
    // The target with the source's keys in its block, and where its body
    // starts now.
    let (head, target_body) = match (&tp, &sp) {
        (Some(tp), Some(sp)) => {
            let merged = merge_blocks(target, tp, sp);
            let end = note_properties(&merged).map(|p| p.block.end).unwrap_or(0);
            (merged[..end].to_string(), merged[end..].to_string())
        }
        (Some(tp), None) => (
            target[..tp.block.end].to_string(),
            target[tp.block.end..].to_string(),
        ),
        // The source's block, as it was written, above a target that had
        // none.
        (None, Some(sp)) => {
            let mut block = source[sp.block.clone()].to_string();
            if !block.ends_with('\n') {
                block.push('\n');
            }
            let body = target.strip_prefix('\u{feff}').unwrap_or(target);
            let bom = if body.len() < target.len() {
                "\u{feff}"
            } else {
                ""
            };
            (format!("{bom}{block}"), body.to_string())
        }
        (None, None) => (String::new(), target.to_string()),
    };
    let body = match place {
        Placement::Append => joint(&target_body, &source_body),
        Placement::Prepend => joint(&source_body, &target_body),
    };
    head + &body
}

// ---------------------------------------------------------------- template

/// What a composer template's variables read.
#[derive(Debug, Clone, Copy)]
pub struct TemplateVars<'a> {
    /// `{{fromTitle}}`: the basename of the note the text comes from.
    pub from_title: &'a str,
    /// `{{newTitle}}`: the basename of the note it goes into.
    pub new_title: &'a str,
    /// The instant `{{date:FORMAT}}` formats.
    pub at: Stamp,
}

/// `template` with its four variables filled in around `content`.
///
/// `{{content}}` is the text as it is; a template that never names it gets
/// the content after `\n\n` (`"T:…|END\n"` became `"T:…|END\n\n\nPICK ME"`).
/// The content is inserted, never scanned, so a `{{` in a note stays text.
/// Only the four variables the settings tab names are filled: `{{title}}`,
/// `{{time}}` and a bare `{{date}}` are left as written, since nothing
/// observed them here.
pub fn expand_template(template: &str, content: &str, vars: &TemplateVars<'_>) -> String {
    let mut out = String::with_capacity(template.len() + content.len());
    let mut had_content = false;
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            out.push_str("{{");
            rest = after;
            continue;
        };
        let body = &after[..close];
        let filled = match body {
            "content" => {
                had_content = true;
                Some(content.to_string())
            }
            "fromTitle" => Some(vars.from_title.to_string()),
            "newTitle" => Some(vars.new_title.to_string()),
            _ => body
                .strip_prefix("date:")
                .filter(|f| !f.is_empty())
                .map(|format| scoria_templates::format(format, &vars.at)),
        };
        match filled {
            Some(text) => {
                out.push_str(&text);
                rest = &after[close + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    if !had_content {
        out.push_str("\n\n");
        out.push_str(content);
    }
    out
}

// ----------------------------------------------------------------- heading

/// The text extract-heading takes for the heading on `line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingSection {
    /// What is cut and replaced: the heading line to the end of the line
    /// before the next heading of its level or higher (or the end of the
    /// note), less the one newline that stays behind with the link.
    pub range: Range<usize>,
    /// The default name the prompt offers: the heading's text as written,
    /// inline markup and all.
    pub name: String,
}

/// The section of the heading on zero-based `line`, from the metadata cache
/// the one Markdown parser built for `text`; `None` when the line is not a
/// heading.
pub fn heading_section(text: &str, meta: &CachedMetadata, line: usize) -> Option<HeadingSection> {
    let ix = meta
        .headings
        .iter()
        .position(|h| h.position.start.line as usize == line)?;
    let heading = &meta.headings[ix];
    let line_start = |offset: usize| text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let start = line_start(heading.position.start.offset);
    let end = meta.headings[ix + 1..]
        .iter()
        .find(|h| h.level <= heading.level)
        .map(|h| line_start(h.position.start.offset))
        .unwrap_or(text.len());
    let mut stop = end;
    if text[start..stop].ends_with('\n') {
        stop -= 1;
        if text[start..stop].ends_with('\r') {
            stop -= 1;
        }
    }
    Some(HeadingSection {
        range: start..stop,
        name: heading.heading.clone(),
    })
}

// ------------------------------------------------------------------- plans

/// Every note whose links to `source` a merge into `target` rewrites, as the
/// rename planner would rewrite them for a rename of `source` to `target`.
/// The source's own rewrite is among them: Obsidian counts its links in the
/// "Update links" question ("11 links in 3 files" with the source the third)
/// though the merged text keeps them as written.
pub fn link_rewrites(
    cache: &MetadataCache,
    source: &str,
    target: &str,
    format: LinkFormat,
    texts: &dyn Fn(&str) -> Option<String>,
) -> Vec<Rewrite> {
    cache.plan_rename_rewrites(&[(source.to_string(), target.to_string())], format, texts)
}

/// `(files, links)` the "Update links" question and the Notice count.
pub fn counts(rewrites: &[Rewrite]) -> (usize, usize) {
    (rewrites.len(), rewrites.iter().map(|r| r.links).sum())
}

/// What one merge writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePlan {
    /// The target's new bytes.
    pub target: String,
    /// The other notes whose links to the source now point at the target,
    /// with their new bytes. The source's own rewrite is not here: its text
    /// went into the target as written.
    pub others: Vec<Rewrite>,
}

/// The merge of `source` into `target`.
///
/// `rewrites` is [`link_rewrites`] when the links are to be updated and
/// empty when not; the target's own rewrite, if any, is applied before the
/// source's text joins it, which is why the merged-in self-link
/// `[[l-src#H]]` stayed as written while the target's `[[l-src]]` became
/// `[[l-dst]]`. `target_text` is empty for the note Ctrl+Enter makes.
pub fn plan_merge(
    source: &str,
    source_text: &str,
    target: &str,
    target_text: &str,
    place: Placement,
    template: Option<(&str, Stamp)>,
    rewrites: &[Rewrite],
) -> MergePlan {
    let base = rewrites
        .iter()
        .find(|r| r.old_path == target)
        .map(|r| r.text.as_str())
        .unwrap_or(target_text);
    let wrap = |body: &str| match template {
        Some((tpl, at)) => expand_template(
            tpl,
            body,
            &TemplateVars {
                from_title: basename(source),
                new_title: basename(target),
                at,
            },
        ),
        None => body.to_string(),
    };
    MergePlan {
        target: merge_notes(base, source_text, place, &wrap),
        others: rewrites
            .iter()
            .filter(|r| r.old_path != source && r.old_path != target)
            .cloned()
            .collect(),
    }
}

/// What one extraction writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractPlan {
    /// The range of the source to replace, and what goes there.
    pub range: Range<usize>,
    pub replacement: String,
    /// The note the text goes into, and its new bytes.
    pub target: String,
    pub target_text: String,
}

impl ExtractPlan {
    /// The source with the replacement made.
    pub fn applied(&self, source_text: &str) -> String {
        let mut out = source_text.to_string();
        out.replace_range(self.range.clone(), &self.replacement);
        out
    }

    /// Where the caret is left: after the replacement.
    pub fn caret(&self) -> usize {
        self.range.start + self.replacement.len()
    }
}

/// The note settings an extraction reads.
#[derive(Debug, Clone, Copy)]
pub struct ExtractOptions<'a> {
    pub replacement: Replacement,
    pub link: scoria_suggest::Options,
    /// The template's text, if `template` names one, and the instant its
    /// dates read.
    pub template: Option<(&'a str, Stamp)>,
}

/// Moves `range` of `source_text` (the note at `source`) into `target`:
/// `target_text` is `None` for a note the extraction creates, which then
/// holds the text (through the template) and nothing else, and the note's
/// current bytes otherwise, which the text joins by [`merge_notes`]'
/// rules. `resolver` must already know `target`, so that the link left
/// behind is the shortest one that reaches it.
#[allow(clippy::too_many_arguments)]
pub fn plan_extract(
    source: &str,
    source_text: &str,
    range: Range<usize>,
    target: &str,
    target_text: Option<&str>,
    place: Placement,
    resolver: &LinkResolver,
    opts: &ExtractOptions<'_>,
) -> ExtractPlan {
    let taken = &source_text[range.clone()];
    let content = match opts.template {
        Some((tpl, at)) => expand_template(
            tpl,
            taken,
            &TemplateVars {
                from_title: basename(source),
                new_title: basename(target),
                at,
            },
        ),
        None => taken.to_string(),
    };
    let target_text = merge_notes(target_text.unwrap_or(""), &content, place, &|b| {
        b.to_string()
    });
    let replacement = match opts.replacement {
        Replacement::None => String::new(),
        kind => scoria_suggest::file_link(
            resolver,
            target,
            source,
            kind == Replacement::Embed,
            &opts.link,
        ),
    };
    ExtractPlan {
        range,
        replacement,
        target: target.to_string(),
        target_text,
    }
}

#[cfg(test)]
mod tests;
