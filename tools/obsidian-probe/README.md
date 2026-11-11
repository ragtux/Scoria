# Obsidian probe harness

Drives a real Obsidian over the Chrome DevTools Protocol to observe behavior
the specification needs. CDP input events are injected by the browser, so they
carry focus and take the same path a person's keyboard and mouse do — unlike
`el.dispatchEvent`, which does not and which is why the earlier
throwaway-plugin probes could not settle interaction.

Not part of `cargo test` and **not run in CI**: these scripts assert
*Obsidian's* behavior, not Scoria's. A failure means Obsidian changed or the
app flaked, which is not a reason to block a merge. Run them by hand when the
Obsidian version moves, read the diff, and update the observation tables and
`docs/scoria/Specification.md` together. The CI-side counterpart is
`crates/scoria-suggest/tests/obsidian_rows.rs`: Scoria asserted against the
tables these runs produce.

## Running a block

```
tools/obsidian-probe/run-block.sh tools/obsidian-probe/search.mjs /tmp/search-results.json
```

`run-block.sh` creates a fresh scratch vault, registers it in
`~/.config/obsidian/obsidian.json` under a fresh id, calls the script in
phases — `fixtures` (Obsidian not running: write the notes), `run` (Obsidian
up), and `after-restart` if `run` ended with `c.quit()` — and restores
`obsidian.json` byte-identical on exit. It refuses to start while an Obsidian
is already running. Never point it at a real vault: not the gate vault
(`$SCORIA_GATE_VAULT`) while a byte-level gate run is open, and
not `docs/scoria`. Scripts get `VAULT` and `RESULTS` in the environment.

With `USER_DATA_DIR=<dir>` the runner launches Obsidian with `--user-data-dir` on
that folder instead: Obsidian holds its single-instance lock per data directory,
so the probe's copy runs beside the one on the desktop, keeps its own
`obsidian.json`, IndexedDB and update there, and never touches
`~/.config/obsidian`. The newest `obsidian-*.asar` of `~/.config/obsidian` is
copied in once so the run is on the version the person runs. `CDP_PORT` picks the
port (default 9222); use another one when a desktop Obsidian may hold 9222.

A shell that exports `LD_LIBRARY_PATH` (the Claude Code session's does, for an
alsa-lib) breaks Obsidian's electron with `GLIBC_2.43' not found` before any
window opens, and the runner then reports no booted `app` twice. Run it as
`env -u LD_LIBRARY_PATH tools/obsidian-probe/run-block.sh …` from such a shell.
A script that leaves its CDP socket open at the end keeps `node` alive and the
runner waiting after its last step; end with `c.close()` or `process.exit(0)`.

Scripts, each one checklist block of `docs/scoria/Interaction probes.md`:

| Script | Block | Phases |
| --- | --- | --- |
| `search.mjs` | the search pane: sort orders, persistence, menus, toggles, keyboard | fixtures, run, after-restart |
| `search-copy-modal.mjs` | the "Copy search results" modal and its 24 format variants; supersedes `search.mjs` step 09 | fixtures, run |
| `search-shots.mjs` | the search pane's look: `Page.captureScreenshot` PNGs of the pane with results, a hovered match, a keyboard-focused row, a collapsed row, the settings section, Explain search terms, Show more context, Collapse results, Match case, the count menu and the light theme (`SHOTS` names the folder), plus the computed styles and markup of every part | fixtures, run |
| `properties.mjs` | properties in the reading view: the editing path, Add property, remove, type change, fold, propertiesInDocument, All properties | fixtures, run, after-restart |
| `canvas.mjs` | canvas write-back: what an edited save, an unedited close and `canvas:new-file` write; key order per node and edge type, the formatter's one-line rule, unknown keys, group ordering | fixtures, run |
| `splits-bookmarks.mjs` | tab groups: what "Split right" and "Split down" put in `workspace.json`, the `dimension` a dragged divider writes, the four focus commands, closing a group and its last tab, "Open link under cursor to the right", restart fidelity; and the Bookmarks plugin: every item type's bytes, each command's modal, the block id `bookmark-current-section` writes, the pane's header and rows | fixtures, run, after-restart |
| `frame-typography.mjs` | the shell's chrome in the hidden frame: the title row, its window buttons and sidebar toggles, the ribbon, every header's height, the Lucide icon each control names, the computed font sizes, weights and colours of the shell and of a note, and what dragging a sidebar's handle and toggling a sidebar write to `workspace.json` | fixtures, run |
| `frame-settings.mjs` | the ribbon's own classes and icons, the dark and light palettes as computed custom properties | fixtures, run |
| `frame-setting-key.mjs` | the "Window frame style" setting: 1.14 opens Settings in a popout window, so its rows are reached through `app.setting.activeTab.containerEl`; records the setting's tab, label, options and default, and that no option writes anything under `.obsidian` | fixtures, run |
| `explorer-header.mjs` | the Files pane's header: its five buttons and their icons, the sort menu and what each of the six orders does to the rows and to `workspace.json`, the auto-reveal toggle with a note opened in a collapsed folder either way, the Expand all / Collapse all swap, and where the header's New note and New folder put their file | fixtures, run |
| `tab-hover.mjs` | the root tab strip's hover: which box the wash paints (the inner box, not the tab), its colour, radius and inset, what hover does to the title, the close button and the dividers of the hovered tab and its neighbours, the active tab and a dock tab hovered, in both themes, with a screenshot of the strip per state (`SHOTS` names the folder) | fixtures, run |
| `tab-list.mjs` | the strip's right end: where the new-tab, tab-list and sidebar-toggle buttons sit against the tab, the divider after the last inactive tab, and the tab-list menu: its rows, icons, checks and separators, what picking a tab, Close all, Stack tabs and Bookmark N tabs do | fixtures, run |
| `new-tab.mjs` | the new-tab button and `workspace:new-tab` with the active tab in the middle of the strip: where the tab goes, whether it is active, what its empty view offers | fixtures, run |
| `explorer-keyboard.mjs` | the Files pane's keyboard: what a click on a file, on the open file and on a folder does to focus, and Up, Down, Left, Right, Enter, Space, Home, End, Escape, F2 and Delete on file and folder rows, entering the tree through `reveal-active-file` | fixtures, run |
| `outgoing-links.mjs` | the Outgoing links pane: its markup and styles, one row per target with a glyph per kind, the subtext under a row, what clicking a heading row and an unresolved row do, the row menu, the unlinked mentions card with its "Link this file" chip and the bytes the chip writes, the two fold keys in `workspace.json`; and the Backlinks pane's rows, hover-only Link button and empty state for comparison | fixtures, run, after-restart |
| `bookmarks-pane.mjs` | the Bookmarks pane's look and interactions on 1.15.0: markup and computed styles of the header and every kind of row, hover and tooltips, the active mark, each kind's right-click menu with icons, Collapse all and a group's fold, the search filter under real keys, Ctrl and middle clicks, Rename on the row, Edit bookmark, New group, drags that reorder, nest and add from Files with the drop indicator, the keyboard, the light theme and the empty state (`SHOTS` names the folder) | fixtures, run |
| `table-keys.mjs` | the Live Preview table's keyboard from inside a cell: Enter, Tab, Shift+Tab, Escape, the arrows, Shift+Enter and Ctrl+Enter, above all from the last cell of the last row, with the table at the end of the note, before a blank line and before a paragraph; where the caret and focus are afterwards; what typing does after each way out; and the re-pad a click into a cell causes | fixtures, run |
| `inline-title-and-table-entry.mjs` | the note's inline title (`.inline-title`) and which cell an arrow key entering a Live Preview table lands in: the title's markup and resting styles, hover, click, double click, Select All, Arrow Down, Tab, Enter, Escape, Arrow Right and Arrow Up from the first line, and what Enter renames; Arrow Down and Arrow Up into a table from two columns and what typing then does | fixtures, run |
| `popout-windows.mjs` | popout windows on 1.15.0: what a popout's document holds (no ribbon, docks or status bar), what `move-to-new-window`, `open-in-new-window`, `open-link-in-new-window`, `new-window` and `close-window` each do, and the `floating` split of `window` nodes `workspace.json` records with each window's bounds, read through `floatingSplit.children[i].win` from the vault window | fixtures, run |
| `gate-leftovers.mjs` | the four observations the Writer gate's week left open: Mod+B's second press over whole bullet, task and numbered lines and over one whole bullet line; the numbered toggle's second and third press over flat and nested bullets; what `theme:switch` opens, lists and writes with one theme installed; which "Update links" button is accented and where focus starts | fixtures, run |
| `table-handles.mjs` | the Live Preview table's row and column handles and "+" bars: their markup, geometry, resting and hovered opacity, what pressing one selects and draws, the bytes a drag leaves in both directions for rows and columns, the drop's before-or-after rule, and where the cursor lands (`SHOTS` names the folder) | fixtures, run |
| `view-header-bookmark.mjs` | the view header's bookmark action on a bookmarked and an unbookmarked note: label, icon, fill and colours, its place among the actions, hover, what a click opens, and whether it follows a bookmark added or removed while the note is open | fixtures, run |
| `explorer-drag-look.mjs` | the Files view with a drag held and never dropped: the dragged row, the target folder's wash and name colour, the root target, and the drag ghost's markup, text and styles, over a folder, a file in a folder, a root file, the space below the rows and with a folder dragged (`SHOTS` names the folder) | fixtures, run |
| `bookmarks-rename.mjs` | what renames, moves and a delete do to `bookmarks.json`: a note with heading and block bookmarks renamed and moved, a bookmarked folder renamed and moved with a note inside it in a group, an attachment renamed | fixtures, run |
| `dock-panes.mjs` | the right dock's Tags, All properties and Outline panes: each pane's markup, computed sizes and colours, header buttons and tooltips, the sort menu and the row order under every entry, the in-pane search filter (and how each pane treats Escape), hover, what a row click opens, the Tags pane's nested-tags toggle, counts and folds, the Outline's current-heading mark in source and reading mode and its auto-scroll toggle, the empty state, and every state key's round trip through `workspace.json` | fixtures, run, after-restart |
| `dock-menus-keys.mjs` | the same three panes, second pass: the row context menus under a real right click (a capture listener records each `contextmenu` and whether it was prevented; `getEventListeners` reads each row's handlers), the tree keyboard after making each pane the active leaf four ways, the focus ring's computed style, and the outline filter's `suggestion-highlight` in both themes. `ONLY=<regex>` runs a subset of steps. First run on Windows (2026-11-04) by hand: Obsidian started with `--user-data-dir` and `--remote-debugging-port`, `CDP_BASE` pointing at it, the phases called directly | fixtures, run |
| `editing-commands.mjs` | the editing commands the backlog waited on: the bare `set-heading`, `cycle-list-checklist`, `toggle-checklist-status`, the six insert-block commands, the highlight colours, `clear-formatting`, Move line's hotkey, the formatting toggles across a multi-line selection, `insert-table` on a non-blank line | fixtures, run |
| `editing-typing.mjs` | the same block's keystroke rows: auto-pairing's open questions (skip-over per closer, the word-boundary rule, context suppression, forward Delete, `[[`), and Shift+Tab on ragged indentation | fixtures, run |
| `footnote-insert.mjs` | `editor:insert-footnote`'s number and where the definition lands, read from disk with the fixture written before launch — one case per run (`FN_CASE`), because the command only lands once per launch when the probe reaches the file with `openFile` | fixtures, run |
| `ui-rules.mjs` | the UI rows: the empty tab, the lucide icon per callout type, the callout fold chevron, the setext underline, the done-task decoration, the view-header title, the property pills, clearing every property, `graph:open-local` with no file, the local graph's camera, a half-typed query, Ctrl+E in a field, the explorer's context menus and the modal backdrop | fixtures, run |
| `attachment-dropdown.mjs` | Settings → Files and links: the attachment-location dropdown's option labels, the string each stores, the dependent rows, and what each stored value resolves to — on its own because the Settings modal has hung this endpoint before | fixtures, run |
| `plugin-options.mjs` | Settings → the Daily notes and Templates tabs: every row's name, description, control and placeholder, the key each field writes to `daily-notes.json` / `templates.json`, the bytes of those files, and what clearing a field writes back | fixtures, run |
| `search-collation.mjs` | the basename tie in the search pane's sort: several same-basename files at different depths, under every sort order, plus the quick switcher's own order | fixtures, run |
| `code-copy.mjs` | block 6: hover, real click, clipboard read back; trailing newlines, unicode and CRLF; the icon's revert delay; Tab; the editor's `code-block-flair`; an embedded block; the frontmatter pre | fixtures, run |
| `heading-fold.mjs` | block 7: the subfolder key, what `{0,0}` is, fold-more/fold-less, source mode, `foldIndent: false`, the reading-view clicks, the edge headings, the link jump, and the cold restart | fixtures, run, after-restart |
| `explorer-dnd.mjs` | block 5: real drags through `Input.setInterceptDrags`, the drag payload, the drop-zone classes, collisions, folders, multi-select, and F2's inline rename committed with Enter | fixtures, run |
| `graph-writeback.mjs` | block 9: every control section expanded, every toggle clicked and slider dragged, the write timing polled, the zoom clamps, unknown keys, an external rewrite, and the local graph's own options | fixtures, run |
| `attachment-paste.mjs` | block 4: a real clipboard image through `nativeImage`, folder creation, `./`, Markdown links, reading view and a canvas, the disk timeline and undo depth, and a real file drop (`FIXTURES` names a directory of source files) | fixtures, run |
| `canvas-menus.mjs` | the canvas's menus for #48: the card menu, the floating row per kind of selection, the right-click list per verified target (card, Markdown and image file cards, URL card, group label and body, edge on and off its label, background, several objects), the Set color, Line direction and Align submenus, Align and Arrange applied so the geometry is recorded, the quick settings and where they persist, Read-only, and what Convert to file writes | fixtures, run |
| `raw-html-remote.mjs` | blocks 8 and 10: real anchor clicks (guarded so the frame cannot navigate), which tags survive, the iframe and CSS-background fetches, raw HTML in every container, and the remote-image layout — runs its own HTTP server (`HTTP_PORT`) | fixtures, run |
| `raw-html-live.mjs` | raw HTML in Live Preview: each inline element with the cursor away, inside, on a tag and at both edges, nesting and Markdown between tags, which HTML blocks become widgets (a `<table>` never does), and a raw table against a Markdown one in the reading view, computed cell styles included; screenshots to `SHOTS` | fixtures, run |
| `bases.mjs` | Bases: the `.base` format and its write-back — whether opening or closing writes, what one real edit writes, key order at every level, the YAML emitter's quoting and indentation, comments, trailing newline, CRLF and BOM, where a new key lands, the unpublished layout keys of all four layouts (driven through "Configure view"), `bases:new-file`, `bases:insert` and `bases:copy-table`, unknown keys at every level, the five ways a base can be broken, and an external rewrite | fixtures, run |
| `bases-ids.mjs` | Bases property ids: which of `order`, `sort`, `groupBy`, `summaries`, `columnSize`, `image` and the `properties` keys drop `note.` on save, what the loader holds in memory, whether a `note.`-keyed `columnSize` sizes a bare-named column, and the key a header drag writes | fixtures, run |
| `bases-ui.mjs` | the Bases UI and menus for look-alike parity: the toolbar per layout, the view header's More options, the view switcher with Configure view and its Default/Duplicate/Delete menu and Add view, the Properties panel with the property and formula pages, the filter panel with every operator list per type and the value editors, Sort, Group, Results, the per-layout Configure view rows, the column header's menu and submenus, the summary menus per column type, the row and cell editors and the link's file menu, the kanban columns and cards with drag and drop, the cards and list layouts, the embedded base in reading view and Live Preview, the commands and New base entry points, the keyboard in the table and the panels, and the computed styles of every surface; three passes (`s15`+ and `s24`+ re-aim what the first pass missed), every state as a PNG in `SHOTS` | fixtures, run |
| `bases-expr.mjs` | Bases: the expression language — precedence and associativity read off the parse tree (the private parser is turned into an oracle by patching `mockContext.evaluateFormula`), the lambda form of `list.filter`/`map`/`reduce`, literals and escapes, coercion, null propagation, durations, every method reachable from a value, the global functions, `this`, and an invalid statement in a filter, a formula and a summary | fixtures, run |
| `pdf.mjs` | the PDF view: where pdf.js's `PDFViewer` hangs off the leaf, the leaf's `workspace.json` record against the `pdfjs.history` localStorage entry that really carries page, zoom, rotation and sidebar, the toolbar, sidebar, find bar and Display options menu with their styles in both themes, the zoom ladder, keys, the page-number rule, `#page=`/`#height=` on links and embeds, every embed of `Embeds.md` in reading view and Live Preview, the canvas file card, menus, the three copy strings, the panes, `broken.pdf`, and restart fidelity; the `b` steps are the clean pass after the first pass polluted the history with a rotation (`SHOTS` names the folder) | fixtures, run, after-restart |
| `vault-switcher.mjs` | the left sidebar's vault profile and the switcher it opens: the footer's parts, the menu's rows against a registry holding two vaults that share a folder name and one whose folder is gone, the check on the open vault, the order the rows come in, and what the row under the separator opens; writes `vault-switcher-menu.png` to `SHOTS` | fixtures, run |
| `vault-manager.mjs` | the vault-manager window "Manage vaults..." raises (`starter.html`, a CDP target of its own with no `app` in it): the list's rows and their order against the registry on disk, the ellipsis menu, the panel on the right, and what `app:open-vault` and `app:switch-vault` each do; writes `vault-manager.png` and `vault-manager-row-menu.png` to `SHOTS` | fixtures, run |
| `code-highlight.mjs` | syntax highlighting: which engine each view uses (Prism in the reading view, CodeMirror in the editor) and what each emits per language; the cascade-winning declaration of every token span's colour in both themes; the collapsed class to `--code-*` table; which of 201 language names actually produce tokens; and, because Obsidian's `.token` rules are unreachable through `document.styleSheets`, what the renderer computes for a span of each candidate class placed inside a real block (`SHOTS` names the folder) | fixtures, run |
| `appearance-tab.mjs` | Settings -> Appearance: every row's name, description, control and markup; the Themes dropdown against `app.customCss.themes` and what picking one writes; whether a theme folder counts when its manifest names something else or is missing (`THEME_IDENTITY=1` writes those fixtures); the CSS snippets sub-page with its reload and folder buttons, what each toggle writes and in which order, and which order the snippets really cascade in; the accent picker and its "Restore default"; the font sub-page; both sections with nothing installed; and Escape on a sub-page. Read through `app.setting.activeTab.containerEl` from the vault window and clicked through the popout's own page target (`SHOTS` names the folder) | fixtures, run |
| `font-pages.mjs` | Settings -> Appearance, second visit: the three font pages clicked through (three picks, a typed name, a drag, every Delete) with `appearance.json` after each; the Quick font size adjustment toggle and Ctrl+wheel over a note, as `WheelEvent`s from the page because a CDP `mouseWheel` never returns on Windows (`QUICK_ON=1` starts with the toggle on, for `s09`, which also quits to see whether the size is saved); and Custom app icon's Choose with the native dialog answered from the renderer and every file under the vault and `UDD` (the `--user-data-dir`) diffed | fixtures, run |
| `theme-registry.mjs` | the theme registry: every custom property declared under `:root`/`body`/`.theme-dark`/`.theme-light` with its raw declaration chain and computed value, and for every surface element the cascade-winning declaration of each painted property (`var()` intact, found by walking the CSSOM) in both themes, hover states under a real pointer, the Settings rows through the popout window's document; `theme-defaults.py` writes `crates/scoria/src/theme/defaults.rs` from the run | fixtures, run |
| `theme-plugins.mjs` | the theme registry's second pass: the Bases, canvas, graph and PDF chrome. Each state (the view at rest, a hover, a selection, every Bases panel, the four layouts, the embeds, the graph's panel open and closed, the PDF's sidebar, find bar and menu) walks the whole view and reads one element per tag-and-class signature with the cascade-winning declaration of each painted property; `s01-rules` dumps every rule naming the four surfaces, for the states a walk cannot reach. Shares the CSSOM walk with `theme-registry.mjs` through `theme-lib.mjs` (`SHOTS` names the folder) | fixtures, run |
| `bases-leftovers.mjs` | the five Bases behaviours the 2026-11-01 run left unobserved: a card with no cover (an empty `.bases-cards-cover` is still drawn), the list layout's *Indent properties* (`indentProperties`) and its nested layout, the Summarize page's *Add summary* and the `summaries:` it writes, table cell selection with Ctrl+C / Ctrl+V / Ctrl+Z and the clipboard's formats, and where a dropped kanban card lands and what its drag carries (`Input.setInterceptDrags` + `Input.dispatchDragEvent`; real mouse moves never start an HTML5 drag). First run on Windows through `run-block.ps1` | fixtures, run |
| `page-preview.mjs` | page preview: the hover sources and their Mod defaults, the show and hide delays timed in the renderer, the popover's markup, box and computed styles, placement below, above and against the right edge, every kind of target (whole note, section, block, missing, image, canvas, base, external, tag), Live Preview and source mode with Mod held before, pressed during and released after, the `page-preview.json` keys flipped at runtime, a nested popover, the wheel, a link clicked, the click that turns it into an editor and what typing there writes, Escape and a click outside, and the surfaces (Files, tab header, Bookmarks, Backlinks, Outgoing links, Outline, Search); `s11` the File properties view, a base's table, a graph node (put on screen through the renderer's pan and scale), a second link above the first while its popover is up, and a click on a missing note's popover. First run on macOS, through `run-block.sh`'s Darwin branch (`SHOTS` names the folder) | fixtures, run |
| `app-window-commands.mjs` | the app and window commands Scoria waited on: Increase/Decrease font size (step, clamps, where the size is kept, `baseFontSizeAction`), Toggle light/dark mode from each `theme`, Toggle ribbon, Toggle default mode for new tabs, the three copy commands' clipboard strings, Open in default app / Show in system explorer / Show trash under each `trashOption`, Ctrl+Tab with three tabs, Show debug info, Show release notes, Open file from outside the vault, Show context menu under cursor, Focus on last note, always-on-top and zoom in the vault window and a popout, and what survives a relaunch. Every shell, dialog and `_external` call is replaced by a recorder in the renderer first (`remote.*` member calls are caught in `ipcRenderer.sendSync`). `ONLY=<regex>` picks steps, `THEME_START` the fixture's theme; `OPEN_FILE_REAL=1` with `WTYPE` lets the real GTK dialog open in a private headless compositor (its Return never landed). First run 2026-11-09 in a private headless sway on Linux | fixtures, run, after-restart |
| `attachments-app-info.mjs` | seven ids phase 4 waited on: `editor:toggle-spellcheck` (app.json over two runs from no key and from `false`), `editor:attach-file` (the file input, where the file is copied under each `attachmentFolderPath`, the text and caret, a taken name, a selection, Markdown links, a file already in the vault), `editor:download-attachments` (what is offered, the names, the rewritten links, collisions, each folder, a deselected item, a 404, Cancel, nothing to download, odd names and types), `app:migrate-frontmatter` (the modal, 45 frontmatter shapes before and after, Undo, a second run), `app:show-debug-info` (the text under two configurations, its three buttons), `app:show-release-notes` offline, `app:open-file` with a path chosen inside and outside the vault. The file input is answered over CDP (`Page.setInterceptFileChooserDialog` with the command run under `userGesture`, then `DOM.setFileInputFiles`); the main process's open dialog is answered by a stub put in through Node's inspector (`OBSIDIAN_ARGS=--inspect=<MAIN_PORT>`), which is a stub at the dialog, not GTK; images come from a local server (`MEDIA_BASE`, its log `MEDIA_LOG`). `ONLY=<regex>`, `RUN_LABEL`, `FM_SET=b` for the second frontmatter set. First run 2026-11-09 in a private headless sway | fixtures, run |
| `exports-importer-audio.mjs` | the five ids phase 4 had never watched: `workspace:export-pdf` (the modal's rows, `pdfExportSettings` after each control, the save dialog's options, the PDF's pages, sizes, margins and title, a cancelled dialog), `canvas:export-as-image` (the modal, each option's PNG and its dimensions, a selection), `markdown-importer:open` (not registered in 1.15.0), `audio-recorder:start`/`stop` (the file's name, folder and container under three `attachmentFolderPath`s, the text written at a caret and over a selection, reading view, an empty tab, a canvas, the ribbon). Needs `OBSIDIAN_ARGS="--inspect=<MAIN_PORT> --use-fake-device-for-media-stream --use-fake-ui-for-media-stream"`, `MAIN_PORT`, and `OUT_DIR` for the stubbed save dialogs; `ONLY=<regex>`. First run 2026-11-10 in a private headless sway | fixtures, run |
| `panes-workspaces-headings.mjs` | what five waiting commands need: `backlink:toggle-backlinks-in-document` (the leaf's `backlinks`/`backlinkOpts` in `workspace.json` off, on and off, the block's markup in all three modes, the menu row, `backlinkInDocument` on a new tab), `footnotes:open` (the leaf, the rows' order and numbering, the empty state, clicks, Open link, editing in the pane), the Workspaces plugin (each command's modal or suggester, `workspaces.json` whole after saves, a re-save, a load, an overwrite, save-and-load and deletes, the `workspace.json` rewrite on load), `editor:rename-heading` (availability, the modal, the bytes in the note and in ten link forms, odd names, `alwaysUpdateLinks: false`) and `markdown:add-alias` over six frontmatter shapes in all three modes. One scenario per run, `SCEN=bl\|blsetting\|fn\|ws\|rh\|al`, each on its own fresh vault and under its own key of one results file; `ONLY_STEPS=<regex>`. First run 2026-11-09 in a private headless sway session (`WAYLAND_DISPLAY` at its socket, `NIXOS_OZONE_WL=1`, `DISPLAY` unset) | fixtures, run |
| `note-composer-unique.mjs` | the commands phase 4 left waiting on a probe: `note-composer:merge-file` (Enter, Shift+Enter and Ctrl+Enter, the separator over a trailing-newline matrix, two frontmatters, the links to the source and the "Update links" prompt, each `trashOption`, the confirmation and its "Don't ask again"), `note-composer:split-file` and `note-composer:extract-heading` (the prompt, both notes' bytes per `replacementText` and link format, `newFileLocation`, the section a heading takes, the default name, templates), the Unique note creator (`zk-prefixer`, `insert-unique-link`: the three keys, names within one minute, folder, template, the inserted text), `random-note` (tab, redraws, the ribbon), both settings tabs diffed row by row, and each command's palette availability. `SCEN=settings\|merge\|split\|zk\|random2\|random1\|random0` picks the fixtures and steps (one vault per scenario), `ONLY=<regex>` a subset | fixtures, run |
| `autopair.mjs` | auto-pairing (predates the runner: start Obsidian yourself with `--remote-debugging-port=9222` on a scratch vault and run it directly) | — |

`results/` keeps the raw JSON of the runs the specification cites, named by
date. A run is evidence; the interpretation of it lives in
`docs/scoria/Specification.md`, and the two should be read together.

Most scripts take `ONLY_STEPS=s03,s04` to re-run a step or two, which is how a
late step gets fixed without paying for the whole block again.

## On macOS

`run-block.sh` runs on macOS too (2026-11-11): there it always uses a data directory of its own
(`USER_DATA_DIR`, default a fresh folder under `$TMPDIR`, its `obsidian.json` carrying
`"updateDisabled": true`), so `~/Library/Application Support/obsidian` is never written, and it
launches `/Applications/Obsidian.app/Contents/MacOS/Obsidian` (`OBSIDIAN_APP` names another
bundle) rather than a PATH `obsidian`, which on a Mac can be the command-line client. Do not run
the binary with `--version` to read the version: it starts the app. Read
`CFBundleShortVersionString` from the bundle's `Info.plist` instead.

## On Windows

`run-block.ps1` is `run-block.sh` for Windows (2026-11-04): the same phases,
always under `USER_DATA_DIR` (default a folder in `%TEMP%`), Obsidian started
from `%LOCALAPPDATA%\Programs\Obsidian` with `--remote-debugging-port=$CDP_PORT`
and killed on the way out by the PID it was started with. The data directory's
`obsidian.json` carries `"updateDisabled": true`; without it the first launch
fetched the newest release into the directory, deleted the `obsidian-*.asar`
copied in from `%APPDATA%\obsidian`, and the rest of the run was on a version the
person does not have. `CDP_TIMEOUT_MS` raises `cdp.mjs`'s 15 s limit on one
protocol call, which the theme probes' CSSOM walks overrun on a loaded machine.

```
$env:USER_DATA_DIR = "$env:TEMP\scoria-probe-udd"; $env:CDP_PORT = 9231; $env:CDP_TIMEOUT_MS = 60000
pwsh tools/obsidian-probe/run-block.ps1 tools/obsidian-probe/theme-plugins.mjs results.json
```

## cdp.mjs

No dependencies — Node 24 has a built-in `WebSocket`. `connect()` waits for a
page target whose `app.workspace.layoutReady` is true (the endpoint answers
before the vault window boots, and the first page target can be the launcher)
and returns:

| | |
| --- | --- |
| `evaluate(body)` | run a function body in the renderer, `app` in scope, returns its value |
| `typeText`, `typeChar`, `pressKey(name, {ctrl,shift,alt,meta})` | real keystrokes |
| `rect`, `clickElement`, `rightClickElement`, `dblClickElement`, `hover`, `clickAt`, `drag`, `mouseMove` | real mouse |
| `dragAndDrop`, `dragOverOnly` | a real HTML5 drag, through `Input.setInterceptDrags` plus `Input.dispatchDragEvent` — `dispatchMouseEvent` never raises `dragstart`, which is why the synthetic passes could not establish a drop. Returns the intercepted payload; `dragOverOnly` hands back a `finish(x, y)` so the hover state over a target can be read before the drop, or `dragCancel`ed |
| `on`, `once` | protocol *events* by method name, which drag interception needs |
| `openMenuItems()` | every open `.menu` as labels in order, `---` for separators |
| `selectInfo`, `selectSet` | a native `<select>`'s options, and setting it with a `change` event |
| `readClipboard`, `writeClipboard` | the system clipboard via Electron |
| `vaultPath()` | which vault this window has open — check it, never assume |
| `quit()` | close the window cleanly and wait for the process to go |

## Traps

- A vault id in `obsidian.json` must be **16 hex characters**; a malformed one
  is ignored and no vault window opens. `run-block.sh` generates one.
- **Reusing an id across folders inherits that id's IndexedDB cache**: a
  search on the new vault returned files from the old one until the re-index
  caught up. Fresh id per scratch vault.
- The version question: the window title and update log say **1.14.4**, the
  user agent says `obsidian/1.13.7`. App 1.14.4 on installer build 1.13.7.
- A previous instance lingers for seconds after its window closes. The kill
  pattern must admit the `--remote-debugging-port` flag between the binary
  and the asar, or it never matches a CDP launch; the runner waits for both
  the process and the endpoint to be gone, then checks `vaultPath()` after
  connecting and relaunches once on a mismatch.
- After a click, also ask whether a `.modal-container` appeared. "Copy search
  results" opens a modal; three scripts watched the clipboard for a write
  that never comes until the modal's own button is pressed.
- Obsidian's CSP blocks the renderer's own `fetch` to localhost; read a probe
  server's log from the host.
- `cp` is aliased to `cp -i` on this machine; the runner uses `command cp -f`.
- **Seed a note through the vault, not through the editor.** `editor.setValue`
  leaves the file and the editor disagreeing, and a command that touches the
  file reloads the editor from disk — which puts the previous case's bytes back
  underneath the command's own edit. `app.vault.modify` plus a settle, then the
  caret, then a `seedOk` check that the editor really holds the seed.
- **A second markdown leaf is poison.** `executeCommandById` acts on the
  *active* leaf; a probe reading `getLeavesOfType('markdown')[0]` then sees an
  untouched document and records every command as a no-op that still reports
  it ran. Reuse one leaf and `setActiveLeaf` it.
- A file-change event landing after `setCursor` moves the caret under the
  command. Settle before placing the caret, not after.
- Clicking an explorer row opens the file and hands focus to CodeMirror, so F2
  and Ctrl+A go to the note. Enter the tree with
  `file-explorer:reveal-active-file`.
- The reading view only builds the sections near the viewport: scroll the whole
  note and merge, or a long fixture reports a fraction of its elements.
- A control below the window (the graph's Forces sliders) reports a rect a
  drag cannot reach. `scrollIntoView` first and check the rect is on screen.
- **Escape clears the canvas selection.** Pressing it to dismiss a menu before
  opening the next one deselects everything, so every "with several selected"
  case recorded afterwards is really a single-card case. Remove the `.menu`
  nodes instead (`canvas-menus.mjs`'s `dismiss`), and read the selection back
  either side of the click.
- A menu's rows are nested in 1.14: `.menu > .menu-scroll > .menu-group >
  .menu-item[data-section]`. Walking `.menu > *` sees two wrappers and nothing
  else, and a row without a `.menu-item-title` (a colour swatch row) is
  dropped by a title-only read. Keep each menu's `outerHTML` as well, so a
  dropped row can be recovered without re-running.
- Separators come in pairs at a group boundary, one painted and one
  `display: none`; record `offsetHeight` per row or the capture reads as two
  separators in a row.
- localStorage is shared across every vault the app has opened, so filter any
  dump to the run's own app id before saving it as evidence.
- **`ONLY_STEPS` must not filter the `fixtures` phase.** `run-block.sh` makes a
  fresh vault every time, so a re-run with `ONLY_STEPS` whose `step()` guard
  also applies to fixtures starts on an *empty* vault and every open misses.
  `bases.mjs` guards it with `phase === 'run'`.
- The vault's own index lags `layoutReady` by a second or two:
  `getAbstractFileByPath` can return null for a file the fixtures phase
  certainly wrote. Poll for the file instead of throwing on the first miss.
- **The Bases table's header row is a `.bases-tr` of `.bases-td.mod-implicit` cells**
  (each holding `.bases-table-header`, `.bases-table-header-sort` and the `draggable`
  resizer), so "the first `.bases-tr` with a `.bases-td`" is the header, not a row. The
  cards and kanban layouts keep a hidden measuring template whose text is `Fj` as their
  first group/column, and the reading view keeps a zero-rect copy of each embed: filter
  by `offsetWidth`, `opacity` and the text. A click right after a DOM-removed `.menu` can
  toggle Obsidian's own menu state off instead of opening the next one; `bases-ui.mjs`'s
  `openPanel` waits for the panel and retries.
- **A second Bases leaf is poison the way a second markdown leaf is.** Every
  `.bases-toolbar-*` selector matches the *first* leaf in the document, which
  may be a stale hidden one, so the click lands in the wrong view and the step
  records nothing. `bases.mjs`'s `openBase` detaches every bases and markdown
  leaf first.
- **The Bases toolbar does not open `.menu-item` lists.** Each control opens a
  `.menu.bases-toolbar-menu` whose body is a panel — `.bases-toolbar-section`s
  of comboboxes, native `<select>`s, range sliders and `.suggestion-item` rows,
  or a `.view-config-menu` form. A probe that reads `.menu-item` titles sees an
  empty menu and reports "no such row". A combobox's options open in a
  `.suggestion-container` that is *not* inside the `.menu`.
- **`pressKey('Enter')` is a `rawKeyDown`, which raises no `keypress`.** A control that
  commits on `keypress` ignores it: the Workspaces modal's name input and the Rename heading
  textarea both stayed open with nothing written, and a probe that stops there records "Enter
  does nothing". `panes-workspaces-headings.mjs`'s `enterWithKeypress` sends a `keyDown` with
  `text: '\r'`; try the raw one first and record which took. (Suggesters, the property pills
  and CodeMirror take the raw one.)
- A command that writes the clipboard itself (`bases:copy-table`) fails with
  Obsidian's own "Unable to copy to your clipboard" notice unless the view has
  real focus: click a row first, then run the command.
- **`executeCommandById` returns `true` for a command whose check refuses**
  (`workspace:show-trash` under `trashOption: system`, the copy commands with no
  file): read availability from `app.commands.listCommands()`, which is what the
  palette lists, never from the return value.
- **A private headless sway runs the whole harness.** `WLR_BACKENDS=headless`,
  then `run-block.sh` with `WAYLAND_DISPLAY` on that compositor's socket,
  `NIXOS_OZONE_WL=1` (the nix wrapper only adds the Wayland flags with it) and
  `DISPLAY` unset, so nothing can fall back to the desktop's X server. Popouts open
  there too, and its clipboard is the compositor's, not the person's. Find the
  socket your sway took with `ss -xlp` — other agents' sways take `wayland-N` too.
  Window-manager state (always-on-top, maximize) is Electron's flag only there.
- Ctrl+= / Ctrl+- / Ctrl+0 sent with `Input.dispatchKeyEvent` left the zoom alone, in
  the vault window and in a popout, and the zoom commands have no entry in
  `hotkeyManager.defaultKeys`. Whether the keys are bound outside the renderer (an
  Electron menu accelerator) cannot be seen from CDP: run the zoom by command id.

- **An "Update links" prompt closed with its × hangs the Note composer.** A merge whose
  source has backlinks waits on that prompt (its Merge button stays `mod-loading`); a step
  that tidies up by clicking every `.modal-close-button` leaves the merge pending, and every
  later merge in the session confirms and then writes nothing. Answer the prompt with one of
  its buttons (`note-composer-unique.mjs`'s `answerUpdateLinks`).
- In a headless sway session a real click at a Settings `combobox-button`'s rect hits the
  empty tab behind the modal (`elementFromPoint` gives `.empty-state`); the button's own
  `click()` opens its `.suggestion-container.combobox`. A second file picker in the same
  session, after the first was picked, did not open at all: probe each tab in a run of its own.
- `iterateRootLeaves` right after a command missed the tab the command had just opened (the
  active leaf was not among them). Count with `getLeavesOfType` and the root's
  `.workspace-tab-header`s.
- Electron's `shell.trashItem` fails in the probe's session ("Failed to move item to trash"),
  so `trashOption: "system"` falls back to `.trash/` there: what the system trash does is not
  observable from this harness.

See [[Development#Driving Obsidian over CDP, 2026-10-18]].
