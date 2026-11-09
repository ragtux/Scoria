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

**The mixed-use half has not run.** Not for want of a blocker: the gate vault was registered in
Obsidian on 2026-10-22 12:50 (which is what created its `app.json`, `appearance.json` and
`core-plugins.json`), so the one thing the October note was waiting on is done. The reason
is simpler — nothing has been edited in that vault since, so the clean
`diff -r snapshot-2026-10-25` is the diff of an inert folder. **A clean diff over a vault
nobody touched is zero evidence, not a pass.** That is the whole of what this week is for.

One thing the registration already bought: the gate vault's `.obsidian` now holds four modelled
files, so the config property covers a real person's set rather than passing vacuously the
way [[Development#Four small entries and a URI, 2026-11-02]] recorded.

## Two vaults, and which one decides

| Vault | Writers | Evidence | Weight |
| --- | --- | --- | --- |
| `$SCORIA_GATE_VAULT` | Scoria and Obsidian only | snapshot and `diff -r` | **the verdict rests here** |
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
`$SCORIA_GATE_DIR` and are deliberately copies rather than hashes alone: a hash
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

**Day 4 delivered two of them** — `templates.json` and `daily-notes.json`, written by Scoria on
2026-11-09 and rewritten by Obsidian the same night, one byte-identical and one with Obsidian's own
key appended to Scoria's bytes, see [[#Day 4, 2026-11-09]].

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
  it starts (days 2 and 3; day 4's Obsidian half opened Settings twice in a running session and
  they did not move). Expect those three in a diff and attribute them before reading anything
  into them.
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

**The Scoria half passes.** Six notes, three in Source mode (`Note A`, `Note B`,
`Note C`) and three in Live Preview (`Note D`, `Note E`,
`Note F`), each typed into, saved, undone and saved again. After every undo all
seventeen files matched `<vault>-2026-11-04.sha256` exactly. Live Preview's display transform
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
*visual* row, so the first attempt typed into the middle of `[[Note G]]` instead of
at the line's end. That is CodeMirror's behaviour and so Obsidian's, and
`scoria-text/src/wrap.rs` states the rule deliberately: "Home, End, Up, Down, the selection
highlight and the painted caret all have to agree". One `ctrl-z` restored the bytes exactly.
Use `ctrl-end`, or the find panel, to place a cursor without wrap ambiguity.

### The Obsidian half, and the verdict

Five notes appended to in Obsidian (`Note D`, `Note F`, `Note B`,
`Note C`, `Note A`), `vimMode` turned off, and `Mindmap.canvas` opened.
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
- **`Mindmap.canvas` is now in Obsidian's own layout.** Obsidian rewrote the
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

**The rename is byte-exact, and it is its own inverse.** `Note F` → `Note F renamed`, the richest rename the vault has: 6 inbound links in 6 files, two of them aliased.
The notice said "Updated 6 links in 6 files." and the diff was exactly six one-line changes —
`[[Note F|first alias]]` → `[[Note F renamed|first alias]]` and
`[[Note F|second alias]]` the same, so **both aliases survived**; the renamed note's
own bytes were untouched, its `# Note F` heading included, which is right because
Obsidian renames the file and not the text in it. Renaming it back then restored
**every one of the six files to byte-identical with `snapshot-2026-11-04b`** — not one byte moved
across the pair. An operation and its inverse returning the original bytes is the same shape as
day 1's type-save-undo-save, and on a different write path.

**The link rewriter preserves a missing trailing newline.** `Note B.md` ends `a` and
`Note A.md` ends `1`, two of the five notes Obsidian left without a final newline on day 1;
both had a link rewritten and both still end that way, while the vault's home note, which has one, kept it.
Day 1 proved the *editor* preserves this. The rewriter never goes through the editor — it reads a
file, replaces a link and writes it back — so this is a second write path held to the same property,
and the one the traps section above could not reach this week.

**A move rewrites nothing, as the format demands.** `Note C.md` (4 inbound links) into a
new `Archive/`: the file moved, its bytes identical, and all four linking files were left alone.
With `newLinkFormat` absent (so `shortest`) and a unique basename, `[[Note C]]` still
resolves from anywhere, so there is nothing to rewrite — which is what
`rename_without_updating_links_leaves_them_dangling` already asserts in the unit tests
(`rename_preview("B.md", "Sub/B.md")` is `(0, 0)`) and what Obsidian was observed doing on
2026-10-25. First time it has been shown on a real vault.

**A delete rewrites nothing either, and goes to the system trash.** `Day 2 Scratch.md` was deleted
with the confirmation `promptDelete` defaults to; it landed in `~/.local/share/Trash/files/`, which
is `trashOption`'s `system` default, and `Note C.md` — which links to it — came back with
the **same sha256** it had before. The link then reported as uncreated (`file-plus`, no path) in
Outgoing links. Obsidian does not rewrite links on a delete, and neither does Scoria.

**A note created from an unresolved link lands at the vault root, not beside its referrer.**
`[[Day 2 Scratch]]` was typed into `Archive/Note C.md` and followed from there; the note
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
  (`tools/obsidian-probe/results/2026-10-25-ui-rules.json`), which is the gate vault's situation exactly.
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
The gate vault raised the prompt (`op: "update-links"`) for the first time, and confirming it restored the
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
`Note C.md` at the root it offered only `Archive`. With the file inside `Archive` it offers
`/`. The chooser excludes the folder the file is already in, and the first reading was of a chooser
doing the right thing. Whether Obsidian also excludes the current parent is unprobed — recorded as
a question, not a gap.

### The Obsidian half, and day 2's verdict

Run by hand, 2026-11-05 00:04–00:08. The probe harness could not do it:
`tools/obsidian-probe/README.md` forbids exactly this use — "Never point it at a real vault: not the
gate vault (`$SCORIA_GATE_VAULT`) while a byte-level gate run is open" — and the rule is
right, because a probe script loose in the deciding vault would put a third author in a folder whose
whole value is that it has two. So this half is a person's, and two of its four answers are a
person's eyes.

**Day 2 passes.** Everything the diff reports is an edit someone made in one of the two apps.
Nothing moved that nobody touched, bar the three `.obsidian` files below, whose bytes did not move
at all.

**The two link rewriters are byte-identical on a real vault.** This is the day's sharpest result and
the one only Obsidian could settle. The 6 linking files were rewritten by Obsidian in a single batch
at `00:06:02.899`–`.906`, and `Note F.md` itself was *not* touched — its mtime is still
day 1's `21:03:06`, which is a rename's signature, since renaming a file is a directory operation
and the links live in the other files. All six then verified **OK** against
`<vault>-2026-11-04b.sha256`. So the chain is:

| Step | Result |
| --- | --- |
| baseline `snapshot-2026-11-04b` | the six files as day 1 left them |
| Scoria renamed away and back | byte-identical to the baseline |
| Obsidian renamed away and back | **byte-identical to the baseline** |

Both rewriters are exact inverses of themselves, and they agree with each other on the same six
lines — `[[Note F|first alias]]` and `[[Note F|second alias]]`
included, so they agree about aliases too. Only the last write's mtime survives, which is why two
renames show as one batch.

**The modal appeared, which settles the default on this vault.** Asked whether a dialog came up on
the rename, the answer was yes. That is the premise of the fix above, now **observed on the gate vault
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
`this is a new note!.md`, and the `See also: [[Day 2 Scratch]]` line in `Note C.md` — the
vault's first unresolved link, which the later days can use. Every one of them a person's edit.

**Day 3 is tables and properties**, and it is the one that needs the vault's lack of frontmatter:
properties have to be created rather than edited, and setting a property's type is what writes
`types.json`, the first of the five config files the week owes the harness.

### What the day left in the vault

Deliberate, and all of it a person's edit: `Note C.md` carries a
`See also: [[Day 2 Scratch]]` line whose target was deleted, so the vault now has its first
unresolved link — useful for the later days; an empty `Archive/` folder and an empty
`Day 2 New Note.md`, the artifacts of the two Files-header items. Delete any of the three if you do
not want them; nothing in the run depends on them.

**No note of the user's was deleted.** The sheet says "delete one", and the note deleted was
`Day 2 Scratch.md`, created during the day for the purpose and linked from a real note so the
delete was still tested against inbound links. The narrowing costs the gate nothing *for bytes* —
Obsidian rewrites no links on a delete, so a delete's only byte is the file's own disappearance —
and it is recorded here rather than left as a silent substitution.

## Day 3, 2026-11-08

**The Scoria half passes; the Obsidian half has not run.** Tables and properties, driven over the
control socket on `Note F`, the vault's note with the most tables, in Source mode with the cursor in
the first table's first body row. The binary was rebuilt from the working tree first, as on day 1.

**Every table command run is byte-exact against its own undo.** For each of fourteen of the fifteen commands the file on
disk was read after autosave, and compared with `snapshot-2026-11-05`, then the command was undone and
the file compared again: *add row above and below, move row down, move row up, duplicate row, delete
row, add column left and right, move column right, duplicate column, delete column, and align left,
center and right*. Every command moved bytes the way an edit should, and every undo returned the file
**byte-identical to the snapshot** (`cmp`, not a hash). Two things worth knowing about what they did:

- **Every table command re-pads the whole table.** Adding a row to a two-column table rewrote the header
  and the delimiter row to the widest cell (a header of two short cells became one padded to the widest cell in each column). That is Obsidian's behaviour too, and it is why the diff of a
  one-row edit shows the header changed.
- **`diff` hunks overstate a move.** Move row down reported `9,10c9,11`, which read like an added line.
  The line count was unchanged (91 before and after) and the rows had simply swapped.

Not covered: only one note's tables, and *Move column left* was not run (the cursor sat in the first
column, where it has nothing to move past). The other eight notes with tables are untested this week.

**One question for the Obsidian half.** *Move row up* on the first body row swapped it with the header:
the bold first body row became the header and the old header became a body row. Whether Obsidian
does that, refuses, or does nothing is unprobed. It is an edit, not a stray byte, so the gate is not
affected, but it is a question for a person at Obsidian.

**Properties.** `markdown:add-metadata-property` gave the note its first front matter:
`---`, `status:`, `---` and nothing else, the empty value Obsidian writes too. It is the only change to
the note's bytes this day. Folding it with `editor:toggle-fold-properties` and unfolding it again left
the note's bytes alone, as a fold lives in the view and not the file.

A driving error cost a few undos and is recorded because it looks like a writer fault: Enter after the
property's name commits the name with an empty value, so the "draft" typed next landed in the note body
after the closing fence, not in the value. Three `ctrl-z` returned the file to the front matter alone.
It was not the writer's.

**`types.json` now exists, and it took a real bug to get it.** Setting a property's type from the All
properties row's menu is what writes the file, and in Scoria that did nothing: *Property type ▸* opened
its submenu, a click on *List* closed the whole menu, and nothing was written. The cause was in
`file_menu.rs`. The menu closes on a press outside its list box, gpui decides "outside" from the list's
own bounds, and the submenu is drawn beside the list, outside them, so a press on a submenu row closed
the menu on the way down, before the click that picks the row arrived. The first fix, a flag set while the
pointer was over the submenu, did not work either: the flag was made afresh in every render, and gpui
reports a hover only when it changes, so after the re-render the submenu's own opening causes it read
`false` again. A temporary log of the three handlers showed `submenu hover true` followed by
`mouse_down_out over_submenu=false` on every click, which is what gave it away; the flag now outlives the
frame. Driven by hand afterwards, Checkbox and List wrote `"checkbox"` and `"multitext"`. The bug never
touched a vault's bytes: it was a menu that did nothing.

The file is `{ "types": { "status": "multitext" } }`, pretty-printed with a two-space indent. It is the
first real copy of `types.json` the harness has had, and `every_config_obsidian_saved_is_written_back_byte_identical`
now covers it: the sixteen properties pass with the vault named. It was written by Scoria, so it is
Scoria's idea of the format and not yet Obsidian's. The Obsidian half should set a type in Obsidian and
compare, which is the check that matters.

### The Obsidian half, 2026-11-08

Run by hand, 23:49–23:58, Obsidian 1.15.0 in Live Preview, on a note of the person's own making rather
than the one Scoria used: `Day 2 New Note` (day 2's empty artifact) renamed to `Day 3 New Note` on its
row, a five-column table typed into it, the table commands run from the palette, two rows appended, and
a line of text typed after it. Day 2's table note was left alone, which is right for attribution — its
only change this day is Scoria's front matter.

**The table bytes agree with Scoria's writer.** What Obsidian left is a table padded to its widest cell
with `---` at the minimum three, Align left written as `:--` at the delimiter's width, and the two
appended rows as `|     |` cells — the shapes recorded on 2026-10-18 and the ones `scoria_text::table`
writes; the harness opened and saved the note tonight without moving a byte (sixteen properties green
with the vault named). The commands ran without an undo between them, so the note holds their sum
rather than a round trip, and that is a person's edit either way.

**Move row up is not offered in the first body row.** The question day 3's Scoria half asked is answered
by a screenshot of the palette: caret in the first body row's first cell, query `move row`, and the
only table row is *Table: Move row down*. Obsidian's check fails there, so the first body row never
swaps with the header — which is what Scoria did, from the 1.14.4 plugin probe. It is an edit Scoria
would make and Obsidian would not, so it is fixed the same night: `table::apply` refuses it, and the
palette now lists a table command only when it would write something, see
[[UI parity backlog#Move row up swapped the first body row with the header]]. No dialog appeared at any
point of the half.

**The three config files moved mtimes and no bytes**, again, at 23:49 — the known trap.

**`types.json`, Obsidian's own, 00:29.** The first pass of the half never reached it — the note Obsidian
was on has no properties — so the table note was opened in Obsidian afterwards and its `status` given
Checkbox, then List. What happened was not quite that: the key was renamed rather than the value set,
`status` → `draft` (Checkbox, value `true`) → `scoria` (List, the `true` kept as a quoted string and
two items typed), and the file Obsidian wrote is the day's result:

```
{ "types": { "status": "date", "aliases": "aliases", "cssclasses": "multitext",
             "tags": "tags", "draft": "checkbox", "scoria": "multitext" } }
```

(pretty-printed, two spaces, no trailing newline — the shape Scoria's copy already had). Three things
in it. **Obsidian seeds `aliases`, `cssclasses` and `tags`** on writing the file, after the keys it
found and before the ones assigned; the specification had said so from the documentation, and Scoria's
writer had never done it — the `types.json` Scoria made on the 8th held one key. Fixed:
`PropertyTypes::seed_defaults` runs before every write, pinned by a test against these bytes. **A key
renamed in one note leaves its old entry** (`status` and `draft` both stayed), which is what Scoria
does for a per-note rename too. And **`status` is `date`**, where Scoria had left `datetime`: a pick
nobody reported, most likely the Date row on the way to Checkbox — the empty `Invalid date.md` created
at 00:26 is what a date-typed `status` holding `draft` renders as, followed as a link. Left in the
vault on purpose. The harness, rerun with the vault named, round-trips the six-key file and the new
front matter (`scoria:` with a quoted `"true"` and two items) byte for byte: sixteen properties green.

**Day 3 passes.** Every file the diff reports is a person's edit in one of the two apps, and the
day paid the week's second job its first real coin: a `types.json` in Obsidian's own bytes, which
showed Scoria's to be short by three entries — a divergence the harness could not have seen, since
it only ever read back what Scoria wrote.

## Day 4, 2026-11-09

**The Scoria half passes; the Obsidian half has not run.** Autocompletion, templates and daily notes,
driven over the control socket in a headless sway session, 00:43–00:51, on `Note C` (the one note in
the vault without a table) in Source mode with the cursor at the end of the file. The binary was
rebuilt from the working tree first, as on day 1: the release build was five source files stale. The
morning `gate-check` found nothing moved since `snapshot-2026-11-09` and sixteen properties green.

**All six suggesters ran, and each wrote what Obsidian writes.** The popup's rows were read from the
socket's `editor.suggest` after every keystroke, and the file after every accept:

- **`[[`** offered every file, notes before the two canvases, most recently modified first; a prefix
  narrowed it to `Note F`, and Enter wrote `[[Note F]]` — the `]]` the auto-pair had already put in was
  consumed, not doubled. Six `ctrl-z` returned the file **byte-identical to the snapshot** (`cmp`).
- **`[[Note F#`** listed the note's headings in document order, the H1 first; a query that matched one
  second-level heading made it the only row, and Enter wrote `[[Note F#Heading]]`.
- **`![[`** offered the same files; the second of the two canvas rows wrote
  `![[Name.canvas]]`, extension and all, which is how a canvas embeds.
- **`#`**: the vault had no tag, so neither `#` nor `#day4` opened a popup and the tag was left as typed.
  After the save that indexed it, `#d` offered `day4` and Enter wrote `#day4 ` — with the trailing
  space Obsidian's tag accept writes.
- **An alias.** The vault had none, so `aliases:` and `  - lake` were typed into `Note F`'s front matter
  in Source mode (two lines added, nothing else in the file moved). Back in `Note C`, `[[lak` offered
  the alias as its one row and Enter wrote `[[Note F|lake]]`.
- **`[[Note F#^`** listed `Note F`'s blocks by their text — headings, paragraphs, tables and list items,
  each list item once on its own and once inside its parent. A query narrowed it to the first paragraph,
  and Enter wrote `[[Note F#^bf1a61]]` in `Note C` **and ` ^bf1a61` onto the end of that paragraph in
  `Note F`**, a note not in front at the time. That is the one path this week that writes into a file
  other than the one being edited: the id went inline after the paragraph's last character, and the
  98-line file's diff is the two alias lines and that one line.

**Three driving faults, no writer faults.** A heading query that matched nothing closed the popup, so
the next Enter was a newline inside the link and the auto-paired `]]` rode along onto the lines typed
after it; that is CodeMirror's behaviour and so Obsidian's. Ten `ctrl-z` returned the note byte-identical
to the snapshot — which is evidence of its own: ten mixed edits (typing, two accepts, newlines) undone
to the original bytes. The tag suggester offered nothing for `#d` before the file was saved, because it
reads the cache and the cache reads the file; Enter on a closed popup is again a newline. The third is
under daily notes.

**Templates.** `Templates/` came from `file-explorer:new-folder` (the prompt, the name typed, Create).
*New note* on the folder's context menu created `Templates/Untitled.md` and opened the rename prompt on
it; renamed there to `Day 4 Template`, then three lines typed in Source mode — `# {{title}}`, a blank
line, `Created {{date}} at {{time}} from the template.` and a final newline. In Settings → Templates
the folder field took `Templates` and Enter, and **`templates.json` existed**:
`{\n  "folder": "Templates"\n}`. Time format `HH:mm:ss` was then appended as `"timeFormat"` — the key
in Obsidian's spelling, two-space indent, no trailing newline, the shape probed on 2026-10-30.
`insert-template` in `Note C` opened the chooser with the folder's one row; Enter inserted the three lines
with `{{title}}` as the note's name, `{{date}}` as `2026-11-09` (the field left empty, so the default)
and `{{time}}` as `00:50:16` in the format just set; the template's own final newline came with it.

**Daily notes.** In Settings → Daily notes the Date format dropdown's second row (labelled with today,
`2026.11.09`) wrote `{"format": "YYYY.MM.DD"}` — the pattern, not the label — then New file location
`Daily` added `"folder"` and Template file location `Templates/Day 4 Template` added `"template"`, the
keys in the order they were written; **`daily-notes.json` existed** after the first of the three.
Then `daily-notes` with no `Daily/` folder yet, and **nothing was created**. The script had not read
the notice and went on; the next step made the folder, and only afterwards was the reason looked up:
Obsidian writes nothing when the folder is missing and says
*Failed to create daily note. Folder “Daily” not found.*, which `the_configured_folder_has_to_exist_already`
pins and Scoria reports the same way. A refusal Obsidian makes, and a driver that did not look.

The folder came from a link: `Yesterday: [[Daily/2026.11.08` typed into `Note C` (the suggester offered
the path as its literal row), and `editor:follow-link` created `Daily/2026.11.08.md`, empty, folder and
all — a link that names a folder creates in that folder, where day 2's bare name went to the root.
`daily-notes` then created `Daily/2026.11.09.md` from the template, 64 bytes:
`# 2026.11.09`, a blank line, `Created 2026-11-09 at 00:50:35 from the template.` and the final newline.
`daily-notes:goto-prev` opened the 8th; again, *No previous daily note*. `goto-next` opened the 9th;
again, *No next daily note*. A second `daily-notes` with today's note in place opened it and wrote
nothing: both notes kept their bytes and their mtimes through the stepping.

**What the diff reports, 00:51.** `Daily/` (two notes), `Templates/` (one), `Note C` (ten lines appended
after the day-1 line, the file still without a final newline because the last line typed has none),
`Note F` (two front-matter lines and one block id), `daily-notes.json` and `templates.json`. Every one
is Scoria's, and every one a person's edit — the block id included, since it is the byte Obsidian also
writes into a target on accepting a block. The three config files Obsidian touches when a settings pane
opens did not move: Obsidian did not run. `workspace.json` was rewritten on quit, excluded as always.

**The harness, with the vault named, is sixteen green** — and `every_config_file_is_written_back_byte_identical`
now reads a real `templates.json` and a real `daily-notes.json`, the two files
[[#What the week has to produce beyond a diff]] asked day 4 for. Both are Scoria's bytes, though, which
is what day 3's `types.json` showed to be worth less than it looks: Scoria's writer agreed with itself
for three weeks and was short three keys. The Obsidian half should open both tabs and change a field in
each, and that comparison is the one that counts.

**For the Obsidian half.** (1) Settings → Templates and Daily notes: change one field on each and compare
key order and spelling. (2) Insert the template in Obsidian: whether `{{time}}` honours `HH:mm:ss`, and
whether the trailing newline comes along. (3) Open today's daily note from the ribbon or the command and
step back and forward. (4) `[[Note F#^` on the paragraph that now carries `^bf1a61`: Obsidian should offer
the existing id and write no second one; accept a block that has none and compare where the id lands.
(5) `#d` should offer `day4`.

### What the day left in the vault

All of it deliberate and a person's edit: `Templates/Day 4 Template.md`, an empty `Daily/2026.11.08.md`
and the templated `Daily/2026.11.09.md`; ten lines on the end of `Note C` (the five accepted completions,
the inserted template and the link to the 8th); on `Note F` the alias `lake` and the block id `^bf1a61`;
and the two config files with the folders, formats and template path above. Delete any of it after the
Obsidian half; nothing later in the run depends on it.

### The Obsidian half, 2026-11-09

Run by hand, 01:01–01:11, Obsidian 1.15.0, from the list the Scoria half left; attribution is by
mtime and bytes, since the diff against `snapshot-2026-11-09` holds both halves.

**`templates.json` came back with Scoria's bytes kept, 01:01.** Time format changed to `HH:mm` and back,
then Date format set to `YYYY-MM-DD`, a key Scoria had not written. Obsidian's file is
`{\n  "folder": "Templates",\n  "timeFormat": "HH:mm:ss",\n  "dateFormat": "YYYY-MM-DD"\n}`: Scoria's two
keys byte for byte and in Scoria's order, the new key appended, two-space indent, no final newline.

**`daily-notes.json` came back byte-identical, 01:02.** The Date format dropdown was moved and set back to
`2026.11.09`; the mtime moved and **not one of the 91 bytes did**. That is the comparison day 3's
`types.json` taught the run to insist on — Obsidian rewriting a file Scoria wrote — and this time it
found nothing.

**The template inserted in `Note C`, 01:05.** `# Note C`, a blank line, `Created 2026-11-09 at 01:05:27
from the template.` and a final newline: `{{time}}` rendered in the `HH:mm:ss` Scoria had written, so
Obsidian read Scoria's `templates.json` and acted on it, and the template's trailing newline came along
as it did in Scoria's insertion. A blank line precedes the block (a person's Enter, or two; not
recorded). `Note C` now ends in a newline for the first time since day 1, because the template's last
byte is now the file's.

**Daily notes, 01:11.** Today's note was opened, not recreated — its 00:50:35 template line is still its
first text — and `we are here!` typed into it; previous and next were stepped, and the 8th's empty
file kept its 00:50:34 mtime, so the stepping wrote nothing.

**The suggesters ran in today's daily note rather than in `Note C`**, which the mtimes say and the
diff of `Note C` (only the template) confirms. Every accepted form is the one Scoria wrote in the
morning: `[[Note F#Heading]]`, `![[Name.canvas]]`, `#day4 ` with the trailing space, and
`[[Note F|lake]]` — the alias Scoria typed into `Note F`'s front matter was offered by Obsidian's
suggester, so the two apps read each other's front matter. The block row picked was a heading rather
than the paragraph that carries `^bf1a61`, so whether Obsidian reuses an existing id stays open; what
it answered instead is the other placement. **Obsidian wrote `^71cf71` under the heading**, turning
`## H\n\n| table` into `## H\n\n^71cf71\n\n| table` — the shape
[[Specification#Observed block reference behavior, Obsidian 1.14.4]] records and
`an_own_line_id_takes_a_blank_line_either_side` pins, so Scoria would have written the same bytes.
Both block-id placements have now been seen on the real vault, inline from Scoria and own-line from
Obsidian, each the other's shape.

**The three config files did not move.** `app.json`, `appearance.json` and `core-plugins.json` still
carry 23:49:07 from the 8th, though Settings was opened twice tonight. The trap above says a pane
"merely opened" rewrites them; days 2 and 3's moves each came with Obsidian being *started*, and
tonight it had been running since 23:49. The trap is narrower than written: launch, not the pane.

**Step 6, the missing folder, 01:20–01:29.** Run after the day's baseline was taken, so it is
attributed against `snapshot-2026-11-09b`. New file location set to `nowhere`, the daily-note button
pressed: Obsidian wrote nothing and said *Failed to create daily note. Folder "nowhere" not found.*
(the person's transcription; whether the quotes are the curly ones the 1.14.4 probe saw was not
checked). That is the refusal Scoria made at 00:50 and the text `the_configured_folder_has_to_exist_already`
pins. The field was then set back to `Daily`, and `daily-notes.json` was rewritten byte-identical,
only its mtime moved. The person first reported no *New file location* row on 1.15.0's tab, then found it: the
tab's rows are as the 1.14.5 probe recorded them, and the specification stands.

One more file from the same quarter hour: `.trash/Untitled.canvas`, empty, 01:20:46 — a canvas
created and deleted in Obsidian while looking for the row (to be confirmed by the person). Worth a
line because of *where* it went: Obsidian put it in the vault's own `.trash` although `app.json`
holds no `trashOption`, where Scoria's day-2 delete under the same default went to the system
trash. Obsidian falls back to `.trash` when the system trash refuses, and this vault sits on a
filesystem where it evidently does for Obsidian and not for Scoria. A person's deletion either way,
not a stray byte, but the two apps' deletes do not land in the same place on this machine.
`workspace.json` moved at 01:07 and again after, excluded as always.

**Day 4 passes.** Every file in the diff is a person's edit in one of the two apps, and the day's
second job closed properly: Scoria wrote the week's first `templates.json` and `daily-notes.json`,
Obsidian rewrote both, and returned one byte-identical and the other with its own key appended to
Scoria's bytes. The harness, rerun with the vault named after both halves, is sixteen green over
Obsidian's copies of both files, both daily notes and the two block ids.

## Day 5, 2026-11-09

**Day 5 passes: Obsidian 1.15.0 kept every line Scoria's canvas writer had written.** The Scoria half
first. Both canvases, driven over the
control socket in a headless sway session, 01:17–01:42, from the baseline `snapshot-2026-11-09c`.
Window coordinates for a node came from the socket's `canvas` dump (`origin`, `size`, `viewport`,
`scale`) and the file's own `x, y, width, height`, so every click was aimed at a known card. The two
files are not alike, and that is the day's subject: `Canvas 1` was authored outside Obsidian and keeps
its author's key order (`id, type, text, x, y, …`); `Canvas 2` is the one Obsidian rewrote compact on
day 1, in the order its pretty-printed source had (`id, x, y, width, height, type, text, color`).

**Every operation on `Canvas 1` changed its own line and undid to byte-identical.** Pan (middle-button
drag and the wheel) and zoom (`ctrl-=`, `ctrl--`, `shift-1`) moved the view and not a byte: the
viewport lives in `workspace.json`, as Obsidian keeps it. A left drag on the background is a marquee,
not a pan, which is Obsidian's rule too. Then, each followed by a save, a `cmp` against the snapshot and
one `ctrl-z`:

- **Move** by 40 by 20 screen pixels at scale 0.475 (84 by 42 units) landed at 80 by 40: snapped to
  the 20-unit grid. One line changed, `x` and `y` only.
- **Resize** from the bottom-right handle, 63 by 32 units asked for, 70 by 30 written: the same snap.
- **Colour** from the floating menu's palette: `"color":"4"` became `"6"` in place.
- **An edge** dragged from a card's top connector onto another card: appended after the last edge as
  `{"id":<16 hex>,"fromNode","fromSide","toNode","toSide"}`, the shape Obsidian writes, no `fromEnd`
  or `toEnd` since both are defaults.
- **A group** over two shift-selected cards (`ctrl-g`): a `type: group` node put *first* in the array
  (groups are read first and written first), bounds the cards' union padded by 40, labelled
  `Untitled group` as [[Specification#Observed canvas view behavior, Obsidian 1.14.4]] records.
- **Duplicate** (`ctrl-d`) appended a copy offset 20 by 20 with a fresh id.
- **Typing into a card**: a double click opened the card editor (the socket's `focus` reads
  `canvas-editor`), `ctrl-end` and ` (day 5)` typed, Escape committed it; the `text` string changed and
  nothing else.

Every undo returned the file byte-identical, the group and the edge included. Left on `Canvas 1` for
the Obsidian half: the typed suffix, the colour and the edge — three lines of a 41-line file.

**`Canvas 2`'s first edit rewrote every node line, and the specification says Obsidian's would too.**
Moving one card changed all 29 node lines: each came back as `id, type, text, x, y, width, height,
color`, and ten undos could not make the file byte-identical again, because the order is not in the
undo history. The data was (every node and edge equal to the snapshot's as JSON). This is the rule
[[Specification#Observed canvas write behavior, Obsidian 1.14.4 (installer build 1.13.7)]] records
from the 2026-10-19 probe and `scoria-formats/tests/canvas/scrambled.out.canvas` holds: *closing* a
canvas rewrites it from the data as loaded, keys in file order — which is what Obsidian did to this
file on day 1 — while an *edited* save rebuilds every object, the model's keys (`x, y, width, height,
color`) removed from the file order and appended after the rest. Obsidian's own fixed order is a fixed
point of that rule; the day-1 order is not, so the first edited save by either app moves every line
once, and never again. Scoria wrote exactly what the rule predicts.

So the question is not whether Scoria diverged from the specification but whether Obsidian 1.15.0
still follows it. **The Obsidian half decides**: move one node of `Canvas 2` in Obsidian. If the file
comes back as Scoria's bytes bar that node's line, the rule holds and the day passes; if Obsidian keeps
`id, x, y, …`, the version moved under the probe and Scoria's writer has 28 lines to answer for.
`Canvas 2`'s other operations — colour, an edge, typing into a card — each changed one line against
Scoria's first save and undid to it byte-identical; one move (80 by 40 units) is left on it.

**What could not be driven.** Copy and paste go through the system clipboard, and under the headless
session's synthetic input the clipboard takes no write (the trap [[Development]] recorded on
2026-10-22): `ctrl-c`, `ctrl-v` left the file untouched and raised no notice. Duplicate stood in for
the write path; the clipboard path wants a person at the desk, one `ctrl-c`, `ctrl-v` on a card in
Scoria. Three driving artefacts, none a writer's: the socket's click modifier is `mods`, not
`modifiers`, so the first shift-click selected one card and `ctrl-g` grouped it alone (group bounds
are the one card padded 40, consistent with the pair); a drop on a card's exact centre chose `toSide:
right` by `facing_side`'s tie-break, which no hand reproduces; and at scale 0.335 a 50-unit-tall card
is 17 pixels, so its centre is inside the 10-pixel connector hit and a click there drew an edge into
empty space and made a new card, as Obsidian's connector does — undone with the rest.

### The Obsidian half, 2026-11-09

**One card of `Canvas 2` moved by hand, and the file came back as Scoria's bytes bar that card's
line.** Obsidian saved it at 09:16:28 (`workspace.json` eight seconds before). The diff against
`snapshot-2026-11-09c` is the 29 node lines, and reading it next to what Scoria had left: 28 of the 29
are Scoria's lines, byte for byte, in Scoria's order, the edited-save order `id, type, text, x, y,
width, height, color` included; the edges and the file's head and tail did not change. So the rule the
specification records from the 1.14.4 probe holds in 1.15.0, Scoria's writer is a fixed point of
Obsidian's, and the 28 lines the morning had to answer for are answered: Obsidian would have rewritten
them the same way on its own first edit, and did not touch them on the second.

**The moved card's line went to the end of the array.** It moved 180 by 200 units, on the 20-unit grid,
and its line left its place among the nodes (eleventh) for the last. The array is the z-order, and
Obsidian's drag lifts the dragged card to the top; nothing on the sheet asked for it, and no probe had
recorded it, because the 2026-10-19 probe moved nodes through the API, not by hand. **Scoria's drag did
not do this**: the morning's move on `Canvas 2` left its card where it was in the array, and so did
every move on `Canvas 1`. A person dragging a card in Scoria and then in Obsidian would have had the
two apps write different files for the same gesture. `scoria-canvas` already had `raise_selection`
(the spec notes it has no menu home in 1.14.5); the view never called it. Fixed today: the first
movement of a drag past the threshold raises the selection, inside the move's own undo step, so the
undo puts the order back with the position (`crates/scoria/src/canvas/view.rs`, test
`a_raise_inside_a_move_is_one_undo_step_with_it`). A card already on top records no step. Whether a
click that selects without dragging raises too is unprobed, and the spec says so:
[[Specification#Observed canvas write behavior, Obsidian 1.14.4 (installer build 1.13.7)]].

**`Canvas 1` was not written by Obsidian**: its mtime is still Scoria's 01:37 save, so it was not
opened and closed there (an unedited close would have rewritten it). The day's sheet asked for one
node moved, and `Canvas 2` supplied the one that mattered. Scoria's three lines on `Canvas 1` (the
typed suffix, the colour, the edge) stand for Obsidian to read another day. Copy and paste on a card
still want the desk's clipboard; the write path they share with duplicate is covered.

**The harness**: sixteen properties green over the vault after the Obsidian half, at 09:30. The
verdict is in the log. The vault goes to `gate.sh baseline` as `snapshot-2026-11-09d`.

## Day 6, 2026-11-09

**The Scoria half passes; the Obsidian half has not run.** Editing commands and folding, driven over the
control socket in a headless sway session, 09:28–09:39, on `Note B` (the one ending in `a` with no final
newline: 95 lines, eleven headings, four flat bullet lists under the `###` headings and the H2s, and no
checkbox or nested list anywhere — the vault has neither, so both had to be made). Source mode for the
commands, both modes for the folds. The binary was rebuilt from the working tree first, as every day: day
5's canvas fix was in the tree and not in the binary. The morning `gate-check` found nothing moved since
`snapshot-2026-11-09d` and sixteen properties green.

**Every editing command wrote the spec's bytes, and every round trip came back byte-identical** — most
of them without undo: a three-press cycle, a pair of toggles, a colour applied and removed. The check
after every command was a save and a byte comparison of the file on disk against the snapshot, not a read
of the buffer. Line numbers below are the socket's, zero-based.

- **`cycle-list-checklist`** on a bullet: `- [ ] `, `- [x] `, bullet — three presses and the file was
  byte-identical to the snapshot again, no undo. A paragraph became `- `, `- [ ] `, `- [x] `, the caret
  riding from ch 3 to ch 9; an empty line `- `, `- [ ] `, `- [x] `. Three undos each, byte-identical.
- **Mod+L as the keystroke `ctrl-l`**: `- [ ] ` on a bullet with no box, then `[x]`, then `[ ]`, the
  caret moving with the insert; `- [ ] ### …` on a heading line; over three bullets all gain `- [ ] `,
  all go `[x]`, all go `[ ]`; the mixed selection (one with a box, two without) inverted each line and
  swapped back on the second press — the period-two row, as recorded.
- **The six colours**: plain `==w==`, red over it inserted the emoji, blue replaced it in place, blue
  again removed the whole highlight — four commands, no undo, byte-identical. Each colour with a bare
  caret on a word inside parentheses wrote `==<emoji>w==` and the same colour again removed it. Over two
  whole bullet lines a colour wraps per line, `==🟢- …==` on each. The plain toggle with a bare caret,
  twice, byte-identical.
- **Clear formatting**: a bullet line with a code span lost its `- ` and its backticks; `## heading`
  became its text; a line whose only markup is a wikilink did not change; the coloured highlight lost
  marks, emoji and the line's `- ` together; a bare caret inside `==…==` changed nothing.
- **The inline toggles over two whole bullet lines**: bold, italics, strikethrough, code and inline math
  each wrapped each line *marker included* (`**- text**`), comments wrapped the pair as one; the second
  press restored the bytes in all six. A selection from mid-line to mid-line took the markers at its
  edges (`- one **two three**` / `**- four fi**ve` for a selection from `two` to inside `five`); a bare caret took the word under it.
- **The line toggles over four bullets**: bullet removed and restored; blockquote wrote `> - …` and
  restored; numbered replaced the bullets with `1.`–`4.`, and the second press left bare lines, not
  bullets — that pair does not round-trip, two undos do. Three bullets and a bare line: the bare one
  gains. A blank line inside the selection: blockquote prefixed it `> `, numbered skipped it and went on
  to number the heading after it (`2. ## …`).
- **Shift+Tab**, `useTab` and `tabSize` absent from `app.json` (so tab, 4): Tab wrote `\t`, Shift+Tab
  took it back; three spaces went to none, five to one, `\t ` to one space, three spaces before a
  paragraph to none, a top-level item did not change; `indent-list` and `unindent-list` gave the same
  bytes as the keys.

**Folding wrote nothing.** The five commands in Source mode and again in Live Preview, the note's bytes
compared after each: `toggle-fold` on the H2 at 66 recorded `{66,72}`, the trailing blank included; on
the list parent at 68 (made by the two Tabs below) `{68,70}`, its last child; on a paragraph nothing.
`fold-more` from a bullet at 50 folded `{49,53}`, then `{41,65}`, then `{0,94}`, then nothing more;
`fold-less` peeled the outermost first. `fold-all` set twelve ranges, one per foldable line, and
`unfold-all` none. The last heading folded to 94, the end of the file, and the first to 94 as well. The
folds crossed the mode switch; reading view showed the folded H2 collapsed behind its indicator
(screenshot), and the editor read the same three folds at full extent on the way back. Leaving the note
wrote `~/.config/scoria/note-fold/<id>.json` in Obsidian's shape (`{"folds":[{from,to}…],"lines":95}`),
outside the vault; coming back restored them; `unfold-all` before quitting emptied the store to `{}`.

**Two faults found and fixed, neither a writer's.** (1) `fold-more` up to the H1 with the view scrolled
to the note's middle left the pane blank: the scroll anchor was inside the fold and nothing was drawn,
not even the placeholder. `refresh_folds` now moves the anchor to the hiding fold's own line
(`fold::hiding_line`, `crates/scoria/src/fold.rs`), verified live in both modes: the one `# …` line.
(2) Clear formatting over a selection with nothing to clear pushed an empty undo step — the prefix pass
emitted an empty cut for every selected line without a prefix — so the `ctrl-z` after it did nothing.
Found because an undo count came out one short. `clear_formatting_plan` drops empty cuts now, with
tests in `format.rs` and `buffer.rs` (`clear_formatting_needs_a_selection_and_keeps_it`).

**Two driving artefacts.** Offsets computed from the file on disk before the buffer was saved put the
caret on the wrong line once: the first attempt to leave `- [x]` on 44 flipped it back to `- [ ]`
(the helper saves first now). And `set_selection` onto a hidden line — the socket can put the caret
inside a closed fold, which a person cannot — left the fold closed until the next reveal, a mode switch,
which opened the fold hiding the caret and looked like the switch dropping a heading fold. With the
caret on a visible line both nested folds survived the switch in both directions.

**What the diff reports, 09:39.** `Note B` alone, six lines: `- [x]` on 44 (the cycle, twice),
`- [ ]` on 45 (Mod+L), a plain highlight on a word of 46, a yellow highlight on a word of 56, and a tab
before 69 and 70 (the nested pair). Every one is Scoria's, and every one a person's edit. The three
config files Obsidian touches did not move: Obsidian did not run. `workspace.json` was rewritten on
quit, excluded as always. The harness, with the vault named, is sixteen green after the half.

**For the Obsidian half.** (1) Open `Note B`: the yellow highlight should render without its emoji, the
two boxes as checkboxes, the nested pair under a list parent with a fold indicator. (2) Click the box on
44 in reading view (`- [ ]`) and Mod+L on 45 (`- [x]`), and compare bytes. (3) `toggle-highlight-yellow`
on the yellow word removes it; a colour over the plain highlight on 46 inserts an emoji. (4) Toggle bold
over two whole bullet lines: does Obsidian wrap the `- ` too (`**- text**`)? The probe's rows were plain
lines. (5) `toggle-numbered-list` over bullets and again: `1.` then bare lines, or the bullets back?
(6) Shift+Tab the two nested lines back and compare bytes. (7) Fold the H2 and the list parent, switch
modes, and `fold-more` from inside a section with the view scrolled down.

## The log

One row per day. "Moved" is what the diff reported; "author" is the answer to the two
questions above.

| Day | Date | Scoria half | Obsidian half | Moved | Author | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 2026-11-04 | — | — | nothing (vault inert since 10-22) | — | baseline `snapshot-2026-11-04` taken, harness green at 15 properties |
| 1 | 2026-11-04 | six notes, 3 source + 3 Live Preview: typed, saved, undone, saved | five notes appended to; `vimMode` off; `Mindmap.canvas` opened | 5 notes +1 line each (no trailing newline now), `app.json`, the canvas rewritten compact, `workspace.json` | every one a person's edit in one of the two apps, bar the canvas (Obsidian normalizing a file authored elsewhere) and `workspace.json` (both apps, every session) | **passes** — nothing moved that nobody touched; the harness's canvas failure was the harness's, not the writer's |
| 2 | 2026-11-05 | rename with 6 aliased inbound links and back; move into a new folder and back; new folder; new note; note created from an unresolved link; delete; three sort orders | renamed `Note F` and back (the modal appeared, **Just once**), new note named on the row; the three `.obsidian` files touched | the 6 link lines rewritten by each app and back to byte-identical both times; `Archive/`, `Day 2 New Note.md`, `this is a new note!.md`, one typed line in `Note C.md` | every one a person's edit in one of the two apps; the three config files moved mtimes and no bytes | **passes** — and the two link rewriters proved byte-identical on a real vault. `alwaysUpdateLinks` defaulted on where Obsidian defaults off, so Scoria's rename rewrote 6 files silently instead of asking; fixed, the ask-path it had made unreachable now runs, and the modal Obsidian showed confirms the default on this vault |
| 3 | 2026-11-08 | 14 of the 15 table commands on `Note F`, each checked and undone to byte-identical; first front matter added, folded and unfolded; property type set from the sidebar menu (Checkbox, then List) | `Day 2 New Note` renamed on the row to `Day 3 New Note` and a table built and commanded in it; `Note F`'s property renamed twice with a type each time, an empty `Invalid date.md` on the way; the three `.obsidian` files touched | `Note F`'s front matter (Scoria's `status:`, then Obsidian's `scoria:` list), the renamed note with its table, `Invalid date.md`, `types.json` (Scoria's one key, then Obsidian's six) | every one a person's edit in one of the two apps; the three config files moved mtimes and no bytes | **passes** — Obsidian's `types.json` showed Scoria's unseeded (fixed: the three defaults on every write), the palette showed Move row up hidden in the first body row (fixed), and a submenu bug fell on the Scoria half |
| 4 | 2026-11-09 | all six suggesters accepted into `Note C` (`[[`, `[[Note#`, `![[`, `#tag`, an alias added to `Note F` for the purpose, a block whose id was written into `Note F`), the first accept and a ten-edit stretch undone to byte-identical; `Templates/` and a template note made in the UI, `templates.json` and `daily-notes.json` written from Settings, the template inserted, yesterday's note made by following a `[[Daily/…]]` link, today's created from the template, stepped back and forward | Settings: Templates time format changed and restored and a date format added, Daily notes dropdown moved and restored; the template inserted in `Note C`; today's note opened, typed into, stepped back and forward; the five suggesters accepted in today's daily note, the block one writing an own-line id under a heading of `Note F` | `Templates/` and `Daily/` (three notes), `Note C` (the morning's ten lines, then Obsidian's template), `Note F` (two alias lines, Scoria's inline id, Obsidian's own-line id), `templates.json` (Scoria's two keys, Obsidian's third appended), `daily-notes.json` (rewritten by Obsidian byte-identical, twice), `.trash/Untitled.canvas` (empty) | every one a person's edit in one of the two apps; the three config files Obsidian rewrites at launch did not move (it was not relaunched) | **passes** — Obsidian rewrote both of the config files Scoria had created and kept Scoria's bytes; the two apps read each other's alias and template settings, and both refuse a daily note whose folder is missing with the same notice; the 1.15.0 Daily notes tab matches the spec, and Obsidian's delete went to `.trash` where Scoria's went to the system trash |
| 5 | 2026-11-09 | both canvases: pan, zoom, select, move, resize, colour, an edge, a group, duplicate (copy/paste needs the desk's clipboard), undo and typing into a card, each undone to byte-identical; `Canvas 2`'s first edit rewrote every node line into the edited-save key order the spec records, the data unchanged | one card of `Canvas 2` moved by hand; `Canvas 1` not opened | `Canvas 1`: Scoria's three lines (the typed suffix, a colour, an edge); `Canvas 2`: every node line once, into the edited-save order, then the moved card's line to the end of the array; `workspace.json` | every one a person's edit in one of the two apps; the key-order rewrite is the spec's rule, done once by Scoria and accepted by Obsidian 1.15.0, which kept the 28 other lines byte for byte | **passes** — Scoria's canvas writer is a fixed point of Obsidian's; the moved card came back last in the array, which Scoria's drag did not do (fixed: the first movement of a drag raises the selection, undone with the move); copy/paste waits on the desk |
| 6 | 2026-11-09 | on `Note B`: the checklist cycle, Mod+L as a keystroke, the six colours and the plain highlight, Clear formatting, the six inline and three line toggles, Shift+Tab's arithmetic — each checked on disk and round-tripped byte-identical, most without undo; the five fold commands in both modes and reading view, record outside the vault | — | `Note B`: two checkboxes, two highlights (one yellow), two lines nested by a tab; `workspace.json` | every one Scoria's and a person's edit; the three config files did not move (Obsidian did not run) | **Scoria half passes** — no writer fault; two view faults fixed (a fold over the scroll anchor left a blank pane; a no-op Clear formatting pushed an empty undo step); Obsidian half pending |
| 7 |  |  |  |  |  |  |
