---
title: Backlinks and the side panes
description: The right dock — Backlinks with linked and unlinked mentions, Outgoing links with unresolved ones, the vault-wide Tags and All properties panes, the Outline of headings, and the editable File properties pane.
---

# Backlinks and the side panes

The right dock holds five tabs, in Obsidian's order: Backlinks, Outgoing links, Tags, All
properties, Outline. Three are about the note you are reading; Tags and All properties are
about the whole vault. `app:toggle-right-sidebar` hides and shows the dock, and each tab has
a command of its own so you can bind a key straight to it. A sixth, File properties, joins
them when its command asks for it, and can be closed again.

## Backlinks

`backlink:open` (`backlink:open-backlinks` opens it [beside the note](#linked-views))

Every note that points at the one you are reading, in two sections.

**Linked mentions** are real links, grouped by the note they come from: a collapsible row
per file with its count, over a card of that file's matches. Each match is its own line of
context with the link highlighted in it, so you can see the sentence it was used in. A note
that links here three times is one group of three matches.

**Unlinked mentions** are places where this note's name — or any of its aliases — appears as
plain text that could have been a link but is not. The matching is deliberately narrow, so
the pane stays useful rather than noisy:

- Whole words only, compared case-insensitively.
- Names *and* aliases from the `aliases` property.
- When two notes' names overlap, the longest name wins.
- Never inside an existing `[[wikilink]]` or `[Markdown](link)`.
- Never inside inline code or a fenced code block.
- Deduplicated per place, so one mention is one row.

A **Link** button appears under the pointer on an unlinked mention and writes the link for
you — `[[Name]]`, or `[[Name|text]]` when the text is an alias or another case, or a
Markdown link under `useMarkdownLinks` — into the note the mention is in, after checking
that file is still the bytes the mention was found in.

### The header

Four buttons, Obsidian's: **Collapse results**, **Show more context** (the non-blank line
either side of each match), **Change sort order** (the Files view's six orders, in a menu
with a check) and **Show search filter**, an in-pane field that keeps groups whose name
matches and other groups' matching lines. Each group folds on its own row. All of it lives
in the leaf's own keys in `.obsidian/workspace.json` — `collapseAll`, `extraContext`,
`sortOrder`, `showSearch`, `searchQuery`, `backlinkCollapsed`, `unlinkedCollapsed` — read at
startup and written back in place.

## Outgoing links

`outgoing-links:open` (`outgoing-links:open-for-current` opens it [beside the note](#linked-views))

Where this note points, as Obsidian lists it: one row per target and subpath, so
`[[Beta]]`, `[Beta](Beta.md)` and `![[Beta]]` in the same note are one row. Each row wears a
glyph for what it points at and carries, underneath, the note a heading or block belongs to
or the folder a note sits in. A row for a note that does not exist yet says *Not created
yet*; clicking it creates the note, as clicking the link in the note itself would.

This pane also lists unlinked mentions *of other notes* found in this note, which is the
other direction of the same idea: notes you have named in passing and could link to. Each
match carries a *Link this file* chip naming the note it would link to.

> **Note** Links in your frontmatter count as links, in both panes and in the graph. A
> `source: "[[Paper]]"` property is a link to *Paper*.

## Tags

`tag-pane:open`

Every tag in the vault with the number of **occurrences** carrying it, written without the
`#`. Counts merge the two ways a note can be tagged, case-insensitively:

- Inline `#tags` in the body.
- The `tags` property in frontmatter — whether written as a YAML list or as a comma-separated
  string, with or without the leading `#`.

So `#Project`, `#project` and `tags: [project]` are one tag with a count of three, spelled
the way its first indexed occurrence spells it.

Nested tags nest: `project/scoria` is a foldable row under `project`, and a parent's count
includes its descendants' — which is also why a `tag:#project` search matches them. Turn
**Show nested tags** off and every tag becomes one flat row carrying its `project/` prefix.
See [search syntax](/docs/reference/search-syntax/).

Clicking a row searches for that tag. The header's four buttons are **Change sort order**
(name or frequency, either direction, ties A to Z at every level), **Show nested tags**,
**Expand all** and **Show search filter**.

## All properties

`properties:open`

Every property key in the vault, with the number of notes carrying it and a glyph for the
widget Obsidian would give it — text, list, number, checkbox, date, datetime, tags, aliases,
or a question mark for a key whose value is a nested mapping. Keys declared in
`.obsidian/types.json` are listed even at a count of zero, and two spellings of one key merge
case-insensitively under the one seen first. The header offers sort order and a search
filter; clicking a row searches for the key.

## File properties

`properties:open-local`

The open note's properties as an editable table, after the five tabs: keys, values, the type
menu and *+ Add property* work as they do in [Live Preview](/docs/format/properties/#editing-properties)
and write the same bytes, whichever mode the note is in. The command makes the tab the first
time and shows it after that, with the keyboard in it. With no note open, or a file that is not
a note, it says *No properties found.* A right click on its tab offers **Close**, and
`Ctrl + W` closes it while it has the keyboard; the dock then shows the tab before it, and
`.obsidian/workspace.json` loses the leaf as Obsidian's does.

## Outline

`outline:open` (`outline:open-for-current` opens it [beside the note](#linked-views))

The note's headings, nested structurally: a heading sits under the nearest preceding heading
of a smaller level, whatever the numbers. Click one to put the cursor on its line, centred.
Works in reading view and while editing, and it follows the buffer as you type rather than
waiting for the file to reach disk.

The heading whose section holds the cursor — or, in reading view, the line at the top of the
view — is marked. The header's three buttons are **Auto-scroll to current section**, which
decides whether that row is also scrolled into view, **Expand all** / **Collapse all**, and
**Show search filter**, which keeps the matching rows and their ancestors.
A note with no headings says *No headings found.*

Each pane's own state — its sort order, its folds, whether its filter box is showing and
what is typed into it — lives in that leaf of `.obsidian/workspace.json` and is read at
startup and written back in place. A vault Obsidian never laid out gets Obsidian's default
docks written for it, so nothing is lost on quit.

## Linked views

Backlinks, Outgoing links, File properties and Outline can also be opened in the main area,
beside a note and about that note only. Right-click the tab of the note in front and choose
**Open linked view**: on a note it offers **Open local graph**, **Open backlinks**, **Open
outgoing links**, **Open file properties** and **Open outline**; on a canvas, a PDF, an image
or a base, **Open backlinks** alone. The view opens in a new pane — below the note for
Backlinks, Outgoing links and File properties, to its right for the Outline and the [local
graph](/docs/app/graph/#the-local-graph) — and its tab is linked to the note's: **Backlinks
for** *the note's name*, **Outgoing links from** …, **File properties for** …, **Outline of**
…. Choose the row again and you get another pane.

A linked view follows the note's tab, whatever is opened there, and leaves the other tabs
alone. A row clicked in it opens in the note's tab and brings that tab to the front; an
Outline heading puts the note's cursor on it. Each view keeps its own buttons' state — a
linked Backlinks pane's collapse, context, sort order, search and folds are its own, not the
dock's — and takes it along when the note moves on. Close the note's tab, or choose **Unlink
tab** on the view's own tab, and the view is unlinked: its tab reads plainly, **Backlinks**, and from then on it follows whatever file is
open. Linked File properties edits the note through the note's own tab; it takes the keyboard
when it opens.

The commands `backlink:open-backlinks`, `outgoing-links:open-for-current` and
`outline:open-for-current` open the same view unlinked, in a new pane to the right, following
whatever file is open. All of these are written to `.obsidian/workspace.json` as Obsidian
writes them, the link included, and open again where they were.

## Right-click menus

A right click on a Backlinks or Outgoing links row opens the menu of the file it stands for:
**Open in new tab**, **Open to the right**, **Open in new window** (a [popout
window](/docs/app/window/#popout-windows)), then the file's own rows. An All properties row
has a menu of its own: Rename, Property type and Delete property.

## Keeping up with the vault

All five panes follow the vault. A note saved in another window, a file added by git, a
folder dropped in — the panes refresh within a moment, and so do the resolved and unresolved
link counts, because a note appearing can make a link elsewhere resolve.

## Not yet

- Dragging a row.
- Per-match "Show more context" chevrons in the Outgoing links pane.
- A local-graph side pane (the [graph](/docs/app/graph/) has a local mode of its own).

## Next

- [Search](/docs/app/search/)
- [Internal links](/docs/format/links/) — how link resolution actually decides.
