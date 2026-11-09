//! `.obsidian/bookmarks.json`, the Bookmarks core plugin's file.
//!
//! The file is `{"items": [...]}`, two-space indented, no trailing newline,
//! new items appended at the end. Every item carries `type` then `ctime`
//! (Unix milliseconds) and then what its type needs — see
//! `docs/scoria/Specification.md`, "Observed bookmarks behavior". An item
//! is kept as the JSON object it was read as, so an item Scoria did not
//! touch is written back key for key, unknown keys included; only items
//! Scoria makes are laid out here, in the order Obsidian writes them.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};

/// One bookmark: a `file` (optionally to a `#Heading` or `#^block`), a
/// `search`, the `graph`, a `folder`, a `url`, or a `group` of bookmarks.
#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark(Map<String, Value>);

impl Bookmark {
    fn new(kind: &str, ctime: i64) -> Bookmark {
        let mut map = Map::new();
        map.insert("type".into(), json!(kind));
        map.insert("ctime".into(), json!(ctime));
        Bookmark(map)
    }

    fn with_title(mut self, title: Option<&str>) -> Bookmark {
        if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
            self.0.insert("title".into(), json!(title));
        }
        self
    }

    /// A note, or a heading or block in one when `subpath` is `#Heading`
    /// or `#^id`.
    pub fn file(path: &str, subpath: Option<&str>, title: Option<&str>, ctime: i64) -> Bookmark {
        let mut b = Bookmark::new("file", ctime);
        b.0.insert("path".into(), json!(path));
        if let Some(sub) = subpath.filter(|s| !s.is_empty()) {
            b.0.insert("subpath".into(), json!(sub));
        }
        b.with_title(title)
    }

    pub fn folder(path: &str, title: Option<&str>, ctime: i64) -> Bookmark {
        let mut b = Bookmark::new("folder", ctime);
        b.0.insert("path".into(), json!(path));
        b.with_title(title)
    }

    pub fn search(query: &str, title: Option<&str>, ctime: i64) -> Bookmark {
        let mut b = Bookmark::new("search", ctime);
        b.0.insert("query".into(), json!(query));
        b.with_title(title)
    }

    /// The graph with its whole option set, as Obsidian saves it: the title
    /// is always written (`Untitled Graph` when the field was left alone).
    pub fn graph(title: &str, options: Value, ctime: i64) -> Bookmark {
        let mut b = Bookmark::new("graph", ctime);
        b.0.insert(
            "title".into(),
            json!(if title.trim().is_empty() {
                "Untitled Graph"
            } else {
                title.trim()
            }),
        );
        b.0.insert("options".into(), options);
        b
    }

    /// A `url` item, as the plugin API adds one (the UI path is unprobed).
    pub fn website(url: &str, title: Option<&str>, ctime: i64) -> Bookmark {
        let mut b = Bookmark::new("url", ctime);
        b.0.insert("url".into(), json!(url));
        b.with_title(title)
    }

    /// A group: `items` before `title`, as "Bookmark all tabs" writes it.
    pub fn group(title: Option<&str>, items: Vec<Bookmark>, ctime: i64) -> Bookmark {
        let mut b = Bookmark::new("group", ctime);
        b.0.insert(
            "items".into(),
            Value::Array(items.into_iter().map(|i| Value::Object(i.0)).collect()),
        );
        b.with_title(title)
    }

    pub fn from_value(value: &Value) -> Option<Bookmark> {
        value.as_object().map(|m| Bookmark(m.clone()))
    }

    pub fn as_value(&self) -> Value {
        Value::Object(self.0.clone())
    }

    fn str(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str)
    }

    pub fn kind(&self) -> &str {
        self.str("type").unwrap_or("")
    }

    pub fn ctime(&self) -> i64 {
        self.0.get("ctime").and_then(Value::as_i64).unwrap_or(0)
    }

    pub fn path(&self) -> Option<&str> {
        self.str("path")
    }

    pub fn subpath(&self) -> Option<&str> {
        self.str("subpath")
    }

    pub fn query(&self) -> Option<&str> {
        self.str("query")
    }

    pub fn url(&self) -> Option<&str> {
        self.str("url")
    }

    pub fn title(&self) -> Option<&str> {
        self.str("title")
    }

    /// A group's bookmarks; empty for anything else.
    pub fn items(&self) -> Vec<Bookmark> {
        self.0
            .get("items")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Bookmark::from_value).collect())
            .unwrap_or_default()
    }

    /// A title typed on the row or in "Edit bookmark": written as `title`,
    /// in its place when the item has one and after everything else when
    /// it has not — `Beta.md` renamed on the row came back
    /// `type, ctime, path, title` (2026-11-07 probe, `s12`).
    pub fn set_title(&mut self, title: &str) {
        self.0.insert("title".into(), json!(title));
    }

    /// Takes the title out, so the item shows its default name again.
    pub fn clear_title(&mut self) {
        self.0.shift_remove("title");
    }

    pub fn set_items(&mut self, items: Vec<Bookmark>) {
        self.0.insert(
            "items".into(),
            Value::Array(items.into_iter().map(|i| Value::Object(i.0)).collect()),
        );
    }

    /// Whether this is the plain bookmark of `path` — the whole note, not a
    /// heading or block in it.
    pub fn is_file(&self, path: &str) -> bool {
        self.kind() == "file" && self.path() == Some(path) && self.subpath().is_none()
    }

    /// What the pane shows: the title when one was given, else what Obsidian
    /// falls back to — `Note`, `Note#Heading`, `Note#^id`, the query, the
    /// folder path, the url.
    pub fn display_title(&self) -> String {
        if let Some(title) = self.title().filter(|t| !t.is_empty()) {
            return title.to_string();
        }
        match self.kind() {
            "file" => {
                let path = self.path().unwrap_or("");
                let name = path.rsplit('/').next().unwrap_or(path);
                let name = name.strip_suffix(".md").unwrap_or(name);
                format!("{name}{}", self.subpath().unwrap_or(""))
            }
            "folder" => self.path().unwrap_or("").to_string(),
            "search" => self.query().unwrap_or("").to_string(),
            "url" => self.url().unwrap_or("").to_string(),
            "graph" => "Untitled Graph".to_string(),
            _ => String::new(),
        }
    }
}

/// The file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bookmarks {
    pub items: Vec<Bookmark>,
    /// Keys beside `items`, kept for the round trip.
    extra: Map<String, Value>,
}

impl Bookmarks {
    pub fn from_value(value: &Value) -> Bookmarks {
        let Some(object) = value.as_object() else {
            return Bookmarks::default();
        };
        let items = object
            .get("items")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Bookmark::from_value).collect())
            .unwrap_or_default();
        let extra = object
            .iter()
            .filter(|(k, _)| k.as_str() != "items")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Bookmarks { items, extra }
    }

    pub fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "items".into(),
            Value::Array(self.items.iter().map(Bookmark::as_value).collect()),
        );
        for (k, v) in &self.extra {
            object.insert(k.clone(), v.clone());
        }
        Value::Object(object)
    }

    /// The bytes Obsidian writes: two-space indented, no trailing newline.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.to_value())
            .unwrap_or_else(|_| "{\n  \"items\": []\n}".into())
    }

    pub fn push(&mut self, item: Bookmark) {
        self.items.push(item);
    }

    /// Whether `path` has a plain file bookmark anywhere, groups included.
    pub fn has_file(&self, path: &str) -> bool {
        fn any(items: &[Bookmark], path: &str) -> bool {
            items
                .iter()
                .any(|i| i.is_file(path) || (i.kind() == "group" && any(&i.items(), path)))
        }
        any(&self.items, path)
    }

    /// Takes out the plain file bookmark of `path` that was added last
    /// (the one Obsidian's lookup by path lands on), wherever it sits.
    /// Returns whether one was found.
    pub fn remove_last_file(&mut self, path: &str) -> bool {
        fn remove(items: &mut Vec<Bookmark>, path: &str) -> bool {
            // Groups first in reverse order, so the deepest latest member
            // goes before an earlier top-level one.
            for ix in (0..items.len()).rev() {
                if items[ix].kind() == "group" {
                    let mut members = items[ix].items();
                    if remove(&mut members, path) {
                        items[ix].set_items(members);
                        return true;
                    }
                } else if items[ix].is_file(path) {
                    items.remove(ix);
                    return true;
                }
            }
            false
        }
        remove(&mut self.items, path)
    }

    /// The bookmark at `at`, a path of indexes into groups.
    pub fn get(&self, at: &[usize]) -> Option<Bookmark> {
        let (first, rest) = at.split_first()?;
        let mut item = self.items.get(*first)?.clone();
        for ix in rest {
            item = item.items().get(*ix)?.clone();
        }
        Some(item)
    }

    /// Changes the bookmark at `at` in place; false when there is none.
    pub fn update_at(&mut self, at: &[usize], change: impl FnOnce(&mut Bookmark)) -> bool {
        fn update(
            items: &mut [Bookmark],
            at: &[usize],
            change: impl FnOnce(&mut Bookmark),
        ) -> bool {
            match at {
                [] => false,
                [ix] => match items.get_mut(*ix) {
                    Some(item) => {
                        change(item);
                        true
                    }
                    None => false,
                },
                [ix, rest @ ..] => {
                    let Some(group) = items.get_mut(*ix) else {
                        return false;
                    };
                    let mut members = group.items();
                    let done = update(&mut members, rest, change);
                    if done {
                        group.set_items(members);
                    }
                    done
                }
            }
        }
        update(&mut self.items, at, change)
    }

    /// Puts `item` at `at`: the last index is its position in the list the
    /// rest of the path names (the top level, or a group's items), clamped
    /// to that list's end. False when the path does not name a group.
    pub fn insert_at(&mut self, at: &[usize], item: Bookmark) -> bool {
        fn insert(items: &mut Vec<Bookmark>, at: &[usize], item: Bookmark) -> bool {
            match at {
                [] => false,
                [ix] => {
                    items.insert((*ix).min(items.len()), item);
                    true
                }
                [ix, rest @ ..] => {
                    let Some(group) = items.get_mut(*ix).filter(|g| g.kind() == "group") else {
                        return false;
                    };
                    let mut members = group.items();
                    let done = insert(&mut members, rest, item);
                    if done {
                        group.set_items(members);
                    }
                    done
                }
            }
        }
        insert(&mut self.items, at, item)
    }

    /// Moves the bookmark at `from` so it lands at `to`, where `to` is an
    /// insertion path read against the list *before* the move (what a drop
    /// indicator points at). A group cannot go inside itself. Returns the
    /// path the item ends up at, or `None` when nothing moved.
    pub fn move_item(&mut self, from: &[usize], to: &[usize]) -> Option<Vec<usize>> {
        if from.is_empty() || to.is_empty() || to.starts_with(from) && to.len() > from.len() {
            return None;
        }
        // Taking `from` out shifts every later sibling, and anything inside
        // them, one place up.
        let mut target = to.to_vec();
        let depth = from.len() - 1;
        if target.len() > depth && target[..depth] == from[..depth] && target[depth] > from[depth] {
            target[depth] -= 1;
        }
        let item = self.remove_at(from)?;
        if self.insert_at(&target, item.clone()) {
            Some(target)
        } else {
            self.insert_at(from, item);
            None
        }
    }

    /// Removes the bookmark at `at`, a path of indexes into groups.
    pub fn remove_at(&mut self, at: &[usize]) -> Option<Bookmark> {
        fn remove(items: &mut Vec<Bookmark>, at: &[usize]) -> Option<Bookmark> {
            match at {
                [] => None,
                [ix] => (*ix < items.len()).then(|| items.remove(*ix)),
                [ix, rest @ ..] => {
                    let group = items.get_mut(*ix)?;
                    let mut members = group.items();
                    let removed = remove(&mut members, rest)?;
                    group.set_items(members);
                    Some(removed)
                }
            }
        }
        remove(&mut self.items, at)
    }
}

impl Serialize for Bookmarks {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Bookmarks {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Bookmarks, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(Bookmarks::from_value(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file Obsidian wrote on 2026-10-20 after the probe's commands,
    /// key order and all.
    const OBSERVED: &str = r##"{
  "items": [
    {
      "type": "file",
      "ctime": 1792522156931,
      "path": "A.md"
    },
    {
      "type": "file",
      "ctime": 1792522164382,
      "path": "A.md",
      "subpath": "#Second heading"
    },
    {
      "type": "search",
      "ctime": 1792522171543,
      "query": "alpha"
    },
    {
      "type": "group",
      "ctime": 1792522177407,
      "items": [
        {
          "type": "file",
          "ctime": 1792522176901,
          "path": "B.md"
        },
        {
          "type": "file",
          "ctime": 1792522176901,
          "path": "A.md"
        }
      ],
      "title": "October 20, 2026 1:49 PM"
    },
    {
      "type": "folder",
      "path": "sub",
      "ctime": 1792522178621,
      "pluginKey": true
    }
  ]
}"##;

    #[test]
    fn the_observed_file_round_trips_byte_for_byte() {
        let parsed: Bookmarks = serde_json::from_str(OBSERVED).unwrap();
        assert_eq!(parsed.items.len(), 5);
        assert_eq!(parsed.to_json(), OBSERVED);
    }

    #[test]
    fn new_items_take_obsidians_key_order() {
        let keys = |b: &Bookmark| b.0.keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            keys(&Bookmark::file("A.md", Some("#^b56db2"), None, 1)),
            ["type", "ctime", "path", "subpath"]
        );
        assert_eq!(
            keys(&Bookmark::file("A.md", None, Some("  "), 1)),
            ["type", "ctime", "path"]
        );
        assert_eq!(
            keys(&Bookmark::search("alpha", None, 1)),
            ["type", "ctime", "query"]
        );
        assert_eq!(
            keys(&Bookmark::graph("", json!({}), 1)),
            ["type", "ctime", "title", "options"]
        );
        assert_eq!(
            keys(&Bookmark::group(Some("Today"), vec![], 1)),
            ["type", "ctime", "items", "title"]
        );
        assert_eq!(
            Bookmark::graph(" ", json!({}), 1).title(),
            Some("Untitled Graph")
        );
    }

    #[test]
    fn display_titles_fall_back_the_way_the_pane_shows_them() {
        assert_eq!(
            Bookmark::file("sub/A.md", None, None, 1).display_title(),
            "A"
        );
        assert_eq!(
            Bookmark::file("A.md", Some("#^b56db2"), None, 1).display_title(),
            "A#^b56db2"
        );
        assert_eq!(
            Bookmark::file("A.md", None, Some("Mine"), 1).display_title(),
            "Mine"
        );
        assert_eq!(Bookmark::search("alpha", None, 1).display_title(), "alpha");
        assert_eq!(Bookmark::folder("sub", None, 1).display_title(), "sub");
    }

    #[test]
    fn removing_the_last_file_bookmark_reaches_into_groups_and_leaves_the_older_one() {
        let mut parsed: Bookmarks = serde_json::from_str(OBSERVED).unwrap();
        assert!(parsed.has_file("A.md"));
        assert!(parsed.remove_last_file("A.md"));
        // The copy inside the group went; the top-level one and the heading stayed.
        assert_eq!(parsed.items[3].items().len(), 1);
        assert!(parsed.items[0].is_file("A.md"));
        assert!(parsed.remove_last_file("A.md"));
        assert!(!parsed.has_file("A.md"));
        assert!(!parsed.remove_last_file("A.md"));
        assert_eq!(
            parsed
                .remove_at(&[2, 0])
                .map(|b| b.path().unwrap().to_string()),
            Some("B.md".into())
        );
        assert!(parsed.remove_at(&[9]).is_none());
    }

    /// The probe's vault, cut down: Alpha, Gamma, then Group one holding
    /// Beta and a nested group.
    fn sample() -> Bookmarks {
        let mut b = Bookmarks::default();
        b.push(Bookmark::file("Alpha.md", None, None, 1));
        b.push(Bookmark::file("Gamma.md", None, None, 2));
        b.push(Bookmark::group(
            Some("Group one"),
            vec![
                Bookmark::file("Beta.md", None, None, 3),
                Bookmark::group(Some("Nested"), vec![], 4),
            ],
            5,
        ));
        b
    }

    fn titles(items: &[Bookmark]) -> Vec<String> {
        items.iter().map(Bookmark::display_title).collect()
    }

    /// A title typed on the row lands after `path` on an item that had
    /// none, which is the order Obsidian wrote for `Beta.md` renamed on its
    /// row, and replaces a group's title where it stands.
    #[test]
    fn a_title_set_on_a_row_goes_last_or_stays_in_place() {
        let mut b = sample();
        assert!(b.update_at(&[2, 0], |i| i.set_title("Beta renamed")));
        let beta = b.get(&[2, 0]).unwrap().as_value();
        let keys: Vec<&String> = beta.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["type", "ctime", "path", "title"]);
        assert!(b.update_at(&[2], |i| i.set_title("Renamed group")));
        let group = b.get(&[2]).unwrap().as_value();
        let keys: Vec<&String> = group.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["type", "ctime", "items", "title"]);
        assert_eq!(b.get(&[2, 0]).unwrap().display_title(), "Beta renamed");
        assert!(!b.update_at(&[7], |i| i.set_title("x")));
    }

    /// Dropped above a row, an item takes that row's place; dropped on a
    /// group it becomes the group's first member, which is where
    /// `Image.png` landed when the probe dropped it on "Group one".
    #[test]
    fn a_drop_above_a_row_or_onto_a_group_lands_where_obsidian_put_it() {
        let mut b = sample();
        assert_eq!(b.move_item(&[1], &[0]), Some(vec![0]));
        assert_eq!(titles(&b.items), ["Gamma", "Alpha", "Group one"]);
        let mut b = sample();
        assert_eq!(b.move_item(&[0], &[2, 0]), Some(vec![1, 0]));
        assert_eq!(titles(&b.items), ["Gamma", "Group one"]);
        assert_eq!(titles(&b.items[1].items()), ["Alpha", "Beta", "Nested"]);
        // Out of a group to the top level, below everything.
        let mut b = sample();
        assert_eq!(b.move_item(&[2, 0], &[3]), Some(vec![3]));
        assert_eq!(titles(&b.items), ["Alpha", "Gamma", "Group one", "Beta"]);
        // Later in the same list: the gap it leaves is closed first.
        let mut b = sample();
        assert_eq!(b.move_item(&[0], &[2]), Some(vec![1]));
        assert_eq!(titles(&b.items), ["Gamma", "Alpha", "Group one"]);
    }

    #[test]
    fn a_group_cannot_be_dropped_inside_itself() {
        let mut b = sample();
        let before = b.clone();
        assert_eq!(b.move_item(&[2], &[2, 1, 0]), None);
        assert_eq!(b.move_item(&[2], &[2, 0]), None);
        assert_eq!(b, before);
    }

    /// A file dragged in from the Files pane is a new item at the drop.
    #[test]
    fn an_item_inserted_at_a_drop_takes_that_place() {
        let mut b = sample();
        assert!(b.insert_at(&[1], Bookmark::file("New.md", None, None, 9)));
        assert!(b.insert_at(&[3, 2], Bookmark::file("Deep.md", None, None, 9)));
        assert!(b.insert_at(&[99], Bookmark::file("End.md", None, None, 9)));
        assert!(!b.insert_at(&[0, 0], Bookmark::file("Not a group.md", None, None, 9)));
        assert_eq!(
            titles(&b.items),
            ["Alpha", "New", "Gamma", "Group one", "End"]
        );
        assert_eq!(titles(&b.items[3].items()), ["Beta", "Nested", "Deep"]);
    }
}
