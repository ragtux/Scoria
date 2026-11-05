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

## Day 2, 2026-11-05

**Dated the 5th, driven on the night of the 4th.** The Scoria half ran 23:37–23:59 on 2026-11-04,
within half an hour of day 1 closing, because the day was asked for rather than waited for. That is
worth stating plainly: **two days of this week ran on one calendar night**, so the week is so far a
week of operations and not a week of elapsed time. Nothing in the gate's claim turns on the clock —
it turns on whether either app moves a byte nobody touched — but "a week of mixed use" reads as
seven days to anyone who has not read this paragraph.

**Every sheet item for day 2 ran in Scoria, and the writer is clean on all of them.** The deciding
evidence is below; the one failure the day found is a *default*, not a writer.

**The rename is byte-exact, and it is its own inverse.** `Data Architecture` → `Data Lake
Architecture`, the richest rename the vault has: 6 inbound links in 6 files, two of them aliased.
The notice said "Updated 6 links in 6 files." and the diff was exactly six one-line changes —
`[[Data Architecture|Build sheet data]]` → `[[Data Lake Architecture|Build sheet data]]` and
`[[Data Architecture|per-year archives]]` the same, so **both aliases survived**; the renamed note's
own bytes were untouched, its `# Data Architecture` heading included, which is right because
Obsidian renames the file and not the text in it. Renaming it back then restored
**every one of the six files to byte-identical with `snapshot-2026-11-04b`** — not one byte moved
across the pair. An operation and its inverse returning the original bytes is the same shape as
day 1's type-save-undo-save, and on a different write path.

**The link rewriter preserves a missing trailing newline.** `Infrastructure.md` ends `a` and
`Tech Stack.md` ends `1`, two of the five notes Obsidian left without a final newline on day 1;
both had a link rewritten and both still end that way, while `PLMCRZY.md`, which has one, kept it.
Day 1 proved the *editor* preserves this. The rewriter never goes through the editor — it reads a
file, replaces a link and writes it back — so this is a second write path held to the same property,
and the one the traps section above could not reach this week.

**A move rewrites nothing, as the format demands.** `Legacy Projects.md` (4 inbound links) into a
new `Archive/`: the file moved, its bytes identical, and all four linking files were left alone.
With `newLinkFormat` absent (so `shortest`) and a unique basename, `[[Legacy Projects]]` still
resolves from anywhere, so there is nothing to rewrite — which is what
`rename_without_updating_links_leaves_them_dangling` already asserts in the unit tests
(`rename_preview("B.md", "Sub/B.md")` is `(0, 0)`) and what Obsidian was observed doing on
2026-10-25. First time it has been shown on a real vault.

**A delete rewrites nothing either, and goes to the system trash.** `Day 2 Scratch.md` was deleted
with the confirmation `promptDelete` defaults to; it landed in `~/.local/share/Trash/files/`, which
is `trashOption`'s `system` default, and `Legacy Projects.md` — which links to it — came back with
the **same sha256** it had before. The link then reported as uncreated (`file-plus`, no path) in
Outgoing links. Obsidian does not rewrite links on a delete, and neither does Scoria.

**A note created from an unresolved link lands at the vault root, not beside its referrer.**
`[[Day 2 Scratch]]` was typed into `Archive/Legacy Projects.md` and followed from there; the note
was created at the root, which is `newFileLocation`'s `root` default — the folder of the current
file would have been `Archive/`. Typing the link also showed the one auto-pair rule supplying `]]`,
and appending to a note with no final newline left it with none.

**Two sort orders, checked against something other than the app.** `alphabetical` →
`alphabeticalReverse` → `byModifiedTime` from the Files header's menu, and under the last one the
rows matched `ls -t` exactly, with the `Archive` folder pinned first — folders do not move between
orders. Sort order lives in `workspace.json`, so it leaves no evidence in the diff.

### What day 2 found: `alwaysUpdateLinks` defaulted the wrong way

**Scoria rewrote six files where Obsidian would have written none without being asked.** The rename
above was silent. It should not have been: `app.json` in this vault holds only `vimMode`, so
`alwaysUpdateLinks` is absent, and `Workspace::update_links_automatically` read an absent key as
**on** (`unwrap_or(true)`). Obsidian reads it as off. Three things say so and none of them needed
the gate vault:

- Obsidian's own row for the setting is "Turn off to be prompted to update links after renaming a
  file" — so off is the prompting state, and the prompt is the thing a default decides.
- It rendered that toggle **off** on a vault whose `app.json` the probe wrote as `{}`
  (`tools/obsidian-probe/results/2026-10-25-ui-rules.json`), which is PLMCRZY's situation exactly.
- The 2026-10-25 drag pass, with the key set to `false` on purpose, saw the **Update links** modal
  on every rename that changed a basename.

The specification had it both ways: its file-operations line said the default was on, its own Files
and links table said the toggle is off. The table came from the probe, so the line was the wrong
half, and it is corrected.

**The consequence is the gate's own subject.** For one keystroke on one vault, Scoria wrote seven
files and Obsidian would have written one — the rename — unless the person pressed a button. That
is a byte divergence produced by a default, which is the quietest way this gate can be failed, and
it survived every one of the sixteen harness properties because the harness drives the writer
directly and never asks what the writer should have been allowed to do.

**The fix is two defaults, and it reveals a feature that was already built.**
`update_links_automatically` now reads an absent key as `false`, the settings row's default matches,
and `a_vault_with_no_always_update_links_key_asks_before_rewriting` pins it with the probe's
reasoning in the comment. Nothing else had to change: `rename_with_links` has had the whole
ask-path since the file operations landed — it previews the rename, skips the question when no link
would move, and opens an "Update links?" prompt otherwise — and **one inverted default had made all
of it dead code on every vault that has never written the key.** Rebuilt, the inverse rename on
PLMCRZY raised the prompt (`op: "update-links"`) for the first time, and confirming it restored the
six files byte-for-byte.

What the prompt is *not* is Obsidian's: two buttons where Obsidian has three, its own sentence, and
the file name spelled with its extension. That is filed as
[[UI parity backlog#The "Update links" prompt is two buttons and Scoria's own wording]] rather than
fixed tonight, because the missing third button — **Always update** — has to write
`alwaysUpdateLinks: true`, and `PromptSpec` has no field for a third action. It is worth having
inside the week: that button is the only path by which either app writes the key, so it is also the
only way the harness gets a real `app.json` holding it, which is the week's second job.

**Built the same day, before day 3.** The modal is now Obsidian's three buttons and its two lines,
and "Always update" writes the key. Driven on a scratch vault — never this one — all four answers
leave the bytes Obsidian's leave, and the rename after "Always update" asks nothing. It also turned
up a fourth answer Scoria had that Obsidian does not: Escape abandoned the rename, where Obsidian's
modal comes up *after* the file has moved, so dismissing it leaves the rename standing and the
links alone. See [[Development#The third button on the rename prompt, 2026-11-05]]. The week's
second job has its first entry; `templates.json`, `daily-notes.json`, `switcher.json`, `types.json`
and `hotkeys.json` are still owed by days 3, 4 and 7.

### One claim withdrawn before it was filed

The "Move file to…" chooser looked like it was missing Obsidian's vault-root row: with
`Legacy Projects.md` at the root it offered only `Archive`. With the file inside `Archive` it offers
`/`. The chooser excludes the folder the file is already in, and the first reading was of a chooser
doing the right thing. Whether Obsidian also excludes the current parent is unprobed — recorded as
a question, not a gap.

### The Obsidian half, and day 2's verdict

Run by hand, 2026-11-05 00:04–00:08. The probe harness could not do it:
`tools/obsidian-probe/README.md` forbids exactly this use — "Never point it at a real vault: not the
gate vault (`~/bstore2/raga/plmcrzy/PLMCRZY`) while a byte-level gate run is open" — and the rule is
right, because a probe script loose in the deciding vault would put a third author in a folder whose
whole value is that it has two. So this half is a person's, and two of its four answers are a
person's eyes.

**Day 2 passes.** Everything the diff reports is an edit someone made in one of the two apps.
Nothing moved that nobody touched, bar the three `.obsidian` files below, whose bytes did not move
at all.

**The two link rewriters are byte-identical on a real vault.** This is the day's sharpest result and
the one only Obsidian could settle. The 6 linking files were rewritten by Obsidian in a single batch
at `00:06:02.899`–`.906`, and `Data Architecture.md` itself was *not* touched — its mtime is still
day 1's `21:03:06`, which is a rename's signature, since renaming a file is a directory operation
and the links live in the other files. All six then verified **OK** against
`plmcrzy-2026-11-04b.sha256`. So the chain is:

| Step | Result |
| --- | --- |
| baseline `snapshot-2026-11-04b` | the six files as day 1 left them |
| Scoria renamed away and back | byte-identical to the baseline |
| Obsidian renamed away and back | **byte-identical to the baseline** |

Both rewriters are exact inverses of themselves, and they agree with each other on the same six
lines — `[[Data Architecture|Build sheet data]]` and `[[Data Architecture|per-year archives]]`
included, so they agree about aliases too. Only the last write's mtime survives, which is why two
renames show as one batch.

**The modal appeared, which settles the default on this vault.** Asked whether a dialog came up on
the rename, the answer was yes. That is the premise of the fix above, now **observed on PLMCRZY
rather than inferred from a scratch vault** — and it settles the toggle without anyone having to
find it, because the dialog only exists when the setting is off. `app.json` is the corroboration:
it still holds nothing but `vimMode`, so the button pressed was **Just once** and not Always
update, which would have written `alwaysUpdateLinks: true`.

Worth keeping in view: **the bytes alone could not have settled this.** "Default off, prompted,
answered Just once" and "default on, rewrote silently" both leave `app.json` without the key and
both leave the six files rewritten. The diff is blind to the difference; only a person watching the
screen is not. A gate that reads only bytes would have confirmed the wrong default.

**Obsidian renames on the row, not in a modal.** The new note is `this is a new note!.md`, 0 bytes,
created at `00:08:21`, and the name was typed **on the row in the file tree** — the
`contenteditable` label the 2026-10-25 pass recorded. Scoria opens a centre prompt for both F2 and
the header's New note. Filed as
[[UI parity backlog#The explorer renames in a modal where Obsidian edits the row in place]]; the
text model for it already exists in `title_field.rs`, written for the view header's inline rename
and kept reusable.

**The three config files Obsidian rewrites on its own, again.** `app.json`, `appearance.json` and
`core-plugins.json` all have mtimes of `00:04:28`–`00:04:29` and **identical content** — the trap
recorded above, fired by opening a settings pane. mtime moved, not one byte did, so they are
attributed and dismissed.

**What is left of the vault's day 2:** an empty `Archive/`, an empty `Day 2 New Note.md`, the empty
`this is a new note!.md`, and the `See also: [[Day 2 Scratch]]` line in `Legacy Projects.md` — the
vault's first unresolved link, which the later days can use. Every one of them a person's edit.

**Day 3 is tables and properties**, and it is the one that needs the vault's lack of frontmatter:
properties have to be created rather than edited, and setting a property's type is what writes
`types.json`, the first of the five config files the week owes the harness.

### What the day left in the vault

Deliberate, and all of it a person's edit: `Legacy Projects.md` carries a
`See also: [[Day 2 Scratch]]` line whose target was deleted, so the vault now has its first
unresolved link — useful for the later days; an empty `Archive/` folder and an empty
`Day 2 New Note.md`, the artifacts of the two Files-header items. Delete any of the three if you do
not want them; nothing in the run depends on them.

**No note of the user's was deleted.** The sheet says "delete one", and the note deleted was
`Day 2 Scratch.md`, created during the day for the purpose and linked from a real note so the
delete was still tested against inbound links. The narrowing costs the gate nothing *for bytes* —
Obsidian rewrites no links on a delete, so a delete's only byte is the file's own disappearance —
and it is recorded here rather than left as a silent substitution.

## The log

One row per day. "Moved" is what the diff reported; "author" is the answer to the two
questions above.

| Day | Date | Scoria half | Obsidian half | Moved | Author | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 2026-11-04 | — | — | nothing (vault inert since 10-22) | — | baseline `snapshot-2026-11-04` taken, harness green at 15 properties |
| 1 | 2026-11-04 | six notes, 3 source + 3 Live Preview: typed, saved, undone, saved | five notes appended to; `vimMode` off; `PLMCRZY Mindmap.canvas` opened | 5 notes +1 line each (no trailing newline now), `app.json`, the canvas rewritten compact, `workspace.json` | every one a person's edit in one of the two apps, bar the canvas (Obsidian normalizing a file authored elsewhere) and `workspace.json` (both apps, every session) | **passes** — nothing moved that nobody touched; the harness's canvas failure was the harness's, not the writer's |
| 2 | 2026-11-05 | rename with 6 aliased inbound links and back; move into a new folder and back; new folder; new note; note created from an unresolved link; delete; three sort orders | renamed `Data Architecture` and back (the modal appeared, **Just once**), new note named on the row; the three `.obsidian` files touched | the 6 link lines rewritten by each app and back to byte-identical both times; `Archive/`, `Day 2 New Note.md`, `this is a new note!.md`, one typed line in `Legacy Projects.md` | every one a person's edit in one of the two apps; the three config files moved mtimes and no bytes | **passes** — and the two link rewriters proved byte-identical on a real vault. `alwaysUpdateLinks` defaulted on where Obsidian defaults off, so Scoria's rename rewrote 6 files silently instead of asking; fixed, the ask-path it had made unreachable now runs, and the modal Obsidian showed confirms the default on this vault |
| 3 |  |  |  |  |  |  |
| 4 |  |  |  |  |  |  |
| 5 |  |  |  |  |  |  |
| 6 |  |  |  |  |  |  |
| 7 |  |  |  |  |  |  |
