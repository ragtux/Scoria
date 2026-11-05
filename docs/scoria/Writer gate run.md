---
tags:
  - scoria/dev
created: 2026-11-04
---
# Writer gate run

The gate for [[Development#Writer (phase 3)]] is a claim about a person, not a feature list:
**the vault round-trips byte-identical after a week of mixed use in both apps.** This note is
the run of that week — what is already settled, what the week has to produce, the day-by-day
sheet, and the log.

## Where the gate stands

**The harness half passes.** Every path from a keystroke to a file's bytes is held by
`crates/scoria-text/tests/writer_roundtrip.rs`: fifteen properties over the thousand-note
corpus and over the real vault, green again on 2026-11-04 after the themes, highlighting and
Bases work that landed since [[Development#The gate run opens, 2026-10-18]]. Nothing is
written to the vault — every edit happens in a buffer in memory.

**The mixed-use half has not run.** Not for want of a blocker: PLMCRZY was registered in
Obsidian on 2026-10-22 12:50 (which is what created its `app.json`, `appearance.json` and
`core-plugins.json`), so the one thing the October note was waiting on is done. The reason
is simpler — nothing has been edited in that vault since, so the clean
`diff -r snapshot-2026-10-25` is the diff of an inert folder. **A clean diff over a vault
nobody touched is zero evidence, not a pass.** That is the whole of what this week is for.

One thing the registration already bought: PLMCRZY's `.obsidian` now holds four modelled
files, so the config property covers a real person's set rather than passing vacuously the
way [[Development#Four small entries and a URI, 2026-11-02]] recorded.

## Two vaults, and which one decides

| Vault | Writers | Evidence | Weight |
| --- | --- | --- | --- |
| `~/bstore2/raga/plmcrzy/PLMCRZY` | Scoria and Obsidian only | snapshot and `diff -r` | **the verdict rests here** |
| `docs/scoria` | Scoria, Obsidian *and* the repo's tooling | git | watched, never decisive |

The deciding vault is the thin one because attribution is what the gate needs: a moved byte
there has exactly two suspects. `docs/scoria` is the folder actually lived in daily — it was
edited today, and Obsidian rewrote its `.obsidian` on 2026-10-30 and 2026-11-02 — so it is
worth watching for free through git, but a byte that moves there has three possible authors
and cannot settle anything.

## How a day runs

```
just gate-check        # the harness, then what moved in both vaults
#   ... the day's mixed use, both apps, same folder ...
just gate-baseline     # today's snapshot, so tomorrow's diff is one day wide
```

`tools/writer-gate/gate.sh` is the whole of it; `just gate-status` says what has been
snapshotted and whether the vault is still registered. Snapshots and hash manifests live in
`~/bstore2/raga/scoria-gate` and are deliberately copies rather than hashes alone: a hash
says a file moved, and the next question is always *which bytes*. `gate.sh check --from-first`
gives the cumulative diff across the whole run.

**Reading a diff.** A file the diff reports is the gate's evidence, not a failure. For each
one, two questions:

1. Which app moved it?
2. Was the move **an edit someone made**, or **an app rewriting bytes nobody touched**?

Only the second kind fails the gate. Record both answers in the log below — the answer is
worth more than the verdict.

**Mixed use means alternating on the same folder.** Each day: do the Scoria half, then open
the same notes in Obsidian and edit them there too, then check. A week of Scoria followed by
a week of Obsidian would prove far less.

## What the week has to produce beyond a diff

Three config writers are ratcheted nowhere, because no `.obsidian` set on this machine has
the file: `templates.json`, `daily-notes.json` and `switcher.json`, and Obsidian does not
create any of them until a setting in that plugin is written. `hotkeys.json` and `types.json`
are in the same position. Day 4 and day 7 below write exactly those settings, so the week
should leave the harness with its first real copy of each — worth more to the project than
the diff itself.

## The sheet

Every Writer item from [[Development#Writer (phase 3)]], spread over seven days. The vault
has nine notes with tables, wikilinks with `|` aliases, fenced ASCII diagrams and two
`.canvas` files, and — worth knowing before day 3 — **no YAML frontmatter in any note**, so
properties have to be created rather than edited.

**Day 1 — plain writing.** Source mode and Live Preview. Type into three notes in each mode,
let autosave run, close and reopen. Then the same notes in Obsidian.

**Day 2 — files and links.** Rename a note that others link to (link rewriting), move one
into a new folder, delete one, create a note by following an unresolved link, and use the
Files header's New note, New folder and two of the six sort orders.

**Day 3 — tables and properties.** The table commands on the nine notes that have tables:
row and column before/after/up/down, duplicate, delete, and the three alignments. Then
`markdown:add-metadata-property` to give a note its first frontmatter, edit it in the
Properties pane, fold it with `editor:toggle-fold-properties`, and set a property's **type**
(which is what writes `types.json`).

**Day 4 — autocompletion, templates, daily notes.** Trigger each suggester: `[[`, `[[Note#`,
an alias, `![[`, `#tag`, and a block reference `[[Note#^`. Then set the Templates folder in
Settings (writes `templates.json`), insert a template, set the Daily notes folder and format
(writes `daily-notes.json`), open today's note and step to the previous one.

**Day 5 — canvas.** Both `.canvas` files: pan, zoom, select, move, resize, draw an edge,
colour a node, group, copy, paste, undo, and type into a card. Then open the same canvas in
Obsidian and move one node. The canvas writer's bytes are the sharpest test in the vault
because Obsidian's canvas JSON is the least forgiving.

**Day 6 — editing commands and folding.** The checklist cycle, `Mod+L` on a line with no
box, the six highlight colours, Clear formatting, the per-line inline toggles, `Shift+Tab`'s
column arithmetic, and the five fold commands over headings and list parents in both modes.

**Day 7 — chrome, settings, and the verdict.** Split right and down, drag a tab between
groups, bookmark a view, a heading and a section, open the Bookmarks pane, then write
settings of all three shapes (a toggle, a dropdown, a slider or text field) and assign one
hotkey in Obsidian (writes `hotkeys.json`). Finish with `gate.sh check --from-first` for the
cumulative diff, and the verdict in the log.

## Traps

- **`vimMode` is `true` in this vault's `app.json`.** Scoria models the setting and writes it
  back, but nothing in the editor acts on it — there is no modal editing anywhere in
  `scoria-text`. So Obsidian's editor is modal this week and Scoria's is not, and the same
  keystrokes will not produce the same text. Left alone on purpose: it is the user's real
  setting, and the gate is about their real vault.
- **`workspace.json` is excluded everywhere**, in the harness and in every diff — but *not*
  because Scoria leaves it alone. It writes it on the way out, and day 1 found the stated
  reason was wrong; see [[#Day 1, 2026-11-04]].
- **`.claude/settings.local.json` is in the vault and in the snapshots.** If it moves, the
  author is Claude Code, not either app.
- **Obsidian rewrites `appearance.json`, `app.json` and `core-plugins.json` on its own** when
  a settings pane is merely opened. Expect those three in a diff and attribute them before
  reading anything into them.
- **The week cannot produce evidence about line endings or byte order marks.**
  All ten notes are uniformly LF, no BOM, and every one ends with a newline
  (surveyed 2026-11-04), so `an_edited_file_keeps_its_line_ending_and_byte_order_mark`
  is covered here by the corpus alone. The uniformity cuts both ways, though: a
  diff that shows a CRLF or a lost trailing newline is unambiguous. Adding one
  CRLF note and one BOM note to the vault would close the gap, at the cost of
  two files a person did not write.
- **Don't run `--stress` or `--synthetic` against this vault.** They rewrite notes; they are
  for scratch vaults (see [[Development#The gate run opens, 2026-10-18]]).

## Day 1, 2026-11-04

**The Scoria half passes.** Six notes, three in Source mode (`Tech Stack`, `Infrastructure`,
`Legacy Projects`) and three in Live Preview (`Business Model`, `Report Engine`,
`Data Architecture`), each typed into, saved, undone and saved again. After every undo all
seventeen files matched `plmcrzy-2026-11-04.sha256` exactly. Live Preview's display transform
does not leak into the bytes: its writes were a single added line and nothing else. Autosave
fired on its own two seconds after the last keystroke, with no explicit save, and wrote the
same bytes. A quit and relaunch read the note back unchanged. The app was built from HEAD
first — the release binary was thirteen app commits stale, and a week spent on a stale writer
would be a week wasted.

**`workspace.json` is the hole in this gate.** Quitting the app rewrote it, which contradicts
the reason it is excluded from the harness and from every diff — "Scoria neither models nor
writes it". `Workspace::save_layout` (`crates/scoria/src/workspace.rs:11625`) writes it
through `Vault::write_config`, "called on the way out, where Obsidian writes it". What is true
is narrower: Scoria does not model it *as a config type*. The layout goes out as a
`serde_json::Value` merged into the file's own JSON, so every key it did not set keeps its
bytes — which is the mitigation, and why nothing has gone visibly wrong.

The consequence was real: **the one file both apps write on every session was the only file the
gate never checked.** Excluding it from the daily diff remains right — both apps rewrite it
constantly, so it is noise there — but it belonged in the harness, and now it is there:
`every_workspace_json_is_written_back_byte_identical`, the sixteenth property. It round-trips
the file's own JSON through `Vault::write_config`, which is the half `scoria-vault` owns and the
half with the bytes in it — the two-space indent, the key order, the empty containers, a byte
order mark, the missing trailing newline, and how a number is spelled. What it cannot reach is
whether the *layout* Scoria computes is the layout Obsidian would compute; that needs the app
and belongs with the tab-group tests.

It runs on a frozen fixture, because every vault in the repository gitignores its own
`workspace.json` as live per-user state and a clean checkout would have nothing to test. The
fixture is the demo vault's, as Obsidian wrote it at 20:58 tonight, kept with a README recording
that provenance — a fixture Scoria wrote would make the test agree with itself.

The test was checked against deliberate breakage rather than trusted because it was green: a
trailing newline on the fixture fails it, and so does respelling a dock width
`315.19744110107420` or `3.151974411010742e2`, both of which `serde_json` re-renders. That last
one is the real reason to keep a float in the fixture — not `f32` narrowing, which is the typed
configs' problem, and which this path does not have.

The false reason is corrected where it is read as current truth — `writer_roundtrip.rs`'s
comment and `gate.sh`'s header. The dated sections of [[Development]] keep what was believed
when they were written, which is what a log is for.

**A driving lesson, not a bug.** `end` on a soft-wrapped paragraph goes to the end of the
*visual* row, so the first attempt typed into the middle of `[[Web Application]]` instead of
at the line's end. That is CodeMirror's behaviour and so Obsidian's, and
`scoria-text/src/wrap.rs` states the rule deliberately: "Home, End, Up, Down, the selection
highlight and the painted caret all have to agree". One `ctrl-z` restored the bytes exactly.
Use `ctrl-end`, or the find panel, to place a cursor without wrap ambiguity.

### The Obsidian half, and the verdict

Five notes appended to in Obsidian (`Business Model`, `Data Architecture`, `Infrastructure`,
`Legacy Projects`, `Tech Stack`), `vimMode` turned off, and `PLMCRZY Mindmap.canvas` opened.
**Day 1 passes:** everything the diff reported is a person's edit in one of the two apps. Nothing
moved that nobody touched, bar the two known cases below.

Three things the day bought the gate, all of which make the vault a better test than it was
this morning:

- **Five notes now end without a trailing newline.** Obsidian does not add one, so typing at
  the end of the last line leaves the file without it. That half-closes the gap recorded in the
  traps above, and the harness proves Scoria preserves the absence: with the vault named,
  `opening_every_note_and_saving_it_changes_no_bytes` passes over all five.
- **`app.json` now holds a non-default value** (`"vimMode": false`, written by Obsidian), so the
  config round trip covers a value a person actually changed rather than a default.
- **`PLMCRZY Mindmap.canvas` is now in Obsidian's own layout.** Obsidian rewrote the
  pretty-printed JSON into compact bytes on opening it; the content is identical to the key
  (29 nodes, 30 edges, semantically equal). A canvas authored elsewhere was skipped by the
  harness before; this one is not.

**The canvas failure was the harness's, not the writer's.** That rewritten canvas made
`every_canvas_obsidian_saved_is_written_back_byte_identical` fail, and the first reading — that
Scoria's canvas writer mangles bytes — was wrong. Scoria wrote
`id, type, text, x, y, width, height, color` for a file holding
`id, x, y, width, height, type, text, color`, which is exactly the reordering
`canvas_obsidian.rs::a_node_made_in_the_app_is_reordered_once_read_back` records Obsidian itself
performing on an edited save — observed on *this vault's canvas* on 2026-10-18. Scoria
reproduced Obsidian. What failed is the guard: `in_obsidians_layout` reads the formatting, not
the key order, so it cannot tell a fixed-point canvas from one Obsidian made in the app, and any
real vault holding the latter fails an assertion Obsidian would fail too.

The fix keeps the property strong rather than excusing the file. Where the bytes differ, two
things must now hold: the rewrite moved **only keys, never a value** (both sides compared as
JSON, which `serde_json`'s `preserve_order` plus `IndexMap`'s order-independent `PartialEq`
makes exactly the right comparison), and the reordering **settles in one pass** — the second
write is a fixed point. The canvas is reported, not silently skipped.

**Still open:** `workspace.json`, filed above, and the Obsidian-half protocol worked — so day 2
is files and links.

## The log

One row per day. "Moved" is what the diff reported; "author" is the answer to the two
questions above.

| Day | Date | Scoria half | Obsidian half | Moved | Author | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 2026-11-04 | — | — | nothing (vault inert since 10-22) | — | baseline `snapshot-2026-11-04` taken, harness green at 15 properties |
| 1 | 2026-11-04 | six notes, 3 source + 3 Live Preview: typed, saved, undone, saved | five notes appended to; `vimMode` off; `PLMCRZY Mindmap.canvas` opened | 5 notes +1 line each (no trailing newline now), `app.json`, the canvas rewritten compact, `workspace.json` | every one a person's edit in one of the two apps, bar the canvas (Obsidian normalizing a file authored elsewhere) and `workspace.json` (both apps, every session) | **passes** — nothing moved that nobody touched; the harness's canvas failure was the harness's, not the writer's |
| 2 |  |  |  |  |  |  |
| 3 |  |  |  |  |  |  |
| 4 |  |  |  |  |  |  |
| 5 |  |  |  |  |  |  |
| 6 |  |  |  |  |  |  |
| 7 |  |  |  |  |  |  |
