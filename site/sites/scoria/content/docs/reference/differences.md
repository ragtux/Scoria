---
title: What differs from Obsidian
description: The honest list — everything Obsidian does that Scoria does not do yet, the places Scoria deliberately behaves differently, and what is permanently out of scope.
---

# What differs from Obsidian

Three kinds of difference, kept apart because they mean different things: **not yet**,
**deliberately different**, and **not ever**.

If you are deciding whether Scoria can be your daily editor, this is the page to read. It is
written against 0.5.0; the [release notes](/releases/) say what each version moved out of the
first list.

## Not yet

Things Obsidian does that Scoria intends to do and does not.

### Rendering

| | |
|---|---|
| **Math** | Typeset with a KaTeX-compatible engine rather than MathJax: the common syntax renders alike, a MathJax-only command shows its source in red. No `\newcommand` across formulas, no `\require` |
| **Some HTML in notes** | Inline tags, `<details>`, `<pre>`, container tags and raw tables render. Not yet: an inline `style` attribute, `<sup>` and `<sub>` at a smaller size and raised or lowered (they draw at body size), `<iframe>`, `<form>` and other tags Scoria has no drawing for (they show as source), and a raw table's `colspan`, `rowspan` and `<caption>` |
| **An image in a line of text** | An image embed that shares its paragraph with text breaks the paragraph in reading view; a remote image in a heading or a table cell stays a link |
| **Mermaid diagrams** | Not rendered |
| **Code highlighting in rarer languages** | Fenced code is highlighted by a port of Prism: 85 languages under 122 names, against the 171 names Obsidian's Prism highlights. A block in any other language renders plain |
| **PDF text, spreads and export** | A PDF opens in a viewer with Obsidian's toolbar, sidebar and find bar, and `![[x.pdf]]` embeds it. The text cannot be selected or copied, there are no two-page spreads and no Adapt to theme, and a password-protected PDF does not open. Export to PDF (`workspace:export-pdf`) lays the reading view out on pages of its own rather than printing it through a browser: the page breaks differ from Obsidian's, a table row or an image taller than a page is not split, callouts have no icon, and an embedded base, PDF or canvas prints as its link |
| **Audio and video players** | Recognized as file kinds; no player, in a tab or embedded |
| **A canvas embedded in a note** | `![[Board.canvas]]` is not drawn; the file opens in a view of its own |

### Views and windows

| | |
|---|---|
| **Popout windows, the remaining corners** | In: the five window commands, **Open in new window** on the menus, tab drags between windows, and popouts reopened on launch. On Wayland a dragged tab stays in its own window, and a popout's recorded zoom is not applied. On X11, after the window manager un-maximizes a popout, Scoria has the window's position wrong until it is moved, so drags to and from it do nothing (a gpui bug). The tab menu has no **Link with tab** and no **Open linked view**, since Scoria has no linked panes. Popouts have not been tried on Windows. See [the window](/docs/app/window/#popout-windows) |
| **Stacked tabs** | Tabs and split groups are in; stacking them (`workspace:toggle-stacked-tabs`) is not |
| **Workspaces** | Saved layouts work (`workspaces:*`, the manage window, the ribbon button). After a layout is loaded `workspace.json` is rewritten when Scoria quits, not a second later |
| **A second vault beside the first** | Opening or switching to another vault from inside Scoria replaces the one in the window, where Obsidian opens it in a window of its own. Run Scoria again to have both open |
| **The local graph as a side pane** | The graph, global or local, is a view of the main area, and `Ctrl + G` returns to the note. `graph:animate` brings the nodes back in creation order at a fixed 0.39 s a step, where Obsidian's steps vary, and the graph has no screenshot export. See [the graph](/docs/app/graph/) |
| **Bases: cell selection** | Shift-click selection of table cells, with copy and paste over it, is not built |
| **Canvas export and options** | Export as image (`canvas:export-as-image`) draws the canvas itself rather than capturing the view: Scoria's mark is in the logo strip, a card's text is clipped at its edge and is drawn in full at any zoom, and an embedded canvas or base in a card shows only its name. The Canvas plugin's settings (default location, wheel and drag behavior, snapping, card names, the zoom threshold) are written for Obsidian but not acted on |

### Editing

| | |
|---|---|
| **Note composer** | Merge, extract selection and extract heading work. `Ctrl + Enter` in an extraction prompt does nothing yet, and the template setting is a path field, not a file picker |
| **Unique note creator** | Both commands work. **Add unique internal link** with several cursors edits only the first; the template setting is a path field |
| **Slash commands** | Built. Within a query, rows that Obsidian's matcher scores equally can come in a different order (`/toggle` puts *Toggle left sidebar* first where Obsidian puts *Toggle pin*), the popup's width is estimated from its longest row, and an editing page preview has no slash popup. The list is the palette's, so it differs from Obsidian's where the palette does |
| **Renaming a heading** | `editor:rename-heading` works as Obsidian's does, oddities included. Links to the heading in frontmatter are not rewritten |
| **File recovery** | Scoria keeps its own snapshots, outside the vault under its configuration folder (`file-recovery/`); Obsidian keeps its in its own database, so neither app sees the other's. Snapshots are taken of notes and canvases on opening and on a change, once per **Snapshot interval**, and pruned by **History length** when the vault opens. The tab's **View** and **Clear** buttons do nothing yet; `file-recovery:open` opens the snapshots. **Show changes** compares a snapshot with the file now, line by line |
| **Editing properties in reading view** | A property's value pill can be removed there; keys, values, the type menu and **Add property** are edited in Live Preview or Source mode only |
| **Editing the title in reading view** | The inline title is an editable field in Live Preview and Source mode; in reading view it is plain text. `F2` works everywhere |
| **Vim mode** | The common core is there (modes, motions, operators, text objects, registers, search, `.`); visual block, macros, marks, `:` commands and `.obsidian.vimrc` are not |
| **Line numbers, indentation guides, right-to-left** | Read and shown in Settings, not acted on. `editor:toggle-line-numbers` writes the setting for Obsidian to read; no gutter is drawn |
| **Spellcheck** | No spellcheck, and `editor:toggle-spellcheck` is not a command yet |
| **Page preview from File properties** | Page preview works from links, panes, tabs, bases and the graph (see [links](/docs/format/links/#page-preview)); Scoria has no File properties view to preview from |
| **Pasting and attachments** | A paste is text: copied HTML is not converted to Markdown, and a pasted or dropped image or file is not saved as an attachment. `editor:attach-file` and `editor:download-attachments` are not implemented. Put the file in the folder yourself |

### Panes and plugins

| | |
|---|---|
| **Footnotes view** | The pane lists the note's footnotes and jumps to a definition; a footnote cannot be edited in the pane |
| **Backlinks at the bottom of a note** | Shown, per tab and by the Backlinks setting. The block's own sort menu and search filter are not drawn |
| **Editing in the File properties pane** | `properties:open-local` shows the current note's properties in a side pane; they cannot be edited there, and the pane cannot be closed |
| **Slides, Audio recorder, Web viewer** | `slides:start` runs, set in the interface font, with its keys live at once (Obsidian's wait for a click on the slide) and links on a slide inert; `audio-recorder:start` and `:stop` record the default microphone to `Recording YYYYMMDDHHmmss.m4a` (AAC-LC, mono) where Obsidian puts it, but the encoder runs slower than real time, so the file lands a while after Stop (about a third of the recording's length) under a **Saving recording…** notice that Obsidian does not have; it asks for 96 kbps and writes about 46–48. The ribbon button toggles. An audio file opens on a placeholder, Scoria having no audio player yet. `webviewer:*` is not implemented (`markdown-importer:open`, the old Format converter, is not in Obsidian 1.15.0 either) |

### Files and settings

| | |
|---|---|
| **New folder** | Asks for a name in a prompt first, where Obsidian creates `Untitled` and renames it on the row |
| **System trash on Windows and macOS** | Implemented on Linux (freedesktop trash with a `.trashinfo` record); elsewhere it falls back to the vault's `.trash/` |
| **Excluded files** | `userIgnoreFilters` is shown in Settings but cannot be edited there, and search, the graph and unlinked mentions do not hide what it excludes |
| **Settings Scoria does not act on yet** | The rows write what Obsidian writes, bar Spellcheck languages, Excluded files and Override config folder, and the option tabs of plugins Scoria does not have, which show their values only. Rows for behavior Scoria lacks are written for Obsidian and marked "not in Scoria yet": Language, Show tab title bar, Native menus, Always focus new tabs, Convert pasted HTML to Markdown, the attachment location rows, and the plugin options above. **Custom app icon** takes effect on X11 only. See [Settings](/docs/settings/) |
| **Search result context menu** | Sorting, copying, collapsing and the keyboard are in; a result has no right-click menu yet. See [search](/docs/app/search/) |
| **URI links from a browser on macOS** | A URI given on the command line works on all three platforms; a `scoria://` or `obsidian://` link clicked in a browser does not reach Scoria on macOS yet |

### Commands

The command palette lists only what Scoria can run. Beyond the ones named above, these
Obsidian commands are not implemented yet:

| | |
|---|---|
| **Window** | `window:zoom-in`, `window:zoom-out`, `window:reset-zoom`, `window:toggle-always-on-top` |
| **App** | `app:reload`. Frontmatter migration converts and re-serialises a note's block byte for byte as Obsidian did, and its Undo lasts as long as its notice. The font size the two font-size commands set lasts the session; Obsidian keeps it across launches. Open file from outside the vault opens a file inside the vault in the current tab, as Obsidian does, but refuses one outside it, which Obsidian opens in a tab of its own and saves back to. Show debug info reports Scoria's version and settings in Obsidian's layout, leaving out the lines Scoria has nothing for. Show release notes shows Scoria's notes for the running version, taken from this site's releases page when the app was built, where Obsidian downloads its changelog |
| **Add alias** | `markdown:add-alias` works in Live Preview; in Source mode it writes the key and opens File properties, which cannot be typed into; it is not offered in reading view |

### Markdown edge cases

One of the 1,009 corpus notes does not match Obsidian's output: a hand-written probe of inline
edge cases, on how runs of `*` and `_` pair up when they are ambiguous on purpose
(`_a_b _a_`, `*a **b* c**`, `****a****`, `**a*b**c*`, `~~*a*~~`) and on
`==== four ==x===y==`. See [compatibility](/docs/reference/compatibility/).

## Deliberately different

Places where Scoria does not copy Obsidian, on purpose.

**Frontmatter links are rewritten in place.** When a rename changes a link in your YAML,
Obsidian re-serialises the whole frontmatter block — which can turn `aliases: [Bee]` into a
block list, changing bytes you did not touch. Scoria replaces the link text and leaves every
other byte alone. This is a difference in Scoria's favor and it is not going to be
"fixed".

**The command palette lists only runnable commands.** This matches how Obsidian hides a
command whose check callback fails, but the effect is more visible here because more commands
are unimplemented. The [Hotkeys tab](/docs/hotkeys/) still lists every command Obsidian names
on desktop — 227 of the 244 ids — so your bindings are visible.

**The default font is Inter, on every platform.** Obsidian's default stack starts with the
system's interface font (`ui-sans-serif`, `-apple-system`, `system-ui`, `Segoe UI`) and falls
back to Inter, which is what it renders on Linux; on macOS and Windows it normally renders the
system's font. Scoria bundles Inter's static faces and uses them everywhere, because the pinned
GPUI cannot select weights from a variable font. The font families in `appearance.json`
override it.

**The default accent is brick, not purple.** With no accent color set, Scoria's accent is
`hsl(14, 62%, 46%)`, a brick from the logo's palette, where Obsidian's is a purple. It is declared exactly
where Obsidian declares its own, so an accent set in Settings, or a theme's, still wins, and
nothing is written to your vault.

**A second bold over task lines removes the bold.** Over `- [ ] **task**` lines Obsidian's
second `Ctrl + B` changes nothing; Scoria's strips the bold, as it does over any other line.

**The URI scheme Scoria registers is `scoria://`.** Obsidian owns `obsidian://` on a machine
that has it; Scoria reads `open`, `new`, `search` and `daily` under its own name, and takes
`obsidian://` too when its desktop entry is installed to claim it. A URI that names another
vault is refused rather than switching the window to it.

## Not ever

**Obsidian's community plugins will not run in Scoria.** They are JavaScript against an
Electron app's API, with Node and DOM access. Scoria is a native binary with neither.
Embedding a JavaScript runtime and a DOM to host them would rebuild the thing whose absence
is the entire point of the project.

What is planned instead is a **native plugin API of Scoria's own** (tier 4, the last phase).
Your `community-plugins.json` and plugin manifests are read and listed in Settings, so you can
see what your vault has — they are simply not executed.

**A theme's CSS rules will not apply.** Scoria reads a theme's custom properties and drops
its selector rules, pseudo-elements, layout overrides and animations; with no DOM there is
nothing for a rule to select. See [tier 3](/docs/reference/compatibility/#tier-3-themes). Nor
does Scoria browse or download community themes: put a theme in `.obsidian/themes/` and pick
it in Settings.

**Obsidian Sync, Publish and the commercial services** are Obsidian's products, not file
formats, and their commands (`sync:*`, `publish:*`) will not exist in Scoria. Scoria has
nothing to do with them. Use any file-level sync — git, Syncthing,
Dropbox, or Obsidian Sync itself: they all move files, and files are all Scoria needs.

## Keeping this page honest

This list is maintained against `docs/scoria/Development.md` in the repository, which carries
the per-item status and the gate notes. If you find something missing from it — a difference
you hit that is not written here — that is worth
[reporting](https://github.com/ragtux/scoria/issues) in its own right.

## Next

- [Obsidian compatibility](/docs/reference/compatibility/) — the tiers and the numbers.
- [Troubleshooting](/docs/reference/troubleshooting/)
