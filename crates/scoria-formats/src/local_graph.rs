//! A `localgraph` leaf's state in `workspace.json`, as Obsidian 1.15.1
//! writes it (`tools/obsidian-probe/results/2026-11-22-local-graph-pane.json`,
//! see [[Specification#Observed local graph pane, Obsidian 1.15.1]]):
//!
//! ```text
//! {"type": "localgraph",
//!  "state": {"file": "Alpha.md", "options": {…}},
//!  "icon": "lucide-git-fork", "title": "Local graph"}
//! ```
//!
//! `file` is absent while the pane has no file. `options` is the leaf's own
//! copy of the graph's settings: `graph.json`'s keys less `showOrphans`, plus
//! `localJumps`, `localBacklinks`, `localForelinks` and `localInterlinks`,
//! in the order [`OPTION_KEYS`] lists. Editing the pane rewrites only the
//! keys whose values changed, so the digits Obsidian wrote and any key this
//! module does not know stay as they were.

use crate::config::GraphConfig;
use serde_json::{json, Map, Value};

/// The leaf's view type.
pub const VIEW_TYPE: &str = "localgraph";
/// The leaf's icon, the global graph's too.
pub const ICON: &str = "lucide-git-fork";
/// The title of a leaf linked to no tab, which is what a sidebar leaf is.
pub const UNLINKED_TITLE: &str = "Local graph";

/// Every key of a fresh leaf's `options`, in the order Obsidian writes them.
pub const OPTION_KEYS: [&str; 23] = [
    "collapse-filter",
    "search",
    "localJumps",
    "localBacklinks",
    "localForelinks",
    "localInterlinks",
    "showTags",
    "showAttachments",
    "hideUnresolved",
    "collapse-color-groups",
    "colorGroups",
    "collapse-display",
    "showArrow",
    "textFadeMultiplier",
    "nodeSizeMultiplier",
    "lineSizeMultiplier",
    "collapse-forces",
    "centerStrength",
    "repelStrength",
    "linkStrength",
    "linkDistance",
    "scale",
    "close",
];

/// The `collapse-*` keys, which [`GraphConfig`] keeps in `extra`.
const COLLAPSE_KEYS: [&str; 4] = [
    "collapse-filter",
    "collapse-color-groups",
    "collapse-display",
    "collapse-forces",
];

/// The four keys only a local graph has.
const LOCAL_KEYS: [&str; 4] = [
    "localJumps",
    "localBacklinks",
    "localForelinks",
    "localInterlinks",
];

/// A leaf's options read into the shapes the graph view works with: the
/// settings it shares with `graph.json` and the four of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalGraphOptions {
    /// Filters, groups, display and forces, `collapse-*` in `extra`.
    pub config: GraphConfig,
    /// `localJumps`, the Depth slider (1 to 5).
    pub jumps: u32,
    /// `localBacklinks`, "Incoming links".
    pub backlinks: bool,
    /// `localForelinks`, "Outgoing links".
    pub forelinks: bool,
    /// `localInterlinks`, "Neighbor links".
    pub interlinks: bool,
}

impl Default for LocalGraphOptions {
    fn default() -> Self {
        LocalGraphOptions::read(&default_options())
    }
}

/// The options block a new leaf gets: the values Obsidian 1.15.1 opened
/// every local graph at, whatever `graph.json` held.
pub fn default_options() -> Map<String, Value> {
    let value = json!({
        "collapse-filter": true,
        "search": "",
        "localJumps": 1,
        "localBacklinks": true,
        "localForelinks": true,
        "localInterlinks": false,
        "showTags": false,
        "showAttachments": false,
        "hideUnresolved": false,
        "collapse-color-groups": true,
        "colorGroups": [],
        "collapse-display": true,
        "showArrow": false,
        "textFadeMultiplier": 0,
        "nodeSizeMultiplier": 1,
        "lineSizeMultiplier": 1,
        "collapse-forces": true,
        "centerStrength": 0.518713248970312,
        "repelStrength": 10,
        "linkStrength": 1,
        "linkDistance": 250,
        "scale": 1,
        "close": false,
    });
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

impl LocalGraphOptions {
    /// Reads a leaf's `options`, each missing key at its default.
    pub fn read(options: &Map<String, Value>) -> LocalGraphOptions {
        let defaults = default_options();
        let mut merged = defaults.clone();
        for (key, value) in options {
            merged.insert(key.clone(), value.clone());
        }
        let mut config: GraphConfig =
            serde_json::from_value(Value::Object(merged.clone())).unwrap_or_default();
        for key in LOCAL_KEYS {
            config.extra.shift_remove(key);
        }
        let flag = |key: &str| {
            merged
                .get(key)
                .and_then(Value::as_bool)
                .or_else(|| defaults.get(key).and_then(Value::as_bool))
                .unwrap_or(false)
        };
        let jumps = merged
            .get("localJumps")
            .and_then(Value::as_f64)
            .map(|j| j.round().clamp(1.0, 5.0) as u32)
            .unwrap_or(1);
        LocalGraphOptions {
            config,
            jumps,
            backlinks: flag("localBacklinks"),
            forelinks: flag("localForelinks"),
            interlinks: flag("localInterlinks"),
        }
    }

    /// The value each of [`OPTION_KEYS`] stands at now.
    fn value_of(&self, key: &str) -> Value {
        let c = &self.config;
        let number = |value: Option<f32>, default: f32| number(value.unwrap_or(default));
        let flag = |value: Option<bool>, default: bool| json!(value.unwrap_or(default));
        match key {
            k if COLLAPSE_KEYS.contains(&k) => {
                json!(c.extra.get(k).and_then(Value::as_bool).unwrap_or(true))
            }
            "search" => json!(c.search.clone().unwrap_or_default()),
            "localJumps" => json!(self.jumps),
            "localBacklinks" => json!(self.backlinks),
            "localForelinks" => json!(self.forelinks),
            "localInterlinks" => json!(self.interlinks),
            "showTags" => flag(c.show_tags, false),
            "showAttachments" => flag(c.show_attachments, false),
            "hideUnresolved" => flag(c.hide_unresolved, false),
            "colorGroups" => {
                let mut groups = json!(c.color_groups);
                integerize(&mut groups);
                groups
            }
            "showArrow" => flag(c.show_arrow, false),
            "textFadeMultiplier" => number(c.text_fade_multiplier, 0.0),
            "nodeSizeMultiplier" => number(c.node_size_multiplier, 1.0),
            "lineSizeMultiplier" => number(c.line_size_multiplier, 1.0),
            "centerStrength" => match c.center_strength {
                Some(v) => self::number(v),
                None => json!(0.518713248970312),
            },
            "repelStrength" => number(c.repel_strength, 10.0),
            "linkStrength" => number(c.link_strength, 1.0),
            "linkDistance" => number(c.link_distance, 250.0),
            "scale" => number(c.scale, 1.0),
            "close" => flag(c.close, false),
            _ => Value::Null,
        }
    }

    /// Writes these options into a leaf's `options`: a key whose value is
    /// unchanged keeps its bytes (`0.518713248970312` read as an `f32` is
    /// still those digits), a changed one is replaced in place, a missing
    /// one goes in after the key Obsidian writes before it, and a key not
    /// in [`OPTION_KEYS`] is left alone.
    pub fn write_into(&self, options: &mut Map<String, Value>) {
        for (ix, key) in OPTION_KEYS.iter().enumerate() {
            let want = self.value_of(key);
            match options.get_mut(*key) {
                Some(have) => {
                    if !same(have, &want) {
                        *have = want;
                    }
                }
                None => {
                    let at = OPTION_KEYS[..ix]
                        .iter()
                        .rev()
                        .find_map(|before| options.keys().position(|k| k == before))
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    options.shift_insert(at, (*key).to_string(), want);
                }
            }
        }
    }
}

/// A leaf's inner `state`: `file` when there is one, then `options`. A
/// `state` read from the file is kept and only these two keys rewritten.
pub fn write_state(
    state: &mut Map<String, Value>,
    file: Option<&str>,
    options: &LocalGraphOptions,
) {
    match file {
        Some(file) => {
            if state.get("file").and_then(Value::as_str) != Some(file) {
                if state.contains_key("file") {
                    state.insert("file".into(), json!(file));
                } else {
                    state.shift_insert(0, "file".into(), json!(file));
                }
            }
        }
        None => {
            state.shift_remove("file");
        }
    }
    let slot = state
        .entry("options")
        .or_insert_with(|| Value::Object(default_options()));
    if !slot.is_object() {
        *slot = Value::Object(default_options());
    }
    if let Some(map) = slot.as_object_mut() {
        options.write_into(map);
    }
}

/// The options a leaf's inner `state` holds, read; the defaults when it
/// has none.
pub fn read_state(state: Option<&Value>) -> LocalGraphOptions {
    state
        .and_then(|s| s.get("options"))
        .and_then(Value::as_object)
        .map(LocalGraphOptions::read)
        .unwrap_or_default()
}

/// A new unlinked leaf, as the sidebar leaf Obsidian wrote after a tab was
/// dragged there: `id`, `type`, and its `state` with the icon and title.
pub fn new_leaf(id: &str, file: Option<&str>, options: &LocalGraphOptions) -> Value {
    let mut state = Map::new();
    write_state(&mut state, file, options);
    json!({
        "id": id,
        "type": "leaf",
        "state": {
            "type": VIEW_TYPE,
            "state": Value::Object(state),
            "icon": ICON,
            "title": UNLINKED_TITLE,
        },
    })
}

/// Two values the same for the file's purposes: numbers equal at the `f32`
/// precision the view keeps them in, everything else equal outright.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => x as f32 == y as f32,
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

/// A number as Obsidian writes one: whole numbers without a fraction, the
/// rest at the shortest digits that read back as the same `f32`.
fn number(value: f32) -> Value {
    if value.fract() == 0.0 && value.abs() < 1e9 {
        json!(value as i64)
    } else {
        value
            .to_string()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// Every number under `value` written as [`number`] writes one.
fn integerize(value: &mut Value) {
    match value {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                *value = number(f as f32);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(integerize),
        Value::Object(fields) => fields.values_mut().for_each(integerize),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The leaf Obsidian 1.15.1 wrote for the sidebar pane after the probe
    /// clicked Neighbor links and Incoming links and dragged Depth to 3
    /// (`s08`), with an unknown key added by hand.
    const EDITED: &str = r#"{
  "collapse-filter": false,
  "search": "",
  "localJumps": 3,
  "localBacklinks": false,
  "localForelinks": true,
  "localInterlinks": true,
  "showTags": false,
  "showAttachments": false,
  "hideUnresolved": false,
  "collapse-color-groups": true,
  "colorGroups": [],
  "collapse-display": true,
  "showArrow": false,
  "textFadeMultiplier": 0,
  "nodeSizeMultiplier": 1,
  "lineSizeMultiplier": 1,
  "collapse-forces": true,
  "centerStrength": 0.518713248970312,
  "repelStrength": 10,
  "linkStrength": 1,
  "linkDistance": 250,
  "scale": 1,
  "close": false,
  "scoriaUnknown": [1, 2]
}"#;

    fn map(text: &str) -> Map<String, Value> {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn the_default_block_is_obsidians_in_its_order() {
        let defaults = default_options();
        let keys: Vec<&str> = defaults.keys().map(String::as_str).collect();
        assert_eq!(keys, OPTION_KEYS);
        assert_eq!(
            serde_json::to_string(&default_options()).unwrap(),
            r#"{"collapse-filter":true,"search":"","localJumps":1,"localBacklinks":true,"localForelinks":true,"localInterlinks":false,"showTags":false,"showAttachments":false,"hideUnresolved":false,"collapse-color-groups":true,"colorGroups":[],"collapse-display":true,"showArrow":false,"textFadeMultiplier":0,"nodeSizeMultiplier":1,"lineSizeMultiplier":1,"collapse-forces":true,"centerStrength":0.518713248970312,"repelStrength":10,"linkStrength":1,"linkDistance":250,"scale":1,"close":false}"#
        );
    }

    #[test]
    fn reads_the_four_local_keys_and_the_shared_ones() {
        let o = LocalGraphOptions::read(&map(EDITED));
        assert_eq!(
            (o.jumps, o.backlinks, o.forelinks, o.interlinks),
            (3, false, true, true)
        );
        assert_eq!(o.config.extra.get("collapse-filter"), Some(&json!(false)));
        assert!(!o.config.extra.contains_key("localJumps"));
        assert_eq!(o.config.repel_strength, Some(10.0));
        let d = LocalGraphOptions::default();
        assert_eq!(
            (d.jumps, d.backlinks, d.forelinks, d.interlinks),
            (1, true, true, false)
        );
    }

    #[test]
    fn writing_back_unchanged_options_changes_no_byte() {
        let mut m = map(EDITED);
        let before = serde_json::to_string_pretty(&m).unwrap();
        LocalGraphOptions::read(&m).write_into(&mut m);
        assert_eq!(serde_json::to_string_pretty(&m).unwrap(), before);
        assert!(before.contains("0.518713248970312"));
        assert!(before.contains("\"scoriaUnknown\""));
    }

    #[test]
    fn an_edit_rewrites_its_key_in_place() {
        let mut m = default_options();
        let mut o = LocalGraphOptions::read(&m);
        o.jumps = 3;
        o.backlinks = false;
        o.interlinks = true;
        o.config
            .extra
            .insert("collapse-filter".into(), json!(false));
        o.config.link_distance = Some(300.0);
        o.config.center_strength = Some(0.52);
        o.write_into(&mut m);
        let keys: Vec<&str> = m.keys().map(String::as_str).collect();
        assert_eq!(keys, OPTION_KEYS);
        assert_eq!(m["localJumps"], json!(3));
        assert_eq!(m["localBacklinks"], json!(false));
        assert_eq!(m["localInterlinks"], json!(true));
        assert_eq!(m["collapse-filter"], json!(false));
        assert_eq!(m["linkDistance"], json!(300));
        assert_eq!(serde_json::to_string(&m["centerStrength"]).unwrap(), "0.52");
    }

    #[test]
    fn a_sparse_block_gains_the_missing_keys_in_order() {
        let mut m = map(r#"{"localJumps": 2, "scoriaUnknown": true, "close": true}"#);
        let o = LocalGraphOptions::read(&m);
        assert_eq!(o.jumps, 2);
        assert_eq!(o.config.close, Some(true));
        o.write_into(&mut m);
        let keys: Vec<&str> = m.keys().map(String::as_str).collect();
        let known: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|k| *k != "scoriaUnknown")
            .collect();
        assert_eq!(known, OPTION_KEYS);
        assert_eq!(m["scoriaUnknown"], json!(true));
        assert_eq!(m["close"], json!(true));
    }

    #[test]
    fn the_state_drops_file_with_none_and_keeps_unknown_keys() {
        let o = LocalGraphOptions::default();
        let mut state = map(r#"{"file": "Alpha.md", "options": {}, "scoriaState": 1}"#);
        write_state(&mut state, Some("Beta.md"), &o);
        assert_eq!(state["file"], json!("Beta.md"));
        assert_eq!(state["scoriaState"], json!(1));
        write_state(&mut state, None, &o);
        assert!(!state.contains_key("file"));
        write_state(&mut state, Some("Gamma.md"), &o);
        assert_eq!(state.keys().next().map(String::as_str), Some("file"));
        let options = state["options"].as_object().unwrap();
        assert_eq!(options.len(), OPTION_KEYS.len());
    }

    #[test]
    fn a_new_leaf_is_the_sidebar_leaf_obsidian_wrote() {
        let leaf = new_leaf(
            "de8440d3ce8d3997",
            Some("Alpha.md"),
            &LocalGraphOptions::default(),
        );
        let observed: Value = serde_json::from_str(&format!(
            r#"{{"id": "de8440d3ce8d3997", "type": "leaf", "state": {{"type": "localgraph",
              "state": {{"file": "Alpha.md", "options": {}}},
              "icon": "lucide-git-fork", "title": "Local graph"}}}}"#,
            serde_json::to_string(&default_options()).unwrap()
        ))
        .unwrap();
        assert_eq!(
            serde_json::to_string(&leaf).unwrap(),
            serde_json::to_string(&observed).unwrap()
        );
        let read = read_state(leaf.pointer("/state/state"));
        assert_eq!(read, LocalGraphOptions::default());
        // No file: `state` is the options alone.
        let empty = new_leaf("x", None, &LocalGraphOptions::default());
        let inner = empty
            .pointer("/state/state")
            .and_then(Value::as_object)
            .unwrap();
        assert_eq!(inner.keys().collect::<Vec<_>>(), vec!["options"]);
    }
}
