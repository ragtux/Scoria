//! `workspaces.json` against the files Obsidian 1.15.0 wrote.
//!
//! The fixtures under `tests/workspaces/` are the probe's own bytes
//! (`tools/obsidian-probe/results/2026-11-09-panes-workspaces-headings.json`,
//! key `ws`), one per stage of the run: `wNN-workspaces.json` is the file
//! after step `wNN`, `wNN-workspace.json` the `workspace.json` of that
//! moment. In order: save "One" from the modal (w02), save again with the
//! command (w02b, only `mtime` moves), save "Two" over a split (w03), load
//! "One" (w04), save over "One" with C open (w06), save it as "Three"
//! (w07), save-and-load from "Three" to "Two" (w08), Load "One" from the
//! modal (w09), then delete "Two" (w10a) and the active "One" (w10b).

use scoria_formats::workspaces::{WorkspaceLayout, Workspaces};
use serde_json::Value;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("workspaces/", $name))
    };
}

const STATES: &[(&str, &str)] = &[
    ("w02", fixture!("w02-workspaces.json")),
    ("w02b", fixture!("w02b-workspaces.json")),
    ("w03", fixture!("w03-workspaces.json")),
    ("w04", fixture!("w04-workspaces.json")),
    ("w06", fixture!("w06-workspaces.json")),
    ("w07", fixture!("w07-workspaces.json")),
    ("w08", fixture!("w08-workspaces.json")),
    ("w09", fixture!("w09-workspaces.json")),
    ("w10a", fixture!("w10a-workspaces.json")),
    ("w10b", fixture!("w10b-workspaces.json")),
];

fn parse(text: &str) -> Workspaces {
    Workspaces::parse(text).unwrap()
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn mtime(file: &Workspaces, name: &str) -> String {
    file.workspaces[name].mtime.clone().unwrap()
}

#[test]
fn every_file_obsidian_wrote_is_written_back_byte_for_byte() {
    for (stage, text) in STATES {
        let file = parse(text);
        assert_eq!(&file.to_text(), text, "{stage}");
        assert!(file.extra.is_empty(), "{stage}: nothing unknown at the top");
    }
}

#[test]
fn a_saved_layout_is_workspace_json_without_last_open_files_plus_mtime() {
    for (stage, workspace, file, name) in [
        (
            "w02",
            fixture!("w02-workspace.json"),
            fixture!("w02-workspaces.json"),
            "One",
        ),
        (
            "w03",
            fixture!("w03-workspace.json"),
            fixture!("w03-workspaces.json"),
            "Two",
        ),
        (
            "w06",
            fixture!("w06-workspace.json"),
            fixture!("w06-workspaces.json"),
            "One",
        ),
        (
            "w07",
            fixture!("w06-workspace.json"),
            fixture!("w07-workspaces.json"),
            "Three",
        ),
    ] {
        let saved = parse(file);
        let expected = &saved.workspaces[name];
        let made = WorkspaceLayout::from_workspace(&json(workspace), mtime(&saved, name));
        assert_eq!(&made, expected, "{stage}");
        // The keys in Obsidian's order, `mtime` last.
        let text = serde_json::to_string_pretty(&made).unwrap();
        let keys: Vec<String> = json(&text).as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            ["main", "left", "right", "left-ribbon", "active", "mtime"],
            "{stage}"
        );
    }
}

/// The whole run replayed through the type, each step from the file before
/// it: every intermediate file comes out as Obsidian's bytes.
#[test]
fn the_probes_saves_loads_and_deletes_replay_to_obsidians_bytes() {
    let at = |stage: &str| {
        STATES
            .iter()
            .find(|(s, _)| *s == stage)
            .map(|(_, text)| *text)
            .unwrap()
    };
    let layout = |workspace: &str, file: &str, name: &str| {
        WorkspaceLayout::from_workspace(&json(workspace), mtime(&parse(file), name))
    };

    // The first save makes the file.
    let mut file = Workspaces::default();
    file.save(
        "One",
        layout(fixture!("w02-workspace.json"), at("w02"), "One"),
    );
    assert_eq!(file.to_text(), at("w02"));

    // Saving the active layout again from the same `workspace.json` moves
    // only its `mtime`.
    file.save(
        "One",
        layout(fixture!("w02-workspace.json"), at("w02b"), "One"),
    );
    assert_eq!(file.to_text(), at("w02b"));

    file.save(
        "Two",
        layout(fixture!("w03-workspace.json"), at("w03"), "Two"),
    );
    assert_eq!(file.to_text(), at("w03"));

    // A load writes `active` and nothing else.
    file.set_active("One");
    assert_eq!(file.to_text(), at("w04"));

    // Saving under an existing name replaces it in place.
    file.save(
        "One",
        layout(fixture!("w06-workspace.json"), at("w06"), "One"),
    );
    assert_eq!(file.to_text(), at("w06"));

    // The active layout saved under a new name is a new entry, active.
    file.save(
        "Three",
        layout(fixture!("w06-workspace.json"), at("w07"), "Three"),
    );
    assert_eq!(file.to_text(), at("w07"));

    // Save and load: "Three" saved over first (the probe had A open by
    // then), then "Two" loaded.
    let three = parse(at("w08")).workspaces["Three"].clone();
    file.save("Three", three);
    file.set_active("Two");
    assert_eq!(file.to_text(), at("w08"));

    file.set_active("One");
    assert_eq!(file.to_text(), at("w09"));

    assert!(file.delete("Two"));
    assert_eq!(file.to_text(), at("w10a"));

    // Deleting the active layout leaves `active` naming it.
    assert!(file.delete("One"));
    assert_eq!(file.to_text(), at("w10b"));
    assert_eq!(file.active.as_deref(), Some("One"));
    assert_eq!(file.active_layout(), None);
    assert!(!file.delete("One"));
}

/// What a load restores is the layout as `workspace.json` has it: after
/// each of the probe's three loads, `workspace.json` was the loaded layout
/// key for key with the session's `lastOpenFiles` appended.
#[test]
fn a_loaded_layout_is_the_workspace_json_obsidian_rewrote_bar_last_open_files() {
    for (stage, workspace, file, name) in [
        (
            "w04",
            fixture!("w04-workspace.json"),
            fixture!("w04-workspaces.json"),
            "One",
        ),
        (
            "w08",
            fixture!("w08-workspace.json"),
            fixture!("w08-workspaces.json"),
            "Two",
        ),
        (
            "w09",
            fixture!("w09-workspace.json"),
            fixture!("w09-workspaces.json"),
            "One",
        ),
    ] {
        let mut written = json(workspace);
        let recent = written
            .as_object_mut()
            .unwrap()
            .shift_remove("lastOpenFiles");
        assert!(recent.is_some(), "{stage}");
        let restored = parse(file).workspaces[name].to_workspace();
        assert_eq!(
            serde_json::to_string_pretty(&restored).unwrap(),
            serde_json::to_string_pretty(&written).unwrap(),
            "{stage}"
        );
    }
}

#[test]
fn unknown_keys_survive_at_every_level() {
    let text = "{\n  \"workspaces\": {\n    \"A\": {\n      \"main\": {},\n      \"future\": [\n        1,\n        2.5\n      ],\n      \"mtime\": \"2026-11-09T20:05:44-06:00\"\n    }\n  },\n  \"active\": \"A\",\n  \"later\": true\n}";
    let file = parse(text);
    assert_eq!(file.to_text(), text);
    assert_eq!(file.extra.get("later"), Some(&Value::Bool(true)));
    // A byte order mark is read past.
    assert_eq!(parse(&format!("\u{feff}{text}")), file);
}
