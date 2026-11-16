//! Undo and redo of a Bases table's cell edits: a paste, a Delete, a typed
//! cell, a checkbox. Observed on Obsidian 1.15.1
//! ([[Specification#Observed Bases cell undo, Obsidian 1.15.1]]).
//!
//! The table keeps its own history, apart from any note's: one step per
//! action however many notes it wrote, Ctrl+Z and Ctrl+Shift+Z (Ctrl+Y is
//! not redo there) while the table itself has focus, the step's selection
//! put back with it, a new action dropping the redo side, and the history
//! gone when the base is closed or the leaf shows another file.
//!
//! A step holds each note's bytes before and after the write. Taking it back
//! writes the note only if it is still what the step left: the same bytes,
//! put back exactly (where Obsidian sets the old values over the new ones and
//! so leaves an added key behind as `key:`), or else the same frontmatter
//! values under a body edited since, where only the frontmatter block is put
//! back and the body edit stays (observed). A note whose frontmatter changed
//! since is left alone, silently, and the step is spent all the same
//! (observed: no Notice, and the redo does nothing either).

use scoria_bases::selection::Selection;
use scoria_formats::properties::Value;
use std::ops::Range;

/// One note an action wrote: its bytes either side of the write.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteChange {
    pub path: String,
    pub before: String,
    pub after: String,
}

/// One action: every note it wrote, and the table's selection after it,
/// which an undo and a redo both put back (observed: Obsidian keeps the
/// view's selection and cursor with each step and restores them).
#[derive(Debug, Clone)]
pub struct Step {
    pub changes: Vec<NoteChange>,
    pub selection: Option<Selection>,
}

/// The table's two stacks.
#[derive(Debug, Default)]
pub struct CellHistory {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

impl CellHistory {
    /// A new action: on the undo side, and the redo side dropped (observed:
    /// a paste after an undo left Ctrl+Shift+Z with nothing to do). An
    /// action that wrote nothing is no step.
    pub fn record(&mut self, step: Step) {
        if step.changes.is_empty() {
            return;
        }
        self.undo.push(step);
        self.redo.clear();
    }

    /// The step Ctrl+Z takes back, moved to the redo side whether or not
    /// its notes can still be written.
    pub fn undo(&mut self) -> Option<Step> {
        let step = self.undo.pop()?;
        self.redo.push(step.clone());
        Some(step)
    }

    /// The step Ctrl+Shift+Z puts back, moved to the undo side.
    pub fn redo(&mut self) -> Option<Step> {
        let step = self.redo.pop()?;
        self.undo.push(step.clone());
        Some(step)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// How many steps each side holds, for the control socket and tests.
    pub fn depth(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
}

/// What taking one note back from `from` to `to` writes, given the bytes on
/// disk now: `None` when the note is not what the step left.
pub fn revert(on_disk: &str, from: &str, to: &str) -> Option<String> {
    if on_disk == from {
        return Some(to.to_string());
    }
    if values(on_disk) != values(from) {
        return None;
    }
    // The body moved on but the frontmatter is the step's: the block alone
    // goes back. A note the step gave a block to loses it again, and one it
    // took a block from gets the block back above the body.
    let disk_block = block(on_disk);
    let to_block = block(to);
    let mut out = String::with_capacity(on_disk.len() + to_block.len());
    out.push_str(&on_disk[..disk_block.start]);
    out.push_str(&to[to_block]);
    out.push_str(&on_disk[disk_block.end..]);
    (out != on_disk).then_some(out)
}

/// The frontmatter as keys and values in file order: what Obsidian compares
/// (as JSON) before it takes a step back. No block is no values.
fn values(text: &str) -> Vec<(String, Value)> {
    scoria_cache::note_properties(text)
        .map(|p| p.entries.into_iter().map(|e| (e.key, e.value)).collect())
        .unwrap_or_default()
}

/// The frontmatter block's bytes, fences and the newline after the closing
/// one included; with no block, the empty span where one would go (after a
/// BOM).
fn block(text: &str) -> Range<usize> {
    scoria_cache::note_properties(text)
        .map(|p| p.block)
        .unwrap_or_else(|| {
            let at = if text.starts_with('\u{feff}') { 3 } else { 0 };
            at..at
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "---\nstatus: todo\nprice: 1.5\nnote: plain\n---\nAlpha body.\n";
    const PASTED: &str = "---\nstatus: Q\nprice: 1.5\nnote: plain\n---\nAlpha body.\n";

    fn change(before: &str, after: &str) -> NoteChange {
        NoteChange {
            path: "cells/Alpha.md".into(),
            before: before.into(),
            after: after.into(),
        }
    }

    fn step(n: usize) -> Step {
        Step {
            changes: vec![change(&format!("{n}"), &format!("{n}'"))],
            selection: Some(Selection::at((n, 1))),
        }
    }

    /// `u01`, `u05`: one step per action; Ctrl+Z three times takes three
    /// actions back in reverse, a fourth does nothing, and redo walks them
    /// forward again.
    #[test]
    fn each_action_is_one_step_undone_in_reverse_and_redone_in_order() {
        let mut h = CellHistory::default();
        for n in 1..=3 {
            h.record(step(n));
        }
        let undone: Vec<usize> = std::iter::from_fn(|| h.undo())
            .map(|s| s.selection.unwrap().cursor.0)
            .collect();
        assert_eq!(undone, [3, 2, 1]);
        assert!(h.undo().is_none());
        assert_eq!(h.depth(), (0, 3));
        let redone: Vec<usize> = std::iter::from_fn(|| h.redo())
            .map(|s| s.selection.unwrap().cursor.0)
            .collect();
        assert_eq!(redone, [1, 2, 3]);
        assert_eq!(h.depth(), (3, 0));
    }

    /// `u10`: a new action after an undo drops what could have been redone.
    #[test]
    fn a_new_action_after_an_undo_drops_the_redo_side() {
        let mut h = CellHistory::default();
        h.record(step(1));
        h.record(step(2));
        h.undo();
        h.record(step(3));
        assert!(h.redo().is_none());
        assert_eq!(h.undo().unwrap().selection.unwrap().cursor.0, 3);
        assert_eq!(h.undo().unwrap().selection.unwrap().cursor.0, 1);
    }

    #[test]
    fn an_action_that_wrote_nothing_is_no_step() {
        let mut h = CellHistory::default();
        h.record(step(1));
        h.undo();
        h.record(Step {
            changes: vec![],
            selection: None,
        });
        assert_eq!(h.depth(), (0, 1), "nor does it drop the redo side");
    }

    #[test]
    fn an_untouched_note_gets_its_bytes_back_exactly() {
        assert_eq!(revert(PASTED, PASTED, SEED).as_deref(), Some(SEED));
        assert_eq!(revert(SEED, SEED, PASTED).as_deref(), Some(PASTED));
        // A key the Delete added and a block it created go again, which
        // Obsidian's value-wise undo leaves behind (`u02`: Charlie kept
        // `note:`, Echo its block).
        let echo = "Echo has no frontmatter.\n";
        let deleted = "---\nnote:\n---\nEcho has no frontmatter.\n";
        assert_eq!(revert(deleted, deleted, echo).as_deref(), Some(echo));
        // A quoted value the step emptied comes back quoted, where
        // Obsidian re-renders it bare.
        let delta = "---\nstatus: \"with\\ttab\"\nnote: \"multi word, comma\"\n---\nD\n";
        let emptied = "---\nstatus: \"with\\ttab\"\nnote:\n---\nD\n";
        assert_eq!(revert(emptied, emptied, delta).as_deref(), Some(delta));
    }

    /// `u07` `bodyOnly`: the body edited from outside after the paste, the
    /// frontmatter as the paste left it. The undo puts `status: todo` back
    /// and keeps the edit, and the redo writes `Q` again over it.
    #[test]
    fn a_body_edited_since_keeps_its_edit_under_the_frontmatter_put_back() {
        let edited = PASTED.replace("Alpha body.", "Alpha body, edited.");
        let undone = revert(&edited, PASTED, SEED).unwrap();
        assert_eq!(undone, SEED.replace("Alpha body.", "Alpha body, edited."));
        let redone = revert(&undone, SEED, PASTED).unwrap();
        assert_eq!(redone, edited);
        // A block the step created goes, the body edit stays.
        let deleted = "---\nnote:\n---\nEcho, edited.\n";
        assert_eq!(
            revert(deleted, "---\nnote:\n---\nEcho.\n", "Echo.\n").as_deref(),
            Some("Echo, edited.\n")
        );
    }

    /// `u07`: another key changed, the pasted value overwritten, or the
    /// value re-quoted from outside (`"P3"` for `P3`, the same string, which
    /// Obsidian's JSON comparison still took back): Obsidian compares
    /// values, so only the first two are refused.
    #[test]
    fn a_frontmatter_changed_since_is_left_alone() {
        let other_key = PASTED.replace("note: plain", "note: changed");
        assert_eq!(revert(&other_key, PASTED, SEED), None);
        let overwritten = PASTED.replace("status: Q", "status: outside");
        assert_eq!(revert(&overwritten, PASTED, SEED), None);
        let requoted = PASTED.replace("status: Q", "status: \"Q\"");
        assert_eq!(revert(&requoted, PASTED, SEED).as_deref(), Some(SEED));
    }

    #[test]
    fn a_bom_and_crlf_note_keeps_both() {
        let before = "\u{feff}---\r\na: 1\r\n---\r\nbody\r\n";
        let after = "\u{feff}---\r\na: 2\r\n---\r\nbody\r\n";
        let edited = after.replace("body", "body, edited");
        assert_eq!(
            revert(&edited, after, before).as_deref(),
            Some(before.replace("body", "body, edited").as_str())
        );
    }
}
