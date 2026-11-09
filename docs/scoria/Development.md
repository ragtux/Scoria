---
tags:
  - scoria/dev
---
# Development

## Environment

The reference development machine runs NixOS on Wayland with an NVIDIA GPU. The GPUI app links libxcb and libxkbcommon and loads Wayland, Vulkan, and GL at run time, so run cargo commands that touch the `scoria` crate inside `nix-shell` (see `shell.nix` at the repository root).

>[!warning] Pinned dependencies
> `gpui` and `gpui_platform` are git dependencies pinned to a Zed release commit; the crates.io release crashes on NVIDIA. `libc` is pinned in `Cargo.lock` because an old `xattr` in gpui's tree needs a constant newer libc removed. Both pins are explained in the root `Cargo.toml`.

## Commands

```
cargo test --workspace
cargo run --release -p scoria -- <vault>                      # Files, reading view, Backlinks/Outline/Tags
cargo run --release -p scoria -- <vault> --note "Name#Heading" # open a note, scrolled to a heading or ^block
cargo run --release -p scoria -- <vault> --search "tag:#x OR y"  # open with the Search tab filled (for screenshots)
cargo run --release -p scoria -- <vault> --switcher "spc"        # open with the quick switcher up (for screenshots)
cargo run --release -p scoria -- <vault> --pane outgoing          # right dock tab: backlinks, outgoing, tags, properties, outline
cargo run --release -p scoria -- <vault> --graph [local]          # open on the graph view
cargo run --release -p scoria -- <vault> --palette "open"         # open with the command palette up
cargo run --release -p scoria -- <vault> --settings hotkeys       # open Settings on a tab (general, appearance, editor, ...)
cargo run --release -p scoria -- <vault> --note X --source        # open a note in Source mode (--reading for reading view; the default is the vault's defaultViewMode)
cargo run --release -p scoria -- <vault> --note X --stress 40     # self-driven edits on that note, then autosave (use a scratch vault)
cargo run --release -p scoria -- <vault> --note X --source --type '[[al'          # type at the cursor: opens the autocompletion popup for a screenshot
cargo run --release -p scoria -- <vault> --note X --source --type '[[al' --accept down,enter   # and press the popup's keys (enter, tab, shift-enter, up, down)
cargo run --release -p scoria -- <vault> --note X --source --command editor:table-row-after   # run command ids at startup, comma separated (use a scratch vault)
cargo run --release -p scoria -- <vault> --note X --cell 2 --type new --press enter   # open a table cell, type in it, press keys (enter, tab, shift-tab, escape, up, down)
cargo run --release -p scoria -- <vault> --note X --template Daily   # open the template chooser and pick what the query matches
cargo run --release -p scoria -- <vault> --note X --find word     # open with the find panel up and the query matched
cargo run --release -p scoria -- <vault> --note X --prompt rename  # open the rename prompt (move, delete, folder) for screenshots
cargo run --release -p scoria -- <vault> --note X --rename "New"   # rename the note at startup, rewriting links (use a scratch vault)
cargo run --release -p scoria -- <vault> --control /run/user/1000/scoria.sock   # the JSON control socket, see the section below (not with --stress)
cargo run --release -p scoria -- <vault> --frame native            # the desktop's own title bar; the default is Obsidian's hidden frame (SCORIA_FRAME=native does the same)
cargo run --release -p scoria -- <vault> --control <sock> --size 1400x900       # window size for a reproducible screenshot
cargo run --release -p scoria-bench -- generate <dir>
cargo run --release -p scoria-bench -- index <vault>
```

`scoria-mcp` speaks the control protocol as an MCP server over stdio, and
launches the app itself. Register it in `.mcp.json`, with an absolute repo path
since `SCORIA_MCP_BIN` runs through `sh -c` when it contains a space:

```json
{
  "mcpServers": {
    "scoria": {
      "command": "/path/to/scoria/target/release/scoria-mcp",
      "env": {
        "SCORIA_MCP_REPO": "/path/to/scoria",
        "SCORIA_MCP_BIN": "nix-shell --run \"target/release/scoria\"",
        "SCORIA_MCP_SOCKET": "/run/user/1000/scoria-mcp.sock",
        "SCORIA_MCP_SCREENSHOT": "spectacle -b -n -a -o {path}",
        "SCORIA_MCP_TIMEOUT_MS": "15000"
      }
    }
  }
}
```

In the app: click a file or a link to open it, Ctrl+P opens the command
palette, Ctrl+E toggles reading view and editing (Live Preview unless
`livePreview` is off), `editor:toggle-source` from the palette switches Live
Preview and Source mode, Ctrl+F and Ctrl+H find and replace in the note,
Ctrl+Alt+Left and Right walk the tab's history,
Ctrl+O opens the quick switcher, Ctrl+Shift+F the Search tab, Ctrl+G the
graph (again to return), Ctrl+, opens Settings, Ctrl+S saves (edits also save
on their own two seconds after the last keystroke), Ctrl+N creates a note,
F2 renames the open one, Ctrl+Q quits. A note opens in the vault's
`defaultViewMode` (editing unless it says `preview`). Every shortcut except
Ctrl+Q is an Obsidian command bound from its default hotkey, so the vault's
`hotkeys.json` rebinds or unbinds it. Right-click a file or folder in Files
for new note, new folder, rename, move, copy and delete; clicking a link to a
note that does not exist, or pressing Enter in the switcher on a name nothing
matches, creates it. In the graph: wheel zooms, drag pans, drag a node to
move it, click a node to open it, Ctrl+0 refits the view.

## Releases

`.github/workflows/release.yml` and `.forgejo/workflows/release.yml` both fire on a `v*` tag. Cut a
release by bumping `version` under `[workspace.package]` in the root `Cargo.toml`, committing that,
then tagging and pushing to both remotes:

```
git tag -a v0.1.0 -m "Scoria 0.1.0"
git push origin v0.1.0 && git push forgejo v0.1.0
```

Either workflow refuses to build when the tag and the workspace version disagree, since the archive
is named from the tag and the binary reports the manifest version. GitHub builds
`x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` — one archive per
target, plus `SHA256SUMS` over all of them — and publishes with `gh release create`. The homelab
Forgejo runner has only the Linux container (see [[#Environment]] and `ci.yml`), so it
builds the x86_64 Linux archive and creates the release through the instance API with the token
Forgejo Actions injects for the run. Release notes on both are the commit subjects since the
previous tag. A tag with a hyphen in it, `v0.2.0-rc1`, is published as a prerelease.

Both builds pass `--locked`, so the pinned `gpui` revision and `libc` come from `Cargo.lock`.

Both workflows sign `SHA256SUMS` with GPG when the repository holds the maintainer's key as the
secrets `GPG_SIGNING_KEY` and `GPG_PASSPHRASE`; without them the release is published unsigned and
its notes say so. The key is `69BF AA44 7DAC E6C3 EF02  6CBA 4EA6 7A61 88DA 92BD`, `ragtux
<ragtux@outlook.com>`, the same ed25519 key that signs the commits here, published on
keys.openpgp.org and named in the README.

What the forges hold is **only its signing subkey**, exported with
`gpg --armor --export-secret-subkeys <fpr>` rather than `--export-secret-keys`: the certify key
stays on the development machine, so a stolen CI secret can sign a release but cannot make new
subkeys, certify another identity, or revoke anything. The primary key is certify-only to begin
with, so the workflow's `--local-user <primary fpr>` resolves to the signing subkey on its own and
needs no change. Set the pair on GitHub with
`gpg --armor --export-secret-subkeys <fpr> | gh secret set GPG_SIGNING_KEY` and
`gh secret set GPG_PASSPHRASE`, and on Forgejo by `PUT`ting `{"data": "<value>"}` to
`/api/v1/repos/ragtux/scoria/actions/secrets/<name>` with the token in
`~/.config/forgejo/token`. Rotating the subkey is the whole recovery procedure if a forge is ever
compromised, and it invalidates nothing already signed. The GitHub workflow also attaches
[build provenance attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations)
to every archive, which need no secret: `gh attestation verify <archive> --repo ragtux/scoria`
confirms the archive came out of this workflow at that commit. Private vulnerability reporting is
enabled on the GitHub repository, and `SECURITY.md` at the root is the policy.

> [!warning] The macOS binary is untested, and Windows is only smoke-tested
> CI runs `cargo check -p scoria` on both and the release job is the first thing that links them.
> Nothing is code-signed or notarized, so macOS Gatekeeper and Windows SmartScreen will both
> object. Linux x86_64 is the build that runs on the reference machine; see
> [[#Windows smoke test, 2026-10-20]] and [[#Windows smoke test, 2026-10-22]] for what the Windows one was
> seen to do.

### Windows smoke test, 2026-10-20

Cut with 0.2.0, on `w11`: Windows 11 Pro (build 26200), rustc 1.98.1 MSVC with the Visual Studio
2022 Build Tools and the cmake they bundle, an NVIDIA GeForce RTX 3060 beside the virtual display
adapter. `cargo build --release --locked -p scoria` compiles the pinned `gpui` revision clean in
3m18s, with no warning of ours; `gpui_windows` builds in place of the Wayland and X11 platform
crates. The binary opens `docs/scoria` and draws the shell: ribbon, file list, tabs, the
properties table, and the reading view with headings, wikilinks and lists. Nothing is written to
stderr.

One thing to know before trusting it: only the opening frame was checked. There is no control
socket on Windows — it is a Unix domain socket, and `--control` says so and carries on — so the
keyboard, editing and saving paths have not been driven there the way the Linux ones are. (Both
were put right two days later: [[#Windows smoke test, 2026-10-22]] and
[[#The control socket on Windows, 2026-10-22]].)

The window wore the desktop's own title bar on the day; see
[[#The hidden frame on Windows, 2026-10-20]] for the fix.

The app has to be started in the interactive session to draw anything: over SSH it lands in
session 0, where there is no desktop. A scheduled task created with `/ru <user> /it` runs it in
the logged-on session, and the same trick takes the screenshot. Point the task at
`powershell.exe -WindowStyle Hidden -File <script>` rather than a `.bat`, or its console window
stands in front of whatever is being photographed.

Beware the coordinates: the test machine is a 3840x2160 display at 150%, and a DPI-unaware caller
like `powershell.exe` is handed the 2560x1440 virtual desktop by `GetWindowRect` and `SetCursorPos`
while `CopyFromScreen` copies real pixels into a bitmap of the virtual size. A window centred by
Windows' own numbers therefore lands right of centre in the screenshot, and is clipped at its
right edge. The app itself is scaled correctly; only the measuring tape is confused.

### The hidden frame on Windows, 2026-10-20

The caption came back because `TitlebarOptions::appears_transparent` was left at its default.
`window_decorations` is the Linux question — Wayland and X11 answer whether the compositor handed
the frame over, and `Decorations::Client` is what the workspace looks for — but Windows has no
implementation of it at all and answers `Server` whatever it does. What the Windows backend reads
is `appears_transparent`: with it set, `WM_NCCALCSIZE` keeps the client area's top edge and the
title bar is gone, while the system keeps the drop shadow and the resize border outside the
window. So the hidden frame now asks for both, and `frameless` asks the platform the question it
can answer: the compositor on Linux, and on Windows what was asked for.

Drawing the row is only half of it. `start_window_move` and `show_window_menu` are no-ops on
Windows; dragging, double-click-to-maximise and the right-click window menu all come from the hit
test instead, by naming the empty part of the title row `WindowControlArea::Drag` so `WM_NCHITTEST`
answers `HTCAPTION`. The three window buttons name themselves `Min`, `Max` and `Close` the same
way and carry no click handler there: gpui performs the action on the non-client mouse-up, and
Windows 11 opens its snap layouts when the pointer rests on a button that answers `HTMAXBUTTON`,
which a click handler could never offer. Each button also needs `occlude`, or the caption hitbox
underneath wins the hit test and pressing a button drags the window.

Verified on `w11` by clicking the buttons through `SetCursorPos` and `mouse_event` from the
logged-on session: the window minimised, then maximised to the full work area, and the title row
holds the three buttons with no caption above it.

### Windows smoke test, 2026-10-22

Cut from `main` at bb888c1 (0.3.0) on `w11`, the same toolchain as above. A cold
`cargo build --release --locked -p scoria` took 14m50s this time, with no warning of ours. The
keyboard, editing and saving paths the 2026-10-20 run left untouched were driven from the logged-on
session with a PowerShell script: `SendKeys` for the keyboard, `SetCursorPos` and `mouse_event` for
the mouse, `CopyFromScreen` for the screenshots, and the script set per-monitor-v2 DPI aware with
`SetProcessDpiAwarenessContext` so `GetWindowRect` and the bitmap agree in real pixels — which is
the fix for the measuring-tape confusion above. The vault was a copy of `docs/scoria`, at 1400x900
logical and 150%.

Seen to work: the hidden frame with its three buttons, maximise to the full work area (3840x2088)
and restore; Ctrl+E between live preview and reading view; typing in live preview, which rendered a
wikilink and bold as typed and autosaved with the "saved" notice, keeping an LF file LF; a CRLF
file written from PowerShell kept CRLF, including the line Scoria inserted; Ctrl+O, Ctrl+P with its
hotkey labels, Ctrl+Shift+F with a click on a result opening the note at its line, a canvas with
cards and an edge, and the Settings modal on Ctrl+,; the watcher, which listed a note created
outside the app and reloaded an open tab whose file was replaced outside; Alt+F4, which exited
with nothing on stderr.

`cargo test --workspace` had never been run on Windows and did not compile: the workspace tests
imported the Unix `symlink`, and the registry round trip's fixture used `/v/two`, which Windows
does not count as absolute and so resolved against the current drive. Both are test-side and fixed
on the day; the symlink half of `vault_paths_cannot_leave_the_vault` skips on Windows without
Developer Mode (`CreateSymbolicLink` wants a privilege, error 1314). All 620 tests pass there now.

The control socket was ported the same day, see [[#The control socket on Windows, 2026-10-22]];
the PowerShell driver was a one-off in the session's scratch folder, and `scoria-mcp` now does its
job there.

### The control socket on Windows, 2026-10-22

Windows has had `AF_UNIX` since Windows 10 1803; what it lacks is the standard library's wrapper,
which only exists under `std::os::unix`. `uds_windows` is that wrapper over WinSock with std's own
API, and it was already in `Cargo.lock`. `scoria-control` now has a `net` module that re-exports
`UnixListener` and `UnixStream` from std or from `uds_windows`, and the app's `control.rs`, the
client and the MCP server's test fakes all take the two types from there, so nothing else about
the protocol changed: `--control <path>` is still a path, `bind` still refuses an existing file,
a stale one is still unlinked first, and a live one is still left alone.

Three things WinSock does differently, each caught by the client's own tests the first time they
ran there:

- Connecting to a socket file that does not exist is `ConnectionRefused`, not `NotFound`. The
  client hands back whichever it got; `scoria-mcp` already read both as "nothing is listening".
- A duplicated handle (`try_clone` is `WSADuplicateSocket`) keeps a receive timeout of its own.
  The client set the timeout on the writer and read from the clone, so `call_timeout` never fired
  on Windows; it now sets it on the handle it reads from, which on Unix is the same socket anyway.
- Reading from a connection whose peer hung up can fail with `ConnectionReset` instead of ending
  the stream. The app's server loop already returned on any read error; the test fakes now do too.

There are no mode bits to set on the socket, so the `0o600` step is Unix-only and the folder's
ACL is the whole protection: `scoria-mcp` keeps the socket in `%TEMP%\scoria\`, and the user's
temp directory is already theirs alone. `sun_path` is 108 bytes there as well, so the path has to
stay short.

In `scoria-mcp`, the shell form of `SCORIA_MCP_BIN` and the screenshot command run through
`powershell.exe -NoProfile -NonInteractive -Command`. There is no `exec`, so the pid the server
holds is the shell's, which waits for the app and ends with it; `quit` goes over the socket before
any kill, and a plain `.exe` path is the better setting on Windows. The default binary is
`target\release\scoria.exe`; the default screenshot is a PowerShell capture of the foreground
window inside DWM's visible frame (the process made per-monitor DPI aware first, so the rectangle
and the pixels agree); and `canonicalize`'s verbatim `\\?\C:\` answer is stripped before the vault
path reaches the app, or the app would title the window with it and write it into the vault
registry as a second entry for the same folder.

Driven on `w11` by piping a batch of JSON-RPC lines into `scoria-mcp.exe`: `scoria_launch` on a
copy of `docs/scoria` (the socket bound under `%TEMP%\scoria`, the index loaded), `scoria_state`,
`scoria_keys` with `ctrl-o`, `scoria_type` with `spec` landing in the switcher's input,
`scoria_screenshot` returning the window's pixels with the switcher open, and `scoria_quit`
exiting 0 and removing the socket file. The repository's `.mcp.json` wraps the binary in
`nix-shell` for the NixOS development machine, which Windows cannot run. Claude Code's local scope
outranks the project file, so on Windows the server is registered there instead, with no
environment at all so the `.exe` defaults apply:
`claude mcp add --scope local scoria -- Z:\repos\scoria\target\release\scoria-mcp.exe`.

## Reader (phase 2)

Started 2026-10-16, after the three spike gates passed. Gate: a daily user can
browse their real vault with no missing content. Passed 2026-10-16, see
[[#Reader gate, 2026-10-16]].

| Reader item | Status |
| --- | --- |
| Open a vault | done: scanned and indexed on a background thread, `.obsidian` config read |
| Files view | done: folder tree, folders first, collapse state, auto-reveal, attachments shown, unknown kinds hidden |
| Reading view | done for text content; see the list below |
| Backlinks | done: linked mentions with the source line, unlinked mentions (names and aliases in plain text outside links and code) |
| Outgoing links | done: resolved and unresolved links of the note, plus unlinked mentions of other notes in it |
| File watching | done: changed, added, removed and moved files re-index in place, links re-resolve, panes and search refresh, an open note reloads keeping its scroll position |
| Outline | done: click scrolls to the heading |
| Tags | done: vault-wide counts merged across inline and frontmatter tags |
| Search | done: full query grammar with Obsidian's explain labels, results grouped per file with context, click opens at the line, the header's four controls, the six sort orders and Copy search results, see [[#Search pane, 2026-10-22]] |
| Quick switcher | done: fuzzy over names, aliases and paths, recent files first, unresolved targets shown as not created; Enter on one creates the note |
| Graph | done: global and local graphs, filters, color groups, forces and display settings from `graph.json`; zoom, pan, drag, hover, click opens |
| Command palette | done: Obsidian's command names and hotkeys, pinned commands from `command-palette.json`, this session's recent commands first; lists only the commands Scoria can run |
| Settings, read-only | done: General, Appearance, Interface, Editor, Files and links, Hotkeys, Core plugins, Community plugins and the Quick switcher and Command palette plugin tabs, every value from `.obsidian/`; nothing is written |

## Writer (phase 3)

Started 2026-10-17. Gate: the vault round-trips byte-identical after a week
of mixed use in both apps. The gate run opened on 2026-10-18, see
[[#The gate run opens, 2026-10-18]], and the week itself starts 2026-11-04 —
the sheet, the log and the day's procedure are [[Writer gate run]], see
[[#The gate week starts, 2026-11-04]].

| Writer item | Status |
| --- | --- |
| Source mode | done, see [[#Source editor, 2026-10-16]] |
| Live Preview | done, see [[#Live Preview, 2026-10-16]] |
| File operations and link rewriting | done, see [[#File operations, 2026-10-17]] |
| Tables (editing) | done: the sixteen commands and editable cells, see [[#Table editing, 2026-10-18]]; the cell keyboard, the inline title, the row and column handles and the "+" bars, to the 2026-11-08 probes, see [[#Live Preview tables, driven, 2026-11-08]] |
| Properties | done, see [[#Properties, 2026-10-18]] |
| Autocompletion | links, headings, aliases, embeds, tags and block references, see [[#Autocompletion, 2026-10-18]] and [[#Block references, 2026-10-18]] |
| Templates | done: the three commands and the chooser, see [[#Templates, 2026-10-18]] |
| Daily notes | done: today's note, the template and stepping between them, see [[#Daily notes, 2026-10-18]] |
| File recovery | out of phase 3: Obsidian keeps its snapshots in IndexedDB outside the vault, so it cannot affect the gate |
| Writing settings back | done for every control Obsidian itself offers as one, see [[#Settings writing, 2026-10-18]], [[#Settings dropdowns, 2026-10-18]] and [[#Settings sliders and text fields, 2026-10-18]] |
| Round-trip harness | done, see [[#Round-trip harness, 2026-10-18]] |
| Canvas writer | done: the bytes Obsidian writes on an edited save, see [[#Canvas writer, 2026-10-19]]; the canvas view saves through it |
| Canvas view | done: a `.canvas` opens in the main area, pans, zooms, selects, moves, resizes, draws edges, colours, groups, copies, pastes and undoes, cards are typed into, the menus, controls and commands are Obsidian's, and it autosaves in Obsidian's bytes, see [[#Canvas view, 2026-10-21]] and [[#Canvas view, measured, 2026-10-21]] |
| Tab groups | done: Split right, Split down, the four focus commands, draggable dividers and `workspace.json`'s `main` tree, see [[#Tab groups, 2026-10-20]]; tabs drag between groups and onto split edges, see [[#Dragging tabs, 2026-10-21]]; the strip's hover is Obsidian's inner box, see [[#Tab hover, 2026-10-22]] |
| Bookmarks | done: the pane, the seven commands and `bookmarks.json` in Obsidian's bytes, see [[#Bookmarks, 2026-10-20]] |
| Files header | done: New note, New folder, the six sort orders, Auto-reveal current file and Expand all / Collapse all, see [[#Files header, 2026-10-21]]; the tree takes the keyboard, see [[#Explorer keyboard, 2026-10-21]], and a row renames on the row rather than in a dialog, see [[#The file tree renames on the row, 2026-11-05]] |
| Folding | done: headings and list parents fold in Live Preview and source mode, headings collapse in the reading view, the five `editor:*fold*` commands run, and the record lives outside the vault, see [[#Folding, 2026-10-26]] |
| Editing commands | done: the insert-block commands, the checklist cycle, Mod+L on a line without a box, the six highlight colours, Clear formatting, per-line inline toggles, the one auto-pair rule and Shift+Tab's column arithmetic, all to the 2026-10-25 probes, see [[#The settled editing commands, 2026-10-26]] |
| The gate itself | open: the harness passes on the real vault (sixteen properties, green 2026-11-05), and the week of mixed use is running on the gate vault with `docs/scoria` watched beside it — days 1, 2 and 3 all pass: day 2 caught an inverted `alwaysUpdateLinks` default and proved the two apps' link rewriters byte-identical, and day 3 brought the first `types.json` in Obsidian's bytes, which showed Scoria's writer leaving out the three seeded defaults (fixed, with Move row up hidden in the first body row as Obsidian 1.15.0 hides it), see [[Writer gate run#Day 3, 2026-11-08]]. Day 4 passes too (2026-11-09): all six suggesters, templates and daily notes in both apps, and the harness has its first `templates.json` and `daily-notes.json` — written by Scoria, rewritten by Obsidian the same night with Scoria's bytes kept, see [[Writer gate run#Day 4, 2026-11-09]]. Day 5 passes (2026-11-09): both canvases over the socket, and Obsidian 1.15.0 kept Scoria's edited-save key order byte for byte, so the canvas writer is a fixed point of Obsidian's; the one divergence was the view's, a dragged card now rises to the end of the array as Obsidian's does, see [[#Day 5, a dragged card rises, 2026-11-09]]. The prompt day 2's fix made reachable is now Obsidian's three buttons, and its "Always update" gives the harness its first path to an `app.json` holding `alwaysUpdateLinks`, see [[#The third button on the rename prompt, 2026-11-05]]. The sheet and the log are [[Writer gate run]], see [[#The gate week starts, 2026-11-04]] and [[#Day 2, files and links, 2026-11-05]] |

## Parity (phase 4)

Started 2026-10-21 with the canvas. Gate: the core plugins a vault actually
uses behave as Obsidian's do.

| Parity item | Status |
| --- | --- |
| Canvas | done: the view and the writer, see [[#Canvas view, 2026-10-21]] and [[#Canvas writer, 2026-10-19]] |
| Bases: the format | done: parsed, modelled and written back in Obsidian's own bytes, `scoria-formats::base` |
| Bases: the expression language | done: the parser and the evaluator, held to 277 parse trees and 1044 evaluated statements, see [[#Bases evaluator, 2026-10-25]] |
| Bases: the table view | done, read-only: a `.base` opens on its table, see [[#Bases table view, 2026-10-25]]; the ribbon's button creates one |
| Bases: the rest of the view | done: the cards, list and kanban layouts, the toolbar's six panels, editing through the table, the five `bases:*` commands beyond `new-file`, and an embedded `![[x.base]]`, see [[#Bases: the rest of the view, 2026-10-25]]; the toolbar and panels rebuilt to the 2026-11-01 record, see [[#The Bases toolbar and its panels, 2026-11-01]]; the embed as the live view and the leaf's chrome, see [[#The embedded base and the base leaf's chrome, 2026-11-01]]; the header, summaries, cells and keyboard, see [[#The column header, summaries, cells and the table keyboard, 2026-11-01]]; the kanban's drags and menus, see [[#The kanban board, 2026-11-02]] |
| PDF | done: a `.pdf` opens in a viewer with Obsidian's toolbar, sidebar and find bar, `![[x.pdf]]` embeds and canvas file cards draw the same viewer, see [[#PDF view, 2026-10-26]] |
| Math | done: `$x$` and `$$x$$` typeset on a KaTeX-compatible engine and painted natively in reading view, Live Preview and the math block widget, see [[#Math, 2026-10-26]]; MathJax-only syntax shows as source in red |
| The themes subset, URI and CLI | the URI landed 2026-11-02 (see [[#Four small entries and a URI, 2026-11-02]]); the theme registry, the CSS loader, the Appearance tab's pickers and syntax highlighting all landed 2026-11-02, and the Bases, canvas, graph and PDF chrome was repointed to its own tokens 2026-11-04, see [[#The theme registry, 2026-11-02]], [[#The plugin chrome repointed, 2026-11-04]], [[#The Appearance tab's pickers, 2026-11-02]], [[#Syntax highlighting, 2026-11-02]] and [[Theming#Progress]] — the subset is done, and highlighting was widened to 85 languages on 2026-11-04, see [[#Syntax highlighting breadth, 2026-11-04]]; the command line and the desktop entry landed 2026-11-02, see [[#The command line, 2026-11-02]]; the rest of the Appearance tab — the three font pages, "Quick font size adjustment" and "Custom app icon" — landed 2026-11-04, see [[#The font pages, quick font size and the app icon, 2026-11-04]] |

### Day 5, a dragged card rises, 2026-11-09

The Writer gate's canvas day, [[Writer gate run#Day 5, 2026-11-09]]. The Scoria half ran every
operation on both of the gate vault's canvases over the control socket and undid each to
byte-identical, and the first edit to the canvas Obsidian had rewritten on day 1 moved all of its
node lines into the edited-save key order the specification records — the rule's prediction, and a
28-line bet on Obsidian 1.15.0 still following a 1.14.4 probe. It does: one card moved by hand in
Obsidian came back with the other 28 lines byte for byte. Scoria's canvas writer is a fixed point of
Obsidian's.

The same save showed the one thing the API probe could not: a card dragged by hand goes to the end
of the nodes array. Obsidian's drag lifts the card to the top of the z-order, and the array is the
z-order. Scoria's view had `Scene::raise_selection` and never called it, so its drags left the line
in place. Now the first movement of a drag past the threshold raises the selection, inside the
move's `begin`/`commit`, so one undo restores the order with the position and a card already on
top records no step (`crates/scoria/src/canvas/view.rs`, `scoria-canvas` test
`a_raise_inside_a_move_is_one_undo_step_with_it`). Whether selecting without dragging raises too
is unprobed; the specification's write-behavior section carries the observation and that gap.

### Live Preview tables, driven, 2026-11-08

Three reports from the desk — Enter at the end of a table would not end it and typing then stopped
working; the note's title could not be edited; the caret vanished when arrowing down through a table —
turned into three probes against Obsidian 1.15.0 (`table-keys.mjs`, `inline-title-and-table-entry.mjs`,
`table-handles.mjs`), three records in [[Specification]], and the evening's five commits. The backlog
holds the per-entry account under [[UI parity backlog#The table cell keyboard, against the probe of 2026-11-08]];
this is the shape of it and what the drive at the end settled.

**The cell field lets go.** The stuck typing was a `CellEdit` that nothing closed when the caret left its
table, so every key went to a cell nobody could see. A cell now closes on undo, redo, a table command, a
click off the grid and any movement through the document; Arrow Up and Down walk the cells and leave above
the header or below the last row; Enter in the last row and Tab in the last cell append a row; Escape
writes the cell rather than discarding it, because Obsidian's typed text is already in the note.

**The inline title is the editor's field** (`title_edit`): a click edits it, Arrow Up from the first line
enters it selected, Enter hands a changed name to `rename_with_links`, Tab, Arrow Down and Escape return to
the document. An arrow key into a table opens its first cell, which is where Obsidian puts the caret.

**The handles and the "+" bars.** Grips above each column and left of each row, hidden until their strip is
hovered; a press selects the line, a drag moves it with the accent bar marking the edge it would land
beside, before or after by the pointer's half (`scoria_text::table::move_line`); 16 px strips under the last
row and past the last column append a line (`table::append_line`) and open its first cell. The commit
messages carry the mechanics: the bar follows the pointer out of the table, clamped to the nearest edge
line, and the editor's own mouse-up makes the move, so a release anywhere drops the line.

**The drag was driven after all.** Both records said the control socket could not make one; the
Bookmarks entry of the day before had already used `mouse` with a `button` on a `move`, which is exactly
a gpui drag. It ran in a **private headless sway session** rather than on the desktop:
`nix-shell -p sway grim`, `WLR_BACKENDS=headless WLR_RENDERER=gles2` with a 2400×1600 output at scale 2,
Scoria launched *inside the repo's own `nix-shell`* against that `WAYLAND_DISPLAY` (launched from the
sway shell it panicked with `NoWaylandLib`), `grim -o HEADLESS-1` for the screenshots, and the eight
drags the probe recorded replayed from the screenshot's geometry — nothing on the desktop touched or
captured, which is the same arrangement `tools/readme-recording` uses. All eight gave the probe's
orders (1 7 4, 4 1 7, 4 1 7, 1 7 4; A C B, B A C, B A C, A C B); the cursor landed in the moved row's
first cell (`6:2` for the row dropped last, the probe's own position) or the moved column's header cell;
the drop re-padded every cell to the delimiter's three characters, as the probe's bytes are padded;
autosave wrote the same bytes; and Escape then Undo returned the buffer and the file to byte-identical
each time. The screenshots show the grip appearing beside the hovered row alone, and while held the
handle filled in the accent with white dots, the row outlined and tinted, and the 4 px bar drawn under row 7,
where the pointer's lower half said the row would land.

Two traps for the next headless drive. A Unix socket path is at most 108 bytes, and the session's
scratch directory is longer than that: sway's `Unable to bind IPC socket` and the app's missing socket
were both that, and `/run/user/<uid>` is the place for them. And a screenshot of a scaled output is
physical pixels: window-local logical is the image coordinate divided by the scale, less the window's
position, which the first attempt got wrong by a factor and aimed every drag at the sidebar. The window
reports `active: false` in that session and takes every mouse and key event regardless.

**Still out:** the re-pad on *entering* a cell, which is a byte question (Obsidian rewrites the table on a
click into it) and wants its own decision before the gate week closes; the table commands, which act on
the buffer's caret and so do nothing for an open cell; the reading view's inline title; and what the title
does when it loses focus, which the probe did not ask.

### The Bookmarks pane, against Obsidian 1.15.0, 2026-11-07

The pane was the last dock pane never measured: text glyphs for icons, 24 px rows cut with an
ellipsis, a heading-style header, a one-item menu. `tools/obsidian-probe/bookmarks-pane.mjs` drove a
real 1.15.0 over it under its own `--user-data-dir`, and the pane is rebuilt to the record
([[Specification#Observed bookmarks pane, Obsidian 1.15.0]]).

**One tree for every dock pane.** `nav_button`, `pane_header`, `tree_row` and the filter field moved
out of `workspace.rs` into `crates/scoria/src/tree.rs`, generic over the host view, so the Bookmarks
pane — its own entity — draws with the code the Tags, All properties and Outline panes use, and a
measurement taken on one moves them all. The workspace keeps one-line wrappers.

**What it does now.** Lucide icons per kind (five fetched from Lucide: `bookmark-plus`, `folder`,
`globe-2`, `toy-brick`, `wrench`); rows that wrap; bookmarks of missing files not drawn; the active
mark on the open note's own row; "Collapse all" turning into "Expand all"; the search filter with
the groups above a match, its state in the leaf's `showSearch`/`searchQuery`; the tree keyboard;
Ctrl and middle clicks to a new tab; each kind's menu (a `Target::Bookmark` in `file_menu.rs`,
pinned by a test against the capture) with the row ringed while it is up; Rename on the row through
`inline_field`; New group as an "Untitled group" named on its row; a group's "Bookmark the active
tab..." landing in that group; drags that reorder or drop into a group, with the drop bar and the
accent fill. The model's moves (`get`, `update_at`, `insert_at`, `move_item`, `set_title`) are in
`scoria-formats::bookmarks` with their own tests.

**Every menu changed a little.** Obsidian writes two separators before a danger item and hides the
second; Scoria drew both. `file_menu::render` now draws one line per break, while `drawn` still
reports both, as the DOM has them.

**Checked over the control socket** on the probe's fixture vault: the pane against the
screenshot, a row's menu, Rename writing `title` after `path`, New group's bytes
(`type, ctime, items: [], title`), a click elsewhere committing a name, the filter's `del`, Escape
hiding it, the ring under the arrows, Collapse all. **Not driven:** a drag, since the socket has no
drag; it rests on the unit tests of `move_item`, `drop_zone` and `insertion_path`.

**The same day, the two bookmark modals and the header action.** `PromptSpec` gained `rows`:
a prompt drawn as Obsidian's settings-style modal — a fixed field, the text field and a dropdown,
each a `setting-item`, and a danger button alone at the left — measured off the probe's DOM. "Add
bookmark" and "Edit bookmark" use it, the dropdown listing the groups depth first, indented as
Obsidian indents them; a group's own "Bookmark the active tab..." preselects that group, and Save
on "Edit bookmark" moves the item when the group changed. Two traps, both found by driving it: the
dropdown's list hangs below the modal, so a pick there was a press outside the modal and cancelled
it; and the list is drawn deferred, so a pick took the keyboard away and Enter did nothing. A
second probe, `view-header-bookmark.mjs`, settled the header action: "Bookmarked", filled in the
accent, only while the file has a bookmark, and a click opens "Edit bookmark" — built with a
filled cut of Lucide's `bookmark`.

**Then the Files view's drags**, which closed that last item. A third probe,
`explorer-drag-look.mjs`, measured what the 2026-10-25 pass had only named: the dragged row's
accent fill, the target folder's whole block washed with its name in the accent, no wash at the
root, and the ghost — icon and name, then `Move into “Dest”` — 5 px off the pointer. The explorer's
rows are now a drag source carrying a `FileDrag`; the target is the folder under the pointer (a
file row's parent, the root below the rows), and a drop moves through the same rename path as
"Move file to...", so links follow `alwaysUpdateLinks` and the "Update links" question, and a
clash takes `available_path`'s ` 1`. A drop on the folder the item is in, or a folder on itself or
below itself, does nothing (`explorer::drop_moves`). The Bookmarks pane takes the same payload,
and its dock tab opens under a held drag, since Scoria shows one left pane at a time.

**Driven, not only tested.** The control socket's `mouse` method makes real gpui drags (a move
that names the held button), so `Gamma` → `sub`, a Files row through the Bookmarks tab to above
Alpha, `Image.png` into "Group one", and `Alpha` onto a `sub` that already has an `Alpha.md`
(arriving as `sub/Alpha 1.md`, the other untouched) all ran in the app with the bytes checked.
Once in that session a click and `file-explorer:open` were both ignored until a stray mouse-up
went in — consistent with a drag gpui still held — and five attempts to repeat the sequence never
did; noted, not explained.

**Bookmarks follow renames.** The clash drop showed the bookmarks of `Alpha.md` left pointing at
nothing — true of every rename Scoria made, not only drags. `bookmarks-rename.mjs` settled what
Obsidian writes: each `path` rewritten in place, folders taking their contents, a delete leaving
the bookmark. `Bookmarks::rename_path` does that from `finish_rename`, and only writes when a
bookmark is touched.


### The file tree renames on the row, 2026-11-05

The other gap day 2 filed
([[UI parity backlog#The explorer renames in a modal where Obsidian edits the row in place]]).
Obsidian names a file on the row in the tree; Scoria opened a dialog in the middle of the window.
F2 on a focused row and the menu's "Rename..." now both turn the row's own label into a field
with the name selected, and no dialog appears on either path.

**One implementation, not a second one.** Obsidian has two inline renames — the view header's
`.view-header-title` and the tree's `.nav-file-title-content` — and they behave alike, so the
header's machinery was lifted into `inline_field.rs` and the header was moved onto it in the same
change rather than the tree getting a copy. The module holds the shaping and painting (the text as
a shaped line, with the selection and the caret drawn into it), the press that lands the caret at
the offset the pointer hit, the platform input method's half so composition and dead keys work,
and the editing keys — the same `text_input` actions under the same key context, so a field edited
in place takes the keys a field in a box does. A host says only where its field is and what
committing means: `Host::editing`, `Host::commit`, `Host::cancel`, three methods each for the
workspace and the explorer. `title_field.rs`, the text model, is unchanged and still free of GPUI.
The extraction is what makes the tree's field *proven* rather than new, because the code it runs
is the code the header has been running since 2026-10-29.

**What the tree's field does, each line of it measured.** The 2026-10-25 pass is the record
([[Specification#The 2026-10-25 pass: real drags, and the inline rename committed]]), and every
row of its table was driven in the app afterwards on a scratch vault:

| | Obsidian | Scoria |
| --- | --- | --- |
| F2 on a focused row | the label becomes the editor, the **basename** selected (`Beta`, `cover` — never the extension) | the same |
| typing | the name changes on the row, the file keeps its old one | the same |
| Enter on a free name | renamed, editor closes | renamed, editor closes, then the Update links prompt where links move |
| Enter on a `.png` | `cover` → `renamed cover` gives `renamed cover.png`, the extension kept unasked | the same |
| Enter on a taken name | **refused silently**: tree unchanged, no notice, no modal, the editor stays open | the same |
| Enter on an empty name | the same | the same |
| Escape | the editor closes, the name is as it was, the row keeps its ring | the same |
| the menu's "Rename..." | the same editor, name selected, no modal | the same |
| a press inside the field | — | the caret lands where the pointer did, as the header's does |

**The silent refusal cost a split in the vault.** Obsidian's answer to a name the filesystem will
not take is to *leave the field open* and say nothing — the probe typed `Alpha` over an existing
`Dest/Alpha.md` and then an empty name, and both times the tree was unchanged, `notices` was empty
and the editor was still up with the typed text. That is not this crate's usual habit, and the row
is the right place for it: the field is where the answer is, so there is nowhere a notice would
belong. But it only works if the refusal is known *before* the field closes, and Scoria's
collision check lived inside `Vault::rename`, which cannot be asked without doing the rename. So
`Vault::check_rename` is now the validation half on its own — the name, the source, a destination
already taken, a folder moved into itself — and `rename` calls it, which is what keeps the two
answers from drifting. `check_rename_answers_for_rename_without_touching_anything` pins both
halves: every refusal, the three allowed cases (a free name, a case-only rename, a rename onto
itself), and that asking moves no byte.

The first attempt closed the field and raised the Update links prompt on a colliding name, which
the app caught and no test would have.

**A field is painted, so it needs a height.** The first build put the shaped line in the row and
drew nothing at all: a `canvas` has no size of its own, and the row's label had no height to give
it. The state dump was right the whole time — `renaming: { text: "Beta", selection: [0, 4] }` —
which is the argument for the control socket seeing what the painting sees.

**The keyboard, and the ways a field has to go away.** An open field takes the keyboard under the
`TextInput` context, so the tree's own keys are off while it is up and Enter and Escape reach the
field; `MenuFocus`, the "focus while open, restore after" helper a menu uses, puts the keyboard
back on the tree afterwards. Four things close a field besides Enter and Escape: a press on
another row, the context menu opening, the tree being focused or revealed from outside, and the
row leaving the tree. The last is the one that matters — a field whose row no longer exists is
painted by nothing and would hold the keyboard with nothing on screen.

**Still as it was:** the view header's menu "Rename..." and `workspace:edit-file-title` keep the
centre prompt, and so does the Files header's New note. What Obsidian does on those is either
unprobed or different in its own way, which is filed as
[[UI parity backlog#The Files header's New folder asks for a name before it creates anything]] —
and correcting that is how this entry's own claim about New note turned out to be wrong.

### The third button on the rename prompt, 2026-11-05

The gap day 2 filed and argued for closing inside the week rather than after it
([[UI parity backlog#The "Update links" prompt is two buttons and Scoria's own wording]]).
"Always update" is the only control a person reaches in either app that writes
`alwaysUpdateLinks: true`, so until it existed the gate's harness had no way to get an `app.json`
holding the key — which is the week's second job, the same reason days 4 and 7 go looking for
`templates.json`, `daily-notes.json`, `switcher.json`, `types.json` and `hotkeys.json`.

**The modal is now Obsidian's, string for string.** Title **Update links**, two lines that name
neither the file nor the new name — "Do you want to update internal links that link to this
file?" and "This will affect 3 links in 2 files." — and three buttons in Obsidian's order with no
Cancel: **Always update**, **Just once**, **Do not update**. The record is the drag pass
(`tools/obsidian-probe/results/2026-10-25-explorer-dnd.json`, step `s05`), whose counts were both
1, so the singular is what was observed and the plural is the inference — Obsidian writing "1 link
in 1 file" rather than "1 link(s)" is what says it pluralises at all.

**`PromptSpec` grew a row rather than a third label.** `confirm` and `secondary` name buttons but
not their order, and the default row puts Cancel between them, so a field for a third button could
not have produced Obsidian's three-in-a-row-with-no-Cancel. `buttons: Vec<PromptButton>` is the
whole row when a prompt states one — each a `{ label, action, cta }`, where the action is one of
the four things the default row already did — and empty, which every other prompt leaves it, is
that default row built from `confirm` and `secondary` exactly as before. `detail` became a `Vec`
in the same change, because Obsidian's two lines are two lines. The row is computed in one place
(`Prompt::buttons`) that both the painting and the control socket's state dump read, so a driver
can now see `["Always update", "Just once", "Do not update"]` instead of aiming at pixels.

**Escape is "Do not update", which the entry had not noticed.** Obsidian has no Cancel because by
the time its modal is up the file has already been renamed — the drag pass dismissed it with
Escape and the linking file stayed byte-identical *while the move stood*. Scoria asked first and
renamed after, so Escape abandoned the whole rename: a fourth answer Obsidian does not have.
Escape and a click outside now finish the rename without rewriting. Scoria still asks before it
renames rather than after, which is not observable — all four answers leave the same bytes — and
is left that way because the ask-path already validates the new name before anything moves.

**Always update writes the setting before it renames, and a failed write takes the rename with
it.** The order matters for exactly the reason this button exists: a rename that rewrote links
while `app.json` still said to ask is the divergence the day-2 fix closed, so if the config write
cannot reach disk the prompt stays up with the error on it and nothing moves.

**Driven in the app on a scratch vault**, never the gate vault, all four answers against a vault
whose `app.json` held only `trashOption` and whose `Two.md` had no trailing newline:

| Answer | The file | The 3 links in 2 files | `app.json` |
| --- | --- | --- | --- |
| Always update | renamed | rewritten | `alwaysUpdateLinks: true`, `trashOption` kept |
| Just once | renamed | rewritten | untouched, byte for byte |
| Do not update | renamed | untouched, same sha256 | untouched |
| Escape | renamed | untouched, same sha256 | untouched |

and the rename after **Always update** asked nothing and reported "Updated 3 links in 2 files.",
which is the written key being read back.

Two unit tests, both checked against deliberate breakage rather than trusted for being green:
`the_update_links_prompt_is_obsidians_three_buttons_and_two_lines` pins the strings, the order and
which button is accented (swapping the plural's arms and dropping the `cta` each fail it), and
`always_update_writes_the_key_and_the_next_rename_reads_it` pins the config write — the key true
in the file, the key already there surviving, and `update_links_automatically` reading it without
a reload.

**Still unprobed:** which of the three buttons Obsidian gives `mod-cta`. The probe recorded labels
and not classes. Enter therefore keeps doing what it did before the third button arrived —
updating once — and "Just once" is the accented one.

### Day 2, files and links, 2026-11-05

The sheet's day 2 in Scoria — rename, move, delete, a note created from an unresolved link, the
Files header's New note and New folder, two sort orders — with the run, the evidence and what is
still owed in [[Writer gate run#Day 2, 2026-11-05]]. The writer came out of it clean on every item;
what the day caught was a default, and the default had been hiding a feature.

**`alwaysUpdateLinks` read an absent key as on; Obsidian reads it as off.** The gate vault's `app.json`
holds only `vimMode`, so the rename of `Note F` — 6 inbound links in 6 files, two
aliased — rewrote all six **silently**. Obsidian would have raised its **Update links** modal and
written nothing but the rename unless the person chose a button. The evidence was already in the
repository: Obsidian's row reads "Turn off to be prompted to update links after renaming a file",
and it rendered that toggle **off** on a vault whose `app.json` the probe wrote as `{}`
(`tools/obsidian-probe/results/2026-10-25-ui-rules.json`) — the gate vault's situation exactly. The
2026-10-25 drag pass, which set the key to `false` deliberately, saw the modal on every basename
change.

[[Specification]] had said both things at once: its file-operations line gave the default as on
while its own Files and links table gave the toggle as off. The table is the probe's, so the line
was wrong, and `CLAUDE.md`'s rule applies in the mild form — the spec disagreed with itself, and the
observation decided it.

**Two defaults, and dead code came alive.** `Workspace::update_links_automatically` now reads an
absent key as `false` and `ALWAYS_UPDATE_LINKS`'s row default matches it. Nothing else needed
changing, which is the part worth recording: `rename_with_links` has carried the whole ask-path
since file operations landed — it previews the rename, skips the question when no link would move,
and opens an "Update links?" prompt otherwise — and **one inverted default had made all of it
unreachable on every vault that has never written the key**, which is every vault nobody has
changed the setting in. Rebuilt, the inverse rename raised the prompt for the first time on a real
vault, and confirming it restored the six files byte-for-byte.
`a_vault_with_no_always_update_links_key_asks_before_rewriting` pins the default, asserting first
that the fixture does not write the key — a default test that writes the value it is testing
asserts nothing — and then that something is actually there to ask about, so the `links == 0`
shortcut cannot pass it vacuously.

The prompt's *shape* is not Obsidian's: two buttons where Obsidian has three, Scoria's own sentence,
and the file name spelled with its extension. Filed as
[[UI parity backlog#The "Update links" prompt is two buttons and Scoria's own wording]]. The missing
third button is **Always update**, which writes `alwaysUpdateLinks: true`; `PromptSpec` has
`confirm` and `secondary` only, so it needs a field. Worth building inside the week, because that
button is the only path by which either app writes the key, and the week's second job is to leave
the harness a real copy of the config files nothing has ever written.

**What the day proved about the writer, beyond the sheet.** Three properties the harness cannot
reach, each now shown on a real vault:

- **The rename is its own inverse.** Six links rewritten and rewritten back left every one of the
  six files byte-identical to `snapshot-2026-11-04b`. Both aliases survived both directions, and the
  renamed note's own bytes — its `# Note F` heading included — never moved.
- **The link rewriter preserves a missing trailing newline.** `Note B.md` and
  `Note A.md`, two of the five notes Obsidian left without a final newline on day 1, each had a
  link rewritten and each still ends without one. Day 1 proved this of the editor; the rewriter is
  a different write path, reading and writing the file without the editor in it at all.
- **A move and a delete rewrite nothing.** The move, because `shortest` plus a unique basename
  leaves `[[Note C]]` resolving from anywhere — `rename_preview("B.md", "Sub/B.md")` is
  `(0, 0)` in the unit tests and now on a real vault too. The delete, because Obsidian rewrites no
  links on one: the linking note came back with the same sha256, and its link reported as uncreated.

**A claim withdrawn before it was filed.** The "Move file to…" chooser offered no vault-root row
with the file at the root, which read as a missing row; with the file inside `Archive` it offers `/`.
It excludes the folder the file is already in. Whether Obsidian does the same is unprobed, and it is
recorded as a question rather than a gap.

**The Obsidian half ran by hand, and day 2 passes.** It could not be driven from here:
`tools/obsidian-probe/README.md` forbids pointing the harness at the gate vault while a byte-level
run is open, and the rule is right — a probe script in the deciding vault would give it a third
author, which is the one thing that vault is chosen for. Two of its four answers were a person's
eyes, and one of those two mattered more than any byte in the diff.

**The two link rewriters are byte-identical on a real vault**, which is the result only Obsidian
could give. Obsidian rewrote the same 6 linking files in one batch at `00:06:02.899`–`.906` and left
`Note F.md` itself untouched at day 1's mtime — a rename's signature, since the rename is
a directory operation and the links live elsewhere — and all six then verified OK against
`<vault>-2026-11-04b.sha256`. Scoria had already renamed away and back to the same bytes, so both
writers are exact inverses of themselves *and* agree with each other, aliases included.

**The modal appeared, so the default is settled on this vault and not merely inferred.** That was
the fix's premise, and `app.json` corroborates the button: it still holds only `vimMode`, so the
answer was Just once rather than Always update, which writes the key. The methodological point is
worth keeping: **the bytes could not have settled this.** "Default off, prompted, answered Just
once" and "default on, rewrote silently" leave identical bytes — the same `app.json`, the same six
rewritten files. A gate that reads only diffs would have confirmed the inverted default. The day's
central finding was reachable only because someone watched the screen, which is the argument for the
mixed-use half of this gate existing at all.

**One gap filed from the other half.** Obsidian named the new note by editing the row's own label in
the file tree; Scoria opens a centre prompt for both F2 and the header's New note. Filed as
[[UI parity backlog#The explorer renames in a modal where Obsidian edits the row in place]] — and it
is wiring rather than new behaviour, since `title_field.rs` is already the `contentEditable` text
model for the view header's inline rename and was deliberately kept free of GPUI to be reused.

`app.json`, `appearance.json` and `core-plugins.json` were all rewritten with identical content when
a settings pane was opened — the documented trap, mtimes moved and no bytes — so they are attributed
and dismissed. `snapshot-2026-11-05` closes the day.

Worth saying once, because the note itself says it: **days 1 and 2 ran on the same calendar night**,
half an hour apart. The week is a week of operations so far, not of elapsed time.

### The gate week starts, 2026-11-04

The gate's other half — a week of a person using both apps on the same
folder — is set up and running, and [[Writer gate run]] holds the sheet, the
traps and the log. Three things had to be settled first.

**The harness still passes.** Fifteen properties over the corpus and over
The gate vault, green in 3.6 s with everything recompiled. Worth re-running before
starting the week rather than trusting the 2026-10-18 result: the theme
registry, 85 languages of highlighting and the whole Bases run landed in
between, and a week of mixed use spent on a writer that had regressed would
be a week wasted.

**The October blocker is gone, and the clean diff means nothing.** The vault
was registered in Obsidian on 2026-10-22 12:50 — which is what wrote its
`app.json`, `appearance.json` and `core-plugins.json` — so "the vault is not
registered in Obsidian yet" no longer holds. `diff -r snapshot-2026-10-25`
is clean, but every file in it was last touched on 2026-10-22 or earlier, so
that is the diff of an inert folder. **A clean diff over a vault nobody
touched is zero evidence, not a pass**, and saying otherwise is the one way
this gate can be passed without being met.

The registration bought something real, though: the gate vault's `.obsidian` now
holds four modelled files, and the config property's per-set
`checked > before` assertion means they genuinely round-tripped — where
[[#Four small entries and a URI, 2026-11-02]] recorded that set as passing
vacuously on a `.obsidian` that held only `workspace.json`.

**Two vaults, and only one decides.** The verdict rests on the gate vault because
only Scoria and Obsidian ever write to it, so a moved byte has exactly two
suspects — which is the question a diff has to answer. `docs/scoria` is the
folder actually lived in (edited today; Obsidian rewrote its `.obsidian` on
2026-10-30 and 2026-11-02), so it is watched for free through git, but a byte
that moves there has a third possible author in the repo's own tooling and
cannot settle anything. The thinness of the deciding vault is the price of
attribution, and it is why the week is driven by a sheet covering all twenty
Writer items rather than by organic use: ten notes used organically would
touch three of them.

`tools/writer-gate/gate.sh` is the procedure, wrapped as `just gate-check`,
`just gate-baseline` and `just gate-status`. It keeps the layout
[[#The gate run opens, 2026-10-18]] established — copies in
`$SCORIA_GATE_DIR`, not hashes alone, because a hash says a file
moved and the next question is always which bytes — and adds the daily shape:
check against yesterday's snapshot, attribute whatever moved, then take
today's. `check --from-first` gives the cumulative diff. `snapshot-2026-11-04`
is the baseline, 17 files, its manifest verifying against the vault.

Beyond the diff, the week has a second job. `templates.json`,
`daily-notes.json`, `switcher.json`, `hotkeys.json` and `types.json` are
modelled and written but ratcheted nowhere, because Obsidian creates none of
them until a setting in that plugin is written and no set on this machine has
one. Days 4 and 7 write exactly those settings, so the week should leave the
harness its first real copy of each.

One trap found in the setup and recorded in the note: this vault's `app.json`
has `"vimMode": true`. Scoria models the setting and writes it back, but
nothing in the editor acts on it — there is no modal editing anywhere in
`scoria-text`. Obsidian's editor is modal this week and Scoria's is not, so
the same keystrokes will not produce the same text. It stays as it is: it is
the user's real setting, and the gate is about their real vault.

### Syntax highlighting breadth, 2026-11-04

[[UI parity backlog#Syntax highlighting covers 85 languages where Obsidian covers 171 names]],
from eleven languages to 85, under 122 names with Prism's aliases.

**The grammars are written out by a script, not by hand.** Prism builds most
of its languages at load time: TypeScript is `extend('javascript')`, C++ is
`extend('c')` with keys spliced in by `insertBefore`, `js-extras` reaches
into JavaScript after it has loaded, and `jsdoc` adds a `doc-comment` to it.
The grammar a language *ends up* with is only known once everything has
loaded, and the key order — which is load-bearing — is the order those
operations leave. Re-deriving that chain by hand for fifty languages is
where a port goes wrong, so `tools/prism-port/port.js` loads Prism, walks the
finished grammar objects and writes each one out as the crate's `static`
data, key for key. A grammar object two languages share is written once, in
the first one's file, and a language reaching another's top level (markup's
`<script>` holds JavaScript) refers to it. The script also translates the
regexes from JavaScript's syntax to `fancy-regex`'s: `/…/i` to `(?i)`, `\/`
and the other identity escapes to the bare character (Rust reads `\<` as a
word boundary), `[`, `&` and `~` escaped inside a class, a bare `{` that is
no quantifier escaped, `[^]` and `[]`, `\uXXXX`, and the one pair of UTF-16
surrogates Prism uses for "an astral character", which Rust has no
surrogates to name. All 3,654 patterns compile.

**The load order had to be Obsidian's, and the corpus settled it.** Loaded
through Prism's own `loadLanguages()`, Prism reproduced every Obsidian sample
but one: JavaScript's `regex-source` came out with its pattern tokenized,
where Obsidian's `ab+c` is a bare string. That is what loading `regex`
before `javascript` does, and the loader re-loads a language when one of its
optional dependencies arrives. Loading `prism.js` (markup, css, clike,
javascript) first, every other component through the loader, and `regex`
last on its own reproduces all twenty Obsidian samples byte for byte, so
the script loads it that way. Two finds from the walk: `xml` is not an alias
of `markup` but a clone taken before CSS and JavaScript gave markup its
`<style>` and `<script>`, so it is its own language (with `ssml`, `atom` and
`rss`); and loading `cilkc` turns `c` itself into Cilk C, so `cilk-c` and
`cilkc` are aliases of `c`, as they are in Obsidian.

**A second corpus, from Prism itself.** The Obsidian corpus has twenty
samples. `port.js` also renders a longer sample per ported language from
`tools/prism-port/samples/` through the same Prism and writes
`tests/corpus/prism-1.29.0.json`; since that Prism matches Obsidian on all
twenty of Obsidian's, it stands in for Obsidian on the rest. Every
language in the table must have a sample there
(`every_ported_language_has_a_prism_sample`), and all 85 match byte for
byte. It caught the hand-ported Nix, which was not Prism 1.29's grammar —
`rec` was a keyword and `-` was not an operator — so Nix is generated now
too. The other hand ports (clike, css, diff, go, ini, json, python, rust,
toml, yaml) matched as written and stay hand-written.

**Three hooks are ported, because they change colour.** `hooks.rs` holds
`tokenize_language`, which is `Prism.highlight` without the stringify, and
both views paint from it. JSX's `after-tokenize` turns the text between a
tag's open and close into one `plain-text` token, so `<p>if "x"</p>` does
not paint `if` as a keyword. Markdown's highlights a fenced block's body in
the fence's language (Prism does it in its `wrap` hook; here it happens on
the tokens, so the painter sees the runs). PHP's, through
`markup-templating`, treats code holding a `<?` as markup with PHP islands:
each `<?…?>` is swapped for a placeholder, the rest tokenized as markup and
the islands put back as `php` tokens — and since the engine's nodes are
ranges of the text it was given, the markup's ranges are mapped back from
the placeholder text onto the source. Markup's `wrap` hook, an entity's
`title`, is reproduced in `to_html` for the corpus only. Not ported:
GraphQL's, which adds aliases no rule styles, and `js-templates`', which
highlights a tagged template's body (`` html`…` ``, `` css`…` ``) in its
tag's language — Scoria paints that body uncoloured, filed in the backlog.

**What came in.** The list asked for — TypeScript, JSX, TSX, C, C++, C#,
Java, Kotlin, Scala, Swift, Ruby, PHP, Lua, R, Haskell, SQL, GraphQL,
Markdown, LaTeX, PowerShell, Batch, Makefile, Dockerfile — with what they
stand on (JavaScript, markup, Bash), and, since the script makes one more
language a sample and a line, the next tier by what notes hold: shell
sessions, Objective-C, Dart, Perl, Julia, Elixir, Erlang, Clojure, OCaml,
F#, Zig, SCSS, Sass, Less, regex, JSON5, Protobuf, HCL, nginx, Apache,
properties, systemd, git, CSV, Vim, Groovy, MATLAB, Arduino, CMake,
CoffeeScript, Crystal, D, Elm, Fortran, GLSL, Lisp, NASM, Nim, Pascal,
Prolog, Racket, Scheme, Solidity, Tcl, VB.NET, Verilog, VHDL and WebAssembly.
84 of the 171 names the Obsidian probe saw highlighted now resolve. The
generated files are some 17,000 lines, mostly because JSX and TSX are
clones of markup with JavaScript inside; they cost nothing until a fence
names them, since a grammar's regexes compile on first use.

### Dock pane keyboard and the outline highlight, 2026-11-04

Probed first, on Windows this time: `tools/obsidian-probe/dock-menus-keys.mjs`
drove Obsidian 1.14.5 with its own `--user-data-dir` on port 9235 (raw run in
`tools/obsidian-probe/results/2026-11-04-dock-menus-keys.json`, two screenshots
beside it), and the findings are
[[Specification#Observed dock pane menus and keyboard, Obsidian 1.14.5]] and the
highlight paragraph of the outline section. The 2026-10-22 run had seen no menu
on a tag or outline row and left open whether its right click had even arrived;
this one moved the pointer first, recorded every `contextmenu` on the document in
the capture phase with whether it was default-prevented, and read
`getEventListeners` up each row's ancestors. The answer is plain: those rows
have no `contextmenu` listener, the event arrived and nobody took it, so there
is no menu to build. The property row, the control, did raise its menu, and
showed something new: on `tags`, `aliases` and `cssclasses` Rename and Delete
property are `is-disabled`. The keyboard was never focus on an element --
`document.activeElement` stays `body` -- but the active leaf's scope: once the
pane is the active leaf (its dock tab, its empty space), Down and Up move a
`has-focus` ring, starting at the first row or the last, stopping at the ends;
Left folds an open parent or climbs to the parent; Right only opens a folded
row; Enter and Space are the row's click; everything else is not the pane's.
The highlight is one rule, `.suggestion-highlight { font-weight: bold }`.

Built: `panes::tree_key` is the keyboard as a function of the visible rows
(`NavRow`: depth, children, folded) and the ringed index, returning a
`NavMove`, unit-tested on the probe's tags tree. The workspace has one
`dock_tree_focus` handle for the three lists, which carry the `DockTree` key
context and its five actions (`dock_tree::TreeUp`, `TreeDown`, `TreeLeft`,
`TreeRight`, `TreeActivate` on enter and space); a press anywhere in a list, or
a click on the pane's dock tab, takes the keyboard, and a row's own click then
hands it on (the search box, the editor) as before. Each pane remembers its
ringed row by its own key -- the tag's lowercased path, the property's name,
the heading's index, which goes with the note as the folds do -- and the ring,
a 2 px `--background-modifier-border-focus` border laid 2 px outside the row
with a 10 px radius so nothing moves, is drawn only while the handle holds
focus, as Obsidian draws it only in the `mod-active` leaf. (It was a
zero-blur spread `BoxShadow` first, Obsidian's own shape, and on this machine
that painted nothing at all; the PDF find bar's miss ring is drawn the same
way and is worth a look on Windows.) The tags and properties lists gained scroll handles so the
ring stays in view. `keyboard_is_parked` treats the handle as parked once its
pane is off screen. The outline's matches are bold in the row's own colour
rather than the accent, and the property menu disables Rename and Delete on the
three reserved keys. The dump's `tags_options`, `properties_options` and
`outline_options` gained `focused`, and `right` gained `tree_keyboard`.

Seen working over the control socket on a debug build, on a copy of the
probe's vault: the Tags tab's click gives the tree the keyboard (`DockTree` on
the focus stack), Down twice rings `common` then `project`, Left folds
`project` and Right opens it, Up stops on `common`, Enter searches
`tag:#project` with the search box focused; on All properties Up from nothing
rings the last key (`status`), Space searches `["status"]`; on the Outline
Left climbs from `Third level under second` to `Second level`, a second Left
folds it, Right opens it, Enter and a row click both put the cursor on the
heading and the editor in focus. The ring was checked on a screenshot.

Not built, on purpose: Escape, Home, End, F2 and Delete in the panes, which
Obsidian leaves alone (F2 reaching the note title is the global hotkey, which
Scoria already has).

### The font pages, quick font size and the app icon, 2026-11-04

The last of the Appearance tab. [[#The Appearance tab's pickers, 2026-11-02]] left three things in
[[UI parity backlog]]: the font rows were text fields where Obsidian's are pages, and two rows were
missing because nobody had seen what they write.

**The probe.** `tools/obsidian-probe/font-pages.mjs`, run on the Windows machine with a
`--user-data-dir` and a CDP port of its own so the person's Obsidian stayed open beside it. It
clicks through a font page — three picks, a typed name, a drag, every Delete — and diffs
`appearance.json` after each; toggles Quick font size adjustment and wheels a note with Ctrl held;
and presses Choose on Custom app icon, with the native dialog answered from inside the renderer
(`@electron/remote`'s `dialog.showOpenDialogSync` patched to hand back a PNG the probe drew), while
every file under the vault *and* the user-data folder is diffed. The record is
[[Specification#Observed font pages, quick font size and app icon, Obsidian 1.14.5]]. Two harness
lessons for the next Windows run: a popout behind another window stops answering
`Input.dispatchMouseEvent`, so every click raises its target first (`Page.bringToFront`); and a
`mouseWheel` event never returns at all, so the wheel is a `WheelEvent` dispatched from the page.

**What it settled.**

- A font field is a **list joined by a bare `,`**, never quoted: `Segoe UI,Arial,Courier New`. A
  pick appends, the drag handle reorders, Delete removes, and the last Delete writes `""` with the
  key kept. The page says the first family the system has is applied — so `theme::first_font`,
  which took the first entry whatever it was, was wrong for any list that opens with a family the
  machine lacks. `theme::fonts::first_installed` replaces it, against the families gpui reports,
  read once at startup (`installed_families` in `main.rs`, which drops gpui's own fallback names
  unless asking for one really lands on it, and the bundled Rubik).
- "Quick font size adjustment" is **`baseFontSizeAction`**, and the size the wheel reaches is
  **written nowhere** — not `baseFontSize`, not local storage, not on a clean quit. One pixel a
  notch, between 10 and 30. That made the behaviour cheap: a session value, not a write path.
- "Custom app icon" is not the vault's at all: Obsidian copies the file into its user-data folder
  as `icon.png` and writes `"icon": "icon.png"` into `obsidian.json`. Scoria's counterpart of that
  file is `scoria.json`, which already holds `frame` for the same reason.

**What was built.** `Page::Font(FontSlot)` and `render_font_page`: the description that changes
with the list, the applied rows in their own faces with a `lucide-x` Delete and a `lucide-menu`
drag handle (gpui's `on_drag`/`on_drop`, a `FontMove` payload) and the "This font is not detected on
your system." warning, then the `Enter font name...` box over a `uniform_list` of the installed
families — so only the rows on screen load their face — with the typed name offered first.
`FontEdit` is the list arithmetic, `font_choices` what the lower card offers, both tested against
the probe's own sequence of bytes. The row reads nothing, the family, or "*N* fonts". `menu.svg` is
Lucide's, added to the bundle.

Quick font size: the toggle is an `AppearanceFlag`, and `Theme::quick_font_size` carries it.
`fonts::quick_size_layer` is a `canvas` laid over the source editor and the reading view that
listens to the wheel in the *capture* phase, so a Ctrl+wheel it takes never reaches the scrolling
underneath and every other wheel passes; a notch is a step, a touchpad's pixels add up to one per
50. The size lives in a `QuickFontSize` global, and `reload_theme` keeps it across rebuilds until
the file's own `baseFontSize` changes — the slider moved, or the file was rewritten — which is
the last size asked for.

Custom app icon: `VaultRegistry::install_icon` copies a PNG, ICO or ICNS beside `scoria.json` as
`icon.<ext>`, deleting an earlier copy of another kind, and records it; `clear_icon` deletes and
drops the key. The row shows the copy at 64 px in place of its description, Restore default only
while one is chosen, and Relaunch once it has changed this session — which spawns
`scoria <vault> --new-window` and quits. **What Scoria cannot do is wear the icon.** gpui takes a
window icon only on X11 (`WindowOptions::icon`, which `custom_window_icon` now fills for every
window); on Windows the icon is the executable's resource and on macOS the bundle's, so there the
row keeps the choice and is tagged "not in Scoria yet", and on Linux "on restart".

**Verified in the app** over the control socket, on a scratch vault holding
`"textFontFamily": "Probe Sans,Georgia"` and an unknown key, with `SCORIA_CONFIG_DIR` pointed at a
scratch folder: the Text font row read "2 fonts"; its page showed Probe Sans with the warning and
Georgia in Georgia, over 186 installed families each in its face; picking Arial wrote
`Probe Sans,Georgia,Arial` and Delete on Probe Sans wrote `Georgia,Arial`, the unknown key kept;
the note rendered in Georgia; typing "Probe Serif" offered it alone with the warning and picking it
appended it; six Ctrl+wheel notches took the note from 16 to 22 px with `appearance.json`
untouched; and with an icon recorded the row showed it, and Restore default deleted the copy and
the key and brought Relaunch up.

**Not built**: the trackpad pinch (gpui raises it as a `PinchEvent`, not a Ctrl+wheel); a filter on
the icon dialog, which gpui's `prompt_for_paths` cannot take, so another kind is refused after the
pick; and the drag's insertion line — the row under the pointer is washed instead. gpui reports
DirectWrite's family names, which fold a few of the faces Obsidian lists on their own (`Arial
Black`, `Arial Narrow`) into their family.

### Inline HTML in Live Preview and the raw table, 2026-11-04

What the raw-HTML entry of [[UI parity backlog]] still had open, taken
against a new probe, `tools/obsidian-probe/raw-html-live.mjs`, run on
Windows with a data directory of its own (results and screenshots under
`tools/obsidian-probe/results/2026-11-04-raw-html-live*`, written up in
[[Specification#The 2026-11-04 pass: Live Preview, and a raw table]]). Two
of its answers were not what the backlog assumed.

**Live Preview does not hide the tags; it replaces the element.**
`<b>bold</b>` with no cursor in it is one `cm-html-embed` widget holding the
rendered element, its content literal: `<b>**md**</b>` draws `**md**` in
bold, where the reading view parses it. A cursor anywhere in the element,
its edges included, brings back the source of that element only.
`preview::html_elements` pairs the line's tag spans -- each inline HTML token
is its own span, so a span holding more than one tag is an HTML block's line
and is left alone -- into outermost elements, an opener with the closer that
balances it on the same line; an untouched one becomes a substitution whose
text is `model::html_inline_widget`, the reading overlay run over the
element's own source with Markdown left as text. `DisplayLine::html` carries
its styled runs, and `runs_for_display` paints them through
`style_html_run` over whatever the element sits in (a heading's weight, an
enclosing `**strong**`), never in the muted colour of its source. What the
overlay cannot draw keeps its source: `<script>` (Obsidian agrees), an
unclosed opener (agrees), a lone `<br>`, an unknown tag.

**A raw table is never a Live Preview widget.** `<div>`, `<pre>`,
`<details>`, `<center>` and `<p>` blocks are `cm-embed-block` widgets, which
the 2026-11-02 work had assumed and the probe now confirms; a `<table>`, on
one line or several, stays highlighted source with the cursor anywhere. So
`HtmlBlockKind::Table` renders in the reading view only, and the widget
predicate excludes it. The reading view draws it as `RBlock::Table`, because
Obsidian gives a raw table the Markdown table's own `div.el-table` and cell
styles: the `<thead>` row or a first row of `<th>` is the header, a body
`<th>` is bold, the first row's `align` sets each column, ragged rows are
padded, and with no header row the view now draws none. The table carries no
cell spans, so nothing offers to edit it in place.

`<sup>`/`<sub>` stay at body size. gpui's `TextRun` has no size or baseline
offset; OpenType `sups`/`subs` (which bundled Inter has) could ride on the
run's `Font`, but `HighlightStyle`, which the reading view styles through,
cannot carry font features, so doing it would mean a second run builder for
the reading view and glyph coverage that stops at digits. The backlog entry
keeps it with that reason.

Driven on Windows over `scoria-mcp`: the probe's two notes in Live Preview
and the reading view, with the inline widgets, the literal Markdown in
`<b>**md** inside</b>` and the four raw tables as the screenshots show them.

### Remote images, and images inside lists and quotes, 2026-11-04

Two backlog entries that meet in the same widget.

**Remote images.** `![alt](https://…)` was a link run with an External
target. The reading model now builds `EmbedKind::Remote(url)` for an `http:`
or `https:` destination of an inline image, a reference image and a raw
`<img src>`, the `|WxH` hint read off the alt by the same `alias_parts` a
vault image goes through, and the URL kept as written -- a raw `<img>`'s src
is percent-decoded only when it is a vault path. `![[https://…]]` is still a
missing file and `//host/x.png` still a vault path naming nothing, as
Obsidian has them. Both views draw the embed through gpui's `img()`, whose
`Resource::Uri` loader fetches with whatever HTTP client the app sets and
whose default client refuses everything; `crates/scoria/src/remote.rs` is
that client, ureq on rustls with ring and the webpki roots (no system TLS
and no cmake on any platform), one thread per request handing its answer
back over a oneshot, `GET` only, `http(s)` only, Chromium's image `Accept`,
no cookies or referrer, a 64 MB body cap. gpui caches a loaded image for the
session, which is the one-request-per-URL the spec records. A failed load
draws the alt text through `with_fallback`: the brief said blank, but the
probe below saw Obsidian show the alt beside Chromium's broken-image glyph,
so the alt it is.

**Images nested in lists and quotes.** Probed first, as the backlog asked
(`tools/obsidian-probe/nested-images.mjs`, Obsidian 1.15.0 on this Windows
machine, launched on its own `--user-data-dir` and port 9232): Obsidian draws
`- ![[pic.png]]` as one line as tall as the picture, the image at the item's
text column and the bullet at the line's foot; ordered, task, nested and
quoted lines the same; a lazy continuation or a loose item's image gets a
no-bullet line at the item's indent. `build_widgets` now walks lists and
quotes at any depth (`nested_widgets`) and makes every paragraph that is
one embed a widget whose span starts at the embed. `widget_lines` still
widens it to its line, so the editor -- which paints a widget *instead of*
its lines -- re-draws what the widget's span leaves out: `nested_prefix`
runs the bytes before the embed through the line's own Live Preview
display (`- ` is `•`, `1. ` stays, `- [ ] ` is `☐`, `> ` is nothing with
its quote depth), the prefix is shaped like any line, painted at the
widget's foot, the quote bars run the widget's height, and the embed is
laid out after it in the width left. No model type changed for it: the
prefix is the span's offset into its line.

The lazy continuation shares its paragraph with text, so it is the
inline-placement case and stays open in the backlog.

Driven on `w11` through this worktree's own `scoria-mcp.exe` on a copy of the
probe's note, with a local server serving the PNG and logging requests: in
Live Preview the bullet, `1.`, nested bullet, task box and quote bar sit
beside their images as in the probe's screenshots, the remote image draws
in a bullet, `|60x30` sizes it in a quote, the 404 shows `missing alt` on
its bullet's line, and the reading view matches. The server logged one
`GET /remote.png` per launch, carrying `User-Agent: Scoria/0.3.2` and the
image `Accept`, however many times the URL was drawn.


### The command line, 2026-11-02

The last line of [[#Parity (phase 4)]]. There was nothing to clone here:
`obsidian --help` parses no flags at all — it opens a window, and (on this
machine) downloaded an update while doing it — and Obsidian's installed
desktop entry is four interesting lines, `Exec=obsidian %u` with
`MimeType=x-scheme-handler/obsidian` and no file types. So the surface is
Scoria's own, and the one behaviour borrowed is the only one a person would
notice: **one window per vault, raised by the second launch**, which
Electron's single-instance lock gives Obsidian for free and a one-window
app has to arrange for itself.

**The crate.** `crates/scoria-cli` has no gpui in it, so the command line
builds and tests in `cargo test --workspace` without a display. `args.rs`
holds `FLAGS`, one entry per flag with its metavar, its default when given
bare, its section and its help line: `parse` looks a flag up there to
decide whether to take the next argument, and `usage` prints the same
table, so a flag cannot work and be undocumented. `the_table_and_the_parser_agree`
parses every entry and asserts it changed something, which is what catches
a flag added to the table and forgotten in the `apply` match.

**The parser is strict, where the old one swallowed everything.** `_ =>
args.note = Some(a)` meant `--surce` was a note name and a run with a typo
opened the wrong thing silently. Now an unknown flag, a missing value, a
third positional and a bad number are each an error naming themselves, and
the status is 2. `--flag=value` is accepted, since a desktop entry and a
shell script both write that. Two older sharp edges went with it: an
optional-value flag no longer eats the flag after it (`--graph --source`
was one flag), while a required value is still taken whatever it looks like
(`--type ---` types three hyphens).

**A file argument.** `scoria Rocks/Basalt.md` opens the vault that holds
the note and then the note: `target.rs` walks up for the nearest folder
with a `.obsidian`, failing that a folder on the machine's own vault list,
failing that the file's own folder — any folder is a vault, and that is
what a person opening a loose note means. The file is then opened **by
path** rather than as a link (`Args::open_path`, the control socket's
`open_path`), because a `.canvas`, `.base` or `.pdf` is no link target and
two notes of one name in different folders are different files.

**The handover.** A run that opens a vault binds a second socket named
after it — FNV-1a over the canonicalized path, in `$XDG_RUNTIME_DIR/scoria`
(`$TMPDIR` on macOS, the user's temp folder on Windows; a machine with
neither gets no handover rather than a socket in shared `/tmp`). A later
`scoria` on the same vault connects, sends `wait_loaded` and then its note,
file or URI, asks for `window {activate: true}` and exits 0; nothing
listening means the file is stale or absent, and this run opens the window
instead. A URI with no vault of its own goes to the window used most
recently, by `ts` in `scoria.json`; a URI whose `vault` names one goes to
that vault's window, which is what makes `obsidian://open?vault=Rocks`
land in the right place rather than being refused by the wrong window.
`--new-window`, `SCORIA_SINGLE_INSTANCE=0`, a spike (`--synthetic`,
`--stress`) and `--control` all opt out: a driven run was spawned by
something that wants *that* process, so handing its arguments to an earlier
Scoria would leave the caller waiting on a window that never comes.

**The handover socket answers eight methods, not thirty.** Its path is
predictable, so `scoria_control::INSTANCE_METHODS` is what it serves —
`ping`, `protocol`, `wait_loaded`, `open`, `open_path`, `set_mode`, `uri`,
`window` — and anything else comes back as "the handover socket does not
answer `keys`; run the app with --control for that". `--control` is
unchanged: the whole protocol, on a path the caller chose.
`control::start` takes a `Scope` and `SOCKETS` is a list now, so a quit
takes both files away. Writing that list turned up a drift the protocol
table was supposed to make impossible: `uri` was handled by the socket and
missing from `METHODS`, so `protocol` never listed it. It is in the table
now, with its params.

**The desktop entry.** `Exec=scoria %u` (one argument, and it may be a
URL), and `MimeType=inode/directory;text/markdown;x-scheme-handler/scoria;`
— a folder, a note, and Scoria's own scheme, which is the `obsidian://`
grammar under a name Obsidian does not own (`uri::parse` takes either
prefix). `install.sh --obsidian-scheme` claims `obsidian://` as well, for a
machine without Obsidian, and warns first if `obsidian` is on PATH.
Being in `MimeType=` only offers the entry, so the script also runs
`xdg-mime default` for the schemes — and only when `--prefix` is the
calling user's own data directory, because that list is always the caller's
`mimeapps.list` whatever `--prefix` says, and a system-wide install has no
business rewriting root's choices. (Found the hard way: a smoke test with
`--prefix` pointing at a temp folder still hijacked `obsidian://` on this
machine.)

**Tests.** 22 in `scoria-cli`: the flag table against the parser and
against the help text, the aliases, the equals form, the optional-value
rule, every error message, the vault-of-a-file ladder, the socket name
(two spellings of one folder are one instance, the folder is 0700), and
the handover itself against a fake instance — the exact method sequence
per argument shape, the mode passed along, `activate: true` at the end, a
note the vault has not got reported rather than passed off as opened, and
each opt-out leaving the instance untouched.

Driven headless on two copies of the demo vault: the first run binding its
socket, then a second run with `--note`, with a file path, with a bare
`obsidian://` URI routed through the vault list, and with
`scoria://open?vault=…` — each one changing the running window's title and
adding no second window; a URI for another vault refused with status 1; a
missing note reported with status 1; `keys`, `write_file` and `state`
refused on the handover socket; `--new-window` opening a second window;
two vaults at once keeping a window and a socket each, with a second run on
each reaching the right one; and a `SIGKILL`ed Scoria's leftover socket
file taken over by the next run rather than refused.

**What is left.** A `SIGTERM`ed app leaves its socket file behind — the
next run unlinks it, as it already did for `--control`, so this is
self-healing rather than broken. macOS needs the open-URLs Apple Event
before `scoria://` works from a browser there; a URI on the command line
works on all three platforms today, and the handover does too.

### The theme registry, 2026-11-02

Step 1 of [[Theming]], first half: the vocabulary, with the call sites still
to follow.

**The probe.** `tools/obsidian-probe/theme-registry.mjs` answers "which
custom property is this surface painted from" for 330-odd elements of the
shell, the Files pane, the reading view, Live Preview, source mode, the
right-dock panes, the file menu, the palette, the switcher, Settings and
the notice, tooltip, empty tab and autocompletion popups, by walking the
CSSOM for the rules that match each element and ranking them by
importance, specificity and order, so the winning declaration comes back
with its `var()` intact; hover states were read under a real pointer, and
the Settings rows through the popout window's document, which the opener
can reach. It also dumps every custom property declared under `:root`,
`body` and the two theme classes with its raw declaration per selector and
the value computed on `body`, in both themes. The run took four passes:
`require('obsidian')` does not resolve from the console (the view is
`app.workspace.activeLeaf.view`); CodeMirror keeps only the viewport's
lines in the DOM, so the editor is read with the caret parked on a
succession of lines; the search box's own suggestion popup covered the
results until it was removed; and three hover targets were off-screen
until `scrollIntoView`. The condensed record is
[[Specification#Observed theme tokens, Obsidian 1.14.5 (installer build 1.13.7)]].

**A second Obsidian beside the desktop's.** `run-block.sh` takes
`USER_DATA_DIR`: Obsidian launched with `--user-data-dir` on that folder
keeps its own `obsidian.json`, IndexedDB, update and single-instance
lock, so the probe no longer needs the person's Obsidian closed and never
swaps `~/.config/obsidian/obsidian.json`. The newest `obsidian-*.asar`
of the real config is copied in once; `CDP_PORT` picks a port the desktop
copy is not on.

**The registry.** `crates/scoria/src/theme/` is a module now.
`registry.rs` declares 313 tokens through a macro, each under Obsidian's
property name with a kind (`Color`, `ColorOpt` for the `inherit` ones,
`Number`, `NumberOpt`, `Length`, `Shadow`) that picks the field's type and
the reader; a value of the wrong shape keeps the default theme's.
`value.rs` is the CSS value grammar those declarations use — `var()` with
fallback, `hsl()`/`rgba()` in both syntaxes with `calc()` channels,
`color-mix(in oklch, …)` (a mix with `transparent` is the colour at that
alpha; two opaque colours interpolate in OKLCH), `calc()`, lengths,
keywords, shadow lists — parsed once and resolved against a table with
memoisation and a cycle guard. `defaults.rs` is written by
`theme-defaults.py` from the probe record: the 317 declarations the tokens
reach, as three layers (`:root` and `body`, then `.theme-dark` or
`.theme-light`), chains intact. `Theme::from_appearance` builds the tokens
once per base theme (`OnceLock`), lays `accentColor` over them as
`--accent-h/s/l` the way Obsidian's inline style does, and derives the old
short fields from the tokens, so the 750 call sites are untouched and the
app paints what it painted — verified by a capture of the docs vault in
each theme from the binary before and after: zero differing pixels.

**Tests.** `theme::tests` resolves every token from the defaults and
compares it with the value Obsidian computed for it, in both themes,
whatever syntax the computed value came back in (`#b3b3b3`, `rgba()`,
`hsl(calc(258 - 3), …)`, `color-mix()`, `calc(400 + 200)`); the short
fields are asserted against the probed shell colours; the accent setting
is shown to recolour links, tags and the tag wash; a malformed override
keeps the default and a palette override propagates. `value::tests`
cover the grammar.

**What the measurement found.** The light theme's `--interactive-accent`
is `--color-accent-1` (`hsl(257, 88.9%, 71%)`), where Scoria had painted
the dark theme's `hsl(258, 88%, 66%)` in both; and `==highlight==` is
`--color-yellow` at `--highlight-opacity` 30 percent, not `#ffd700` at 40.
Both are now the registry's. Neither shows on the Welcome note the
capture was made on.

**The first surfaces repointed, the same day.** The shell went over
first: the explorer's rows (`--nav-item-color`, `-active`, `-hover`, the
chevron `--nav-collapse-icon-color`, the extension tag `--nav-tag-color`),
every `clickable-icon` (`icon_button`, the explorer's and the panes'
header buttons, the dock tabs, the new-tab button: `--icon-color` at
`--icon-opacity`, `--icon-color-hover` opaque under the pointer,
`--icon-color-focused` while active), the tab strip
(`--tab-container-background`, `--tab-outline-color`, `--tab-divider-color`,
`--tab-background-active`, the three `--tab-text-color-focused*` titles,
`--tab-inner-background-hover`), the ribbon, the docks, the view header,
the vault profile and the status bar. Then the editor's palette, the
context menu (`--menu-*`, `--text-error` for a warning row, which also
retired the two literal reds the Bases toolbar carried), the command
palette (`--prompt-background`, `--prompt-border-color`, the hotkey chip
as `--code-normal` on `--hotkey-background`), and the reading view's
inline runs, task boxes, list markers, rule, code blocks, tables,
properties block and pills. Each pass was captured against the binary
from before the registry; every differing region was a colour the table
said was wrong, which is the finding the design note predicted: icons
sit at 0.85 opacity until hovered; an inactive tab's title and an
unsupported file's row are muted, not faint; the new-tab plus is the
icon colour, not the text's; the vault name is weight 500; a
`nav-action-button` that is active is the normal text colour on the hover
wash, not the accent; `<del>` keeps its colour; a list marker and an
unchecked box are the faint colour; the table header has no background
of its own; the properties title is the normal colour at weight 500 and
a pill is a bordered transparent box; the palette sits on the primary
background and highlights its matches in bold alone; the prompt's key
hints are muted. `code_bg` is the first short field retired.

**The rest, the same evening.** The right dock's rows went over to the
nav tokens (`--nav-heading-color` and `-collapsed` for a section's
header, `--nav-item-color-active` for a file's title, which Obsidian
keeps at the normal colour, `--nav-indentation-guide-color` for the
tree guides, `--collapse-icon-color` for the chevrons, `--text-selection`
for a hovered match, `--search-result-background` for the card); the
search pane the same plus `--search-icon-color`,
`--search-clear-button-color` and the toggle's
`--background-modifier-border-hover` track under a `--toggle-thumb-color`
knob; the Settings window its own record (`--modal-background`,
`--modal-sidebar-background` — the sidebar sits on the modal's ground,
not the secondary colour — nav items in `--text-normal` whether active
or not, `--dropdown-background`, `--background-modifier-form-field`
inputs, `--slider-track-background` and `-fill-`, buttons as
`--text-normal` on `--interactive-normal`); the switcher, the chooser and
the prompt on `--prompt-background`; the autocompletion popup on
`--suggestion-background`; every text field on
`--background-modifier-form-field` with the focus ring in
`--background-modifier-border-focus` rather than the accent. Everything
the record does not name — the Bases, canvas, graph and PDF chrome, the
bookmarks, help and vault manager — was renamed mechanically to the
token each short field already was, which a capture of the graph and
the welcome note confirmed invisible, and the short fields are gone:
`Theme` is `dark`, the fonts, the base size, the canvas palette and
`tokens`.

**Step 2, the loader, the same night.** `theme/css.rs` is the property
scanner the design note asked for and nothing more: a tokenizer that
walks blocks, strings and parentheses, descends into `@media`,
`@supports`, `@layer` and `@container`, skips `@font-face` and
`@keyframes` whole, and keeps a declaration only when its property
begins `--` and its rule's selector is a compound of `html`, `:root`,
`body`, `.theme-dark` and `.theme-light` (the most specific readable
part of a list). Every other selector is skipped without being
understood. The declarations go into a `Cascade` with the scope
(inherited from the root, or on the body), the theme class, the
specificity and the order, and `!important`; `table(dark)` picks one
winner per property the way CSS does on `<body class="theme-dark">`: a
body-level declaration beats an inherited one, then importance, then
specificity, then order. That rule is what lets a theme's
`.theme-dark { --color-base-00 }` beat the app's and keeps a theme's
`body { --x }` from beating the app's `.theme-dark { --x }` — which is
why community themes are written the way they are. Obsidian's own
declarations enter the cascade first: `defaults.rs` now carries each
with the selector it was declared under, and the generator keeps every
declaration per property in sheet order. `Theme::load` reads
`themes/<cssTheme>/theme.css` and then each `enabledCssSnippets` entry
under `snippets/`, a missing file skipped; `accentColor` goes in as an
inline declaration, which outranks any sheet short of `!important`.

**Reload.** The vault watcher already covered `.obsidian/`; the
workspace now rebuilds the theme global and repaints when a path it is
handed is `appearance.json`, a `theme.css` under `themes/` or a `.css`
under `snippets/`, and at once when the Settings window writes an
appearance change. The window's answer to the desktop's scheme lives on
the workspace (`set_system_dark`), read each frame, so a vault on
`theme: "system"` follows a desktop switch too.

**Tests.** The scanner against a sheet in a community theme's shape
(comments with braces, `@charset`, `url()` with a semicolon, a string
with a brace, a last declaration without its semicolon, `!important`, a
rule nested in `@media`, `@font-face` and `@keyframes`, a descendant
selector); the cascade's five rules; a palette override propagating
through the app's chain while the theme's private variables resolve;
and `Theme::load` on a temp vault with a theme, an enabled snippet, a
snippet not enabled and one missing, in both base themes.

**Next.** A probe of the surfaces the record has not got — Bases,
canvas, graph, PDF — so their mechanical names can become their own;
and, separately, syntax highlighting (step 3). (The Appearance tab's
pickers followed the same day:
see [[#The Appearance tab's pickers, 2026-11-02]]; the probe of the four
surfaces on 2026-11-04, see [[#The plugin chrome repointed, 2026-11-04]].)

### The plugin chrome repointed, 2026-11-04

The last of step 1: the Bases, canvas, graph and PDF call sites had been
renamed mechanically to the token each old short field was, and now read
the token Obsidian paints the surface from.

**The probe.** `tools/obsidian-probe/theme-plugins.mjs`, on Windows 11
against 1.14.5; the record is
[[Specification#The Bases, canvas, graph and PDF chrome, 2026-11-04]].
The CSSOM walk moved out of `theme-registry.mjs` into `theme-lib.mjs` so
both scripts share it. Unlike the first probe this one names no elements:
each state walks the whole view and reads one element per tag-and-class
signature, which is what turned up the toolbar badge, the hidden card
labels and the cards' title rule that nobody would have listed. What a
walk cannot reach — a focused canvas node, a dragged kanban card, the
graph's colour elements, which the renderer makes and drops while it
reads them — came from a dump of every rule naming the four surfaces.
Three things the run taught: `run-block.sh` has a Windows twin,
`run-block.ps1`; the data directory's `obsidian.json` needs
`updateDisabled`, because the first pass was quietly upgraded to 1.15.0
after its first launch and had to be thrown away; and the walks overrun
`cdp.mjs`'s 15 s call limit on a machine running five builds, so
`CDP_TIMEOUT_MS` raises it. `theme-defaults.py` writes LF on Windows now.

**The registry.** 35 more tokens: the 22 `--bases-*` the views read, the
six `--canvas-color-N` the presets are, `--pdf-background`,
`--pdf-sidebar-background`, `--pdf-shadow`, `--pdf-thumbnail-shadow`,
`--flair-background`, `--flair-color` and `--slider-thumb-background`.
Their declarations were already in the 2026-11-02 record, so `defaults.rs`
was regenerated from it rather than from this run, and every line it had
is byte-for-byte what it was. Two helpers read the shadow tokens:
`theme::ring` (the colour of a `0 0 0 1px` ring, which Scoria paints as a
border) and `theme::box_shadows` (the outer layers as gpui shadows; gpui
has no inset). `CanvasColors` reads its presets from `--canvas-color-N`,
has the control item's hover colours, and gives the light theme's card
menu the primary background.

**What moved.** About 150 of the four surfaces' 400 call sites changed
token; the rest the probe confirmed. Where the colour moved too:

- the graph: tags `--graph-node-tag` (green, was the accent), attachments
  `--graph-node-attachment` (yellow, was faint), unresolved notes
  `--graph-node-unresolved` at half opacity, links `--graph-line` opaque
  (was the faint grey at 0.6), arrowheads `--text-normal` at half
  opacity, the open note `--graph-node-focused`, labels `--graph-text`;
  the panel a menu, its toggles and sliders the Settings ones, its "New
  group" button `--interactive-normal`;
- Bases: cards on `--bases-cards-background` with the
  `--bases-cards-shadow` ring, deepening under the pointer instead of
  washing; kanban cards the same, kanban columns on
  `--bases-kanban-column-background` in a border, covers on their own
  tokens; no row wash in the table; the header row on its own ground and
  rule, its resizer `--divider-color-hover`; the edited cell's ring from
  `--bases-table-cell-shadow-focus`; summary rows on the group or footer
  ground, name and value muted, the footer without column rules and its
  "Summary" placeholder shown only under the pointer; titles never
  link-coloured on cards and kanban cards; card, kanban and list values
  in the normal colour, the list's separators muted and its markers
  `--list-marker-color`; pills and checkboxes their own tokens; text
  boxes, comboboxes and the formula editor form fields, filter rows
  `--bases-filter-input-background`; the toolbar's active button
  `--icon-color-active` and its open one the normal colour; every panel
  and menu `--menu-background` in `--menu-border-color`, the combobox
  and cell-value lists `--suggestion-background`;
- the canvas: the selection ring, the marquee (now 2 px) and the card
  menu's hover `--color-accent`; a card's fill `--background-primary`;
  the floating menu and the colour row the primary background, the
  menu's icons `clickable-icon`s; the right-click list a menu; the
  tooltip the shell's dark one; a control under the pointer
  `--text-normal` on `--interactive-hover`; the zoomed-out text card's
  icon its colour at 0.3;
- the PDF view: the toolbar's and find bar's bottom rule, the sidebar's
  end border, `--pdf-background` and `--pdf-sidebar-background`, the
  pages' and thumbnails' frames from `--pdf-shadow` and
  `--pdf-thumbnail-shadow` (the light theme's thumbnails gain their drop),
  the buttons `--icon-color` and, active, `--icon-color-active` on
  `--background-modifier-active-hover`, the find bar's miss ring
  `--background-modifier-error` at 0.6 in both themes; the embed's
  viewer ground `--pdf-background`, its page box a form field and its
  pages framed.

**Tests.** `theme::tests` gains two: the plugin tokens against the
probed colours in both themes (with the old wrong value named beside
each that moved), and a theme sheet that sets
`--bases-kanban-column-background`, `--graph-node-tag`,
`--pdf-background` and `--canvas-color-1` reaching all four.

**Not done.** The group heading keeps its one shape for every layout
(Obsidian puts the property name above the value and has no count); an
embedded base still has no frame of its own (`--bases-embed-border-color`
reads only on the fallback box); the graph's unresolved node is still a
ring rather than a filled circle; a coloured group's label text is still
chosen by theme rather than by the colour's lightness
(`mod-foreground-light`/`-dark`); the Bases error block's colour
(`.bases-error` is `--text-warning`) was not tied to Scoria's block,
whose element was not identified.

### The Appearance tab's pickers, 2026-11-02

The third of [[Theming]]'s loose ends: until now a theme was chosen by
writing `cssTheme` and `enabledCssSnippets` into `appearance.json` by
hand, because the Themes, CSS snippets and Accent color rows were all
tagged "not in Scoria yet".

**The probe.** `tools/obsidian-probe/appearance-tab.mjs` reads the tab
through two handles at once: the vault window's JS, which can query the
Settings popout through `app.setting.activeTab.containerEl` whatever
document it lives in, and a bare CDP client attached to the popout's own
page target, which is the only thing that can inject a click into it or
capture it. Eighteen steps: every row's name, description, control and
markup; what picking a theme, toggling a snippet, setting an accent and
picking a font each write; both sections with nothing installed; and the
three questions below. The record is
[[Specification#Observed Appearance tab, Obsidian 1.14.5 (installer build 1.13.7)]].

**Three things the probe settled that guessing would have got wrong.**

- A theme folder is listed **only when its `manifest.json` names the
  folder**. `folder-name` with a manifest saying `Display Name` was
  dropped, and so was a folder with no manifest; only the one whose name
  and folder agreed was offered. `Vault::css_themes` is that rule, and it
  means `cssTheme` is simultaneously the folder, the manifest name and
  the dropdown value.
- **Enabled snippets cascade in the snippets folder's order, not the
  enabled list's.** Enabling `tabs`, `wider`, `dim` in that order left
  the `<style>` elements as `dim`, `tabs`, `wider`; and `zz-last`
  enabled first beat `aa-first` enabled second. Obsidian rebuilds the
  whole set on every toggle from its sorted file list. `theme::sheets_of`
  had read them in the file's order since the loader landed that night,
  which was wrong for any pair that set the same property; it now sorts,
  and `enabled_snippets_cascade_in_the_folder_s_order` pins it.
- **The tab is a drill-in.** "CSS snippets" and the three font rows are
  `.setting-item.mod-navigable.tappable` that push a `.setting-page`
  with a back chevron and its own title; the snippets are not rows on
  the tab. The first pass looked for them there and found none. Escape
  on a page backs out to the tab and leaves Settings open, which
  `escape_closes` now has as a rung of its own between a popover and the
  modal.

**What was built.** `Control` gained `Color` and `Navigable`; `Item`
gained a `Picker` (a dropdown whose options are the vault's rather than
a static `Typed` table), a `Page` and a `folder` button. The Themes row
is a `Picker` over `Vault::css_themes()` with the `lucide-folder-open`
"Open themes folder" button beside it; "Current community themes"
counts them, with the plural following and the "Check for updates"
button dropped at zero, as Obsidian does. The accent row is a swatch
behind a `lucide-rotate-ccw` "Restore default": GPUI raises no colour
dialog, so the swatch opens the row's text field and a hex is typed,
and `normalize_accent` stores the lowercase `#rrggbb` Obsidian's native
picker would have — or the empty string, which is what "Restore
default" writes and what Obsidian reads as "no accent set". The CSS
snippets page is `render_snippets_page`: the reload and folder buttons,
a `Search snippets...` box and one toggle per snippet, in the folder's
order, backed by a new `Flag::CssSnippet` that appends on the way on and
removes on the way off.

`Vault::css_themes` and `css_snippets` do the folder reading, and
`SettingsView` caches them in `CssFolders`, refreshed when the modal
opens, when the vault's state is set again and when the page's reload
button is pressed — `items` is called per render, so a folder scan
cannot live there. The "*N* enabled" the collapsed row shows counts the
enabled snippets whose file still exists, which is why a name left
behind by a deleted `.css` stops counting; Scoria had "None enabled" at
zero, where Obsidian says "0 enabled".

**Base color scheme, the fonts and the font size lost their "on
restart" tags.** They were right when `Theme` was built once at startup;
since the loader landed, `reload_theme` rebuilds the whole global —
fonts and base size included — from `appearance.json` on every write, so
all five rows are live. Only "Current community themes" keeps a tag.

**Verified in the app**, on a scratch vault with two themes (plus one
Obsidian would drop) and three snippets: the dropdown offered Default,
Ember and Zephyr and not `folder-name`; picking Ember recoloured the
Settings window and the workspace as the click landed; the three
snippet toggles wrote `["tabs", "wider", "dim"]` and dimmed what `dim`
dims; the accent wrote `#16a34a` and "Restore default" wrote `""`;
Escape backed out of the page; and the collapsed row read "3 enabled".

**Not built**, and filed in [[UI parity backlog]]: the "Manage" button
(Obsidian's community theme browser — the probe could not record it
offline, and Scoria will not download themes), the three font pages, and
the "Quick font size adjustment" and "Custom app icon" rows, whose keys
the probe did not record.

### Syntax highlighting, 2026-11-02

Step 3 of [[Theming]], and the last of the themes subset.

**The probe came first and changed the plan.** `code-highlight.mjs` found
that Obsidian runs **two** highlighters over one palette — Prism in the
reading view, CodeMirror in the editor — and that they disagree with each
other: a JSON key is `--code-property` rendered and `--code-function` in
the editor, a Python decorator is punctuation in one and `--code-comment`
in the other. Both resolve to the same eleven `--code-*`, which the
registry has had since step 1, so no token was added for this.

It also found that Obsidian's `.token` rules are unreachable through
`document.styleSheets` — a dump returned 184 CodeMirror rules and none of
Prism's, with 39 token spans on screen. The question was put to the
renderer instead: a span of each candidate class placed inside a real
rendered block, its computed colour read back and matched to the variable
holding it. **Thirty-three classes are styled; everything else inherits
`--code-normal`.** `--code-operator` and `--code-tag` hold the same value
in both themes, so only the cascade record separates them. The table is in
[[Specification#Observed code highlighting, Obsidian 1.14.5 (installer build 1.13.7)]].

**Prism was ported, not wrapped.** [[Theming]] had expected `syntect` to
be the quick way to breadth; measuring it killed that: its bundled set is
75 syntaxes against the 171 names Obsidian highlights, with no TypeScript,
TOML, Kotlin, Swift, Nix, Dockerfile or GraphQL — and breadth was the
whole argument for it. Prism's grammars are MIT and upstream, which is the
same footing as the Lucide icons this repo already cuts from source, and
Prism's token names are what a theme addresses, so porting the engine buys
*exactness* that no mapping from another engine's scopes can.

`crates/scoria-highlight` is that port.

- `engine.rs` is `matchGrammar` from `prism-core.js`. Prism's output
  depends on its grammar's key order, on a greedy pattern's right to reach
  across tokens already made, and on the rematch that follows when one
  does, so the algorithm had to come over whole. Two things differ and
  neither changes the output: a `Vec` with the same splice points instead
  of the linked list, and every node a **byte range** rather than a string,
  since Prism only ever stores slices of what it was given. Prism's list
  has a head sentinel and a `Vec` has not, which cost one bug — the first
  run tokenized nothing at all because `startNode.next` had become
  `start + 1` past the end.
- `grammar.rs` is the data: an ordered map of name to patterns, with
  Prism's four flags. `lookbehind` is not a regex lookbehind — the first
  capture group is context that is not part of the token — and `greedy` is
  what lets a string win over what is inside it.
- `compiled.rs` compiles a grammar once and memoises it by address. The
  grammars are `static`s of pure data because **a `static` holding any
  interior mutability cannot be const-promoted at all**, so the cache
  cannot live in the pattern; keying by address also settles the recursion,
  since Prism's grammars refer to themselves (a Nix interpolation holds
  Nix, a Python format field holds Python) and compiling eagerly would not
  terminate.
- `palette.rs` is the thirty-three-class table, and `languages/` the
  grammars.

**The corpus is the ratchet.** `tests/corpus/obsidian-1.14.5.json` holds,
per language, a sample and the exact `innerHTML` Obsidian rendered it to. A
grammar is correct when Scoria's HTML is identical, which compares every
token's name, its aliases, its nesting and its boundaries in one
assertion. Eight languages match byte for byte — css (with `css-extras`,
which Obsidian loads and which is where `unit`, `hexcode` and the selector's
`pseudo-class` come from), diff, go, json, python, rust, toml and yaml —
and clike, ini and nix are ported without a sample. Two mismatches were
real finds: `>` has to be escaped because `innerHTML` serialises it that
way, and CSS without `css-extras` is visibly poorer than Obsidian's.
`every_pattern_compiles` guards the translation from JavaScript's regex
syntax, where `/…/i` becomes `(?i)…`, `\/` is not an escape and a `[`
inside a class must be.

**Both views paint from it.** The reading view's block is a `StyledText`
with one highlight per run; the editor computes per-line runs in the same
background parse as the markdown spans and lays them over the run list, so
the mono family and the block's background stay what the markdown said
while the colour and the emphasis come from the token. The body the parser
hands back has its fence and indentation stripped, so an offset in it is
not an offset in the document: the mapping is done a line at a time, since
a body line is a suffix of the document line it came from and the
difference in length is the prefix removed. A line the parser rebuilt
rather than sliced is left plain rather than placed wrongly.

Scoria's two views therefore agree with each other, where Obsidian's do
not, and both match Obsidian's rendered block.

### Four small entries and a URI, 2026-11-02

The open backlog's concrete entries, taken together.

**The reading model is lazy in the editing modes.** `show_file` used to
build the reading model for every markdown open; now a note opened in Live
Preview or Source mode reaches the reading view as text
(`ReadingView::set_note_later`), and the model is built on the first read:
the view's render, a fold sync on the way into reading view, or the
control socket's state dump, which calls `Workspace::ensure_reading_built`
first so a dump still describes the note. The built note is tracked apart
from the shown one so a reload of the same note keeps the reader's place
and another note drops the stale model at once.

**Raw HTML blocks.** The reading model classifies an HTML block by its
first tag, a leading comment skipped: `<pre>` becomes a code block with
`wrap` set and `copyable` only for a `<code>` child, as observed;
`<details>` becomes `RBlock::Details`, its summary and body built through
`rich_html`, which tokenizes raw HTML into the same overlay inline HTML
takes, decodes entities and collapses whitespace; a container keeps its
content as a paragraph; the rest stays source. The reading view wraps a
`wrap` block and draws the copy button only when `copyable`; a details
block folds through the callout's toggle. Each renders as a Live Preview
widget, since the widget predicate asks the same classifier.

**The All properties row's menu.** `file_menu::Target::Property` draws
Rename, Property type ▸ and Delete property; items gained `warning`,
`checked` and a `submenu`, which the menu draws beside the row the pointer
last rested on (`Menu::submenu`, set through the new `on_submenu`
callback), flipped to the row's left when the menu stands near the
window's right edge. Rename prompts (`PendingOp::RenameProperty`) and then
`rewrite_property_everywhere` plans `Change::Rename` over every note whose
cached frontmatter carries the key, under the cell edit's unsaved-buffer
guard; Delete plans `Change::Remove` the same way; both take the
`types.json` entry along through `VaultState::rename_property_type` and
`remove_property_type`. Property type writes `types.json` as the Bases
header's submenu does.

**`obsidian://` URIs.** `crates/scoria/src/uri.rs` parses the four
documented actions -- `open` with `file` or `path`, `new` with `name`,
`path`, `content` and the `silent`, `append`, `prepend` and `overwrite`
flags, `search` with `query`, and `daily` -- from Obsidian's public help,
and `Workspace::handle_uri` acts on them, refusing a `vault` that names
another vault. A URI reaches the app as a bare `obsidian://` argument, as
`--uri`, or over the control socket's `uri` method; the scheme is not
registered with the desktop, which Obsidian owns on a machine that has
it.

**The Writer harness.** The config round trip over the real vault failed
for a reason that was not a round-trip failure: that vault's `.obsidian`
holds only `workspace.json`, which Scoria neither models nor writes, so no
modelled file existed to check. The test now says so and passes
vacuously; the fourteen note round trips are the gate there.

Driven headless: the pre, details, container and table cases in both
views; a note opened live then switched to reading; Rename writing
`colour` into eight notes and Delete removing it from the same eight; the
four URI actions over the socket, with the wrong-vault refusal.

### Escape closes every menu, 2026-11-02

The backlog's oldest small bug: the file menus closed on a press outside
and on nothing else. The fix is one mechanism rather than one binding per
host. `file_menu::MenuFocus` is a focus handle a host hands to its menus,
and `menu_box` makes a list box a menu: it tracks that handle under the
`Menu` key context, where Escape is bound to `CloseMenu`, so the menu
holds the keyboard while it is up and Escape reaches it before the
editor's, the explorer's or the Bases view's own Escape. The host calls
`MenuFocus::sync` from its render with whether any of its menus is open:
the first render with one open takes focus, remembering where it was, and
the first render with none gives it back -- so a pick, a press outside and
Escape all restore focus the same way, with no window needed at any close
site. The workspace's file menu and its backlinks, tags and properties
sort menus, and the explorer's row and sort menus, go through it; the
canvas's own Escape closes its context menu before clearing the
selection; the Bases menus already closed through their view's layers.

### The kanban board, 2026-11-02

The last entry of [[UI parity backlog#Bases]], built to `s10-kanban`,
`s20-fix-kanban` and `s26-kanban-rest` of the 2026-11-01 record.

A column is the observed 46 px header — the group's property and value,
the count, *Create new note* on hover — over its cards and a 41 px *New*
footer. A card drags (`CardMove`, with a title preview at the pointer and
the source drawn faint); the board, not the column, computes the drop
target from the pointer, because a drag-move listener in gpui fires only
for the element the drag started on, so a column cannot watch for another
column's card. A card-sized placeholder stands at the slot in the target
column — the uniform list holds one more item there — and the drop asks the
workspace to set the grouped property of the note to the column's value,
through the same `SetProperty` path a cell uses; the no-value column clears
it. A column header drags (`ColumnMove`) with an accent line at the slot;
the drop reorders `KanbanState::order` in memory, which a resolve keeps by
mapping the old keys onto the new columns, and writes `groupOrder` only
when the view's order is already Manual. *Hide column* writes `groupOrder`
as every drawn column but that one, which is what makes the order Manual
in the first place, exactly as the record shows after hiding *done*. *Add
item*, the header's plus and the footer's *New* put the column's value
into the new note's frontmatter beside the filters' keys
(`KanbanState::pending_group`, consumed by `take_new_item`). *Edit
groups...* opens the Group panel through the new `show_panel`. Escape
closes the column menu like every other layer.

Driven headless on the demo's Volcanoes board: the menu, a card dragged
from *active* to *dormant* with the placeholder showing and `status:
dormant` written to Kīlauea, a column dragged in memory with the file
untouched, *Hide column* writing `groupOrder: [extinct, dormant]`, the next
column drag writing the new order, *New* creating a note with `status:
dormant`, and *Edit groups...* opening the Group panel.

### The column header, summaries, cells and the table keyboard, 2026-11-01

The two large entries of [[UI parity backlog#Bases]] and the small one on
card and list menus, built as one campaign against the record.

**The header.** Each header is its type icon and name — the properties
panel's own icon for the same id — with a `chevron-down` or `chevron-up`
while the column sorts, a 6 px resizer that turns accent under the pointer,
and a header drags to another slot: `HeaderMove` on the header, the drop
line drawn at the nearest column edge over the table's own height, and
`writes::move_column` rewriting `order` on the drop. The context menu is
`edit::header_menu_rows` per `HeaderKind`, with icons, the checked sort,
the warning-coloured *Clear sort*, and a *Property type* list beside its
row on hover that asks the workspace to write `types.json`. *Group by this
property* writes `groupBy` ascending; *Edit property...* and *Edit
formula...* open the properties panel's own pages, hung under the header
through the toolbar's new `pinned_at`; Escape closes the menu first.

**Summaries.** `scoria_bases::summary` is the engine side: the class of a
column's values, the options it offers in Obsidian's case-insensitive
alphabetical order with the base's own `summaries` statements among them,
and what each computes — two decimals, date-times, a humanized date range,
a custom statement evaluated with `values` bound. The engine's `round` now
takes the digits argument the fixture's own `customMean` uses. In the app,
`bases::summary` draws the footer under an ungrouped view's rows and
`Line::Summary` under each group heading when the view has any summary, and
the Summarize page as `Panel::Summary` — opened from the header's row or a
summary cell, picking writes `summaries.<id>`, *Hide summary* removes it,
*Add summary* is drawn inert because what it creates was never observed.

**Cells.** The name cell is a link whose right click raises the file menu
(`file_menu::Target::Row`, also on a card and a list line's link, through
`BasesEvent::FileMenu`); a frontmatter cell opens its field inside the
accent ring with the value selected; a text or list cell shows the
property's distinct values under it (`infer_properties` gathers them with
the kinds), narrowed by the typed text or the list item being typed, walked
with Up and Down, taken by Enter or a click; a list cell draws pills and
edits as its comma-joined text; a date cell carries *Open daily note*,
which `Workspace::open_daily_on` answers for that day; `file.*` and formula
cells take no click.

**The keyboard.** `TableState::focused` is the invisible cursor: Tab,
Shift+Tab, Left, Right, Up and Down move it (the toolbar's own actions
first, the cursor when nothing is open), Enter and Delete open the cell — a
formula's Edit formula page, a checkbox flipped — a printable key opens it
holding that character through the root's `on_key_down`, Enter in the field
commits and keeps the cursor, and the second Escape leaves the cell with
the cursor gone.

Driven headless on a scratch base over the rocks and volcanoes with list,
text, number and date columns: the menu and submenu, the drag writing the
new order, Group by, the Edit page under the header, the Summarize page
with its values, a footer and group rows, Hide summary by keyboard, typing
`fel` + Down + Enter writing `felsic`, the row and card menus, the daily
note of `1943-02-20` opened from a cell, and the cursor's every move. Open
after this: *Add summary*, shift-click selection, the list cell's own
pills-with-input editor, and the kanban entry.

### The embedded base and the base leaf's chrome, 2026-11-01

Two more entries of [[UI parity backlog#Bases]], built to the same record.

**The embed is the view.** Obsidian's `![[x.base]]` is the whole Bases view
working in place — toolbar, panels, the layout the view names, cell editors,
*New* — in a box with no title, border or padding, as tall as its content
and scrolled by the note. So an embed is now a live `BasesView`, the same
entity a leaf shows, in an *embedded* mode: `bases::embed` keeps a registry
of those views (`EmbedViews`), one per host and embed; the reading model
carries only the path, the view the `#subpath` names and the embed's ordinal
among the ones naming the same base and view, so the same base written
twice is two views. A host — a reading view or an editor — syncs the
registry whenever its model changes, which makes a view for each new
embed, keeps the ones still drawn with their open panels, and drops the
rest; the workspace subscribes to every view the registry makes (through a
deferred `on_create`, since the view is made inside the host's own update),
saves the ones that edit their file through the same `flush_base_view` as
the leaf's, re-resolves them on every vault change and reloads them when
their `.base` is rewritten on disk. In embedded mode the table and list lay
their lines out in full rather than through a virtualised `list`, the cards
grid drops its scroll box, and the kanban board is as tall as its tallest
column (unobserved: the run embedded a table and a cards view). Live
Preview draws Obsidian's **Edit this block** over the embed, occluding the
*New* button it overlaps, and a click puts the caret on the embed's line.
The canvas, which has no registry, draws a titled box that opens the base.

**The leaf's chrome.** The tab icon is `lucide-table`; the base leaf's
header draws back, forward, the title and **More options** alone, which
opens the observed menu right-aligned under the button, less the three rows
Scoria cannot run — `file_menu::Target::View`, with "Split right", "Split
down" and "Delete file" routed to the group's splits and the delete prompt.
Every ribbon button carries its observed tooltip. A new base is one `file
name` column over every file of the vault: `implied_columns` is `file.name`
alone now, and `rows::env_for` hands the engine every scanned file rather
than the notes, an attachment's row carrying its names, size and stamps and
no cache fields. The explorer's doubled separator before Rename on a
`.base` row turned out to be Obsidian's own.

Driven headless on a copy of the demo vault with a note embedding the
cards, list and board views, a view name no view has, a missing base and
the same base twice: every panel opened inside the embed, a row click
opened its note, *New* created `Rocks/Untitled.md` with the filter's
prefill, removing a sort through the embed wrote `Rocks.base` after the
autosave and the leaf then opened on the new bytes. One lesson for the next
drive: `open` with `Name#Heading` puts the embed at a repeatable position,
where a pixel scroll does not.

### The Bases toolbar and its panels, 2026-11-01

The first build against the Bases record of 2026-11-01
([[UI parity backlog#Bases]]): the bar and the six surfaces its buttons open,
rebuilt to the observation rather than to the readings `toolbar.rs` had
carried since 2026-10-25.

- **The bar** is Obsidian's eight `text-icon-button`s in a 40 px header —
  the view switcher, *N results*, Sort, Group, Filter, Properties, Search,
  New — labelled, iconed, accent-coloured while their control holds a value,
  and with no tooltips. Search drops a `Find...` row under the bar that
  narrows the rows by their displayed cells; New creates the note
  `bases:add-item` does, with the frontmatter the filters imply.
- **Panels hang under their button** and close on Escape, on a second click
  of the button and on a press outside, one layer at a time; the views and
  properties panels focus their search field, where Down, Up and Enter walk
  and pick. Every combobox opens a `suggestion-container` of its own under
  its trigger, with a search field when Obsidian's has one.
- **Views**: search, the rows with their gear, Add view (named `View`, the
  fifth included). The Configure page carries the name field, the Layout
  combobox, the layout's own rows under Obsidian's labels, and the
  `more-vertical` menu — Default view, Duplicate view, Delete view.
- **Properties**: shown columns first, then the hidden `file.*`, formulas
  and note properties, faint; Add formula (`Untitled: ""` and its column),
  Hide all (`order: []`), `Add "x" property`; the gear's pages — display
  name and property type, or display name and the formula with its live
  *Valid formula* / *Invalid formula* line.
- **Filter**: rows of property, operator and value from
  `scoria_bases::rule`, with the observed operator lists per kind, pills for
  a list, a checkbox for a boolean, the advanced toggle to the statement and
  back, Add filter and Add filter group, and the collapsible sections with
  their count flairs.
- **Sort and group**: *Sort by* with grips that reorder by drag, *Add sort*
  appending a row that waits for its property; *Group by* with *Manual*,
  which is `groupOrder`, and the group list under it with its checkboxes
  and grips.
- **Results**: *Show all* faint until a limit is set, *Copy to clipboard*,
  *Export CSV...* through the desktop's save dialog.

Behind it, in the gpui-free crates: `View.order` is `Option<Vec<String>>`
so `order: []` round-trips; `groupOrder` hides the groups it does not name;
`scoria_bases::rule` parses a statement into a row and prints the row back;
`scoria_bases::csv` and the view, formula and display-name helpers in
`layout.rs`.

**Three things the headless drive taught.** The control socket's `type`
never reaches a pane's own `TextInput` — send `keys` one character at a
time. A deferred panel positioned from a measurement must read the
measurement *before* the frame that clears it, or it lands at the click
point and covers the bar; the box map now lives across frames and is pruned
when a panel closes. And a panel that moves by 15 px invalidates every
coordinate a script learned from the previous screenshot — read the state
dump after each click rather than trusting the aim.

Not built here, filed to stay: the list layout's *Indent properties* toggle
(its key is still unobserved), *Hide all* in the group list (an empty
`groupOrder` is not representable), the formula editor's completions and the
value fields' suggestion lists.

### Eight backlog entries, and the probe that unblocked three, 2026-10-30

A pass over what was left of [[UI parity backlog]], run as parallel worktrees
with the integration done here. What landed:

- **The local graph's Depth and direction options.** `localJumps`,
  `localBacklinks`, `localForelinks` and `localInterlinks` on `GraphView`, the
  Filters section drawing the Depth slider and three toggles in local mode, and
  a `local_subgraph` that records each node's BFS depth, walks out to
  `localJumps` hops and follows an edge only in the directions the flags allow.
  With Neighbor links off it keeps only edges whose endpoints differ by one
  depth, which hides the neighbour-to-neighbour edge. The options live for the
  session: Scoria writes no localgraph leaf, so there is nowhere to persist
  them.
- **Inline HTML as styles.** A tag stack in `RichBuilder` maps the measured
  subset onto runs, with four new `InlineStyle` flags (`underline`, `kbd`,
  `sup`, `sub`); an unclosed opener runs to the end of its paragraph, which is
  what the sanitizer capture records. A block of nothing but comments and
  `<script>` elements now draws nothing at all. `<pre>` is deliberately not
  promoted: Obsidian wraps it and gives it a copy button only with a `<code>`
  child, and the reading view's code block does the opposite on both counts, so
  the model promotion waits on two changes in the view.
- **Markdown image size hints and embeds outside a paragraph.** One
  `alias_parts` helper serves both spellings, so `![a|100](p.png)` and
  `![[p.png|100]]` cannot disagree again; an embed in a heading or a table cell
  falls through to the link arm rather than showing its own brackets.
- **The reading view's property pills** got their remove control, through one
  narrow sink (`ReadingHost::on_remove_item`) rather than the whole
  `PropertiesHost`. The workspace takes the event the way it takes
  `TaskToggled`, through the editor's buffer, so a pill writes the same bytes
  whichever view was clicked.
- **The file menu became `file_menu.rs`**, drawn once for every view that
  raises it, and the Backlinks and Outgoing links rows raise it now. **Show in
  system explorer** came with it (the D-Bus `FileManager1.ShowItems` call, with
  `xdg-open` on the parent folder as the fallback), and **Make a copy** on a
  folder: `Vault::duplicate` walks what the scanner would index and copies every
  file's bytes verbatim, rewriting no link, because what Obsidian does to a
  copied folder's links is unobserved.
- **The inline title.** `showInlineTitle` had no reader; the note's name now
  sits above its first block at h1 size in both views, as a *sibling* of the
  reading view's list so block indices and folds keep their meaning, and the
  editor reserves the same height above line 0 so a mode switch does not move
  the text.
- **The window frame style has a store**: the `frame` key of `scoria.json`,
  beside the vault list, where Obsidian's `obsidian.json` keeps its own.

Three more landed on the probe's own evidence, later the same day: **every
remaining core-plugin options tab** (Backlinks, Canvas, Page preview, Note
composer, Unique note creator, File recovery and Web viewer, each with its own
config type and `SettingChange`, every row tagged "not in Scoria yet" because
nothing reads these keys yet, and the rows whose key the probe never reached
drawing but writing nothing); **the editor's context menu**, which places the
caret and builds its rows from the selection the way the probe recorded, as an
extension of `file_menu` rather than a second widget; and **selection and copy
in the reading view**, an anchor and a head over a document-order walk of the
note's runs, with the handlers hung off each run of text because gpui can only
answer "which byte is under this point" for text that has been prepainted, and
`list` measures items it never paints.

**The probe is the part worth keeping.** Three entries had been filed as
"nobody has observed Obsidian", so `tools/obsidian-probe/editor-menus.mjs` went
and looked: the editor's right-click menu, the view header's actions, and every
core plugin's options tab with each toggle flipped twice, each dropdown walked,
and the `.obsidian` files diffed after every click, so each control's key is
observed rather than guessed. Both write-ups are in [[Specification]]. Three
things in it are worth remembering: a right click **places the caret** and the
rows are built from the selection as it stood; the **reading view has no text
menu at all**, only a link raises one; and the view header's **Bookmark action
has a zero rect** in the ordinary state, so it is hidden rather than missing.

**Driving the running app is what found the unfiled bug.** With everything
merged, the app was opened on the real vault over the control socket: the
inline title draws at one height in both modes, a scroll down and back returns
to an exact top, and the new Backlinks row menu matches the capture row for
row. Then Escape, which closes no context menu in Scoria and never has --
two passes of reading the code had missed it, and it is filed now.

**One integration trap, twice.** Several worktrees share one
`CARGO_TARGET_DIR`, and cargo's path-source metadata hashes do not include the
checkout path, so a build in one checkout is handed another's stale artifacts:
twice this showed up as `cargo` insisting a function that plainly exists is
missing. `touch` the files you edited, and check your new tests appear by name.

### The Daily notes and Templates options tabs, 2026-10-30

The backlog's "Every core-plugin options tab but two shows one 'Options not
shown yet' row". The configs were there (`DailyNotesConfig`, `TemplatesConfig`,
both round-tripping byte-exact), the rows were not, and neither were the
labels: the project rule is Obsidian's exact strings, observed, and the
Specification had none for these two tabs.

`tools/obsidian-probe/plugin-options.mjs` read both tabs off
`app.setting.activeTab.containerEl` and drove every control, reading the
plugin's file after each (`results/2026-10-30-plugin-options.json`, and
[[Specification#Observed Daily notes and Templates tabs, Obsidian 1.14.5]]).
Two things it settled that a guess would have got wrong:

- **"Date format" is a dropdown, not a field**, with four presets labelled by
  today's date in each (`2026-10-30`, `2026.10.30`, ...) and a fifth, Custom,
  that writes nothing and reveals a "Custom format" field. A preset writes the
  pattern as `format`, the default included: picking `YYYY-MM-DD` on a vault
  with no `daily-notes.json` creates `{"format": "YYYY-MM-DD"}`.
- **There is no toggle.** The "Open daily note on startup" row (`autorun`) the
  backlog entry named is not in 1.14.5's tab, so Scoria does not draw one.

The rows are the existing `TextField` and `Typed` shapes on the two config
types, with two additions to `Typed`. `label_of` renders an option's label
from its value, which is what makes the four presets read as dates rather
than patterns (and keeps `every_dropdown_writes_its_own_key_and_nothing_else`
true, since the write is still the static value). `local` names the option
that is a pick rather than a value: `pick` remembers it in
`SettingsView::local_pick` instead of writing it, `selected_of` shows it, and
`plugin_items` reveals the Custom format row for it — or for a stored format
that is none of the presets, which `shape` maps to the same option. The
descriptions carry Obsidian's "Your current syntax looks like this:" sample,
rendered from `workspace::now()` through `scoria_templates::format`, falling
back to the placeholder's pattern when the field is empty, as Obsidian's does.

`SettingChange` grew `DailyNotes` and `Templates`; `apply_setting` writes
`daily-notes.json` / `templates.json` through the same `write_config` the
switcher uses, whose bytes already match (two-space, no trailing newline),
and updates the `RwLock` the daily-note and template commands read from, so
every row is live and none is tagged.

Left as Obsidian has it only in shape: "Template file location" is a file
suggester there and a text field here, and its placeholder was not observed.

### Bases property ids, 2026-10-30

Obsidian 1.14.5 rewrote the demo vault's `Rocks.base` while it was open there:
every `note.composition`, `note.silica` and `note.density` in `order` and `sort`
came back bare, `properties` kept its `note.silica` key, and a header drag had
written `columnSize` keyed `note.silica` beside an `order` that now said
`silica`. The 2026-10-25 probe never saw this because none of its fixtures put
a qualified id in `order` or `sort`; `layout::qualify` was built on what it did
see — a bare `status` came back `status`, the image picker wrote `note.aliases`
— and wrote `note.x` for every id the UI added.

`tools/obsidian-probe/bases-ids.mjs` asks the question directly: one base with
every id-bearing key in both spellings, opened, saved through `setLimit(7)`,
then a drag on the `status` column. The answers, all from the one file
(`results/2026-10-30-bases-ids.json`, fixtures `ids.in.base` / `ids.out.base`):

- The loader qualifies in memory: the `Bare` view's `order: [file.name, status,
  price]` was `["file.name", "note.status", "note.price"]` in `q.views[2].order`.
- The serializer strips `note.` from `order`, `sort[].property`,
  `groupBy.property` and the keys of a view's `summaries`. `file.` and
  `formula.` ids pass through. The top-level `summaries` is keyed by summary
  name and is not an id map at all.
- The keys of `properties` are written as they are: `note.price` and a bare
  `status` both survived. So does `image: note.cover`.
- `columnSize` is keyed by the qualified id and never normalized: the fixture's
  `note.status: 150` sized the status column (150 px measured) while `order`
  said `note.status` and again after it said `status`, and the drag wrote
  `note.status: 239` in place.

Scoria now does the same, at the writer rather than the model.
`scoria_formats::base::written_id` strips `note.` in `View::to_value` for the
four keys, so the model keeps the file's spelling (the toolbar still adds
`note.x` through `qualify`, as Obsidian holds it) and the bytes come out as
Obsidian's. `TableLayout::width_of` looks an id up qualified first and as
written second, which is what makes `order: silica` find `note.silica: 90`;
`set_column_size` writes the qualified key and `clear_column_size` drops both
spellings. The demo vault's `Rocks.base` and `Volcanoes.base` were normalized to the
bare ids by hand so Obsidian stops rewriting them, which
`the_demo_vaults_bases_are_the_bytes_obsidian_writes` now insists on; the sorts
and widths that Obsidian session added to `Rocks.base` are in a stash, not in
the file, because `the_demo_vaults_rock_table_shows_the_rocks` reads the
committed view as "densest first".

Unprobed: whether a `columnSize` key written bare (`status: 150`, as an older
Scoria or a hand would write) sizes its column in Obsidian. `width_of` honours
it either way.

### Settings in a window of its own, 2026-10-30

Asked for: Settings should come up in a real window. Obsidian's does not — its
Settings is `.modal.mod-settings`, a modal over the workspace that owns the keyboard
until it closes (and, oddly, has no `.modal-bg`; see the Specification's note). This is
a deliberate divergence, taken on the grounds that the vault manager and Help are
already windows and that a settings panel is a thing you keep open beside the vault
rather than in front of it.

`SettingsView` stays where it was, on the workspace: it is the panel, it holds the open
tab, the open dropdown and the field being typed into, and the workspace still listens
to it for every `SettingChange` it writes. What moved is the frame around it.
`SettingsWindow` is a root view holding that one entity, opened with the desktop's own
decorations at 1100x720 — Obsidian's modal width, and a height to match — and the panel
lost the border, rounding, shadow and `max_w` it wore as a modal, because the frame
around it is now a real one. `Modal::Settings` is gone from the workspace's modal enum
altogether, which is the point of the change: `hotkeys_live` and `note_has_keys` are
both `self.modal == Modal::None`, so with Settings out of that enum the vault behind it
keeps its hotkeys and its focus.

Three things had to be kept whole across the move:

- **The socket's contract.** `state.modal.kind` has said `"settings"` since protocol 2,
  with the open tab's rows under it, and a caller should not have to learn that it is a
  window now. `describe_modal` reports `"settings"` when the window is up — the handle
  is checked with `WindowHandle::read`, so one left behind by a window the desktop
  closed reads as gone — and `focus_name` answers `"settings"` for the same reason: the
  focus is in another window and is not the workspace's to report.
- **Input.** `Ctl::input` is the window a caller is driving: the Settings window while
  it is up, the workspace's own otherwise. Keys, typing, clicks, the mouse, the wheel
  and `wait_idle`'s frame chain all go through it, and so does `window`'s activation
  and the size it reports back, since click coordinates are window-local and a
  screenshot captures whatever is in front. A resize still sizes the app's own window.
- **The window's lifetime.** The panel writes through the workspace, so a Settings
  window outliving it would show a vault that is gone and write nothing; worse, gpui
  quits on Linux only when the *last* window closes, so the orphan would keep the
  process alive with no workspace in it. `SettingsWindow` holds a release subscription
  on the `Workspace` entity and takes itself down with it. Verified by closing the main
  window with a KWin script while Settings was up: both windows went and the process
  exited.

Checked on the real app over the socket: `app:open-settings` raises the window and a
second one raises the same window rather than a second; a click at window-local
coordinates switches tabs; Escape and the panel's × close it and the editor takes focus
back in the main window; a toggle clicked in the window wrote `readableLineLength:false`
into the test vault's `app.json` and changed nothing else; and a dropdown's options
still paint over the rows below them.

Left as it was: a write that fails still reports into the workspace's notice corner,
which is now behind the Settings window rather than under its backdrop. The dropdowns
are still the panel's own popovers, clipped by the window as they were by the modal —
Obsidian's are native `<select>` popups, which the platform draws in a window of their
own and never clips.

### The vault manager, and a menu that would not close, 2026-10-29

Two reports against the switcher that had landed that morning: its menu stayed open
when the footer row was clicked a second time (it flickered and came back), and
"Manage vaults..." should raise a window to pick a vault from, the way Obsidian's
does.

**The menu that would not close** is the ordering of two handlers. The menu closes on
`on_mouse_down_out`, and a click on the footer row *is* a mouse down outside the menu:
the press closed it, then the click that ended the same press ran the row's toggle and
opened it again. Anything built this way has the bug, so the fix is the one a row with
a menu hanging off it needs: the row tracks its own hover, and the out-handler returns
without closing while the pointer is on the row, leaving the press to the row's own
toggle. The flicker was the frame between the two.

**The window** had never been read, only inferred: the switcher probe knew only that
`app:open-vault` left no modal behind. It leaves no modal because it opens a *window* —
`app://obsidian.md/starter.html`, a CDP page target of its own with no `app` in it, so
`cdp.mjs`'s `connect()` (which waits for a booted vault window) cannot see it.
`tools/obsidian-probe/vault-manager.mjs` drives the vault window to open it, attaches to
the new target by hand, and reads it (`results/2026-10-29-vault-manager.json`).

Three findings shaped the build. A row is the folder's **name over its parent folder's
path**, with the full path as the tooltip — not the full path twice. The list is **`ts`
descending**, which is *not* the switcher menu's order: the same registry draws its rows
in document key order in the menu and most-recent-first in the window, and the second
probe run proved it by sorting the file on disk six ways and matching only that one. And
the ellipsis is a seven-row menu; Scoria draws the three that do not move the user's
folders about (Copy vault ID, Reveal vault in system explorer, Remove from list) and
leaves "Rename vault..." and "Move vault..." for their own change.

Where Obsidian offers Sync, Scoria's third row is the sandbox vault, so the two rows the
switcher's tail used to carry both live in the window now; the menu's tail is Obsidian's
single "Manage vaults...". Obsidian had also pruned the missing vault's entry from
`obsidian.json` by the time the window was up; Scoria drops the row and keeps the entry,
on the grounds that a vault on an unplugged drive should not be forgotten.

There is no input automation on this desktop, so the ellipsis menu could not be clicked
to check where it lands: the window was built once with the first row's menu forced open
behind a throwaway environment variable, screenshotted, and the hook taken out again.

### The vault switcher, 2026-10-29

Reported as a bug: the switcher's menu was "showing duplicate vault paths" — two
dozen rows all reading `vault`. The registry was innocent; its 43 entries were 43
distinct folders, most of them scratch vaults a probe or an agent session had opened
under `/tmp`, and a row is the folder's name. So the question was what Obsidian does
with a list like that, which [[Specification#Observed shell typography and chrome, Obsidian 1.14.4 (installer build 1.13.7)]]
had been guessing at since 2026-10-21. `tools/obsidian-probe/vault-switcher.mjs` registered two vaults
named `vault` under different parents and one whose folder had been deleted, opened
the menu and read it (`results/2026-10-29-vault-switcher.json`).

Obsidian draws the duplicates exactly as Scoria did: a row is a `.menu-item-title`
holding the folder's name, with no path, no `aria-label` and no icon, so two vaults
named `vault` are two identical rows. What it does differently is drop a vault whose
folder is gone — Scoria listed it faint — and list the rows in the registry
document's key order rather than by recency, which the spec had claimed and
`VaultRegistry::vaults` had sorted for. Both are now matched: the sort is gone (a
vault keeps the place it was first opened in, a new one goes last), missing folders
are filtered out of the menu but kept in `scoria.json`, and the path stays as the
row's tooltip, which Obsidian has no equivalent of. Obsidian's tail row is a single
"Manage vaults..." opening its vault-manager window, which Scoria did not have then;
it does now, and the tail matches — see the next note.

### The explorer's context menus, 2026-10-28

The UI-rules pass of the same day captured the file-explorer menus but did not
build them, and filed an entry saying nine of the items were commands Scoria
has not got. Auditing that before building turned out to matter: **four** are.
`ExplorerEvent::OpenInNewTab` and `Workspace::open_in_new_tab` were already
there for Ctrl+click, tab groups had landed so "Open to the right" is a split
with a file of its own, `cx.write_to_clipboard` was already used in three
places, and `Search::set_query` behind `show_search` makes "Search in folder"
two lines. So the job was mostly drawing after all.

- **The model is a list, not a drawing.** `menu_items(&Target)` returns one
  `MenuItem` per row -- label, lucide icon, `Section`, and the
  `ExplorerEvent` it emits -- and `separators_above` puts a separator wherever
  the section changes, with **two** above `Danger` for the empty section
  Obsidian emits there. Nothing writes a separator down one at a time, so the
  menus cannot drift out of the shape the capture recorded, and both are a
  unit test that reads as the spec's own table. `Target` is `File`, `Folder`
  or `Blank`, which is also the whole of what the right-click has to decide.
- **The empty space below the last row** is Obsidian's third menu, the four
  `action-primary` items rooted at the vault. `uniform_list` hands an event
  outside every row straight through, so the list is wrapped in a div that
  takes the right-click, and a row's own handler calls `cx.stop_propagation()`
  so exactly one of the two menus opens.
- **Five icons** were cut from Lucide's source: `clipboard`, `folder-open`,
  `folder-search`, `separator-vertical` and `pen-line`. Two names in the
  capture are Obsidian's older aliases and resolve to drawings under current
  names -- `lucide-edit` is `square-pen`, already bundled, and `lucide-edit-3`
  is `pen-line` -- both read out of Lucide's own `aliases` metadata rather
  than guessed, the same trap the callout icons hit with `check-circle-2`.
- **Four new handlers.** "Open to the right" factored `split` into
  `split_beside`, where `None` duplicates the active tab (what a split is) and
  `Some(path)` is a fresh tab on that file with no history or view state to
  inherit; the flush and stash stay *above* the tab construction, because the
  `view_state` a split copies is the one the stash just wrote. "Copy path"
  writes the vault-relative path and shows no notice, because Obsidian shows
  none. "Open in default app" is one `std::process::Command` per platform and
  adds no crate. "Search in folder" writes `path:"<folder>/"`, the trailing
  slash keeping `Notes/` from also matching `Notes archive/`. `new_canvas` and
  `new_base` grew a `folder` argument; the commands still pass the vault root.
- **Obsidian's file row has no creation items.** Scoria's offered "New note"
  and "New folder" on a file -- the one thing in these menus that was wrong
  rather than missing. A test asserts no file-row item begins with "New ".
- **The socket reports the menu.** `state.docks.left.explorer.menu` carries
  the target, its path and the rows a reader would see, `"---"` per separator.
  Without it a harness could right-click and not read back what came up, which
  is what had kept these menus untestable from outside the process -- the same
  fix `state.modal.rows` got on 2026-10-24.

Driven through the release binary in a headless sway session, so nothing on
the desktop was touched. The three menus read back exactly as the capture
records them. The file menu's rows were then mapped by *effect* rather than by
guessing their geometry -- clicking every third pixel down the menu and
recording what changed -- which produced the bands below, and they are the
menu: nine items, every separator where Obsidian has one, and the doubled
separator above Rename visibly twice the width of the singles.

```
+  6..+ 33  a tab opened        Open in new tab
+ 36..+ 60  a group appeared    Open to the right
+ 63..+ 69  -                   separator
+ 72..+ 99  a file was copied   Make a copy
+102..+156  a prompt opened     Move file to... / Bookmark...
+159..+165  -                   separator
+168..+195  (menu closed only)  Copy path
+198..+204  -                   separator
+207..+234  (menu closed only)  Open in default app
+237..+252  -                   separator x2   <- 16 px against 7
+255..+297  a prompt opened     Rename... / Delete
```

Two rows show only that the click landed. "Copy path" is the clipboard
limitation the backlog already records under
[[UI parity backlog#Blocked by the harness, not by the code]]: a Wayland
client cannot own the selection without a real input serial, and a headless
session driven by synthetic events has none, so `wl-paste` reads back empty
however the handler behaves. "Open in default app" has no desktop handler to
answer it there. Both are pinned by unit tests on the string each produces
rather than by the harness.

What is left out, and why, is
[[UI parity backlog#Five context-menu items wait on features Scoria has not got]].
Two payloads the capture never recorded are marked OBSERVE in the spec: what
Obsidian's "Copy path" actually writes, and what its "Search in folder" puts
in the box.

### The settled UI rules, 2026-10-28

The backlog's "Settled by the probes of 2026-10-25 — the UI rules" entry held
fifteen answers and, it said, no code. That turned out to be half wrong: the
entry had gone stale, and eleven of its fifteen items had been built by the
work of the week after it was written without anyone striking them off. So this
pass began with a verification sweep rather than with code, and the sweep is
the reason it is worth writing down — it found three bugs that the stale text
had been hiding, one of them bad enough that it had been quietly wrong for
every vault.

What was already there, verified item by item against the running tree: the
callout icons for all 28 types, the ephemeral fold chevron and the absent
affordance on a callout with no `-` or `+`, the setext underline at body size,
the struck-through and muted done task, clearing every property down to
`\nbody\n`, Ctrl+E swallowed inside a text input, `graph:open-local` not
offered with no file open, the local graph's own scale 1, the collation tie
broken by the full path descending, and three of the four property-pill
gestures. Six of those eleven had no test pinning them, which is how an entry
like this goes stale in the first place.

- **The vacuous `All`**, the worst of what the sweep found. `foo OR` matched
  *every note in the vault*: the dangling branch parsed to a `Query::All([])`,
  and `eval`'s `All` arm returned true with no children to check, so the `Any`
  above it was satisfied by everything. `Any([])` already answered false, so
  the asymmetry was the bug. It is fixed in the matcher rather than in
  `Search::is_empty`, because `foo (` and `foo AND` do hold a matcher —
  Obsidian runs the search and prints "No matches found.", a real zero and not
  a gated pane — and because every other consumer of a `Search` inherited the
  same default. One of them was doing so visibly: a graph colour group with an
  empty query coloured every note in the graph. That now reads the empty query
  explicitly, the way the graph's search filter already did, so the behaviour
  is stated where it happens instead of falling out of the matcher.
  A dangling `-` no longer searches for a literal hyphen either, and `foo (`
  keeps its empty group instead of having it flattened away. All ten of the
  spec's shapes are a fixture now, asserting the match outcome rather than the
  parse tree, since two of the three bugs parsed fine and failed at eval.
  Whether `AND` is an operator at all is still unprobed and deliberately not
  implemented: the probe vault's `foo` notes contain no `and`, so the observed
  zero is consistent with "dangling operator" and with "ordinary word that
  matches nothing", and only the trailing shape is built.
- **The callout colours were the light theme's, in both themes.**
  `Theme::callout_color` took `&self` and never read `self.dark`, so every
  callout but `quote` was the wrong colour on the theme Obsidian ships. The
  values it needed were all already in the tree — `--callout-color` is a
  `--color-<name>` token, the same six the canvas presets and the highlight
  washes draw from — so the hue is picked by type now and the value comes from
  the theme. `CanvasColors`' own doc comment had claimed those presets were
  "the ones `Theme::callout_color` has for the same hues" since the canvas
  landed; it is true as of this pass. A fourth test locks the callout hues and
  the highlight washes to one source so they cannot drift apart again, and it
  caught the light blue being `0x086dff` here and `0x086ddd` in
  `highlight_color` for the one token — the spec records `#086ddd`.
- **The property pills take Backspace twice.** The gesture is a pure state
  machine (`PillStep` in `properties.rs`), and `Field::backspace` returns it
  as a `#[must_use]` rather than offering a plain delete beside it: the old bug
  was exactly a key path taking the character route and skipping the gesture,
  so now no caller can. Press one selects the last pill and writes nothing,
  press two issues the existing `remove_item` change, and the walk backwards
  rebuilds the trailing input from the row as it then stands — `ListEdit`'s
  items are a pre-removal snapshot, and committing one would write the removed
  item back. Enter now leaves the input open the way Obsidian does, with the
  bytes it writes unchanged, and the remove control is `lucide-x` instead of a
  `×` character.
- **The empty tab** is Obsidian's: `New tab` wherever its name is shown — the
  tab strip had it, the view header and the window title still said "No file is
  open" — four pills (`New note Ctrl + N`, `New base`, `New canvas`,
  `Go to file Ctrl + O`) each running the command it names through the existing
  registry, so `hotkeys.json` applies and a pill whose plugin is off is left
  out; no mode switch in the header; and a status bar that reads `0 properties`,
  `0 words`, `0 characters` rather than nothing. The `self.report("No file is
  open")` status messages are a different string for a different thing and were
  left alone. The logo stays above the pills, where Obsidian has pills only.
- **The view-header title renames in place.** A single click puts the caret at
  the clicked offset, typing edits while the file keeps its name, Enter commits
  through the existing `rename_with_links`, Escape abandons. It is not a
  `TextInput`: that paints its own bordered box and exposes no way to place a
  caret, so the title has a small model of its own (`title_field.rs`) and the
  header paints a shaped line in a `canvas` so a click maps to a byte offset.
  The resting look is unchanged plain header text. Two gpui traps showed up
  and are worth remembering: a `canvas` host has no intrinsic height, so the
  title vanished until it was given one, and a focus handle only joins the
  dispatch tree with the frame that paints it, so the first click needs
  `window.on_next_frame` — the same rule as
  [[#The frame, the type and the icons, 2026-10-20]].
- **Both location dropdowns** are selects now rather than text fields. The
  attachment one writes the four shapes Obsidian writes (`/`, `./`,
  `./Attachments`, `Attachments`) and reveals Subfolder name or Attachment
  folder path off the *selected option*, so the dependent row appears the
  moment the option is picked and all three are absent at the default
  settings; the first pick on an empty `app.json` leaves exactly
  `{"attachmentFolderPath": "/"}`. Where each shape sends a file is
  `scoria_vault::ops::attachment_path` — a leading `./` is relative to the
  note's folder, anything else is vault-absolute, `/` and `""` are both the
  root — with the spec's six rows as tests. It has no caller: Scoria creates
  no attachments yet, which is also why all three rows keep their
  `not_yet()` tag. `Honored::No` is "not read at all yet: the value is written
  for Obsidian", and a resolver with no caller honours nothing; the tag the row
  shows reads "not in Scoria yet", and that is still true. It comes off with
  the first caller.
- **A click that dismisses a dropdown is spent on the dismissal**, and Escape
  over an open list closes the list rather than the modal. Both are pure
  decision functions (`popover_swallows_click`, `escape_closes`) called by
  every clickable in the modal, so the next control added cannot forget them.
- **The spec gained two corrections and lost one wrong row.** "Default location
  for new notes" was recorded as "the same four values as attachments"; the
  2026-10-25 capture itself holds that row as *three* options over
  `newFileLocation`'s own enum (`root`, `current`, `folder`), which is what
  `Specification.md` says now. The half-typed-query prose that ended "Scoria
  gating only a wholly empty query is right for the first group and wrong for
  the second" is now the record of what was built, with the `AND` question
  marked OBSERVE. The callout table says which of its values were read and
  which are the same token's light value rather than an observation.

Left undone on purpose, each for a stated reason: the reading view's pills
still have no remove control, because `PropertiesHost` is built only by
`EditorElement` and a dead × is worse than its absence (the handoff is an
event beside `TaskToggled` plus one subscription, written down in
`reading/view.rs`); the explorer's context menus stay a short subset, because
the captured menu wants `Open in new window`, `Merge entire file with...`,
`Open version history`, `Open in default app` and `Show in system explorer`,
commands Scoria has not got, and listing a row that cannot run is the thing
the palette refuses to do; the empty tab's header has no Bookmark or More
options, which no view header in the app has yet; and the `.inline-title` in
the note body is a separate element and a separate job.

### Folding, 2026-10-26

Four backlog entries said the same thing from four sides: no heading or list
folding in the editor, no heading folds in the reading view, `foldHeading`
with no consumer, and `editor:toggle-fold-properties` a no-op from a preview
leaf. All four are closed, to
[[Specification#Observed heading fold behavior, Obsidian 1.14.4]] and its
2026-10-25 pass; the extents in both tables are unit tests in
`crates/scoria/src/fold.rs`, and the commands, the hover indicators, the
reading-view chevron, the mode-switch hand-over and the relaunch were driven
through the release binary on the probe's own note.

- **`fold.rs`** holds the model: `foldables` walks the one parse for the
  lines that fold — top-level headings through the line before the next of
  the same or a higher level, trailing blanks included and the last one to
  the end of the file; list items with lines under them, the blank line
  after excluded; nothing in a blockquote or callout, no setext underline,
  no heading on an unterminated last line. A fold is remembered by a
  `FoldKey` (kind, text, which of the lines with that text), resolved
  against the foldables after every parse: an edit above a heading moves
  its fold with it, an edit to the heading's text re-keys it by line, a
  heading that goes takes its fold away. `fold-more` and `fold-less` walk
  outward and back, as observed.
- **The editor** keeps `folds`, the resolved `hidden` runs and the
  foldables from each parse. Hidden lines take no height, prepaint steps
  over them, a vertical move passes them, and the folded line shows `…`
  (a display-only piece of the `DisplayLine`) in Live Preview and source
  mode alike. The indicator is a chevron 15 px left of a heading line and
  10 px left of a list line, shown on hover and always while folded; a
  mouse-down on it toggles. A fold command leaves the caret where it is,
  as CodeMirror does, so `fold-less` from inside a folded section still
  finds it; a caret that *arrives* in a fold — a link, a search hit, an
  outline row, an edit — opens it. Only the parse of the current text
  settles the folds: a stale parse's foldables would drop them.
- **The reading view** collapses a heading's section — the blocks after it
  to the next heading of the same or a higher level return empty, so they
  are absent rather than hidden — behind Obsidian's
  `heading-collapse-indicator`: a chevron 28 px into the left margin,
  revealed on hover, that only its own click toggles; the heading's text
  and the space beside it do nothing, as observed. The heading is drawn by
  the view itself so its margins sit outside the box the chevron is placed
  in, and the box reaches into the margin so hovering the chevron counts.
  List folds do not collapse anything here; they ride along in the key set.
- **The two views share one set.** A mode switch hands the keys and the
  properties fold across, so the editor's fold collapses the same section
  in reading view and a section collapsed there is folded on the way back.
- **The record** is Obsidian's `{"folds":[{"from","to"}],"lines"}`, per
  note, in `note-fold/<vault id>.json` under Scoria's configuration folder
  (`~/.config/scoria`, where `scoria.json` lives) and never in the vault.
  It is written on leaving a note and on quit, read as a note opens, and the
  editor re-derives each `from` line's extent from the heading — a stored
  `to` is advisory, as observed. A rename carries the record with the
  note. `{0,0}` is the properties fold, kept and restored with the rest.
- **`foldHeading` and `foldIndent`** gate the two kinds at parse time; with
  `foldHeading` off the reading view draws no chevron.
- **`editor:toggle-fold-properties`** from a preview leaf now folds the
  reading view's own properties block.

What this build decided on its own is listed in
[[Specification#What building folding settled, 2026-10-26]].

### The settled editing commands, 2026-10-26

The backlog's "Settled by the probes of 2026-10-25 — the editing commands"
entry held thirteen answers and no code. All of it is implemented now, to the
bytes in
[[Specification#Observed editing command behavior, Obsidian 1.14.5 (installer build 1.13.7)]]
and the 2026-10-25 block of
[[Specification#Observed auto-pair behavior, Obsidian 1.14.4]]; each shape in
those tables is a unit test in `scoria-text`, and the whole set was driven once
through the release binary over the control socket on a scratch vault, which
read back the same bytes.

- **Fourteen command ids join `RUNNABLE`**: `editor:insert-callout`,
  `-codeblock`, `-mathblock`, `-horizontal-rule`, `-tag` and `-footnote`
  (`format::block_insert_plan`); `editor:cycle-list-checklist`
  (`cycle_checklist_plan`); the six `editor:toggle-highlight-<colour>`
  commands; `editor:clear-formatting` (`clear_formatting_plan`, which walks
  the one parser's tree, so `snake_case` and the inside of a code span are
  safe); and `editor:set-heading`, registered as a command that runs and
  changes nothing, with a comment saying that this is what Obsidian does.
- **Mod+L inserts `- [ ] `** on a line without a checkbox — after a list
  marker, a `> `, leading whitespace — and the caret moves with it; over a
  selection the decision is taken once (flip all, add to all, or invert
  each), blank lines ignored. The "no checkbox to toggle" notice is gone.
- **The cycle** is bullet → `[ ]` → `[x]` → bullet, the marker kept, a
  custom status discarded on the first press. Over a selection with a line
  that is not a list item every line is pulled back to a bullet; otherwise
  each line advances one step — the converging rule where Obsidian's own
  output oscillates.
- **Highlight colours** write one emoji after the opening `==`. The parser's
  `Highlight` node now carries `color: Option<HighlightColor>`, taking the
  emoji off its first text node, so the reading view paints `<mark
  data-highlight="red">` without showing it; the HTML renderer writes the
  attribute. The wash is the theme's `--color-<name>` at 0.3 alpha; only red
  was measured (`oklch(0.659737 0.216875 23.8105 / 0.3)`, the dark theme's
  `#fb464c`), the other five take the canvas preset tokens. Live Preview
  still shows the emoji in the source text; whether Obsidian's does is
  unprobed.
- **Inline toggles wrap each line** of a multi-line selection (blank lines
  skipped, mid-line edges kept), `toggle-comments` excepted; a bare caret
  takes the word under it. After a multi-line wrap the selection covers the
  wrapped text, markers included, so a second press strips every line.
- **Auto-pairing is the one rule**: a bracket closes when the character
  ahead is whitespace, the end of the line or a closing bracket; a quote or
  marker needs that and whitespace or the start of the line behind. No
  context suppresses it. Skip-over is provenance: `TextBuffer::planted`
  holds the closers the editor wrote, shifted by every edit, dropped by
  undo, by a vertical move or a click (`forget_planted`) and by any edit
  that lands on them, so `x|)` typed `)` writes `x))` and `[[Ty|]]` plus
  `]` `]` steps out. The old unmatched-opener scan is gone.
- **Shift+Tab** takes one unit of display columns off, a tab counting
  `tabSize`, and writes the rest back as tabs then spaces (`useTab`) or
  spaces; with `useTab: false` the unit — for Tab too — is four spaces
  whatever `tabSize` says, as observed. Enter on an empty nested item keeps
  its own one-step rule, which the probes did not touch.

What this build decided on its own is listed in
[[Specification#What building the editing commands settled, 2026-10-26]].

### Math, 2026-10-26

`$x$` and `$$x$$` showed their TeX in the accent colour; they are now typeset.
The engine is [RaTeX](https://github.com/erweixin/RaTeX) 0.1.15 (MIT), a
KaTeX-compatible TeX math engine in pure Rust that was chosen over the
alternatives on crates.io for coverage: the whole of KaTeX's math syntax plus
mhchem, with a display list as output rather than an image. Obsidian uses
MathJax, which accepts more (`\require`, `\newcommand` living across
formulas, some AMS extensions); where the two disagree the formula fails to
parse here and is shown the way a syntax error is shown there.

- `scoria-math` (no gpui) wraps the engine: `typeset(tex, display)` parses,
  lays out in display or text style and flattens RaTeX's `DisplayList` into
  a `Formula` of glyphs, rules, rectangles and paths in em units, with the
  baseline at `y = height`. Every result, errors included, is cached
  process-wide behind a mutex, because a frame asks for the same formula
  several times and the views have no state of their own to keep it in. The
  KaTeX faces (SIL OFL, twenty TrueType files, 590 KB) come through
  `ratex-katex-fonts`; `Face` names the family, weight and style, and
  `rasterize` draws a character of one at a device size and subpixel offset
  with `ab_glyph`. A glyph is named by the character, not a glyph id: RaTeX
  lays bold and script letters out by their Unicode mathematical
  alphanumeric code points, but KaTeX's files keep those glyphs at the ASCII
  letter, and `katex_ttf_glyph_char` undoes that. Black, the engine's
  default, becomes "the text colour"; a `\textcolor` keeps its own.
- The faces are **not** registered with gpui's text system, and the first
  build did exactly that and drew nonsense: gpui's cosmic-text backend drops
  any face whose cmap has no `m` when a family is loaded (a guard against
  icon fonts), which is every KaTeX size face, AMS, Caligraphic and Script,
  so big operators, matrix parentheses and `\mathbb{R}` fell back to Inter;
  and the KaTeX italic files set no italic bit (`fontdb` reads every one as
  `Normal`, angle 0), so the family matcher handed `KaTeX_Main` regular out
  as the italic file and digits and `\text` came out slanted. Both are
  properties of the files and the backend, not of this code. Registering
  them would also have put twenty `KaTeX_*` families in the Settings font
  pickers.
- `scoria::math` paints a `Formula`: each glyph as a bitmap (the text colour
  with coverage as alpha, straight BGRA as gpui takes images), snapped to a
  quarter device pixel horizontally and a whole one vertically and placed so
  its pixels land on the grid one to one, cached per face, character, size,
  subpixel variant and colour; quads for rules and boxes; `PathBuilder` fill
  or stroke for radicals, stretched delimiters and wide accents. Rules are
  never thinner than a device pixel. A `FormulaElement` is the fixed-size
  element the reading view's `$$` block and the Live Preview widget lay out.
- **Inline math is the hard part**: gpui shapes text and nothing else, so a
  line has no slot for an element. An inline formula is therefore laid out
  as a run of no-break spaces (figure space, no-break space, narrow no-break
  space, measured in the text font at the size in force) at least as wide as
  the formula, which wraps, selects and maps clicks like text; the formula
  is painted over the run afterwards, on the row's baseline and centred in
  the run's slack, which is under the narrowest of the three spaces. In the
  reading view `Renderer::rich_at` rebuilds the paragraph string with the
  placeholders, carries the other runs and the links across the edits, and
  wraps the `InteractiveText` in an `Overlay` element that reads the
  `TextLayout` after prepaint for where each placeholder landed. In Live
  Preview `preview::display_line_with` takes a placeholder provider and
  substitutes the whole `$...$` span (the Sub piece maps every source byte
  of the formula to the placeholder's start, so a click lands inside the
  construct and reveals it); `DisplayLine::math` records each formula's
  display range, `shape_display` grows the row for a formula taller than the
  font's ascent and descent, and the paint pass draws the formulas after the
  text. The editor measures the three spaces at the start of every prepaint,
  so the first frame already has placeholders. A formula that does not parse
  falls back to the old behaviour there (dollars hidden, source in the
  accent colour); in the reading view the source is shown in MathJax's error
  red.
- `$$x$$` on one line inside a paragraph is display math: the reading model
  splits the paragraph at it (`Segment::Math`) and draws it as a centred
  block, as MathJax's `display="true"` container does; in a heading, a table
  cell or Live Preview's line it stays inline, in display style.
- A `$$` block wider than the note scrolls sideways, as MathJax's container
  does, instead of wrapping or clipping.

Seen over the control socket on a scratch note and on the demo vault's
`Vesiculation` and `Magma viscosity` notes, which now carry a formula each:
inline fractions, sums with limits and `\sqrt{2}` on the baseline in the
reading view, in a heading at the heading's size and in list items; a `$$`
block centred with a full-height integral and matrix parentheses from the
size faces and `\mathbb{R}` from the AMS face; `aligned` rows; a one-line
`$$...$$` splitting its paragraph; both error forms in red; the same note in
Live Preview with the matrix line and the wide line grown to hold their
formulas; the cursor placed inside `$e^{i\pi}$` bringing its source and
dollars back; a click on the rendered `\sqrt{2}` landing on its opening
dollar; the dark theme painting the formulas in its text colour.

Not probed against Obsidian, and said so in `Specification.md`: the error
rendering (MathJax's `merror` is remembered as red source text; the message
is in a tooltip there and shown nowhere here), the vertical margins of a
display block, whether an inline formula grows its line in Live Preview the
way it does here, and a one-line `$$x$$` in Live Preview, which stays on its
line here in display style where Obsidian's CodeMirror shows a block widget.

### PDF view, 2026-10-26

A `.pdf` opened on the "not supported yet" placeholder; it now opens in a
viewer, and `![[x.pdf]]` draws the same viewer inside a note. Four pieces,
built in parallel and joined on a shell that was written first:

- `scoria-pdf` rasterizes pages with [hayro](https://github.com/LaurenzV/hayro)
  0.8, a pure-Rust PDF renderer (Apache/MIT; its own test suite is the pdf.js
  and PDFBox corpora). `Document::open` parses the bytes, `render(page,
  scale)` gives straight RGBA over white, `outline()` walks `/Outlines`
  through named destinations and GoTo actions, `title()` decodes the Info
  dictionary, `page_text()` is a text-only `Device` for find, and
  `parse_subpath` reads `#page=3&height=400`. `Document` is `Send + Sync`;
  nothing panics on garbage, truncations or an empty file. A text-only A4
  page renders in 3–5 ms at 1x and 10–12 ms at 2x in release. The fixtures
  are typst output (`tests/fixtures/pages.typ`) plus two hand-written PDFs
  for a rotated and a blank page.
- `scoria::pdf::documents` is the one place a file is parsed and a page
  rendered: an app-wide cache filled on the background executor, answering
  "not yet" until the work lands and refreshing the windows then. A page is
  an `ImageSource::Custom` closure, so an `img` asks for its page on first
  paint and nothing renders for a page nobody drew; rendered pages are
  bounded at 256 MB and the least recently drawn are let go, telling gpui to
  drop the texture too.
- `scoria::pdf::view` is the leaf: the 38 px toolbar, the 140 px sidebar
  with thumbnails and the outline tree, the find bar, pdf.js's zoom ladder
  and its page-number rule, Right/Left paging, `#page=` from links and the
  socket, and `state.pdf` on the control socket.
- `scoria::pdf::embed` is the same viewer in a box: one page tall by
  default, `#height=` otherwise, lazily built, scrolling its own pages until
  it runs out and then the note's; the canvas's file card draws it too, at
  the node's `subpath` page. Live Preview needed nothing: an embed-only
  paragraph already renders through the reading renderer.

Nothing of the chrome was guessed: `tools/obsidian-probe/pdf.mjs` ran 36
steps against Obsidian 1.14.5 first, and
[[Specification#Observed PDF view behavior, Obsidian 1.14.5 (installer build 1.13.7)]]
is what the view was conformed to. Three of its findings shaped the build.
**The leaf is file-only**: `workspace.json` carries
`{"type":"pdf","state":{"file":"doc.pdf"}}` and never a page or a zoom; those
live in pdf.js's own `localStorage['pdfjs.history']`, outside the vault, so
Scoria writes nothing either and keeps a tab's page and zoom in memory for
the session only (the same reasoning that put file recovery outside phase
3). **There is no zoom select and no rotate**: two step buttons on a ×1.1
ladder rounded to tenths, and a Display options menu; `Ctrl+=` is the
window's zoom, not the viewer's, so it is not bound here. **An embed is a
whole viewer**, toolbar included, sized to its first page — which is why
the embed and the leaf share one drawing rather than the embed being a
picture of a page.

The probe cost one trap worth recording: pdf.js's history is keyed by the
document's fingerprint and shared across vaults, and a rotation left in it
by the first pass came back on every later open of the same fixture, in
leaves, embeds and canvas cards alike. The clean `b` steps of the results
file are what the spec reads. [[Specification#What building the PDF view settled, 2026-10-26]]
lists what is this build's reading rather than an observation.

Not built, and said so in the menus: two-page spreads, Adapt to theme, the
find bar's settings toggles, text selection with its three copy strings
(there is no text layer), the sidebar resizer, the hover popover, the
embed's *Open* menu (the button opens the file), *Save current position in
document* on a canvas card (the probe saw it do nothing), match
highlighting, and password-protected files (`Error::Encrypted`).

On the method: the split was five agents — the probe, the crate, the view,
the embeds, and this write-up — with the renderer's API frozen in the brief
and the cache written before the view and the embeds started, so the two
could build against the same thing without waiting on each other. Two
things went wrong that the next split should know. An agent that starts a
long command in the background ends its turn when the command does not,
and nothing wakes it; long commands belong in the foreground with a
timeout. And a usage limit cut all three builders off at once mid-run;
each resumed from its transcript without losing work, but a shared icon
file was deleted by one and restored by another in the gap, which is the
kind of thing a frozen file list prevents.

### Bases: the rest of the view, 2026-10-25

A `.base` is four layouts over one engine, and the view drew one of them,
read-only. The rest landed in a day, in one shape: the shell (`bases::view`)
holds the parsed file, the showing view and the table it resolved to, and each
layout adds its own methods and its own state to it. Every layout draws the
**same resolved table**, so `groupBy`, `groupOrder`, `sort` and `limit` are
answered once in `scoria-bases` and nothing in the app reorders a row.

- `scoria-bases::layout` reads a view's layout keys off its own map and writes
  them back as Obsidian writes them: an integral number bare, a default
  dropdown value as an *absent* key, and a layout change keeping the previous
  layout's keys in place. All three are observed.
- **cards** is a wrapping grid, **list** a virtualised line per row, **kanban**
  a column per group with each column virtualising its own cards. A card's
  image comes from the `image` property, which the engine evaluates beside the
  cells because it is not one of the columns.
- **the toolbar** is six panels rather than menus of rows, which is what the
  run observed, and every write goes through `edit_view`/`edit_base`.
- **editing through the table** writes the *note's* frontmatter, planned over
  that note's own bytes, so its CRLF, its BOM and every key the edit did not
  name survive.
- **an embedded base** resolves through the same engine on the file's first
  view, with a per-model cache: one resolve of a 20 001-row view is 66 ms, and
  a note with two embeds of it builds in 112 ms.

Two decisions are worth keeping. A note is written by the **workspace**, not
by the view: a base's row is another file, it may be open in a buffer with
unsaved edits, and a write behind that buffer's back would be thrown away by
its own next save — so the cell reports the change it wants and the workspace
guards it, the way the unlinked-mention write does. And `bases::writes` holds
one implementation of each view write, because the toolbar's panels and the
column header's menu both write `sort` and `order`; consolidating the two
copies turned up a checkbox that collapsed a view with no `order` to a single
column.

The work was split six ways over the files — the layouts, the toolbar, the
editing, the commands, the embed — which worked because the shell was built
and frozen first and each piece owned its own file. What it cost is the
duplication the split invited: four helpers and the scroll-body walk existed
twice before the consolidating pass moved them into `bases::cells`, and two
surfaces had grown their own `prepend_sort`.

Almost nothing in this build is a new observation: the runs rendered a table
and nothing else. [[Specification#What building the rest of the Bases view settled, 2026-10-25]]
is therefore a list of **readings** rather than of behaviors — what a card's
title is, what a kanban column's header says, whether a cell can be edited in
place at all (the one captured cell control is a *disabled* checkbox, which
argues against it), and three toolbar controls left inert because what each
writes is not derivable. "Hide all" is the sharpest of those: `order` is
exactly the shown columns, but an absent `order` shows the implied ones, so a
list meaning "none" has no spelling. Each is one click of a probe.

### Bases table view, 2026-10-25

A `.base` opens on its table instead of the "not supported yet" placeholder.
Read-only: the file is drawn, never written. Six pieces, which is also how the
work was split —

- `scoria-bases::table` resolves a view against the vault: the base's filters
  and the view's own, the columns of `order`, the cells, `sort`, `groupBy` and
  `limit`. It reads no vault; the caller hands it one `FileData` per note.
- `scoria-bases::cell` is what one cell shows, which is **not** `toString()`:
  null and a boolean are empty, a List joins with a bare `,` while each element
  keeps its own rendering, a Link is its text.
- `scoria::bases::rows` is the bridge from the vault's scan and cache to those
  values, with no disk reads at all — 114 ms over 20 000 notes, against the
  2 s index gate and the 256 ms the load itself costs.
- `scoria::bases::view` draws it: a 30 px header row, rows in a reset-not-
  spliced variable-height list, a disabled checkbox for a boolean, group
  headers, the parse-error banner.
- The workspace wires it as it wires the canvas: `MainView::Bases`, a
  `FileKind::Base` arm in `show_file`, `release_bases`, the socket's `bases`
  state, a `lucide-layout-list` leaf.
- `tests/table_conformance.rs` holds the observed rules, and
  `docs/scoria-demo/Rocks.base` and `Volcanoes.base` are the dogfood.

A base is a view over the *whole vault*, which is the one place it differs from
every other view in the shell: a canvas card is rebuilt when its own file
changes, but any change at all may change a base's rows. So `after_changes`
invalidates every group's base, and the table follows the vault live — adding a
note that matches takes the demo's table from 8 rows to 9 without reopening it.

What the build settled against the observations is in
[[Specification#Observed Bases table view, Obsidian 1.14.5 (installer build 1.13.7)]]:
the header's `displayName` lookup is by the *qualified* id, a `file.*` column's
header is Obsidian's own label rather than the id's last segment, an empty
`.base` is one default table of the whole vault, and the no-value group leads
under an ascending `groupBy`. The one thing nobody should mistake for observed
is `groupOrder`: it is in the schema and in the view object's own property
list, but nothing in either probe run ever wrote one, so the implementation's
ordering is a guess with a probe named for it.

### Bases evaluator, 2026-10-25

`scoria-bases` evaluates a statement as well as parsing one. The crate reads no
vault: the host implements a `Context` and fills in a `FileData` per row, so
nothing here depends on `scoria-vault` or on gpui, and a test builds a row in
memory.

The ratchet is the point, as it was for the parser. The same CDP run that
captured 277 parse trees also captured what Obsidian *answered* for each
statement it evaluated, so `tests/obsidian_values.tsv` holds 1044 of those —
the statement, the rendered text and the `lucide-*` type tag, or Obsidian's
exact refusal — and the harness pins the clock to the run's own second and
rebuilds the run's row. 1166 observations were read and 122 dropped with a
reason each: the wall clock and `random()`, the values the probe could not
serialize (a Link or a File alone: `String(value.data)` cannot say which it
read), and the steps keyed by a label rather than by a statement, which is the
trap the parser's extractor hit.

Three rules decide most of the behaviour, and all three are observed. **Null
absorbs**: a missing property is null and null answers every call and every
access with null, with exactly three exceptions — `isEmpty()` is true,
`isTruthy()` is false, `toString()` is the String "null". **Comparison coerces
where arithmetic refuses**: `"3" > 2` is true while `1 + true` is
`Invalid operator between Number and Boolean`. And **comparison against null is
three-valued**: `nosuchprop > 1` is null rather than false, where `"abc" > 1`
*is* false — two failures that are easy to collapse into one and must not be.

What the build itself settled, against the summary written the day before, is
in [[Specification#What building the evaluator settled, 2026-10-25]]: the
refusal order every function follows, `this` as the base file's own context
rather than a File, `key` and `value` as what an Object's `filter` binds, a
Tags value naming itself `List`, and eight smaller corrections. The one that
cost real work is the Duration: its properties are fractional *totals*, and
`months` is the duration laid on the calendar from the Unix epoch, which is the
only reading that reproduces the recorded `9.761398643219833` — a fixed-length
month gives 9.7449, 9.7513 or 9.8868.

What is not built: the view. Nothing in the app calls the evaluator yet.

### The active tab joins the view, 2026-10-22

The tab strip drew its outline as a bottom border, which ran under the active tab
too and cut it off from the view header. The outline is now a 1 px hairline laid
along the strip's bottom edge: the active tab reaches that edge and paints over it,
so it joins the header below in one colour, and the other tabs stop a pixel short
and keep the line under them, as Obsidian's root tabs do. The join has Obsidian's
feet too: outside each bottom corner a 10 px region (the top radius again) between the
tab's edge, the strip's bottom and a quarter-circle, painted as a path in the tab's
colour so the tab's edge curves out into the header. Only that region is painted: a
first cut covered the square with a strip-coloured rounded block, which showed as a
square whenever the neighbouring tab's hover wash lit up under it. They are painted
deferred so that wash stays underneath the curve.

### Tab hover, 2026-10-22

Scoria washed the whole 34 px tab outline on hover; Obsidian washes a smaller rounded
box inside it. Probed over CDP first (`tools/obsidian-probe/tab-hover.mjs`, four tabs,
each hovered in turn, the close button, the active tab and a dock tab, both themes,
with a screenshot of the strip per state): the tab is padded `1px 4px 4px` around
`.workspace-tab-header-inner`, a 4 px-rounded box padded `0 3px 0 6px`, and hover
paints `--background-modifier-hover` on that inner box only. The title keeps its
colour, the close button appears and brightens its cross under the pointer without a
wash of its own, the active tab shows nothing, and no divider hides: the divider is
the inner box's `::after` at the tab's right edge in `--tab-divider-color` (#3f3f3f
dark, #dadada light), bare only around the active tab. Two earlier notes fell: the
spec's "a hovered tab hides its own divider" and its #555555, both read off a
screenshot. Findings in [[Specification#Observed shell typography and chrome, Obsidian 1.14.4 (installer build 1.13.7)]]'s "Chrome"
paragraph; raw data in `results/2026-10-22-tab-hover.json`.

Built: `render_tab_strip` pads the tab 1/4/4 and puts the title and close button in
an inner box that takes `bg_alt` through `group_hover("tab")` when the tab is not
active; the divider no longer hides on hover; the close button lost its hover disc
and its cross goes `text_muted` → `text` through a `tab-close` group; `tab_divider`
is now the measured token in both themes. Seen over the control socket at 1.45x:
the hovered tab's wash is a box inset from its outline, measured at gray 54 over the
strip's gray 40, which is 6.7 percent white, the lines on both sides of it still
there, nothing on the active tab. The strip's 2 px gap between tabs went the same day:
Obsidian's tabs touch, and with the gap the divider on a tab's left edge sat 6 px from
the box on its left and 3 px from the one on its right. The tabs now touch, the line
straddles the edge (`left(-0.5)`), and at 1.45x it measured 6 device pixels from the
hovered box on either side; the new-tab button keeps its 8 px by margin.

The strip's right end followed (`tools/obsidian-probe/tab-list.mjs`, findings in the
spec's "The strip's right end"). Three changes: the divider moved to each inactive tab's
right edge, Obsidian's `::after`, so a hairline now parts the last inactive tab from the
new-tab button; the new-tab button, the tab-list button and the sidebar toggles sit at
Obsidian's y 8.66 and y 8 (margins 7 and 8 under the strip's bottom) instead of 5, which
puts them on the titles' line, measured at 1.45x as plus, chevron and titles all centred
on logical y 45.5; and the strip gained the tab-list button, `chevron-down`, whose menu
is Obsidian's: Stack tabs (runs `workspace:toggle-stacked-tabs`, which Scoria still
reports as not available), Bookmark N tabs... (the group's tabs with a file, through
`BookmarkDraft::Tabs`), Close all (`close_group`), then a row per tab with a check on the
active one. Two more Lucide icons came in for it, `layers` and `file`. The menu hangs off
a `relative` wrapper around the button like the explorer's sort menu; a wrapper lays the
anchored menu out *below* the button, so the offset is the 2 px gap, not the button's
height.

A regression on the way, caught by the user: once the divider moved to the tab's right
edge, the next tab, painted later with an opaque background, covered the half of the
line astride the edge, and whether any sliver survived depended on where the edge fell
between device pixels at 1.45x. One line showed, its neighbour did not. The divider is
now `deferred`, like the active tab's feet, so it paints after every tab.

New tab, probed the same day (`new-tab.mjs`, the second of four tabs active): Obsidian
appends the new tab at the strip's right end and activates it, from the button and from
`workspace:new-tab` alike, and a second press appends another after it. Scoria had been
inserting it right after the active tab; `new_tab` now pushes to the end, which
`open_in_new_tab` inherits.

### Explorer keyboard, 2026-10-21

Probed first again: `tools/obsidian-probe/explorer-keyboard.mjs` found that a click
which opens a file hands the keyboard to the editor — so the first run's arrow keys
all landed in the editor — and the second run entered the tree through
`reveal-active-file` instead. The findings are
[[Specification#Observed file explorer keyboard behavior, Obsidian 1.14.4 (installer build 1.13.7)]]:
arrows walk the visible rows without opening anything, Enter opens, Right opens a
folder or steps into it, Left climbs to the parent and closes it, Escape goes back
to the editor, F2 renames and Delete asks.

Built: `FileExplorer` holds a `FocusHandle` under an "Explorer" key context, with
the row the keyboard is on kept by path (`focused`, a 2 px border in
`border_focus`, the new theme colour). `file-explorer:open` focuses the tree where
it was, `reveal-active-file` puts the ring on the open file; a folder click or a
click on the open file's row focuses the tree, and an `Open` from a row or Enter
hands the keyboard to the main area through `focus_main`, since `request_focus_main`
only moves a parked keyboard. The socket's `focus` method and the MCP tool take
`explorer`, and the state dump's `explorer` carries `focused`. Seen over the control
socket: six Ups from Alpha to a folder, Right opening it and Right again ringing its
first child, Escape back to the editor, Enter opening the next row with the editor
focused, Left at the root doing nothing and F2 opening the rename prompt.

Also on the way: `run-block.sh` killed whatever Obsidian was running when it refused
to start because one was running. It now only kills the one it launched.

### Files header, 2026-10-21

The Files pane's `nav-header`, probed first: `tools/obsidian-probe/explorer-header.mjs`
drove Obsidian over CDP through the five buttons, every sort order, the auto-reveal
toggle with a note opened in a collapsed folder either way, and the Expand all /
Collapse all swap, and the findings are
[[Specification#Observed file explorer header behavior, Obsidian 1.14.4 (installer build 1.13.7)]].
What that settled that a guess would have missed: folders keep their name order under
every sort, the six orders live in the leaf's `sortOrder` in `workspace.json` and never
in `app.json`, the last button's label follows the tree (one folder opened by hand
makes it "Collapse all"), and turning auto-reveal on reveals the open file at once.

Built: `SortOrder` in `explorer.rs` with Obsidian's strings and labels; `FileTree`
keeps its folder tree and re-lists the files of each folder on `set_sort`, by name,
`mtime` or the new `VaultFile::ctime` (the birth time, the mtime where the filesystem
has none); `collapse_all` and `expand_all` lost their `cfg(test)`; the header is a
40 px row of 28 by 24 icon buttons with tooltips, the sort menu hangs under its
button with a check on the order in force, and the auto-reveal button wears the
accent while on. `FileExplorer::follow` replaces the unconditional `reveal` where a
file becomes the open one: the row is highlighted, and only with auto-reveal on are
its folders expanded and the row scrolled to. `set_tree` no longer reveals anything,
so a write elsewhere in the vault no longer moves the list. The two keys are read
from the file-explorer leaf in `workspace.json` at restore and written back in place
at save, in Obsidian's key order (`explorer_leaf_state_is_read_and_written_in_place`).
The header's New note runs `file-explorer:new-file`; New folder opens the existing
prompt at the vault root, where Obsidian's inline rename is still a backlog entry.
The socket's `explorer` state gained `sort_order`, `auto_reveal` and `any_expanded`.

Seen working in the app over the control socket on the probe's own vault: the menu
with its check, the rows in `byModifiedTime` order, the toggle revealing
`Projects/Deep/Inner.md` through two collapsed folders and leaving `Archive/old.md`
alone once off, Collapse all, both creation buttons, and the two keys coming back
from `workspace.json` after a quit. One fix on the way: a menu item's click also
reached the explorer row under it until the menu was `occlude()`d.

### The frame, the type and the icons, 2026-10-20

Three asks in one session: lose the desktop's title bar, make the shell look typeset, and make
sure the panels collapse. The last was already true (`app:toggle-left-sidebar`,
`app:toggle-right-sidebar`, the toggles in the title row), so the work was the first two plus
the one panel thing Obsidian had and Scoria lacked: a draggable sidebar width.

**What Obsidian does was probed first** ([[Specification#Observed shell typography and chrome, Obsidian 1.14.4 (installer build 1.13.7)]]).
Three CDP scripts read the running app's computed styles, bounding boxes, icon class names and
`workspace.json` writes. Two traps cost a run each: `require('obsidian')` is not in the renderer's
scope, and **Obsidian 1.14 opens Settings in a popout window**, so `document.querySelector('.setting-item')`
in the vault window finds nothing — the rows are reached through
`app.setting.activeTab.containerEl`, which is a different document. The "Window frame style"
setting turned out to live in the Interface tab, to default to **Hidden** on Linux, and to write
nothing under `.obsidian` whichever option is chosen, so there is no vault key for Scoria to honour.

**The frame.** The window opens with gpui's client-side decorations and a transparent background,
and `workspace::client_frame` wraps the shell the way gpui's `window_shadow` example does: a 24 px
margin that holds a soft shadow and the resize edges (the pointer gets the right cursor from a
hitbox canvas, a press on the margin calls `start_window_resize`), a one pixel border, and none of
it on an edge that is tiled or maximised, so a snapped window meets the screen without a gap.
The margin was 12 px until 2026-10-21, with a shadow of sigma 12 at 40 % that the margin cut off
while it was still an eighth black, which read as a dark band around the window. Obsidian's
hidden frame measures black at 32 %, sigma 13, pushed 12 px down, in margins of 23 px at the
sides, 15 above and 46 below ([[Specification#Observed shell typography and chrome, Obsidian 1.14.4 (installer build 1.13.7)|the spec records the profile]]);
gpui's client inset is one value for all four sides, so Scoria keeps Obsidian's side margin and a
shorter shadow (sigma 11, 6 px down, the same 32 %) that is as dark at the content's edge and gone
before the margin ends.
Since 2026-10-21 the corners are rounded 10 px, which is what Obsidian's hidden frame measures on
this desktop (`--window-border-radius` is unset in its CSS, so the rounding is the frame's own):
gpui clips children to rectangles, so `WindowCorners` works out which corners meet no tiled edge
and every element that reaches one — the workspace's own root box, the ribbon, a sidebar, the tab
strip, the close button, the status bar, the Settings backdrop — rounds that corner itself, as
Zed's title bar does. The root box is the one that is easy to miss: it paints `theme.bg` over the
whole content, so left square it fills each corner with a notch outside every rounded child, and
the window looks square with a dark fleck in the corner. A canvas
tab paints its own background and still squares a corner it reaches with the ribbon hidden and the
left sidebar collapsed. The
40 px title row is Obsidian's hidden frame: the left sidebar toggle above the ribbon, the dock tab
headers and the tab strip, the right toggle at the right end of whichever header is rightmost, and
three 44 by 40 window buttons overlaying that header's right end (which gets 132 px of padding to
stay clear). Empty parts of the row are `window_handle`s: a press drags the window, a double click
zooms it, a right click opens the compositor's window menu. `--frame native` (or
`SCORIA_FRAME=native`) asks the desktop for its own frame instead, and the Interface tab shows the
choice as Obsidian's "Window frame style" row, inert until a per-user settings store exists.

**The type.** The app bundles Inter's six static faces (OFL, `crates/scoria/assets/fonts`) and
registers them before the window opens, so the shell no longer depends on what the machine has,
and `Theme` defaults both families to it; `appearance.json` fonts still override. The palette is
the one Obsidian computes: #1C1C1C / #282828 / #333333 with #dadada, #b3b3b3 and #666666 text
in the dark theme, #ffffff / #f6f6f6 / #e4e4e4 with #222222, #5c5c5c and #ababab in the light,
the accent `hsl(258, 88%, 66%)`, and hover and active rows a 6.7 percent wash of white or black
rather than a flat colour. Sizes are `theme.rs` constants now — 13 px UI small, 12 px smaller,
15 px medium, 40 px headers, 44 px ribbon, 16 px icons, 300 px sidebars — and headings use the
observed scale (h1 1.618em at 700 down to h6 1em at 600, 1.2 and 1.3 line heights) in the reading
view and in Live Preview alike. The status bar became Obsidian's bottom-right tab rather than a
bar across the window.

**The icons.** Every control that Obsidian draws with a Lucide icon names it in the DOM
(`svg.lucide-search`), so Scoria fetched those drawings from Lucide's own repository (ISC,
`crates/scoria/assets/icons`) and serves them through an `AssetSource`; `assets::icon(name, size,
color)` is the one way to draw one. The ribbon, the dock tabs (icons only, like Obsidian's), the
tab strip's close and new-tab buttons, the view header's arrows and actions, the explorer's folder
chevrons and the window buttons all use them. Where Obsidian uses an icon of its own (the sidebar
toggles, backlinks and outgoing links, the folder triangle) Scoria picks a Lucide neighbour
(`panel-left`, `arrow-down-left`, `arrow-up-right`, `chevron-right`), which the clean-room rule
requires and which reads the same.

**The sidebars.** Each dock is `workspace.json`'s `left.width` / `right.width` wide (300 when the
file has none), and a 3 px handle on its inner edge — a hairline at rest, the accent under the
pointer — drags it between 150 px and what leaves the main area room. The width is written back
only once a drag moved it, so a file's `300` stays the integer Obsidian wrote. `state.docks.*.width`
reports it over the socket, and a drag is driven with `mouse down/move/up` (the MCP tools have no
drag).

Verified on the desktop over the MCP server: the window has no KWin bar, maximise and restore work
from the corner buttons and the shadow goes away while maximised, the right toggle and the window
buttons move onto the tab strip when the right dock collapses, and a 120 px handle drag widened
the left dock. Not done, and filed: the explorer header buttons (#24), the tab list button, the
view header's bookmark and "More options" actions, and a store for the frame style.

### Tabs, the layout file, the ribbon, the Hotkeys tab and the graph panel, 2026-10-20

The five largest entries left in [[UI parity backlog]] after the campaign were builds rather than
fixes, and they went in one after another, each driven over the control socket on a scratch copy of
this vault before it was committed.

**Tabs.** A `Tab` is a path, a mode and its own history. The active tab's fields are the ones the
workspace always had — `current`, `mode`, `history`, `history_ix` — so every route that opens a
note is unchanged, and `tabs[active_tab]` is a snapshot refreshed before anything reads it. That
shape was chosen over accessors because seventy call sites read those fields and none of them
needed to know. The editor still holds one buffer: switching tabs saves and reloads, the way opening
a note does, which is also why a tab switch loses the scroll position the way history navigation
does (the open entry on history covers both). The last close leaves Obsidian's empty "New tab", and
a closed tab remembers its index so `undo-close-pane` puts it back where it was.

**workspace.json.** Reading it is a walk of the leaves under `main`, depth first across groups —
Scoria has one strip, so a split layout is flattened to show it. Writing it back is the decision
the backlog had left open under the vault-is-truth rule; the answer is to write only what the
session changed and only what Scoria models: a restored leaf keeps its id and every key Obsidian
gave it, a new tab gets a sixteen-hex-digit id, `lastOpenFiles` is brought up to date, the dock
`collapsed` keys follow the toggles, and a main area with more than one tab group is left exactly
as it was, because flattening it on disk would lose the user's layout the next time Obsidian
opened the vault. The quit write goes through the merging config writer, so an unchanged layout is
byte-identical. The diff of the scratch vault's file after a session that closed every tab was
one leaf, `active`, and the order of `lastOpenFiles`.

**The ribbon** is five command buttons and Settings; `left-ribbon.hiddenItems` hides by the plugin
id its key starts with, and a disabled core plugin's entry is not drawn. The sidebar toggles were
the real gap: with the ribbon hidden, a collapsed left dock had no way back from the window.

**The Hotkeys tab** turned up a writer bug worth remembering. The merging `write_config` keeps a
key the new value lacks — right for every config file that is a struct, wrong for `hotkeys.json`,
which *is* a map of entries, where dropping a command's entry is what restores its defaults. The
first live run showed the tab on defaults while the file still held the custom chord. The fix is a
second writer, `Vault::replace_config`, that writes the value whole, used for that file alone. The
chord itself is a gpui `Keystroke` turned into Obsidian's spelling: `KeyboardEvent.key`
(`ArrowLeft`, `F5`, ` ` for space, letters upper-cased), modifiers alphabetical, Ctrl as `Mod`,
Super as `Meta`. Rebinding is a clear of every key binding and all of them again from the rebuilt
registry; one false alarm during the run was pressing Escape and the new chord in the same socket
request, before the frame that restores the workspace's handlers after a modal closes had painted.

**The graph panel** replaces the chip row. Its sliders are canvases with their own hitbox, and the
panel occludes, which is what keeps a slider drag from also panning the graph: the graph's canvas
used to accept any mouse-down inside its bounds and now asks its hitbox. `graph.json` is written a
third of a second after a change. A vault without the file gets the twenty keys in Obsidian's
order — the test pins it at the 494 bytes Obsidian's own creation write measured — and a vault
with it gets the changed keys merged in so `0.518713248970312` keeps its digits; whole numbers are
written without a decimal point the way JavaScript prints them, and the last group removed is
written as `[]`, both of which the merge would otherwise have got wrong. Force changes reach the
running layout's `forces` and reheat it; filter and group changes rebuild; display changes repaint.

### The parity campaign, 2026-10-20

[[#Control socket and MCP server, 2026-10-19]] ends with the first thing the socket caught: three
backlog entries that were one bug seen from three panes, settled by asking the running app what the
editor did after a heading jump. This is what happened when that question was asked of the whole
window.

**What the socket made possible.** Every flag under [[#Commands]] arranges a state at startup and
then the app is on its own, so an audit over those flags is an audit of screenshots: you can see
that something looks wrong and you cannot ask why, or what the app thinks it just did. Over the
socket a finding is a transcript — the keystroke, the `state` before, the `state` after — and the
reviewer reads the same transcript the auditor did. Two measuring tricks came out of the run and
are worth keeping, because the state dump exposes no *rendered* text (`Editor::describe` reports
the cursor, selection, scroll and visible lines, never the `DisplayLine`), so colour, weight, pills
and labels are simply not observable from outside. The first is a y-scan: click every second pixel
down the editor and read back which line each point resolves to, which gives every row's exact
height — a 24 px text row, a 56 px collapsed callout, a ~104 px image widget — and so separates a
laid-out widget from a line of text without a screenshot. The second is an x-scan: sweep across one
row and map display columns back to source columns, which shows exactly which bytes are hidden or
substituted, and settles a question like `Note#Heading` versus `Note > Heading` to within a
character width. Both are race-free. The other thing the socket bought is plain parallelism: eight
agents, one subsystem and one app each, at the same time.

**The shape of the audit.** Eight probes ran in parallel — Files, Live Preview, reading view, right
dock and graph, search and the modals, Settings, the source editor, the workspace shell — and every
finding then went to an adversarial reviewer whose main job was the *Obsidian* half. That is where
the previous pass had failed: of its four refutations, all four were Scoria behaving correctly
against an Obsidian behaviour the auditor had remembered rather than observed. The pass confirmed 82
findings over seven subsystems and refuted seven, and two of the refutations
are worth remembering by anyone who drives the app this way, because they were the harness, not the
app: "Tab types a literal tab into the find field" and "Shift+Enter inserts a newline into the
prompt" both come from gpui's `Window::dispatch_keystroke`, whose first line is
`keystroke.with_simulated_ime()` — it fabricates a `key_char` for `tab`, `enter` and `space`
whenever control, alt and platform are absent, and *shift is not excluded*. No user can reproduce
either. A socket-driven claim about a key needs that check before it is filed.

**The accident: there were two audits.** That first pass did not finish in one sitting — it hit a
session limit partway and was resumed. The resumed probe agents did not replay from cache; they
re-ran, and what came back was a genuinely independent second audit of the same window, worded
differently and reaching 92 findings where the first reached 82. Nobody noticed at the time. The
fix campaign took the first pass plus the second pass's Live Preview file — which is the only Live
Preview file either run produced — and worked from those 95, while the second pass's other 79
findings sat unread. That is a process error, and the arithmetic it produced is what gave it away:
the counts in the notes did not add up.

It was also the luckiest thing that happened. Two blind audits of one window, graded against each
other, say more than either alone. Triaging the second pass's 79 against the *finished* binary
gave a result that is worth more than a re-read of the first pass's list: 27 were already fixed, 52
still reproduced, and the triage's evidence is against the current code rather than the code as it
was, so where the two passes overlap its line numbers and its repros are the ones to act on. It
corrected the first pass in both directions — it narrowed the properties-in-reading-view finding to
one third of its filed scope, settled the `0` chip question against the backlog's own prediction,
showed that the published fix sketch for the empty-rename error pointed at the wrong file, and
caught a fix the first verification pass had scored as complete when only one of its two routes was
closed. And it found, by a route neither audit had used, the worst bug in the campaign. Two
independent passes is not a cost to apologise for; the thing to fix is that only one of them was
read.

Seven waves of fixes followed, each wave a set of agents partitioned by file ownership, with
`cargo fmt --all`, `cargo check`, `cargo clippy --workspace --all-targets` and
`cargo test --workspace` as the gate between waves. Two fleets of verification agents then drove
the rebuilt release binary: the first found 35 fixes confirmed, 12 partly done, one not done and 11
regressions, and the second re-drove the repairs and swept the window unscripted.

**What the fixes found that code reading had not.** The Writer gate turned out to be the sharpest
tool in the campaign, because `drive_session` now runs every new editing command over all 1000
corpus notes and over the real vault. It caught two byte hazards nobody had predicted: the first
`swap_lines` happily swapped the last content line with the phantom empty line a file's trailing
newline creates, which would have stripped that newline from every file it touched, and
`delete_paragraph` can empty a note to zero bytes — correct behaviour, but the harness's "the
session wrote something" guard had to be made honest about it. Auto-pairing produced a second one
only a test could see: wrapping two *touching* selections wrote `(alpha()beta)`, because
`apply_edits` sorts back to front and an opener and a closer landing at the same offset are
order-sensitive; writing the wrap as one edit over the whole selection removed the whole class.
The frontmatter-link fix was reproduced as a unit test before it was written, and the test showed a
self-contradiction inside one cache — `resolved_links("Source.md")` counted the property link while
`backlinks("Target.md")` was empty. And the setext fix proved a negative: dumping what
`highlight_lines` actually emits showed the clipped heading span covering the whole of *both* rows
(`0..2` over `==` and `0..2` over `==`), so a pure function of one line's text and spans cannot tell
a title row from an underline row, whatever heuristic it uses. That is what moved the fix into the
parser, where `BlockKind::Heading` has carried a `setext` flag all along and `collect_blocks` was
throwing it away.

**The three things only an unscripted pass could have found.** The second verification fleet was
told to drive the rebuilt binary the way a person would rather than to re-run the repros, and that
is where the campaign's own damage surfaced. Two of the three were regressions this campaign
introduced, and both compiled, passed clippy and passed every test.

The first is focus after a delete. The fix for "typing into a deleted note is silently discarded"
parks the keyboard on the workspace handle when the main area goes empty — correct — and nothing
ever hands it back, because `open_file` never called the new `focus_main`. So: delete the open
note, then open any other note by any route — the switcher, an explorer row, history — and the
editor paints the note while `focus` is still `{workspace, ["Workspace"]}`, `type` answers
`{"target":"none"}` and the arrow keys answer `handled: false`. Every note opened for the rest of
the session is keyboard-dead until the user happens to click inside the editor. No test could have
caught it: focus lives in a painted frame's dispatch tree, and `crates/scoria` has no gpui window
harness.

The second is an empty list property. The property-pill work routes a click on a list-valued row's
value cell to `PropertyAction::EditItem { index: items.len() }` — the empty input Obsidian keeps
after the pills. For `aliases: []` the pill strip has no children, so the flex container has zero
height, so gpui gives it no hitbox, so the handler never runs and the click falls through to the
editor behind. The visible result is worse than a dead cell: the next keystrokes go to the editor
caret, and the sweep that found it left `Zed, Alpha` on its own line *inside* the YAML fences —
corrupted frontmatter from two clicks and a word. The cause is layout, not the action, and the fix
is `Row::is_blank()` drawing the same faint placeholder an absent value draws, so the row has a
height and therefore a hitbox.

The third was not ours, and it is the widest: clicking any dock row — an explorer row, a search
result, an outline row, a backlink — blurs the editor. The root div's `track_focus` focuses the
workspace handle on any mouse-down outside a focusable child, and none of those row handlers asked
for the keyboard back, so the note opens and then neither typing nor the arrow keys do anything
until the user clicks inside the text. It had been true all along; nobody had written a repro for
"open a note the way a person opens a note, then try to type". All three are fixed, the first two
in wave 7 through one rule — when what the main area shows changes, the keyboard goes to whatever
that area paints, requested by the handlers that have no `&mut Window` and honoured at the top of
the next `render`.

The honest reading of that is that parallel agents editing one tree produce regressions, and the
gates did not catch them because a regression of this kind compiles and passes. The live sweep did.
Budget for it: a wave of fixes is not finished when the gate is green, it is finished when somebody
has driven the thing.

**Wave 8: the bug nobody was looking for.** The triage agents were asked, among other things, to
try the app with text that is not ASCII. One of them wrote a note holding 45 letters, a non-breaking
space, and then the search term — and the window disappeared. The log:

```
thread '<unnamed>' panicked at crates/scoria/src/search.rs:552:31:
start byte index 46 is not a char boundary; it is inside '\u{a0}' (bytes 45..47)
thread 'main' panicked at async-task-4.7.1/src/task.rs:452:45: Task polled after completion
```

The snippet windowing walked back to the whitespace before the hit with
`body[start..first].find(char::is_whitespace)` and then `start += sp + 1`, which is correct only
while whitespace is one byte; U+00A0, U+2009 and U+3000 are not, and the slice two lines later cut
inside the character. Reproduced again with the ideographic space. Note where the panic ended up:
it was raised on the background `run_search` task and still took the main thread down with it, so
*any* panic in search is fatal to the process and to whatever the user had not saved.

The second thing is the same shape without the crash, filed by the second audit pass — the one the
campaign never read — and confirmed live by the same triage. Backspace and the arrow
keys stepped by `char`, so one Backspace turned `é` into a bare `e`, split 🇩🇪 into a lone regional
indicator, or left a dangling zero-width joiner at the end of a family emoji — and two seconds later
the autosave wrote that into the vault. That is the only silent corruption either audit turned up,
and the ground rule it breaks is the first one in the repo.

Both are now fixed. The windowing moved out of the gpui crate into a pure
`crates/scoria-search/src/snippet.rs` whose `floor`/`ceil` helpers snap every window edge — and
every hit range, so a stale range widens instead of slicing — to a character boundary, with the
live panic written as a unit test *before* the fix and a second test sliding a lead-in of emoji,
CJK, a combining accent and an NBSP past the window edge one byte at a time. The agent then went
looking for siblings of the class and found one in `fuzzy.rs`, where `simple_search` mapped
lowercased byte offsets back to the candidate by counting characters; it now carries a per-byte
source map, and `İstanbul` is its test. `scoria-text` steps by grapheme cluster for Backspace,
forward Delete, Left/Right, the shifted variants and word motion, and the auto-pair rule only
treats a single-character cluster as half of a pair. Wave 8 also closed the second route into the
typeable empty state: deleting the open note was hardened in wave 1, but `Editor::new` still seeded
the name `"untitled"`, which is exactly the thing `main_view` tests to decide whether a buffer has
a file behind it — so every launch that opened no markdown, an empty vault or a vault holding only
a canvas, handed the user a live editor over nothing and discarded what they typed into it.

**The second lesson.** The two deepest defects in the whole campaign — a process-killing panic and
a silent corruption that reached the vault — were found by typing unusual text into the running
app. Not by reading code: both sites read perfectly well, and one of them had been read by an
auditor who filed four other findings in the same file. Not by a scripted repro either, because a
repro is written from a hypothesis and nobody hypothesised a non-breaking space. They came from an
agent with a licence to try odd input against a live binary. A vault of real prose is full of
pasted NBSPs, CJK punctuation, accented names, flags and family emoji, and every one of them is an
input the test corpus does not contain. The class to grep for is narrow and worth grepping for
again: an offset into user text computed by arithmetic — `+ 1`, `len()`, `chars().count()` — rather
than taken from `char_indices`, a regex engine or a grapheme cursor.

**The lesson worth keeping.** Partitioning by file ownership is what let eight agents edit one tree
at once, and it worked — the waves landed with no merge conflicts and the gate was green between
each. What it cost is every item that needed two owners. The list property is the worked example.
Wave 4a built the whole per-item model in `properties.rs`, with tests proving a comma survives the
round trip; wave 4b built the renderer in `reading/view.rs` that draws an open field in its pill's
place. Both landed, both were correct, and the user-visible behaviour did not change at all,
because the only route from a click to an open field runs through `Editor::property_action`, whose
`match` over `PropertyAction` is exhaustive — adding the variant the pills needed would have left
`editor.rs` not compiling for every other agent in the wave. Three waves of careful handoff notes
later, the item was fixed in one wave by one agent holding `editor.rs`, `reading/view.rs` and
`properties.rs` together. The same shape explains the callout chevron (fold state has to live on
`Editor`, the chevron is drawn in `reading/view.rs`), the modal focus theft (two `.occlude()` calls
in `workspace.rs`, useless without the hitbox the editor agent had already added), and the
frontmatter links (the cache half shipped a wave before the pane that reads it). The rule to carry
forward: scope a group by the *change*, not by the file, and when a change needs two files, give
one agent both — a handoff note across a wave boundary costs more than the conflict it avoids.

**Where it ended.** 501 tests pass, clippy is clean across the workspace, and the Writer gate over
the real gate vault still round-trips 14/14 byte-identical, which is the number that matters:
nothing in a campaign's worth of UI work moved a byte in a vault, and the one path that *was*
moving bytes it should not — the grapheme split — is closed. Of the 95 findings the waves worked
from, 54 are fixed. Of the second pass's other 79, 27 were already fixed by that work and two more
by wave 8, leaving 50 that still reproduce — most of them the same large gaps the first pass had
filed, a dozen of them sharper than the first pass had them, and twenty that nobody had filed at
all. All of it, with the probes the declined items are waiting on, is in [[UI parity backlog]].

### Control socket and MCP server, 2026-10-19

The flag list under [[#Commands]] kept growing, and every flag on it does the
same kind of thing: it arranges a state at startup and then the app is on its
own. `--type` can open the autocompletion popup for a screenshot, but it cannot
ask whether the popup opened, which row is selected, or what the status bar said
afterwards. [[#The probe batch, 2026-10-18]] recorded why the obvious escape is
shut: there is no `xdotool` on this machine and a synthetic event carries no
focus, so input has to be driven from inside the process — which is what
`--stress`, `--type` and `--accept` already were, self-driven input one scenario
at a time, compiled in. A socket keeps the self-driven input and moves the loop
outside the binary, so whatever drives can read the answer and decide what to
do next.

It is the Scoria-side counterpart to `tools/obsidian-probe/`. That harness
drives a real Obsidian over CDP to learn what Obsidian does
([[#Driving Obsidian over CDP, 2026-10-18]]); this one drives Scoria to check
what Scoria does against it. Two apps, the same shape of harness, and neither
belongs in CI.

Protocol 2 added three things an audit of the Settings modal and the window
frame needed: `state.modal` carries the open Settings tab's **rows** (every
row's name, description, control kind and value, and the tag saying how far
Scoria gets with it; the Hotkeys tab reports commands instead, with their ids
and bound chords), `state.window` and `window` both carry the window
**title**, and `state` carries `status_bar`, the right half of the status bar
as it is on screen.

**The protocol.** `--control <path>` binds a Unix domain socket (on Windows too, see
[[#The control socket on Windows, 2026-10-22]]) and speaks
newline-delimited JSON over it: one request per line, one response line per
request, in order.

```
--> {"id":1,"method":"state","params":{"text":false}}
<-- {"id":1,"result":{...}}
<-- {"id":2,"error":"unknown method: frobnicate"}
```

Positions are window-local logical pixels. Byte offsets are zero-based into the
note's UTF-8 bytes, like Obsidian's `Pos`, and line and column are zero-based
too. Every failure — unknown method, a parameter of the wrong type, no vault
open, a path outside the vault — comes back as a plain string in `error`.
Nothing that crosses the socket may panic.

| method | params | result |
| --- | --- | --- |
| `ping` | – | `{"ok","protocol","version"}` |
| `protocol` | – | the method table as JSON |
| `wait_loaded` | `timeout_ms?` | `{"loaded","timed_out"}` |
| `wait_idle` | `frames?`, `timeout_ms?` | `{"frames","parsing","timed_out"}` |
| `state` | `text?`, `rows?` | the state dump |
| `open` | `name` (`Note#Heading`, `Note#^id`), `mode?` | `{"current"}` |
| `open_path` | `path`, `mode?` | `{"current"}` |
| `set_mode` | `mode`: reading, live, source, graph | `{"mode"}` |
| `uri` | `uri`: an `obsidian://` or `scoria://` URI | `{"status"}` — refused when it names another vault |
| `command` | `id` | `{"status"}` — the status bar after it ran |
| `keys` | `keys`: gpui keystrokes, `["ctrl-shift-f","enter"]` | `{"handled","pending"}` |
| `type` | `text` | `{"target"}`: editor, input or none |
| `click` | `x`, `y`, `button?`, `count?`, `mods?` | `{"ok"}` |
| `mouse` | `action`: down, up, move; `x`, `y`, `button?`, `mods?` — a `move` naming a `button` is a drag in progress | `{"ok"}` |
| `scroll` | `x`, `y`, `dx`, `dy`, `unit?`: pixels, lines | `{"ok"}` |
| `set_selection` | `start`, `end?` (byte offsets) | `{"cursor"}` |
| `scroll_to` | `line` | `{"ok"}` |
| `focus` | `set?`: workspace, editor, reading, graph | `{"name","context_stack"}` |
| `window` | `activate?`, `width?`, `height?` | `{"width","height","scale","active","title"}` |
| `read_file` | `path` | `{"text","bytes"}` |
| `write_file` | `path`, `text` | `{"bytes"}` |
| `list_files` | `folder?` | path, kind and size per file |
| `list_commands` | – | id, name, hotkeys and whether it is runnable |
| `notice_clear` | – | `{"ok"}` |
| `quit` | – | `{"ok"}`, written before the app exits |

A run that opens a vault binds a second socket of its own, named after the
vault, so that the next `scoria` on it can hand its arguments over rather
than open a window — see [[#The command line, 2026-11-02]]. That socket
answers eight of the methods above and refuses the rest by name: `ping`,
`protocol`, `wait_loaded`, `open`, `open_path`, `set_mode`, `uri` and
`window`, which is what a launcher needs and nothing that presses a key,
reads the window back or writes a file. Its path is predictable, where
`--control`'s is the caller's own choice.

**The state dump.** `state` answers one object in which every key is always
present and absence is `null`, so a driver can index into it without guarding
each step. `vault` is the root, name and load state; `file` the open path, mode,
dirty flag and tab history; `tabs` the strip — each tab's path, mode and
title, and which is active; `docks` the explorer's rows and the search pane's
query, counts and results on the left, backlinks, outgoing links, tags, all
properties and the outline on the right; `modal` which of the switcher, palette, Settings, prompt or
chooser is up and its query, selection and row labels; `editor` the cursor,
selection, scroll, visible line range, line count, find panel and suggest
popup; then `reading`, `graph`, `status`, `focus` and `window`. The note's text
is there only when `params.text` is true, and list-valued keys are cut to
`params.rows` entries with a sibling `*_total` saying how many there were.

**What the MCP server adds.** `scoria-mcp` is the thing a user registers in
`.mcp.json`: a Model Context Protocol server over stdio, JSON-RPC 2.0, with no
dependencies beyond `scoria-control`, `serde` and `serde_json`. Stdout carries
the protocol, so everything it logs goes to stderr. Three things it does that
the socket cannot: it launches and reaps the app itself, so `scoria_launch`
spawns the binary with `--control`, waits for the socket to appear and then
blocks on `wait_loaded`; it takes screenshots, through `spectacle` into base64
image content; and it builds its tool input schemas from
`scoria_control::METHODS` and `params_schema` instead of hand-writing them, so
a method's parameters and its tool's parameters are the same list by
construction and renaming one cannot leave a model calling the old name. The
*table* of tools is still written by hand, though, and that is a trap worth
knowing: a method added above does **not** become a tool on its own, and a
method nobody lists is a method no model can reach. `mouse` sat unexposed that
way long enough for [[UI parity backlog#Blocked by the harness, not by the code]]
to record drag-select as unverifiable, when what was missing was the entry. After any tool that changes state it
calls `wait_idle` before returning, so the next `scoria_state` or screenshot is
current rather than one frame behind. A tool that fails is `isError: true`
content, not a JSON-RPC error — only an unknown method or malformed parameters
are those.

**Six pitfalls, in the order they are likely to bite.**

- **Focus decides where keys go.** `type` routes text to whatever is focused
  and reports which of the editor, a text input or nothing received it; `focus`
  names the focused handle and returns the key context stack. Read one of them
  before typing, not after wondering where the text went.
- **A modal captures keys.** While `modal.kind` is anything but `none`, the
  switcher, palette, prompt, chooser or Settings has the keyboard, and keys
  meant for the editor go there instead. `state.modal` carries its query,
  selection and rows, which is also how a driver tells a modal apart from a
  slow frame.
- **Autosave's two-second delay makes `read_file` stale.** Edits save on their
  own two seconds after the last keystroke, so a `read_file` straight after a
  `type` reads the bytes from before the edit. Either wait the delay out or run
  the save command — `list_commands` has its id — before reading.
- **`wait_idle` can time out with nothing wrong.** KWin stops delivering frame
  callbacks when the monitors sleep, so a wait for the next frames never
  completes; it answers `timed_out: true` rather than hanging, and the fix is
  at the desk, not in the code. Wake the display before a run that needs
  repaints.
- **Wayland reports no window origin.** `Window::bounds().origin` is always
  zero there, so `window` answers size, scale and active state and no position,
  and the screenshot grabs the *active* window (`spectacle -a`) rather than a
  rectangle on the screen. The window sets its `app_id` to `scoria` for that
  reason: KWin honours the xdg-activation token `activate_window()` asks for
  only from a window it can identify, and without it a screenshot can catch
  whatever had focus before.
- **The clipboard moves only while the window has focus.** A socket-driven
  click *does* write it -- a click on a code-block flair or the reading view's
  copy button left `wl-paste` holding the block's body -- but a Wayland client
  cannot set a selection without focus and a real input serial, so the same
  click aimed the same way while another application held focus changed
  nothing at all. `window` with `activate` first, then assert by reading
  `wl-paste`; the copy button's check reverts at 1000 ms, which is shorter
  than a screenshot round trip, so a screenshot is the wrong instrument for
  it. [[UI parity backlog#Blocked by the harness, not by the code]] records
  what this unblocks.

**The first thing it caught.** Eight readers went over the window looking for
places where Scoria and Obsidian disagree, and the list is in
[[UI parity backlog]]; what the socket added was a way to settle one of them
without squinting at a screenshot. Three of those findings were the same bug
seen from three panes: following `[[Note#Heading]]`, clicking an outline row,
a backlink or a search hit only scrolled the reading view. Asking the running
app, in Live Preview, what the editor did after a heading jump:

```
live    scroll   0 ->   0  cursor   0 ->   0  visible [0, 33]
```

Nothing. The jump went to a view that was not on screen, and Live Preview is
the default mode, so the whole of that navigation did nothing a user could
see. `Workspace::reveal_offset` now moves both views and `Editor::reveal_offset`
puts the cursor on the target, revealing it on the next paint through the
editor's own `pending_reveal` (there is no window at hand on that path, so the
scroll cannot be computed where the click is handled). The same question
afterwards:

```
live    scroll   0 ->  89  cursor   0 -> 123  visible [89, 123]
```

Obsidian also flashes the target line, which is still missing.

### Canvas writer, 2026-10-19

[[#What opening the vault in Obsidian changed, 2026-10-18]] left
`Canvas::to_json` as a known byte loss waiting for phase 4 to connect it,
with three open questions. Rather than fix it to the one observed file, the
questions went to the CDP harness first: `tools/obsidian-probe/canvas.mjs`
writes scratch canvases with every node and edge type, every optional key in
a scrambled order and unknown keys at every level, then opens, closes,
drags, moves and creates in a real Obsidian and reads the bytes back. Four
runs, a few minutes each. The answers are in
[[Specification#Observed canvas write behavior, Obsidian 1.14.4 (installer build 1.13.7)]];
what follows is what they changed in the design.

**The key order is not a table, it is an algorithm.** The first file
suggested a fixed order with `text` before the geometry. The scrambled
fixture showed the truth: Obsidian's `getData` starts from the object it
read, with the keys the model owns deleted, and assigns the model's keys
onto it — JavaScript keeps an existing key's position and appends a new
one. So `text`, `type`, `file`, `url`, `backgroundStyle` and any plugin key
stay where the file had them, and `x, y, width, height, color` always land
after them. That is why the gate vault's `id, x, y, ..., type, text` became
`id, type, text, x, y, ...`, and why a node Obsidian creates (no file order)
is written `id, x, y, width, height, type, text` — an order that is *not* a
fixed point of its own reader. `Node`, `Edge` and `Canvas` now carry the
file's key order and write through that algorithm; a node made in Scoria
carries none and comes out as an app-made one.

**Two mistakes the fixtures caught.** The first draft sorted groups first
on every write, because the loaded fixtures came back that way. The nodes
created through the API did not: Obsidian partitions on *load* and appends
afterwards, so the partition moved into the reader. The second draft
asserted that every Obsidian-written fixture round-trips byte-identical,
and two did not — the app-made ones, for the reason above. Both are now
tests that say the opposite of what the draft assumed.

**The formatter rule is `typeof`.** A container stays on one line unless
one of its values is `null`, an object or an array — JavaScript's
`typeof v === 'object'`, null included, which took a fourth run with
`[true,false]` against `[null]` to separate from "has a nested container".
Numbers follow `JSON.stringify` (`1e3` → `1000`, `-0` → `0`), so `Node`'s
geometry is `f64` now; a canvas with `"x":0.5` parsed in Obsidian and
would have been refused by the old `i64`.

**Closing is a write.** Opening a canvas writes nothing; closing it
rewrites the file through the formatter even when nothing was edited,
keeping the file's own key order. For the gate run that means a canvas
Obsidian merely looked at will show in the diff — reformatted, not
reordered — and Scoria, which writes nothing on close, will not do the
same. That asymmetry is recorded rather than copied.

**Checked against the real vault.** The harness has a new property: every
`.canvas` in Obsidian's layout is written back byte-identical. The gate
vault's `Mindmap.canvas`, the 4731-byte file Obsidian wrote
on 2026-10-18, passes; `Mindmap.canvas` is reported as authored
elsewhere, since Obsidian itself will rewrite it whole the first time it is
saved there. Nothing in the app calls the writer yet; when phase 4 does,
the bytes are settled.

### Porting two checklist blocks to CDP, 2026-10-19

The first two blocks of [[Interaction probes]] — the search pane and
properties in the reading view — were ported from hand steps to scripts
under `tools/obsidian-probe/` and run against Obsidian 1.14.4 (installer
build 1.13.7) in fresh scratch vaults. Each script runs in three phases:
`fixtures` with Obsidian down (the notes are written with `fs`, mtimes set
with `utimes`), `run` with it up, and `after-restart` if `run` ended by
quitting it — which is how "does the sort survive a restart" and "does the
fold survive a restart" are answered with a real relaunch. `run-block.sh`
owns the lifecycle: fresh vault, fresh id, `obsidian.json` swapped in and
restored byte-identical. Search took 31 steps and properties 15, no
failures, and the observations are in [[Specification]] under the two
sections' 2026-10-19 paragraphs.

**Three traps in the runner, each found the hard way.**

- The anchored kill pattern from the plugin days,
  `^/nix/store/[^ ]*electron /nix/store/[^ ]*obsidian`, never matches an
  Obsidian launched with `--remote-debugging-port`: the flag sits between the
  binary and the asar. So the kill was a no-op, the survivor's single-instance
  lock swallowed the next launch, and the properties script drove the
  **search** vault for thirteen steps while believing otherwise. The script
  had even recorded `vault: "vault-search"` in its first step and carried on.
  The runner now waits for both the process and the CDP endpoint to be gone,
  and `launch` refuses to proceed unless `app.vault.adapter.basePath` is the
  vault it asked for.
- A vault id in `obsidian.json` must be sixteen hex characters. A malformed
  one is silently ignored and no vault window opens.
- Reusing an id across folders inherits that id's IndexedDB cache: a search
  on a brand-new vault returned the previous vault's files until the re-index
  caught up. Fresh id per scratch vault, and no trust dialog is involved when
  no plugin is enabled — CDP needs none.

**The copy-results detour.** "Copy search results" lives in the result-count
menu, and a real click on it left the clipboard untouched for two seconds,
with no notice. Calling the handler directly threw on `preventDefault`;
reading its source gave `function(e){e.preventDefault(),new
x2(this.app,this.dom).open()}` — it opens a **modal**, and three scripts had
watched the clipboard and the menus without once looking for one. The modal
has a Show path toggle, Link style (None, Wikilink, Markdown link) and List
prefix (None, Dash, Asterisk, Numbered) dropdowns, a preview textarea and a
`Copy results` button; a fourth script enumerated all twenty-four
combinations and pressed the button, and the clipboard matched the preview
byte for byte. The lesson is general: after a click, also ask the DOM whether
a `.modal-container` appeared.

**A second kind of mistake, caught by reading the output.** The first
twenty-four-variant run looked complete and was wrong: it set the *first*
`<select>` for every setting, so the List prefix rows measured the Link style
control being handed values it did not have. The rows did not change, which
read as "List prefix does nothing" until the script was reread. A probe that
reports its own input back is the failure mode the verifier stage exists for;
here it was caught before that stage by noticing the rows were identical.

### Driving Obsidian over CDP, 2026-10-18

[[#The probe batch, 2026-10-18]] concluded that interaction could not be
probed on this machine, because a synthetic DOM event carries no focus and
there is no `xdotool`. That conclusion was wrong, and the correction matters
more than the batch did.

**Obsidian is Electron, so it speaks the Chrome DevTools Protocol.** Launch
it as `obsidian --remote-debugging-port=9222` and `http://127.0.0.1:9222/json`
lists the page target. CDP's `Input.dispatchKeyEvent` and
`Input.dispatchMouseEvent` are injected by the *browser*, not by JavaScript,
so they carry focus and run the same input path a person's keyboard does —
exactly what `el.dispatchEvent` cannot do. Node 24 has a built-in
`WebSocket`, so the client needs no dependencies at all; the throwaway
plugin is not needed either, since `Runtime.evaluate` reaches `app`.

The endpoint also settles the version question the batch left open: the
target title reads **Obsidian 1.14.4** while the user agent reads
`obsidian/1.13.7 Chrome/150.0.7871.47 Electron/43.1.0`. App 1.14.4 on
installer 1.13.7.

The first run typed thirteen characters and answered the whole auto-pair
item — the one [[Interaction probes]] ranked first and estimated at fifty
minutes of hand work — in about a minute, including the question the batch
had flagged as confounded: **skip-over is provenance-tracked** (Obsidian
skips only a closer it inserted; a planted `)` is pushed along) while
**backspace-deletes-both is positional** (it fires on any empty pair).
Both are now in [[Specification#Observed auto-pair behavior, Obsidian 1.14.4]].

**What this means for the rest of [[Interaction probes]].** Nearly every
block in it is CDP-automatable: real clicks on the search sort dropdown,
real typing into a property field, real mouse drags in the file explorer,
a real anchor click, a scripted quit-and-relaunch for the cold-load case.
What stays human is aesthetic judgement — how `<sup>` looks, whether a
hover affordance feels right — which is a small residue, not eight hours.

**It does not belong in CI, and the reason is not the plumbing.** These
runs assert *Obsidian's* behavior, not Scoria's: a red build would mean
Obsidian changed or the app flaked, which is not a reason to block a merge,
and Obsidian is proprietary, needs a display, and would have to be pinned
and installed on a self-hosted runner. The division that works is the one
this repo already uses for autocompletion in
`scoria-suggest/tests/obsidian_rows.rs`:

- **CI, every push:** Scoria asserted against the *recorded* observation
  tables. Pure Rust, deterministic, no Obsidian. A failure here is a real
  Scoria regression.
- **On demand, when the Obsidian pin moves:** the CDP harness re-derives
  those tables from the live app and the diff is read by a person. A change
  here is news about Obsidian, and it is the moment to update the tables
  and the spec together.
- **By hand, rarely:** the aesthetic residue.

### The probe batch, 2026-10-18

Eleven of the thirteen small-backlog items were blocked on the same thing:
the project implements from observed Obsidian behavior, and nobody had
observed it. So they were probed in one batch rather than one at a time.

**Method.** A throwaway plugin in a disposable scratch vault under the
already-trusted id `5c0r1ac0rpu50001`, eleven independent scenarios run in
sequence from `onLayoutReady`, each in its own `try`/`catch` behind a 25 s
`Promise.race`, with the result file rewritten after every scenario so a
later hang could not cost the earlier answers. A local HTTP server on
127.0.0.1:8731 served images and logged request headers. `obsidian.json` was
backed up, pointed at the scratch vault, and restored byte-identical after
each of the four launches. The gate vault was untouched: the gate vault's mtimes all
predate the run and it still matches `snapshot-2026-10-18` exactly.

Three traps, all new:

- **Obsidian's CSP blocks the plugin's own `fetch`.** The run's
  `fetch('http://127.0.0.1:8731/log')` failed with `TypeError: Failed to
  fetch` even though the renderer had just loaded twenty images from that
  same origin. Read a probe server's log from the shell, not from the plugin.
- **A dispatched click on a raw `<a>` navigates the frame.** The HTML
  scenario would have taken the whole app down mid-batch; it needs a
  bubble-phase listener that records `defaultPrevented` as Obsidian left it
  and then cancels the default itself.
- **One hung step loses the whole scenario.** `html-render` burned both a
  25 s and a 90 s budget inside a single step. Bounding every *step*, not
  just the scenario, recovered eleven of its thirteen steps.

**Version.** `apiVersion` reports **1.14.4** while the installer build and
the HTTP user agent both say **1.13.7** (`obsidian/1.13.7 Chrome/150`). The
new sections are labelled 1.14.4 with the installer build named, since the
running code is what was observed.

**Outcome.** Ten new `## Observed ...` sections in [[Specification]], every
one ending in an `**Unprobed:**` paragraph. Each section was written from a
first pass that an adversarial reader then checked claim by claim against the
raw JSON; the checks killed real errors, among them a graph.json claim about
zoom that its own timestamps contradicted, and an attachment-name claim about
percent-encoding that the run never exercised.

**No item came back ready to build.** The limitation is structural and worth
recording: there is no input automation on this machine, and a synthetic
event does not carry focus. Anything that needs a real keystroke or a real
drag — typing into a property field, opening the search sort dropdown, F2
with the explorer focused, a drag with timing — returned nothing. The probe
settled formats, storage, DOM shapes and command ids; it could not settle
interaction. Those remainders are listed per feature in [[Specification]],
and closing them wants a person at the keyboard, not a bigger plugin.

**One result reversed a planned change.** See
[[#The checkbox the padding hid, 2026-10-18]]: clicking a task with a custom
status writes `[ ]` in Obsidian too, so Scoria already matches and
`editor.rs` stays as it is.

### The checkbox the padding hid, 2026-10-18

Planning the reading view's checkbox toggle turned up a defect in the one that
already ships. Live Preview's checkbox came from `highlight.rs`, which placed
the brackets by arithmetic:

```rust
let start = item.marker.end + 1;
```

The block parser is looser than that. `- [x]` may carry **1 to 4 spaces or a
tab** between the marker and the brackets (`block.rs`, `spaces_after_marker` in
`1..5`), so `marker.end + 1` is right for one space, right for a tab by
coincidence, and wrong for every other padding. A `starts_with('[')` guard
caught the miss and dropped the span, so nothing corrupt was ever written —
the checkbox simply **was not there**: `-  [x] a` parsed as a task, rendered
without a box, and ignored the click. Confirmed before the fix across the five
paddings; only one space and a tab produced a span.

The repair is the ground rule applied literally. `ListItem` now carries
`task_span` alongside `task`, recorded in the one place that already knows
where the brackets are — the detection site in `block.rs`, where `self.offset`
is absolute and container markers are consumed, so a task inside a blockquote
or callout spans correctly for free. `highlight.rs` reads that span instead of
recomputing it, and the arithmetic is gone. One parser, one answer.

Two tests pin it: every padding the parser accepts yields exactly one span
that starts and ends on a bracket, and the shapes it rejects (`- [ ]` at end
of line, `- [ ]no space`, five spaces) yield none. Both ratchets are unmoved —
1007/1009 corpus notes and all 652 CommonMark examples — because `render_html`
never read the old derivation.

One thing that looked like a second defect is not one. Clicking a task writes
`[ ]` whenever the status byte is not a space, so `[X]`, `[?]`, `[-]` and
`[>]` all collapse to empty, and the corpus has all four
(`tests/corpus/vault/edge/Blocks.md`) while [[Specification]] commits to "task
items with any single character". That looked like data loss — but the probe
run the same day shows **Obsidian does exactly the same thing**, so
`editor.rs` is right and stays as it is. See
[[#The probe batch, 2026-10-18]].

### What opening the vault in Obsidian changed, 2026-10-18

The vault was opened in Obsidian once, to start the mixed-use half. Sixteen of
the seventeen files came back byte-identical. The one that moved is worth the
whole exercise.

**The four `.obsidian` files were all rewritten and all byte-identical.**
Every one carries a fresh mtime from the moment Obsidian opened the vault, and
every hash matches. So Obsidian rewrites its config set on open and reproduces
it exactly — which is the behavior `Vault::write_config` was built to match,
now confirmed from the other side.

**`Mindmap.canvas` changed.** The pre-open bytes were recovered
from Obsidian's own file-recovery store
(`~/.config/obsidian/IndexedDB/app_obsidian.md_0.indexeddb.leveldb`, the
canvas text sitting in the log in the clear) and they hash to the baseline
exactly, so the diff is a real one and not a guess. It has two separable
halves:

- **Geometry.** Six nodes have different `x`, `y` or `height`. That is content,
  not formatting, and it is a canvas that was open in front of a person.
- **Format, for the whole file.** 6215 bytes became 4731. The old file was
  2-space pretty-printed with every field on its own line and the key order
  `id, x, y, width, height, type, text, color`. Obsidian wrote back **one node
  per line**, tab-indented, `"nodes":[` with no space after the colon, and the
  key order `id, type, text, x, y, width, height, color`. Neither file ends in
  a newline.

So a canvas written by anything but Obsidian is rewritten whole the first time
Obsidian saves it — the same shape as a table command rewriting its whole
table, one level up. Worth knowing before phase 4 writes one.

**And Scoria could not reproduce that file today.** `Canvas::to_json` uses
serde_json's pretty formatter with a tab, which puts every field on its own
line, and `Node`'s field order puts `text` after `color`. Fed the canvas
Obsidian had just written, it returns 6011 bytes for 4731 in — every line
different. Nothing is broken by it yet, because nothing in Scoria writes a
canvas: `to_json` has no caller outside its own test, and that test only
asserts the output starts `{\n\t"nodes"` and re-parses equal, which a
semantically faithful but byte-different file passes. It is a byte loss
waiting for phase 4 to connect it, and the fix is known: Obsidian's key order
and a formatter that holds a node on one line.

### The gate run opens, 2026-10-18

Every Writer item is done, so the gate is the work. It has two halves. The
half that is ours to decide — every path from a keystroke to a file's bytes —
is the harness, and it now passes over the real vault: all thirteen
properties, `SCORIA_ROUNDTRIP_VAULT="$SCORIA_GATE_VAULT"`,
nothing written to the vault (checked after the run by mtime, as the harness
promises: every edit happens in a buffer in memory). The other half is a week
of a person using both apps on the same folder, which has not run.

A **baseline snapshot** makes that week measurable rather than remembered.
It is a copy, not a list of hashes: a hash says a file moved, and the next
question is always *which bytes*.

```
GATE=$SCORIA_GATE_DIR; VAULT=$SCORIA_GATE_VAULT
rsync -a --exclude workspace.json "$VAULT"/ "$GATE"/snapshot-<date>/   # take it
diff -r "$GATE"/snapshot-<date> "$VAULT" --exclude workspace.json      # after the week
```

Seventeen files on 2026-10-18. `workspace.json` is left out for the reason it
is left out of the harness: Scoria neither models nor writes it, and Obsidian
rewrites it constantly. A file the diff reports is the gate's evidence — then
the question is which app moved it, and whether the move was a user's edit or
an app rewriting bytes nobody touched.

A hash manifest is kept beside the snapshot for a quick check, but note that
its paths are relative to the vault root, so it only means anything run from
there:

```
( cd "$VAULT" && sha256sum -c "$GATE"/vault-2026-10-18b.sha256 )
```

The vault is not registered in Obsidian yet (`~/.config/obsidian/obsidian.json`
holds only `docs/scoria`), so the mixed-use half cannot start until it is
opened there once with **Open folder as vault**.

Three gaps came out of pointing the harness at a real vault, all in the
configs rather than the notes:

- **The config round trip ignored the vault.** `config_dirs()` named the two
  sets in the repository and nothing else, so `SCORIA_ROUNDTRIP_VAULT` covered
  a person's notes but never their `.obsidian` — the set that has what a repo
  fixture cannot invent: files missing outright, an `appearance.json` that is
  a bare `{}`, no trailing newline on any of them. It now includes the named
  vault's, and all four of the gate vault's round-trip byte-identical.
- **The test could not tell whether the vault had run.** It asserted
  `checked >= 7` against a total the two repository sets reach on their own
  (they hold eight files between them). Each set now has to contribute a file
  of its own, so naming a vault that round-trips nothing fails instead of
  passing quietly.
- **`switcher.json` is written by the app and ratcheted nowhere.**
  `Workspace` writes `app.json`, `appearance.json`, `switcher.json`,
  `core-plugins.json`, `community-plugins.json` and `types.json`; the harness
  listed neither `switcher.json` nor the other per-plugin files. All four are
  listed now, but Obsidian does not create any of them until a setting in that
  plugin is written, so no set on this machine has one and they still cover
  nothing. The same is true of `hotkeys.json`, which no set here has either —
  though Scoria never writes that one. `types.json` has its own round trips in
  `scoria-vault`.

**Thin, and worth saying plainly:** the real vault adds ten hand-written notes
to the corpus's thousand. Pointing the harness at the whole
the gate vault's parent tree finds 214 more `.md` files, but they are
`node_modules` READMEs rather than notes a person wrote — breadth of Markdown,
not breadth of vault. The corpus is still what covers the shapes; the real
vault covers that a real `.obsidian` survives a write.

### Table editing, 2026-10-18

Obsidian's sixteen table commands work: add, move, duplicate and delete
rows and columns, set a column's alignment, and insert a table. They are in
the palette, bound through their own command ids, and hidden unless the
cursor is in a table — which is what Obsidian's own check callback does.

The commands were probed the way the suggester was, by calling each
command's `editorCheckCallback` over eight shapes of table with the cursor
in two cells, then reading the note back; 186 of the 480 runs had the check
pass, and only those were used. Two traps: the check needs `editor.focus()`
first (without it `executeCommandById` returns true and does nothing), and
it only ever passes in **Live Preview** — in raw source mode Obsidian will
not run a table command at all. Scoria runs them in Source mode too, since
its Source mode is a real editing mode rather than a fallback.

The finding that shapes everything: **a table command rewrites the whole
table.** Obsidian pads every cell to its column's width (minimum three, what
the delimiter needs), adds outer pipes to a table written without them,
fills short rows out with empty cells, and grows the delimiter to match
(`:-----`, `:----:`, `-----:`). So "edit one cell" is never a small diff,
in either app — which is fine for the gate as long as Scoria pads
identically, and `scoria-text/src/table.rs` holds the observed rows as
tests.

The table under the cursor comes from the one Markdown parser, not from
looking for pipes: `BlockKind::Table` already carries the alignments and a
span per cell, so a `|` inside a code span splits a cell here exactly where
it splits one in the reading view. That turned up a real difference:
`` | `a|b` | **bold** | `` has three header cells against a two-cell
delimiter, so GFM — and Scoria's parser — call it a paragraph, while
Obsidian's table editor rewrites it into a three-column table. Scoria does
nothing there rather than turn prose into a table.

The planner is `scoria-text::table`: `tables` and `table_at` (which table,
which cell), `apply` (the text a command writes and where the cursor lands)
and `insert_table`. `TextBuffer::table_command` applies it as one undo step.
The round-trip harness runs eight of the commands over every table in the
corpus and a real vault: nothing outside the table's own bytes moves, and
undo restores the note.

Cells are editable too. Clicking one opens a field in it, the way a
property row's value opens; Tab and Shift+Tab walk the cells, Enter drops
into the cell below, Escape abandons the edit, and the column grows with
what is typed. A table now stays a grid wherever the cursor is, as
Obsidian's `table-editor` does, instead of unfolding to its pipes — Source
mode is where the markdown is edited directly.

A second probe settled what a cell edit writes, which the first had left
open: **ordinary typing re-pads nothing.** A cell changed through the
editor API kept every other byte of the table as it was, through the cursor
moving to another cell and out of the table entirely. So the padded form is
what the commands write, not an invariant, and committing a cell replaces
that cell's bytes and nothing else. Two rules of Scoria's own: a `|` typed
into a cell is written `\|` and a newline becomes a space, because either
would otherwise restructure the table (`table::cell_text`); and a commit
writes nothing unless the cell still holds the bytes the field was opened
on, so a document that moved underneath is left alone.

**Unprobed:** where Obsidian leaves the cursor after each command (the
columns it reported came from an unfocused editor and disagreed with
themselves), whether a column counts characters or display columns for CJK
and emoji — Scoria counts characters — and what Obsidian writes for a `|`
typed into a cell.

### Daily notes, 2026-10-18

`daily-notes` opens today's note, creating it from the configured template
when it is not there; `daily-notes:goto-prev` and `daily-notes:goto-next`
step between the notes that exist. All three are the legacy unprefixed
command ids Obsidian still registers.

The probe settled the three rules that matter:

- **The configured folder has to exist.** Obsidian refuses with
  "Failed to create daily note. Folder “X” not found." and writes nothing,
  rather than creating it — while folders the *format* names
  (`YYYY/MM/YYYY-MM-DD`) are created on the way.
- **An existing note is opened, not rewritten.** The probe's note had a
  byte order mark, CRLF endings and the body "hand written, do not touch";
  its bytes, size and modification time were all the same afterwards.
- **Stepping skips the empty days.** From a note on the 10th with the 9th
  missing, previous is the 2nd and next is the 20th.

Stepping needs the open note's own date, and a moment format cannot be read
backwards — `DD-MM-YYYY` and `YYYY/MM/DD` would each need their own parser.
So `daily_note_day` renders instead: it walks a window of days around today
and takes the one whose rendered path is the path in hand. The same walk
finds the neighbour, which is why a format that does not sort
chronologically still steps correctly. A format Scoria cannot render is
refused outright, because here the format is a path and a wrong one is a
note in the wrong place.

### Driving the app's writes from a test, 2026-10-18

The gate review's other finding was that no test drove a write from the app
to the disk: the harness reaches `scoria-text` and the corpus ratchets
reach `scoria-cache`, but everything the app itself writes went through
`VaultState` or GPUI with nothing exercising the bytes that land.

Four tests close most of it, in the app crate against a temporary vault:
writing a block id reaches the target's bytes and a second write of the
same plan is refused; a CRLF note keeps its endings when an id goes in;
creating a daily note writes the template expanded and leaves the template
itself alone, and a second run finds the note rather than making another;
and a daily note whose folder is missing creates nothing at all.

`Workspace::flush_save` is the one left. It needs a GPUI window, and gpui's
`TestAppContext` is behind a `test-support` feature that pulls a leak
detector and proptest — too much weight for the three lines of IO around a
guard that is itself tested in `scoria-text` and was verified in the
running app with `--stress`. Worth revisiting if the app grows more of
these.

### Line endings and the byte order mark, 2026-10-18

The specification has carried an **Unprobed** note since the reader
milestone: what Obsidian itself writes for a file whose line endings
disagree. It is answered now, and the answer is stronger than the question.

**Obsidian's editor normalizes on save.** Every ending becomes `\n` and a
UTF-8 byte order mark is dropped — not just for a mixed file but for a
uniform CRLF one, which comes back all LF after a single character is
typed. A note that is only *opened* is not touched; the normalization
belongs to the save.

Scoria keeps doing the opposite, deliberately: it writes the file's own
dominant ending and puts the mark back. That means Scoria never introduces
a difference, while a vault used in both apps drifts towards LF with no
mark, driven by Obsidian. Keeping more of the user's file than Obsidian
does is the safe side of this to be on, and it is now a recorded decision
rather than an open question.

The first run of this probe reported that an untouched file *had* been
rewritten. It had not: the probe opened each case in the same leaf, so
opening the next file saved the previous one. Re-running that case alone,
with nothing opened after it, showed the file byte-identical. Worth the
second run.

### A config file with a byte order mark, 2026-10-18

Found while closing the gate gaps, and the same class of loss. A byte order
mark is not JSON, so `serde_json` refused a config file carrying one: every
setting in it read as its default, and the first write replaced the file
wholesale — taking every key Scoria does not model with it. Windows editors
leave such a mark, so this was reachable with no misuse at all.

The mark is now stripped for parsing on both sides and written back on
save, since it is the user's byte and not Scoria's to drop. A file that is
genuinely unparseable is still replaced rather than merged into, which is
the existing documented decision and has its own test; this was never about
that case.

### Adapt to system, and an invented clamp, 2026-10-18

Two bugs found while landing the Settings dropdowns, neither of which
writes a byte, so neither was a gate problem — but the first made a
setting a lie.

`Theme::from_appearance` matched `moonstone` and sent everything else to
the dark theme, so `theme: "system"` — Obsidian's default, and what a vault
with no theme key means — rendered dark forever, while the Settings row
promised "on restart". The scheme now follows the desktop: gpui's
`Window::appearance` answers it, and since the theme global is built before
a window exists, it is rebuilt once there is one.

The other was `base_font_size.clamp(10, 40)`, a range Scoria invented. The
probe settled both halves of it: Obsidian's own slider offers 10 to 30, and
Obsidian neither clamps nor rewrites a file holding something else — 99 sat
in `appearance.json` through a whole session untouched. So the file's value
is honoured, with a floor that only keeps a zero out of the layout, and the
slider's own range is the observed one.

### The save that would not refuse, 2026-10-18

A gate review of every path by which Scoria writes into the vault turned up
the one that loses data, and it is the shape the phase 3 gate is actually
about: a note open and edited in Scoria while Obsidian saves the same note.

`Workspace::flush_save` wrote the whole buffer over the file with no
comparison against what was there, and the watcher deliberately does not
reload a note whose buffer is dirty — it reports "changed on disk; unsaved
edits kept" and returns. So Obsidian's save survived for about two seconds
and was then overwritten by Scoria's autosave, silently.

`TextBuffer` now keeps a fingerprint of the bytes it last knew on disk, set
when a file is loaded and when a save writes one. `flush_save` re-reads the
file first and refuses when it is anything else, keeping the buffer's edits
and saying so. That is the guard `VaultState::write_block_id` already had
for the other note it writes; the save path simply never got one.

Verified in the running app: with `--stress` editing and autosaving a note,
writing the file from a shell and waiting through ten seconds of further
autosaves leaves the shell's bytes on disk. The harness holds the property
over every note of the corpus and a real vault.

**Unprobed:** what Obsidian itself does when it finds a note changed
underneath. Scoria refuses and says so, which loses nothing either way;
merging or writing a conflict copy would be inventing behavior.

### Templates, 2026-10-18

`insert-template` opens a chooser over the template folder's notes — by
basename, sorted, with Obsidian's own `Type name of a template...` — and
writes the chosen one at the cursor with its placeholders filled in.
`insert-current-date` and `insert-current-time` write the same `{{date}}`
and `{{time}}` without a template around them.

What a placeholder does was observed rather than guessed, and the literal
half is the surprising one: `{{DATE}}` expands and `{{ date }}` does not,
`{{date:}}` falls back to the setting, an empty setting falls back to
Obsidian's own default, and nothing is exempt from expansion — not fenced
code, inline code, math or wikilinks. `{{title}}` is the target note's
basename, never its first heading and never a frontmatter `title`. The
whole template the probe expanded is a test in `scoria-templates`, byte
for byte, and so is the moment token table behind the date formats.

`templates.json` holds `folder`, `dateFormat` and `timeFormat`, exists only
once a setting is written, and keeps a cleared field as the empty string
rather than dropping the key; the bytes Obsidian wrote are a round-trip
test in `scoria-vault`. A format token Scoria cannot render is refused with
a message rather than written wrong, which matters more for daily notes,
where the format is a path.

The one new dependency is `chrono`, for the local civil date — it was
already in the tree through gpui, so the lockfile gains an edge and no
crate. `scoria-templates` itself stays clock-free: the instant is passed
in, which is why every test is a fixed date.

### Block references, 2026-10-18

`[[Note#^` lists the target note's blocks, and `[[#^` the blocks of the note
being edited. The query after the `^` matches a block's text or its id,
either way and whatever the case, so `^ara` finds "A paragraph." and
`^BLOCKONE` finds the block with that id. A row shows the block's text with
the `^id` taken off and the id beside it, and accepting one writes the path
exactly as it was typed with no display text — the heading rule — or
`[](Alpha.md#^id)` under `useMarkdownLinks`, with the cursor between the
brackets.

What Obsidian does was observed in the phase 3 probe (see
[[Specification#Observed block reference behavior, Obsidian 1.14.4]]):
the kinds it offers and their labels, how the caret query filters, the
six-lowercase-hex id (180 generated in a row were 180 distinct values), and
the byte-for-byte placement of a written id for a paragraph and a heading.

Every block is offered, and accepting one whose block has no id writes the
id into the *target* note — a write into a file the user never opened,
which is the shape of change the Writer gate exists to catch. The second
probe settled the placement for every kind, and there are only two shapes:
a block whose text ends a line takes ` ^id` inline, and a block that spans
lines takes `^id` on a line of its own with a blank line either side. The
id is six lowercase hex characters that the note does not already use.

The write goes through `VaultState::write_block_id`, which re-reads the
file and refuses unless the bytes are the ones the insertion was planned
against; the watcher then re-indexes it, the same path a save takes. A
block in the note being edited is the load-bearing exception: its id is
folded into the editor's own edit, so one undo takes back the whole accept
and the next save cannot write the id straight back out.

Two things are Scoria's own. Obsidian writes LF newlines into a CRLF note
and leaves it mixed, as it does for properties; Scoria writes the note's
own ending. And a block an id cannot be written onto is refused rather than
written to — `blocks::plan_id` applies the insertion and re-indexes it
before offering it, which the real vault proved necessary: a note whose
code fence is never closed runs to the end of the file, so an id written
after it lands *inside* the fence, indexes as nothing, and would leave a
dangling link in a file that had changed for it.

### Autocompletion, 2026-10-18

Typing `[[`, `![[` or `#` opens a popup over the editor. The rows are the
vault's files, their aliases, a note's headings and the vault's tags;
Enter, Tab and Shift+Enter accept one three different ways.

What Obsidian does here was observed rather than guessed. Its four
`app.workspace.editorSuggest.suggests` instances are drivable from a
throwaway plugin (see [[obsidian-behavior-probes]] in the session notes and
[[#File operations, 2026-10-17]] for the method): `onTrigger` was called for
37 shapes of line, `getSuggestions` for what each offers, and the popup's
own `scope` key handlers for what every accept key writes, across both link
styles, all three `newLinkFormat` values and a source note in a subfolder.
Three runs; the results are written up in
[[Specification#Observed autocompletion behavior, Obsidian 1.14.4]] and
encoded row for row in `scoria-suggest/tests/obsidian_rows.rs`.

What the probes settled that guessing would have got wrong:

- **Tab is not Enter.** Enter finishes the link and leaves the cursor after
  the `]]`; Tab leaves it *inside*, so a `#heading` or `|alias` can follow
  what was just accepted. Shift+Enter ignores the selected row and links the
  text as typed. On a tag, Enter writes a trailing space and Tab does not.
- **A heading pick keeps the path as typed** and never takes display text:
  `[[Sub/Gamma#` stays `Sub/Gamma` where `shortest` would have written
  `Gamma`. Every other pick is reformatted to the vault's `newLinkFormat`.
- **Display text follows the path, not the name.** `|Alpha` is added exactly
  when the path part carries a `/`, and only for notes, so `[[Sub/Alpha|Alpha]]`
  but `[[../pic.png]]`.
- **An empty `[[` is ordered by modification time**, newest first — not by
  recency of opening, which is what the quick switcher uses. The list is also
  files and aliases only: the switcher offers unresolved link targets, this
  does not.
- Nothing suppresses the trigger: it fires inside a code span, a fenced
  block, a `%%comment%%` and frontmatter alike. That is why the trigger is a
  scan back along one line rather than a parse.

The code is a new crate, `scoria-suggest`, free of gpui like `scoria-text`:
`trigger` (what the line asks for), `suggestions` (what the vault offers)
and `insert` (the bytes accepting one writes, reusing
`scoria_cache::links::link_path` so there is one link formatter). The GPUI
side is `scoria/src/suggest.rs`, a list anchored under the query, and about
sixty lines in the editor that hand the popup the keys it owns while it is
open. `VaultFile` grew an `mtime` for the ordering above; the metadata was
already being read for `size`.

The Writer gate's harness covers the new write path: accepting a suggestion
at five places in every corpus note replaces the query and nothing else, and
undoes back to the file's bytes — over the real vault too.

Ranking is Scoria's own fuzzy matcher (the one behind the quick switcher), so
the order within a query can differ from Obsidian's; what each row *writes*
is what matches.

**Not done:** block references. `[[Note#^` offers nothing rather than
offering a row it cannot write, because accepting a block with no id of its
own means writing a six-character id into the *target* note — inline on a
paragraph, on its own line after a heading. That is a write into a second
file and wants its own change. `Ctrl+P`/`Ctrl+N`, which Obsidian's popup also
binds to the selection, are left to Scoria's own bindings.

### Settings sliders and text fields, 2026-10-18

The two sliders write — font size (`baseFontSize`, 10 to 30) and indent
visual width (`tabSize`, 2 to 8) — and so do the two text fields Obsidian
offers as free text: the folder for new notes and the attachment folder
path. Each is the same shape as a dropdown's `Typed`: a key and the one
accessor pair its display is read through and its write goes through, with
a test that walks every one of them for both properties.

The ranges are the ones Obsidian's own controls offer, read off its
rendered tabs in the phase 3 probe. Obsidian neither clamps nor rewrites a
file holding something outside them — 99 sat in `appearance.json` through a
whole session — so the range bounds only what Scoria's control can set, and
a file that already holds something else keeps it.

One behavior is worth the test it got: clearing the attachment folder field
writes the **placeholder**, `Attachments`, not an empty string, while
clearing the new-note folder writes `""`. Both were observed.

The slider is a cell per step rather than a dragged thumb. The ranges are
21 and 7 values wide, so a click lands on a value without measuring the
track, and dragging can come later. The text field is the inline-field
pattern the properties row and the table cell already use, with one
`TextInput` owned by the view rather than by a row: the rows are rebuilt
from the vault on every render, so a row cannot own anything.

**Left read-only, deliberately:** the three font families and spellcheck
languages (Obsidian offers a font picker and a 58-entry select, not free
text, so a text field would be inventing a control), excluded files (a
dialog), accent colour (a colour input), themes (nothing scans
`.obsidian/themes`), and the vault name, vault folder, config folder and
language rows, which are not settings of the vault at all.

### Settings dropdowns, 2026-10-18

Seven dropdowns write: the base colour scheme, the default view for new
tabs, the default editing mode, properties in document, where new notes go,
the new link format and what happens to a deleted file. Clicking one shows
its options and picking one writes the key.

A dropdown is a `Typed`: the key, the option list, and the one pair of
accessors the control's label is read through and a pick is written
through. That is the same guarantee the toggles' `Flag` gives — a row
cannot show one key and write another — and it let the seven hand-written
label matches go, since the table now owns both directions. Two tests walk
every dropdown: one asserts a pick writes its own key and nothing else,
the other writes each option, reloads the vault and asserts the control
reads that option back.

The values came from the phase 3 probe, which also settled the question
this work was waiting on: **a setting at its default is written, not
removed**, for every kind of value. So `Vault::write_config` needed no way
of removing a key, which is what the research had flagged as the one answer
that could have forced a change there.

`livePreview` is the odd one: a two-option dropdown over a boolean key, so
it reads and writes `true` and `false` as the strings its options name
rather than getting a mechanism of its own.

The options are drawn with `deferred`, the way the file explorer's context
menu is. Without it the rows below paint over the open menu, because a
later sibling wins — this version of gpui has no `z_index`.

**Unprobed:** the option labels are Scoria's existing ones, read off
Obsidian's settings tab when the read-only modal was built, not re-observed
in the phase 3 run — that run drove the config keys rather than the
controls, so it settled what is written and not what the tab reads.

### Settings writing, 2026-10-18

Every toggle in the Settings modal writes. A click clones the config the
control reads, flips its one field and emits it; `Workspace::change_setting`
writes the file through `Vault::write_config` — which merges into the file's
own JSON, so only that key moves — stores it in the `VaultState` and pushes
what the running app can change into the open views. The banner now says
which controls write; dropdowns, text fields and sliders are still read-only.

What a toggle writes is a `Flag`: the key Obsidian writes, what the key
means when it is absent, and the accessors for it. The control's state is
read through the same flag that a click writes, so the two cannot drift
apart, and a test walks every flag of every file to check that toggling
writes that key, flips it, and touches nothing else. The one control
Obsidian words backwards ("Use [[Wikilinks]]" is `useMarkdownLinks`
negated) carries an `invert` bit rather than its own code path.

The config a toggle changes has to be mutable while the vault is open, so
`app`, `appearance`, `core_plugins`, `community_plugins` and `switcher` moved
behind locks in `VaultState`, as `property_types` already was, and are read
through accessors.

Applied on the spot, from `Workspace::apply_app_config`: readable line
length, smart lists, indent with tabs or spaces, properties in document and
strict line breaks (the editor rebuilds its widgets), plus show-all-file-types
in Files. The rest (inline title, folding, line numbers, RTL, spellcheck,
auto-pairing, Vim mode, native menus, the ribbon, tab title bars) is written
for Obsidian to honor; Scoria does not implement those yet.

Which is which is now on the row. Every item carries an `Honored`: nothing
on a setting Scoria follows as you click it, "on restart" for the theme and
the fonts (the `Theme` global is built once from `appearance.json` at
startup), and "not in Scoria yet" for everything written only for Obsidian
to read — the whole Core plugins tab included, since Scoria's own Files,
Search and Graph do not turn off with the plugins that own them in Obsidian.
The vault is shared, so writing a setting Scoria ignores is worth doing;
letting a click look like it changed something here is not. A test walks
every tab and asserts that exactly seven toggles go untagged, so
implementing one of the tagged settings means dropping its tag in the same
change.

Two rules came out of the shapes of the files themselves rather than a probe:
a vault with no `core-plugins.json` gets a whole map of every plugin at its
default, because a one-key file would read as "everything else off", and the
legacy array shape keeps its shape.

The question this note used to leave open — whether Obsidian removes a key
when a setting goes back to its default — was settled by the phase 3 probe:
it does not. Every default-valued write kept its key with the explicit
value, so what Scoria already writes is what Obsidian writes, and no way of
removing a config key is needed. See
[[Specification#Observed settings write behavior, Obsidian 1.14.4]].

### Round-trip harness, 2026-10-18

The Writer gate — the vault round-trips byte-identical after a week of mixed
use — now has a harness:
`crates/scoria-text/tests/writer_roundtrip.rs`, over the 1000-note corpus and
both `.obsidian` sets in the repository. Four properties, one per shape of
writing:

- opening a note and saving it changes no bytes;
- every editing command undoes back to the file's bytes (a scripted session
  of typing, Enter, Tab, Shift+Tab, Backspace, Delete, Ctrl+Backspace,
  paste, typing over a selection and cutting a line, at five places in every
  note, wound all the way back);
- an edit is local: typing changes the bytes at the cursor and nothing else,
  and a property write stays inside its block;
- a config file read and written back is unchanged.

Invariants are checked after every step, not just at the end: the line index
is the one the text implies, every cursor sits on a character boundary, and
what a save would write reads back as the text in hand. Setting
`SCORIA_ROUNDTRIP_VAULT` to a vault's path runs the same round trips over it
(nothing is written — the edits happen in a buffer in memory), which is how
the gate's real vault is checked without a test depending on one.

To make the editor testable the text model moved out of the GPUI view into
`crates/scoria-text`: `TextBuffer` (the text, its line index, the selections,
the undo history and one method per editing command), `FileText`, the
`lists`, `find` and edit-plan modules, all free of gpui.
`crates/scoria/src/editor.rs` keeps the shaping, scrolling, decorations, find
panel and properties table and holds a `TextBuffer`; its actions are now
`buf.<command>()` followed by `after_edit`. Nothing changed in behavior —
the suite and a 400-step stress run on a 20k-line note are the evidence.

Writing the harness found three bugs on the spot:

- **A stray `\r\n` rewrote a whole file.** `FileText` took any `\r\n` as
  "this file uses CRLF", so a note from a real vault with one such line
  among 4430 Unix ones would have had every line ending rewritten by the
  first save. The ending is now the dominant one, and a file whose endings
  disagree is kept verbatim and written back unchanged until it is edited.
- **`core-plugins.json` and `hotkeys.json` were sorted maps.** Obsidian
  writes both in its own order, so the first save would have reordered every
  line. Both are `IndexMap` now, like `types.json`.
- **A struct cannot reproduce a config file.** Serializing `GraphConfig`
  gave Obsidian's `graph.json` back with its keys in declaration order, `1.0`
  where the file said `1`, `0.51871324` where it said `0.518713248970312`
  (an `f32` field) and no `colorGroups` at all (skipped when empty).
  `Vault::write_config` now merges the new value into the file's own JSON:
  a key whose value did not change keeps its bytes and its place, a changed
  one is replaced where it stands, a new one is appended, and a key the type
  does not serialize is left alone.

Not covered here, by design: link rewriting on rename
(`scoria-cache/tests/rename_corpus.rs`) and the properties planner
(`scoria-cache/tests/properties_corpus.rs`) have their own corpus ratchets;
this harness drives the writer's path into them rather than their internals.

### Properties, 2026-10-18

The frontmatter block is now a typed, editable properties table in Live
Preview and a read-only one in the reading view.
`scoria-formats::properties` is the model: it scans the YAML into ordered
entries with byte spans, reads each value with `serde_yaml`, infers types,
serializes values the way Obsidian writes them, and plans edits as byte
ranges. `scoria-cache::note_properties` is the one place that joins it to
`extract_frontmatter`. `scoria::properties` turns entries into display rows
and holds the field-editing rules as pure functions; `scoria::reading::view`
draws the rows (read-only, or editable when Live Preview passes a
`PropertiesHost`) and `scoria::editor` owns the open field, routes keys into
it and applies a commit.

The rules came from Obsidian 1.14.4 directly, with the same throwaway-plugin
method as the file operations: `scoria-probe` in a scratch vault drove
`processFrontMatter` through about forty scenarios, dumped
`registeredTypeWidgets` and the inferred and assigned type of every value
shape, ran the three property commands, and read every file back byte for
byte. What it showed is listed in [[Specification#Observed properties behavior, Obsidian 1.14.4]] and encoded in the tests. The headline:

- Obsidian rewrites the **whole** block on any property write. A
  `processFrontMatter` call that mutates nothing still deletes YAML comments,
  turns `tags: [one, two]` into a block sequence and drops blank lines. A
  block already in that canonical form round-trips byte-identical.
- So Scoria writes Obsidian's canonical form, but only for the entries whose
  value actually changes. `plan` compares *values*, not rendered text, which
  is what keeps a flow sequence and its quoting when the value is unchanged;
  everything else in the block keeps its bytes.

`crates/scoria-cache/tests/properties_corpus.rs` is the ratchet over the
1005 corpus notes that have frontmatter: rewriting every value with itself
plans no edit, every entry span really covers its key and stays inside the
block, a real edit leaves the bytes before and after the entry identical,
clearing leaves exactly the body (plus a BOM where there is one), and adding
a property appends one line and reorders nothing. Writing the ratchet found
two bugs on the spot: bare dates were being quoted on rewrite, and an
unchanged flow list was being reflowed.

The UI: a foldable "Properties" heading, then a row per property with its
type glyph, key and value. Lists, tags and aliases draw as pills; a
wikilink value draws as the link (its alias, or the target without a
subpath) and follows on click; a tag item is clickable whether or not it is
written with `#`; a checkbox draws a box; an empty value shows a
placeholder; a value that does not fit its type is marked and kept as
written. In Live Preview a click on a key or value opens a field — the
editor routes typing, the arrows, Home, End, Backspace and Delete into it,
Enter commits, Tab commits a key and moves to its value, Escape abandons —
and a commit is one undo step through `apply_edits`. The row icon opens the
type menu, the `×` removes the property, `+ Add property` opens an unnamed
row that writes nothing until it is named. The three commands
(`markdown:add-metadata-property`, `markdown:clear-metadata-properties`,
`editor:toggle-fold-properties`) are runnable, and `propertiesInDocument`
picks the table, nothing or the raw YAML.

Choosing a type writes `.obsidian/types.json`, the first config file Scoria
writes: `Vault::write_config` serializes with two-space indentation and no
trailing newline, as Obsidian does, and `PropertyTypes` keeps insertion order
(an `IndexMap`) and its unknown keys so the file does not churn.

Two parser corrections fell out of the probe. YAML's `...` does not close a
properties block in Obsidian, and no longer does in `extract_frontmatter`.
Obsidian also does not recognise frontmatter after a byte order mark and
prepends a second block when a property is added; Scoria keeps reading
through the BOM and edits the block in place, which is a deliberate
divergence from a case where Obsidian loses the original block. Obsidian
likewise leaves mixed line endings when it writes into a CRLF block; Scoria
writes the file's own ending.

Not yet: editing properties in the reading view (Obsidian allows it there
too), drag-to-reorder, a calendar picker for `date` and `datetime`, value
autocompletion from other notes, and the Properties pane for the current
file (`properties:open-local`). The vault-wide pane (`properties:open`)
landed with the All properties tab, see [[#Tags, All properties and Outline panes, 2026-10-22]].

### File operations, 2026-10-17

New note (Ctrl+N, `file-explorer:new-file`, the Files menu, a click on a
link nobody has created, Enter in the switcher), new folder, rename (F2,
`workspace:edit-file-title`), move (`file-explorer:move-file`, a fuzzy
folder picker), make a copy, and delete (`app:delete-file`) exist, each
through a small prompt modal (`crates/scoria/src/prompt.rs`) or the
explorer's right-click menu. `scoria-vault::ops` does the disk work and
updates the file and folder lists (the vault now tracks folders, so empty
ones show in Files); `scoria-cache::links` plans the link rewrites;
`VaultState` in `workspace.rs` runs both and brings the index up to date
before the watcher's events arrive (they then find nothing to change).

The rules were taken from Obsidian 1.14.4 directly: a throwaway plugin in a
scratch vault (`scoria-probe`, kept in the session's scratch space, not the
repository) renamed files through `fileManager.renameFile` under every
`newLinkFormat`, resolved links through `getFirstLinkpathDest`, created and
trashed files, and dumped the resulting notes. What it showed, now encoded
in the two modules' tests:

- Every link that resolved to the renamed file is regenerated with the
  vault's `newLinkFormat` whatever form it had (`[[a]]`, `[[A.md]]`,
  `[[Sub/A]]`, `[A](./A.md)`, `[A](/A.md)` all become `[[A2]]` or
  `[A2](A2.md)`); a link whose written path already matches is left byte
  for byte, so moving a note with a unique name changes nothing. Shortest
  format falls back to the full path when the basename is ambiguous.
- The style survives: wikilink or markdown, embed or link, angle-bracketed
  destination or not. Markdown destinations keep `.md` and encode only
  spaces (`Ä%20(b)%20100%.md`). The subpath is kept; the alias is kept
  verbatim, except that an alias equal to the old basename and different
  from the written path follows the new basename (`[[Sub/S|S]]` becomes
  `[[S2|S2]]`, `[[S|S]]` becomes `[[S2|S]]`). A markdown link's text follows
  the new basename when it was the old one, or the new link path when it was
  the old destination without `.md`.
- Code spans and fenced code are untouched; reference-style links and their
  definitions are not updated; `[[#Heading]]` self links are not touched.
- Links inside a moved note are regenerated only when they would stop
  resolving (`[B](../B.md)` after the note moved into a subfolder).
- Frontmatter links update. Obsidian re-serializes the whole YAML block when
  it does (`aliases: [Bee]` became a block list); Scoria replaces the link
  text in place instead, keeping the file's bytes.
- `newFileLocation`: a new note is `Untitled.md`, `Untitled 1.md`, ... at
  the root, in the open note's folder, or in `newFileFolderPath` (falling
  back to the root when that folder is missing); a `folder/Name` link
  creates the folders. Creating over an existing path fails with "File
  already exists.", renaming onto one with "Destination file already
  exists!"; a case-only rename succeeds.
- `trashOption` defaults to `system` (freedesktop trash on Linux, with a
  `.trashinfo` record; the vault's `.trash` is the fallback elsewhere);
  `local` moves a file flat into `.trash/` and a folder with its contents,
  numbering collisions ` 2`, ` 3`. `promptDelete` defaults to on.
- `alwaysUpdateLinks` defaults to on; off asks first ("Update links?" with a
  "Don't update" button), as Obsidian's dialog does.

The same probe corrected link resolution: with duplicate basenames Obsidian
picks a file at the vault root first, then one in the source's own folder,
then the shortest path, then path order (`[[Zed]]` from `Long/Way/Down/C.md`
is `Zed.md`, not the sibling `Long/Way/Down/Zed.md`; `[[X]]` there is the
sibling `Long/Way/Down/X.md` over `Mid/X.md`). `./` and `../` paths resolve
relative to the source only, `/path` from the root only. The spec's
"same folder first" rule was wrong and is fixed in [[Specification]].

`crates/scoria-cache/tests/rename_corpus.rs` renames every note of the
1000-note corpus, moves its folders under every format and renames its
attachments, and checks that every resolved link still resolves to the same
file, no link appears or disappears, and nothing outside the links changes.
Not yet: drag and drop in Files, inline title editing, attachment
pasting, the system trash on Windows and macOS.

### Reader gate, 2026-10-16

Checked against a real 10-note vault (project notes with wikilinks and
aliases, tables in nine of them, fenced ASCII diagrams, inline code, ordered
and bulleted lists, two `.canvas` files) by opening every note and the lower
sections of the long ones. Every block rendered with its content; backlinks,
outgoing links and unlinked mentions matched the text. Two fidelity bugs
found and fixed on the way: table columns were all equal, which broke long
identifiers mid-word (columns now follow HTML's automatic table layout, see
below), and fenced code wider than the note wrapped instead of scrolling.

Still not rendered, none of it present in that vault: MathJax (math shows as
its source), HTML tags (shown as text), remote images, Canvas and Bases
files (Parity phase), and the local graph beyond depth one.

### Command palette and Settings, 2026-10-16

`scoria-commands` now carries the command names (`data/names.txt`) and
default hotkeys as Obsidian 1.14.4 registers them, captured from the running
app with a throwaway plugin that dumped `app.commands` and the settings
tabs; the ids gained the nineteen 1.14.4 additions. `CommandRegistry`
keeps registration order, applies `hotkeys.json` (an entry replaces the
defaults, an empty list unbinds) and turns a binding into a label
(`Ctrl + Shift + F`) or a GPUI keystroke. At startup the workspace binds a
`RunCommand { id }` action for every command it can run, so the shell has
no hard-coded shortcuts left; `Workspace::run_command` dispatches by id.

The palette (`crates/scoria/src/palette.rs`) shows only runnable commands,
the way Obsidian hides commands whose check callback fails: palette,
switcher, search, graph and local graph, reading view toggle, back and
forward, settings, sidebar toggles, Files, reveal, Backlinks, Outgoing
links, Outline and Tags. Everything else stays visible in the Hotkeys tab.

Settings (`crates/scoria/src/settings.rs`) is a modal with Obsidian's
navigation and setting rows (name, description, control). Controls are
drawn in their current state from `app.json`, `appearance.json`,
`core-plugins.json`, `community-plugins.json` and the plugin manifests,
`switcher.json` and `command-palette.json`; the Hotkeys tab filters all,
assigned, unassigned and assigned-by-me over the registry and marks custom
bindings. A banner says the view is read-only; writing settings waits for
the writer milestone because Obsidian rewrites the whole file on every
change. Not shown: settings whose `app.json` key is unobserved (default file
to open, delete attachments, Mermaid, quick font size, zoom, frame style,
URI callbacks), the Keychain tab, and options of other core plugins.

### Reading view, 2026-10-16

`crates/scoria/src/reading/model.rs` turns the parser tree into an owned
render tree (styled runs with clickable ranges, resolved embeds, footnotes in
reference order, frontmatter as properties); `view.rs` draws it with a lazily
measured `list`, one item per top-level block, so a 50k-line note lays out
only what is on screen. Clicks on wikilinks, markdown links, tags and footnote
references go back to the workspace, which resolves them with the vault
cache: `[[Note#Heading]]` and `[[Note#^id]]` scroll to the target block.

Rendered: headings, paragraphs with soft breaks, emphasis, strong, strike,
highlight, inline code, inline math (raw), tags, images (`![[pic.png|200x100]]`
sizes), embedded notes, sections and blocks to a depth of four, lists with
numbers and task checkboxes, block quotes, callouts with fold state and
Obsidian's type colors, tables with column alignment, code blocks with a
language label, display math (raw), raw HTML (as text), footnotes.

Tables lay their columns out the way an HTML table's automatic layout does:
each column wants its widest cell and needs at least its longest word (both
measured with the real font), the table is as wide as the sum when that
fits the note, and otherwise the slack between minimum and wish is shared
in proportion. The view records its painted width for this and re-lays out
when it changes. Code blocks never wrap; long lines scroll sideways.

Not yet: Live Preview, MathJax, HTML rendering, remote images, checkbox
toggling, heading folding, copy buttons, PDF export.

### Graph view, 2026-10-16

`crates/scoria/src/graph/layout.rs` is the force simulation: Barnes-Hut
repulsion over a quadtree, springs along links at `linkDistance`, a weak
pull to the center, simulated annealing that stops when cool; drags reheat
it. `view.rs` builds nodes and edges from the cache on the background
executor (notes, optional attachments, unresolved targets and tags, orphans
on or off, the `search` filter and `colorGroups` from `graph.json` evaluated
with the search grammar), steps the layout in the background, and paints
through a canvas: one stroked path for the edges, circles for nodes, labels
once zoomed past a threshold that grows with node count. The local graph
keeps the current note and its depth-1 neighbors; opening another note
re-centers it. The view auto-fits until the user zooms or pans.

Not yet: depth beyond one and the local-graph side pane, arrows, the
timelapse animation, screenshot export, writing display changes back to
`graph.json`.

### File watching and mentions, 2026-10-16

`notify` watches the vault root; the workspace drains the event channel every
300 ms, so a burst of writes becomes one batch. `Vault::refresh_path` turns
each path into added, modified or removed files (walking a moved-in folder,
dropping everything under a removed one, ignoring dot-folders), then
`VaultState::apply` re-reads and re-indexes the notes among them under write
locks and calls `MetadataCache::reresolve` when files appeared or vanished,
because links elsewhere may now resolve. Unlinked mentions use one regex
alternation over every note name and alias (`mentions::Finder`), skipping
matches inside wikilinks, markdown links, inline code and fenced code.

### Bookmarks, 2026-10-20

The Bookmarks core plugin, from the same probe run as the tab groups
([[Specification#Observed bookmarks behavior, Obsidian 1.14.4 (installer build 1.13.7)]]).
`scoria-formats::bookmarks` reads `bookmarks.json` keeping every item as the
JSON object it was — so an item Scoria never touched is written back key for
key, unknown keys included — and lays out the items Scoria makes in
Obsidian's order (`type`, `ctime`, then `path`/`subpath`, `query`, `title`
and `options`, or `items` then `title` for a group). The vault writes the
file whole, two-space indented, no trailing newline, as observed; the
observed file is a round-trip test.

The pane is a third tab of the left dock, shown while `core-plugins.json`
leaves the plugin on: the header's first three buttons ("Bookmark the
active tab...", "New group", "Collapse all" — the search filter is not
built), one row per item with a glyph per kind, groups folding, the open
note's row marked, a right-click "Remove". A file row opens the note at its
heading or block, a search row fills the Search pane, a graph row opens the
graph, a folder row shows the folder in Files, a url row opens the browser.
`workspace.json`'s left dock restores to it when its `currentTab` is the
`bookmarks` leaf.

The commands: `bookmarks:open`; `bookmark-current-view` (the note, or the
graph with the whole option document of `graph.json`; a note already
bookmarked opens "Edit bookmark" whose Remove takes it out);
`bookmark-current-heading` and `bookmark-current-section` (the block id
written into the buffer as one edit when the block has none, exactly as the
link suggester writes one); `bookmark-current-search`;
`bookmark-all-tabs` (a group of every tab in every group, each member
stamped with one time, the title defaulting to the moment in Obsidian's
`MMMM D, YYYY h:mm A`); and `unbookmark-current-view`, which removes the
plain file item added last, groups included, as Obsidian's lookup by path
does. Each goes through the same prompt as a rename, "Add bookmark", with
the default name as the Title field's placeholder; the Files menu gained
"Bookmark..." for files and folders.

Checked over the control socket in the scratch vault: each command's file
after Enter matched the probe's bytes shape for shape, `bookmark-current-section`
wrote ` ^caceeb` onto the paragraph and the autosave saved it, the pane
listed the items with the open note marked, and clicking the block row
opened A at the block.

### Canvas view, 2026-10-21

Until now a `.canvas` opened as a placeholder line. `crates/scoria/src/canvas/view.rs`
is the element that draws it, over the gpui-free model in `scoria-canvas`
(viewport math, hit testing, snapping, the `Scene` with its selection and
whole-snapshot undo) and the writer in `scoria-formats::canvas`. It is one
`canvas()` element: prepaint lays out each visible card's content with
`layout_as_root` at the card's screen size (the content comes from
`canvas/cards.rs`), paint draws the dot grid anchored to canvas space, the
groups with a wash and their label, the edges as cubic Béziers with filled
arrow heads and a pill for a label, the card frames with the content clipped
inside the border, the selection outline and the eight handles, the
connector dots of the hovered card and the marquee, and then registers the
mouse listeners. The keyboard goes through a `CanvasView` key context beneath
the workspace's, so Ctrl+Z undoes a card move instead of reaching the
editor's command. The zoom buttons are gpui divs in the corner, with
Lucide's `plus`, `minus`, `rotate-ccw` and `maximize`; the colour palette is
a row of swatches floating above the selection.

The drag arithmetic is worth a note. A move keeps the canvas delta it has
applied so far and on every event moves the selection by `pointer - press -
applied`, then snaps the pressed card to the grid and carries the rest with
it, so the cards sit on the grid while they move and never drift; Alt holds
the snap off. A resize computes the whole target rectangle from the press,
snaps the moving sides only, and asks the model for the delta that gets it
there. `begin` on the press and `commit` on the release make the drag one
undo step, and a press that never travels three pixels commits nothing. A
connector drag wins over the mid-side handle it overlaps, since on a hovered
card the dot is what is drawn there.

Persistence is the workspace's, the way the editor's is: the view emits
`Edited`, the same two-second `save_task` runs, and `flush_save` now flushes
the canvas before the editor through `write_path` and `write_no_follow`,
refusing when the file is not the bytes the view read or last wrote. That
same comparison tells the watcher's event for the view's own save from a
change by someone else, which goes through `Scene::reload`. An empty file is
an empty canvas, since `canvas:new-file` writes zero bytes and the first
edit writes `{"nodes":[...],"edges":[]}`; a file that does not parse shows
its error and is never written. Each tab group has a `canvas` entity beside
its editor and reading view, `MainView::Canvas` paints it in every mode but
the graph's, and the tab records the view's `viewState` as it changes so the
leaf is written as Obsidian's — `type: canvas`, `lucide-layout-dashboard`,
the title without the extension — whether or not the tab is live on quit.
`canvas:new-file` creates `Untitled.canvas` empty in the vault root and opens
it in a new tab unless the active one is empty.

Checked on the desktop over the control socket, in a scratch copy of this
vault with a showcase canvas of every node type, the six presets and a hex,
labelled edges both ways and two groups, aiming by the `canvas.origin`,
`size` and `viewport` the state dump now carries rather than by guessed
pixels: the wheel panned and the keys zoomed; a drag of a card landed
snapped at `(520, 40)` and two seconds later the file came back in
Obsidian's bytes with only that node's `x` and `y` changed (and the writer's
fixed edge key order, as Obsidian's own edited save does); Ctrl+Z put it
back and the save followed; a double click made a card and Delete took it; a
handle resize went `300` to `400` wide; a swatch set `color` and undo
cleared it; the marquee selected what it crossed, Shift-click toggled,
Ctrl+A took everything, Shift+2 framed the selection; a broken file showed
its error; an empty one opened and its first card was written exactly as
`new.out.canvas`; `canvas:new-file` made a zero-byte `Untitled.canvas` in a
new tab; a move written to the file by hand reloaded the view; and the leaf
in `workspace.json` after quit carried the `viewState`. What is not there:
typing into a card (`editing` is the hook, set by a double click), menus,
dropping an edge on empty space to make a card, and every `OBSERVE`
constant's real value, which the probe owes.

### Canvas view, measured, 2026-10-21

The probe ran the same day, after the user's own Obsidian was closed
([[Specification#Observed canvas view behavior, Obsidian 1.14.4 (installer build 1.13.7)]]).
Two things about running it are worth keeping. The harness's first Obsidian
hung with its debugging port gone after a settings step timed out, and
nothing but a kill and a relaunch brought it back; and with the window only
1024 px wide, every input event has to be aimed after centring the view on
its target, or it lands in a sidebar — the first pass right-clicked the
ribbon and opened the graph in the canvas's leaf. The fixture lives on in
the results folder, the scripts beside the harness.

What the measurements changed: the constants in `scoria-canvas` (zoom -4 to
1, threshold -1.7, the 7 unit edge inset, the [70, 150] control reach, the
10.4 by 13 arrow), the edge geometry (a stub on an arrowless end, the arrow's
tip on the card), the √scale counter-scaling of labels and strokes, the
card's 0.7 border and 0.07 tint alphas, the group label pill, the dot grid's
doubling rule, the controls' move to the top right in Obsidian's four groups,
the card menu at the bottom, a group body that is background, a double click
that centres the card on the pointer, and the arrow keys as nudges.

What landed with them: typing into a card. `sync_editor` runs at the start
of every render and makes an `Editor` entity match the `editing` field, so
a double click, Enter, the menu's *Edit* and an edge dropped on empty space
all just set the field. The editor is laid out in the card's frame in place
of the card's content, with the vault for its embeds and autocompletion;
every `Edited` event copies its text into the node and marks the scene
dirty without closing the undo step, so the autosave writes while the
session stays one step; Escape or a click elsewhere commits it, and a card
left empty is removed, as Obsidian's is. The socket's `type` reaches it
(`focus.name` is `canvas-editor`). Then the floating menu with Obsidian's
items per selection (the swatches under *Set color*), the right-click
menus, the card menu's three buttons (the note and media pickers reuse the
chooser, with a `ChooserPurpose` so the workspace knows what a pick means
and a `pending_canvas_pick` because the subscription has no window),
`canvas:jump-to-group` through the same chooser, `canvas:convert-to-file`
(the note named after the card's first line), clipboard, duplicate and
group keys, task checkboxes inside text cards, and the status bar's word
count hidden for a canvas tab.

Checked over the socket on the probe's own fixture: a double click made a
card at `(1075, 620)` for a pointer at `(1200, 650)` and opened it, typed
text appeared in the state dump, Escape wrote the node with `"text":"Hello
card"` two seconds later and Ctrl+Z took the text back in one step;
`canvas:jump-to-group` listed the three groups and framed *Colored group*;
`canvas:convert-to-file` on *Yellow 3* wrote `Yellow 3.md` and the card came
back as `"type":"file"` with its colour kept; the card menu's note button
opened the chooser over the notes and *Plan* became a second `Plan.md`
card. The screenshots against Obsidian's rendering of the same fixture
match in layout, colours, labels and edges; the differences are listed in
the Specification's divergences paragraph.

### Dragging tabs, 2026-10-21

A tab header can be picked up and dropped: on another tab (in front of it),
on a strip's open space (at its end), or on a group's content, where the
outer band of each side makes a new group on that side and the middle joins
the strip. The zones light up as the pointer crosses them, the ghost is the
tab's title, and a drag let go anywhere else changes nothing. The split a
side drop makes goes through the same `place_beside` as "Split right" and
"Split down" — a sibling when the enclosing split runs that way, a wrapping
split otherwise, the `dimension` halved or handed to the wrapper — only now
the new group can go in front as well as behind, so `Layout::insert_beside`
took an `after` flag. The tab itself moves between the groups' stashed tab
lists with every group stashed first; a group whose active tab left shows
the next one, which means making it live for a moment since views only load
through the live fields; the target then goes live with the dropped tab
active, and a group left empty closes through `close_group`. gpui ends a
drag on the mouse up and redraws, so the next frame clears the zones when
no drag is under way, whatever ended it.

The geometry is a guess: the band is 30 percent of the group's shorter
side, the nearer edge wins in a corner, and Obsidian's own has not been
probed (the Specification says so). Checked on the desktop over the control
socket in a scratch vault of three notes, driving `mouse` `down`, `move`
with `button: "left"` and `up` from a script: a third tab dragged to the
right edge made a second column and became active; the left group's only
tab dragged to the bottom edge of the right one closed the left group and
wrapped the right in a horizontal split with the new group below; that tab
dropped on the upper strip joined it at the end and the split kept its one
child; a drop on the first tab moved it in front; and a drag released over
the sidebar left everything as it was with no zone still lit. Mid-drag
screenshots show the lit half and the ghost. Two unit tests cover inserting
in front and the zone geometry.

### Tab groups, 2026-10-20

The main area was one strip of tabs over one editor and one reading view;
`workspace.json`'s `main` was flattened into it on read and left alone on
write when it held more than one group. It is now a tree. `Layout` is the
tree Obsidian's `main` records — a root `split` over `tabs` groups and nested
splits — and every `TabGroup` owns an editor and a reading view of its own, so
two groups show two notes at once. The active group's tabs and note stay in
the workspace's live fields, exactly as the active tab's always did, and
switching groups stashes them and loads the other's without reloading a
file: the group's views already show its note. A mouse down anywhere in a
group makes it active before the view handles the click, so an edit always
lands in the live editor and the autosave, the explorer highlight, the graph
centre and the right dock all follow.

What was built, each from the probe run of the same day
([[Specification#Observed tab group behavior, Obsidian 1.14.4 (installer build 1.13.7)]]):
`workspace:split-vertical` and `split-horizontal` (a sibling when the
enclosing split runs that way, a wrapping split otherwise, the new group a
copy of the active leaf and active); `editor:focus-left/right/top/bottom`
(the sibling on that side, and inside it the most recently used group);
`workspace:close-tab-group`, `close-others-tab-group` (the group's other
tabs, as Obsidian's), and a group's last tab closing the group with focus
going to the most recently used group left; `editor:open-link-in-new-split`;
a draggable divider between every pair of children that writes Obsidian's
`dimension` percentages on both, halved by a split and rescaled to a hundred
by a close; and `main` read as a tree and written back as one, each node
over the keys it was read with, so a nested layout Obsidian wrote comes back
to it byte for byte. The socket's `state` gained a `groups` object (active
id, the tree, every group's tabs and dimension), and its `mouse` `move`
takes a `button` so a divider can be dragged from a script.

Two limits worth knowing. There is one graph view: the group that shows it
is the active one when that is in graph mode, else the first that is, and a
second group asking for the graph says so in its place. And a leaf type
Scoria has no view for is still dropped on read, as before.

Checked on the desktop over the control socket in a scratch vault: split
right, open B, split down, click back into the left group, focus-right
landing on the bottom-right group, close-tab-group taking the group away,
quit writing the nested `main` with `active` on the right leaf, and a
relaunch bringing all three groups back with C in reading view. `cargo test
-p scoria` has four new tests for reading `main`, splitting, removing,
neighbours and the `dimension` slot.

### Search and quick switcher, 2026-10-16

`crates/scoria-search` implements the query grammar once: implicit AND,
uppercase `OR`, parentheses, `-` negation, quoted phrases with `\"`,
`/regex/flags`, the `file:` `path:` `content:` `tag:` `line:` `block:`
`section:` `task:` `task-todo:` `task-done:` `match-case:` `ignore-case:`
operators, and `[prop]`, `[prop:value]`, `[prop:null]`, `[prop:true]`,
`[prop:>n]` properties. `Query::explain` prints the tree with the labels
Obsidian's "Explain search terms" shows, which the tests check. Matching is
regex-based (case-insensitive unless `match-case:`) and returns byte ranges
for highlighting; scoped operators evaluate per line, block, section or task.
The Search tab runs the compiled query over the indexed note texts on the
background executor with rayon and lists results per file. The fuzzy scorer
(`fuzzy_match`) rewards word starts, consecutive runs and short candidates;
the switcher ranks file names, aliases and paths with it. The text field is a
single-line `TextInput` painted through a canvas element with GPUI's input
handler, so IME works.

> [!note] Fonts
> The pinned gpui does not pick weights out of a variable font, so with Noto
> Sans (one `NotoSans.ttf` for every weight) nothing renders bold. Since
> 2026-10-20 the app bundles Inter's static faces and registers them at
> startup, so the default text and interface font is Inter on every machine;
> `appearance.json` fonts override it. Never bundle `InterVariable.ttf`.

## Spike gates

The [[Specification]] sets three gates before any reader or writer work starts.

| Gate | Target | Status |
| --- | --- | --- |
| Editor prototype | 50k-line note with wikilink and heading decorations | passed, see below |
| Parser conformance | 1000-note HTML diff corpus at 99 percent | passed: 1007/1009 notes against Obsidian 1.13.7, plus CommonMark 652/652 |
| Index performance | 20k-file vault indexed cold in under 2 s | measured, see below |

### Source editor, 2026-10-16

`crates/scoria/src/editor.rs` is a plain-text Markdown editor over one `String`
with a line index. A custom GPUI element shapes and paints only the visible
lines; decorations come from `scoria_markdown::highlight`, recomputed on a
background thread after each edit (one parse in flight, one queued) and
swapped in when ready, so typing never waits for the parser. Input goes
through GPUI's input handler scoped to the cursor line, so IME composition
works; everything else is an action.

Grown from the spike prototype the same day: soft wrap at the view width
(capped at the readable line length and centered, like Obsidian), a
selection (shift with every movement key, drag, double-click word,
triple-click line, Ctrl+A), word movement and deletion, undo and redo with
CodeMirror's grouping (consecutive typing or deleting within half a second
is one step), clipboard copy, cut and paste (the whole line when nothing is
selected), and the note's text font with the monospace font for code. The
scroll position is a line plus a pixel offset into it rather than an
absolute pixel, so wrapped-row heights are only measured near the viewport
and a 50k-line note never needs a full layout pass; Page Up and Down,
revealing the cursor and clamping at the end all walk rows from that anchor.

Saving: the buffer holds `\n`; the file's BOM and `\r\n` endings are
remembered at load and put back on write, so a save changes nothing but the
edited bytes (verified with `--stress` on a BOM-and-CRLF note in a scratch
vault). Edits save two seconds after the last keystroke, on Ctrl+S
(`editor:save-file`), when another note opens, when the view switches to
reading mode (which then rebuilds from the saved text), and when the window
closes. The watcher's reload of the open note is skipped when the file is
what the editor last wrote, keeps the cursor when the note changed
underneath an unedited buffer, and leaves unsaved edits alone otherwise.

```
cargo run --release -p scoria -- docs/scoria --synthetic 50000          # open a generated 50k-line note
cargo run --release -p scoria -- docs/scoria --synthetic 50000 --stress  # 600 self-driven edits, then a report
```

Stress run on the reference machine with soft wrap on, 50,001 lines (4.3 MB,
244k decorated spans; the synthetic note is far denser in syntax than a real
one):

| Measure | Result |
| --- | --- |
| Frames painted during 600 edit, selection, undo and navigation steps in 7 s | 607 |
| Shaping the visible lines, per frame | 0.17 ms mean, 1.95 ms max |
| Full reparse plus decoration pass (background) | 222 ms mean |
| Edit to refreshed decorations | 244 ms |

The frame path is the gate: scrolling and typing stay far under a frame
budget regardless of note size. Decoration latency scales with note length
because the whole note is reparsed; a real 50k-line note of ordinary prose is
about half this size, and incremental reparse of the edited block is the
planned follow-up. Not yet: auto-pair of brackets and markdown markers, and
Ctrl+click page previews.

### Find and replace, 2026-10-16

Ctrl+F (`editor:open-search`) opens Obsidian's search panel at the top
right of the editor, switching to editing first; Ctrl+H
(`editor:open-search-replace`) adds the replace row, which a toggle on the
panel also shows. The query starts as the selection when it sits on one
line. Matching is case-insensitive and literal (`crates/scoria/src/find.rs`,
capped at 20k matches); every match is tinted, the selected one more so,
and the panel counts them. Typing selects the first match from the cursor
on, Enter and the arrows step forward and back with wrap-around, Enter in
the replace field or the Replace button replaces the selected match and
steps on, Replace all does every match in one undo step, Escape closes.
Matches refresh after every edit. `--find q` opens a note with the panel
up, for screenshots.

### Smart lists and the scrollbar, 2026-10-16

Enter with `smartIndentList` continues what the line started: its quote
markers, its bullet or the next number, and an empty checkbox after a task;
the rest of a split item moves to the new line and the ordered items that
follow renumber in the same undo step. Enter on an empty item clears it, or
outdents it when nested. Shift+Enter is a plain newline. Tab indents a list
item (or every selected line) by one step, a tab or `tabSize` spaces per
`useTab`, and Shift+Tab removes one. `crates/scoria/src/lists.rs` holds the
prefix parser and the rules as pure functions with tests.

The vertical scrollbar is line-based like the scroll position: the thumb
sits at the first visible line's share of the lines the view can start at
(the last of them is found by walking one screenful back from the end) and
is as tall as the share of lines that last screenful holds, so it never
needs the height of every line. Both shares are fixed for a note and a
view, which keeps the thumb the same size as it moves; the lines on screen
at one scroll position vary with wrapping and widgets and would make it
breathe. The part of the first line scrolled off the top moves the thumb
between lines. Dragging it or clicking the track scrolls to that line.

### The tab drop bar, 2026-10-22

Obsidian shows where a dragged tab will land with an accent bar between the two
tabs nearest the pointer, and the bar slides to each new boundary rather than
jumping. Scoria used to wash the tab under the pointer instead. The strip now
listens to the drag's moves: each tab records its bounds on every move, and the
strip picks the boundary nearest the pointer among the tabs' left edges and the
last tab's right edge (the end). The bar is 3 px of accent, 25 px tall astride
that boundary, painted deferred after the dividers and feet. A change of
boundary starts a 120 ms ease-out slide from wherever the bar was drawn, which
`render` keeps alive with `request_animation_frame` until it settles. A drop on
the strip or on a tab lands at the bar's boundary; dropping on a group's content
keeps the zone rules. The bar's width, height and timing are Scoria's own
guesses at Obsidian's, not probed values. The drag ghost used to be drawn where
the tab sat under the pointer, which put it up and left of the cursor over the
strip and hid the bar as it slid left; its top left corner now sits at the
cursor's bottom right (8 px past the hotspot, where the grabbing hand's own
corner falls in a 24 px Breeze cursor) like Obsidian's, clear of the strip, and the cursor is the
grabbing hand for the drag: the ghost's view paints last and asks for the
window cursor without a hitbox, which gpui lets win over the tab's pointer.

### Backlinks pane, 2026-10-22

The Backlinks and Outgoing links panes were flat per-occurrence lists: forty rows all
labelled "Development", each with a `line: text` detail and nothing marking the link.
They now have Obsidian's shape, built in `crates/scoria/src/backlinks.rs` with no gpui
in it: a `Section` ("Linked mentions", "Unlinked mentions", "Links") holds one
collapsible `Group` per file with its count, and a group holds its `Match`es, each
carrying its context and the matched text's byte range inside it, so the pane can paint
the link or the mentioned name with the search highlight. The context is the match's
line trimmed, with the range moved to fit; "Show more context" adds the non-blank line
either side. Unlinked mentions are deduplicated per `(path, range)` (the old list
carried every duplicate the regex produced and printed the 200-row cap as the count),
grouped by file, and keep the mention's own range for the Link button, which writes
`[[Name]]` — `[[Name|text]]` when the text is an alias or another case, a markdown link
under `useMarkdownLinks` — through the editor's buffer when the note is the open one and
straight to disk otherwise, after checking the file is still the bytes the mention was
found in. A Links row carries the link as written and a click goes through
`follow_link`, so the subpath, the alias and the "create the note" case all come for
free; the Links section is flat, as Obsidian's is.

The Backlinks header is Obsidian's four `nav-action-button`s — Collapse results
(`list`), Show more context (`move-vertical`), Change sort order (the Files view's six
orders, in a menu with a check) and Show search filter (an in-pane field that keeps
groups whose name matches whole and other groups' matching lines) — and the leaf's
seven keys (`collapseAll`, `extraContext`, `sortOrder`, `showSearch`, `searchQuery`,
`backlinkCollapsed`, `unlinkedCollapsed`) plus the outgoing leaf's two are read from
`workspace.json` at restore and written back in place, through a `leaf_state` /
`set_leaf_state` pair that the Files leaf now uses too. Matches wrap, so the list is a
scrolling column rather than a uniform list; every row and card is `flex_shrink_0`,
because a flex column's children otherwise shrink to fit the viewport and a card of 21
matches showed two.

Seen working over the control socket on a scratch copy of this vault with
Specification open: 43 linked mentions in five groups, Development's 21 in one card;
more context, collapse results, the sort menu and the filter each change
`backlinks_options` in the state dump and the list on screen; the Link button on an
unlinked mention in the backlog note rewrote `Specification.md` to `[[Specification]].md`
in that file alone, the status bar reported the change, and the row moved from
Unlinked (14 to 13) to Linked (10 to 11). The dump's `backlinks` and `outgoing` are now
flattened `section` / `file` / `match` rows with `collapsed`, `ranges` and `mention`.

### Outgoing links pane, 2026-10-22

Probed first this time: `tools/obsidian-probe/outgoing-links.mjs` drove Obsidian through
the pane on a five-note vault, and the findings are
[[Specification#Observed outgoing links pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]].
What that settled that the earlier guess had wrong: the Links section lists one row per
target and subpath (ten links written, seven rows; `[[Beta]]`, `[Beta](Beta.md)` and
`![[Beta]]` are one row) and never shows an alias; each row wears a glyph for what it
points at (`link`, a heading glyph, `layout-list`, `file-plus` with "Not created yet")
and, under its label, the note a heading or block belongs to or the folder a note sits
in; the section headers have no fold triangle at all, the count is faint, and a folded
header is faint throughout; a match card has the note's background and no border; and
the Outgoing pane's unlinked mentions are one card for the note itself with a "Link this
file" chip naming the mentioned note under each match, where the Backlinks pane has a
Link button that only appears under the pointer.

Built: `backlinks::outgoing_links` dedups by resolved target plus subpath and gives each
`Group` a `subtext` and an `icon`; `unlinked_outgoing` is one group in text order whose
matches carry their `target`; `render_sections` draws the section rows, Links rows and
cards to the measured sizes (25 px rows with an 8 px radius, 24 px gutter, 11 px faint
subtext, cards at 4 px radius with 8 px 20 px 8 px 12 px matches, the accent at 33% under
the pointer), the chip in the Outgoing pane and the hover-only Link button in the
Backlinks pane, and "No backlinks found." for an empty Linked mentions. Two Lucide icons
joined the assets, `heading` standing in for Obsidian's own heading glyph. The dump's
file rows gained `subtext` and `icon`, match rows `target`.

Seen working over the control socket on a copy of the probe's vault: the seven rows in
Obsidian's order with their glyphs and subtexts; the chip under `Gamma` wrote the same
bytes Obsidian wrote and the list went from six mentions to five; the `Missing` row
created an empty `Missing.md` and opened it; the `Heading` row opened Beta. Still open:
the row context menu (no file menu exists anywhere in the app yet) and the per-match
"Show more context" chevrons.

### Tags, All properties and Outline panes, 2026-10-22

Probed first again: `tools/obsidian-probe/dock-panes.mjs` drove Obsidian through the three panes
on an eight-note vault (raw run in `tools/obsidian-probe/results/2026-10-22-dock-panes.json`), and
the findings are the three sections
[[Specification#Observed tags pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]],
[[Specification#Observed all properties pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]]
and [[Specification#Observed outline pane behavior, Obsidian 1.14.4 (installer build 1.13.7)]].
What that settled that the old panes had wrong: the three share one row — `tree-item-self`, 276 by
25 at a 27 px pitch, 13 px muted text with the count a 12 px faint flair, an 8 px radius, the text
indented 17 px per depth with a fold triangle 20 px before it and a hairline guide per level — and
one header, four, two and three `nav-action-button`s centred 2 px apart with a tooltip each and a
30 px search box that pushes the rows down 38 px. The tags pane shows no `#`, counts occurrences
rather than notes and gives a parent its descendants' occurrences too (`project` 6 = `#Project` 1 +
`alpha` 3 + `beta` 2), spells a tag as its first indexed occurrence (`#Project` folds into Alpha's
`project`), nests `a/b` under `a` unless "Show nested tags" is off, when every tag is one flat row
with its `project/` prefix, and sorts by name or frequency with ties A to Z at every level; its
leaf keeps `sortOrder`, `useHierarchy`, `showSearch` and `searchQuery`, and Escape in its box is not
its own (the editor takes the keyboard, the rows stay filtered). The All properties pane wears a
Lucide icon per widget (`text`, `list`, `binary`, `check-square`, `calendar`, `clock`, `tags`,
`forward`, `file-question` for a nested map), keeps `sortOrder`, `showSearch` and `searchQuery`,
and Escape there does hide the box and clear the query. The Outline nests structurally — a heading
under the nearest preceding heading of a smaller level, whatever the numbers — wraps long headings
rather than cutting them, marks the heading whose section holds the cursor (or, in reading mode,
the line at the top of the view) with the hover wash, scrolls that row into view only while
"Auto-scroll to current section" is on, folds per heading with "Collapse all" swapping to "Expand
all", filters to the matching rows and their ancestors with the match highlighted, says "No
headings found." for a note without any, keeps `file`, `followCursor`, `showSearch` and
`searchQuery`, and a click puts the cursor on the heading's line at column 0, centred.

Built: `crates/scoria/src/panes.rs`, gpui-free with unit tests, holds the models — `PaneSort` with
Obsidian's four stored values and menu labels, `tag_tree` / `sort_tags` / `tag_rows` (inclusive
counts, the first-file spelling, hierarchy or flat, folds, the filter), `property_rows` with
`property_icon`, and `outline_depths` / `heading_at` / `outline_rows` (structural nesting, folds,
the cursor mark, the filter with its ranges). The workspace draws all three through one
`tree_row`, one `pane_header`, one `render_pane_filter` (with the round clear button) and one
`render_pane_sort_menu`; each pane has its own `TextInput` filter and its own state struct, read
from and written to `workspace.json` in place through `leaf_state` / `set_leaf_state` (a `null`
now removes a key, which is how the outline's `file` goes away for an empty tab). The Outline's
mark follows the editor and the reading view through `cx.observe` on both and recomputes at draw
time, so a mode or note switch cannot leave it stale; the editor gained `reveal_offset_centered`
for the row click. Seven Lucide icons joined the assets: `folder-tree`, `text`, `binary`,
`square-check` (Lucide's current name for Obsidian's `check-square`), `clock`, `forward` and
`file-question`; the sort button reuses `arrow-up-narrow-wide`, which is what Lucide renamed
`sort-asc` to. The dump's `tags`, `properties` and `outline` rows gained depth, fold, parent, icon,
mark and highlight fields (the old `name`, `count`, `level`, `text` and `offset` keys stay), and
each pane has an `_options` sibling with its sort, hierarchy, follow-cursor, search and query.

Seen working over the control socket on a copy of the probe's vault: the tags in Obsidian's
frequency order with the same counts and spelling (`common` 9, `project` 6 with `alpha` 3 and
`beta` 2 holding `deep` 1, `Lambda`, `topic`, `a`, `Kappa`, `meta`, `single`), the nested toggle
flattening them, a triangle folding `project` from 11 rows to 8, the sort menu, `proj` keeping the
four `project` rows and Escape leaving them filtered, a row click searching `tag:#project` with
four results; the fourteen property rows in Obsidian's order with their icons, `boo` keeping the two
`book.` keys, Escape clearing it, a row click searching `["Status"]`; the 35 outline rows at
Obsidian's depths, the mark moving with the cursor and with the reading view's scroll, the pane
scrolling to `Extra 24` once auto-scroll was on, "Collapse all" leaving the three top-level rows
and no mark, `second` keeping seven rows with their ranges, a row click landing the cursor at
column 0 of line 79 with the line centred, "No headings found." for `NoHeadings.md`; and the
probe's own `workspace.json` (flat, `frequencyReverse`, `com`; `alphabeticalReverse`, `st`;
`followCursor` on) coming back exactly as Obsidian's restart had shown it, then written back with
the same keys. Not replicated: the tags pane's "Expand all" quirk (Obsidian's first click did
nothing and its label never changes; Scoria's label never changes either but its first click folds
everything), the search highlight's exact styling (the spec has no computed values for
`suggestion-highlight`, so the accent colour stands in), and anything the spec lists as unprobed —
whether the tag filter matches the whole path (Scoria matches the whole path, case-insensitively),
the row context menus, keyboard navigation, dragging a row. Kept on purpose: the Outline still
rebuilds from the buffer as you type, where Obsidian waits for the file to reach disk, because
stale offsets would land the row click in the wrong place. The screenshots came out of the
desktop's active window, which was the user's browser for most of the run, so the sizes and
colours above were checked once against the first capture of the tags pane and otherwise through
`scoria_state`.

**After the verifier.** An independent agent compared the running app with the findings and
found five things the build had missed; all landed the same day. A vault Obsidian never laid out
got no dock in `workspace.json` at all, so every pane setting was lost on quit: `layout_json`
now writes Obsidian's default docks (`default_dock_json`, the fresh file's own shape: one split
of one tab group per side, the five right leaves and three left ones with their default state,
`main` first) when the file has none, and the states go into them. The Outline's follow-cursor
scroll put the marked row flush with the bottom edge, under the status bar; `scroll_outline_to`
stops it 32 px short, the pane's own bottom padding, and takes the first row back to the very
top. "Collapse all" folded every level; it folds the top level and leaves the deeper folds alone.
A tag row's search listed its files with no match lines and "0 results": a `tag:` hit now
highlights where the tag is written (`#tag`, `#tag/sub`, the name on the frontmatter's tags
line), and a property row's search found nothing because the parser kept the quotes in
`["Status"]`; a property hit now highlights its key's line too. The three panes' sort menus open
at the pointer, top-right corner at the click, as measured; the filter boxes gained the
magnifier and the 30 px inset; and the Backlinks and Outgoing links leaves start with Unlinked
mentions folded, Obsidian's default. Still as the verifier left them: Escape in the Tags filter
keeps the typed text where Obsidian drops it, the fold triangles are Lucide chevrons rather than
Obsidian's `right-triangle`, and the search leaf's own `query` is not restored at launch.

### Search pane, 2026-10-22

The Search pane was a field, a count line and a uniform list of one-line rows with a text
triangle per file. It now has Obsidian's shape, measured off a CDP run that captured the
pane itself (`tools/obsidian-probe/search-shots.mjs`, written up under
[[Specification#Observed search pane behavior, Obsidian 1.14.4]]): the `search-row` with
the magnifier, the Match case decorator and the clear button inside the field and the
Search settings button after it; the Explain search terms block; the three toggles; the
results bar with the count (its menu: Copy search results, Bookmark...) and the sort
control; and the list — a 25 px title row per file with the chevron, the name and the
match count, over a card of its matches, each padded 8/20/8/12, wrapping, the matched
text on the yellow wash with its weight unchanged, the accent wash and the two "Show
more context" chevrons under the pointer. Matches wrap, so the uniform list became
gpui's `list` with a `ListState`: a new result set resets it to the top (a splice of the
whole range left it at the end), and a fold or a change of context splices so the scroll
position holds.

The leaf's six keys — `query`, `matchingCase`, `explainSearch`, `collapseAll`,
`extraContext`, `sortOrder` — are read from the left dock's `search` leaf at restore and
written back in place, so a query survives a restart as Obsidian's does. Match case
wraps the parsed query in `Query::CaseSensitive`; the sort control re-sorts the list it
has without running the query again, the name orders through `natural_cmp` and the time
orders by the time alone with a stable sort, which is what kept Obsidian's tie groups in
their previous order; Show more context and a match's chevrons go through
`scoria_search::context_window`, whole trimmed lines around the match that stop at a
blank line, like the Backlinks pane's. Explain now quotes the term (`Matches text:
"needle"`), which the live DOM shows and the earlier label dump had not.

Keyboard: Down and Up move the focus ring (Obsidian's `has-focus`) over file rows and
match rows alike, Enter opens the focused row at its match, Left folds and Right unfolds
the file, Escape drops the ring and then returns the keyboard to the main view. gpui
dispatches a matched action before any key listener sees the key, so the pane takes the
field's own Left and Right actions in the capture phase rather than listening for keys.

"Copy search results" is a modal of its own (`crates/scoria/src/copy_results.rs`): the
preview, Show path, Link style and List prefix, Copy results (which writes the clipboard,
shows "Results copied to your clipboard." and leaves the modal open) and Done. Its text
comes from `search::copy_text`, pinned by a test to the 24 lines the spec recorded and to
the exact bytes the probe read off the clipboard. The modal's two selects are menus and
its preview is read-only, where Obsidian's are a native `<select>` and a textarea.

### Live Preview, 2026-10-16

Live Preview is the same editor and buffer with a per-line display
transform (`crates/scoria/src/preview.rs`): from the line's highlight spans
it hides markers or substitutes glyphs, and returns the display text with a
piece map between source and display offsets. Every construct the cursor
or selection touches keeps its source, so a line's markers appear as the
cursor enters them and go when it leaves, per construct rather than per
line, as in Obsidian. The editor shapes the display text and maps clicks,
the cursor, the selection and IME bounds through the pieces; Source mode is
the identity transform.

Rendered today: heading markers hidden and the line scaled (Obsidian's h1
to h6 sizes at a tighter line height, so rows now have per-line heights
and all scroll math walks those), bold, italic, strikethrough, highlight and
inline code markers, wikilinks shown as their alias or target, markdown
links and images as their text, autolink brackets, footnote brackets,
comments and block ids hidden, bullets as dots, task markers as checkboxes
(click toggles them without moving the cursor), quote markers as a left bar
per level, callout type blocks folded into the title, thematic breaks as a
rule, and code blocks and frontmatter on a band. Clicking a rendered link
follows it (Ctrl+click in both editing modes). Tables, embeds, images and
math showed their source with decorations until the widgets below.

`crates/scoria-markdown::highlight` gained a `QuoteMarker` span per `>` so
the transform knows where each level's marker sits. The 300-step stress run
on a 530-line note in Live Preview shaped in 0.07 ms per frame; the 50k-line
Source mode gate is unchanged at 0.17 ms.

### Regex search, multiple cursors and block widgets, 2026-10-17

The find panel gained Obsidian's two toggles, `Aa` (match case) and `.*`
(regex), both off by default; `crates/scoria/src/find.rs` now compiles the
query with the `regex` crate (a literal query is escaped), skips empty
matches and treats a pattern that does not parse as no results. A regex
replacement expands `$&`, `$1` to `$9` and `$$`. Find all (the button, or
Alt+Enter in the query) turns every match into a selection, closes the panel
and focuses the editor.

The editor keeps extra selections beside the primary one (`extra`, sorted and
disjoint); `cursors()` and `set_cursors()` move between that and a plain
list, merging selections that overlap. Typing, Backspace, Delete, word
deletion, Enter (a plain newline when there are several cursors), Tab, cut,
copy and paste go through `edit_all`, one replacement per selection in one
undo step; the history records the selections either side, so undo restores
all cursors, and a typing step joins the previous group when it starts
exactly where that one ended. Movement runs the same per-cursor target
function for every selection; `add_cursor` (the `editor:add-cursor-above`
and `below` commands) puts one a row above the topmost or below the
bottommost, Alt+click adds or removes one, Escape and a plain click
collapse to the primary. The status bar counts cursors. IME composition
still addresses the primary cursor only; the others are shifted past its
edit.

Tab and Shift+Tab with smart lists on now renumber: `lists::renumber_block`
walks the list block around the changed lines and gives every ordered item
from the first changed line down its previous sibling's number plus one,
or 1 when it has none (so an indented item starts a nested list at 1 and
the items it left behind close the gap). The edits are planned in pre-edit
coordinates (`indent_plan`) so the indentation and the new numbers are one
undo step.

Live Preview renders tables, math blocks and embed-only paragraphs as
widgets. The background parse also builds them (`reading::model::build_widgets`,
with the vault as embed source, so embedded notes and images resolve), and
the editor folds a widget's lines into one entry whose height is the
rendered element's; `height_of` returns that height for the first line and
zero for the rest, so the scroll math is unchanged. The widget element
comes from the reading view's `Renderer`, which no longer depends on the
`ReadingView` entity but takes link and fold handlers, and is laid out as a
child element inside the editor's custom element (the editor entity is
leased for the frame so the app is at hand). Any selection touching the
block unfolds it: arrowing into a table or math block shows its source, as
does clicking one (at the clicked row); an embed widget keeps its clicks so
links inside work. Inline math drops its dollars like other inline markers.
Math is shown as its TeX source, centered and in the accent color, as in the
reading view; there is no formula renderer yet. `--find-all q` opens a note
with every match selected, for screenshots.

### Parser, 2026-10-16

`scoria-markdown` now parses CommonMark, GFM tables, strikethrough, task lists,
footnotes and bare URLs, and Obsidian's wikilinks, embeds, tags, block ids,
callouts, highlights, comments and math into one tree with byte spans. All 652
examples of the CommonMark 0.31.2 spec render byte-identical HTML
(`tests/commonmark.rs`, a ratchet that must not go down).

The Obsidian side of the gate is `tests/corpus.rs`: `tests/corpus/vault` is a
vault of 1000 generated notes (`scoria-bench generate --notes 1000`) and nine
hand-written edge-case notes, and `tests/corpus/expected` holds the HTML
Obsidian 1.13.7 produced for each through the `scoria-corpus` plugin that
ships inside the vault. Both sides are reduced to a canonical token stream
(no copy buttons, icons, MathJax output or `dir` attributes) before
comparison. 1007 of 1009 notes match; the two left are pathological
delimiter runs and a wikilink inside markdown link text. The observed
behaviors are listed in [[Specification]].

To regenerate the expected HTML after changing the corpus: set the corpus
vault as the open vault in `~/.config/obsidian/obsidian.json` (or open it from
Obsidian's vault switcher), create an empty `.scoria-autorun` file in the vault,
start Obsidian, and trust the plugin when asked the first time. The plugin
writes `../expected`, then closes its window.

### Index benchmark, 2026-10-15

Synthetic vault from `scoria-bench generate`: 20,000 notes, 2,000 attachments, 35 MB of Markdown.
Page cache was warm; cold numbers need the cache dropped first.

| Threads | Scan | Read | Index | Resolve | Total |
| --- | --- | --- | --- | --- | --- |
| 128 | 78 ms | 46 ms | 24 ms | 32 ms | 180 ms |
| 1 | 77 ms | 144 ms | 369 ms | 137 ms | 726 ms |

Folder scanning is the largest fixed cost and the stage to watch as the indexer grows.

### Index benchmark, 2026-10-16

Same vault, with the cache built from the full parser tree instead of a line
scanner. Warm page cache.

| Threads | Scan | Read | Index | Resolve | Total |
| --- | --- | --- | --- | --- | --- |
| 128 | 75 ms | 19 ms | 11 ms | 34 ms | 139 ms |
| 1 | 76 ms | 116 ms | 563 ms | 140 ms | 895 ms |

The full parser costs about 200 ms more than the old line scanner on one
thread (35 MB in 563 ms, roughly 60 MB/s) and is invisible with the thread
pool. The gate still passes cold with margin; the parser is not the bottleneck.
