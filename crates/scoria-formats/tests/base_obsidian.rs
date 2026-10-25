//! The base model against what Obsidian 1.14.5 wrote, observed over CDP on
//! 2026-10-25 (`tools/obsidian-probe/bases.mjs`, raw run in
//! `tools/obsidian-probe/results/2026-10-25-bases.json`). `scrambled.in.base`
//! under `tests/base` is the file the probe wrote into a scratch vault, and
//! `scrambled.out.base` beside it is what Obsidian wrote back when the view
//! was closed — 1352 bytes in, 1256 out, with nothing edited.
//!
//! There is no writer yet, so what these assert is the half the model owes it:
//! that the key order the model computes is the key order Obsidian wrote, read
//! back out of the observed bytes rather than copied from the prose. The
//! writer and its byte-for-byte gate are the next commit.

use std::path::PathBuf;

use scoria_formats::base::Base;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/base")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The keys of a block in `text` at exactly `indent` spaces, in the order they
/// appear — the written order, taken from the bytes themselves so that these
/// tests cannot drift from the run by being retyped.
fn keys_at(text: &str, indent: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let depth = line.len() - line.trim_start().len();
        if depth != indent {
            continue;
        }
        let trimmed = line.trim_start();
        // A key line, not a sequence item and not a comment.
        if trimmed.starts_with('-') || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = trimmed.split_once(':') {
            if !key.is_empty() && !key.contains(' ') {
                out.push(key.to_string());
            }
        }
    }
    out
}

/// The top level: the preserved keys first in their file order, then the
/// serializer's own. `summaries` is in both and keeps its file position;
/// `filters` is consumed and does not.
#[test]
fn the_top_level_key_order_is_the_one_obsidian_wrote() {
    let base = Base::from_yaml(&fixture("scrambled.in.base")).unwrap();
    let written = keys_at(&fixture("scrambled.out.base"), 0);
    assert_eq!(
        written,
        [
            "summaries",
            "unknownTopKey",
            "filters",
            "formulas",
            "properties",
            "views",
            "newItemFolder",
            "newItemTemplate"
        ],
        "the fixture is not the file this test was written against"
    );
    assert_eq!(base.key_order(), written);
}

/// A view's own fields in the serializer's order, then its unknown keys — and
/// every layout key is an unknown key, so `rowHeight` and a plugin's key sort
/// together at the end rather than among the documented ones.
#[test]
fn a_views_key_order_is_the_one_obsidian_wrote() {
    let base = Base::from_yaml(&fixture("scrambled.in.base")).unwrap();
    let out = fixture("scrambled.out.base");
    // The first view's block: from its `- type:` line to the next one.
    let first = out
        .split("  - type:")
        .nth(1)
        .expect("a first view in the written bytes");
    let mut written = vec!["type".to_string()];
    written.extend(keys_at(first, 4));
    assert_eq!(
        written,
        [
            "type",
            "name",
            "groupBy",
            "order",
            "sort",
            "limit",
            "summaries",
            "unknownViewKey",
            "nestedUnknown"
        ]
    );
    assert_eq!(base.views[0].key_order(), written);
}

/// A property writes its unknown keys before `displayName`, which is the
/// reverse of the order the fixture had them in.
#[test]
fn a_propertys_key_order_is_the_one_obsidian_wrote() {
    let base = Base::from_yaml(&fixture("scrambled.in.base")).unwrap();
    let input = fixture("scrambled.in.base");
    assert!(
        input.contains("    displayName: Status\n    unknownPropKey: kept"),
        "the fixture no longer has displayName first, so this proves nothing"
    );
    assert_eq!(
        base.properties["status"].key_order(),
        ["unknownPropKey", "displayName"]
    );
}

/// `note.unused: {}` is in the file and not in the written bytes: an empty
/// config serializes to null and a null is left out of the map.
#[test]
fn an_empty_property_config_is_dropped() {
    let base = Base::from_yaml(&fixture("scrambled.in.base")).unwrap();
    let out = fixture("scrambled.out.base");
    assert!(fixture("scrambled.in.base").contains("note.unused"));
    assert!(!out.contains("note.unused"));
    assert!(base.properties["note.unused"].is_dropped());
}

/// Statement text survives byte for byte. Only the YAML quoting around it
/// changes, which is the emitter's business and not the model's.
#[test]
fn statement_text_survives_byte_for_byte() {
    let base = Base::from_yaml(&fixture("scrambled.in.base")).unwrap();
    // A double space inside a string literal, and a trailing space inside one.
    assert_eq!(base.formulas["spaced"], r#"type + " "  + file.name"#);
    assert_eq!(base.formulas["trailingSpace"], r#"status + "x" "#);
    let out = fixture("scrambled.out.base");
    for statement in base.formulas.values() {
        assert!(
            out.contains(statement.as_str()),
            "{statement:?} is not in the bytes Obsidian wrote"
        );
    }
}

/// Every key of the written file round-trips into the model: nothing Obsidian
/// kept is dropped on the way in.
#[test]
fn what_obsidian_wrote_parses_back_to_the_same_model() {
    let from_in = Base::from_yaml(&fixture("scrambled.in.base")).unwrap();
    let from_out = Base::from_yaml(&fixture("scrambled.out.base")).unwrap();
    // The one difference the canonicalization makes to the *model* is the
    // dropped empty property config; everything else is byte noise the model
    // does not hold.
    assert!(from_in.properties.contains_key("note.unused"));
    assert!(!from_out.properties.contains_key("note.unused"));
    assert_eq!(from_in.key_order(), from_out.key_order());
    assert_eq!(from_in.formulas, from_out.formulas);
    assert_eq!(from_in.summaries, from_out.summaries);
    assert_eq!(from_in.filters, from_out.filters);
    assert_eq!(from_in.views, from_out.views);
    assert_eq!(from_in.new_item_folder, from_out.new_item_folder);
    assert_eq!(from_in.new_item_template, from_out.new_item_template);
}
