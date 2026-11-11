---
title: The window
description: A tour of Scoria's window — the left dock with Files and Search, the note in the middle, the right dock with Backlinks, Outgoing links, Outline and Tags, the status bar, popout windows, and how navigation works.
---

# The window

Scoria opens one vault window per vault, in a three-part shell: a dock on the left, the open
note in the middle, a dock on the right. Both docks can be hidden, which leaves the note
alone on screen. Tabs can also be taken out into [popout windows](#popout-windows) of their
own.

```
┌─────────────┬────────────────────────────────┬──────────────┐
│ Files       │ ▸ Note.md  ▸ Other.md          │ Backlinks    │
│ Search      │          the open note         │ Outgoing     │
│ Bookmarks   │   (reading, Live Preview or    │ Tags         │
│  ← left     │          Source mode)          │ Properties   │
│    dock     │                                │ Outline   →  │
├─────────────┴────────────────────────────────┴──────────────┤
│ vault                              412 words, 2310 characters│
└─────────────────────────────────────────────────────────────┘
```

## The left dock

Three tabs:

- **[Files](/docs/app/files/)** — the folder tree of the vault. The tab Scoria opens on.
- **[Search](/docs/app/search/)** — vault-wide search with the full query grammar.
- **Bookmarks** — the tree in your vault's `bookmarks.json`: files, folders, headings,
  blocks, searches and groups, read and written in Obsidian's bytes.

`Ctrl + Shift + F` opens Search; `file-explorer:open` returns to Files and
`bookmarks:open` to Bookmarks. `Ctrl + E` is not involved — toggling the dock itself is
`app:toggle-left-sidebar`.

## The note

The middle is one note, in one of three modes:

| Mode | What it is | Switch with |
|---|---|---|
| Reading view | The finished note, rendered | `Ctrl + E` (`markdown:toggle-preview`) |
| Live Preview | Editing, with syntax hidden unless the cursor is in it | `Ctrl + E` from reading view |
| Source mode | Editing, with every marker visible | `editor:toggle-source` |

Which one a note opens in follows your vault's settings: `defaultViewMode` decides reading
view or editing, and `livePreview` decides which of the two editing modes. See [editing a
note](/docs/app/editing/).

The middle is a tab strip over one or more tab groups. `Ctrl + T` opens a tab,
`workspace:split-vertical` (Split right) and `workspace:split-horizontal` (Split down)
divide the area into groups, the dividers between groups drag, and a tab drags between
groups or onto a group's edge to make a new one. `Ctrl + Tab` and `Ctrl + 1`…`8` walk the
tabs, and `editor:focus-left` and its three siblings move between groups. The layout is
written back to `workspace.json` in the shape Obsidian writes it, and the next launch opens
on it again.

Right-click a tab's header for its menu: **Close**, **Close others**, **Close tabs after**
and **Close all** (the others only when the group has more than one tab, and Close tabs
after only when there is a tab to its right), **Pin** or **Unpin**, and **Move to new
window**. On the tab in front the menu goes on with **Split right**, **Split down** and
**Open in new window**, then the tab's file rows — Rename, Move, Bookmark, Copy path,
Delete and the rest of the [file menu](/docs/app/files/#the-right-click-menu), plus **Add
file property**, **Find...** and **Replace...** on a note. The right click does not bring
the tab to the front. The view header's **More options** offers the same split and window
rows.

A pinned tab shows a pin where its close button was; click the pin to unpin it. A note
opened from a pinned tab — a link followed, a file picked in the quick switcher — opens in
a new tab, `Ctrl + W` unpins it rather than closing it, and Close others and Close all
leave it open. **Toggle pin** in the command palette pins and unpins the tab in front.

## Popout windows

A popout window is a second window on the same vault that holds tabs and nothing else: no
ribbon, no docks, no status bar. It is titled like the vault window, after the note in
front, and inside it tabs, splits, the dividers and `Ctrl + T` all work as they do in the
main area.

| To | Do |
|---|---|
| Move the current tab into a new window | `workspace:move-to-new-window`, or **Move to new window** on the tab's right-click menu |
| Open a copy of the current tab in a new window | `workspace:open-in-new-window`, or **Open in new window** on the tab's menu or the view header's **More options** |
| Open an empty window | `workspace:new-window` |
| Open the link under the cursor in a new window | `Ctrl + Alt + Shift + Enter` (`editor:open-link-in-new-window`) |
| Open a file in a new window | **Open in new window** on its right-click menu — in Files, on a Backlinks or Outgoing links row, a bookmark, a Bases row or card, or a link in the editor |
| Close a window | `Ctrl + Shift + W` (`workspace:close-window`), the window's own close button, or closing its last tab |

Moving the only tab of the vault window leaves an empty tab behind, as Obsidian does. A
link to a note that does not exist creates it, as following one does. `Ctrl + Shift + W`
closes the window it is pressed in: in a popout, that popout and its tabs; in the vault
window, Scoria itself, with every popout going with it.

**Focus follows the desktop.** The window you bring forward — by clicking it, with
`Alt + Tab`, however your desktop does it — makes its own most recent tab the active one,
so commands, hotkeys and the history act there. The quick switcher, the command palette,
the prompts, the right-click menus and notices all come up in the window you raised them
from, and a note picked in a popout's switcher opens in that popout.

**Tabs drag between windows.** A tab dropped on another window's tab strip joins it there,
and dropped on the edge of its content splits it, exactly as within one window. A popout
whose last tab leaves closes; a tab let go over no window stays where it was.

**Popouts come back.** When Scoria quits with popouts open, each is written to
`workspace.json`'s `floating` section with its position, size and maximized state, in the
shape Obsidian writes it, and both apps reopen them there on the next launch. A recorded
position that is on no display any more is centered instead.

> **Note** On Wayland a dragged tab stays in the window it started in, because a Wayland
> window is not told where it sits on the screen; reordering and splitting inside the window
> still work. A popout's recorded zoom is kept in the file but not applied. Popouts have so
> far been exercised on Linux; the Windows and macOS builds are still to be tried with them.

## The right dock

Five tabs, in Obsidian's order. Three are about the open note; Tags and All properties are
about the whole vault:

- **[Backlinks](/docs/app/panes/)** — notes that link here, and unlinked mentions of this
  note's name or aliases.
- **[Outgoing links](/docs/app/panes/)** — where this note points, resolved and unresolved.
- **[Tags](/docs/app/panes/)** — every tag in the vault with its count.
- **[All properties](/docs/app/panes/)** — every property key in the vault with its type and
  count.
- **[Outline](/docs/app/panes/)** — this note's headings; click one to scroll to it.

`app:toggle-right-sidebar` hides and shows the dock. Each tab also has a command of its own
(`backlink:open`, `outgoing-links:open`, `tag-pane:open`, `properties:open`,
`outline:open`), so you can bind a key straight to the pane you want.

## The status bar

Along the bottom: the vault's status on the left, and the open note's word and character
count on the right. While you are editing, the right-hand side also carries the cursor's
line and column, the number of cursors when there is more than one, and the parser and
shaping timings for the note you are in:

```
Ln 212, Col 7  ·  parse 18.4 ms, 9120 spans  ·  shape 0.21 ms  ·  412 words, 2310 characters
```

Those timings are there on purpose rather than left in a debug build: the editor reparses
the note in the background after each edit, and this is where you can see that it is keeping
up. See [performance](/docs/reference/performance/).

## Navigating

| To | Do |
|---|---|
| Open a note | Click it in Files, or click a link in a note |
| Find a note by name | `Ctrl + O` — the [quick switcher](/docs/app/switcher/) |
| Run anything by name | `Ctrl + P` — the [command palette](/docs/app/switcher/) |
| Go back / forward | `Ctrl + Alt + ←` / `Ctrl + Alt + →` |
| Jump to a heading | Click it in Outline, or follow a `[[Note#Heading]]` link |
| See the vault's shape | `Ctrl + G` — the [graph](/docs/app/graph/) |
| Open Settings | `Ctrl + ,` |

Clicking a link to a note that does not exist creates it — the same as Obsidian, and the
same as pressing Enter in the quick switcher on a name nothing matches. See [managing
notes](/docs/app/manage/).

History is per window and linear: opening a note from the graph, from a link, from Files or
from the switcher all push onto the same back stack.

## Keys

Every shortcut in Scoria is an Obsidian command bound from its default hotkey, so your
vault's `hotkeys.json` rebinds or unbinds all of them. The one exception is `Ctrl + Q`,
which quits.

The 198 commands Scoria can actually run are the ones the command palette offers; the rest
are still listed in Settings' Hotkeys tab, so you can see your bindings for commands that
do not do anything yet. The full list is in [commands and
hotkeys](/docs/reference/commands/).

## The theme

Colors come from your vault's `appearance.json` — the base theme (dark or light) and the
accent color — and are read before the window opens, so the first frame is already right.
A CSS theme (`cssTheme`) is not applied; that is a later phase.

Fonts come from `appearance.json` too: the interface font, the text font and the monospace
font, with the font size.

> **Note** The default text and interface font is Liberation Sans rather than something more
> modern, because the pinned GPUI cannot pick weights out of a variable font — with a
> single-file variable font, nothing renders bold. Set `textFontFamily` and
> `interfaceFontFamily` in `appearance.json` to any font with real static Bold faces and it
> is honored.

## Next

- [The Files view](/docs/app/files/)
- [Reading view](/docs/app/reading-view/)
