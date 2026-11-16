---
tags:
  - scoria/dev
created: 2026-11-16
---
# Parity gate run

The gate for [[Development#Parity (phase 4)]] is **a vault's core plugins behave as Obsidian's
do.** Every row of that section's table is done, so the gate is all that is left to close
phase 4. This note sets out how it is run: what counts as evidence, which vault and plugins it
covers, a sheet for each day, and the log. It follows [[Writer gate run]], and everything that
held there still holds here.

## What the gate claims, and what it does not

The Writer gate was about bytes, and a diff could settle it. This gate is about behavior, so
it needs two kinds of evidence at once:

1. **Bytes, as before.** Every file either app writes during the run is held to the Writer
   gate's rule: nothing moves that nobody touched. `tools/writer-gate/gate.sh` is reused
   unchanged (`just gate-check`, `just gate-baseline`), and its baselines carry on from
   `snapshot-2026-11-09g`.
2. **Behavior, side by side.** Each day does the same gestures in both apps on the same
   notes, and writes down every difference a person would notice: a row, a count, an order,
   a label, a missing control, a key that does nothing. The comparison is with your own eyes,
   because no harness can read both apps' views.

Each difference is sorted into one of four kinds, and the verdict depends on the kind:

| Kind | Meaning | Effect on the verdict |
| --- | --- | --- |
| **fixed** | a fault found by the day and fixed with it, with a test | passes |
| **listed** | already in the site's `differences.md`, and it doesn't get in the way of daily use of this vault | passes |
| **accepted** | new, and too small to be worth fixing now: added to `differences.md` the same day | passes, but only if you agree it is small |
| **blocking** | it changes what the vault means, loses work, or stops a plugin's job being done | fails the day until it is fixed |

The gate passes when every enabled core plugin has had its day, no difference is left
**blocking**, and the byte diff from the first snapshot to the last has an author for every
line.

What it does not claim: that Scoria is pixel-identical; anything about plugins this vault
leaves off; anything gpui blocks (window zoom, always-on-top on Wayland, the X11 un-maximize
position bug). Those stay in `differences.md` under their own rows.

## The vault, and the plugins it turns on

The deciding vault is the gate vault again (`$SCORIA_GATE_VAULT`, never named here). After the
Writer gate it holds sixteen notes in the root, `Archive/`, `Daily/` and `Templates/`, two
canvases, a little front matter, `types.json`, `bookmarks.json`, `hotkeys.json`, and the
Templates and Daily notes settings. `core-plugins.json` turns on these 19, and they are the
gate's scope:

| Plugin | Day |
| --- | --- |
| Files (`file-explorer`), Quick switcher, Command palette, Bookmarks | 1 |
| Search (`global-search`) | 1 |
| Backlinks, Outgoing links, Outline, Tags (`tag-pane`), Page preview | 2 |
| Graph | 2 |
| Properties, Word count, the status bar (`editor-status`) | 3 |
| File recovery | 3 |
| Daily notes, Templates, Note composer | 4 |
| Canvas | 5 |
| Bases | 6 |
| Settings, Appearance and Hotkeys (the app around the plugins) | 7 |

`sync` is on too, and it is out of scope: Sync is Obsidian's service, not a format ([[#Traps]]).
The vault turns off Footnotes, Slash commands, the Unique note creator, Random note, Slides,
the Audio recorder, Workspaces, Publish and the Web viewer, so this gate says nothing about
them. Each of those has a probe of its own on record in [[Development]].

## How a day runs

```
just gate-check        # the harness, then what has moved since the last baseline
#   the Scoria half: the day's gestures, noting what Scoria shows
#   the Obsidian half: the same gestures by hand, noting every difference
just gate-check        # the bytes, attributed as on the Writer gate
just gate-baseline     # the day's snapshot
```

The Scoria half can be driven over the control socket in a headless sway (as the Writer gate's
days 4 to 7 were), or by hand. **The Obsidian half is always done by hand**, from a numbered
list with 1-based line numbers, and you say "done" when it is finished. A day's notes go under
its heading below. Each difference gets one line with its kind, and a fix links to its
Development section. When both apps are open on the vault at once, close one before the other
writes, because neither app watches the other's in-memory state.

Build the app from HEAD before each day. A stale release binary wastes the day, as it nearly
did on the Writer gate's day 1.

## The sheet

**Day 1 — finding things.** Files: expand and collapse, the six sort orders, New note and New
folder from the header and from a folder's menu, inline rename, drag a note into a folder and
back, and the file menu's rows on a note, a folder and a canvas. Quick switcher: a partial name,
an alias, an unresolved name with Enter (to create it) and Shift+Enter, a path with a
`/`. Command palette: the first ten rows with an empty query, a fuzzy query, a pinned command.
Bookmarks: open each item, drag to reorder, make a group. Search: a word, a phrase, `path:`,
`file:`, `tag:`, `line:(a b)`, `section:`, a regex, case on and off, collapse and the sort menu,
the result menus on a file row and a match line, and Excluded files with `Archive/` added and
taken out again. Bytes to expect: `bookmarks.json`, `app.json`.

**Day 2 — links.** For a note with the most inbound links: the Backlinks pane (linked and
unlinked, collapse, sort, filter, Link on an unlinked mention), backlinks in the document,
Outgoing links (with unresolved targets), Outline (click, drag a heading, collapse), and Tags
(counts, nested tags, sort). Page preview: hover a link in reading view, Live Preview and
Source mode (with the modifier key set as the vault has it), a heading link, a block link, and
a preview from each pane's rows. Graph: the global graph with each filter, groups, forces and
the Depth of the local graph, with `graph.json` before and after. Bytes to expect: the note
Link writes into, `graph.json`.

**Day 3 — the note's own data.** Properties: add one of each type, change a type (it writes
`types.json`), rename a key, delete one, the All properties pane with its counts and sort, and
the File properties pane in reading view. Word count and the status bar: the words and
characters of the same note, of a selection, of a note with code, a table and a link, and of
CJK text if any is typed. Compare the numbers exactly. File recovery: the two apps keep
separate stores ([[#Traps]]), so compare only the tab, the snapshot list and Show changes on a
note edited in that app, plus the Settings rows.

**Day 4 — making notes.** Daily notes: open today's from the ribbon, with and without the
template, then previous and next. Templates: insert one with `{{date}}`, `{{time}}` and
`{{title}}`, from a template folder with a sub-folder. Note composer: merge one note into
another (Enter, then Shift+Enter on a second pair), extract a selection, and extract a heading,
each under the vault's link format. Compare the bytes of every note involved and of every note
that linked to the source. These are the sharpest bytes of the phase, because they rewrite
links across the vault.

**Day 5 — canvas.** Both canvases, with what the Writer gate left open: copy and paste of a
card (at a desk with a clipboard), card links and file cards, `![[x.canvas]]` embedded in a
note, Export as image, and the Canvas settings (snapping, wheel behavior, the Mod-drag,
card labels, new-file location) each changed in Obsidian and then tried in Scoria. Bytes:
both `.canvas` files and `canvas.json`.

**Day 6 — Bases.** The vault has no `.base` yet, so make one in each app over the notes with
front matter: a table filtered by folder, a formula, a sort, a group, a summary. Then cards,
list and kanban; edit a cell (which writes a note's front matter), a range paste and its Ctrl+Z,
and an embedded `![[x.base]]` in a note. Open each app's `.base` in the other app. Bytes: both
`.base` files and every note a cell edit touched.

**Day 7 — the app around the plugins, and the verdict.** Settings: one row of each kind on
every enabled plugin's tab; Appearance: the theme mode, the font size with the commands, a
snippet toggled; Hotkeys: bind one in each app and check the other reads it. Then `gate.sh
check --from-first`, the verdict below, and the log row.

## Traps

All the [[Writer gate run#Traps]] still apply (never `--stress` or `--synthetic` here,
`workspace.json` is excluded from the diffs, Obsidian rewrites three config files at launch,
and `.claude/` in the vault is Claude Code's). New ones for this gate:

- **`sync` is on in `core-plugins.json`.** If Obsidian Sync is actually connected to this vault,
  a third writer is on the folder. Check Obsidian's Sync tab on day 1, and if Sync is
  connected, pause it for the run.
- **File recovery has two stores.** Scoria keeps snapshots under its own configuration
  folder, and Obsidian keeps them in its IndexedDB. Neither can see the other's snapshots, so
  the day compares behavior, not contents. That difference is listed, not blocking.
- **Bases enabled, no `.base`.** Day 6 adds two files the vault did not have. They are a
  person's edits like any other, and they stay unless you want them gone.
- **`baseFontSize` is 19 in this vault.** Counts that depend on line wrapping (the status bar
  does not, but positions in the outline and the page preview's size do) will differ from a
  default vault's.
- **Settings is a popout window in Obsidian 1.15.** A rect measured in the main window is not
  where its rows are ([[Development#Excluded files, 2026-11-16]]).

## The verdict

Not run yet.

## The log

| Day | Date | Scoria half | Obsidian half | Differences (kind) | Bytes moved, and their authors | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | — | — | — | — | baseline carried on from `snapshot-2026-11-09g` | — |
