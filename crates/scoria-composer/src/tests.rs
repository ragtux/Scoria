//! Every case is a step of the 2026-11-09 probe, its bytes copied from
//! `tools/obsidian-probe/results/2026-11-09-note-composer-unique.json`
//! (the step's name is beside it).

use super::*;
use scoria_cache::index_markdown;

fn identity(body: &str) -> String {
    body.to_string()
}

/// (source, target, key, target after) for every merge the probe ran
/// without links or a template.
#[rustfmt::skip]
const MERGES: &[(&str, &str, Placement, &str)] = &[
    // s05-append-yy
    ("src line 1\nsrc line 2\n", "dst line 1\ndst line 2\n", Placement::Append, "dst line 1\ndst line 2\n\nsrc line 1\nsrc line 2\n"),
    // s06-prepend-yy
    ("src line 1\nsrc line 2\n", "dst line 1\ndst line 2\n", Placement::Prepend, "src line 1\nsrc line 2\n\ndst line 1\ndst line 2\n"),
    // s05-append-yn
    ("src line 1\nsrc line 2\n", "dst line 1\ndst line 2", Placement::Append, "dst line 1\ndst line 2\n\nsrc line 1\nsrc line 2\n"),
    // s06-prepend-yn
    ("src line 1\nsrc line 2\n", "dst line 1\ndst line 2", Placement::Prepend, "src line 1\nsrc line 2\n\ndst line 1\ndst line 2"),
    // s05-append-ny
    ("src line 1\nsrc line 2", "dst line 1\ndst line 2\n", Placement::Append, "dst line 1\ndst line 2\n\nsrc line 1\nsrc line 2"),
    // s06-prepend-ny
    ("src line 1\nsrc line 2", "dst line 1\ndst line 2\n", Placement::Prepend, "src line 1\nsrc line 2\n\ndst line 1\ndst line 2\n"),
    // s05-append-nn
    ("src line 1\nsrc line 2", "dst line 1\ndst line 2", Placement::Append, "dst line 1\ndst line 2\n\nsrc line 1\nsrc line 2"),
    // s06-prepend-nn
    ("src line 1\nsrc line 2", "dst line 1\ndst line 2", Placement::Prepend, "src line 1\nsrc line 2\n\ndst line 1\ndst line 2"),
    // s05-append-whitespace
    ("\n\nsrc after two blank lines\n\n", "dst with trailing blank lines\n\n\n", Placement::Append, "dst with trailing blank lines\n\n\n\n\nsrc after two blank lines\n\n"),
    // s06-prepend-whitespace
    ("\n\nsrc after two blank lines\n\n", "\n\ndst after two blank lines\n", Placement::Prepend, "\n\nsrc after two blank lines\n\n\n\ndst after two blank lines\n"),
    // s05-append-crlf
    ("crlf src 1\r\ncrlf src 2\r\n", "crlf dst 1\r\ncrlf dst 2\r\n", Placement::Append, "crlf dst 1\r\ncrlf dst 2\r\n\ncrlf src 1\r\ncrlf src 2\r\n"),
];

/// The same for the six frontmatter steps.
#[rustfmt::skip]
const FRONTMATTER: &[(&str, &str, Placement, &str)] = &[
    // s07-fm-append-distinct
    ("---\nsrckey: s\n---\nsrc body\n", "---\ndstkey: d\n---\ndst body\n", Placement::Append, "---\ndstkey: d\nsrckey: s\n---\ndst body\n\nsrc body\n"),
    // s07-fm-prepend-distinct
    ("---\nsrckey: s\n---\nsrc body\n", "---\ndstkey: d\n---\ndst body\n", Placement::Prepend, "---\ndstkey: d\nsrckey: s\n---\nsrc body\n\ndst body\n"),
    // s07-fm-append-clash
    ("---\ntitle: from src\ntags:\n  - s1\n  - shared\naliases: [sa]\nonly: s\n---\nsrc body\n", "---\ntitle: from dst\ntags:\n  - d1\n  - shared\naliases: [da]\n---\ndst body\n", Placement::Append, "---\ntitle: from src\ntags:\n  - d1\n  - shared\n  - s1\naliases:\n  - da\n  - sa\nonly: s\n---\ndst body\n\nsrc body\n"),
    // s07-fm-prepend-clash
    ("---\ntitle: from src\ntags:\n  - s1\n  - shared\naliases: [sa]\nonly: s\n---\nsrc body\n", "---\ntitle: from dst\ntags:\n  - d1\n  - shared\naliases: [da]\n---\ndst body\n", Placement::Prepend, "---\ntitle: from src\ntags:\n  - d1\n  - shared\n  - s1\naliases:\n  - da\n  - sa\nonly: s\n---\nsrc body\n\ndst body\n"),
    // s07-fm-append-srconly
    ("---\nsrckey: s\n---\nsrc body\n", "dst body\n", Placement::Append, "---\nsrckey: s\n---\ndst body\n\nsrc body\n"),
    // s07-fm-prepend-srconly
    ("---\nsrckey: s\n---\nsrc body\n", "dst body\n", Placement::Prepend, "---\nsrckey: s\n---\nsrc body\n\ndst body\n"),
];

#[test]
fn the_eleven_joints_pad_the_first_part_to_a_blank_line_and_keep_the_second() {
    assert_eq!(MERGES.len(), 11);
    for (source, target, place, after) in MERGES {
        assert_eq!(
            merge_notes(target, source, *place, &identity),
            *after,
            "{source:?} into {target:?}, {place:?}"
        );
    }
}

/// The CRLF step's padding is a bare `\n` between two CRLF parts.
#[test]
fn the_padding_is_a_bare_newline_even_between_crlf_parts() {
    assert_eq!(joint("a\r\n", "b\r\n"), "a\r\n\nb\r\n");
}

#[test]
fn two_frontmatters_become_one_with_the_targets_keys_first() {
    for (source, target, place, after) in FRONTMATTER {
        assert_eq!(
            merge_notes(target, source, *place, &identity),
            *after,
            "{source:?} into {target:?}, {place:?}"
        );
    }
}

/// s19c: the note Ctrl+Enter makes holds the source's bytes exactly, and
/// a new note an extraction makes its selection exactly (s01).
#[test]
fn an_empty_target_takes_the_text_unpadded() {
    assert_eq!(
        merge_notes("", "k3b src\n", Placement::Append, &identity),
        "k3b src\n"
    );
    assert_eq!(
        merge_notes("", "PICK ME", Placement::Append, &identity),
        "PICK ME"
    );
    let with_block = "---\nk: v\n---\nbody\n";
    assert_eq!(
        merge_notes("", with_block, Placement::Append, &identity),
        with_block
    );
}

fn stamp() -> Stamp {
    Stamp {
        year: 2026,
        month: 11,
        day: 9,
        hour: 20,
        minute: 21,
        second: 0,
    }
}

const COMPOSER: &str = "T:{{fromTitle}}|{{newTitle}}|{{date:YYYY}}|{{content}}|END\n";
const NO_CONTENT: &str = "T:{{fromTitle}}|{{newTitle}}|END\n";

/// s12-template and s13-template-no-content.
#[test]
fn a_merge_wraps_the_source_in_the_template() {
    let plan = plan_merge(
        "tpl-src.md",
        "tpl src\n",
        "tpl-dst.md",
        "tpl dst\n",
        Placement::Append,
        Some((COMPOSER, stamp())),
        &[],
    );
    assert_eq!(
        plan.target,
        "tpl dst\n\nT:tpl-src|tpl-dst|2026|tpl src\n|END\n"
    );
    let plan = plan_merge(
        "tpl2-src.md",
        "tpl2 src\n",
        "tpl2-dst.md",
        "tpl2 dst\n",
        Placement::Append,
        Some((NO_CONTENT, stamp())),
        &[],
    );
    assert_eq!(
        plan.target,
        "tpl2 dst\n\nT:tpl2-src|tpl2-dst|END\n\n\ntpl2 src\n"
    );
}

#[test]
fn a_template_without_content_gets_it_after_two_newlines() {
    let vars = TemplateVars {
        from_title: "tpl-split2",
        new_title: "tpl-new2",
        at: stamp(),
    };
    assert_eq!(
        expand_template(NO_CONTENT, "PICK ME", &vars),
        "T:tpl-split2|tpl-new2|END\n\n\nPICK ME"
    );
}

/// The content is put in, never scanned: braces in a note stay text, and
/// the variables nothing observed stay as written.
#[test]
fn the_content_is_not_expanded_and_unknown_variables_stay() {
    let vars = TemplateVars {
        from_title: "a",
        new_title: "b",
        at: stamp(),
    };
    assert_eq!(
        expand_template("{{content}} {{title}} {{date}}", "{{fromTitle}}", &vars),
        "{{fromTitle}} {{title}} {{date}}"
    );
}

fn resolver(paths: &[&str]) -> LinkResolver {
    LinkResolver::new(paths.iter().copied())
}

fn opts(replacement: Replacement, markdown_links: bool) -> ExtractOptions<'static> {
    ExtractOptions {
        replacement,
        link: scoria_suggest::Options {
            format: LinkFormat::Shortest,
            markdown_links,
        },
        template: None,
    }
}

/// The five s01 steps and s10b: the selection's place under each
/// `replacementText` and link format, and the new note's bytes.
#[test]
fn a_split_leaves_a_link_an_embed_or_nothing_in_the_vaults_link_format() {
    let source = "before\nPICK ME\nafter\n";
    let range = 7..14;
    let cases = [
        (
            "s-link-wiki.md",
            "new-link-wiki.md",
            Replacement::Link,
            false,
            "before\n[[new-link-wiki]]\nafter\n",
        ),
        (
            "s-embed-wiki.md",
            "new-embed-wiki.md",
            Replacement::Embed,
            false,
            "before\n![[new-embed-wiki]]\nafter\n",
        ),
        (
            "s-none-wiki.md",
            "new-none-wiki.md",
            Replacement::None,
            false,
            "before\n\nafter\n",
        ),
        (
            "s-link-md.md",
            "new-link-md.md",
            Replacement::Link,
            true,
            "before\n[new-link-md](new-link-md.md)\nafter\n",
        ),
        (
            "s-embed-md.md",
            "new-embed-md.md",
            Replacement::Embed,
            true,
            "before\n![new-embed-md](new-embed-md.md)\nafter\n",
        ),
        (
            "Sub/s-sub-md.md",
            "md-from-sub.md",
            Replacement::Link,
            true,
            "before\n[md-from-sub](md-from-sub.md)\nafter\n",
        ),
    ];
    for (from, to, replacement, md, after) in cases {
        let plan = plan_extract(
            from,
            source,
            range.clone(),
            to,
            None,
            Placement::Append,
            &resolver(&[from, to]),
            &opts(replacement, md),
        );
        assert_eq!(plan.applied(source), after, "{from} -> {to}");
        assert_eq!(plan.target_text, "PICK ME");
        assert_eq!(plan.caret(), 7 + plan.replacement.len());
    }
}

/// s02: wherever the new note lands, the shortest link reaches it.
#[test]
fn a_split_into_a_folder_still_writes_the_shortest_link() {
    let source = "before\nPICK ME\nafter\n";
    for to in [
        "folder-root.md",
        "Sub/folder-current.md",
        "Inbox/folder-folder.md",
    ] {
        let plan = plan_extract(
            "Sub/s-folder.md",
            source,
            7..14,
            to,
            None,
            Placement::Append,
            &resolver(&["Sub/s-folder.md", to]),
            &opts(Replacement::Link, false),
        );
        let name = basename(to);
        assert_eq!(plan.applied(source), format!("before\n[[{name}]]\nafter\n"));
    }
}

/// s03, s03b and s04: into an existing note by the joint rule.
#[test]
fn a_split_into_an_existing_note_joins_it_by_the_merge_rule() {
    let source = "before\nPICK ME\nafter\n";
    let r = resolver(&["s-existing.md", "s-existing-target.md"]);
    let o = opts(Replacement::Link, false);
    let plan = plan_extract(
        "s-existing.md",
        source,
        7..14,
        "s-existing-target.md",
        Some("existing target\n"),
        Placement::Append,
        &r,
        &o,
    );
    assert_eq!(plan.target_text, "existing target\n\nPICK ME");
    assert_eq!(
        plan.applied(source),
        "before\n[[s-existing-target]]\nafter\n"
    );
    let plan = plan_extract(
        "s-existing.md",
        source,
        7..14,
        "s-existing-target.md",
        Some("existing target\n\nPICK ME"),
        Placement::Prepend,
        &r,
        &o,
    );
    assert_eq!(plan.target_text, "PICK ME\n\nexisting target\n\nPICK ME");

    let multi = "before\nline one\nline two\nafter\n";
    let plan = plan_extract(
        "s-multiline.md",
        multi,
        7..24,
        "s-existing.md",
        Some("before\n[[s-existing-target]]\nafter\n"),
        Placement::Append,
        &resolver(&["s-multiline.md", "s-existing.md"]),
        &o,
    );
    assert_eq!(
        plan.target_text,
        "before\n[[s-existing-target]]\nafter\n\nline one\nline two"
    );
    assert_eq!(plan.applied(multi), "before\n[[s-existing]]\nafter\n");
}

/// s11-template and s12-template-no-content.
#[test]
fn a_split_into_a_new_note_goes_through_the_template() {
    let source = "before\nPICK ME\nafter\n";
    for (tpl, to, want) in [
        (
            COMPOSER,
            "tpl-new.md",
            "T:tpl-split|tpl-new|2026|PICK ME|END\n",
        ),
        (
            NO_CONTENT,
            "tpl-new.md",
            "T:tpl-split|tpl-new|END\n\n\nPICK ME",
        ),
    ] {
        let mut o = opts(Replacement::Link, false);
        o.template = Some((tpl, stamp()));
        let plan = plan_extract(
            "tpl-split.md",
            source,
            7..14,
            to,
            None,
            Placement::Append,
            &resolver(&["tpl-split.md", to]),
            &o,
        );
        assert_eq!(plan.target_text, want);
    }
}

/// The six extract-heading steps: the source after, the new note, and the
/// name offered.
#[test]
fn a_headings_section_runs_to_the_next_heading_of_its_level_or_higher() {
    let nested =
        "# Zulu\ntop body\n## Yankee\nmid body\n### Xray\ndeep body\n## Whiskey\nnext body\n";
    let cases = [
        // s06-heading-top-default
        (
            nested,
            0,
            "Zulu",
            "[[Zulu]]\n",
            "# Zulu\ntop body\n## Yankee\nmid body\n### Xray\ndeep body\n## Whiskey\nnext body",
        ),
        // s07-heading-nested-default
        (
            nested,
            2,
            "Yankee",
            "# Zulu\ntop body\n[[Yankee]]\n## Whiskey\nnext body\n",
            "## Yankee\nmid body\n### Xray\ndeep body",
        ),
        // s08-heading-middle
        (
            "intro\n## Uniform\nsec body\n\n## Tango\nafter body\n",
            1,
            "Uniform",
            "intro\n[[Uniform]]\n## Tango\nafter body\n",
            "## Uniform\nsec body\n",
        ),
        // s07b-heading-last
        (
            "# Zulu\ntop body\n## Victor\nlast body\n\n\n",
            2,
            "Victor",
            "# Zulu\ntop body\n[[Victor]]\n",
            "## Victor\nlast body\n\n",
        ),
        // s08c-heading-eof-no-newline
        (
            "pre\n## Oscar\noscar body",
            1,
            "Oscar",
            "pre\n[[Oscar]]",
            "## Oscar\noscar body",
        ),
    ];
    for (text, line, name, after, new_note) in cases {
        let meta = index_markdown(text);
        let section = heading_section(text, &meta, line).expect("a heading line");
        assert_eq!(section.name, name);
        let to = format!("{name}.md");
        let plan = plan_extract(
            "h.md",
            text,
            section.range,
            &to,
            None,
            Placement::Append,
            &resolver(&["h.md", &to]),
            &opts(Replacement::Link, false),
        );
        assert_eq!(plan.applied(text), after, "{name}");
        assert_eq!(plan.target_text, new_note, "{name}");
    }
}

/// s07c: a typed name in place of the heading's.
#[test]
fn a_typed_name_takes_the_section_all_the_same() {
    let text = "# Zulu\ntop body\n## Papa\npapa body\n";
    let meta = index_markdown(text);
    let section = heading_section(text, &meta, 2).unwrap();
    let plan = plan_extract(
        "h-new.md",
        text,
        section.range,
        "typed-heading-name.md",
        None,
        Placement::Append,
        &resolver(&["h-new.md", "typed-heading-name.md"]),
        &opts(Replacement::Link, false),
    );
    assert_eq!(
        plan.applied(text),
        "# Zulu\ntop body\n[[typed-heading-name]]\n"
    );
    assert_eq!(plan.target_text, "## Papa\npapa body");
}

/// s08d-heading-with-template.
#[test]
fn an_extracted_heading_goes_through_the_template() {
    let text = "pre\n## Mike\nmike body\n## Lima\n";
    let meta = index_markdown(text);
    let section = heading_section(text, &meta, 1).unwrap();
    let mut o = opts(Replacement::Link, false);
    o.template = Some((COMPOSER, stamp()));
    let plan = plan_extract(
        "h-tpl.md",
        text,
        section.range,
        "Mike.md",
        None,
        Placement::Append,
        &resolver(&["h-tpl.md", "Mike.md"]),
        &o,
    );
    assert_eq!(plan.applied(text), "pre\n[[Mike]]\n## Lima\n");
    assert_eq!(
        plan.target_text,
        "T:h-tpl|Mike|2026|## Mike\nmike body|END\n"
    );
}

/// s08b: a caret in a body under a heading is not on a heading line, and
/// s10: the raw heading text is offered and refused.
#[test]
fn only_a_heading_line_has_a_section_and_its_raw_name_may_be_refused() {
    let text = "pre\n## Sierra/Romeo: Quebec? *em* [[Link]]\nbody\n";
    let meta = index_markdown(text);
    assert!(heading_section(text, &meta, 2).is_none());
    let section = heading_section(text, &meta, 1).unwrap();
    assert_eq!(section.name, "Sierra/Romeo: Quebec? *em* [[Link]]");
    assert_eq!(
        check_name(&section.name),
        Err("Error: File name cannot contain any of these characters: \\\u{a0}/\u{a0}:")
    );
    assert!(check_name("Zulu").is_ok());
}

/// s05-append-yy's confirmation, curly quotes and all.
#[test]
fn the_confirmation_names_both_notes_in_curly_quotes() {
    assert_eq!(
        confirmation("m-app-yy-src.md", "Sub/m-app-yy-dst.md"),
        "Are you sure you want to merge \u{201c}m-app-yy-src\u{201d} into \u{201c}m-app-yy-dst\u{201d}? \u{201c}m-app-yy-src\u{201d} will be deleted."
    );
}

fn cache(notes: &[(&str, &str)]) -> MetadataCache {
    MetadataCache::new(
        LinkResolver::new(notes.iter().map(|(p, _)| *p)),
        notes.iter().map(|(p, t)| (*p, index_markdown(t))),
    )
}

/// s08-links: every link form to the source becomes one to the target,
/// the target's own included, the merged-in self-link excepted -- and the
/// count is the one Obsidian asked about, the source's link among them.
#[test]
fn links_to_the_source_follow_it_into_the_target_but_its_own_stay() {
    let src = "# H\nsrc body ^blk\n\nself [[l-src#H]]\n";
    let dst = "dst body links [[l-src]]\n";
    let links = "wiki [[l-src]]\nheading [[l-src#H]]\nalias [[l-src|x]]\nblock [[l-src#^blk]]\nembed ![[l-src]]\nmd [x](l-src.md)\nmd heading [x](l-src.md#H)\nmd embed ![x](l-src.md)\npath [[l-src.md]]\n";
    let notes = [("l-src.md", src), ("l-dst.md", dst), ("l-links.md", links)];
    let cache = cache(&notes);
    let texts = |p: &str| {
        notes
            .iter()
            .find(|(n, _)| *n == p)
            .map(|(_, t)| t.to_string())
    };
    let rewrites = link_rewrites(&cache, "l-src.md", "l-dst.md", LinkFormat::Shortest, &texts);
    assert_eq!(counts(&rewrites), (3, 11));
    let plan = plan_merge(
        "l-src.md",
        src,
        "l-dst.md",
        dst,
        Placement::Append,
        None,
        &rewrites,
    );
    assert_eq!(
        plan.target,
        "dst body links [[l-dst]]\n\n# H\nsrc body ^blk\n\nself [[l-src#H]]\n"
    );
    assert_eq!(plan.others.len(), 1);
    assert_eq!(
        plan.others[0].text,
        "wiki [[l-dst]]\nheading [[l-dst#H]]\nalias [[l-dst|x]]\nblock [[l-dst#^blk]]\nembed ![[l-dst]]\nmd [x](l-dst.md)\nmd heading [x](l-dst.md#H)\nmd embed ![x](l-dst.md)\npath [[l-dst]]\n"
    );
}

/// s08c-links-across-folders.
#[test]
fn links_across_folders_take_the_targets_shortest_form() {
    let notes = [
        ("Sub/sub-src.md", "sub src\n"),
        ("Other/sub-dst.md", "other dst\n"),
        ("Sub/sub-links.md", "see [[sub-src]] and [[Sub/sub-src]]\n"),
    ];
    let cache = cache(&notes);
    let texts = |p: &str| {
        notes
            .iter()
            .find(|(n, _)| *n == p)
            .map(|(_, t)| t.to_string())
    };
    let rewrites = link_rewrites(
        &cache,
        "Sub/sub-src.md",
        "Other/sub-dst.md",
        LinkFormat::Shortest,
        &texts,
    );
    assert_eq!(counts(&rewrites), (1, 2));
    let plan = plan_merge(
        "Sub/sub-src.md",
        "sub src\n",
        "Other/sub-dst.md",
        "other dst\n",
        Placement::Append,
        None,
        &rewrites,
    );
    assert_eq!(plan.target, "other dst\n\nsub src\n");
    assert_eq!(plan.others[0].text, "see [[sub-dst]] and [[sub-dst]]\n");
}

#[test]
fn replacement_text_defaults_to_a_link() {
    assert_eq!(Replacement::from_config(None), Replacement::Link);
    assert_eq!(Replacement::from_config(Some("embed")), Replacement::Embed);
    assert_eq!(Replacement::from_config(Some("none")), Replacement::None);
    assert_eq!(Replacement::from_config(Some("??")), Replacement::Link);
}
