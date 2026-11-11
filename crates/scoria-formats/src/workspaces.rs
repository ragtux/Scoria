//! `.obsidian/workspaces.json`, the Workspaces core plugin's saved layouts.
//!
//! Observed with Obsidian 1.15.0 (`tools/obsidian-probe/results/2026-11-09-panes-workspaces-headings.json`,
//! key `ws`): the file is absent until the first save, and then is
//! `{"workspaces": {<name>: <layout>, ...}, "active": <name>}`, two-space
//! indented with no trailing newline. A layout is `workspace.json` as it
//! stood at the save with `lastOpenFiles` left out and `mtime` appended —
//! local time with its offset, to the second (`2026-11-09T20:05:44-06:00`).
//! Layouts keep insertion order; saving under a name already there replaces
//! that entry in place; `active` is the active layout's *name*, set by a
//! save and by a load, and left alone by a delete, so deleting the active
//! layout leaves it naming one that no longer exists.
//!
//! A layout is kept as the JSON object it was read as, so one Scoria did
//! not save is written back key for key, unknown keys included.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The key `workspace.json` has and a saved layout does not.
pub const LAST_OPEN_FILES: &str = "lastOpenFiles";

/// The whole file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Workspaces {
    /// The saved layouts by name, in the order they were first saved.
    #[serde(default)]
    pub workspaces: IndexMap<String, WorkspaceLayout>,
    /// The name of the layout last saved or loaded; it may name one that
    /// has since been deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One saved layout: `workspace.json`'s keys, then `mtime`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    /// `main`, `left`, `right`, `left-ribbon`, `active` (the active leaf's
    /// id) and whatever else `workspace.json` held, in its order.
    #[serde(flatten)]
    pub layout: Map<String, Value>,
    /// When the layout was saved, as Obsidian writes it: local time with
    /// the offset, to the second. It goes last, where Obsidian appends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime: Option<String>,
}

impl WorkspaceLayout {
    /// The layout a save makes of `workspace` (the `workspace.json` of the
    /// moment): every key but `lastOpenFiles`, in order, and then `mtime`.
    /// Anything that is not an object saves as an empty layout.
    pub fn from_workspace(workspace: &Value, mtime: impl Into<String>) -> WorkspaceLayout {
        let mut layout = workspace.as_object().cloned().unwrap_or_default();
        layout.shift_remove(LAST_OPEN_FILES);
        // A `workspace.json` that somehow carries an `mtime` of its own
        // would otherwise write the key twice.
        layout.shift_remove("mtime");
        WorkspaceLayout {
            layout,
            mtime: Some(mtime.into()),
        }
    }

    /// The layout as a `workspace.json` to restore: every key but `mtime`.
    /// It has no `lastOpenFiles`; a load keeps the session's.
    pub fn to_workspace(&self) -> Value {
        let mut layout = self.layout.clone();
        layout.shift_remove(LAST_OPEN_FILES);
        Value::Object(layout)
    }
}

impl Workspaces {
    /// Parses the file's text; a byte order mark is not JSON and is skipped.
    pub fn parse(text: &str) -> serde_json::Result<Workspaces> {
        serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text))
    }

    /// The file's text as Obsidian writes it: two-space indentation and no
    /// trailing newline.
    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).expect("a JSON map always serializes")
    }

    /// Saves `layout` as `name`: in place when the name is already there,
    /// at the end otherwise, and makes it the active layout.
    pub fn save(&mut self, name: &str, layout: WorkspaceLayout) {
        // `IndexMap::insert` keeps an existing key's position.
        self.workspaces.insert(name.to_string(), layout);
        self.active = Some(name.to_string());
    }

    /// Marks `name` as loaded. A load changes nothing else in the file.
    pub fn set_active(&mut self, name: &str) {
        self.active = Some(name.to_string());
    }

    /// Deletes layout `name`, keeping the others' order. `active` stays as
    /// it was, even when it named this one (observed).
    pub fn delete(&mut self, name: &str) -> bool {
        self.workspaces.shift_remove(name).is_some()
    }

    /// The active layout's name, when it still names a saved layout.
    pub fn active_layout(&self) -> Option<&str> {
        self.active
            .as_deref()
            .filter(|name| self.workspaces.contains_key(*name))
    }
}
