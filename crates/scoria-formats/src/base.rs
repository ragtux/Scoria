//! Bases (`.base`) YAML files.
//!
//! Modelled on what Obsidian 1.14.5 was observed to do rather than on the
//! published syntax, which is wrong in three places that matter: see
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]].
//!
//! A `.base` is **canonicalized, not preserved**. Obsidian rewrites the whole
//! file from its parsed model on the close that follows an open, losing
//! comments, quoting, flow style, CRLF and a BOM. So this model's job is not to
//! remember the file's own shape, as [`crate::canvas`] does; it is to hold
//! exactly what Obsidian's own serializer holds, in the order that serializer
//! emits, so the bytes can be reproduced from the model alone.
//!
//! Statement strings are kept verbatim and this module never parses one. That
//! is not caution: Obsidian keeps the original text beside the parse tree and
//! writes the text back, down to a double space inside a string literal, so
//! verbatim *is* what matches. The expression language is parsed elsewhere, by
//! a crate this one must never depend on.
//!
//! There is deliberately no writer here yet. The emitter has a quoting rule and
//! a key order of its own, both observed and neither reachable through
//! `serde_yaml::to_string`, and a writer that does not match Obsidian byte for
//! byte would be worse than none: the vault is the source of truth.

use indexmap::IndexMap;
use serde_yaml::Value;
use std::fmt;

/// The view Obsidian creates for a base that has none — a zero-byte file, or
/// `{}`. Observed, not published.
pub const DEFAULT_VIEW_TYPE: &str = "table";
/// The name that view is given.
pub const DEFAULT_VIEW_NAME: &str = "View";

/// Top-level keys the parser consumes, and so removes from [`Base::preserved`].
///
/// `summaries` is deliberately not here. Obsidian parses it into its summary
/// formulas *and* leaves it among the unrecognized keys, which is why it is
/// written back at the position the file gave it rather than the serializer's.
const CONSUMED: &[&str] = &[
    "filters",
    "formulas",
    "properties",
    "views",
    "newItemFolder",
    "newItemTemplate",
];

/// A parsed `.base`.
///
/// Field order here is the serializer's own, which is also [`Base::key_order`]'s
/// fallback: `filters`, `formulas`, `summaries`, `properties`, `views`,
/// `newItemFolder`, `newItemTemplate`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Base {
    /// Every top-level key the parser did not consume, in the file's own order,
    /// plus `summaries` when the file had it. Obsidian spreads this map first,
    /// so a key in it keeps its file position even when it is also a key the
    /// serializer would otherwise place.
    pub preserved: IndexMap<String, Value>,
    pub filters: Option<Filter>,
    /// Name to statement, in file order.
    pub formulas: IndexMap<String, String>,
    /// Custom summary name to statement, in file order. Present only when the
    /// file had the key: an absent `summaries` is not written back as empty.
    pub summaries: Option<IndexMap<String, String>>,
    /// Property id to its config, in file order. A config that is empty is
    /// dropped on write, and when every config is empty the whole key goes.
    pub properties: IndexMap<String, PropertyConfig>,
    pub views: Vec<View>,
    pub new_item_folder: Option<String>,
    pub new_item_template: Option<String>,
}

/// Why a `.base` did not parse. Obsidian shows each of these in the view, under
/// the banner `Unable to parse your base file: `, and in every one of these
/// cases it leaves the file's bytes and its mtime alone.
#[derive(Debug)]
pub enum BaseError {
    Yaml(serde_yaml::Error),
    /// The document is not a mapping.
    NotAMapping,
    /// `views` is present but is not a sequence.
    ViewsNotASequence,
    /// A view is not a mapping. The index is 1-based, as Obsidian counts.
    ViewNotAMapping(usize),
    /// A view has no `name`, or an empty one. Obsidian requires a non-empty
    /// name on every view; the published schema does not say so.
    ViewName(usize),
    /// A filter mapping carries something other than exactly one of `and`,
    /// `or` and `not`. This is the one place unknown keys are not tolerated.
    FilterConjunction,
}

/// What Obsidian puts before a [`BaseError`] in the view.
pub const PARSE_ERROR_BANNER: &str = "Unable to parse your base file: ";

impl fmt::Display for BaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseError::Yaml(e) => write!(f, "{e}"),
            BaseError::NotAMapping => write!(f, "a base must be a mapping"),
            BaseError::ViewsNotASequence => write!(f, "\"views\" must be a list"),
            BaseError::ViewNotAMapping(n) => write!(f, "view {n} must be a mapping"),
            BaseError::ViewName(n) => write!(f, "Missing or invalid \"name\" in view {n}"),
            BaseError::FilterConjunction => write!(
                f,
                "\"filters\" may only have one of an \"and\", \"or\", or \"not\" keys."
            ),
        }
    }
}

impl std::error::Error for BaseError {}

impl From<serde_yaml::Error> for BaseError {
    fn from(e: serde_yaml::Error) -> Self {
        BaseError::Yaml(e)
    }
}

/// A filter is a statement string, or a mapping carrying exactly one of `and`,
/// `or` and `not` over a list of filters.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Statement(String),
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Vec<Filter>),
}

/// A property's configuration.
///
/// The unknown keys come first on write: `unknownPropKey` was written before
/// `displayName` even though the fixture had it second.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropertyConfig {
    pub preserved: IndexMap<String, Value>,
    pub display_name: Option<String>,
}

impl PropertyConfig {
    /// Whether Obsidian drops this config. Its `serialize()` returns null for an
    /// empty config, and a null is left out of the written `properties` map —
    /// so `note.unused: {}` disappears, and a `properties` whose configs are all
    /// empty loses the whole key.
    pub fn is_dropped(&self) -> bool {
        self.preserved.is_empty() && self.display_name.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupBy {
    pub property: String,
    pub direction: Option<String>,
}

/// One entry of a view's `sort`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sort {
    pub property: String,
    pub direction: Option<String>,
}

/// A view's keys in the order Obsidian writes them.
const VIEW_FIELDS: &[&str] = &[
    "type",
    "name",
    "filters",
    "groupBy",
    "groupOrder",
    "order",
    "sort",
    "limit",
    "summaries",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// `table`, `cards`, `list`, `kanban`, `map`, or a plugin's own. An
    /// unknown type is not a parse error: the file parses, the type is kept,
    /// and the view says "Unknown view type: ..." instead of showing rows.
    pub kind: String,
    /// Required and non-empty. Obsidian refuses the whole file otherwise.
    pub name: String,
    pub filters: Option<Filter>,
    pub group_by: Option<GroupBy>,
    /// Values rather than strings: `groupOrder` may carry a `null`, which means
    /// the files with no value for the grouped property.
    pub group_order: Vec<Value>,
    pub order: Vec<String>,
    pub sort: Vec<Sort>,
    pub limit: Option<u64>,
    /// Property id to summary name, in file order.
    pub summaries: IndexMap<String, String>,
    /// Every key of the view that is not one of [`VIEW_FIELDS`], in file order.
    ///
    /// Unlike the top level, these are appended **last**, which is where every
    /// layout key lives too (`rowHeight`, `columnSize`, `cardSize`, `image`,
    /// ...). So a layout key the UI wrote lands after a plugin's unknown key
    /// rather than among the documented ones.
    pub preserved: IndexMap<String, Value>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn as_str(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.to_string())
}

fn string_list(v: Option<&Value>) -> Vec<String> {
    match v.and_then(|v| v.as_sequence()) {
        Some(items) => items.iter().filter_map(as_str).collect(),
        None => Vec::new(),
    }
}

fn string_map(v: Option<&Value>) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    if let Some(m) = v.and_then(|v| v.as_mapping()) {
        for (k, val) in m {
            if let (Some(k), Some(val)) = (k.as_str(), as_str(val)) {
                out.insert(k.to_string(), val);
            }
        }
    }
    out
}

impl Filter {
    fn parse(v: &Value) -> Result<Filter, BaseError> {
        if let Some(s) = v.as_str() {
            return Ok(Filter::Statement(s.to_string()));
        }
        let Some(m) = v.as_mapping() else {
            return Err(BaseError::FilterConjunction);
        };
        // Exactly one of the three, and nothing beside it: a stray key here is
        // the one unknown key Obsidian refuses.
        if m.len() != 1 {
            return Err(BaseError::FilterConjunction);
        }
        let (key, body) = m.iter().next().ok_or(BaseError::FilterConjunction)?;
        let key = key.as_str().ok_or(BaseError::FilterConjunction)?;
        let items = body
            .as_sequence()
            .ok_or(BaseError::FilterConjunction)?
            .iter()
            .map(Filter::parse)
            .collect::<Result<Vec<_>, _>>()?;
        match key {
            "and" => Ok(Filter::And(items)),
            "or" => Ok(Filter::Or(items)),
            "not" => Ok(Filter::Not(items)),
            _ => Err(BaseError::FilterConjunction),
        }
    }
}

impl PropertyConfig {
    fn parse(v: &Value) -> PropertyConfig {
        let mut cfg = PropertyConfig::default();
        if let Some(m) = v.as_mapping() {
            for (k, val) in m {
                match k.as_str() {
                    Some("displayName") => cfg.display_name = as_str(val),
                    Some(k) => {
                        cfg.preserved.insert(k.to_string(), val.clone());
                    }
                    None => {}
                }
            }
        }
        cfg
    }
}

impl View {
    fn parse(v: &Value, ix: usize) -> Result<View, BaseError> {
        let m = v.as_mapping().ok_or(BaseError::ViewNotAMapping(ix))?;
        let get = |k: &str| m.get(Value::String(k.to_string()));

        let name = get("name").and_then(as_str).unwrap_or_default();
        if name.is_empty() {
            return Err(BaseError::ViewName(ix));
        }

        let group_by = get("groupBy").and_then(|v| {
            let m = v.as_mapping()?;
            Some(GroupBy {
                property: m
                    .get(Value::String("property".into()))
                    .and_then(as_str)
                    .unwrap_or_default(),
                direction: m.get(Value::String("direction".into())).and_then(as_str),
            })
        });

        let sort = get("sort")
            .and_then(|v| v.as_sequence())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let m = item.as_mapping()?;
                        Some(Sort {
                            property: m
                                .get(Value::String("property".into()))
                                .and_then(as_str)
                                .unwrap_or_default(),
                            direction: m.get(Value::String("direction".into())).and_then(as_str),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut preserved = IndexMap::new();
        for (k, val) in m {
            if let Some(k) = k.as_str() {
                if !VIEW_FIELDS.contains(&k) {
                    preserved.insert(k.to_string(), val.clone());
                }
            }
        }

        Ok(View {
            kind: get("type").and_then(as_str).unwrap_or_default(),
            name,
            filters: get("filters").map(Filter::parse).transpose()?,
            group_by,
            group_order: get("groupOrder")
                .and_then(|v| v.as_sequence())
                .cloned()
                .unwrap_or_default(),
            order: string_list(get("order")),
            sort,
            limit: get("limit").and_then(|v| v.as_u64()),
            summaries: string_map(get("summaries")),
            preserved,
        })
    }
}

impl Base {
    /// Parses a `.base`. An empty document is an empty base, which is what a
    /// zero-byte file from `bases:new-file` is.
    pub fn from_yaml(text: &str) -> Result<Base, BaseError> {
        let value: Value = serde_yaml::from_str(text)?;
        if value.is_null() {
            return Ok(Base::default());
        }
        let m = value.as_mapping().ok_or(BaseError::NotAMapping)?;
        let get = |k: &str| m.get(Value::String(k.to_string()));

        let views = match get("views") {
            None => Vec::new(),
            Some(v) => v
                .as_sequence()
                .ok_or(BaseError::ViewsNotASequence)?
                .iter()
                .enumerate()
                .map(|(i, v)| View::parse(v, i + 1))
                .collect::<Result<Vec<_>, _>>()?,
        };

        let mut preserved = IndexMap::new();
        for (k, val) in m {
            if let Some(k) = k.as_str() {
                if !CONSUMED.contains(&k) {
                    preserved.insert(k.to_string(), val.clone());
                }
            }
        }

        Ok(Base {
            filters: get("filters").map(Filter::parse).transpose()?,
            formulas: string_map(get("formulas")),
            summaries: get("summaries").map(|v| string_map(Some(v))),
            properties: get("properties")
                .and_then(|v| v.as_mapping())
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| {
                            Some((k.as_str()?.to_string(), PropertyConfig::parse(v)))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            views,
            new_item_folder: get("newItemFolder").and_then(as_str),
            new_item_template: get("newItemTemplate").and_then(as_str),
            preserved,
        })
    }

    /// The top-level keys in the order Obsidian writes them.
    ///
    /// The preserved keys are spread first, in their file order, and every key
    /// the serializer would otherwise place is appended only if the spread did
    /// not already put it there. That is why `summaries` comes back where the
    /// file had it while `filters` does not: the parser leaves one in the
    /// unrecognized map and takes the other out.
    pub fn key_order(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.preserved.keys().cloned().collect();
        let mut push = |k: &str, present: bool| {
            if present && !keys.iter().any(|seen| seen == k) {
                keys.push(k.to_string());
            }
        };
        push("filters", self.filters.is_some());
        push("formulas", !self.formulas.is_empty());
        push("summaries", self.summaries.is_some());
        push(
            "properties",
            self.properties.values().any(|p| !p.is_dropped()),
        );
        push("views", !self.views.is_empty());
        push("newItemFolder", self.new_item_folder.is_some());
        push("newItemTemplate", self.new_item_template.is_some());
        keys
    }
}

impl View {
    /// This view's keys in the order Obsidian writes them: its own fields
    /// first, in the serializer's order and only when set, then every
    /// preserved key in file order.
    pub fn key_order(&self) -> Vec<String> {
        let mut keys = vec!["type".to_string(), "name".to_string()];
        let mut push = |k: &str, present: bool| {
            if present {
                keys.push(k.to_string());
            }
        };
        push("filters", self.filters.is_some());
        push("groupBy", self.group_by.is_some());
        push("groupOrder", !self.group_order.is_empty());
        push("order", !self.order.is_empty());
        push("sort", !self.sort.is_empty());
        push("limit", self.limit.is_some());
        push("summaries", !self.summaries.is_empty());
        keys.extend(self.preserved.keys().cloned());
        keys
    }
}

impl PropertyConfig {
    /// This config's keys in the order Obsidian writes them: the unknown ones
    /// first, then `displayName`.
    pub fn key_order(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.preserved.keys().cloned().collect();
        if self.display_name.is_some() {
            keys.push("displayName".to_string());
        }
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probe's own fixture, scrambled the way it was written, so the key
    /// order these tests assert is the order the run observed coming back.
    /// `tools/obsidian-probe/results/2026-10-25-bases.json`.
    const SCRAMBLED: &str = r#"
summaries:
  customAverage: 'values.mean().round(3)'
unknownTopKey:
  nested: 1
filters:
  and:
    - 'status != "done"'
    - file.hasTag("rock")
formulas:
  ppu: "(density / silica).toFixed(2)"
  label: 'type + " "  + file.name'
properties:
  status:
    unknownPropKey: 1
    displayName: Status
  note.unused: {}
views:
  - type: table
    name: My table
    groupBy:
      property: status
      direction: ASC
    order:
      - file.name
      - status
    sort:
      - property: status
        direction: DESC
    limit: 10
    summaries:
      formula.ppu: Average
    unknownViewKey: kept
    rowHeight: 2
newItemFolder: Notes
"#;

    #[test]
    fn parses_the_probe_fixture() {
        let b = Base::from_yaml(SCRAMBLED).unwrap();
        assert!(matches!(b.filters, Some(Filter::And(ref v)) if v.len() == 2));
        assert_eq!(b.formulas["ppu"], "(density / silica).toFixed(2)");
        // The statement's double space survives, because Obsidian's own
        // round trip keeps the text rather than re-printing the parse tree.
        assert_eq!(b.formulas["label"], r#"type + " "  + file.name"#);
        assert_eq!(
            b.summaries.as_ref().unwrap()["customAverage"],
            "values.mean().round(3)"
        );
        assert_eq!(b.new_item_folder.as_deref(), Some("Notes"));
        assert_eq!(b.views[0].name, "My table");
        assert_eq!(b.views[0].limit, Some(10));
        assert_eq!(b.views[0].sort[0].direction.as_deref(), Some("DESC"));
    }

    #[test]
    fn formula_and_summary_order_is_the_file_order() {
        let b = Base::from_yaml(SCRAMBLED).unwrap();
        // A sorted map would put `label` first and rewrite the file on the
        // next save, which is the reason this is an IndexMap.
        assert_eq!(b.formulas.keys().collect::<Vec<_>>(), ["ppu", "label"]);
    }

    #[test]
    fn preserved_keys_are_spread_first_and_summaries_is_one_of_them() {
        let b = Base::from_yaml(SCRAMBLED).unwrap();
        // `summaries` is parsed AND left among the preserved keys, so it keeps
        // the position the file gave it; `filters` is consumed, so it does not.
        assert_eq!(
            b.preserved.keys().collect::<Vec<_>>(),
            ["summaries", "unknownTopKey"]
        );
        assert_eq!(
            b.key_order(),
            [
                "summaries",
                "unknownTopKey",
                "filters",
                "formulas",
                "properties",
                "views",
                "newItemFolder"
            ]
        );
    }

    #[test]
    fn a_views_unknown_and_layout_keys_are_appended_last() {
        let b = Base::from_yaml(SCRAMBLED).unwrap();
        assert_eq!(
            b.views[0].key_order(),
            [
                "type",
                "name",
                "groupBy",
                "order",
                "sort",
                "limit",
                "summaries",
                "unknownViewKey",
                "rowHeight"
            ]
        );
    }

    #[test]
    fn a_propertys_unknown_keys_come_before_its_display_name() {
        let b = Base::from_yaml(SCRAMBLED).unwrap();
        assert_eq!(
            b.properties["status"].key_order(),
            ["unknownPropKey", "displayName"]
        );
    }

    #[test]
    fn an_empty_property_config_is_dropped_and_takes_the_key_with_it() {
        let b = Base::from_yaml(SCRAMBLED).unwrap();
        // `note.unused: {}` parses, but Obsidian's serialize() returns null for
        // it and a null is left out of the written map.
        assert!(b.properties["note.unused"].is_dropped());
        assert!(!b.properties["status"].is_dropped());

        let all_empty = Base::from_yaml("properties:\n  a: {}\n  b: {}\n").unwrap();
        assert!(!all_empty.properties.is_empty());
        assert!(!all_empty.key_order().iter().any(|k| k == "properties"));
    }

    #[test]
    fn a_view_must_have_a_non_empty_name() {
        // The one hard structural rule the published schema does not state.
        let err = Base::from_yaml("views:\n  - type: table\n").unwrap_err();
        assert_eq!(err.to_string(), "Missing or invalid \"name\" in view 1");
        let err = Base::from_yaml("views:\n  - type: table\n    name: ''\n").unwrap_err();
        assert_eq!(err.to_string(), "Missing or invalid \"name\" in view 1");
        // The index is 1-based and names the offending view.
        let two = "views:\n  - type: table\n    name: Fine\n  - type: table\n";
        assert_eq!(
            Base::from_yaml(two).unwrap_err().to_string(),
            "Missing or invalid \"name\" in view 2"
        );
    }

    #[test]
    fn a_filter_group_is_the_one_place_an_unknown_key_is_refused() {
        let err = Base::from_yaml("filters:\n  and: []\n  extra: 1\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "\"filters\" may only have one of an \"and\", \"or\", or \"not\" keys."
        );
        // ... while an unknown key anywhere else is kept.
        let b = Base::from_yaml("unknown: 1\nviews: []\n").unwrap();
        assert_eq!(b.preserved["unknown"], Value::Number(1.into()));
    }

    #[test]
    fn an_unknown_view_type_parses_and_is_kept() {
        // Obsidian says "Unknown view type: nonsense" in the view and keeps
        // the file: it is not a parse error.
        let b = Base::from_yaml("views:\n  - type: nonsense\n    name: V\n").unwrap();
        assert_eq!(b.views[0].kind, "nonsense");
    }

    #[test]
    fn an_empty_file_is_an_empty_base() {
        // What `bases:new-file` creates is zero bytes.
        let b = Base::from_yaml("").unwrap();
        assert_eq!(b, Base::default());
        assert!(b.key_order().is_empty());
        assert!(Base::from_yaml("{}\n").unwrap().key_order().is_empty());
    }

    #[test]
    fn group_order_keeps_a_null() {
        let b =
            Base::from_yaml("views:\n  - type: table\n    name: V\n    groupOrder: [todo, null]\n")
                .unwrap();
        assert_eq!(b.views[0].group_order.len(), 2);
        assert!(b.views[0].group_order[1].is_null());
    }
}
