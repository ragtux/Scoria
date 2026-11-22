---
title: What differs from Obsidian
description: The honest list — everything Obsidian does that Scoria does not do yet, the places Scoria deliberately behaves differently, and what is permanently out of scope.
---

# What differs from Obsidian

Three kinds of difference, kept apart because they mean different things: **not yet**,
**deliberately different**, and **not ever**.

If you are deciding whether Scoria can be your daily editor, this is the page to read. It is
written against 0.7.0; the [release notes](/releases/) say what each version moved out of the
first list.

## Not yet

Things Obsidian does that Scoria intends to do and does not.

### Rendering

| | |
|---|---|
| **Math** | Typeset with a KaTeX-compatible engine rather than MathJax: the common syntax renders alike, a MathJax-only command shows its source in red. No `\newcommand` across formulas, no `\require` |
| **Some HTML in notes** | Inline tags, `<sup>`, `<sub>`, `<small>`, `<details>`, `<pre>`, container tags and raw tables with `colspan`, `rowspan` and `<caption>` render (see [basic syntax](/docs/format/basic/#html-in-notes)), and an inline `style` draws its colour, background, weight, slant and line. Not yet: the rest of `style` (a size, a box, `display`, a `var(--…)` colour), `<iframe>`, `<form>` and other tags Scoria has no drawing for (they show as source). A line holding a `<sup>` or `<sub>` keeps its height where Obsidian's grows by a few pixels, a `<sup>` inside a `<sup>` is drawn one level up rather than two, `<kbd>` and `<code>` written as HTML are drawn at the text's size rather than 14 px, and `<samp>` and `<tt>` are not set in monospace |
| **An image in a line of text** | An image embed that shares its paragraph with text breaks the paragraph in reading view; a remote image in a heading or a table cell stays a link |
| **Mermaid diagrams** | Drawn as Obsidian draws them, laid out by a Rust port of Pele (Obsidian's own Mermaid renderer), in the reading view, Live Preview, embeds, hover previews and Export to PDF. Text is measured in Inter's static faces rather than its variable one, so a box can come out a pixel or a few wider or narrower; a label in a script Inter lacks (日本語) is a line shorter; dashed lines print solid in a PDF; there are no clickable `click` links or tooltips |
| **Code highlighting in rarer languages** | Fenced code is highlighted by a port of Prism: 85 languages under 122 names, against the 171 names Obsidian's Prism highlights. A block in any other language renders plain |
| **PDF selection and export** | A PDF opens in a viewer with Obsidian's toolbar, sidebar and find bar, and `![[x.pdf]]` embeds it. Its text selects and copies, with Obsidian's "Copy as quote" and "Copy link to selection", and a `#page=N&selection=` link marks what it names; two-page spreads, Adapt to theme and the password prompt work in a tab. A link copied from a PDF whose text pdf.js cuts differently from Scoria can name other characters; an embedded PDF has no selection and no Display options; Adapt to theme is forgotten at quit. Export to PDF (`workspace:export-pdf`) lays the reading view out on pages of its own rather than printing it through a browser: the page breaks differ from Obsidian's, a table row or an image taller than a page is not split, callouts have no icon, and an embedded base, PDF or canvas prints as its link |
| **Video** | Scoria has no video decoder. A video file, in a tab or embedded with `![[clip.mp4]]`, is a card with its name, picture size, length, codecs and file size, and **Open in default app**, where Obsidian plays it in place |
| **The audio player** | Audio plays in a tab and embedded with Obsidian's (Chromium's) controls: play and pause, the time, a seek bar, the volume slider that opens over the volume button, and the ⋮ menu's Playback speed (0.25 to 2, the pitch kept); Space plays and pauses once the player has been clicked, and every embed is a player of its own. The controls are drawn in the theme's colours rather than Chromium's grey, and a playing embed scrolled far out of view keeps playing where Obsidian's pauses. An audio file in a tab sits in the note's readable column rather than across the whole view |

### Views and windows

| | |
|---|---|
| **Popout windows, the remaining corners** | In: the five window commands, **Open in new window** on the menus, tab drags between windows, and popouts reopened on launch. On Wayland a dragged tab stays in its own window, and a popout's recorded zoom is not applied. On X11, after the window manager un-maximizes a popout, Scoria has the window's position wrong until it is moved, so drags to and from it do nothing (a gpui bug). The tab menu has **Link with tab** and **Open linked view**. Popouts have not been tried on Windows. See [the window](/docs/app/window/#popout-windows) |
| **Stacked tabs** | `workspace:toggle-stacked-tabs` stacks a group and writes Obsidian's `"stacked": true`. Only the active pane is live: the others draw their note as a reading view, and anything else as a header over nothing. The scroll jumps where Obsidian's eases, the turned titles are always in Inter, and a spine does not drag |
| **Workspaces** | Saved layouts work (`workspaces:*`, the manage window, the ribbon button). After a layout is loaded `workspace.json` is rewritten when Scoria quits, not a second later |
| **A second vault beside the first** | Opening or switching to another vault from inside Scoria replaces the one in the window, where Obsidian opens it in a window of its own. Run Scoria again to have both open |
| **The graph and the local graph** | `graph:open-local` splits the main area with a local graph linked to the note's tab, as Obsidian does, and a local graph in the right sidebar, from a `workspace.json` that has one, follows the active file; both keep their options in `workspace.json`. Link with tab and Unlink tab work on the graph's tab as on any other; their **Open linked view** opens a linked local graph, Backlinks, Outgoing links, File properties or Outline beside the note (see [linked views](/docs/app/panes/#linked-views)), a tab cannot be dragged into the sidebar, and the sidebar pane's tab menu offers only Close, so a sidebar pane cannot be linked. The global graph is a view of the main area, and `Ctrl + G` again returns to the note. `graph:animate` brings the nodes back in creation order at a fixed 0.39 s a step, where Obsidian's steps vary, and the graph has no screenshot export. See [the graph](/docs/app/graph/#the-local-graph) |
| **Bases: cell selection** | Built: Shift + click, Shift + arrows and `Ctrl + A` select cells, `Ctrl + C` copies them, `Ctrl + V` pastes over them and Delete empties them. `Ctrl + Z` and `Ctrl + Shift + Z` undo and redo a paste, a Delete, a typed cell and a checkbox, one step each. Three differences: a paste several columns wide writes each cell its own value, where Obsidian 1.15.1 repeats one clipboard column across the row (a bug that scrambles notes, not followed); an undo puts a note's bytes back exactly, so a key or a frontmatter block the edit added goes again, where Obsidian leaves it behind empty; and the copy is plain tab-separated text, so it keeps its types (a list as a list, a number as a number) only when pasted back into Scoria, where Obsidian also puts a Markdown table and its own typed format on the clipboard. See [formats](/docs/reference/formats/#bases) |
| **Canvas export and options** | Export as image (`canvas:export-as-image`) draws the canvas itself rather than capturing the view: Scoria's mark is in the logo strip, a card's text is clipped at its edge and is drawn in full at any zoom, and an embedded canvas or base in a card shows only its name. The Canvas plugin's settings are acted on as Obsidian 1.15 acts on them: where a new canvas goes, what the wheel does, snapping to the grid and to other cards with their guides, when card names show and the zoom at which card content hides. Ctrl + drag works for every behavior but "Add web page", which does nothing (Scoria cannot make a web page card yet, so its creation menu has no such row), and Scoria's canvas has no snapping rows in its background menu or its settings button |

### Editing

| | |
|---|---|
| **Note composer** | Merge, extract selection and extract heading work, `Ctrl + Enter` included, and the template setting is Obsidian's file picker, in Obsidian's order |
| **Unique note creator** | Both commands work. **Add unique internal link** with several cursors edits only the first |
| **Slash commands** | Built. Within a query, rows that Obsidian's matcher scores equally can come in a different order (`/toggle` puts *Toggle left sidebar* first where Obsidian puts *Toggle pin*), the popup's width is estimated from its longest row, and an editing page preview has no slash popup. The list is the palette's, so it differs from Obsidian's where the palette does |
| **Renaming a heading** | `editor:rename-heading` works as Obsidian's does, oddities included. Links to the heading in frontmatter are not rewritten |
| **File recovery** | Scoria keeps its own snapshots, outside the vault under its configuration folder (`file-recovery/`); Obsidian keeps its in its own database, so neither app sees the other's. Snapshots are taken of notes and canvases on opening and on a change, once per **Snapshot interval**, and pruned by **History length** when the vault opens. `file-recovery:open` and the tab's **View** open the snapshots; **View** opens them over the main window rather than over Settings, which is a window of its own. **Clear** asks, then empties Scoria's store. **Show changes** compares a snapshot with the file now, line by line |
| **Editing properties in reading view** | In the note itself a property's value pill can be removed, and **Add alias** types a new alias into the table as Obsidian does; keys, values, the type menu and **Add property** are edited in Live Preview, or from the File properties pane, which works in reading view too |
| **Editing the title in reading view** | The inline title is an editable field in Live Preview and Source mode; in reading view it is plain text. `F2` works everywhere |
| **Vim mode** | The common core is there (modes, motions, operators, text objects, registers, search, `.`), and `:` takes a line number, `:$` and `:noh`; visual block, macros, marks, other `:` commands and `.obsidian.vimrc` are not |
| **Line numbers, indentation guides, right-to-left** | Built. On a line that mixes directions the arrow keys move by the line's direction rather than visually through each run; a link in a right-to-left paragraph of the reading view does not follow and its text cannot be selected; with **Right-to-left** on, the reading view's code blocks stay left-aligned; a wrapped list line's guide stops at its first row |
| **Spellcheck** | Checked with Hunspell dictionaries rather than Chromium's: English (United States) is built in, other languages come from the system's Hunspell folders or `dictionaries/` in Scoria's configuration folder, not from Chromium's download. Text already in a note is checked as soon as it opens; the Properties view and the inline title are not checked. The custom dictionary opens as a page of Settings rather than a modal. See [editing](/docs/app/editing/#spellcheck) |
| **Pasting and dropping** | Copied HTML, a clipboard image, files a file manager copied and files dropped on the editor all work as in Obsidian. On Linux the files and the HTML are read through `wl-paste` or `xclip`, so without those tools a paste is the plain text or the image; on macOS and Windows a paste is the plain text or the image for now. Text or HTML dragged in from another app is not dropped (only files are), and a paste or drop into a canvas or an editing page preview does nothing |

### Panes and plugins

| | |
|---|---|
| **Footnotes view** | The pane lists the note's footnotes and jumps to a definition; a footnote cannot be edited in the pane |
| **The File properties pane** | Edited, closed and reopened as in Obsidian. Its tab's right-click menu has **Close** alone, where Obsidian's also offers Pin, Link with tab, Move to new window and the splits; the other side tabs have no menu, so only File properties and Footnotes can be closed. The key column keeps the note's width, which leaves the values a narrow column in the sidebar |
| **Slides, Audio recorder, Web viewer** | `slides:start` runs, set in the interface font, with its keys live at once (Obsidian's wait for a click on the slide) and links on a slide inert; `audio-recorder:start` and `:stop` record the default microphone to `Recording YYYYMMDDHHmmss.m4a` (AAC-LC, mono) where Obsidian puts it, but the encoder runs slower than real time, so the file lands a while after Stop (about a third of the recording's length) under a **Saving recording…** notice that Obsidian does not have; it asks for 96 kbps and writes about 46–48. The ribbon button toggles, and the recording plays in Scoria's audio player like any audio file. `webviewer:*` is not implemented: Scoria has no web engine (`markdown-importer:open`, the old Format converter, is not in Obsidian 1.15.0 either) |

### Files and settings

| | |
|---|---|
| **System trash on Windows and macOS** | Implemented on Linux (freedesktop trash with a `.trashinfo` record); elsewhere it falls back to the vault's `.trash/` |
| **Settings Scoria does not act on yet** | The rows write what Obsidian writes, bar Override config folder (Spellcheck languages, which Obsidian keeps outside the vault, goes to `scoria.json`), and the option tabs of plugins Scoria does not have, which show their values only. Rows for behavior Scoria lacks are written for Obsidian and marked "not in Scoria yet": Language, Native menus, and the plugin options above. **Custom app icon** takes effect on X11 only. See [Settings](/docs/settings/) |
| **URI links from a browser on macOS** | A URI given on the command line works on all three platforms; a `scoria://` or `obsidian://` link clicked in a browser does not reach Scoria on macOS yet |

### Commands

The command palette lists only what Scoria can run. Beyond the ones named above, these
Obsidian commands are not implemented yet:

| | |
|---|---|
| **Window** | The zoom is kept in Scoria's own folder (`window/<vault id>.json` beside `scoria.json`), not in Obsidian's data folder, so the two apps do not share it. At a zoom other than 0 an input method's composition window opens off the caret by the zoom's factor. A popout's recorded zoom is kept in `workspace.json` but not applied, and a reload does not give a popout the vault window's zoom as Obsidian's does. *Toggle window always on top* works on X11, macOS and Windows; on Wayland, which has no way for a window to ask, it says so in a notice. The View and Window menus are not drawn: Ctrl+=, Ctrl+- and Ctrl+0 zoom, and the vault window cannot be kept on top |
| **App** | *Reload app without saving* reads the vault, its config, theme, snippets, hotkeys and core plugins again and puts the layout back, dropping the open note's unsaved edits, carets, scroll and history as Obsidian's does; a canvas or a base with a save pending is written first, which Obsidian was not watched doing. Frontmatter migration converts and re-serialises a note's block byte for byte as Obsidian did, and its Undo lasts as long as its notice. The text size the two font-size commands, Ctrl+wheel and the Font size slider set is kept across launches as Obsidian keeps it, but in Scoria's own configuration folder beside the window zoom rather than in Obsidian's local storage, so a vault opened in both apps keeps a size in each; like Obsidian's, none of them writes `baseFontSize`, and the file's size is only read until a size is kept. Scoria's slider has no Restore default. Open file from outside the vault opens a file inside the vault in the current tab, as Obsidian does, but refuses one outside it, which Obsidian opens in a tab of its own and saves back to. Show debug info reports Scoria's version and settings in Obsidian's layout, leaving out the lines Scoria has nothing for. Show release notes shows Scoria's notes for the running version, taken from this site's releases page when the app was built, where Obsidian downloads its changelog |

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

**A canvas minimap draws edges that record no side.** Obsidian 1.15.0 draws no picture at all
for an embedded canvas that holds an edge without a `fromSide` or `toSide` — the embed is its
name alone. Scoria draws the edge from the side facing the other card, the way the canvas
view does.

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
