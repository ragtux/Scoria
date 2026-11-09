---
tags:
  - scoria/dev
---
# UI parity backlog

Where Scoria's window still differs from Obsidian 1.14, as of 2026-10-20. Every entry below was
produced by driving the *running* app over the control socket
([[Development#Control socket and MCP server, 2026-10-19]]) and reading `state` back after every
step, and each claim then went to an adversarial reviewer whose main job was the Obsidian half —
the pass before this one had refuted four leads and every one of them had failed there.

It happened twice. The first pass filed 82 findings over seven subsystems; its run hit a session
limit, and when it resumed the probe agents re-ran rather than replaying their cache, so what came
back was an independent second audit — 92 findings, 13 of them Live Preview. The fix campaign only
ever saw the first pass plus the second pass's Live Preview file, 95 findings in all, and fixed 54
of them over seven waves with a fmt/check/clippy/test gate between each. The second pass's other 79
findings were triaged against the *finished* binary afterwards, and one more wave then fixed a
crash and a corruption bug that neither pass had looked for. The story is in
[[Development#The parity campaign, 2026-10-20]].

So an entry here is worth more than it was on 2026-10-19, and where the two passes cover the same
ground the triage is the better source: it was driven against the current code, so its line numbers
and its evidence are the ones to act on, and several of its entries narrow a finding the first pass
stated too widely. What an entry does *not* promise is that the fix is settled: several say plainly
that the Obsidian behaviour is documented rather than probed, and the probes those wait on are
collected under [[#Was declined until Obsidian was probed — all of it now is]], which as of
2026-10-25 holds the answers rather than the questions.

**Read the `Landed` lines first.** An entry whose first line after the severity reads
`**Landed <date>**` describes behaviour that has since been fixed; the evidence under it is kept
because the Obsidian half is still the record of what was observed, but the **Scoria** paragraph
describes a binary that no longer exists. Forty entries are marked so far, twenty-five of
them on 2026-10-24, three (the folding ones) on 2026-10-26 and two on
2026-10-28 — the pass of 2026-10-28 also found that eleven unmarked items of
the UI-rules entry had been built without anyone striking them off, which is
the clearest warning this file can give about acting on an unmarked entry. Before acting on an unmarked entry, check the behaviour in the running app:
several of the ones marked that day turned out to have been fixed already, by work that landed
after this audit was written.

The vocabulary is unchanged. *bug* is behaviour Scoria gets wrong on its own terms; *parity-gap* is
behaviour that works but does not match Obsidian; *missing-feature* is something Obsidian has that
Scoria has not built. Severity is the reviewer's, not the author's.

**What the campaign worked from** — the first pass's 82 (`F` numbers) plus the second pass's Live
Preview file (`L` numbers):

| Subsystem | Confirmed | Fixed | Still open |
| --- | --- | --- | --- |
| Files view and file operations | 13 | 4 | 9 |
| Live Preview | 13 | 9 | 4 |
| Reading view | 12 | 5 | 7 |
| Right dock panes and graph | 12 | 6 | 6 |
| Search, switcher, palette and autocompletion | 12 | 8 | 4 |
| Settings | 9 | 4 | 5 |
| Source editor | 12 | 12 | 0 |
| Workspace shell | 12 | 6 | 6 |
| **Total** | **95** | **54** | **41** |

**What the second pass's other seven subsystems said when triaged against the finished binary.**
These 79 findings overlap the table above heavily and are worded differently, so the two sets are
not additive — this one is a check on the campaign's work rather than a new backlog:

| Subsystem | Findings | Already fixed | Fixed by wave 8 | Still reproduce |
| --- | --- | --- | --- | --- |
| Files view and file operations | 12 | 4 | – | 8 |
| Reading view | 11 | 2 | – | 9 |
| Right dock panes and graph | 11 | 2 | – | 9 |
| Search, switcher, palette and autocompletion | 9 | 4 | – | 5 |
| Settings | 11 | 5 | – | 6 |
| Source editor | 13 | 7 | 1 | 5 |
| Workspace shell | 12 | 3 | 1 | 8 |
| **Total** | **79** | **27** | **2** | **50** |

Plus one the triage found that neither audit had: a panic that killed the app, fixed in wave 8.

Most of the 50 are the same large gaps the first pass had already filed — no tabs, no ribbon, the
Hotkeys tab, the graph controls panel — and they are not duplicated below. What the triage added is
twenty entries nobody had filed before and fresh evidence on a dozen more; both are folded into the
sections that follow, so the sections held **61 open entries** in total; the five large builds of 2026-10-20 and the
four small entries they carried closed nine of them, so **52 remain** — plus one filed after the frame work of the same evening, below; the Files header of 2026-10-21 closed one more and halved two, and the explorer keyboard the same day closed the halved header entry. The UI-rules pass of 2026-10-28 closed two more and filed two of its own (the explorer's context menus and the reading view's property pills), both of them work its own build deliberately left. The Bases pass of 2026-11-01 filed thirteen more under [[#Bases]], the first entries this file holds for that view, and the build of the same day landed the first seven.

## Files view and file operations

### The "Update links" prompt is two buttons and Scoria's own wording

*medium, parity-gap, small* · **Filed:** 2026-11-05

**Landed 2026-11-05**: the modal is Obsidian's title, its two lines and its three buttons in its
order, with no Cancel, and "Always update" writes `alwaysUpdateLinks: true` before it rewrites.
`PromptSpec` grew `buttons`, a whole row of `PromptButton { label, action, cta }` that replaces
the default `[secondary] [Cancel] [confirm]` when a prompt states its own, and `detail` became a
`Vec` so the two lines are two lines. The row also reaches the control socket's state dump, so a
driver can read "Always update" rather than aim at it. See
[[Development#The third button on the rename prompt, 2026-11-05]].

Found by the Writer gate's day 2, as the second half of an inverted default; the default itself is
fixed, see [[Development#Day 2, files and links, 2026-11-05]]. Until that fix the prompt was
unreachable on any vault that had never written `alwaysUpdateLinks`, which is why its shape had
never been compared with Obsidian's.

**Obsidian.** Three buttons, and no file name in the text
([[Specification#The 2026-10-25 pass: real drags, and the inline rename committed]]):

| | |
| --- | --- |
| Title | **Update links** |
| Text | `Do you want to update internal links that link to this file?` then `This will affect 1 link in 1 file.` |
| Buttons | **Always update**, **Just once**, **Do not update** |

**Always update** is also a config writer: it turns the setting on, so the next rename does not ask.

**Scoria, as it was.** Title "Update links?"; the detail named the file *with its extension* — "6
links in 6 files point to `Note F renamed.md`. Update them to the new name?" — and there
were two buttons, "Update links" and "Don't update". So the sentence was Scoria's, the count was
phrased the other way round, and the third button was missing.

**What the fix found beyond its own entry.** Obsidian has no Cancel because by the time its modal
is up the file has already been renamed: the drag pass dismissed it with Escape and the linking
file stayed byte-identical *while the move stood*. Scoria asked before renaming, so Escape
abandoned the rename altogether — a fourth answer Obsidian does not have. Escape and a click
outside are now "Do not update". Scoria still asks before it renames rather than after, which is
not observable: all four answers leave the same bytes.

Which of the three buttons carries `mod-cta` was never recorded, so Enter keeps doing what it did
before the third button arrived — updating once — and "Just once" is the accented one. That is the
one piece of this modal still unprobed.

### The explorer renames in a modal where Obsidian edits the row in place

*medium, parity-gap, medium* · **Filed:** 2026-11-05

**Landed 2026-11-05**: F2 on a focused row and the menu's "Rename..." both turn the row's own
label into a field with the name selected, and there is no dialog on either path. The behaviour
lives in `inline_field.rs`, which the view header's title field was moved onto in the same change,
so the two inline renames Obsidian has are one implementation here. See
[[Development#The file tree renames on the row, 2026-11-05]].

Observed in the Writer gate's day 2, on the gate vault: the Obsidian half's new note was named by
typing **on the row in the file tree**, and Scoria opens a dialog in the middle of the window for
the same thing. See [[Writer gate run#The Obsidian half, and the verdict]].

**Obsidian.** The row's own label is made `contenteditable`
([[Specification#The 2026-10-25 pass: real drags, and the inline rename committed]]): the editable
element is `.tree-item-inner.nav-file-title-content` (`.nav-folder-title-content` for a folder), the
row gains **`is-being-renamed`** beside `has-focus` and `is-active`, and the **basename without the
extension** is pre-selected — `Beta`, not `Beta.md`. F2 reaches it only with focus genuinely on a
row, which a click cannot give; the Files header's New note puts a fresh `Untitled` row straight
into that state.

**Scoria.** `F2` on a focused row and the header's New note both emit `ExplorerEvent::Rename` and
open the centre prompt pre-filled with the basename (`explorer.rs:703`, `rename_with_links`). The
tree has no in-place editing at all: no row state for it, and nothing in `state.docks.left.explorer`
reports one.

**Fix.** The text model is already written and already proven: `title_field.rs` is the
`contentEditable` behaviour for Obsidian's `.view-header-title` — click-to-caret, typing while the
file keeps its old name, Enter commits, Escape abandons — deliberately kept free of GPUI so it can
be reused. So this is wiring `TitleField` into the tree row and painting the row's label as its own
text, not new behaviour. Worth doing with the keyboard entry below, since F2's precondition
(focus on a row, which only `reveal-active-file` or the tree keyboard gives) is the same one.

**One claim in this entry was wrong, and is withdrawn.** It read the gate log as saying the Files
header's **New note** puts the fresh row into that state. The 2026-10-25 pass says otherwise, and
the spec has carried the measurement all along
([[Specification#The 2026-10-25 pass: real drags, and the inline rename committed]]): New **note**
opens the file in a new tab *with the note's own inline title focused*, and it is New **folder**
that creates `Untitled/` and puts its row into inline rename, Escape keeping the folder under that
name. Both header buttons are therefore their own change and neither is in the one above —
Scoria's New folder still asks for the name before it creates anything, which is the opposite
order. Filed below.

### The Files header's New folder asks for a name before it creates anything

*low, parity-gap, small* · **Filed:** 2026-11-05

Found while landing the row's inline rename above, by checking its claim about New note against
the measurement rather than the gate log.

**Obsidian** ([[Specification#The 2026-10-25 pass: real drags, and the inline rename committed]],
the header table). **New folder** creates `Untitled/` at the vault root *first* and puts its row
straight into inline rename (`is-being-renamed`, `contenteditable`) — so Escape keeps the folder,
under the name `Untitled`. **New note** is the other shape: `Untitled.md` in the folder
`newFileLocation` names, opened in a new tab **with the note's own inline title focused**, not the
tree row.

**Scoria.** New folder opens the centre prompt and creates nothing until it is confirmed
(`start_new_folder`), so Escape leaves no folder; New note creates the file and then opens the
rename prompt (`new_note`). Neither matches, and they are wrong in different directions.

**Fix.** New folder: create `Untitled` (or the next free number) and call
`FileExplorer::start_rename` on its row, which is now there. New note: focus the note's inline
title in the editor instead of opening a prompt — which needs the inline title to be a field the
app can focus, and that is `show_inline_title`'s widget rather than the view header's, so check
what it has before costing this.

### Autosaving the open note re-centers the Files list and re-expands every ancestor folder

*medium, bug, small*

**Landed 2026-10-24** (`6dc12d5`): `is_own_write` recognises the app's own save coming back
through the watcher -- every change to the open file a modification, the editor on that file, the
text on disk the text in the buffer -- and refreshes the panes instead of re-entering `show_file`.
A content check, not an mtime stamp, so a real external write still reloads.

**Scoria.** Confirmed twice, most recently against the finished binary with a sentinel status so the
new toast notices could not mask the result: append to an unrelated file from a shell, so
`after_changes` sets `status = "zzz.md changed"`; type one character into the open note; at t+3 s
the status is "saved" and at t+10 s, once the notice has expired, it is
"Untitled.md · reading view built in 0.0 ms". That string is only ever set by `show_file`, so the
app's own autosave write came back through the watcher and re-entered it (workspace.rs:1769), and
`show_file` unconditionally calls `explorer.reveal(path)` (workspace.rs:2495), which strips every
ancestor out of `collapsed` and scrolls the row to centre. `flush_save` records nothing about the
bytes it just wrote, so `after_changes` cannot tell a self-write from an external edit.

**Obsidian.** Obsidian never scrolls the explorer on its own unless the file-explorer leaf's
`autoReveal` is on, and both Obsidian-written `workspace.json` fixtures in the repo have it off.
Saving a note moves nothing. (That last sentence is inference from the fixture, not an observation
— see [[#Was declined until Obsidian was probed — all of it now is]].)

**Fix.** Two independent halves. The explorer half landed on 2026-10-21 with the Files header:
`set_tree` no longer reveals anything and `show_file` calls `FileExplorer::follow`, which only
highlights the row unless the leaf's `autoReveal` is on — so the re-entry still happens but moves
nothing. What remains is the self-write itself: in `after_changes`, a Modified(current) whose
on-disk text the editor already matches should call `refresh_panes` instead of `show_file` — a
content check, not an mtime stamp, so a genuine external write is never swallowed.

### Rename accepts `/` and silently creates folders; an empty name reports the dot rule

*medium, bug, small*

**Landed 2026-10-24** (`ca2a63e`): the rename prompt validates the stem it was typed with
`validate_renamed_file` before the extension goes back on, so `/` is refused by the
forbidden-character rule and an empty box reads as empty rather than as a leading dot.

**Scoria.** F2 on a note, type `Sub/Renamed`, Enter: no error, the file becomes `Sub/Renamed.md`
and a `Sub/` directory that did not exist is created. F2 then Ctrl+A then Backspace then Enter
leaves the modal open with "File name must not start with a dot.". `bad?name` is refused with a
message that *names* `/` among the forbidden characters. `PendingOp::Rename` (workspace.rs:3249)
never validates the typed value; the only check is per-segment `validate_path` (ops.rs:87-95). A
path escape is **not** possible — `../Escaped` and a leading slash are both refused and nothing was
written outside the vault — so this is folder creation inside the vault, which is why the triage
raised it from low to medium.

**Obsidian.** Rename refuses any of `* " \ / < > : | ?` and an empty name with "File name cannot be
empty."; the inline rename edits a basename, not a path, so it cannot create folders.

**Fix.** `scoria_formats::validate_file_name(value)` on the raw trimmed value as the first statement
of the `PendingOp::Rename` arm, with `Prompt::set_error` on Err, before `ext`/`new_name`/`to` are
built. The strings are already there. Note the cause of the empty-name half precisely: the validator
is not wrong, the value never reaches it, because workspace.rs:3263 turns `""` into the path `.md`
first. The sibling `PendingOp::Move` arm has the same hole, where an empty value renames a file to
itself.

### The explorer keeps highlighting a file it no longer has

*low, bug, small*

**Landed 2026-10-24** (`0593de9`): `detach_current` clears the explorer's highlight, at all four
places a file goes away underneath the window.

**Scoria.** Delete the open note: `file.path` is null but `explorer.active` is still the dead path,
painted with `bg_active`. Recreate the path from a shell and the watcher re-reveals it, so the row
comes back highlighted with no file open. `delete_now` clears `self.current` and calls
`detach_current` and `forget_history`, but never touches the explorer's `active`, and `set_tree`
re-reveals the stale path on every rebuild.

**Obsidian.** `is-active` tracks the active leaf's file; with no file open no row carries it.

**Fix.** `FileExplorer::clear_active`, called from `delete_now` beside `detach_current`; and have
`set_tree` drop an `active` whose path is no longer in the tree, so a stale highlight cannot survive
a rescan.

### Creating a folder steals the active-row highlight from the open note

*low, bug, small*

**Landed 2026-10-24** (`0593de9`): the two folder call sites go through `reveal_folder`, which
scrolls without claiming the row is the open file.

**Scoria.** With `Welcome.md` open, `file-explorer:new-folder` + "Archive" + Enter leaves
`explorer.active` on `Archive` while `file.path` is still `Welcome.md`, and a forced tree rebuild
does not repair it. `PendingOp::NewFolder` calls `explorer.reveal`, and `reveal` sets `active`.

**Obsidian.** Creating a folder never moves `is-active`; the spec's explorer capture shows it on a
file row.

**Fix.** The `scroll_to(path)` from the first entry, called from the NewFolder arm. `reveal` and
`set_active` should only ever be handed the path the main area is showing.

### Rename surfaces a raw OS error with the absolute host path

*low, bug, small*

**Landed 2026-10-24** (`ca2a63e`): a 255-byte length rule keeps `ENAMETOOLONG` from being reached
at all, and `OpError::user_message` gives a dialog the action, the file's name and the reason in
words. The full `Display`, path and errno included, is still what a log gets.

**Scoria.** F2, clear, 300 `L` characters, Enter: the modal shows
`rename /tmp/.../vault/Welcome.md: File name too long (os error 36)` — the machine's filesystem path
and an errno. Confirmed again by the second pass. `validate_file_name` has no length rule and the
rename arm never runs it on the raw value anyway.

**Obsidian.** Rename errors are short sentences about the name; no absolute paths, no errno.

**Fix.** Make `OpError::Io`'s Display print the vault-relative path (the `io("rename", &from_abs)`
call at ops.rs:253 already has `from`), and add a 255-byte-per-segment check to
`validate_file_name` so the common case is caught before the syscall.

### File-operation feedback is Scoria's wording, and fires even when nothing was rewritten

*low, parity-gap, medium*

**Landed 2026-10-24** (`d885d8c`): `apply_rename` emits exactly `Updated {n} link{s} in {m}
file{s}.` and only when links moved; the "Deleted" and "Copied" reports are gone. The
changed-on-disk notice is kept deliberately -- Obsidian never skips, so it has no counterpart,
and silence would leave a stale link with no sign of it.

**Scoria.** Half of this is now fixed: the notice layer means `show_file`'s "reading view built"
line can no longer overwrite the report — a rename sampled at t=0,1,2,3 s keeps saying
"Renamed zzz.md to yyy.md". The wording half reproduces exactly. That rename rewrote zero links and
still produced a message, and the link case reads "Renamed Target.md to Sub/Renamed.md, updated 3
links in 1 file". "Deleted X" and "Copied X to Y" are reported too.

**Obsidian.** A move that rewrites links shows a Notice reading exactly `Updated 1 link in 1 file.`
(Specification.md:1972-1978); a move that leaves the basename alone produces no notice, and nothing
about a file operation reaches the status bar.

**Fix.** In `apply_rename`'s summary block, return without reporting when
`outcome.rewritten_links == 0`, and otherwise emit exactly `Updated {n} link{s} in {m} file{s}.`
with no "Renamed X to Y" prefix. Drop the "Deleted" and "Copied" reports.

### Non-markdown rows show the extension inline instead of as a tag

*low, parity-gap, small*

**Landed** (`e778ba4`): `Row` carries `name` (the stem) and `tag` (the upper-cased extension),
rendered as Obsidian's `nav-file-tag` at the row's right end. The state dump's `name` is the stem
alone rather than stem+ext as the fix below suggested.

**Scoria.** `explorer.rows` reports `{"path":"cover.png","name":"cover.png"}` — the whole filename
is the row's single label — and the row is painted `text_muted` entirely, because `display_name`
strips only `.md`.

**Obsidian.** A non-markdown row renders the basename in `.nav-file-title-content` and the extension
in a sibling `.nav-file-tag`: `cover.png` is "cover" plus a "png" chip.

**Fix.** Give Row a `stem` and `ext` beside `name`, render the stem in `theme.text` and the extension
as a small rounded chip, keep sorting on the stem, and keep `describe`'s `name` as stem+ext so the
state dump is unchanged.

### The explorer's context menus are a short subset, with no icons and no sections

*low, parity-gap, large* · **Filed:** 2026-10-28

**Landed 2026-10-28**, the same day it was filed, and this entry's own estimate
of what was blocked was wrong: it said nine of the captured items were commands
Scoria has not got. Four are. `OpenInNewTab` and `Workspace::open_in_new_tab`
already existed, tab groups make "Open to the right" a split with a file of its
own, `cx.write_to_clipboard` is used in three other places, and
`Search::set_query` plus `show_search` make "Search in folder" two lines -- so
the split this entry asked for was much the smaller half. All three menus are
built, with Obsidian's sections, separators and lucide icons; `Section` and
`menu_items` in `explorer.rs` are the model, and the separators fall out of
where the section changes, including the doubled one above `danger`. Five icons
were cut from Lucide's own source (`clipboard`, `folder-open`, `folder-search`,
`separator-vertical`, `pen-line`). See
[[Development#The explorer's context menus, 2026-10-28]].

What is still left out, and why, is
[[#Five context-menu items wait on features Scoria has not got]] -- four items
plus a folder's "Make a copy". Obsidian's file row has no creation items;
Scoria's used to offer "New note" and "New folder" there, which was the one
thing in these menus that was wrong rather than merely missing, and it is gone.

**Obsidian.** The file-row menu is fifteen items in six `data-section`s with a lucide icon each, the
folder-row menu is fourteen in five, and the empty space below the last row gives four
`action-primary` items; both menus carry two consecutive separators before Rename, i.e. an empty
section. Every item, its icon and its section are in
[[Specification#The file explorer's context menus]], captured 2026-10-25.

**Scoria.** `explorer.rs` builds six items for a file row (New note, Rename..., Move file to...,
Make a copy, Bookmark..., Delete) and five for a folder, with no icons, no sections and no
separators, and nothing at all on empty space.

**Fix.** Done as described below, bar the count: six of the nine named here turned out to be
buildable, and `picture-in-picture-2`, `git-merge` and `history` were never fetched because their
items are the ones still missing. What the paragraph got right is the rule — the menu got the
sections, separators and icons it can honour, and the items it cannot run were filed rather than
drawn dead. *Nine of the
captured items are commands Scoria has not got* — `Open in new window`, `Merge entire file with...`,
`Open version history`, `Open in default app`, `Show in system explorer` and the three `Open in...`
rows beyond `-leaf` — and listing a row that cannot run is the thing the command palette refuses to
do ([[#Settled by the probes of 2026-10-25 — the UI rules (issues #8, #19, #41)]], last paragraph).
So this wants splitting: give the menu the sections, separators and lucide icons it can honour for
the items that already work, and file the missing commands as their own entries rather than drawing
dead rows. Nine icons would need cutting from lucide's own source first (`separator-vertical`,
`picture-in-picture-2`, `git-merge`, `clipboard`, `history`, `edit-3`, `edit`, `folder-open`,
`folder-search`).

### Five context-menu items wait on features Scoria has not got

*low, missing-feature, large* · **Filed:** 2026-10-28

**Two of the five landed 2026-10-30.** *Show in system explorer* is in the file, folder and Links
menus: the D-Bus `org.freedesktop.FileManager1.ShowItems` call through `dbus-send`, with `xdg-open`
on the parent folder as the fallback, `open -R` on macOS and `explorer /select,` on Windows, one
platform table with the file-URL escaping tested beside it. *Make a copy* is a folder row:
`Vault::duplicate` takes a file or a folder, the copy is a sibling on the first free name (a folder's
name is never split on a dot), and a folder's copy walks what the scanner would index and copies
every file's bytes verbatim. **No link is rewritten** -- this entry assumed a folder copy must
rewrite the links of the notes it copies, and that is unobserved; a byte-identical copy loses
nothing, so it waits for a probe. OBSERVE: what Obsidian does to the links of a copied folder's
notes. Three items are still out: *Open in new window* (no second window), *Merge entire file
with...* and *Open version history*. **Landed 2026-11-09**: *Open in new window* is drawn on every
menu that had it, since the popout windows exist (multi-window step 3, see
[[Development#Multi-window, the design, 2026-11-09]]), and `open-link-in-new-window` runs; the
two others stay out.

**Obsidian.** The captured menus
([[Specification#The file explorer's context menus]]) hold five rows the
2026-10-28 build left out rather than draw dead, each with its section:

| Item | Menu | What it needs |
| --- | --- | --- |
| Open in new window | file, `open` | a second window; `open-link-in-new-window` is unrunnable for the same reason |
| Merge entire file with... | file, `action` | a merge, and a chooser to merge with |
| Open version history | file, `view` | file history, which is Obsidian Sync's and which Scoria has no store for |
| Show in system explorer | file and folder, `system` | selecting a file *in* the desktop's file manager: the D-Bus `org.freedesktop.FileManager1.ShowItems` call, with opening the containing folder as the fallback where nothing answers it |
| Make a copy | **folder**, `action` | a recursive folder copy; `Vault::duplicate` is one file (it splits the name on its extension), and a folder copy has to rewrite the links of every note it copies |

**Scoria.** Each row is absent, which is deliberate: the menu lists only what
it can run. Dropping the only item of a section drops that section's separator
with it, so the file menu has no `view` separator where the capture has one.

**Fix.** Four jobs in rising order of size. *Show in system explorer* is the
nearest: a D-Bus call with an `xdg-open` on the parent folder as the fallback,
beside the `open_in_default_app` that landed. *Make a copy* on a folder is a
`scoria-vault` job -- walk the folder, copy it to `available_path`, and run the
copied notes through the link rewriting a move already uses. *Open in new
window* waits on multi-window, which also unblocks `open-link-in-new-window`.
*Merge entire file with...* and *Open version history* are features rather than
menu rows, and want entries of their own if they are ever wanted at all.

## Live Preview

### Heading and list folding does not exist

*medium, missing-feature, large*

**Landed 2026-10-26**: `crates/scoria/src/fold.rs` plus the editor's hidden lines, placeholder,
indicators and the five commands; see [[Development#Folding, 2026-10-26]].

**Scoria.** `editor:toggle-fold`, `fold-all`, `unfold-all`, `fold-less` and `fold-more` are all
registered with `"runnable": false`; Ctrl+P then "fold" offers only the properties fold and fuzzy
noise. No fold indicator is reachable anywhere. This is also the open remainder of the source
editor's command-palette finding.

**Obsidian.** [[Specification#Observed heading fold behavior, Obsidian 1.14.4]] is a 14-step CDP
probe: every foldable line carries a hover-revealed indicator (headings at `left: -15px`, list
parents at `-10px`), a heading folds through the line before the next heading of the same or higher
level with trailing blanks included, the last heading folds to the end of the file, the folded line
gets a `cm-foldPlaceholder` ellipsis, and nothing is written to the note — the state lives in
localStorage under `<appId>-note-fold-<path>`.

**Fix.** Give `Editor` a `folded: Vec<Range<usize>>` of line ranges computed from the highlight
spans, skip folded lines in prepaint the way folded widgets are skipped, append the ellipsis to the
folded line's `DisplayLine`, and add the five ids to RUNNABLE. Persist per-note ranges in Scoria's
own app data, never in the vault.

### Image embeds render only as a whole top-level paragraph

*medium, parity-gap, large*

**Half landed 2026-11-04, the nested half.** Probed first
([[Specification#Image embeds nested in lists and quotes, Obsidian 1.15.0]]): Obsidian draws
`- ![[pic.png]]` as **one** line as tall as the picture, the image at the item's text column and
the bullet at the line's foot; an ordered item, a task, a nested item and `> ![[pic.png]]` the
same. `build_widgets` now walks lists and quotes at any depth and makes every paragraph that is one
embed a widget whose span starts at the embed, not the line; the editor shapes the bytes before it
through the line's own Live Preview display (`- ` is `•`, `- [ ] ` its box, `> ` nothing plus the
bar), paints that at the widget's foot and lays the embed out after it in the width left. A
callout was already a widget whole. **Still open**: an embed sharing its paragraph with text --
`Look: ![[real.png]] here` and the lazy continuation `- item` / `  ![[pic.png]]`, which Obsidian
draws on a no-bullet line of its own at the item's indent -- is the inline placement below.
(A click on a nested widget's box, the `☐` beside an image, was listed here too; re-driven on
Windows on 2026-11-04 it toggles `- [ ]` and `- [x]` both ways and autosaves, so it is struck.
The same run saw a stray `-` painted after that box once it was checked, see
[[#A task's checkbox is a font glyph, and a checked one is a colour emoji on Windows]].)

**Scoria.** With an 80×80 PNG in the vault, `- ![[real.png]]`, `Look: ![[real.png]] here` and
`> ![[real.png]]` each measure one 24 px text row and a click on them navigates to the file, while
the same embed alone on a line measures ~104 px. Sizing works (`|40` gives ~64 px); only placement
is the limit. `build_widgets` walks only `doc.blocks` and only single-inline paragraphs.

**Obsidian.** Obsidian renders image embeds inline wherever they occur. The repo's evidence for the
*nested* case is weak: Specification.md:1041 and :1192-1198 disclaim the captured rows as unlaid-out
widget output rather than a measured Live Preview session, and the corpus holds no image embed at
all. Probe the list-item case before designing the inline-in-a-sentence one.

**Fix.** Two steps. The nested case first: have `build_widgets` walk list items', blockquotes' and
callouts' children, with the widget carrying the leading marker and indent as a prefix the renderer
re-draws — a naive promotion loses the bullet, because `widget_lines` widens a widget's span to
whole lines and the editor paints the widget *instead of* them. Inline placement then needs a
non-text piece kind in `preview.rs`'s `DisplayLine` threaded through shaping, hit testing, selection
and the cursor maths, which is an `editor.rs` project of its own.

### A task's checkbox is a font glyph, and a checked one is a colour emoji on Windows

*low, parity-gap, small* · **Filed:** 2026-11-04

**Landed 2026-11-04**: the `[ ]` is now shaped as two figure spaces (`preview::TASK_BOX`), and
the editor paints the box over them the way inline math is painted: an em-sized rounded square
in `--checkbox-border-color`, filled with `--checkbox-color` and a stroked tick in
`--checkbox-marker-color` when checked, both on a line and in a nested widget's marker. No glyph
is left for a fallback font to pick. The stray `-` was a done task's strikethrough landing on
the space after the box; that gap is no longer struck, on a nested marker or on a plain line.
The fix did not need the non-text piece the entry predicted. Checked in the app on Windows: a
click on the box still toggles and saves.

**Scoria.** Live Preview draws a task's box as text: `preview.rs` replaces `[ ]` with `☐` and
`[x]` with `☑` and styles them as a run. Inter has neither glyph, so the font fallback picks one,
and on Windows 11 that is Segoe UI Emoji: a checked task is a purple-blue emoji tile, while the
same task in reading view is the `--checkbox-color` box in the vault's accent (brick on a vault
that sets none). The unchecked `☐` comes out as an outline that roughly fits. Seen on the
2026-11-04 backlog check, Windows, 150 % scaling, with a `- [x] ![[real.jpg|80]]` line, which also
painted a stray `-` after the checked box at the widget's foot. A plain `- [x] done` line was not
looked at on Windows, and the Linux fallback has not been checked either.

**Obsidian.** The box is an `<input type=checkbox>` styled from `--checkbox-color`,
`--checkbox-border-color` and `--checkbox-marker-color`, the same in Live Preview and reading view.

**Fix.** Paint the box the way `reading/view.rs` already does (the three tokens, a drawn marker)
instead of a glyph, which also takes away the dependence on the platform's fallback font. That
means a non-text piece in `DisplayLine`, so it overlaps the inline-placement work above.

### HTML other than an `<img>`-only block is still raw muted source

*medium, parity-gap, medium*

**Half landed 2026-10-30, the inline half** (in the reading model, which Live Preview's widgets
share): `InlineStyle` gained `underline`, `kbd`, `sup` and `sub`, and a tag stack in `RichBuilder`
maps `<b>/<strong>`, `<i>/<em>`, `<s>/<del>/<strike>`, `<mark>`, `<u>`, `<kbd>`, `<sup>`, `<sub>`
and `<a href>` onto runs, drops `<span>/<div>/<font>/<center>/<small>/<p>` keeping their content,
turns `<br>` into a line break and leaves everything else -- `<script>`, an unknown tag, a
malformed token -- as the muted source it was. **A first slice of the block half landed the same
day**: a block of nothing but comments and `<script>` elements draws nothing at all, which is what
the sanitizer capture and the DOM both say, instead of showing its own source; a block holding
anything else is untouched. An unclosed opener runs to the end of its paragraph,
as the sanitizer capture records. Still open: **HTML blocks** (`RBlock::Html`: `<div>`, `<pre>`,
`<details>`, a comment on its own line), `sup`/`sub` drawn at body size because a gpui text run
cannot shift its baseline, Live Preview's own source transform (`<b>` still shows as source while
editing), and `<ins>/<samp>/<var>/<tt>/<big>`.

**The block half landed 2026-11-02**: a raw `<pre>` is a code block that wraps and carries the copy button only with a `<code>` child; `<details>` folds its `<summary>` over its body, open when the tag says so; a container (`<div>`, `<p>`, `<center>`, `<span>`, `<font>`, `<small>`, `<big>`, `<section>`, `<article>`, `<ins>`, `<samp>`, `<var>`, `<tt>` and the inline tags) draws as a paragraph of its content through the same overlay inline HTML takes, with entities decoded and a leading comment dropped; each of those is a Live Preview widget. A `<table>`, `<iframe>`, `<form>` and the rest stay source. **Still out**: `sup`/`sub` at body size, Live Preview's own source transform of inline tags, and a raw table.

**Landed 2026-11-04, the Live Preview transform and the raw table**, against the probe recorded in
[[Specification#The 2026-11-04 pass: Live Preview, and a raw table]]. Obsidian replaces a whole
inline element -- opener, content, balancing closer -- with one `cm-html-embed` widget while no
cursor touches it, edges included, and shows the content literally (`<b>**md**</b>` draws `**md**`
in bold). Scoria now does the same: `preview::html_elements` pairs the line's tag spans into
outermost elements, and an untouched one is substituted by its rendering
(`model::html_inline_widget`, the reading overlay with Markdown left literal), styled by
`style_html_run`. A raw `<table>` block draws as a table in the reading view -- the `<thead>` row
or a first row of `<th>` as the header, a body `<th>` bold, `align` on the first row's cells as the
column alignment, ragged rows padded -- and stays source in Live Preview, which is what Obsidian
does: a raw table is never a widget there.

**Still out**, each a deliberate gap:

- `<sup>`/`<sub>` at body size. Obsidian draws them at 5/6 of the body on a raised or lowered
  baseline. gpui's `TextRun` has no size and no baseline offset (the pinned Zed commit: `len`,
  `font`, `color`, `background_color`, `underline`, `strikethrough`), so a run cannot be shrunk or
  shifted. The one per-run lever is the font's OpenType `sups`/`subs` features, which bundled
  Inter has; but `HighlightStyle`, which the reading view styles through, carries no font features
  (only a family override), so the reading view would need its own run builder replicating the
  container's text style, and the features only cover the glyphs Inter draws superior (digits and
  a few signs). Not clean enough to take; revisit if gpui gains per-run size or features in
  `HighlightStyle`.
- Live Preview keeps the source of an element the overlay cannot draw where Obsidian still makes
  a widget: an unknown tag (`<foo>x</foo>` is a widget of `x` there), a lone `<br>` (a widget that
  breaks the line, which one display line cannot hold), an element spanning lines.
- An inline `style` attribute is not drawn in either view (`<span style="color: red">` is red in
  Obsidian), and Live Preview's `<mark>` widget wears the theme's highlight where Obsidian's shows
  the browser's own yellow behind black text.
- The reading view still shows an unknown inline tag and an inline `<script>` as muted source;
  Obsidian unwraps the first and drops the second with its content.
- A raw table's `colspan`/`rowspan`, a cell's `style`, `border="1"`'s outset frame and a
  `<caption>` are not drawn; a table nested in a cell leaves the block as source.

**Scoria.** An HTML block that holds nothing but `<img>` tags now resolves and renders as an image
widget in both views. Everything else — comments, `<div>`, `<pre>`, `<a>`, inline `<u>`, `<sup>`,
`<kbd>` — still falls through to `RBlock::Html`, drawn as muted monospace source. The promotion is
deliberately narrow: the parser drops CommonMark's html-block discriminator, so a comment and a
`<script>` arrive as the same variant and a blanket promotion would fold their source lines away to
show the same muted text.

**Obsidian.** [[Specification#Observed raw HTML behavior, Obsidian 1.14.4 (installer build 1.13.7)]]
records the measured subset: `<a>` kept with `target`/`rel`, `<pre>`, and `<b>/<i>/<u>/<kbd>/<sup>/
<sub>/<details>` kept and themed, with only scripts and event handlers stripped.

**Fix.** The reading renderer's `RBlock::Html` arm plus the inline path. `RichText` runs are flat
`(range, style)` pairs with no nesting, so `<sup>`, `<sub>` and `<kbd>` need new `InlineStyle` flags
rather than only text.

**`<pre>` is the next block worth taking, and it wants two changes in the view first.** A raw
`<pre>` is observed to render as a real `<pre>` with literal content inside `div.el-pre`, wrapping
(`white-space: pre-wrap`), and with a copy button **only** when it has a `<code>` child. Promoting
it to `RBlock::Code` in the model alone would get both of those wrong: the reading view's code block
sets `whitespace_nowrap` and scrolls sideways, and it draws the copy button unconditionally. So the
view needs a wrapping variant and a "no copy button" case before the model promotion is worth
making; doing the model half on its own would trade one wrong rendering for another.

### `propertiesInDocument: hidden` still leaves a 44 px gap in Live Preview

*low, bug, small*

**Landed 2026-10-24** (`7ebc6e2`): `Editor::widget_is_hidden` gives the properties widget no room
at all under `hidden`, in the prepaint and in `height_of` alike. The leftover was
`measured.height.max(line_height)` on an element the renderer draws as empty.

**Scoria.** The reading view honours the setting exactly — with it `hidden`, a note with no
frontmatter and a note with two properties both put their first clickable content in the band
82..106, a zero-pixel difference. Live Preview does not: the same two notes give first content at
y 62..78 and y 106..128, a 44 px difference where there should be none. The second pass measured a
24 px residue before the campaign, so the gap moved rather than closed.

**Obsidian.** Under `hidden` the in-document block is absent in both views; only the inline title
remains in the header (Specification.md's properties-mode table).

**Fix.** Find what the editor still reserves for the properties widget when
`PropertiesInDocument::Hidden` — the reading view's guard is in `Renderer::properties`, which both
views share, so the leftover height is in the editor's own widget accounting
(`widget_heights`/`height_of`) rather than in the renderer.

## Reading view

### A block jump lands on the wrong block after a `%%comment%%` or a footnote definition

*medium, bug, small*

**Landed 2026-10-24** (`cb2c021`): `Builder::blocks_with_starts` builds `blocks` and `starts`
together at the top level so neither can drop a block the other kept, and the appended Footnotes
section gets a start of its own.

**Scoria.** `Model.starts` has one entry per **parsed** block while `Model.blocks` is a `filter_map`
that drops comments and footnote definitions, so `block_at` and `scroll_to_block` index two
different lists. Measured: the same note with and without one leading `%%a block comment%%` jumps to
`{item:1}` and `{item:2}` for the same heading; a note with two footnote definitions pushes the
index past the end, so `fn#Tail Heading` is clamped to `{item:48}` of a 64-block list and the reader
lands nowhere near the heading. Every heading or offset jump — from the Outline, from Backlinks,
from a `[[Note#Heading]]` — is affected in any note holding a block comment or a footnote.

**Obsidian.** Not a parity question: this is one internal index space disagreeing with another.

**Fix.** Build `starts` with the same predicate the builder drops on, so the two cannot drift
(model.rs:232). Sturdier: have `Builder::blocks` return `(RBlock, usize)` pairs and derive `starts`
from the built list, and push one more start for the appended Footnotes section so an offset inside
a definition resolves to that block rather than past the end.

### No keyboard at all: neither scrolling nor selection nor copy

*high, missing-feature, large*

**Landed 2026-10-25, the scrolling half** (`c95d250`): PageDown/PageUp, Space/Shift+Space, Up/Down, Home/End and Ctrl+Home/Ctrl+End are bound in the `ReadingView` context and scroll the list.

**The rest landed 2026-10-30.** A selection is an anchor and a head, each
`{block, rich, offset}`, ordered over a document-order walk of every `RichText`
in the note rather than by id, since ids come off a build counter. A press,
drag, double click (a word) and triple click (a run) hang off each run of text
rather than off the view's root, because gpui can only be asked where a point
fell once the text has been laid out *and* prepainted, and `list` measures items
it never paints. Offsets are exact (`TextLayout::index_for_position`, taking the
nearest byte as well as a hit), walked back to a char boundary before any slice.
The wash goes through the existing highlight path, splitting any style run it
starts or ends inside, so selected bold stays bold. `Ctrl+A` takes the note,
`Ctrl+C` copies plain text with runs joined by newlines and a table row's cells
by tabs, and `reading.selection` reports it all on the control socket.

Not selectable, having no inline model: code blocks (which keep their own copy
button), raw HTML, display math and the properties block. **Known limits**: a
paragraph with inline `$x$` lays out a placeholder-substituted string, so an
offset inside such a run is in the shaped string's space and can be off by the
difference between the TeX source and its placeholder; Shift+click to extend and
keyboard-driven selection are not built; and the clipboard *write* is unasserted
(reading it back would clobber what the user has on theirs).

**Scoria.** Focus is `{reading, [Workspace, ReadingView]}` and every key is inert. PageDown,
PageUp, Down, Up, Home, End, Ctrl+Home, Ctrl+End and Space all leave `reading.scroll` at
`{item:0, offset:0.0}`, while a wheel event in the same session moves it to `{item:2}` — so the list
scrolls and only the keyboard is dead. A press-drag-release selects nothing, `ctrl-a` and `ctrl-c`
answer `handled: false`, and `editor:copy` answers "not available in Scoria yet". The cause is one
line of policy: `view.rs:366` declares `.key_context("ReadingView")` and `bind_keys` registers every
binding with `ctx = Some("Workspace")`, so nothing is ever bound in that context.

**Obsidian.** The reading view is DOM: the page keys scroll it, click-drag selects across blocks,
double-click takes a word, Ctrl+A selects the note and Ctrl+C copies plain text.

**Fix.** Two sizes, and the small one is worth landing alone. Scrolling: add ScrollPageDown/Up,
ScrollLineDown/Up and ScrollToTop/Bottom actions, bind them with `ctx = Some("ReadingView")`, and
implement them beside `scroll_to_block` over the existing `ListState` (`scroll_to`, the already
wrapped `scroll_to_end`, and a `scroll_by(px)`). Selection is the large job: an anchor/head
(block index, `RichText` id, byte offset) on `ReadingView`, mouse down/move/up on the root div, the
ranges painted through the existing highlight path, and a Copy action joining the selected runs.

### Every reload throws away the reader's folds

*medium, bug, small*

**Landed 2026-10-26**: `set_note` clears `toggled` and `folds` only when the note changes (`same_note`), so a fold survives the watcher's reload after an autosave. **The rest landed 2026-10-27** (`1808961`): `crates/scoria/src/fold.rs`'s `FoldStore` keeps every note's fold record outside the vault in `note-fold/<vault id>.json` under Scoria's configuration folder, the way Obsidian's `localStorage` key does, and `Workspace::restore_folds` gives the reading view its keys back as a note opens. Verified 2026-10-30; nothing of this entry is open.

**Scoria.** Expand a collapsed callout in the reading view, then write the note's own bytes back
unchanged: 1.5 s later the callout is collapsed again. `ReadingView::set_note` computes
`let same = path.is_some() && self.path == path` and uses it to keep the scroll position, but
`self.toggled.clear()` three lines later runs unconditionally. Since the watcher re-enters
`show_file` on every autosave, a fold survives only until the next save of the note you are reading.

**Obsidian.** A fold survives a reload and a relaunch; Obsidian keeps it per file outside the vault,
under `<vault id>-note-fold-<path>` (Specification.md:1463-1470).

**Fix.** `if !same { self.toggled.clear(); }` — `same` is already computed two lines up. For real
persistence promote the field to `HashMap<String, HashSet<usize>>` keyed by vault-relative path and
store it outside the vault, the way Obsidian does.

### `editor:toggle-fold-properties` is a visible no-op in reading view

*low, bug, small*

**Landed 2026-10-26**: the command arm branches on the mode and folds the reading view's own
properties block from a preview leaf. The `PropertiesHost` half (editing properties from reading
view) is still open.

**Scoria.** Two thirds of this is now fixed: the gate moved from `mode.is_editing()` to
`note_has_keys()`, and `markdown:clear-metadata-properties` really does edit and autosave from
reading view. What is left is the fold: the command flips the hidden editor's `properties_folded`
while the reading view folds off its own `toggled` set keyed by `PROPERTIES_FOLD_ID`, so the block
on screen does not move — the properties band measured [212..236] before and after.
`markdown:add-metadata-property` likewise writes nothing and has no reachable row.

**Obsidian.** Specification.md:1467-1469 records a real 1.14.4 running
`editor:toggle-fold-properties` from a preview leaf and toggling the fold, and
[[Specification#Observed properties-in-reading-view behavior, Obsidian 1.14.4]] shows the in-document
block there as fully live — an "Add property" button, a key input per row and a type menu.

**Fix.** Branch on the mode in the command arm: when `!self.mode.is_editing()`, route to the reading
view's own `toggle_callout(PROPERTIES_FOLD_ID)` (make it `pub`), else keep the editor call. The
bigger half is giving `ReadingView` a `PropertiesHost` whose `on_action` emits a `ReadingEvent` the
workspace turns into the same `properties::plan` write the editor uses.

### No inline title anywhere; `showInlineTitle` has no reader

*medium, parity-gap, medium*

**Landed 2026-10-30**: `showInlineTitle` has a reader in both views, set from `apply_app_config`.
The reading view draws the note's basename at h1 size in a header row that is a *sibling* of the
list, never an item, so block indices, `scroll_to_block` and the folds keep their meaning, and it
carries the 24 px top padding the first block used to hold. The editor reserves the same height
above line 0 from the same `header_height`, so the first line sits at one y in both modes and a mode
switch does not move the text; for the scroll maths the header belongs to line 0. Only a markdown
note gets one -- not the image a synthetic model embeds, nor a file no view can draw. The h1
metrics are the reading view's own `heading_style(1)`, not a fresh measurement of Obsidian's
`.inline-title`. Click-to-rename on the inline title is **not** built, and the entry's own note
stands: what a plain click on the *view header* title does was never settled.

**Scoria.** `show_inline_title` occurs exactly twice in the workspace: the Settings row and the
serde field. Live, with it true and the properties block hidden, a note whose whole body is one
embed puts that embed at y 82..106 — the pane header plus the list's first-block padding — with no
room for an h1-sized title. The view header's title is not clickable either: a single and a double
click on it leave `modal.kind` at "none", so renaming is only reachable from F2 or the context menu.

**Obsidian.** With `showInlineTitle` (default on) the reading view opens with `div.mod-header.mod-ui`
holding the `.inline-title`, the basename at h1 size, above the properties block. What a plain click
on the *view header* title does is **not** settled — Specification.md:2026-2028 leaves it open — so
the inline title is the backed half of this entry and click-to-rename is not.

**Fix.** A `show_inline_title: bool` on `ReadingView` set from `apply_app_config`, drawn in Render as
a sibling header row above the list — a sibling, never a list item, so block indices and
`scroll_to_block` keep their meaning. Do the same in the editor so the two modes agree.

### Clicking a footnote reference scrolls to the end of the note

*medium, bug, medium*

**Landed 2026-10-24** (`cb2c021`): the model records each definition's offset in
`Model.footnotes`, and `LinkTarget::Footnote(n)` reveals it through the path every heading jump
takes -- the Footnotes block in reading, the definition's own line while editing.

**Scoria.** Now driven directly, contrary to the first pass's note that it was unreachable: 60 hits
across two x clusters on the `[^1]` and `[^2]` glyphs all take `reading.scroll` from `{item:0}` to
`{item:48}`, the clamped list end, past the whole Footnotes section — so the footnote number is
provably discarded. `LinkTarget::Footnote(_)` maps to `scroll_to_end` and touches only the reading
view, so the same click in a Live Preview widget moves nothing at all.

**Obsidian.** Each reference is `<sup class="footnote-ref"><a data-footref="1" href="#fn-1-<hash>">`
— a per-footnote anchor — and every definition ends with a `footnote-backref` link back. The corpus
holds 18 of each and 19 `<sup>` elements.

**Fix.** Record the Footnotes block's index and the per-label row order on `Model` while building,
then resolve the label and call the same mode-aware reveal the heading jumps use — `scroll_to_block`
in reading, the definition's byte span in Live Preview and Source. The superscript and the backref
arrow are separate cosmetic follow-ups.

### Headings cannot be folded; `foldHeading` has no consumer

*medium, parity-gap, medium*

**Landed 2026-10-26**: the reading view collapses a heading's section behind a hover-revealed
chevron, shares the fold with the editor and persists it outside the vault; `foldHeading` gates
the chevron. See [[Development#Folding, 2026-10-26]].

**Scoria.** `RBlock::Heading` renders as a bare styled div with no `.id()`, no hover state and no
click handler, and the model records no section extent. Click sweeps over heading rows and the left
gutter change nothing. `foldHeading` occurs only in settings.rs and config.rs.

**Obsidian.** [[Specification#Observed heading fold behavior, Obsidian 1.14.4]]: the indicator is
`span.heading-collapse-indicator.collapse-indicator.collapse-icon`, absolute at left≈32 px, opacity
0 until hover, `is-collapsed` when folded; collapsed content is absent from the preview DOM, the
fold is shared with the editor and persisted per file outside the vault, and `foldHeading` defaults
to true.

**Fix.** Record for each heading the range of top-level block indices it owns, keep
`folded: HashSet<usize>` beside `toggled`, render a hover-visible triangle as an absolutely
positioned child at -28 px, and return an empty element for indices inside a folded range. Land it
with the fold-persistence fix above, since both want the same per-path store.

### The reading model is rebuilt for every file in every mode, and the status bar says so

*low, bug, medium*

**Half landed 2026-10-24** (`48e76a2`): nothing about a build time reaches the status bar any
more -- the "reading view built in N ms" suffix is behind `SCORIA_DEBUG_STATUS`. The work
itself is unchanged: `show_file` still calls `reading::build_with` for every markdown open in
every mode, so an editing session still pays for a view it is not showing.

**Landed 2026-11-02**: a note opened in an editing mode hands the reading view its text without building it (`ReadingView::set_note_later`), and the model is built on the first read -- the switch to reading view, a fold sync, a control-socket state dump -- so an editing session no longer pays for a view it is not showing. The debug status still times the build where it happens.

**Scoria.** Opening three notes in each of live, source and reading gives the same work and the same
wording in all three: "Specification.md · reading view built in 2.9 ms",
"Development.md · … 1.4 ms", "UI parity backlog.md · … 0.8 ms". `show_file` calls
`reading::build_with` unconditionally for every markdown open before any mode check, so an editing
session pays for a view it is not showing, and the status bar reports a reading view that is not on
screen.

**Obsidian.** A leaf renders the view it is showing; nothing about build time reaches the status
bar.

**Fix.** Keep the note text on the workspace and build lazily — build only in Reading mode,
otherwise hand the view an empty model (or mark it stale) and build on the way into reading when the
stored model is not this note's. Set the "reading view built in …" status only on the path that
actually built, and give the other modes the note's own status line; see also the status-bar entry
under Workspace shell.

### Markdown image size hints are dropped

*low, bug, small*

**Landed 2026-10-30**: one `alias_parts` helper splits an alias for both spellings -- every
`|`-separated part that parses as `W` or `WxH` claims the size, the rest stays the alt -- and
`wikilink_embed` and the markdown-image arm both call it, so `![alt|100](pic.png)` and
`![[pic.png|100]]` cannot disagree again.

**Scoria.** With a real 400×300 PNG in the vault, `![alt|100](pic.png)` and `![alt](pic.png)` lay
out identically (click band 404..428) while `![[pic.png|100]]` shrinks to 180..204. The
markdown-image branch strips everything after `|` from the alt and then calls `self.embed(..)` with
width and height hardcoded `None`, while `wikilink_embed` parses the hint.

**Obsidian.** Both spellings honour the hint; `|WxH` becomes width and height
([[Specification#Observed remote image behavior, Obsidian 1.14.4 (installer build 1.13.7)]] records
it for the markdown spelling specifically).

**Fix.** Split the alt the way `wikilink_embed` does instead of discarding the tail — let a trailing
part that parses as `W` or `WxH` claim the size and keep the rest as the alt — and reuse the one
`size_hint` helper so the two spellings cannot disagree again.

### An embed inside a heading, a table cell or a callout title renders as literal `![[Name]]`

*low, bug, small*

**Landed 2026-10-30**, the cheapest correct step this entry names: the embed arm of `rich()` falls
through to the non-embed one, so an embed in a heading or a table cell is readable and clickable
rather than showing its own brackets. Drawing the embedded picture *in* the cell still wants the
inline-embed run the entry describes, and is open.

**Scoria.** `InlineKind::Wikilink { embed: true }` pushes the three literal pieces `"![["`, the
display text and `"]]"` into the builder as plain unstyled text and never attaches a link, while the
non-embed arm styles it and attaches `LinkTarget::Internal`. That path is `rich()`, which is what
headings, table cells and callout titles are built from; only paragraph content goes through
`segments()`, the one place `Segment::Embed` is produced. So an embed written anywhere but a
paragraph shows its own markup and is not even clickable.

**Obsidian.** An embed in a table cell renders the picture; in a heading it renders the embedded
content inline. Neither shows raw brackets.

**Fix.** Cheapest correct step: make the embed arm fall through to the non-embed one, so it is at
least readable and clickable. Properly: give `RichText` an inline-embed run (or let `rich()` return
`Vec<Segment>` the way `segments()` does) and draw it in the cell and heading paths.

### Callouts have no type icon, and the fold affordance is a text triangle

*medium, parity-gap, medium*

**Landed 2026-10-25** (`2891b06`): both prerequisites this entry names were met in the meantime --
the app bundles Lucide icons behind an `AssetSource`, and the 2026-10-25 probe read the icon of
every built-in type out of a real Obsidian (see
[[Specification#The lucide icon and colour of every built-in callout type]]). `callout_icon(kind)`
in `assets.rs` is that table, keyed by the canonical type so the aliases fold into it and an
unrecognized type takes note's pencil; seven drawings joined the bundle for it. The fold is
`chevron-right` collapsed and `chevron-down` expanded. What the probe did **not** settle is which
element Obsidian rotates -- it holds `lucide-chevron-down` in both states and `.callout-fold`'s own
transform reads `none` -- so the orientation here is the text triangle's, drawn with the turned
icon. The **Fix** below asks for the chevron to change with `explorer.rs`, `search.rs` and
`settings.rs`: those three moved to the icon on their own before this landed, and the two text
triangles left in the app are the Bookmarks group row and the local graph's section headers.

**Scoria.** The done-task half of this finding landed — a completed task is muted *and* struck
through. The icon half did not: the callout title row is still `div().flex_1().child(rich(title))`
with no icon slot, and the fold glyph is still `▸`/`▾`. No `AssetSource` is registered anywhere in
the app, so `svg()` would silently paint nothing.

**Obsidian.** The corpus proves the markup (`callout-icon`) but every one of those `<svg>` elements
is empty, so which lucide icon belongs to which canonical callout type is the one thing it does not
show.

**Fix.** Two prerequisites, in order: a probe that reads the icon name per callout type out of a
real Obsidian, and the app's first asset pipeline (an embedded `AssetSource` wired in `main.rs`).
`explorer.rs`, `search.rs` and `settings.rs` draw the same text triangles, so the chevron should
change with them, not alone.

### The reading view's property pills have no remove control, and the block is read-only

*low, missing-feature, medium* · **Filed:** 2026-10-28

**Landed 2026-10-30**: the reading view hands the renderer one narrow sink,
`ReadingHost::on_remove_item`, which takes `PropertyAction::RemoveItem` and nothing else and raises
`PropertyItemRemoved`; the workspace takes that the way it takes `TaskToggled`, through the editor's
buffer and `Editor::remove_property_item`, so a pill removed in either view writes the same bytes in
one undo step and the file's BOM and line endings come through untouched. A canvas card passes no
sink and draws no control, having no note to write. Still read-only in reading view, and still the
gap with Obsidian: the key and value fields, the type menu and the "Add property" row.

**Obsidian.** Obsidian edits properties in reading view as well as in Live Preview, and the reading
view's own DOM carries the control: a tag pill is `div.multi-select-pill[tabindex=0]` holding
`div.multi-select-pill-content > span` **and** a `div.multi-select-pill-remove-button` with a
`lucide-x` ([[Specification#Observed properties-in-reading-view behavior, Obsidian 1.14.4]]).

**Scoria.** Every property affordance is gated on a `PropertiesHost`, and `PropertiesHost` is built
only by `EditorElement`; the reading view passes `None`, so its whole properties block is
read-only and the pills draw no ×. The editor's pills got theirs on 2026-10-28; this is the half
that was deliberately left, because `ReadingView` holds a built `Model` rather than a buffer, and a
× that does nothing would be worse than none.

**Fix.** The write path, in two files. In `reading/view.rs`, an event beside `TaskToggled` —
`PropertyItemRemoved { path, key, index }` with an `EventEmitter` impl — emitted by a properties
host (or a narrower remove handler) the way `on_task` emits `TaskToggled`. In `workspace.rs`, one
subscription beside the `TaskToggled` one calling a `remove_property_item` that mirrors
`toggle_task`: read the note, `scoria_cache::note_properties`, `properties::rows` +
`properties::remove_item`, `scoria_formats::properties::plan`, write, refresh. The removal itself
needs nothing new — `properties::remove_item` is already the editor's.

### Raw and inline HTML is literal source; remote images render as a link

*medium, parity-gap, large*

**Half landed 2026-10-30**: the inline subset is mapped onto styles -- see the Live Preview entry
above for exactly which tags and what is left. Remote images are untouched: `![remote](https://…)`
is still an External link rather than a fetched `<img>`, and `EmbedKind::Remote` does not exist.

**The remote-image half landed 2026-11-04**: `EmbedKind::Remote(url)` is built in the model for an
`http:`/`https:` destination of `![alt](…)`, of a reference image and of a raw `<img src>`, the
`|WxH` hint read off the alt by the same `alias_parts` a vault image uses, the URL kept as written.
Both views draw it through gpui's `img()`, which fetches through the app's HTTP client
(`remote.rs`: ureq on rustls/ring, `GET` only, an image `Accept`, no referrer or cookies, redirects
followed, a 64 MB cap); gpui caches a loaded image for the session. Nothing is reserved while it
loads, and a failed load is its alt text -- the 2026-11-04 probe saw Obsidian's broken image show
the alt beside Chromium's broken glyph, not a blank. `![[https://…]]` stays a missing file and
`//host/x.png` stays a vault path that names no file, as observed: nothing is requested. **Still open**: a remote image in a heading or a table
cell is still the link it was, and one mid-sentence breaks its paragraph in the reading view (as a
vault image does) and stays text in Live Preview -- both the inline-placement case; and a 404 is
not re-requested on the next render the way Obsidian re-requests one.

**Scoria.** `line one<br>line two`, `<u>underlined</u>` and `<sup>2</sup>` all appear as their tags;
`![remote](https://…)` becomes a link-styled run with an External target, so clicking it would open
the system browser rather than draw an image. (An `<img>`-only HTML block is the one case that now
renders — see the Live Preview entry above.)

**Obsidian.** [[Specification#Observed remote image behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
every `http(s):` image destination is fetched with no prompt and rendered as
`<img referrerpolicy="no-referrer">`, `|200x100` becoming width and height, a failed load staying a
blank `<img>`.

**Fix.** Handle the subset in the model rather than the view: map `<br>` to a line break, map the
inline tags onto `InlineStyle` (adding underline, sup, sub, kbd), drop unknown tags keeping their
content, and add `EmbedKind::Remote(url)` for scheme-bearing image destinations with the `|WxH`
hint parsed as `wikilink_embed` already does, rendered through gpui's `img()` with nothing visible
on failure.

## Right dock panes and graph

### A jump into Live Preview lands short of the target and never corrects itself

*medium, bug, small*

**Landed 2026-10-24** (`7ebc6e2`): the corrective reveal is armed when a widget's measured height
differs from whatever the reveal was computed from -- `previous.unwrap_or(line_height *
lines.len())` -- rather than only when a previous height existed.

**Scoria.** Click Outline row 7 of Specification.md in Live Preview (offset 34938): the cursor goes
to line 432 but the scroll stops at line 392 with `visible_lines [392,425]`, so the heading the user
asked for is below the fold — and it stays there, byte-identical at +1 s, +2 s and +3 s. A second
click on the same row gets it right. Source mode is correct on the first click. The cause is one
predicate: after a widget's real height is measured, `previous.is_some_and(|p| p != height)` arms a
corrective reveal only when the widget had been measured *before*, so the first jump over an
unmeasured table or code fence — which the reveal computed from `height_of`'s line-count estimate —
never gets its correction. Every jump inherits it: Outline, Backlinks, Outgoing links and search
hits.

**Obsidian.** The jump lands on the target the first time.

**Fix.** Compare against the estimate rather than against `None`:
`let estimate = previous.unwrap_or(line_height * w.lines.len() as f32); if estimate != height { pending_reveal = true; }`
— exactly one corrective reveal on the frame where a widget's real height first differs from the
guess, and silence once measured. This is the remaining half of the reveal work described in
[[Development#Control socket and MCP server, 2026-10-19]].

### Graph filter chips silently reset to `graph.json` on every vault change

*medium, bug, small*

**Landed**: held by the `if !self.config_dirty` guard in `set_state` together with the panel's
debounced `schedule_save`, which the write-back entry below added. Verified 2026-10-24: with
`showTags` and `showAttachments` on, a note written from a shell left both on and `graph.json`
untouched.

**Scoria.** With `showTags: false` in `graph.json`: the graph shows 11 nodes, clicking the Tags chip
gives 14, and writing any note into the vault gives 12 — 11 plus the new note, with tags off again.
Repeated with a second note: 15 after a click, 13 after the write. `set_state` reassigns
`self.config = state.vault().graph_config().unwrap_or_default()` unconditionally, and the workspace
calls it on every vault refresh. The camera is now guarded against exactly this; the filters are
not.

**Obsidian.** A filter toggled in the controls panel stays toggled, and is written back to
`graph.json` about 300 ms later.

**Fix.** Load `graph_config()` only on the first `set_state` — a `config_loaded: bool` beside
`adopted_scale`, the same shape as the camera's `adopts_saved_scale` guard — so a vault refresh only
swaps the state and rebuilds. This must land together with the write-back entry below, or each
write is immediately re-read and undone.

### Graph wheel zoom is inverted

*medium, bug, small*

**Landed 2026-10-24** (`604ec0e`): `wheel_zoom_factor(dy) = (dy * 0.004).exp()` -- the delta is no
longer negated, so wheel-up zooms in, the direction `Ctrl+=` goes.

**Scoria.** `scroll dy:-4` takes the graph scale 0.932 → 1.413 (zoom in) and `dy:+4` takes it back
(zoom out). The control in the same session: `dy:+3` in the editor scrolls *up*, so `dy > 0` is
wheel-up — and `ctrl-=` (Zoom in) raises the scale. So the wheel disagrees both with the editor's
own convention and with the keyboard command.

**Obsidian.** Wheel-up zooms in.

**Fix.** Drop the minus in `let factor = (-f32::from(delta.y) * 0.004).exp();` so a positive
(wheel-up) delta gives a factor above 1, matching `zoom_in`.

### A local graph centred on a file the filters hide renders nothing at all

*medium, bug, small*

**Landed 2026-10-24** (`89ea6a7`): the centre is added to the index whatever the filters say, as
Obsidian's `localFile` is independent of them, and an edge whose target is the centre is kept for
the same reason.

**Scoria.** With an unlinked `lonely.png` open, `graph:open-local` gives `{local: true, nodes: 0}` —
a view named after a file, with the file not in it. Toggling the Attachments chip flips it 1/0/1/0,
which proves the filter is the cause: the centre is looked up in an index the filters have already
pruned, and the build returns an empty `Built` when it is missing.

**Obsidian.** `localFile` is a separate engine option from `showAttachments`/`showTags`; the centre
of a local graph is always drawn.

**Fix.** When the centre is missing from the index, add it rather than returning empty — the BFS
then starts from a real node and the centre survives any filter. Separately, `show_graph` should
refuse `local == true` when no file is open; today it silently falls back to the global graph, which
is better than the empty canvas it used to paint but still not Obsidian's unavailable command.

### `graph.json` is never created and never written back

*medium, parity-gap, medium*

**Landed**: `GraphView::schedule_save`/`save_now` write the file, debounced, whole when it does
not exist and merged when it does.

**Scoria.** Byte-identical through ~15 chip clicks, several wheel zooms, a local graph and a quit:
md5 `41ffa915…` before and after, mtime unchanged, and with the file deleted it is never created.
`grep` settles it — `write_config("graph.json", ..)` appears only inside two `#[test]` bodies, so
there is no production writer anywhere.

**Obsidian.** [[Specification#Observed graph.json write behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
opening the global graph creates the file once as a complete twenty-key document with nobody
touching the controls; each section-header click writes exactly one key ~300 ms later; wheel zoom
writes `scale`; the Close button writes `close`; opening a *local* graph writes nothing.

**Fix.** `GraphView::save(cx)` serialising the whole `GraphConfig` through the existing
`write_config` path (whose round-trip test already shows key order and float precision survive),
debounced ~300 ms, called from the three toggles and from `zoom_at` with the wheel-target scale
rather than the animating one, and once on the first global open. Skip it while a local graph is up.
Land it with the chip-reset fix above.

### Unlinked mentions emit duplicate rows and the count is the cap, not the total

*medium, bug, small*

**Landed 2026-10-22**: mentions are deduplicated per `(path, range)`, the section's count is the
real total, and a capped list says so with a `+`.

**Scoria.** A note named `the`: the Backlinks pane shows "Unlinked mentions (200)" over 202 entries,
of which 60 distinct `(path, offset)` pairs are duplicated — `(Development.md, 4433)` three times,
`(Development.md, 6061)` four times — every copy carrying the same line offset, so they all navigate
to exactly the same place. The 200 is the `take(MAX)` cap printed as if it were the real total. The
outgoing half of the same function already dedupes with a `seen` set; the incoming half does not.

**Obsidian.** One row per mention, and the section count is the number of mentions.

**Fix.** `found.dedup()` after the existing `found.sort()` (the tuple is `(path, line_offset, line)`,
so equal tuples are exactly the same-line duplicates), then capture `let total = found.len();`
before `.take(MAX)` and build the header from `total`, marking it capped when it exceeds MAX.

### Outgoing links rows drop the subpath and the alias, and never carry an offset

*low, bug, small*

**Landed 2026-10-22**: a Links row is labelled with the link's display text (or the written
`path#subpath`) and carries the link as written, and a click follows it through `follow_link`
exactly as clicking it in the note does, subpath included.

**Scoria.** `See [[Specification#Parser notes]] and [[GA#GA]].` gives two rows labelled
"Specification" and "GA" with detail the target filename and `offset: null` on both — the subpath is
gone, so a click lands at the top of the note. `[[GA|an alias for GA]]` renders as "GA": the alias
is gone too. The Backlinks pane does carry offsets and clicking one puts the cursor exactly there,
so the asymmetry is inside `refresh_panes`, not in the navigation code.

**Obsidian.** The pane lists each link with its display text, and clicking one navigates exactly as
clicking the link in the note does, heading and block subpaths included.

**Fix.** `outgoing_link_rows` currently drops the `LinkCache` after taking `l.link.as_str()`. Carry
the whole link: label from `display_text` when present, else the written `parse_linktext` path plus
subpath; and fill `offset` by resolving the subpath against the target's `CachedMetadata` with the
existing `scroll_to_subpath` resolver, so `pane_row`'s click handler reveals it.

### Unresolved rows in Outgoing links are inert

*low, parity-gap, small*

**Landed 2026-10-22**: an unresolved row carries the link as written too, so a click takes the
same path a dangling link in the note takes and creates the note.

**Scoria.** `pane_row` attaches cursor, hover and `on_click` only when `path.is_some()`, so an
unresolved row has no pointer, no hover and no handler at all.

**Obsidian.** Unresolved links render dimmed (`is-unresolved`) and clicking one creates the note,
exactly as clicking an unresolved wikilink in the editor does.

**Fix.** Carry the link body on the row (a new `PaneRow` variant, or an `unresolved: Option<_>`
field) so `pane_row` can route a click to the same create-from-link path the quick switcher's
Create event uses, keeping the muted colour.

### "Open graph view" while a local graph is on screen closes the graph

*low, bug, small*

**Landed 2026-10-24** (`89ea6a7`): the toggle asks `!self.graph.read(cx).is_local()`, so asking
for the global graph from a local one gives the global graph.

**Scoria.** With a local graph up, `graph:open` leaves graph mode entirely — `file.mode` goes back
to "live" and `graph.open` to false — so the user who asked for the global graph gets the editor
back and has to press it again. `open_graph_action` toggles whenever the mode is Graph, without
asking which graph is showing; `graph:open-local` is not a toggle, so the pair is inconsistent.

**Obsidian.** "Open graph view" from a local graph shows the global graph.

**Fix.** Guard the toggle on the graph being the global one:
`if mode == Graph && !self.graph.read(cx).is_local() { leave } else { show_graph(false) }`.
`is_local()` already exists.

### Local graph: one hop, interlinks always drawn, no Depth or direction controls

*low, parity-gap, medium*

**Landed 2026-10-30**: `GraphView` carries Obsidian's four options (`localJumps: 1`,
`localBacklinks: true`, `localForelinks: true`, `localInterlinks: false`), the Filters section draws
the Depth slider and the "Incoming links" / "Outgoing links" / "Neighbor links" toggles in the
probe's order when the graph is local, and `local_subgraph` records each kept node's BFS depth,
walks out to `localJumps` hops, follows an edge only in the directions the flags allow, and with
interlinks off keeps only edges whose endpoints differ by one depth -- which hides the
neighbour-to-neighbour edge. The options live for the session: Scoria writes no localgraph leaf, so
there is nowhere to persist them, and the control socket reports them instead. The slider's range is
1..5 step 1, which is **not** observed -- the probe's `localControls` records no min, max or step for
it. OBSERVE: that range, and what Obsidian's interlinks rule does at depth 2 (only the depth-1 case
was probed).

**Scoria.** The local graph of a note with two forward links, one of which also links the other,
keeps the neighbour-to-neighbour edge and every one-hop neighbour in either direction. Depth is the
literal `if depth >= 1 { continue; }`, and `grep` for `local_jumps`, `interlink`, `forelink` or
`Depth` across the graph view and the config types returns nothing — the four Obsidian options do
not exist anywhere in the tree.

**Obsidian.** The localgraph leaf's own option set is `localJumps: 1`, `localBacklinks: true`,
`localForelinks: true`, `localInterlinks: false`; with interlinks off Obsidian hides edges between
two neighbours that do not pass through the centre, and the Filters section exposes Depth plus the
three toggles.

**Fix.** Carry the four options on `GraphView`, record each kept node's BFS depth in a map rather
than a set, walk out to `local_jumps`, follow an edge only in the directions the flags allow (which
needs the directed edge list the build already has), and drop same-depth edges in `finish` when
interlinks are off.

### Backlinks and Outgoing are flat per-occurrence lists

*medium, parity-gap, large*

**Landed 2026-10-22** ([[Development#Backlinks pane, 2026-10-22]]): both panes are now
Section > FileGroup > Match with the matched range highlighted, a header row of the four
actions, per-file collapse and a Link button on every unlinked mention. What is still open is
keyboard navigation of the rows, which the search pane entry below also wants. Still so on
2026-11-04: with the Backlinks tab clicked, Down answers `handled: false` twice. The dock pane
keyboard of that day covered Tags, All properties and Outline, not these two panes.

**Scoria.** Development.md's Backlinks tab showed "Linked mentions (41)" followed by 41 rows all
labelled "Development", each detail a `{line}: {trimmed source line}` string, with no grouping and
nothing marking the matched text. Unlinked mentions carry a path and an offset but no way to turn
the text into a link, and there is no filter, sort, collapse-all or more-context control anywhere.
(A property link now shows the property name as its context — that half landed.)

**Obsidian.** A header row (search filter, collapse, show more context, sort), two sections with
count badges, results grouped per file as collapsible tree items with a per-file count, each match
showing surrounding text with the link highlighted, and a "Link" button on every unlinked mention.

**Fix.** Replace `PaneRow` with Section > FileGroup > Match and render it with the explorer's
tree-item styling, carrying the match's byte range so the context can highlight it. The Link button
rewrites the matched range to `[[Name]]` through the text crate's edit plan.

### Backlinks and Outgoing links rows have no context menu

*low, missing-feature, medium*

**Landed 2026-10-30**: the explorer's menu became `crates/scoria/src/file_menu.rs`, drawn once for
every view that raises it, and every Backlinks and Outgoing links row that stands for a file raises
it -- the probe's list for a Links row, which differs from the explorer's (Rename sits with Move and
Bookmark under one separator, there is no Make a copy and no Delete, and "Reveal file in navigation"
is extra). "Open in new window" was left out, as everywhere else, until the popouts landed
(2026-11-09). The row icons are the explorer's for
the same labels: the 2026-10-22 probe read the labels and separators but not the icons, and
"Reveal file in navigation" has no explorer twin, so its `folder-open` is marked OBSERVE in the code.

**Scoria.** A right click on a Links row, a mentions group row or a match does nothing;
the app has no file context menu anywhere yet (the Files view's is a separate entry).

**Obsidian.** [[Specification#Observed outgoing links pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
a right click on a Links row opens the file menu — Open in new tab, Open to the right,
Open in new window, ---, Rename..., Move file to..., Bookmark..., ---, Copy path, ---,
Open in default app, Show in system explorer, Reveal file in navigation.

**Fix.** Build the file menu once (the Files view needs the same one) on
`canvas::menu::context_menu`, and hang it off every row that stands for a file.

### Escape does not close a context menu

*low, bug, small* · **Filed:** 2026-10-30

**Obsidian.** Escape closes an open context menu. Every probe run relies on it: `ui-rules.mjs` and
`outgoing-links.mjs` both press Escape between captures to take one menu down before raising the
next, and the captures that follow are of a window with no menu in it.

**Landed 2026-11-02**: every menu drawn through `file_menu::menu_box` -- the workspace's file menu and its three pane sort menus, the explorer's row and sort menus -- takes the keyboard under a `Menu` key context whose Escape closes it, and hands focus back where it was; the canvas's context menu closes on its own Escape before the selection does, and the Bases header and kanban menus already did. See [[Development#Escape closes every menu, 2026-11-02]].

**Scoria.** Neither menu closes on Escape. The explorer's and the workspace's file menus are both
dismissed by `on_mouse_down_out` alone, with no key binding and no action; a press on the running
app answers `handled: false` and the menu stays. Verified in the app on 2026-10-30, with the file
menu open on a Backlinks row. This is not new -- the explorer's menu has behaved this way since it
was built on 2026-10-28 -- and it is one gesture, not two: a click elsewhere does close it.

**Fix.** One Escape action bound while a menu is open, closing it and spending the press, in
`file_menu` so both hosts get it at once. Key dispatch has a trap worth remembering here: an action
runs before `capture_key_down`, so a menu that wants Escape before anything else wants
`capture_action`.

### Tags, All properties and Outline rows have no context menu and no keyboard

*low, missing-feature, medium*

**Scoria.** A right click on a tag, property or outline row does nothing, and the rows take no
keys. Everything else these panes do landed on 2026-10-22
([[Development#Tags, All properties and Outline panes, 2026-10-22]]).

**The properties half landed 2026-11-02**: a right click on an All properties row raises Rename, Property type ▸ (the six types, the current one checked, writing `types.json`), a separator, Delete property in the warning colour; Rename prompts for the key and rewrites it in every note that carries it with the entry kept in place, Delete removes it from every note, both skipping a note with unsaved edits and taking the `types.json` entry along.

**Landed 2026-11-04**, after a probe with real input
([[Specification#Observed dock pane menus and keyboard, Obsidian 1.14.5]]): the tag and
outline rows have no context menu in Obsidian -- no `contextmenu` listener at all -- so none
was built; Rename and Delete property are disabled on `tags`, `aliases` and `cssclasses`, as
observed; and the three panes take the keyboard once their dock tab or their empty space is
clicked: Down and Up move a 2 px `--background-modifier-border-focus` ring (first or last row
when none has it, no wrap), Left folds an open row or climbs to the parent, Right opens a
folded one, Enter and Space do what a click does. See
[[Development#Dock pane keyboard and the outline highlight, 2026-11-04]].

**Obsidian.** [[Specification#Observed all properties pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
a property row's menu is Rename (`lucide-edit-3`), Property type (`lucide-menu`, with a submenu),
a separator, Delete property (`is-warning`, `lucide-trash-2`). The tag and outline rows raise no
menu and the three panes share one tree keyboard:
[[Specification#Observed dock pane menus and keyboard, Obsidian 1.14.5]].

**Fix.** The property menu on `canvas::menu::context_menu`, with Rename and Delete rewriting every
note's frontmatter through the text crate and Property type writing `types.json`; probe the tag and
outline rows before building theirs.

### The tags pane's "Expand all" and the outline's search highlight are approximations

*low, parity-gap, small*

**Settled 2026-10-30, by decision rather than by a build.** The label already never changes
(`Workspace::toggle_tag_folds` is documented as such and the button is drawn with one label), which
is the parity point; the inert first click is a flag in Obsidian that starts out of step with its
rows -- a defect, not a behaviour -- and Scoria does not reproduce it. The outline's match colour
stays the accent until a probe reads `suggestion-highlight`'s computed style; that single
measurement is all that is left, and it is listed in [[Interaction probes]] rather than here.

**Landed 2026-11-04**: the measurement is in (block 12 of [[Interaction probes]]). The only rule
on `span.suggestion-highlight` is `font-weight: bold` -- no colour, no background, no token -- so
a match is bold in the row's own colour, muted at rest and brightened under the pointer, in both
themes; the accent is gone. See
[[Development#Dock pane keyboard and the outline highlight, 2026-11-04]].

**Scoria.** "Expand all" folds every tag on its first click and opens them on the second, and the
outline's search matches are drawn in the accent colour.

**Obsidian.** [[Specification#Observed tags pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
the first click did nothing, the second folded every row, the third opened them, and the label
and icon never changed; the outline wraps a match in `span.suggestion-highlight`, whose computed
colour and weight the probe did not read.

**Fix.** Probe why the first click is inert (an internal flag that starts out of step with the
rows, most likely) and read `suggestion-highlight`'s computed style, then match both.

## Search, switcher, palette and autocompletion

### A property search is case-sensitive about the key

*low, parity-gap, small*

**Landed 2026-10-22**: the key was never case-sensitive; the query parser kept the quotes the pane
writes (`["Status"]` searched for a key named `"Status"`). The quotes are stripped now, and a
property hit highlights its key's line in the frontmatter so the row counts as a result — the
highlight is an inference from the tag hit, not an observation of Obsidian's row.

**Scoria.** The All properties row for `Status` (4 notes: `Status`, `status`, `STATUS`,
`status`) searches `["Status"]` as Obsidian's does, and the search pane answers 0 files.

**Obsidian.** [[Specification#Observed all properties pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
`["status"]` returned all 4 notes, so the key is matched case-insensitively.

**Fix.** Lowercase the key in `scoria-search`'s property matcher the way the All properties pane
merges its counts.

### Search results cannot be navigated or opened from the keyboard

*medium, parity-gap, medium*

**Landed 2026-10-22** ([[Development#Search pane, 2026-10-22]]): Down and Up move a focus
ring over the rows (file rows and match rows both), Enter opens the focused row, Left folds
and Right unfolds its file (taken from the field's own actions in the capture phase, since
gpui dispatches a key's action before any key listener sees it), and Escape clears the ring,
then hands the keyboard back to the main view. The dump carries `focused`.

**Scoria.** With results on screen and focus on the query field, Down, Down and Enter each answer
`handled: true` — the `TextInput` consumed them — and change nothing: no row becomes focused,
`file.path` does not move, and Escape is swallowed with the query intact. The pane's state dump
still carries only `query`, `error`, `files`, `matches` and `results`: there is no focused-row field
at all, and the subscription matches only `InputEvent::Changed`. Mouse clicks on results do work.

**Obsidian.** ArrowDown calls `dom.changeFocusedItem("forwards")` with no input guard, ArrowLeft and
Right collapse or expand the focused file row, and Enter calls `focusedItem.onResultClick(e)`; the
focused row carries `has-focus`.

**Fix.** A `focused: Option<usize>` index into `self.rows` (which already interleaves File and Match
rows), moved by Up/Down in the subscription, painted with `bg_active`, with Submit emitting the same
`SearchEvent::Open` the mouse path emits; bind Left/Right to the existing `toggle`, and have Escape
clear the query and return focus to the editor.

### Search pane has no Match case, no settings toggles, no sort select and no count menu

*medium, missing-feature, large*

**Landed 2026-10-22** ([[Development#Search pane, 2026-10-22]]): the `search-row` with the
Match case decorator and clear button, the Search settings button and its three toggles, the
Explain block, the results bar with the count's menu (Copy search results opens the modal
with its toggle, two selects and two buttons; Bookmark... goes to the bookmarks prompt) and
the sort control, and the six leaf keys in `workspace.json`. What remains unlike Obsidian's:
the sort control and the modal's two selects are menus, not native `<select>`s, and the
modal's text is a read-only preview rather than a textarea.

**Scoria.** `SearchPane::render` emits exactly three things: the input, an optional summary line and
the list. `Search::new` is always called without a case flag, `run_search` always sorts one way, and
`Query::explain` is never called. A grep for `matchingCase`, `sortOrder`, `explainSearch` or
`extraContext` across the app and the config types returns nothing at all — none of the six keys
Obsidian serialises exists. Per-file collapse is the one adjacent thing that landed.

**Obsidian.** [[Specification#Observed search pane behavior, Obsidian 1.14.4]]: a Match case
decorator, a sliders button revealing exactly three toggles (Collapse results, Show more context,
Explain search terms), a result-count control whose left click opens a two-item menu, and a native
`<select>` with six sort orders in the `search-results-info` bar.

**Fix.** A `SearchLeafState` of those six keys on `SearchPane`, a header row above the input, Match
case wrapping the parsed tree in `Query::CaseSensitive`, and `sort_order` threaded into
`run_search`'s sort using `VaultFile` mtime and birthtime.

### The result summary disappears when the query is emptied

*low, parity-gap, small*

**Landed 2026-10-22**: a `has_run` flag, and the count reads "0 results" once a query has
run in the session.

**Scoria.** Search `kiwi`, then Ctrl+A and Backspace: the pane reports
`{query:"", files:0, matches:0, results:[]}` and the summary line is suppressed entirely, because
the empty-query arm returns an empty string and render only draws a non-empty one. (The other two
thirds of this finding are fixed: the summary is now `{total} results` with no file count, and a
name-only hit correctly adds to that total without adding to the file's chip.)

**Obsidian.** Once a search has run in the session Obsidian shows "0 results"; only a fresh restart
shows nothing.

**Fix.** A `has_run: bool` set when a non-empty query is dispatched, and the empty-query arm
returning "0 results" when it is true.

### The palette lists ids it cannot run, and omits ids Obsidian does have

*low, parity-gap, large*

**Landed 2026-10-24** (`39c58bc`), the first half: `Workspace::can_run` is the check callback --
implemented, plugin on, a table command needs the cursor in a table, an `EDITOR_ONLY` id needs the
note with the keyboard -- and the palette filter and the socket's `runnable` flag both read it.
**The second half closed 2026-10-30 on inspection**: `workspace:close`, the two splits and (since
folding landed on 2026-10-27) `editor:toggle-fold` are all in `RUNNABLE` and dispatched in
`run_command`; `app:reload` is the one id still absent, and that is deliberate -- Scoria has no
renderer to reload. Nothing of this entry is open.

**Scoria.** Running an editor-only command in reading view now says why — "Insert table: only
available while editing" — and the three properties ids that Obsidian really does run from a preview
leaf were exempted again in wave 5. What did not change is the list itself: `open_palette` still
filters on the static `RUNNABLE` plus an in-table check, so in reading view "template" still offers
the three Templates rows. The other direction is also still true: of 244 registered commands 86 are
runnable (up from 59), but `editor:toggle-fold`, `workspace:close`,
`workspace:split-vertical`/`-horizontal` and `app:reload` are all `runnable: false` and absent from
the palette although they are real entries in a 1.14.4 palette.

**Obsidian.** An `editorCallback` command is filtered out of the palette when there is no editor to
run it in, rather than listed and refused — and the ids above are offered.

**Fix.** Replace the static `RUNNABLE` allowlist with a `can_run(&self, id)` mirroring Obsidian's
check callbacks (editor ids need an editor with the keyboard, file ids need an open file), and
update the three call sites — the palette filter, the hotkey binder and the socket's `runnable`
flag — together so they cannot disagree. Keep the spoken refusal: it also covers the hotkey path,
which a palette-only filter would leave silently inert.

### The quick switcher ignores `switcher.json`

*low, bug, small*

**Landed 2026-10-24** (`43803d7`): `offers` and `offers_unresolved` read all three keys.
`showAllFileTypes` gates `FileKind::Other` *behind* the vault-wide `showUnsupportedFiles`, since a
file the vault does not list cannot be offered here either.

**Scoria.** With `{"showExistingOnly": true, "showAttachments": false}` written and the app
relaunched, the switcher still offers an unresolved target for the query `Nonexistent` and still
offers `pic.png` for `pic`. `set_state` reads only `app().show_unsupported_files`, the unresolved
loop runs unconditionally, and `state.switcher()` is read nowhere outside settings.rs.

**Obsidian.** "Show attachments" off hides images, PDFs and other attachments; "Show existing files
only" on hides the not-created-yet targets. Scoria already models and writes both keys.

**Fix.** Read `state.switcher()` at the top of `set_state`: skip `Image | Audio | Video | Pdf` when
`show_attachments` is false, use `show_all_file_types` for `FileKind::Other`, and guard the
unresolved block behind `show_existing_only != Some(true)`. Call `set_state` again on
`SettingChange::Switcher` so a toggle takes effect without a restart.

## Settings

*Settings moved out of the workspace into a window of its own on 2026-10-30
(see Development's [[Development#Settings in a window of its own, 2026-10-30]]).
The two entries below landed against the modal, so their `#settings-frame`,
backdrop and viewport-width details describe a frame that no longer exists;
what they fixed — the control column inside the window, a scrolling nav — is
still in the panel.*

### On a window narrower than about 1180 px no control in the modal can be clicked

*medium, bug, small*

**Landed 2026-10-24** (`539384e`): the frame carries the width,
`w(relative(0.9)).max_w(px(1100.))`, and the panel is `w_full().max_w(px(1100.))`.

**Scoria.** At 900×800: the nav column still works (clicks at x=60 and x=140 switch tabs), but a
sweep of x = 700, 760, 780, 800, 810, 820, 830, 845 across six row heights left `app.json`
byte-identical and `modal.menu`/`modal.editing` null every time, and x ≥ 860 is the backdrop, which
closes the modal. The identical click at x=1190 on a 1400 px window flips the toggle. The panel is
still a hard `.w(px(1100.))` inside a frame that is only `.h_full().max_w_full()`, so the frame is
clipped to the window while the panel keeps laying out at 1100 px and the whole control column sits
past the clip. The campaign fixed how the modal handles the mouse and never checked that the
controls were under it.

**Obsidian.** The settings modal fits the window.

**Fix.** `.w_full().max_w(px(1100.))` on the panel and `.w(relative(0.9)).max_w(px(1100.))` on
`#settings-frame`. The nav is already `flex_shrink_0` and the content pane already `flex_1().min_w_0()`,
so the content column absorbs the shrink and the controls come back inside the window.

### The Settings nav column does not scroll

*low, bug, small*

**Landed 2026-10-24** (`539384e`): `.id("settings-nav")` and `.overflow_y_scroll()`.

**Scoria.** At 1400×320, the nav shows general through core-plugins and everything below is clipped:
y = 284, 300 and 316 land on the backdrop and close the modal, and a wheel event over the nav leaves
the same four y values mapping to the same four tabs. `render_nav` builds a plain
`div().flex().flex_col().w(px(220.)).h_full()` with no id and no `overflow_y_scroll`, inside a root
that is `.overflow_hidden()`.

**Obsidian.** The settings nav scrolls.

**Fix.** Give the nav div an `.id("settings-nav")` and `.overflow_y_scroll()`, exactly as the content
list already does.

### Every core-plugin options tab but two shows one "Options not shown yet" row

*medium, missing-feature, medium*

**Landed 2026-10-30, whole.** The Daily notes and Templates tabs came first (see
[[Development#The Daily notes and Templates options tabs, 2026-10-30]]); the probe of the same day
([[Specification#Observed core-plugin options tabs, Obsidian 1.14.5 (installer build 1.13.7)]])
then settled every other tab, and all seven landed: Backlinks, Canvas, Page preview, Note composer,
Unique note creator, File recovery and Web viewer, each with its own `#[serde(flatten)] extra`
config type in `scoria-formats`, loaded by `VaultState` and written through its own `SettingChange`.
Only twelve core plugins have a tab at all. Sync gets a row of its own saying it is a paid Obsidian
service Scoria does not speak, rather than the placeholder, which would have been false twice over.

**Every new row is tagged "not in Scoria yet", and that is the honest part**: the values are written
for the vault, which Obsidian reads, but no Scoria behaviour reads one of these keys yet. The
ratchet test now runs in both directions -- a toggle may lose its tag only when something really
reads the key.

The rows with no observed key are drawn and write **nothing**, rather than guessing: the canvas
folder name and zoom threshold, all three Unique note creator rows, all four File recovery rows, the
Note composer template path, and the Web viewer homepage, saved-page folder, ad-block rules and
update frequency. OBSERVE: each of those keys needs a probe that drives a file suggester, types into
a `type=number` field, edits a textarea or drags a slider.

**Scoria.** `plugin_items` matches only `switcher` and `command-palette`; everything else falls to a
catch-all returning the single row "Options not shown yet — Scoria does not read this plugin's
settings file yet." The configs are already there: `DailyNotesConfig` and `TemplatesConfig` exist in
`scoria-formats`, and their byte-exact round trip is already asserted in the vault tests.

**Obsidian.** Each enabled core plugin's tab carries its own options.

**Fix.** Add arms for `daily-notes` (format, folder, template, "Open daily note on startup") and
`templates` (folder, dateFormat, timeFormat) using the existing `field()`/`Typed` shapes, backed by
those config types, plus the matching `SettingChange` variants. While there, expose the rendered row
names in `state.modal.rows` — see [[#Blocked by the harness, not by the code]].

### An unparseable `.obsidian` json is replaced wholesale on the first write

*low, bug, medium*

**Landed 2026-10-24** (`95b4071`): `write_config_with` returns `VaultError::Unparseable` rather
than replacing, for the merging write and the whole-file one alike, and the window names the
files it is standing in for on the way in. Valid JSON of the wrong shape still merges.

**Scoria.** With `app.json` holding `{"readableLineLength": false, "myPluginData": {"keep": "this"},}`
— a trailing comma — the app launches with no warning anywhere in `state`, and one toggle click
rewrites the file to exactly `{"readableLineLength": false}`. `myPluginData` is gone, and the
in-memory config had silently fallen back to all-defaults, so the toggle also ignored the value the
file actually held.

**Obsidian.** Not an Obsidian question: this is the repo's own ground rule — never write a file
Scoria did not change, preserve unknown keys — and the code comment at
`scoria-vault/src/lib.rs:359` admits the behaviour is deliberate.

**Fix.** Make the unparseable branch refuse instead of overwrite (a `VaultError::Json` that
`change_setting` surfaces), and carry the load error too, so Settings can show "app.json could not
be read; settings are showing defaults and will not be written" instead of pretending the defaults
are the vault's values.

### Core-plugin toggles write the file and change nothing in the app

*low, parity-gap, medium*

**Landed 2026-10-24** (`1bb40b6`): `scoria_commands::plugin_of` maps an id to its plugin and
`retain_enabled` drops a disabled plugin's commands from the registry, which is what the palette
lists and what the keys are bound from. The dock tabs follow the same flag and `run_command`
refuses an id whose plugin is off.

**Scoria.** Turning off Files, Graph view, Command palette and Daily notes wrote
`core-plugins.json` correctly — key order and every untouched key preserved — and nothing changed:
the explorer is still there, Ctrl+P still opens the palette whose first row is literally "Command
palette: Open command palette", and "daily" still lists the three Daily notes commands. The Settings
nav *does* honour the file, so the mechanism exists one function away.

**Obsidian.** A disabled core plugin is unloaded: its view disappears and its commands are
unregistered, so neither the palette nor the Hotkeys tab lists them.

**Fix.** Reuse the filter `render_nav` already has: a `plugin_enabled(state, id)` applied in
`load_commands` and in the palette's list, both rebuilt after a `SettingChange::CorePlugins`. Hiding
the Files/Graph/Backlinks panes is the larger half and can follow.

### "Default location for new attachments" is inert

*low, parity-gap, medium*

**Landed 2026-10-28**: `ATTACHMENT_LOCATION` is a real four-option `Typed`
dropdown writing `/`, `./`, `./Attachments` and `Attachments`, and the
Subfolder name / Attachment folder path rows appear off the selected option.
The resolution rule is `scoria_vault::ops::attachment_path`, with the spec's
six rows as tests. The row keeps its `not_yet()` tag even so: nothing in
Scoria creates an attachment, so the value really is written for Obsidian and
nothing else, which is what `Honored::No` means. The tag comes off with the
first caller. See [[Development#The settled UI rules, 2026-10-28]].

**Scoria.** Clicks down the whole row band leave `modal.menu` null and `app.json` byte-identical, and
no "Attachment folder path" or "Subfolder name" row ever appears. The row is built with
`dropdown(...).not_yet()`, and `dropdown` is a display-only control with no `Typed` behind it, unlike
`choice`. The dependent rows are pushed only when the stored value already has that shape, so there
is no path out of the default state.

**Obsidian.** Four options — Vault folder (`/`), Same folder as current file (`./`), In subfolder
under current folder (`./<name>`) and In the folder specified below (`<folder>`) — each writing
`attachmentFolderPath` at once, with the dependent field appearing for the chosen option.

**Fix.** Replace the `dropdown(...)` with a `choice(...)` over a `Typed` on `attachmentFolderPath`,
and key the dependent rows off the selected option rather than the stored string's shape. The
`ATTACHMENT_FOLDER` text field already implements the write, Obsidian's empty→"Attachments" rule
included. Keep the `.not_yet()` tag until the labels and stored values are probed —
[[Interaction probes]] records them as an unperformed probe.

### A click that dismisses a dropdown also activates whatever it lands on

*low, parity-gap, small*

**Landed 2026-10-28**: `popover_swallows_click` and `escape_closes` are pure
decision functions that every clickable in the modal asks first, so a click
that dismisses a dropdown is spent on the dismissal and Escape over an open
list closes the list rather than the modal. Both are unit-tested, and routing
every control through them is what keeps the next control added from
forgetting. See [[Development#The settled UI rules, 2026-10-28]].

**Scoria.** Open the `newFileLocation` menu, then click the "Always focus new tabs" toggle: the menu
closes **and** `focusNewTab` flips, in one click. Escape with a dropdown open closes the whole modal
rather than just the popover.

**Obsidian.** The control here is a native `<select>` (captured in the properties probe's JSON),
which swallows the first outside click.

**Fix.** Have the root's `dismiss_open_controls` stop the click when it actually dismissed a menu,
and bind Escape to close the popover first and the modal only when nothing is open. Minor, and
arguably better as it is — decide it deliberately rather than by accident.

## Source editor

### Right-clicking in the editor does nothing: no context menu, no caret placement

*low, missing-feature, large*

**Landed 2026-10-30**, on the probe of the same day
([[Specification#Observed editor context menu and view header actions, Obsidian 1.14.5 (installer build 1.13.7)]]).
A right click places the caret, keeps a selection it lands inside and moves the
caret when it lands outside -- and the rows follow the probe's mixed behaviour
exactly: the Search row is built from the selection *at the click*, Cut and Copy
from the one that *survived* it, so a click outside a selection carries
`Search for "…"` with Cut and Copy drawn disabled. The menu is `file_menu`
extended, not a second widget: a new `Target::Editor`, three new sections and a
`disabled` flag the renderer draws greyed and inert. A wikilink wraps the text
rows in the whole file menu; a dangling link reads as plain text, since Rename
and Move would have no file. The reading view gets **no** text menu, as
observed -- only a link raises one.

Rows left out for want of a command, rather than drawn dead: the **Format,
Paragraph and Insert submenus** (never opened in the probe, so their contents
are unobserved and were not guessed -- OBSERVE), *Extract current selection...*
(no note composer), *Edit link* and *Edit tag* (no inline rename), *Open in new
window* and *Start presentation*. Dropping the last empties the `view` section,
so the capture's two consecutive separators are one here.

**Scoria.** A right click leaves `modal` at `{kind: "none", rows: []}`, the selection untouched and
the caret where it was; `editor:context-menu` answers "Show context menu under cursor: not available
in Scoria yet" and is `runnable: false`. The mouse-down hook filters on
`event.button == MouseButton::Left`, and a grep for any context-menu widget across the app returns
nothing — the file explorer's deferred menu is the only menu in the tree.

**Obsidian.** Right-click places the caret (or keeps the selection when the point is inside it) and
opens a context menu of file and editor commands.

**Fix.** Accept `MouseButton::Right` in the mouse-down hook — the hitbox guard beside it is already
correct — place the caret when the point is outside the current selection, and emit an
`EditorEvent::ContextMenu { position }`. The bulk of the work is the menu widget itself: a positioned
overlay list dispatching command ids, which the explorer's menu could be generalised into.

## Workspace shell

### The frame style has no store, and four title-row controls are still missing

- **Severity:** low · **Kind:** parity-gap · **Filed:** 2026-10-20

**Obsidian.** "Window frame style" sits in Settings → Interface with `hidden` (the Linux default),
`custom` and `native`, needs a restart, and writes nothing under `.obsidian` — the choice is kept
in Obsidian's own config directory, outside the vault
([[Specification#Observed shell typography and chrome, Obsidian 1.14.4 (installer build 1.13.7)]]).
The hidden frame's title row also has a tab-list button (`lucide-chevron-down`) at the right end of
the tab strip, and the view header has a bookmark action and a "More options" menu beside the mode
toggle; the Files pane header has its five buttons (#24).

**Landed 2026-10-30, the store**: the "Window frame style" row is a real dropdown
(`settings.rs` `FRAME_STYLE`, the three options and stored strings of the 2026-10-20 probe) and a
pick is `SettingChange::Frame`, which `Workspace::change_setting` writes to the `frame` key of
`scoria.json` beside the vault list -- where Obsidian's own `obsidian.json` keeps its `frame` -- and
never to the vault; `main.rs` reads it on the next start, under `--frame` and `SCORIA_FRAME`, which
now override it for one run. The row keeps its "on restart" tag because the open window keeps the
frame it was opened with. The two header controls are the half still open; see the probe of the
same day below.

**Scoria.** Since [[Development#The frame, the type and the icons, 2026-10-20]] the window draws
Obsidian's hidden frame by default and `--frame native` / `SCORIA_FRAME=native` asks for the
desktop's; the Interface tab shows the row with that value but cannot change it, since Scoria has
no per-user settings file to keep it in. The tab-list button and its menu are drawn since
2026-10-22; the bookmark action and the "More options" menu are not, and on 2026-11-04 the view
header still held only the local-graph and mode buttons.

**Fix.** A per-user config file (XDG config dir on Linux) holding the frame style, read before the
window opens; then the two remaining controls, each after a probe of what its menu holds.


### History back and forward restore the file but not the mode or the scroll

*medium, parity-gap, medium*

**Landed 2026-10-24** (`de99e7c`): `HistoryEntry` carries the path, the mode and the offset -- the
caret while editing, the top block in Reading -- written just before every navigation away and
kept current by `set_mode`.

**Scoria.** Open a note in Reading, open another in Source, go back: the first comes back in
*source*. Scroll likewise — a note scrolled to editor line 400 comes back at line 0. The reading
view also loses its place on any mode switch or revisit, which the editor does not. (The graph half
of this finding is now its own entry above, because it turned out to affect every open route and not
only history.)

**Obsidian.** Each history entry stores the leaf's full view state, so Navigate back returns to the
mode and position the note was left in.

**Fix.** `history: Vec<HistoryEntry { path, mode, offset }>`; stamp the outgoing entry in
`open_file` before pushing, and in `navigate_history` call `set_mode` and `reveal_offset` after
`show_file`. For the reading view, keep a per-path scroll beside `set_note` the way the editor keeps
its line.

### Status bar shows parse timings where Obsidian shows the backlink count

*medium, parity-gap, small*

**Landed 2026-10-24** (`48e76a2`): the right half is `{n} backlink{s} · {words} words, {chars}
characters`, with `Ln, Col` in front of it while editing and nothing at all with no file open.
The parse, span and shape numbers and the "reading view built in" suffix are behind
`SCORIA_DEBUG_STATUS`.

**Scoria.** At rest the left half reads "Welcome.md · reading view built in 0.0 ms" — in every mode,
including Source, where no reading view is on screen — and on an empty vault "0 files, 0 notes
indexed in 8 ms". The right half is
`Ln {}, Col {} · parse {:.1} ms, {} spans · shape {:.2} ms · {words} words, {chars} characters`, and
nothing renders a backlink count although `self.linked` is already computed for the right dock.

**Obsidian.** The default status bar is right-aligned only and carries the Backlinks item
("N backlinks") and the Word count item; there is no cursor position and no diagnostics.

**Fix.** Keep only "Ln, Col" (plus the cursor-count clause) and move the parse/spans/shape fragment
behind a debug flag read once at startup. Add an `{n} backlinks` child from `self.linked.len()` left
of the word count, and drop the "· reading view built in N ms" suffix from the status `show_file`
writes, so the left half carries notices only. The word count should also be skipped when no file is
open.

### Renaming with an empty name reports "File name must not start with a dot."

*low, bug, small*

**Landed 2026-10-24** (`ca2a63e`): the stem is checked before the extension goes back on, so an
empty box reads as empty.

**Scoria.** `workspace:edit-file-title`, Ctrl+A, Backspace, Enter: `modal.error` is "File name must
not start with a dot." with an empty query — byte-identical to the pre-campaign observation.

**Obsidian.** "File name cannot be empty." — which Scoria's own New folder prompt already produces,
because that one validates the whole value.

**Fix.** Not in the validator: `validate_file_name` already tests empty before dot and already
returns the right string. The empty value never reaches it, because `prompt_submitted` builds
`format!("{value}.{ext}")` first, so `""` becomes the path `.md`, which is non-empty and
dot-prefixed. Guard the stem in the `PendingOp::Rename` arm before the extension is appended — the
same guard the `/`-accepting entry under Files needs — and add it to the `PendingOp::Move` arm too,
where an empty value silently renames the file to itself.

## Bases

Filed 2026-11-01 from a pass of its own: every surface of the Bases view compared between
Obsidian 1.14.5 (installer build 1.13.7) and Scoria at `48d66a9`. The Obsidian half was driven over
CDP by `tools/obsidian-probe/bases-ui.mjs` on a scratch vault with eight typed notes and a
four-view base — 26 steps in `tools/obsidian-probe/results/2026-11-01-bases-ui.json`, 343
screenshots in `2026-11-01-bases-ui-shots/` (untracked, 30 MB; each step's `shot` names its file).
The Scoria half was driven over the control socket in a headless sway session on a copy of
`docs/scoria-demo`, 105 captures with the state dump beside each. The public docs and changelogs
through 1.14.5 were read for the vocabulary, and the 2026-10-25 record was re-read: it already held
all eight toolbar items with classes, labels, icons and rects, and the full markup of every panel,
while `toolbar.rs` and [[Specification#What building the rest of the Bases view settled, 2026-10-25]]
say the toolbar was never measured. Those comments are stale, and so is the `View 2` name in
`bases/commands.rs`: Obsidian names every added view `View`, the fifth one included.

Two things the records settle that earlier entries left open: **cells are editable in place** in
Obsidian (the disabled checkbox of 2026-10-25 was a cards-view reading), and a **new empty base
shows exactly one column, `file name`**, over every file in the vault.

Order below is roughly the order a user meets them. Icons are Lucide ids.

### The toolbar is five icons in Scoria's order; Obsidian's is eight labelled buttons

*high, parity-gap, medium*

**Landed 2026-11-01**: the bar is the eight labelled buttons in Obsidian's order and icons, accent-coloured while their control holds a value, no tooltips; Search shows the `Find...` row with *Showing N* and narrows the rows; New creates the note with the frontmatter the filters imply. See [[Development#The Bases toolbar and its panels, 2026-11-01]].

**Obsidian.** `.bases-toolbar`, 29 px, the same eight items in every layout, each a 25 px
`text-icon-button` with a 13 px label, radius 5, `--text-muted` idle, hover
`--background-modifier-hover`, `--text-accent` with `is-active` when the control has a value, no
tooltips anywhere on the bar. Left to right: the view switcher (the layout's icon `table` /
`layout-grid` / `list` / `kanban-square`, the view's name, `chevrons-up-down`); **7 results**
(`text-align-start`, the live count); **Sort** (`arrow-up-down`, a `.flair.toolbar-badge` with the
count); **Group** (`stretch-horizontal`); **Filter** (`list-filter`, badge of the view's own
filters); **Properties** (`list`); **Search** (`search`); **New** (`plus`). Search reveals a
`.bases-search-row` under the bar with a `Find...` input and `Showing N`; Escape clears and hides
it. New creates `Untitled.md` with its frontmatter prefilled from the filters and the group
(`tags: [rock]`, `rating: 1`, `status: done`, empty keys for the view's other properties) and
opens it in a `bases-new-item-popover` editor with the inline title focused.

**Scoria.** A 30 px bar: the switcher at the left, then a spacer, then five 22 px icon squares at
the right — `sliders-horizontal`, `square-check`, `arrow-up-narrow-wide`, `layers`, `list` — with
tooltips of the bare words `filter`, `properties`, `sort`, `group`, `results`. No count, no
badges, no active colour, no Search, no New. `toolbar.rs:96-100` calls the order and icons its own.

**Fix.** Redraw the bar from `s02-toolbar-per-layout` of the record: labelled buttons in
Obsidian's order with its icons and badges, the count as a button, a search row, and `New` running
the add-item path with the prefill (that path is the entry below).

### No panel closes on Escape or on a second click, and tooltips draw over them

*medium, bug, small*

**Landed 2026-11-01**: Escape closes one layer at a time (menu, list, panel, search row), a second click on the button closes, the tooltips are gone, the views and properties panels focus their search field where Down, Up and Enter work, and a pick from a list closes the list alone.

**Obsidian.** Escape closes every panel and returns focus to its button; the views and properties
panels focus their search input on open, Down moves `is-selected`, Enter picks or toggles; the
sort, filter, group and results panels leave focus on the button. Clicking the button again
closes.

**Scoria.** Escape closes nothing in the Bases view — not a panel, not the views menu, not a combo
list, not the header menu. Clicking the open panel's button leaves it open; only an outside press
closes it. The button's tooltip is drawn over the open panel while the pointer rests on it. After
Escape on a combo list the next click in the page is swallowed. Picking from the group panel's
`+` list or from the Configure page's Layout combo closes the whole panel. Tab in the properties
field inserts a literal tab. This is the same gap as [[#Escape does not close a context menu]],
one tier wider.

### The view switcher has no search, no gear menu, and names a new view `View N`

*medium, missing-feature, medium*

**Landed 2026-11-01**: the search field, the gear, the Configure page with its name field and Obsidian's labels, the header menu (Default view, Duplicate view, Delete view) and Add view naming every view `View`. **Still out**: the list layout's *Indent properties* toggle, whose key is unobserved. **Landed 2026-11-04** after the probe (`bases-leftovers.mjs` `s02`): the toggle sits between Markers and Property separator and writes `indentProperties: true` or `false` (never dropping the key), and the list draws each value on its own bulleted line 33.75 px in from the title, with no separator.

**Obsidian.** The panel (228 px, `.bases-toolbar-views-menu`) opens with a `Search...` input that
filters the rows live; one row per view with its layout icon, the active one `mod-active`, and a
`chevron-right` gear at the right; then **Add view** (`plus`). The gear opens the Configure view
page in place: a header with `chevron-left` **Configure view**, a `more-vertical` menu and an `x`
close; a **View name** text input; the **Layout** combobox (Table / Cards / List / Kanban with
icons; Table always marked, Obsidian's own bug); then the layout's rows under these labels —
table **Row height** (Short / Medium / Tall / Extra tall); cards **Card size** (50–800 step 10),
**Image property** (a clearable combobox of `file` and every property with type icons), **Image
fit** (Cover / Contain), **Image aspect ratio** (0.25–2.5 step 0.05); list **Markers** (Bullet /
Number / None), **Indent properties** (checkbox), **Property separator** (text, `, `); kanban
**Column width** (200–500 step 10), then the three image rows. The header's `more-vertical` menu:
**Default view** (`layers-2`, `mod-checked`), **Duplicate view** (`files`), separator, **Delete
view** (`trash-2`, `is-warning`). Add view opens that page with the name `View` prefilled and
writes `- type: table` / `name: View` on close — `View`, whatever the count. Right-click on a row
is a plain click.

**Scoria.** Rows with an icon and a name, nothing else; the row's icon opens Configure. No search,
no Add view row, no Default / Duplicate / Delete, no close button, no name field. The Configure
rows are labelled with the raw keys — `cardSize`, `image`, `imageFit`, `imageAspectRatio`,
`columnWidth`, `markers`, `separator` — and Indent properties is not drawn. `bases:add-view`
names the view `View N` and opens Configure without the name field.

### The properties panel is unordered, unstyled, overflows the window, and has no formula editor

*high, missing-feature, large*

**Landed 2026-11-01**: the observed order and styling, scrolling inside the window, Add formula, Hide all (`order: []`), `Add "x" property`, and both gear pages — display name and property type, or the formula with its live validity line. **Still out**: the editor's completions. **The completions landed 2026-11-04**: typing in a formula editor -- the properties panel's and the Add summary page's -- lists the sixteen globals as `name()`, the idents `file`, `formula`, `note` and `this`, and the vault's frontmatter names, by case-insensitive prefix and in name order, each with its type icon, as the run's own `d` list (`date()`, `density`, `done`, `due`, `duration()`) shows; after `file.`, `this.`, `note.` and `formula.` the list is that receiver's, from [[Specification#The function table]]; a summary's editor adds `values`. Up/Down and Enter or a click take a row, and Escape closes the list before anything else. `bases::complete` holds the rules and their tests. **Still out**: Obsidian's description panel beside the list, the matched prefix's own style, and any receiver whose type needs the expression evaluated (`status.`, a call's result).

**Obsidian.** `Find or create...` input; rows of checkbox, type icon (`info` for `file.*`, `text`,
`binary`, `square-function`, …), name and a `chevron-right` gear; the shown columns first, then
the hidden `file.*` rows (`mod-implicit`), then hidden formulas (`mod-formula`), then note
properties, hidden rows in `#666`; actions **Add formula** (`square-function`) and **Hide all**
(`eye-off`, writes `order: []`). An unknown name puts `Add "x" property` (`plus`) first. The gear
opens **Edit &lt;name&gt;**: **Display name** input and **Property type** select (Checkbox / Date /
Date &amp; time / List / Number / Text). On a formula it opens **Formula**: Display name, then a
CodeMirror editor with placeholder `x + y`, bracket auto-pair, a status row (`x-circle` **Invalid
formula** with the parse position, else valid), and completions in a `suggestion-container` with
a doc line per function. Add formula writes `Untitled: ""` under `formulas`.

**Scoria.** A text field and a checkbox list: the 12 `file.*` labels alphabetically, then the
formula, then every note property, no icons, no hidden styling, no gear. The panel is taller than
the window on the demo vault and does not scroll. No Add formula, no Hide all, no property page,
no formula editor.

### Filters are expression fields; Obsidian's are property, operator and value rows

*high, missing-feature, large*

**Landed 2026-11-01**: rows of property, operator and value with the observed operator lists per kind (`scoria_bases::rule`), typed values, pills, the advanced toggle both ways, Add filter, Add filter group, nested groups with their own trash, and the collapsible sections with count flairs. **Still out**: the value fields' suggestion lists.

**The suggestion lists landed 2026-11-04**: a text or list row's value field opens the
`suggestion-container mod-property-value` the run saw on it (`s05` `afterEnter`), with what a
table cell of that property lists -- the property's values across the vault, narrowed by
containment in any case, without the value typed in full -- and on a list row without the pills it
has. A click or Down/Up and Enter takes a value; on a list it becomes a pill and the field stays
open for the next. The list's *contents* on that field were not captured, only the container,
so the cell's rule stands in for them. The same pass fixed every row's value field sharing one
element id, which left a click on the second row's field dead. **Still out**: the pill row does
not wrap, so a third or fourth pill pushes the field out of the expression box (its list then
hangs past the box's right edge).

**Obsidian.** A 520 px panel of two collapsible sections, **All views** and **This view**, each
with a count flair. A group is a header select — **All the following are true** (`and`), **Any of
the following are true** (`or`), **None of the following are true** (`not`) — and rows reading
`where` / `and` + a property combobox with its type icon + an operator combobox + a value: a
contenteditable text with a value suggestion list, a number input, a date input, a checkbox, or
multi-select pills. Each row has **Advanced filter** (`code-xml`, aria-label; swaps the row to a
CodeMirror expression and itself to **Simple filter**, `mouse-pointer-click`) and **Remove
filter** (`trash-2`). A group's footer has **Add filter** and **Add filter group** (`plus`); a
nested group is a row with its own select and a `trash-2`. Operators — file: links to, in folder,
has tag, has property, does not link to, is not in folder, does not have tag, does not have
property; number: =, ≠, &lt;, ≤, &gt;, ≥, is empty, is not empty; text: is, is not, starts with, ends
with, is empty, contains, contains any of, contains all of, does not start with, does not end
with, is not empty, does not contain, does not contain any of, does not contain all of; checkbox:
is, is not. Enter in a value opens its suggestions; Escape closes only those.

**Scoria.** The sections, a conjunction pill reading `and` / `or` / `not`, an **empty rule field
drawn above the existing rules**, and per rule a verbatim statement field, an inert `code-xml`
button whose tooltip is the misspelled class name `filer-expression-mode-toggle`, and a trash.
Typing in the empty field appends a statement; editing an existing rule and pressing Escape
**commits** instead of cancelling. No Add filter, no Add filter group, no operators, no typed
values, no collapse. The expression language itself is complete (`scoria-bases`), so the work is
the row UI and the mapping from a statement to a row, observed for every operator above in
`s05-filters` and `s17-fix-filters`.

### Sort and group: no headers, no grips, no Manual order, and an empty group panel

*medium, parity-gap, medium*

**Landed 2026-11-01**: both headers, grips that reorder by drag, Add sort at the bottom appending a row that waits for its property, number directions, the group panel's empty row, *Manual* writing `groupOrder` and the group list with checkboxes and grips. **Still out**: *Hide all* in the group list, drawn faint, since an empty `groupOrder` is not representable.

**Obsidian.** Sort: header **Sort by**; each rule a `grip-vertical` handle, property combobox,
direction combobox (**A → Z** / **Z → A**, **0 → 1** / **1 → 0** on numbers), `trash-2`; **Add
sort** (`plus`) at the bottom opens the new row's property list at once. Group: header **Group
by**; one row of property, direction (**A → Z** / **Z → A** / **Manual**), trash; with Manual a
`.bases-group-list` appears — grip rows with a checkbox per value, **Add group**, **Hide all**
(`eye-off`) — and hiding writes `groupOrder`. Group headings in the table, cards and list are
`.bases-group-heading.mod-collapsible`: the property name in 12 px muted, the value bold, a
`chevron-right`; a click folds the group.

**Scoria.** Sort: a `+` at the top, no header, no grips. Group with no `groupBy` is an empty box
with a `+`; a pick closes the panel. No Manual, `groupOrder` is never written. Group headers read
`active 3` and take no click.

### Results: Show all is always drawn and rewrites the file, and there is no CSV export

*low, parity-gap, small*

**Landed 2026-11-01**: Show all faint until a limit is set and writing only then; Export CSV... through the desktop's save dialog with `scoria_bases::csv`'s dialect.

**Obsidian.** **Limit number of results**, a number input with placeholder `e.g. 10`; **Show all**
(`rotate-ccw`) only while a limit is set; **Copy to clipboard** (`copy`); **Export CSV...**
(`file-down`).

**Scoria.** Show all is drawn always, with the count, and clicking it with no limit marks the
base dirty and saves it. No Export CSV.

### The column header has no icon, no sort mark, no reorder, and half a menu

*high, missing-feature, large*

**Obsidian.** Each header is the type icon + name in 13 px muted, a `chevron-up` / `chevron-down`
when sorted, a 6 px resizer that turns accent on hover, and the header is draggable to reorder
columns (a `.table-drag-target.mod-col` line marks the drop). A left click does nothing; there is
no `+` column control. The context menu: **Hide column** (`eye-off`) · **Summarize...**
(`calculator`) · **Group by this property** (`group`) | **Sort A → Z**
(`arrow-down-wide-narrow`, `mod-checked` when active) · **Sort Z → A** (`arrow-up-narrow-wide`) ·
**Clear sort** (`eraser`, `is-warning`, only when sorted) | **Edit property...** (`edit-3`) |
**Property type ▸** (`info`: Checkbox, Date, Date &amp; time, List, Number, Text, the current one
checked). The `file name` header has no Group by and no Property type; a formula header offers
**Edit formula...** instead and no Property type. **Summaries** are a whole feature: Summarize
and every summary cell open a page (`Find or create...`, rows showing the computed value, **Add
summary** (`square-function`), **Hide summary** (`trash-2`)); the options are, for numbers and
formulas, Average, Max, Median, Min, Range, Stddev, Sum plus Empty, Filled, Unique; for dates
Earliest, Latest, Range; for checkboxes Checked, Unchecked; for text, lists and files Empty,
Filled, Unique. A `.bases-table-footer` row holds the table's summaries and a
`.bases-table-group-summary-row` sits under each group heading, with a `Summary` placeholder in
`#666` and a `chevrons-up-down` expand button.

**Landed 2026-11-01**: the type icon, the sort chevron, the 6 px accent resizer, drag-to-reorder with the drop line (writing `order`), and the whole menu per kind of column — Group by this property, Edit property... / Edit formula... opening the properties panel's page under the header, the Property type submenu writing `types.json`, Clear sort in the warning colour — with Escape closing it. Summaries are built: the footer and the row under each group heading, the Summarize page with every observed option computed (`scoria_bases::summary`), Hide summary. **Still out**: *Add summary*, drawn inert, since what it creates was never observed. See [[Development#The column header, summaries, cells and the table keyboard, 2026-11-01]].

**Add summary landed 2026-11-04** after the probe (`bases-leftovers.mjs` `s03`): the row opens an
**Edit new summary** page in the same menu -- *Summary name* (value and placeholder `new summary`,
focused), *Formula* (placeholder `values.reduce( ... )`) and the validity line -- and the click
writes nothing. Once the name is set and the formula parses, the base gets a top-level
`summaries: {name: formula}` and the column's summary becomes that name; a later rename moves the
key in place. Escape goes back to the Summarize list, where the summary is listed by name among
the built-ins. The same pass lets a panel hung from a summary row near the window's foot slide up
to fit, as Obsidian's did, instead of being cut off below. **Still out**: the page header's
`more-vertical` menu (its rows were not opened), and whether Obsidian writes a half-typed
formula -- Scoria writes each version that parses. The icon a custom summary's row wears is
unsettled: the 2026-11-01 run drew `square-function` for `customMean` in most captures and
`binary` in some, the 2026-11-04 run `binary` for `Doubled`.

**Scoria.** Name only, no icon, no sort mark, no reorder. The menu is Hide column, Summarize...
(inert), Sort A → Z, Sort Z → A, Clear sort, Edit property... (inert), with no icons. No Group by
this property, no Property type, no summaries at all (`summaries` only round-trips).

### Cells edit through one text field; Obsidian has an editor per type, a file menu and a keyboard

*high, parity-gap, large*

**Obsidian.** The name cell is an `internal-link` (click opens the note in the leaf; right-click
raises the file menu, with **Copy path ▸** as Obsidian URL / from vault folder / from system
root). Every other property cell clicks straight into an editor with a 2 px accent ring: text is a
contenteditable with a value suggestion list, number an `input[type=number]`, date an
`input[type=date]` with a hover-only **Open daily note** (`link`) icon, checkbox an input that
writes on click, list and tags a pill container with suggestions, a link shows `[[Basalt]]` raw.
Enter commits and leaves; Escape reverts; double-click is a click; formula cells are read-only
and Enter on one opens Edit formula. With nothing editing the table holds an invisible cursor:
Tab, Shift+Tab and the arrows move it, Enter opens the cell, typing starts editing, Delete opens
the editor without clearing. The docs add shift-click cell selection with Copy, Paste, Undo and
Redo, which the probe did not drive.

**Landed 2026-11-01**: the name cell is a link with the file menu on a right click; a frontmatter cell opens its editor in the 2 px accent ring with the value selected, a text or list cell with the property's values suggested under it (Up, Down, Enter, click; Escape closes the list, then the cell), a list cell drawn as pills, a date cell with *Open daily note* on hover, a checkbox written on click; `file.*` and formula cells take no click. The keyboard: an invisible cursor moved by Tab, Shift+Tab and the arrows, Enter and Delete opening the cell (a formula's Edit formula page), typing opening it with the character. **Still out**: shift-click cell selection with Copy, Paste, Undo and Redo, which the probe never drove; a list cell edits as its comma-joined text rather than as pills with an input.

**Scoria.** One shared `TextInput` for text, number, date and list alike (a list is edited as its
joined text); checkbox clicks write; Enter commits, Escape cancels. No row or cell menu, no
suggestions, no date picker, no pills, no keyboard at all: Tab, the arrows, Enter and Escape are
inert with nothing editing.

### Kanban cards and columns have no drag and no menus

*medium, missing-feature, large*

**Obsidian.** A column is 280 px, `#232323`, radius 8; its header is draggable (grab cursor) with
the value, the count and a `plus` **Create new note**; right-click on a header or a card gives
**Add item** (`plus`) | **Hide column** (`eye-off`) · **Edit groups...** (`stretch-horizontal`,
opens the Group panel). A card is draggable with `is-being-dragged` and a drop placeholder in the
target column; the drop writes the grouped property to the note (`status: done`). A column drop
reorders in memory until the order is Manual; Hide column writes `groupOrder` and sets Manual.
Cards hover with a `#3f3f3f` shadow, any click opens the note, right-click is the file menu.

**Landed 2026-11-01**: a card drags with a card-sized placeholder in the target column and the drop writes the grouped property to its note; a column header wears the grab cursor and drags, reordering the board in memory and writing `groupOrder` once the order is Manual; the header carries the count and *Create new note* on hover, the footer *New*, both prefilling the column's value; a right click on a header or a card is *Add item* | *Hide column* · *Edit groups...*, with Hide column writing `groupOrder` without the column and Edit groups opening the Group panel. **Still out**: where a dropped card lands inside its column (the sort re-resolves it), and the `obsidian://` drag-out. See [[Development#The kanban board, 2026-11-02]].

**Both settled 2026-11-04 by the probe** (`tools/obsidian-probe/bases-leftovers.mjs`, `s05`,
Obsidian 1.14.5): Obsidian ignores the drop position too. A card dropped mid-column, at the top
or below the last card lands in its name-sorted place, the 88 px placeholder already sits at that
slot during the drag, only the note's grouped property is written, and the `.base` never gains an
order key; a drop within its own column draws no placeholder and changes nothing. So the
re-resolving sort *is* the behaviour. The drag carries no `obsidian://` URL: its only type is
`text/plain`, the note's basename (`Tuff`), after a `setDragImage`. A board with an explicit sort
or a Manual group order was not tried.

**Scoria.** 300 px columns, `active 3` headers that take nothing, cards that highlight on hover
and open on click. No drag (`kanban.rs:35-40` leaves it out on purpose), no column plus, no
menus.

### Cards and list rows have no file menu, and a list line opens on any click

*low, parity-gap, small*

**Obsidian.** A card (`#1C1C1C`, radius 8, hover shadow `#3f3f3f`) opens on click, raises the
file menu on right-click, and drags out as an `obsidian://` URL. A list item is a `.list-bullet`,
the link, and `.bases-list-separator`; only the link opens, and right-click on the link is the
file menu.

**Landed 2026-11-01**: a card's and a list link's right click raise the file menu; only the link of a list line opens the note. **Still out**: the empty grey box a cover-less card draws, and a card's drag-out as an `obsidian://` URL.

**The grey box is right, settled 2026-11-04 by the probe** (`bases-leftovers.mjs` `s01`): a card
whose note has no `cover`, or an empty one, still gets its `.bases-cards-cover` -- the full
cover height, `background-color` rgb(35, 35, 35), no image, no children -- and every card keeps
the same height, so the entry's "Obsidian draws no cover at all" was never observed and is
wrong. The drag-out is not an `obsidian://` URL either: see the kanban entry above.

**Scoria.** Cards and list lines open on a click anywhere and have no menu; a card with no cover
draws an empty grey box where Obsidian draws no cover at all.

### The embed draws a titled, bordered first view; Obsidian embeds the whole toolbar

*high, parity-gap, medium*

**Landed 2026-11-01**: an embed is the live view itself — the toolbar with its counts and badges, every panel, the layout the view names (cards, list and kanban included), cell editing and New — in a box with no title, border or padding, as tall as its content and scrolled by the note; `#View` picks the view and a name no view has leaves the first showing; Live Preview draws **Edit this block** top-right, which puts the caret on the embed's line; an edit from an embed writes the `.base` through the same save as a leaf's. See [[Development#The embedded base and the base leaf's chrome, 2026-11-01]].

**Obsidian.** `![[x.base]]` is `.internal-embed.bases-embed.interactive-child` with the full
toolbar and live counts, no title, no border, no padding; `#View` picks the view. Every panel,
cell editor and New works in place and writes the file. Live Preview adds **Edit this block**
(`code-2`) top-right, which places the caret on the line; right-click there is the editor menu.

**Scoria.** A bordered, rounded box titled `↗ All rocks`, the first view only, no toolbar, 50
rows then `n more rows`, the subpath ignored; a cards, list or kanban view is drawn as table rows.

### The base leaf's chrome is a note's, and the new base is wrong on both ends

*medium, parity-gap, small*

**Landed 2026-11-01**: the leaf's icon is `lucide-table` (observed on the tab header, `s15` `tabMenu.tab`); the base leaf's header draws **More options** alone — Bookmark is zero-rect there as on a note — with the observed rows less the three Scoria cannot run (Open in new window, Open version history, Open linked view; Open in new window is drawn since 2026-11-09, the popouts); every ribbon button carries its observed tooltip; a new base is one `file name` column over every file of the vault. The doubled separator before Rename on the explorer's `.base` row is Obsidian's own (`s01` `baseFileMenu` has the pair), so it stays.

**Obsidian.** The tab icon is `table` and the title the stem; the view header is the title with
no icon, back and forward, **Bookmark** and **More options** (`more-vertical`): Split right ·
Split down · Open in new window | Rename... · Move file to... · Bookmark... | Copy path ▸ | Open
version history · Open linked view ▸ (Open backlinks) | Open in default app · Show in system
explorer · Reveal file in navigation | Delete file. The ribbon's **Create new base**
(`layout-list`) has that tooltip. A new base is `Untitled.base`, 0 bytes, one view named `View`,
one column `file name`, over every file in the vault (`15 results`).

**Scoria.** The leaf icon is `layout-list`, and the view header still draws the graph button and
the Reading/Edit mode switch over a base (`workspace.rs:2253`, `7975`), which the mode ignores.
The ribbon button has no tooltip. A new base opens with every note property as implied columns
and no `file name` column. The explorer's file menu on a `.base` row has a doubled separator
before Rename.

### Status of the commands

`bases:new-file`, `bases:insert` and `bases:copy-table` match. `bases:add-view` named the view
`View N` until 2026-11-01, and names it `View` now. `bases:add-item` opened a plain new tab
without the frontmatter prefill until 2026-11-01; the toolbar's *New* — in a leaf and in an
embed — now creates the note with the prefill and opens it. `bases:change-view` is a chooser,
unverified against Obsidian's. None has a default hotkey in either app.

## What the campaign fixed

One line each, with the detail in [[Development#The parity campaign, 2026-10-20]] and the full
write-ups in the wave reports. Everything here was re-driven against the rebuilt binary by one of
the two verification fleets or by the triage of the second audit pass, except where a line says
otherwise.

**Files view and file operations.** A file deleted outside the app is no longer recreated by the
next autosave — the only `fs::write` is now behind a verified-unchanged read, and the open note
detaches with its buffer kept and marked. Typing into a deleted note's buffer no longer vanishes
silently; the editor is gone and the main area paints a placeholder. The explorer collates
naturally, so `1, 2, 10, a, B, Note 1, Note 2, Note 10` is exactly Obsidian's order. The rename
prompt preselects the name it is pre-filled with, and Ctrl+A in it selects all.

**Live Preview.** A click in the left margin or past the end of a line no longer toggles a checkbox
or follows a link, and the caret lands after the construct rather than inside it. Callouts render as
callout widgets and their chevron folds. `[[Note#Heading]]` reads `Note > Heading`. A closed ATX
heading drops its trailing hashes, and a setext underline is hidden at body size — the second of
those needed a new parser span, because the display layer provably could not tell the title row from
the underline. A padded task renders one checkbox instead of a bullet and a checkbox. The empty
callout title no longer leaks a heap string on every shape (unverified live; a leak needs RSS over
many frames). List-typed properties are pills, one editable item each, with a per-pill remove, and
an explicitly empty `key: []` row takes a first item instead of swallowing the click.

**Reading view.** `propertiesInDocument: hidden` is honoured — the block occupies zero height, where
it used to occupy 130 px. Task checkboxes toggle, byte-exactly and in both directions, writing
through the editor's buffer by line and column so a BOM or CRLF file cannot be corrupted. A note
deleted on disk no longer stays rendered, and an unsupported file type no longer leaves the previous
note live underneath the placeholder. The code block's permanent language label is gone and a hover
copy button took its place — see [[#Blocked by the harness, not by the code]]. A completed task is
struck through as well as muted.

**The five large builds, 2026-10-20.** Written up in
[[Development#Tabs, the layout file, the ribbon, the Hotkeys tab and the graph panel, 2026-10-20]];
each was driven over the socket on a scratch copy of the docs vault before it was committed. The
main area has tabs: a strip above the view header, `workspace:new-tab`, `close`, `close-others`,
`next-tab`, `previous-tab`, `goto-tab-1..8`, `goto-last-tab` and `undo-close-pane` runnable with
their registry hotkeys, Ctrl+click on an explorer row, a reading-view link or an editor link opening
a new tab, `editor:open-link-in-new-leaf` doing the same from the cursor, "Create new note" going to
a new tab unless the active one is empty, and "Default view for new tabs" honoured by every new tab,
which closes that entry on the behaviour rather than the tag. `workspace.json` is read at launch —
the leaves under `main` become tabs in their recorded modes, the `active` id picks the tab, each
sidebar's `collapsed` and `currentTab` are honoured, and with a layout read the startup flags no
longer pick a first note — and written on quit only when the session changed it, keeping every key
Obsidian gave a restored leaf; a main area with more than one tab group is left alone rather than
flattened on disk. A ribbon runs down the left edge with the switcher, graph, daily note, template
and palette, honouring `left-ribbon.hiddenItems`, and each sidebar has a toggle that
outlives its collapse. The left sidebar ends in Obsidian 1.14's vault profile (2026-10-21): the
vault's name behind a `chevrons-up-down` switcher, Help and the Settings gear, which is where the
gear lives now rather than at the foot of the ribbon; the switcher drops up a menu of every vault
this machine has opened (kept in `scoria.json` outside the vaults, Obsidian's `obsidian.json` shape)
with "Open vault..." and "Open sandbox vault" under a hairline, switching puts the open vault away as
quitting would before the next restores its own layout, and Help opens a window of Obsidian's shape
(mark, name, version, four rows with buttons) rather than the browser. The Hotkeys tab binds a chord from a key capture, removes a chip, restores a
default and marks duplicates; `hotkeys.json` is written whole in Obsidian's `{modifiers, key}`
shape, and the registry and every key binding are rebuilt live. The graph has Obsidian's four-section
controls panel with timelapse, restore and close, writing `graph.json` a third of a second after a
change — the twenty-key document when the vault has none, the changed keys merged in when it does —
and draws arrowheads when `showArrow` is on. Along the way: the window title names the note and the
vault, and opening a note from the graph shows the note.

**Right dock panes and graph.** The layout simulation no longer dies when a rebuild lands while it
is running. Deleting the open note clears the Outline, Backlinks and Outgoing rows and re-centres
the graph. Links written in properties now reach both panes, with the property name as the row's
context. The `scale` saved in `graph.json` survives the auto-fit, and Ctrl+0 now outlives the next
vault change. The Orphans filter drops unlinked attachments too. The Outline rebuilds from the
buffer as you type, so its offsets match what is on screen.

**Search, switcher, palette and autocompletion.** Ctrl+Shift+F with the dock hidden or on another
tab no longer kills the keyboard. A workspace hotkey pressed over a modal no longer runs behind it.
A bare term matches the file name, highlighted in the result title and counted as a result. A
half-typed query — `(`, `"`, `/`, `file:`, `((()))` — no longer lists the whole vault. Collapsed
result rows survive a vault change. Ctrl+N and Ctrl+P no longer steal the link suggester's keys.
Results collate numerically and locale-aware. Every text input gained selection, word motion,
double- and triple-click, drag-select and the clipboard bindings. **And the app no longer dies**
when a search term sits behind a multibyte whitespace — see the wave-8 paragraph below.

**Settings.** Clicks, drags and the wheel no longer pass through the modal into the note behind it,
and global hotkeys no longer fire through it. Enter or Escape in a text field no longer strands
focus on an unrendered input. A field commits on blur, a tab switch, a backdrop click and the × as
well as on Enter — and a toggle clicked while a field is open no longer loses one of the two changes
to a stale snapshot. An open dropdown is dismissed by a click elsewhere and does not survive
reopening the modal. The two auto-pair rows no longer claim Scoria ignores them.

**Source editor.** Down no longer sticks at the first wrap boundary, and Home and End no longer sit
one visual row out. Clicks in the find panel no longer fall through to the note, and the Replace
button replaces. Ctrl+H opens on the query field, Tab moves between query and replacement, and both
fields select their contents on open. `editor:insert-table` writes a block of its own. Auto-pairing
implements exactly the probed rows, wrapping a selection instead of destroying it. Twenty-seven
command ids landed — the seven formatting toggles, the checkbox toggle, heading 0..6,
bullet/numbered/blockquote, indent/unindent, delete paragraph, move line, insert link, wikilink and
embed, follow link — each driven over the 1000-note corpus and the real vault by the Writer gate; of
100 `editor:*` ids, 50 are runnable where 23 were. Shift+Tab outdents by `tabSize`. Status-bar
notices expire. **And Backspace and the arrow keys no longer split a grapheme cluster** — see below.

**Workspace shell.** The two keyboard-death traps are gone, and so is the one the socket could reach
(`focus` now refuses a view the main area is not painting, naming what is painted). History no
longer strands `current` on a missing file; a deleted note is pruned from it outright. A deleted
note is not recreated by the next keystroke. A detached buffer says so, reattaches when the file
comes back, and says out loud when its edits are discarded. Opening a canvas, an image or an
unreadable file no longer leaves the previous note's buffer under the new name. Notices are a
bottom-right toast stack that expires by id. **Both** routes into the typeable empty state are now
closed: deleting the open note was fixed in wave 1, and launching on a vault with nothing to open —
an empty vault, or one holding only a canvas — in wave 8.

### Wave 8: a crash and a corruption

Three things landed after the triage of the second audit pass, all verified live against the
rebuilt binary. They are listed apart because the first was filed by neither audit, and the other
two only by the pass the campaign never read.

- **The search panic.** A search term more than 40 bytes into a line, with a multibyte whitespace
  (U+00A0, U+2009, U+3000 — ordinary in pasted text and in CJK notes) as the first whitespace before
  it, sliced the snippet at a non-character boundary and panicked. The panic was inside the
  background `run_search` task and took the main thread with it (`Task polled after completion`), so
  the window vanished and any unsaved buffer went with it. The windowing moved out of the gpui crate
  into a pure `scoria-search::snippet` with `floor`/`ceil` helpers that snap every window edge — and
  every hit range — to a character boundary, with the live repro written as a unit test before the
  fix. A sibling of the same class in `fuzzy.rs`, where `simple_search` mapped lowercased byte
  offsets back by counting characters, now carries a per-byte source map instead.
- **Grapheme-aware editing.** Backspace and the arrow keys stepped by `char`, so one Backspace left
  a bare `e` where `é` had been, split 🇩🇪 into a lone regional indicator, or left a dangling
  zero-width joiner at the end of a family emoji — and the autosave wrote the damage to the vault.
  `scoria-text` now steps by grapheme cluster for Backspace, forward Delete, Left/Right, the
  shifted variants and word motion, and the auto-pair Backspace rule only treats a single-character
  cluster as half of a pair. Vertical motion and mouse clicks still land by pixel, so a click can
  still put the caret mid-cluster; that is in `editor.rs` and is the handoff.
- **The empty state on launch**, above.

The final gate: 501 tests pass, clippy is clean across the workspace, and the real-vault Writer gate
still round-trips 14/14 byte-identical.

## Was declined until Obsidian was probed — all of it now is

These were not deferred for effort. Each one would write bytes into a user's vault, or register a
command, on a rule nobody had observed in a real Obsidian — which the clean-room rule forbids. They
were a to-do list for `tools/obsidian-probe/`, and on **2026-10-25 every line of it was run**
against Obsidian 1.14.5 over CDP, in five scripts: `editing-commands.mjs`, `editing-typing.mjs`,
`footnote-insert.mjs`, `ui-rules.mjs`, `attachment-dropdown.mjs` and `search-collation.mjs`.

So nothing below is waiting on an observation any more. What each entry now holds is the answer and
the work it unblocks; the observations themselves are in
[[Specification#Observed editing command behavior, Obsidian 1.14.5 (installer build 1.13.7)]],
[[Specification#Observed UI rules, Obsidian 1.14.5 (installer build 1.13.7)]] and the two new
blocks of [[Specification#Observed auto-pair behavior, Obsidian 1.14.4]], with the raw runs under
`tools/obsidian-probe/results/2026-10-25-*.json`.

Four answers refuted what this note had assumed, so read those first: Mod+L **does** insert a
checkbox on a line that has none; `editor:set-heading` does nothing at all; the inline formatting
toggles wrap **each line** of a multi-line selection; and Shift+Tab subtracts one indent unit
rather than dedenting to the previous multiple.

### Settled by the probes of 2026-10-25 — the editing commands (issue #7)

**Landed 2026-10-26**: every row below is implemented — the six insert-block commands, the
checklist cycle, Mod+L on a line without a box, the six highlight colours (parser, reader and
command), `editor:clear-formatting`, the no-op `editor:set-heading`, per-line inline toggles,
the one auto-pair rule with provenance-tracked skip-over, Shift+Tab's column arithmetic and the
four-space unit under `useTab: false`. See
[[Development#The settled editing commands, 2026-10-26]]; the rows are kept as the record of what
was observed.

`tools/obsidian-probe/editing-commands.mjs`, `editing-typing.mjs` and
`footnote-insert.mjs` answered every editing row below against Obsidian 1.14.4
(installer build 1.13.7). The observations are in
[[Specification#Observed editing command behavior, Obsidian 1.14.5 (installer build 1.13.7)]]
and the two new blocks of
[[Specification#Observed auto-pair behavior, Obsidian 1.14.4]]; the raw runs are
`tools/obsidian-probe/results/2026-10-25-editing-commands.json` and
`-editing-typing.json`. What each one leaves to do:

- **`editor:set-heading` is a no-op in Obsidian** — a palette entry whose
  check passes and which changes nothing, on every line shape, in both modes,
  through the palette, a bound hotkey and its own callback. There is no cycle
  to implement. Scoria does not register the id at all today; registering it
  as a runnable command that changes nothing is what parity asks for, and it
  is worth a comment saying so, because it looks like a bug.
- **`editor:cycle-list-checklist` is bullet → `- [ ]` → `- [x]` → bullet**, a
  non-list line becoming a bullet first, the marker (`*`, `+`, `1.`) kept, and
  a custom status discarded on the first press. So it is *not* the path to
  `[?]`, `[-]` and `[>]` — nothing in Obsidian writes those. New work: the
  command is unimplemented in Scoria (`RUNNABLE` in `workspace.rs` has no
  entry for it). The multi-line rule is in the spec, with the two selections
  where Obsidian's own output oscillates instead of converging; Scoria should
  implement the converging rule and not copy that.
- **Mod+L on a line with no checkbox inserts `- [ ] `** — after any list
  marker, after a blockquote's `> ` and after leading indentation, on an empty
  line too. This is the one row the backlog had backwards: both triage passes
  called Scoria's "leave every other line alone" deliberate, and it is a
  parity gap. `format::checklist_plan` skips those lines today (its own doc
  comment says the case is unprobed) and needs to add the prefix; the
  selection rule — flip the status when every line has a box, invert presence
  when only some do — is in the spec.
- **The insert-block commands** all have their bytes recorded: callout with
  `> [!NOTE] Title` / `> Contents` and `NOTE` left selected, codeblock and
  mathblock wrapping the caret's line, the rule splitting a line only for
  `insert-horizontal-rule`, `insert-tag` writing a bare `#` with no suggester,
  and `insert-footnote` numbering by the highest existing *numeric* label plus
  one with the definition always at the end of the document. None of the six
  is implemented in Scoria yet; all six are now unblocked.
- **The six highlight colours write `==🔴word==`** — one colour emoji directly
  after the opening `==`, which the reader consumes as `data-highlight="red"`
  and does not show. The emoji per command, and the replace/remove rules, are
  in the spec. Scoria has `editor:toggle-highlight` (plain `==`) and none of
  the six colours; the parser also has to learn to swallow a leading colour
  emoji inside a highlight, or `docs/scoria` notes written in Obsidian will
  render the emoji.
- **`editor:clear-formatting`** strips emphasis, highlight, strikethrough,
  inline code, inline math and a line's `#`/`>`/`-`, leaves wikilinks alone,
  and does nothing without a selection. Unimplemented in Scoria; unblocked.
- **Move line up/down are unbound in Obsidian.** The default hotkey table
  holds 41 ids and neither is among them; the Hotkeys tab shows both rows as
  `Blank`, and Alt+Up/Alt+Down in the editor do nothing. Scoria's current
  reading was right — no change, and the question is closed.
- **The inline formatting toggles wrap *each line* of a multi-line
  selection.** `one`/`two`/`three` selected whole gives `` `one` ``/`` `two` ``/
  `` `three` ``, and the same per-line shape for bold, italics,
  strikethrough, highlight and inline math. `editor:toggle-comments` is the
  exception: one `%%` pair around the whole selection. So
  `Buffer::toggle_wrap` is right for comments and wrong for the other six —
  it wraps the whole selection in one pair. The mixed-selection rule for the
  list and quote toggles ("remove only when every line already has the
  prefix"), numbering an ordered toggle from 1 rather than continuing the list
  above, and skipping blank lines for the bullet and numbered toggles but not
  the quote toggle, are all confirmed as Scoria has them.
- **Auto-pairing's open rows** reduce to one rule. A bracket closes when the
  character *ahead* is whitespace, end of line or a closing bracket; a
  markdown or quote character needs that **and** whitespace or start of line
  behind. `_` behaves like `*`; `~ = $ % <` never close. No context suppresses
  pairing — blockquote, fence body, `$$`, heading, list and task item all pair
  like a plain line — and the inline-code case is the ahead-character rule,
  not a context rule. Skip-over is provenance-tracked for *every* closer and
  survives moving within the line but not leaving it; forward Delete between a
  pair removes only the closer; `]` inside `[[|]]` skips; an abandoned closer
  is never removed. Scoria left all of these inert, so each is now a small
  piece of work with an observed rule behind it.
- **Shift+Tab removes exactly one indent unit, clamped at zero** — not
  "dedent to the previous multiple", which is what this entry assumed. A tab
  counts `tabSize` columns and the result is rewritten as tabs plus spaces;
  with `useTab: false` the unit is four spaces whatever `tabSize` says.
  Scoria's `min(tabSize, leading spaces)` is already the right arithmetic for
  a space-indented line; what it has to add is the tab accounting and the
  `useTab: false` unit.
- **`editor:insert-table` on a non-blank line** keeps the line, puts a blank
  line after it and the table below, and does **not** split the line when the
  caret is mid-line. Scoria's "blank line before, exactly one after" is
  right; the mid-line split is the only correction.

### Settled by the probes of 2026-10-25 — the UI rules (issues #8, #19, #41)

**Landed 2026-10-28.** All of it, and the entry was already stale when it was
read: eleven of the fifteen items below had been built by the work of the week
after it was written. Verifying that first is what made the pass worth running
— it turned up three bugs nobody had filed, and six landed items with no test
pinning them. See [[Development#The settled UI rules, 2026-10-28]].

Built this pass: the four-option attachment dropdown and its dependent rows
(#41), the three-option new-notes dropdown, `scoria_vault::ops::attachment_path`
for the resolution rule, the click-dismiss and Escape popover rules (#41's
second half), the empty tab's four pills and its `0 properties / 0 words /
0 characters` status bar, `New tab` in the view header and the window title,
the view-header inline rename, the two-press Backspace on the property pills,
Enter leaving the pill input open, the `lucide-x` remove control, and the
callout colours.

Already built, verified item by item against the running tree rather than
taken on trust: the 28 callout icons, the ephemeral fold chevron and the absent
affordance without `-` or `+`, the setext underline at body size, the
struck-through and muted done task, clearing every property, Ctrl+E swallowed
in a text input, `graph:open-local` not offered with no file, the local graph's
own scale 1, the collation tie, and three of the four pill gestures.

Three bugs the stale text had hidden, all fixed: `foo OR` matched **every note
in the vault** (an `All` node with no children was vacuously true in the
matcher, which also meant an empty-query graph colour group coloured the whole
graph); a dangling `-` searched for a literal hyphen; `foo (` was flattened
back to `foo`. And `Theme::callout_color` took `&self` but never read
`self.dark`, so every callout but `quote` drew the light theme's colour on the
dark theme Obsidian ships.

Not built, each for a reason given in the Development entry: the reading
view's pills still have no remove control, the explorer's context menus stay a
subset, the empty tab's header has no Bookmark or More options, and
`.inline-title` is untouched. **As with every entry here, read the Obsidian
half below as the record of what was observed; the "Scoria" halves describe a
binary that no longer exists.**

`tools/obsidian-probe/ui-rules.mjs`, `attachment-dropdown.mjs` and
`search-collation.mjs` answered every UI row below against **Obsidian 1.14.5**
(the probe's own window title; the app had updated itself since the earlier
passes). The observations are in
[[Specification#Observed UI rules, Obsidian 1.14.5 (installer build 1.13.7)]]
and the raw run is `tools/obsidian-probe/results/2026-10-25-ui-rules.json`.

- **The property pills.** Enter in the trailing input adds an item. Backspace
  takes **two** presses: the first moves focus to the last pill, the second
  removes it — so "Backspace removes the previous pill" was half right, and
  the reviewer was right to call it an assumption. The remove × is **always
  drawn**, not revealed on hover (`display: flex`, `opacity: 1`, 14 px, with
  the mouse parked away). Removing the last item leaves **`tags:` with an
  empty value** — not `tags: []`, and the key is not deleted.
- **Clearing every property** turns `---\nalpha: one\nbeta: two\n---\n\nbody\n`
  into `\nbody\n`, first byte `0x0A`, from reading view and Live Preview alike
  — which is exactly what Scoria writes, so that entry is closed with no work.
  A note that is only frontmatter becomes a **zero-byte file**; worth a test.
- **The callout fold chevron is ephemeral.** Every click, with a save after
  each, left the note byte-identical: collapsing `[!tip]+` does not rewrite it
  to `-`. Scoria's ephemeral fold is parity. New detail to match: a callout
  with **no** `-` or `+` has no fold affordance at all, and the chevron is
  `lucide-chevron-down` with `.callout-fold.is-collapsed` for the folded state.
- **The setext underline row stays, at body size.** 16 px / weight 400 / 24 px
  tall on its own line, classed `HyperMD-header-line HyperMD-header-line-<n>`,
  while the heading line above it is the same size the reading view gives
  `h1`/`h2`. Of the three guesses this entry listed, "demote to body size" is
  right — and nothing is hidden, the `=====` characters stay visible.
- **Callout type icons** (issue #19): all 28 built-in types, their lucide icon
  and their `--callout-color`, are now in the spec table — `note` is
  `lucide-pencil`, `tip`/`hint`/`important` `lucide-flame`, and an unknown type
  falls back to note's pair. The corpus's empty `<svg>` was a serialisation
  artefact; the DOM carries the name in the svg's own class. *Superseded
  2026-10-28: the Rust half was already done — `Assets` is registered and
  `assets::callout_icon` carries the whole table, cut from lucide's own source.
  What was still wrong was the colour, not the icon: `Theme::callout_color`
  answered the light theme's values in both themes.*
- **The done-task decoration** is `text-decoration-line: line-through` plus a
  muted `rgb(179, 179, 179)` against an unchecked item's `rgb(218, 218, 218)`,
  in the reading view and on the `.cm-line` in Live Preview. So Scoria's
  strikethrough is observed behaviour after all; the muted colour is new work.
- **The empty tab** is titled **"New tab"**, not "No file is open" — the
  paraphrase this entry rested on was wrong. Four pills: `New note Ctrl + N`,
  `New base`, `New canvas`, `Go to file Ctrl + O`. No mode switch in the
  header, but the word count is **not** hidden: the status bar reads
  `0 properties`, `0 words`, `0 characters`. Wave 8 was right to leave the UI
  alone; building it is now a specified job.
- **The view-header title** is `contentEditable="true"` at rest. A single
  click puts the caret in it at the clicked offset, typing edits in place, and
  **Enter renames the file**; Escape abandons. So the three lines of work this
  entry predicted are real, and the gesture is a single click, not a double.
- **The attachment-location dropdown** (issue #41): four options, each storing
  a different `attachmentFolderPath` — `Vault folder` → `/`, `Same folder as
  current file` → `./`, `In subfolder under current folder` → `./Attachments`
  (revealing a **Subfolder name** row), `In the folder specified below` →
  `Attachments` (revealing an **Attachment folder path** row). The resolution
  rule is in the spec: a leading `./` is relative to the note's folder,
  anything else is vault-absolute, and `/` and `""` both mean the root. The
  row can lose its `.not_yet()` tag. The whole Files-and-links tab's rows are
  captured too, which is more than #41 needed and feeds the Settings entries.
  #41's second half never needed a probe: a click that dismisses a dropdown
  must not also activate what it lands on, and Escape with a dropdown open
  must close the popover rather than the modal.
- **`graph:open-local` with no file open is not offered at all.**
  `checkCallback(true)` returns undefined and `app.commands.listCommands()`
  omits the id entirely; running it anyway opens nothing. So Scoria showing
  the global graph is a parity gap — the command should report that it cannot
  run, which the "ask each command whether it can run" machinery already
  supports.
- **The local graph's camera** is confirmed: with `graph.json` at `scale: 8`
  the global graph opened at 7.926 and the local graph at **1**, with its own
  option block. Scoria keeping the global graph's pinned zoom for a local
  graph is a real finding.
- **A half-typed query is not handled uniformly.** `foo OR`, `foo -`, `foo )`,
  `(foo`, `"foo` and `foo "` all match exactly as `foo` does (2 results);
  **`foo AND` and `foo (` match nothing**, and so do a bare `OR` and a bare
  `(`. So Scoria gating only a wholly empty query is right for the first group
  and wrong for the second, and the fix is narrower than this entry feared:
  two shapes to fail, not "an empty term inside the matcher". *Landed
  2026-10-28, and narrower was not the whole story: `foo OR` was matching every
  note in the vault, because an `All` node with no children was vacuously true.
  Fixed in the matcher, where every other consumer of a `Search` inherited it
  too. All ten shapes are a fixture now. Whether `AND` is an operator rather
  than a word is still unprobed — the probe vault's `foo` notes hold no `and`,
  so the observed zero does not tell the two apart — and only the trailing
  shape is implemented.*
- **The search collation tie is explained.** The sort is a *stable* sort on
  the basename over a file list already in **full-path descending** order:
  eight `apple` notes come back `zzz/`, `z/`, `m/n/`, `m/`, root, `a/b/c/`,
  `a/`, `APPLE`, which is the path descending and is exactly
  `app.vault.getMarkdownFiles()`'s own order; Z to A reverses only the
  basename groups. So the tie-break to implement is full path **descending**,
  and the fixture no longer has to leave the pair out.
- **Ctrl+E inside a text input does nothing.** Not End, not a mode toggle: in
  the search input and in the quick switcher's prompt the value, the caret and
  the focus were all untouched and the note's mode did not change, while the
  same chord in the editor toggles reading view. Scoria's Ctrl+E-is-End in a
  field is a divergence to remove.
- **The explorer's context menus and the modal backdrop** are both captured:
  every item of the file-row and folder-row menus with its lucide icon and
  `data-section`, the four-item menu on empty space, and the backdrop's
  `rgba(10, 10, 10, 0.4)` at `opacity: 0.85` with the modal at 560 px (prompt
  700 px), `max-width` 80% of the window, radius 12 px and a three-layer
  shadow. The delete confirmation's title, text and two button classes are
  there too.

One group is declined for a different reason and needs no probe: `editor:undo`, `redo`, `copy`,
`cut`, `paste`, `go-start`, `go-end` and the `move-caret-*` ids are absent from `names.txt`, i.e.
Obsidian does not register them on desktop. Both triage passes checked and agreed. Adding them to
RUNNABLE would create palette rows Obsidian has no command for, shown as raw ids, and would desync
the Hotkeys tab — which is also why fifteen ids still report a name equal to their id. The same
held for `open-link-in-new-leaf`, `-split` and `-window` until tabs existed; `-leaf` is runnable now
that they do, while `-split` and `-window` still wait on splits and windows (`-split` landed with
the tab groups, `-window` with the popout windows on 2026-11-09).

## The Appearance tab, against the probe of 2026-11-02

Filed from `tools/obsidian-probe/appearance-tab.mjs`
([[Specification#Observed Appearance tab, Obsidian 1.14.5 (installer build 1.13.7)]]). The
Themes picker, the "Current community themes" row, the accent swatch and the CSS snippets page
were built the same day — [[Development#The Appearance tab's pickers, 2026-11-02]] — and what
follows is the rest of the tab.

### The three font rows are text fields; Obsidian's are pages of installed families

*medium, parity-gap, medium*

**Landed 2026-11-04**: each row opens its page (`Page::Font`, `render_font_page`): the line saying
what the list does, the applied families in their own faces with Delete and a drag handle and the
"not detected" warning, then `Enter font name...` over the families gpui reports, each drawn in
itself, with the typed name offered first. A pick appends, Delete removes, a drag reorders, and the
field is written as Obsidian writes it — `Segoe UI,Arial,Courier New`, `""` when emptied. The row
reads nothing, the family, or "*N* fonts"; and the theme now applies the first family of the list
the machine has, not merely the first. See
[[Development#The font pages, quick font size and the app icon, 2026-11-04]]. Left: the applied
rows are not in a card of their own, and the drag shows no insertion line, only the row under it.

**Obsidian.** Interface font, Text font and Monospace font are
`.setting-item.mod-navigable.tappable`, and each opens a `.setting-page` with the titlebar, a
`.setting-page-desc` ("No custom font is applied right now. Add one below."), an empty
`.setting-group.mod-list` holding the families applied so far, then a second group with an
`Enter font name...` search box and one `.setting-item.mod-action.tappable` per installed family,
each row drawn in the family it names. Picking one writes `interfaceFontFamily` and moves it into
the first group, so the setting is a *list* of families built by picking, not a string typed once.

**Scoria.** A text field per row holding whatever `appearance.json` says, with "Default" when it
is empty. The value is honoured (the loader reads all three), but nothing enumerates the
machine's families and a family is typed by hand.

**Fix.** The page machinery is already there — `Page`, `render_snippets_page` and the back
chevron landed with the CSS snippets page. What is missing is the font list: gpui's font database
can enumerate families, and `theme::first_font` already reads the CSS-style list the config holds.

### "Manage" is not built, and will not be

*low, missing-feature, —*

**Obsidian.** A `Manage` button at the right of the Themes row, which opens its community theme
browser.

**Scoria.** No button. The probe could not record what it opens: offline, both the modal count
and the page stayed unchanged, so even the surface is unrecorded.

**Fix.** None intended. Scoria does not download themes from the community catalogue, and a
button that cannot do what its label says is worse than its absence. The folder button beside it
is the working half: it opens `themes/` so a theme can be dropped in by hand. Revisit only if
Scoria ever grows a catalogue of its own.

### "Quick font size adjustment" and "Custom app icon" are missing

*low, missing-feature, small*

**Landed 2026-11-04**: the probe settled both keys. "Quick font size adjustment" writes
`baseFontSizeAction` (`true`, then `false`, the key kept) and is live: Ctrl+wheel over a note steps
the size a pixel a notch between 10 and 30 and writes nothing, as Obsidian does, until the file's
own `baseFontSize` changes. "Custom app icon" is Obsidian's `"icon": "icon.png"` in `obsidian.json`
with the file copied beside it, so Scoria's is the same key in `scoria.json` with the copy in its
config folder; Restore default deletes both, and Relaunch starts a new Scoria on the vault and
quits. **Scoria cannot swap its own icon**: gpui takes a window icon only on X11, where the choice
is applied from the next start; on Windows the icon is the executable's resource and on macOS the
bundle's, so there the row only keeps the choice and is tagged "not in Scoria yet". See
[[Development#The font pages, quick font size and the app icon, 2026-11-04]]. Left: the trackpad
pinch (gpui's `PinchEvent`) does not step the size, and Scoria's dialog cannot filter to PNG, ICO
and ICNS — another kind is refused after the pick.

**Obsidian.** Two rows the tab has and Scoria does not: "Quick font size adjustment" ("Adjust the
font size using Ctrl + Scroll, or using the trackpad pinch-zoom gesture.") as a toggle under the
Font heading, and, under an **Advanced** heading, "Custom app icon" ("Set a custom icon for the
app") with a hidden-at-default "Restore default", a `Choose` button and a `mod-cta` `Relaunch`.

**Scoria.** Neither row, and no Advanced heading on this tab.

**Fix.** Blocked on the config keys: the probe recorded the rows but not what either writes, and
the ground rule is that a key is added only with the observed string. One more probe step that
clicks each and diffs `.obsidian/appearance.json` settles it. The behaviour behind the first —
Ctrl+Scroll and pinch to change the font size — is its own piece of work.

### "Check for updates" is inert

*low, parity-gap, —*

**Obsidian.** The "Current community themes" row's button checks the catalogue for updates to the
installed themes.

**Scoria.** The row is drawn to the record, with the count and the plural right and the button
dropped at zero themes as Obsidian drops it, but the button does nothing and the row carries the
"not in Scoria yet" tag. Same reasoning as "Manage".

### Syntax highlighting covers 85 languages where Obsidian covers 171 names

*low, missing-feature, medium*

**Landed 2026-11-02** as the engine and a first tranche of eleven, and **widened 2026-11-04** to
85 languages under 122 names; what follows is the gap that remains. See
[[Development#Syntax highlighting, 2026-11-02]] and
[[Development#Syntax highlighting breadth, 2026-11-04]].

**Obsidian.** Of 201 language names the probe tried, 171 produced tokens — effectively Prism's
whole component set, including every common language and a long tail of rare ones.

**Scoria.** 85 languages, 122 names with Prism's aliases (`js`, `ts`, `sh`, `shell`, `yml`, `md`,
`html`, `cs`, `kt`, `hs`, `rb`, `dockerfile`, `tex`, …): every common one a note holds —
JavaScript, TypeScript, JSX, TSX, markup (HTML, XML, SVG), Bash, C, C++, C#, Java, Kotlin, Scala,
Swift, Go, Rust, Python, Ruby, PHP, Perl, Lua, R, Julia, Haskell, Elixir, SQL, GraphQL, Markdown,
LaTeX, PowerShell, Batch, Makefile, Dockerfile, CSS, SCSS, Less, YAML, TOML, JSON, Nix and more.
84 of the probe's 171 highlighted names resolve, and so do six it listed as plain only because its
sample line held nothing for them (markup, xml, diff, docker, dockerfile, latex). A fence whose
language is not among them renders plain, which is exactly what Obsidian does for a name Prism
has no grammar for, so the gap shows as missing colour rather than as wrong colour.

**Fix.** The rest of the long tail — 87 of the probe's names, among them abap, actionscript, ada,
applescript, autohotkey, gherkin, haxe, hlsl, jq, lilypond, livescript, llvm, mongodb, plsql,
processing, pug, puppet, purescript, qml, reason, sas, smalltalk, sparql, stylus, textile, vala,
visual-basic and xquery: add the id to
`PORT` in `tools/prism-port/port.js`, write a sample in `tools/prism-port/samples/`, and re-run
the script; `every_ported_language_has_a_prism_sample` refuses an id without one. One hook is not
ported and does change colour: `js-templates`, which highlights the body of a tagged template
(`` html`…` ``, `` css`…` ``, `` sql`…` ``) in the language its tag names — Scoria paints that body
uncoloured. Adding an Obsidian sample to the first corpus needs
`tools/obsidian-probe/code-highlight.mjs` re-run against the Obsidian of the day.

## The Bookmarks pane, against the probe of 2026-11-07

Filed from `tools/obsidian-probe/bookmarks-pane.mjs`
([[Specification#Observed bookmarks pane, Obsidian 1.15.0]]). The pane itself was rebuilt the same
day — [[Development#The Bookmarks pane, against Obsidian 1.15.0, 2026-11-07]] — and what follows
is what that left.

### "Edit bookmark" is the one-field prompt; Obsidian's is setting rows with a group dropdown

**Obsidian.** Rows: Path (a disabled field), URL or Query; Title; "Bookmark group", a dropdown of
the groups with nested ones indented — then a red Remove on the left, Cancel and Save on the right.
"Add bookmark" has the same rows. Moving an item between groups from here is the only way that is
not a drag.

**Scoria.** Path, URL or Query as a line of detail text over one Title field, and Remove, Cancel,
Save in a row. Save and Remove do what Obsidian's do; there is no way to choose a group. Needs a
dropdown row in `prompt.rs`.

**Landed 2026-11-07**: `PromptSpec::rows` draws a prompt as setting rows (a fixed field, the text
field, a dropdown) with a danger button at the left; both bookmark modals use it, the dropdown
preselects the item's group (or the group whose menu asked), and Save moves the item.

### A note dragged in from the Files pane is not bookmarked

**Obsidian.** Dropped on the Bookmarks pane, a note from Files becomes a `file` item at the drop
(`s16`). **Scoria.** The Files view has no drag source at all, so this waits on the explorer's
drags (block 5 of [[Interaction probes]]).

**Landed 2026-11-07**, with the explorer's drags: a Files row drags with Obsidian's ghost, washes
its target folder, and drops by moving (numbered on a clash, links by the rename rules); held over
the Bookmarks tab it opens that pane, and dropped there it is a new bookmark at the drop. A rename
of any kind now carries bookmarks with it, as Obsidian's does.

### The view header has no filled bookmark on a bookmarked note

**Obsidian 1.15.0** draws a filled `bookmark` action in the view header of a bookmarked note, to the
left of the mode button, and none on a note that is not bookmarked — which is why 1.14.5's
was measured zero-rect on an unbookmarked note. **Scoria** draws no bookmark action there. Unprobed:
what a click on it does (likely "Edit bookmark").

**Landed 2026-11-07**, after `view-header-bookmark.mjs` settled it: "Bookmarked" is drawn first in
the header's actions while the file has a plain bookmark, filled in `--icon-color-active`, and a
click opens "Edit bookmark" on it.

## The table cell keyboard, against the probe of 2026-11-08

Filed from `tools/obsidian-probe/table-keys.mjs`
([[Specification#Observed table cell keyboard, Obsidian 1.15.0]]), after a report that Enter at the
end of a table would not end it and that typing stopped working in the app.

### The table cell keyboard is a field that never lets go

**Obsidian.** The caret lives in the editor, inside the table's source; there is no separate cell
field. Enter in the last row appends a row, Tab in the last cell appends a row, and the way out is
Arrow Down (or Arrow Right at the end of the last cell), which puts the caret at column 0 of the line
after the table. Escape does nothing. A click into a cell re-pads the table.

**Scoria.** A click opens a `CellEdit` (`table_field`) that owns the keys. Enter on the last row only
commits it, Tab on the last cell falls through to the editor, Escape *discards* the cell, and nothing
but Escape, a click on another cell or a note switch ever closes it, so a caret that has moved out of
the table (Arrow keys, a click below it, a table command, an undo) leaves the field open and **every
key goes to a cell nobody can see**. Seen live on 2026-11-08: the editor caret on the last line of a note,
`field: "cell"`, typing swallowed until Escape. Three things to do, in this order of importance:
close the field whenever the caret leaves the cell's table; make Arrow Down and Arrow Right at the end
of the last cell leave the table to the next line; make Enter in the last row and Tab in the last cell
append a row. Whether a click should re-pad the table is a byte question and wants its own decision.

**Landed 2026-11-08**: all three, and one more the probe showed. Arrow Up/Down walk the cells while one
is open, leaving above the header or below the last row; Arrow Right at the end of the last cell leaves
below; Enter in the last row and Tab in the last cell append a row (`RowAfter`) and open the cell in the
same column or the first. A click that is not on a cell, undo, redo, a table command and any other
movement through the document close the field first, which is what the stuck typing was. **Escape now
writes the cell** instead of discarding it, because Obsidian's typed text is already in the note and a
discard could lose it. Driven on a scratch vault: Enter and Tab appended padded rows, Arrow Down put a
typed character on the line after the table, Arrow Up on the line before it, a click on the paragraph
below left typing working, and Escape kept `4w`. **Still out**: the re-pad on entering a cell, and the
table commands, which act on the buffer's caret and so do nothing for an open cell.

### The inline title is not a field, and an arrow key into a table loses the caret

Filed from `tools/obsidian-probe/inline-title-and-table-entry.mjs`
([[Specification#Observed inline title and table entry, Obsidian 1.15.0]]), after a report that the
note's title could not be edited or selected and that the caret vanished when arrowing down through a
table.

**Obsidian.** The inline title is a `contenteditable` field: a click edits it, Arrow Up from the first
line enters it selected, Arrow Down, Tab and Escape return to the document, Enter renames the file. An
arrow key into a table puts the caret in its first cell.

**Scoria.** The inline title was only painted, and a table is always folded to its grid, so the
editor's own caret in its lines was never drawn and no field was open to take the keys.

**Landed 2026-11-08**: both. The title is the editor's own field (`title_edit`), with a selection, the
editing keys and the clipboard, painted through `inline_field` and renamed through the workspace's
`rename_with_links`; an arrow key entering a table opens its first cell, and the caret follows the open
cell. Driven on a scratch vault: a click put the caret mid-word and typing edited it, Escape restored
it, Arrow Up selected it, Enter renamed the file, Arrow Down left it, and three Downs from above a table
went blank line, header cell, first body cell. **Still out**: the reading view's inline title, Obsidian's
behaviour when the title loses focus (a press elsewhere abandons it here), and a re-pad on entering a
table.

### Move row up swapped the first body row with the header

*low, parity-gap, small* · **Filed:** 2026-11-08, from the Writer gate's day 3 ([[Writer gate run#Day 3, 2026-11-08]]).

**Obsidian.** 1.15.0, Live Preview, caret in the first body row: the palette offers *Table: Move row
down* and no *Move row up*. The check fails, so the row stays under the header.

**Scoria.** `table::apply` swapped the first body row with the header, as the 1.14.4 plugin probe had
recorded, and the palette offered the command anywhere in a table.

**Landed 2026-11-09**: the first body row no longer moves up, and the palette lists a table command
only when it would write something (`table::applies`, through `can_run`), which also hides Move row
down in the last row and the two column moves at the edges — inferred from the one case seen, and
recorded as such in [[Specification#Observed table editing behavior, Obsidian 1.14.4]].

### A table has no row or column handles

Filed from `tools/obsidian-probe/table-handles.mjs`
([[Specification#Observed table handles, Obsidian 1.15.0]]).

**Obsidian.** A grip above each column and left of each row, shown while hovered, that selects the line
when pressed and moves it when dragged, with an accent bar marking where it would land.

**Scoria.** The table was a grid with no handles, so a row or column could only be moved with the table
commands.

**Landed 2026-11-08**: the grips, the selection outline, the drag and its drop bar, and the move itself
(`table::move_line`, tested). Pressing a handle commits the open cell, selects the line and leaves the
caret alone; Escape, a click on a cell and a click anywhere else clear the selection. The drop bar tracks the pointer
beyond the table (clamped to the nearest edge line) and a release anywhere drops the line, because the
editor's own mouse-up makes the move rather than a drop target on the grid. **Driven 2026-11-08** in a
headless sway session, since the socket's `mouse` method does make a drag: the probe's eight drags all gave its
orders and undid to byte-identical, see [[Development#Live Preview tables, driven, 2026-11-08]].

**The "+" bars landed 2026-11-08**: "Add row below" under the last row and "Add column to the right"
past the last column, 16 px strips as long as the table with a `plus` in the middle, invisible until
hovered, with the resize cursors Obsidian's carry. A click commits the open cell, appends the line
through `table::append_line` (the row and column commands at the table's last line, so the table is
re-padded as Obsidian's is) and opens the new row's first cell or the new column's header cell, which
is where the probe found Obsidian's cursor. The bar is not a handle, so it also drops a handle's
selection. Driven on a scratch vault over the socket: a click just past the last column wrote a fourth
`|     |` column with the header cell open, a click just under the last row wrote `|     |     |     |     |`
with its first cell open, and typing there landed in the file. Seen in passing: typing into an empty
cell inserts before its padding (`|xy     |`), because the parser's span for an empty cell is zero-width
at the cell's start; what Obsidian writes there is not probed.

## Blocked by the harness, not by the code

Five things could not be settled from outside the process. They are not findings and not fixes —
they are measurements the socket cannot make, and each needs a human at the desk or a change to the
state dump.

**The clipboard.** *Unblocked 2026-10-25 (`69d8063`): it is not a no-op, and the condition it
needs is the window's own focus.* Driven over the socket with Scoria focused, a click on a Live
Preview code-block flair and a click on the reading view's copy button each left `wl-paste`
holding the block's body with exactly one trailing newline off; the empty fence's flair, hovered
and clicked in the same run, left the clipboard untouched. The same clicks, with the pointer aimed
the same way while another application held focus, changed nothing at all -- which is the serial
the paragraph below is about, and the reason roughly 1500 attempts once came back empty. So a
clipboard assertion is now a thing the harness can make, as long as whoever drives it makes sure
the window is focused and reads the clipboard rather than a screenshot: the check on the copy
button reverts at 1000 ms, which is shorter than a screenshot round trip. The reading view's copy
button is confirmed; the round trip in every text input is still unverified. What follows was the
state before that.

gpui's `write_to_clipboard` is a no-op in a socket-driven session: a Wayland
client cannot set or receive a selection without keyboard focus and a real input serial, and
synthetic events carry none. The control is the editor's own pre-existing copy — Ctrl+A then Ctrl+C
then Ctrl+V left the note byte-identical, and `wl-paste` never saw anything the app wrote.
Confirmed again by the second pass's source triage, which verified Ctrl+A, Shift+arrows and Tab in
the find fields directly and had to mark Ctrl+V fixed on the code alone. So two things are
implemented, reviewed and unconfirmed: the reading view's code-block copy button (roughly 1500
hover-then-click attempts over its computed box produced no clipboard change, with the button's
geometry verified independently in the same session) and the clipboard round trip in every text
input. Worth checking with a real mouse: gpui's `Interactivity::paint` returns early on
`Visibility::Hidden` *before* it paints mouse listeners, so an `.invisible()` button has no click
listener in any frame where its group is not hovered.

**Drag-select.** *Unblocked 2026-10-25: the premise was already out of date, and what was missing
was a tool rather than the event.* `control.rs`'s `mouse` action takes a `button` on a `move` and
sends it as `pressed_button`, which is what makes the move a drag; what no harness could reach was
the method itself, because `scoria-mcp`'s tool table is hand-written and never listed it. It does
now. Driven over the socket: down at the editor's line 1, two moves holding left, up on line 2
left `editor.selection` at bytes 7..115, and the same gesture across the find panel's query
replaced the five characters it caught when the next keystroke arrived (`alpha beta gamma` typed
over as `alpha betaZa`). So both halves are verified -- the editor's own drag-select and the one
added to `TextInput`. What follows was the state before that.

`control.rs`'s `mouse` action always sends `MouseMoveEvent { pressed_button: None }`,
so down/move/move/up moves the caret and selects nothing. The drag-select added to `TextInput` is
therefore unverified, although single, double, triple and shift-click all verified.

**Rendered rows in the Settings modal.** *Unblocked 2026-10-24 (`ffe5394`, protocol 2):
`state.modal.rows` now carries the open tab's rows — name, description, control kind and value, and
the honesty tag — and the Hotkeys tab reports commands with their ids and bound chords. What follows
was the state before that.* `state.modal` reports the tab, the open dropdown by config
key, the open text field by config key and the hotkey filter — enough to settle most of the
subsystem by clicking a coordinate grid and watching for a state change or a byte change in
`.obsidian/*.json` — but `modal.rows` is empty for this modal, so no row's text can be read. Every
purely visual claim about it (toggle size, slider thumb, the explanatory banner, the "not in Scoria
yet" chips, the uppercase nav headings, the rows Obsidian has that Scoria lacks, the inert buttons)
is unreported in either direction, the Hotkeys rows cannot be read at all, and the plugin tabs'
"Options not shown yet" row had to be settled from the source. Exposing the row names in
`state.modal.rows` would make the whole tab testable.

**The status bar, for five seconds after anything.** The notice layer means `state.status` returns
the newest live notice in preference to the real status, so any finding whose evidence is a
status-bar string is unreadable for five seconds after any notice fires. The way around it, which is
how the autosave-reveal entry was settled: set a sentinel from a non-notice path (touch an unrelated
file, so `after_changes` writes "<path> changed"), let the notices drain, then act and re-read.

**The window title.** *Unblocked 2026-10-24 (`ffe5394`): `window` and `state.window` both carry
`title` now.* The socket's `window` method answers width, height, scale and active, with no
title field, and the machine has no `wmctrl`, `xdotool` or `xprop`. Adding `title` to that method is
one line and would close the last could-not-test in the shell subsystem.
