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
- **`workspace.json` is excluded everywhere**, in the harness and in every diff: Scoria
  neither models nor writes it, and Obsidian rewrites it constantly.
- **`.claude/settings.local.json` is in the vault and in the snapshots.** If it moves, the
  author is Claude Code, not either app.
- **Obsidian rewrites `appearance.json`, `app.json` and `core-plugins.json` on its own** when
  a settings pane is merely opened. Expect those three in a diff and attribute them before
  reading anything into them.
- **Don't run `--stress` or `--synthetic` against this vault.** They rewrite notes; they are
  for scratch vaults (see [[Development#The gate run opens, 2026-10-18]]).

## The log

One row per day. "Moved" is what the diff reported; "author" is the answer to the two
questions above.

| Day | Date | Scoria half | Obsidian half | Moved | Author | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 2026-11-04 | — | — | nothing (vault inert since 10-22) | — | baseline `snapshot-2026-11-04` taken, harness green at 15 properties |
| 1 |  |  |  |  |  |  |
| 2 |  |  |  |  |  |  |
| 3 |  |  |  |  |  |  |
| 4 |  |  |  |  |  |  |
| 5 |  |  |  |  |  |  |
| 6 |  |  |  |  |  |  |
| 7 |  |  |  |  |  |  |
