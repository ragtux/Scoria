---
title: Install and run
description: Getting a Scoria binary for Linux, Windows or macOS, building it from source, the command-line flags it accepts, and the extra step NixOS needs.
---

# Install and run

There is no installer. A release is one archive per platform holding one binary; unpack it,
put it wherever you keep such things, and run it with the path to a vault.

## Get a binary

Releases are published on [GitHub](https://github.com/ragtux/scoria/releases) (Linux x86-64,
Windows x86-64, macOS on Apple silicon) and on
[git.ragtux.com](https://git.ragtux.com/ragtux/scoria/releases) (Linux x86-64). Both come
from the same `v*` tag and the same commit; the homelab runner only has a Linux container,
which is the whole reason the two lists differ.

```shell
tar xf scoria-x86_64-unknown-linux-gnu.tar.gz
sha256sum -c SHA256SUMS      # against the file in the same release
./scoria ~/notes
```

> **Warning** Nothing is code signed or notarized. Linux is the platform Scoria is developed on
> and the one that gets the hours; since 0.3.1 the Windows and macOS builds are run on real
> hardware too, each drawing its own platform's window. On macOS, Gatekeeper
> will refuse the first launch — open it once with
> *right-click › Open*, or run `xattr -d com.apple.quarantine scoria`. On Windows,
> SmartScreen warns — *More info › Run anyway*.

## Requirements

Scoria's interface is [GPUI](https://gpui.rs), which renders on the GPU.

| Platform | Needs |
|---|---|
| Linux | A Vulkan-capable driver. Wayland and X11 both work; libxcb and libxkbcommon are linked, and Wayland, Vulkan and GL are loaded at run time |
| Windows | Windows 10 or 11 with a Direct3D 12 GPU |
| macOS | Apple silicon, macOS with Metal |

Memory follows the size of your vault, not the size of the note you are reading: the
metadata cache holds every note's links, headings, tags and sections. A 20,000-note vault is
comfortable on an ordinary machine — see [performance](/docs/reference/performance/) for
measured numbers.

## Build from source

The toolchain is pinned by `rust-toolchain.toml`, and the dependency set is pinned by
`Cargo.lock`. Build with `--locked` and let the lock file decide:

```shell
git clone https://github.com/ragtux/scoria
cd scoria
cargo build --release --locked -p scoria
cargo test --workspace            # library suites, including the conformance ratchets
./target/release/scoria ~/notes
```

> **Note** `gpui` and `gpui_platform` are git dependencies pinned to a Zed release commit,
> because the crates.io release is stale and crashes on NVIDIA. `libc` is pinned too, for an
> old `xattr` in gpui's tree. Both pins are explained in the root `Cargo.toml`. This is why
> `--locked` matters: resolving the dependencies freshly can get you a build that does not
> run.

A release build is the one to use. In a debug build the editor reparses the note on every
keystroke in the foreground, which is fine for a test and unpleasant for typing.

## On NixOS

GPUI loads its X11, Wayland and Vulkan libraries at run time, so a `nix-shell` supplying
them is the straightforward way to both build and run:

```shell
cd scoria
nix-shell                          # shell.nix at the repository root
cargo run --release -p scoria -- ~/notes
```

The same applies to a downloaded binary: it needs those libraries on the loader path like
any other dynamically linked program, which on NixOS means a shell that provides them (or
`nix-ld`, or patching the binary) rather than running it bare.

## Command line

The only argument you need is the vault. A bare second argument is taken as a note name, so
`scoria ~/notes Welcome` works like `--note Welcome`.

```shell
scoria [options] [<vault>] [<note>]
scoria [options] <file>                     a note, canvas, base or PDF
scoria [options] obsidian://open?file=Note
scoria --help
```

A **file** argument opens the vault that holds it and then the file: the vault is the nearest
folder above it with a `.obsidian` in it, failing that one on your vault list, failing that the
file's own folder. That is what makes *Open With → Scoria* work on a note in a file manager.

A **URI** argument is acted on once the vault has loaded: the `obsidian://` actions `open`
(`file` or `path`), `new` (`name`, `path`, `content` and the `silent`, `append`, `prepend` and
`overwrite` flags), `search` (`query`) and `daily`, and the same grammar under `scoria://`. A
URI whose `vault` names a different vault is refused rather than opened.

With **no argument** Scoria opens the vault manager, which is also where *Open folder as vault*
is.

### One window per vault

Scoria runs one process per vault — its vault window and any [popout
windows](/docs/app/window/#popout-windows) it has opened — so a second `scoria` on a vault that
is already open does not start a second copy: it hands the note, file or URI to the vault window
and raises it, then exits. That is how `scoria ~/notes Basalt` behaves like a
"go to this note" command from a terminal or a hotkey, and how a `scoria://` link opens in the
window you are already looking at.

The rendezvous is a socket named after the vault, in `$XDG_RUNTIME_DIR/scoria` (the user's own
temporary folder on macOS and Windows). It answers only the handover calls. `--new-window`, or
`SCORIA_SINGLE_INSTANCE=0`, starts a second Scoria instead; a run with `--control` never takes
part, because something outside it is already driving that process.

### Options

| Flag | Effect |
|---|---|
| `--note <name>` | Open this note at startup. Takes Obsidian's link syntax: `Name`, `Name#Heading`, `Name#^block-id` |
| `--source` | Open the note in Source mode |
| `--reading` | Open the note in reading view |
| `--uri <uri>` | Act on an `obsidian://` or `scoria://` URI; a bare argument of that scheme does the same |
| `--new-window` | Start another Scoria rather than handing the arguments to the window already showing the vault |
| `--size <w>x<h>` | Window size in logical pixels |
| `--frame <style>` | `hidden` (the default) or `native`, the desktop's own title bar |
| `--search <query>` | Open with the Search tab filled and run. Takes the full [query grammar](/docs/reference/search-syntax/) |
| `--switcher <text>` | Open with the quick switcher up and this typed into it |
| `--palette <text>` | Open with the command palette up and this typed into it |
| `--pane <name>` | Which right-dock tab is showing: `backlinks`, `outgoing`, `outline`, `tags`, `properties` |
| `--graph [local]` | Open on the graph view; `local` for the local graph of the open note |
| `--settings [tab]` | Open with Settings up, on a tab: `general`, `appearance`, `editor`, `hotkeys`, … |
| `--menu <key>` | Open the Settings dropdown that writes this configuration key |
| `--find <text>` | Open with the find panel up and this matched |
| `--find-all <text>` | As `--find`, then select every match (a cursor per match) |
| `--rename <name>` | Rename the opened note at startup, rewriting links. **Writes to the vault** |
| `--prompt <kind>` | Open a file-operation prompt: `rename`, `move`, `delete`, `folder` |
| `--cell <n>` | Open this cell of the note's first table for editing |
| `--template <text>` | Open the template chooser and pick the template this matches |
| `--type <text>` | Type this at the cursor in the editor, which is how the completion popup is opened for a screenshot |
| `--accept <keys>` | Press these keys after `--type`: `enter`, `tab`, `shift-tab`, `escape`, `up`, `down` |
| `--command <ids>` | Run these command ids at startup, comma separated |
| `--control <socket>` | Bind the control socket, so a test or an MCP server can drive this window |
| `--synthetic [n]` | Ignore the vault's notes and generate one of `n` lines (default 50,000) |
| `--stress [n]` | Drive `n` self-directed edits on the open note, then report timings (default 600) |
| `--help`, `--version` | Print the flags, or the version, and exit |

Most of the middle of that table exists so a screenshot or a benchmark can be taken without
clicking through the interface, and they are useful for exactly that. A value can be attached
with an equals sign (`--note=Basalt`), and an unknown flag is an error that names itself rather
than being taken for a note.

Three environment variables stand in for flags across a whole session: `SCORIA_FRAME` for
`--frame`, `SCORIA_SINGLE_INSTANCE=0` for `--new-window`, and `SCORIA_CONFIG_DIR` for where
`scoria.json` — the vault list — lives.

> **Warning** `--rename` and `--stress` both write to the vault you pass them. Use a scratch
> copy.

## Desktop entry

On Linux the menu entry, the window icon and the file associations are three installed files
rather than anything inside the binary, and the repository's installer puts them in place:

```shell
tools/linux-desktop/install.sh                  # from the repository or the archive
tools/linux-desktop/install.sh --binary ~/bin/scoria
tools/linux-desktop/install.sh --obsidian-scheme   # only where Obsidian is not installed
tools/linux-desktop/install.sh --uninstall
```

The entry offers Scoria for a folder (a vault), for a Markdown file, and for `scoria://` links.
`obsidian://` is left alone unless you ask for it: on a machine that has Obsidian, that scheme
is Obsidian's, and taking it would break its own links.

## Next

- [Open a vault](/docs/open-a-vault/) — what happens on first launch, and what gets read.
- [Coming from Obsidian](/docs/from-obsidian/) — if you have a vault already.
