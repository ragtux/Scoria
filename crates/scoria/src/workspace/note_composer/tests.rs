//! The three commands end to end over a vault on disk: the probe's notes
//! written into a `tempfile::tempdir()`, the operation run through the
//! same `VaultState` calls the commands make, and every file of the vault
//! compared with the bytes Obsidian left (2026-11-09,
//! `tools/obsidian-probe/results/2026-11-09-note-composer-unique.json`;
//! the step is named on each test).

use super::*;
use std::collections::BTreeMap;

fn stamp() -> scoria_templates::Stamp {
    scoria_templates::Stamp {
        year: 2026,
        month: 11,
        day: 9,
        hour: 20,
        minute: 21,
        second: 0,
    }
}

/// `app.json` for the merges that land in `.trash`. The probe's vaults had
/// `{}` and still got `.trash/`, but that was Obsidian falling back when
/// the session's system trash refused the move; Scoria's unset
/// `trashOption` is the system trash, which a test must never reach, so
/// these say `local` -- the one value whose destination was observed.
const LOCAL: &str = r#"{"trashOption":"local"}"#;

/// A vault of `notes`, with `app.json` and `note-composer.json` as given.
fn vault(
    notes: &[(&str, &str)],
    app: &str,
    composer: Option<&str>,
) -> (tempfile::TempDir, VaultState) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".obsidian")).unwrap();
    std::fs::write(root.join(".obsidian/app.json"), app).unwrap();
    if let Some(c) = composer {
        std::fs::write(root.join(".obsidian/note-composer.json"), c).unwrap();
    }
    for (path, text) in notes {
        let at = root.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    let state = VaultState::load(&root.to_path_buf()).unwrap();
    (dir, state)
}

/// Every file of the vault outside `.obsidian`, `.trash` included, with
/// its bytes.
fn files(dir: &tempfile::TempDir) -> BTreeMap<String, String> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == ".obsidian" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read_to_string(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir.path(), dir.path(), &mut out);
    out
}

fn expect(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect()
}

/// The merge as the command runs it once the questions are answered:
/// the target and the links written, then the source through the delete
/// path's `VaultState::trash`, by `trashOption`.
fn merge(
    state: &VaultState,
    source: &str,
    target: &str,
    create: bool,
    place: Placement,
    update: bool,
) -> MergeOutcome {
    let outcome = state
        .merge_files(source, target, create, place, update, stamp())
        .unwrap();
    state.trash(source).unwrap();
    outcome
}

const L_SRC: &str = "# H\nsrc body ^blk\n\nself [[l-src#H]]\n";
const L_DST: &str = "dst body links [[l-src]]\n";
const L_LINKS: &str = "wiki [[l-src]]\nheading [[l-src#H]]\nalias [[l-src|x]]\nblock [[l-src#^blk]]\nembed ![[l-src]]\nmd [x](l-src.md)\nmd heading [x](l-src.md#H)\nmd embed ![x](l-src.md)\npath [[l-src.md]]\n";

/// s08-links, "Just once": every link to the source points at the target,
/// the target's own included, the merged-in self-link as written, the
/// source in `.trash` (`trashOption` unset), and the Notice's 11 in 3.
#[test]
fn merge_file_rewrites_every_link_but_the_merged_self_link_and_trashes_the_source() {
    let (dir, state) = vault(
        &[
            ("l-src.md", L_SRC),
            ("l-dst.md", L_DST),
            ("l-links.md", L_LINKS),
        ],
        LOCAL,
        None,
    );
    assert_eq!(state.merge_link_counts("l-src.md", "l-dst.md"), (3, 11));
    let outcome = merge(
        &state,
        "l-src.md",
        "l-dst.md",
        false,
        Placement::Append,
        true,
    );
    assert_eq!((outcome.links, outcome.files), (11, 3));
    assert_eq!(
        files(&dir),
        expect(&[
            ("l-dst.md", "dst body links [[l-dst]]\n\n# H\nsrc body ^blk\n\nself [[l-src#H]]\n"),
            ("l-links.md", "wiki [[l-dst]]\nheading [[l-dst#H]]\nalias [[l-dst|x]]\nblock [[l-dst#^blk]]\nembed ![[l-dst]]\nmd [x](l-dst.md)\nmd heading [x](l-dst.md#H)\nmd embed ![x](l-dst.md)\npath [[l-dst]]\n"),
            (".trash/l-src.md", L_SRC),
        ])
    );
}

/// s08b's answer, "Do not update", on the same vault: the links are left
/// dangling and nothing but the target changes.
#[test]
fn merge_file_without_updating_leaves_the_links_alone() {
    let (dir, state) = vault(
        &[
            ("l-src.md", L_SRC),
            ("l-dst.md", L_DST),
            ("l-links.md", L_LINKS),
        ],
        LOCAL,
        None,
    );
    let outcome = merge(
        &state,
        "l-src.md",
        "l-dst.md",
        false,
        Placement::Append,
        false,
    );
    assert_eq!(outcome.links, 0);
    assert_eq!(
        files(&dir),
        expect(&[
            (
                "l-dst.md",
                "dst body links [[l-src]]\n\n# H\nsrc body ^blk\n\nself [[l-src#H]]\n"
            ),
            ("l-links.md", L_LINKS),
            (".trash/l-src.md", L_SRC),
        ])
    );
}

/// s08c-links-across-folders: the source in `Sub/` trashes flat, and both
/// link forms take the target's shortest one.
#[test]
fn merge_file_across_folders_trashes_flat_and_writes_the_shortest_links() {
    let (dir, state) = vault(
        &[
            ("Sub/sub-src.md", "sub src\n"),
            ("Other/sub-dst.md", "other dst\n"),
            ("Sub/sub-links.md", "see [[sub-src]] and [[Sub/sub-src]]\n"),
        ],
        LOCAL,
        None,
    );
    let outcome = merge(
        &state,
        "Sub/sub-src.md",
        "Other/sub-dst.md",
        false,
        Placement::Append,
        true,
    );
    assert_eq!((outcome.links, outcome.files), (2, 1));
    assert_eq!(
        files(&dir),
        expect(&[
            ("Other/sub-dst.md", "other dst\n\nsub src\n"),
            ("Sub/sub-links.md", "see [[sub-dst]] and [[sub-dst]]\n"),
            (".trash/sub-src.md", "sub src\n"),
        ])
    );
}

/// s09-trash-local and s09-trash-none.
#[test]
fn merge_file_sends_the_source_where_trash_option_says() {
    let (dir, state) = vault(
        &[("t-local-src.md", "local\n"), ("t-local-dst.md", "dst\n")],
        r#"{"trashOption":"local"}"#,
        None,
    );
    merge(
        &state,
        "t-local-src.md",
        "t-local-dst.md",
        false,
        Placement::Append,
        true,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            ("t-local-dst.md", "dst\n\nlocal\n"),
            (".trash/t-local-src.md", "local\n")
        ])
    );
    let (dir, state) = vault(
        &[("t-none-src.md", "none\n"), ("t-none-dst.md", "dst\n")],
        r#"{"trashOption":"none"}"#,
        None,
    );
    merge(
        &state,
        "t-none-src.md",
        "t-none-dst.md",
        false,
        Placement::Append,
        true,
    );
    assert_eq!(files(&dir), expect(&[("t-none-dst.md", "dst\n\nnone\n")]));
}

/// s07-fm-prepend-clash, Shift+Enter, through the files.
#[test]
fn merge_file_at_the_top_keeps_one_frontmatter() {
    let src =
        "---\ntitle: from src\ntags:\n  - s1\n  - shared\naliases: [sa]\nonly: s\n---\nsrc body\n";
    let dst = "---\ntitle: from dst\ntags:\n  - d1\n  - shared\naliases: [da]\n---\ndst body\n";
    let (dir, state) = vault(
        &[("f-pre-clash-src.md", src), ("f-pre-clash-dst.md", dst)],
        LOCAL,
        None,
    );
    merge(
        &state,
        "f-pre-clash-src.md",
        "f-pre-clash-dst.md",
        false,
        Placement::Prepend,
        true,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            ("f-pre-clash-dst.md", "---\ntitle: from src\ntags:\n  - d1\n  - shared\n  - s1\naliases:\n  - da\n  - sa\nonly: s\n---\nsrc body\n\ndst body\n"),
            (".trash/f-pre-clash-src.md", src),
        ])
    );
}

/// s19c: Ctrl+Enter's note is the query numbered past a taken name, at the
/// root with `newFileLocation` unset even from `Sub/`, holding the source's
/// bytes exactly.
#[test]
fn merge_file_into_a_new_note_names_it_from_the_query_and_copies_the_source() {
    let (dir, state) = vault(
        &[
            ("k3b-src.md", "k3b src\n"),
            ("Sub/k3d-src.md", "k3d src\n"),
            ("k3-existing.md", ""),
        ],
        LOCAL,
        None,
    );
    let target = state.composer_new_path("k3-existing", "k3b-src.md");
    assert_eq!(target, "k3-existing 1.md");
    merge(&state, "k3b-src.md", &target, true, Placement::Append, true);
    let target = state.composer_new_path("k3-exi", "Sub/k3d-src.md");
    assert_eq!(target, "k3-exi.md");
    merge(
        &state,
        "Sub/k3d-src.md",
        &target,
        true,
        Placement::Append,
        true,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            ("k3-existing.md", ""),
            ("k3-existing 1.md", "k3b src\n"),
            ("k3-exi.md", "k3d src\n"),
            (".trash/k3b-src.md", "k3b src\n"),
            (".trash/k3d-src.md", "k3d src\n"),
        ])
    );
}

const COMPOSER: &str = "T:{{fromTitle}}|{{newTitle}}|{{date:YYYY}}|{{content}}|END\n";

/// s12-template: `template` names the file with `.md`.
#[test]
fn merge_file_wraps_the_source_in_the_composer_template() {
    let (dir, state) = vault(
        &[
            ("tpl-src.md", "tpl src\n"),
            ("tpl-dst.md", "tpl dst\n"),
            ("Templates/Composer.md", COMPOSER),
        ],
        LOCAL,
        Some(
            r#"{"askBeforeMerging":true,"replacementText":"link","template":"Templates/Composer.md"}"#,
        ),
    );
    merge(
        &state,
        "tpl-src.md",
        "tpl-dst.md",
        false,
        Placement::Append,
        true,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            (
                "tpl-dst.md",
                "tpl dst\n\nT:tpl-src|tpl-dst|2026|tpl src\n|END\n"
            ),
            ("Templates/Composer.md", COMPOSER),
            (".trash/tpl-src.md", "tpl src\n"),
        ])
    );
}

/// s14-dont-ask-again: the checkbox writes `askBeforeMerging: false`, the
/// way the command does it.
#[test]
fn dont_ask_again_writes_ask_before_merging_false() {
    let (dir, state) = vault(&[("a.md", "a\n")], LOCAL, None);
    let mut next = state.note_composer().clone().with_defaults();
    next.ask_before_merging = Some(false);
    state
        .apply_setting(&SettingChange::NoteComposer(next))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".obsidian/note-composer.json")).unwrap(),
        "{\n  \"askBeforeMerging\": false\n}"
    );
    assert_eq!(state.note_composer().ask_before_merging, Some(false));
}

/// An extraction as the command runs it: the target written, and the
/// source's replacement made -- by its editor in the app, here on the text
/// and saved, which is what the editor's save writes.
fn extract(
    dir: &tempfile::TempDir,
    state: &VaultState,
    source: &str,
    range: Range<usize>,
    target: &str,
    create: bool,
    place: Placement,
) {
    let text = std::fs::read_to_string(dir.path().join(source)).unwrap();
    let outcome = state
        .extract_into(source, &text, range, target, create, place, stamp())
        .unwrap();
    std::fs::write(dir.path().join(source), outcome.plan.applied(&text)).unwrap();
}

const PICK: &str = "before\nPICK ME\nafter\n";

/// `app.json`, the notes besides the source, and where the new note lands.
type Location = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static str,
);

/// The five s01 steps: `replacementText` by link format.
#[test]
fn split_file_leaves_a_link_an_embed_or_nothing_and_writes_the_selection() {
    let cases = [
        (
            "link",
            false,
            "new-link-wiki",
            "before\n[[new-link-wiki]]\nafter\n",
        ),
        (
            "embed",
            false,
            "new-embed-wiki",
            "before\n![[new-embed-wiki]]\nafter\n",
        ),
        ("none", false, "new-none-wiki", "before\n\nafter\n"),
        (
            "link",
            true,
            "new-link-md",
            "before\n[new-link-md](new-link-md.md)\nafter\n",
        ),
        (
            "embed",
            true,
            "new-embed-md",
            "before\n![new-embed-md](new-embed-md.md)\nafter\n",
        ),
    ];
    for (replacement, md, name, after) in cases {
        let app = format!("{{\"useMarkdownLinks\":{md}}}");
        let composer =
            format!("{{\"askBeforeMerging\":true,\"replacementText\":\"{replacement}\"}}");
        let (dir, state) = vault(&[("s.md", PICK)], &app, Some(&composer));
        let target = state.composer_new_path(name, "s.md");
        extract(
            &dir,
            &state,
            "s.md",
            7..14,
            &target,
            true,
            Placement::Append,
        );
        let new_note = format!("{name}.md");
        assert_eq!(
            files(&dir),
            expect(&[("s.md", after), (new_note.as_str(), "PICK ME")]),
            "{replacement}, markdown links {md}"
        );
    }
}

/// s02-folder-root, -current and -folder (Inbox there, and not).
#[test]
fn split_file_puts_the_new_note_where_new_file_location_says() {
    let cases: [Location; 4] = [
        (r#"{"newFileLocation":"root"}"#, &[], "folder-x.md"),
        (r#"{"newFileLocation":"current"}"#, &[], "Sub/folder-x.md"),
        (
            r#"{"newFileLocation":"folder","newFileFolderPath":"Inbox"}"#,
            &[("Inbox/keep.md", "")],
            "Inbox/folder-x.md",
        ),
        (
            r#"{"newFileLocation":"folder","newFileFolderPath":"Inbox"}"#,
            &[],
            "folder-x.md",
        ),
    ];
    for (app, extra, want) in cases {
        let mut notes = vec![("Sub/s-folder.md", PICK)];
        notes.extend_from_slice(extra);
        let (dir, state) = vault(&notes, app, None);
        let target = state.composer_new_path("folder-x", "Sub/s-folder.md");
        assert_eq!(target, want, "{app}");
        extract(
            &dir,
            &state,
            "Sub/s-folder.md",
            7..14,
            &target,
            true,
            Placement::Append,
        );
        let mut wanted = vec![
            ("Sub/s-folder.md", "before\n[[folder-x]]\nafter\n"),
            (want, "PICK ME"),
        ];
        wanted.extend_from_slice(extra);
        assert_eq!(files(&dir), expect(&wanted), "{app}");
    }
}

/// s03 and s03b: into an existing note, Enter then Shift+Enter.
#[test]
fn split_file_into_an_existing_note_appends_then_prepends() {
    let (dir, state) = vault(
        &[
            ("s-existing.md", PICK),
            ("s-existing-target.md", "existing target\n"),
        ],
        LOCAL,
        None,
    );
    extract(
        &dir,
        &state,
        "s-existing.md",
        7..14,
        "s-existing-target.md",
        false,
        Placement::Append,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            ("s-existing.md", "before\n[[s-existing-target]]\nafter\n"),
            ("s-existing-target.md", "existing target\n\nPICK ME"),
        ])
    );
    std::fs::write(dir.path().join("s-existing.md"), PICK).unwrap();
    let state = VaultState::load(&dir.path().to_path_buf()).unwrap();
    extract(
        &dir,
        &state,
        "s-existing.md",
        7..14,
        "s-existing-target.md",
        false,
        Placement::Prepend,
    );
    assert_eq!(
        files(&dir)["s-existing-target.md"],
        "PICK ME\n\nexisting target\n\nPICK ME"
    );
}

/// s11-template: the new note is the template around the selection.
#[test]
fn split_file_into_a_new_note_goes_through_the_template() {
    let (dir, state) = vault(
        &[("tpl-split.md", PICK), ("Templates/Composer.md", COMPOSER)],
        LOCAL,
        Some(r#"{"template":"Templates/Composer.md"}"#),
    );
    extract(
        &dir,
        &state,
        "tpl-split.md",
        7..14,
        "tpl-new.md",
        true,
        Placement::Append,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            ("tpl-split.md", "before\n[[tpl-new]]\nafter\n"),
            ("tpl-new.md", "T:tpl-split|tpl-new|2026|PICK ME|END\n"),
            ("Templates/Composer.md", COMPOSER),
        ])
    );
}

/// s06, s07, s07b, s08 and s08c: the heading's section by the one parser,
/// under the default name. The probe ran extract-heading under the default
/// settings only; it goes through the split's code, so the other
/// `replacementText` values and link formats are the split's, by
/// inference.
#[test]
fn extract_heading_moves_the_section_and_leaves_a_link_named_after_it() {
    let nested =
        "# Zulu\ntop body\n## Yankee\nmid body\n### Xray\ndeep body\n## Whiskey\nnext body\n";
    let cases = [
        (
            nested,
            0,
            "[[Zulu]]\n",
            "# Zulu\ntop body\n## Yankee\nmid body\n### Xray\ndeep body\n## Whiskey\nnext body",
        ),
        (
            nested,
            2,
            "# Zulu\ntop body\n[[Yankee]]\n## Whiskey\nnext body\n",
            "## Yankee\nmid body\n### Xray\ndeep body",
        ),
        (
            "# Zulu\ntop body\n## Victor\nlast body\n\n\n",
            2,
            "# Zulu\ntop body\n[[Victor]]\n",
            "## Victor\nlast body\n\n",
        ),
        (
            "intro\n## Uniform\nsec body\n\n## Tango\nafter body\n",
            1,
            "intro\n[[Uniform]]\n## Tango\nafter body\n",
            "## Uniform\nsec body\n",
        ),
        (
            "pre\n## Oscar\noscar body",
            1,
            "pre\n[[Oscar]]",
            "## Oscar\noscar body",
        ),
    ];
    for (text, line, after, new_note) in cases {
        let (dir, state) = vault(&[("h.md", text)], LOCAL, None);
        let section = scoria_composer::heading_section(text, &index_markdown(text), line).unwrap();
        let target = state.composer_new_path(&section.name, "h.md");
        extract(
            &dir,
            &state,
            "h.md",
            section.range,
            &target,
            true,
            Placement::Append,
        );
        assert_eq!(
            files(&dir),
            expect(&[("h.md", after), (target.as_str(), new_note)]),
            "{}",
            section.name
        );
    }
}

/// s08d-heading-with-template.
#[test]
fn extract_heading_goes_through_the_template() {
    let text = "pre\n## Mike\nmike body\n## Lima\n";
    let (dir, state) = vault(
        &[("h-tpl.md", text), ("Templates/Composer.md", COMPOSER)],
        LOCAL,
        Some(r#"{"template":"Templates/Composer.md"}"#),
    );
    let section = scoria_composer::heading_section(text, &index_markdown(text), 1).unwrap();
    extract(
        &dir,
        &state,
        "h-tpl.md",
        section.range,
        "Mike.md",
        true,
        Placement::Append,
    );
    assert_eq!(
        files(&dir),
        expect(&[
            ("h-tpl.md", "pre\n[[Mike]]\n## Lima\n"),
            ("Mike.md", "T:h-tpl|Mike|2026|## Mike\nmike body|END\n"),
            ("Templates/Composer.md", COMPOSER),
        ])
    );
}

/// The target is the one note a merge cannot leave alone: changed on disk
/// since the index read it, nothing is written and the source stays.
#[test]
fn a_target_changed_on_disk_is_never_overwritten() {
    let (dir, state) = vault(&[("a.md", "a\n"), ("b.md", "b\n")], LOCAL, None);
    // Changed after the index, but reindexed by the merge itself.
    std::fs::write(dir.path().join("b.md"), "b edited\n").unwrap();
    state
        .merge_files("a.md", "b.md", false, Placement::Append, false, stamp())
        .unwrap();
    assert_eq!(files(&dir)["b.md"], "b edited\n\na\n");
    // A new note whose name was taken meanwhile is refused.
    assert!(state
        .merge_files("a.md", "b.md", true, Placement::Append, false, stamp())
        .is_err());
}
