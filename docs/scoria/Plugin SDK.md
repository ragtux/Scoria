---
tags:
  - scoria/dev
  - scoria/design
created: 2026-11-15
---
# Plugin SDK

The roadmap's last row is "5. Ecosystem | Native plugin SDK | Not started", and the compatibility tiers promise "a new native plugin API; existing JS plugins are out of scope". This note is the design study to start that phase from: what Obsidian's plugin API offers, how much of it Scoria already models, the runtimes a native SDK could sit on, what plugins mean for the vault's bytes, and a recommendation with a first milestone. Nothing here is built into the app; the runtime spike in `tools/plugin-spike` (see [[#Spike, 2026-11-16]]) sits outside the workspace. Claims about outside projects carry their source; claims about Scoria name the crate and type, as of `c9479a5` (0.6.0).

## What Obsidian's plugin API is

A community plugin is a folder `.obsidian/plugins/<id>/` holding `manifest.json`, `main.js` and an optional `styles.css`, and it runs when its id is listed in `.obsidian/community-plugins.json` and Restricted mode is off ([manifest reference](https://docs.obsidian.md/Reference/Manifest), [plugin security](https://obsidian.md/help/plugin-security)). `main.js` is a CommonJS bundle whose default export subclasses `Plugin`. The API is typed in the MIT-licensed [`obsidian-api`](https://github.com/obsidianmd/obsidian-api) package: one `obsidian.d.ts` of about 300 KB, version 1.14.4 at the time of writing, which imports `@codemirror/state` and `@codemirror/view` as peer dependencies and `moment` as a dependency ([package.json](https://github.com/obsidianmd/obsidian-api/blob/master/package.json)). It also augments the global DOM: `createEl`, `createDiv`, `createSpan` and helpers on `Node` and `HTMLElement` ([obsidian.d.ts](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts)).

The parts a plugin actually uses, grouped:

| Area | Surface ([API reference](https://docs.obsidian.md/Reference/TypeScript+API/Plugin)) |
| --- | --- |
| Lifecycle | `Plugin.onload`, `onunload`, `onExternalSettingsChange`; `register`, `registerEvent`, `registerDomEvent`, `registerInterval` (all undone on unload); `loadData` / `saveData` (the plugin's `data.json`) |
| Commands | `addCommand(Command)`: `id`, `name`, `icon`, `hotkeys`, `repeatable`, `mobileOnly`, and one of `callback`, `checkCallback(checking)`, `editorCallback(editor, view)`, `editorCheckCallback` ([Commands](https://docs.obsidian.md/Plugins/User+interface/Commands)). The id is namespaced as `<plugin-id>:<id>` |
| Chrome | `addRibbonIcon`, `addStatusBarItem`, `Notice`, `Modal`, `SuggestModal` / `FuzzySuggestModal`, `Menu`, the `file-menu` and `editor-menu` events |
| Views | `registerView(type, factory)` with `ItemView` (`getViewType`, `getDisplayText`, `onOpen`, `onClose`, `containerEl`), `registerExtensions(exts, viewType)` to own a file extension ([Views](https://docs.obsidian.md/Plugins/User+interface/Views)) |
| Rendering | `registerMarkdownPostProcessor((el, ctx) => …)` over reading view's HTML, `registerMarkdownCodeBlockProcessor(lang, (source, el, ctx) => …)` for a fenced language, `MarkdownRenderer.render`, `MarkdownRenderChild` ([post processing](https://docs.obsidian.md/Plugins/Editor/Markdown+post+processing)) |
| Editor | `registerEditorExtension(Extension)`: a CodeMirror 6 extension, "the same thing as a CodeMirror 6 extension" in Obsidian's own words, usually a `ViewPlugin` or a `StateField` ([Editor extensions](https://docs.obsidian.md/Plugins/Editor/Editor+extensions)); `registerEditorSuggest(EditorSuggest)`; the `Editor` facade (`getValue`, `replaceRange`, `getCursor`, `transaction`) |
| Settings | `addSettingTab(PluginSettingTab)` whose `display()` builds rows with the `Setting` builder ([Settings](https://docs.obsidian.md/Plugins/User+interface/Settings)) |
| Vault | `Vault.read`, `cachedRead`, `modify`, `process`, `create`, `delete`, `trash`, `rename`, `getMarkdownFiles`, `getAbstractFileByPath`, the `adapter`; events `create`, `modify`, `delete`, `rename` ([Events](https://docs.obsidian.md/Plugins/Events)) |
| Metadata | `MetadataCache.getFileCache` → `CachedMetadata`, `getFirstLinkpathDest`, `resolvedLinks`, `unresolvedLinks`; events `changed`, `resolve`, `resolved`. `FileManager.renameFile` (with link rewriting), `processFrontMatter`, `generateMarkdownLink` |
| Workspace | `getLeaf`, `getActiveViewOfType`, `iterateAllLeaves`, `revealLeaf`, `onLayoutReady`; events `file-open`, `active-leaf-change`, `layout-change`, `editor-change`, `quit` |
| Other | `registerObsidianProtocolHandler(action, …)` for `obsidian://<action>`, `registerHoverLinkSource` for Page preview, `requestUrl` (CORS-free HTTP), `moment`, `Platform` |

Two things in that table shape everything below. The rendering and editor hooks are not abstractions over Obsidian's model: a post-processor is handed a DOM element, and an editor extension is CodeMirror 6 itself. And nothing is sandboxed — "Obsidian cannot reliably restrict plugins to specific permissions or access levels", so a plugin has the app's access to the disk, the network and child processes ([plugin security](https://obsidian.md/help/plugin-security)).

## What Scoria already models

Most of the non-UI surface has a Rust type already, because the core plugins needed it. The plugin API's job here is mostly to *expose* these, not to write them.

| Obsidian | Scoria today | Gap for a plugin API |
| --- | --- | --- |
| `Command`, `addCommand` | `scoria_commands::Command` and `CommandRegistry` (`register`, `apply_overrides`, `retain_enabled`, `plugin_of`) | The registry is dynamic, but what *runs* is not: `RUNNABLE` in `crates/scoria/src/workspace.rs` is a `const &[&str]` and `Workspace::run_command` is one `match id`. Plugin commands need a runtime set and a fallthrough to the plugin host. `hotkeys.json` entries for unknown ids are already kept and registered unnamed, so a plugin's bindings survive today |
| `checkCallback`, `editorCheckCallback` | the palette's own narrowing (`can_run`, the 63 editor-only ids, the table-only ones; see the manual's commands page) | A plugin's `checking` call becomes one more predicate; it must be cheap and synchronous, which constrains the runtime |
| `CachedMetadata`, `getFileCache` | `scoria_cache::CachedMetadata` (links, embeds, tags, headings, sections, list items, blocks, frontmatter links, footnotes) and `scoria_cache::MetadataCache::file` | Field names already follow Obsidian's `Pos`/cache shapes, so a serialised `CachedMetadata` is close to what a JS plugin author expects |
| `resolvedLinks`, `getFirstLinkpathDest` | `MetadataCache::resolved_links`, `unresolved_links`, `resolve`; `scoria_cache::LinkResolver::resolve` | none |
| `Vault`, `TFile`, vault events | `scoria_vault::Vault` (`files`, `markdown_files`, `write_bytes`, `rescan`, `refresh_path`), `VaultFile`, `FileChange::{Added, Modified, Removed}`; the watcher in `crates/scoria/src/watch.rs` | No rename event as such (a rename is Removed + Added to the watcher); a plugin write must go through the same path as Scoria's own so the "skip the reload of what Scoria wrote" rule holds |
| `FileManager.renameFile` | the rename with link rewriting, `scoria_cache::links::rewrite_note`, `Rewrite` | none, beyond exposing it |
| `processFrontMatter` | `scoria_formats::properties` (model, serializer, surgical edit planner, `Change`) | Obsidian's call re-serialises the whole block; Scoria's must stay surgical (see [[#The vault's bytes]]) |
| `loadData` / `saveData` | `Vault::read_config` / `write_config` / `replace_config` with `#[serde(flatten)] extra` | A raw JSON pass-through for `plugins/<id>/data.json`, which Scoria cannot type |
| `PluginSettingTab`, `Setting` | the Settings window, `crates/scoria/src/settings.rs`: `Tab`, `Item`, `Control` (toggle, field, choice, button), `SettingChange` | Rows are built from Rust code per tab; a plugin tab needs `Control` values described as data and a change routed back to the plugin |
| `manifest.json`, `community-plugins.json` | `scoria_formats::config::PluginManifest` (with `extra`), `CommunityPlugins = Vec<String>`, `Vault::plugin_manifests`, `Vault::community_plugins`; the Community plugins tab lists them, each toggle marked "not in Scoria yet" | The runtime itself |
| `registerMarkdownCodeBlockProcessor` | the reading model's `RBlock::Code { language, text, … }` in `crates/scoria/src/reading/model.rs`; the reading view's highlighting already dispatches on the fence's language | A table of language → plugin renderer, and a drawing a plugin can return (below) |
| `registerMarkdownPostProcessor` | `reading::model::build` turns `scoria_markdown::Document` into `RBlock`s; there is no HTML DOM in the reading view | A post-processor cannot get an element; it would get the `RBlock` tree or the parser's AST |
| `registerEditorSuggest` | `scoria_suggest` (`trigger`, `candidates`, `insert`, `slash`) | Triggers are a fixed set; a plugin trigger is one more `Triggered` source |
| `registerHoverLinkSource` | `hover_preview::Source`, a closed enum mapped to `page-preview.json` keys | Make it open for plugin sources |
| `registerObsidianProtocolHandler` | `uri::Uri` and `uri::parse` (`open`, `new`, `search`, `daily`) | An unknown action falls to a plugin table |
| `Notice`, `Modal`, `SuggestModal`, `Menu` | `Workspace::report` (notices), `prompt.rs`, `chooser.rs`, the switcher, `file_menu::Item` / `items(Target)` | Each needs a data description a plugin can send |
| `ItemView`, `registerView` | the workspace's leaves (`MainView`, `RightTab`) and the `workspace.json` round trip | The hardest gap: a view is arbitrary UI (see below) |
| `registerEditorExtension` | `scoria_text` (buffer, selections, edit plans) and the editor in `crates/scoria/src/editor.rs` | No equivalent of CodeMirror's extension system; the editor would need decoration and transaction hooks of its own |

The pattern: the *model* half of Obsidian's API (commands, metadata, vault, links, frontmatter, settings data) is already in gpui-free crates; the *UI* half (views, post-processors, editor extensions, modals) has no counterpart a plugin could reach, because Scoria draws with gpui elements rather than a DOM.

## The runtime options

Five ways to run third-party code in a native Rust app. Each is judged on sandboxing, ABI stability, cross-platform builds (Linux, Windows, macOS, which Scoria ships), hot reload, and how hard it is to write a plugin.

### WebAssembly components (wasmtime, or extism over it)

A plugin is a `.wasm` file; the host is [Wasmtime](https://docs.wasmtime.dev/), whose `wasmtime::component` module runs WebAssembly components against an interface written in WIT and generates host bindings with `bindgen!` ([wasmtime::component](https://docs.wasmtime.dev/api/wasmtime/component/index.html), [the component model](https://component-model.bytecodealliance.org/)). [Extism](https://github.com/extism/extism) is a plugin framework on the same idea, BSD-3-Clause, built on wasmtime (its runtime depends on `wasmtime` 49, [runtime/Cargo.toml](https://github.com/extism/extism/blob/main/runtime/Cargo.toml)), with plugin kits for Rust, JS, Python, Go, C, C++, Zig, .NET, Haskell and AssemblyScript; it passes bytes (usually JSON) rather than WIT types.

The precedent that matters most is Zed, whose gpui Scoria is built on: Zed extensions are Rust compiled to Wasm, loaded into Wasmtime as components, their API defined in WIT with `wit_bindgen` on the guest and `wasmtime::component::bindgen!` on the host, and called asynchronously so "a slow extension doesn't block Zed" ([Zed decoded: extensions](https://zed.dev/blog/zed-decoded-extensions)). Zed's API covers languages, themes, snippets and slash commands, and not new UI panels or unrestricted file access (same source).

- **Sandboxing:** the best on offer. A component sees only the imports the host gives it; no disk, network or clock unless granted. Runaway code is stopped with fuel or epoch interruption ([interrupting Wasm](https://docs.wasmtime.dev/examples-interrupting-wasm.html)).
- **ABI:** stable by construction — the WIT file is the ABI, versioned like any interface. A plugin built once runs on every host that speaks its world.
- **Cross-platform:** one `.wasm` for all three platforms and both architectures; the plugin author ships a single file. Wasmtime is Rust (with Cranelift) and builds on all three.
- **Hot reload:** drop the instance, instantiate the new file. Trivial.
- **Writing a plugin:** Rust is first class; other languages work through their component toolchains with varying maturity. JS through a component toolchain or extism's JS kit is possible but means shipping a JS engine inside the `.wasm`.
- **Cost:** wasmtime is a large dependency (compile time and binary size), and every call crosses a copy boundary, so passing a whole `CachedMetadata` per keystroke is not free.

### Dynamic Rust libraries (`cdylib` / `dylib`)

A plugin is a `.so` / `.dll` / `.dylib` loaded with `libloading`.

- **Sandboxing:** none. It is native code in the process.
- **ABI:** Rust has no stable ABI; the layout of `repr(Rust)` types is unspecified ([Rust reference: ABI](https://doc.rust-lang.org/reference/abi.html)), so a plugin must be built with the same compiler and the same dependency versions as the app, or everything crosses an `extern "C"` boundary. [`abi_stable`](https://github.com/rodrimati1992/abi_stable_crates) makes that boundary tolerable but is a framework of its own.
- **Cross-platform:** every plugin is built three times (six with arm64), and on macOS each must be signed for the app to load it.
- **Hot reload:** fragile; unloading a library with live thread-locals or callbacks is a crash waiting to happen.
- **Writing a plugin:** easiest for a Rust author who builds Scoria, impossible for anyone else.

This is only defensible for Scoria's own optional features, not for a community.

### An embedded scripting language

- **[Rhai](https://rhai.rs):** a Rust-native scripting language, sandboxed in the sense that an immutable engine "cannot mutate the containing environment", protected against stack overflow, oversized data and runaway scripts with an operation limit ([features](https://rhai.rs/book/about/features.html), [max operations](https://rhai.rs/book/safety/max-operations.html)). Builds anywhere Rust does. Slow next to Wasm and a language nobody writes outside Rhai hosts.
- **Lua through [mlua](https://github.com/mlua-rs/mlua):** Lua 5.1–5.5, LuaJIT and Luau, MIT; its `vendored` feature compiles the Lua C sources, so a C compiler is in the build on every platform; `Lua::sandbox` exists for Luau only (same source). Lua is a known plugin language (Neovim, many games), fast, small.
- **JavaScript through [Boa](https://github.com/boa-dev/boa)** (Rust, MIT/Unlicense, "more than 90%" of the latest ECMAScript, 0.21 at the time of writing) or **[rquickjs](https://github.com/DelSkayn/rquickjs)** (MIT bindings to QuickJS-NG, which "needs to compile a C-library"; Windows MSVC marked experimental by QuickJS). JS is the language Obsidian's plugin authors already write.

All three: hot reload is easy (re-evaluate), there is no ABI beyond the bindings Scoria writes, and sandboxing is whatever the host does not expose. The weakness they share is that the host API is hand-bound per language, and there is no type checker between the plugin and the API unless Scoria ships one (`.d.ts` for JS, Luau types).

### Out of process, over a socket

Not in the brief's list but already half-built: `scoria-control` is a line-delimited JSON protocol on a Unix socket (Windows included, `scoria_control::net`) with a method table, `METHODS`, that the MCP server builds its tool schemas from. A plugin could be any executable speaking an extension of it, the way LSP servers or Neovim's remote plugins work. Isolation is the OS process; any language works; a crash cannot take the app down. But every call is a round trip, so a `checkCallback` on every palette keystroke or a post-processor on every reparse is too slow, and the user has to trust and install an executable. Good for integrations (a sync tool, an LLM), poor as the general SDK.

### Running Obsidian's own community plugins (JS engine plus a DOM shim)

The question every user will ask. The answer is no, and this is why, plainly:

1. **The API is the DOM.** Post-processors, views, setting tabs, modals, ribbon icons and status bar items all receive an `HTMLElement` and build UI with `createEl` and CSS classes, styled by the plugin's `styles.css` against Obsidian's own class names. A shim would have to implement enough of the DOM — layout, CSS cascade, events, `getBoundingClientRect` — to draw what plugins build. That is a browser engine. [[Theming]] already found the same wall for theme CSS rules: with no DOM there is nothing for a selector to select.
2. **The editor is CodeMirror 6.** `registerEditorExtension` takes CM6 `ViewPlugin`s and `StateField`s that read CM6's syntax tree and write CM6 decorations ([Editor extensions](https://docs.obsidian.md/Plugins/Editor/Editor+extensions)). Supporting them means running CodeMirror inside Scoria and mirroring its state into `scoria_text`, which is a second editor and, with its Lezer Markdown grammar, a second Markdown tokenizer — forbidden by the ground rules in `CLAUDE.md`.
3. **They use Node and Electron.** Plugins are allowed and expected to `require` Node's `fs`, `path`, `child_process` and Electron where `isDesktopOnly` is true ([manifest](https://docs.obsidian.md/Reference/Manifest)), and many popular ones do. A shim for those is a reimplementation of Node.
4. **They reach past the public API.** The typings are not the whole contract: plugins routinely touch undocumented internals (`app.plugins`, `app.commands`, `app.internalPlugins`, private fields of views) and Obsidian's own DOM structure. Matching a private, changing surface across Obsidian releases is unbounded work.
5. **Semantics, not just syntax.** Even the clean parts (Vault, MetadataCache) carry timing and event-order behavior that plugins depend on (`onLayoutReady`, `resolved` firing once). Each would need probing to the standard [[Specification]] holds the core plugins to, per plugin.

A partial shim — the model half only, commands plus Vault plus MetadataCache over Boa or rquickjs — would run a small class of headless plugins (formatters, linters, some automation). But users would not experience it as "Obsidian plugins work": most popular plugins (Dataview, Templater, Excalidraw, Kanban, Calendar) draw UI or patch the editor, and would fail in ways that look like Scoria bugs. The cost is a permanent compatibility treadmill for a fraction of the ecosystem. The manual already commits to this in "What differs from Obsidian", under *Not ever*; this study agrees and recommends keeping it there.

### Compared

| | Sandbox | ABI | Builds | Hot reload | Writing one |
| --- | --- | --- | --- | --- | --- |
| Wasm components | strong, capability based | WIT, versioned | one file for all | trivial | Rust easy; others vary |
| Rust dylibs | none | none (same rustc) | per platform, signed | fragile | Rust, in-tree only |
| Rhai | good | host bindings | anywhere Rust builds | trivial | new language |
| Lua (mlua) | Luau only | host bindings | C compiler | trivial | familiar, untyped |
| JS (Boa / rquickjs) | host-defined | host bindings | Boa pure Rust; rquickjs C | trivial | familiar to Obsidian authors |
| Out of process | OS process | JSON protocol | the plugin's own | restart it | any language |
| Obsidian JS + DOM shim | none in practice | Obsidian's, moving | a browser engine | — | none to write; most do not run |

## UI: what a plugin may draw

Whichever runtime, a plugin cannot hand Scoria a gpui element: gpui is a Rust API with lifetimes and closures, it is pinned to a Zed commit that moves (see [[Development#Environment]]), and exposing it would freeze that pin. So plugin UI is *data the host draws*:

- **Commands, notices, menus, suggest modals, ribbon and status bar items:** small descriptions (label, icon name from the Lucide set the app bundles, enabled, checked), drawn by the code that draws Scoria's own.
- **Settings tabs:** a list of rows in the shape `settings.rs` already uses (`Item` with a `Control`): heading, toggle, text field, dropdown, button. That covers most of what `Setting` builders produce.
- **Rendered blocks (code-block processors, post-processors):** a plugin returns Markdown for the one parser to render (the simplest, and it keeps the one-parser rule), or a small block vocabulary close to `RBlock` (paragraph, list, table, callout, code, image from bytes or a vault path). An SVG result can go through the same path as the math renderer's drawing list.
- **Views:** deferred. A view of its own is the largest, least-bounded surface; the first versions should not have one.

## Security model

Obsidian's model is all or nothing: Restricted mode on means no community code, off means every enabled plugin has the app's full access ([plugin security](https://obsidian.md/help/plugin-security)). A Wasm host lets Scoria do better at no cost to authors:

- **Capabilities in the manifest, granted at enable time.** Read the vault, write the vault, read and write the plugin's own `data.json`, network (to listed hosts), clipboard, run commands. A plugin with only "read vault" cannot corrupt it.
- **Vault writes go through the host.** A plugin never gets a path or a file handle; it asks for `modify(path, text)` or a planned edit, which the host applies through `scoria_vault` (no-follow writes, the vault-root check in `Vault::write_path`), the same path as Scoria's own edits, and which undo and the watcher see.
- **Budgets.** Fuel or epochs per call, and a wall-clock limit on `checkCallback`-style predicates so a slow plugin greys out its command rather than freezing the palette.
- **Off by default**, with Obsidian's switch: Scoria should honour the same Restricted mode state Obsidian keeps, so a vault that is restricted in one app is restricted in the other. Where Obsidian stores it is not yet on record (see [[#Open questions]]).

## The vault's bytes

The ground rule is that the vault is the only source of truth and Scoria never writes a file it did not change. Plugins add three files to that rule:

- **`community-plugins.json`.** Obsidian's list of enabled ids. Scoria reads it today and writes it only when a toggle in its Community plugins tab changes it, through `Vault::write_config`. If Scoria plugins share the list, enabling one in Scoria also tells Obsidian to load it, and Obsidian will fail to load a folder with no `main.js`. So either Scoria's plugins keep a list of their own, or the shared list is used only for dual plugins (below). Never rewrite the file on launch, never sort it, never drop an id Scoria does not know.
- **`.obsidian/plugins/<id>/`.** Obsidian's community browser installs and updates the folder's `manifest.json`, `main.js` and `styles.css`. Scoria must not write `main.js` or `styles.css`, and must preserve unknown manifest keys (`PluginManifest` already has `#[serde(flatten)] extra`). One attractive design is the **dual plugin**: the same folder carrying both `main.js` for Obsidian and a `.wasm` for Scoria, sharing one id, one manifest and one `data.json`, so a plugin author can serve both apps and the user enables it once. Whether Obsidian's updater leaves an extra file alone is untested.
- **`data.json`.** Written by the plugin through the host's `saveData`. The host writes only when the plugin calls it, and skips the write when the bytes are unchanged, so opening a vault in Scoria never touches a plugin's settings. A dual plugin's `data.json` is shared with its Obsidian half; its schema is the author's concern, and the host must not reformat it.

And for notes: a plugin's edit is a byte-range edit like Scoria's own (`scoria_cache::links::apply_edits`, the properties planner). `processFrontMatter`, if offered, must be Scoria's surgical frontmatter edit, not Obsidian's re-serialisation — the manual lists that as a deliberate difference, and a plugin must not be a way round it.

Scoria-only plugin folders and an enabled list of Scoria's own would be new file names in a vault. `CLAUDE.md` asks for names to match Obsidian's observed strings; a name Obsidian never wrote is a decision to record in [[Specification]] before it is made.

## Recommendation

**WebAssembly components on wasmtime, with a WIT world of Scoria's own; Rust first.** It is the only option that is sandboxed, ABI-stable and one-file cross-platform at once, it hot-reloads, and it is the architecture gpui's own editor chose for the same problem. Extism is worth a spike as the host layer if JSON payloads and its multi-language kits outweigh WIT's typed bindings; the WIT route is the one with a typed contract and Zed's precedent. The spike of both, [[#Spike, 2026-11-16]], chose WIT. A JS kit can come later as a component that embeds a JS engine, which gives Obsidian authors a familiar language without promising their existing plugins run.

The API should mirror Obsidian's *names and shapes* where the concept is the same — `addCommand` with `id`/`name`/`checkCallback`, `CachedMetadata` with Obsidian's field names, `<plugin-id>:<command>` ids, `data.json` — so an Obsidian author recognises it and a port is mechanical for the model half. It should not mirror the DOM or CodeMirror.

Keep "Obsidian's community plugins will not run" in the manual's *Not ever*.

## First milestone

Commands and code-block processors over Wasm, nothing else:

1. **`crates/scoria-plugin`** (no gpui): the WIT world, the wasmtime host, manifest and capability parsing, loading from a folder, fuel limits. Unit-tested with tiny guest components built in the crate's tests, in a `tempfile::tempdir()` vault.
2. **Commands.** A plugin registers commands with a name and an optional check; the palette and hotkeys list them through `CommandRegistry::register`; `RUNNABLE` becomes the built-ins plus the plugins' ids; `run_command` falls through to the host. A command may call `vault.read`, `vault.modify` (capability gated) and `notice`.
3. **Code-block processors.** A plugin claims a fence language; the reading model's `RBlock::Code` for that language is replaced by the Markdown the plugin returns, rendered by `scoria-markdown`. Errors draw the block's source with the message, as an unknown math command does now.
4. **Read-only metadata.** `metadata.file(path)` returning `CachedMetadata`, `metadata.resolve(linktext, source)`, `vault.files()`.
5. **`data.json`** load and save, with the byte rule above.
6. **A sample plugin** in the repo (a word-count command and a `chart`-style fence) and a page in the manual.

Out of the milestone, in rough order after it: settings tabs as data, vault and metadata events, suggest and menus, post-processors over the block tree, protocol handlers and hover sources, a JS kit, and only then views and editor decorations.

## Spike, 2026-11-16

The two open questions that had to be answered before anything enters the workspace — what wasmtime costs in binary size and build time, and WIT components or extism — were settled with a spike of milestone step 2 on both routes. It lives in `tools/plugin-spike`, outside the cargo workspace like `docs/anya_scoria_gen` (each crate has its own empty `[workspace]` table), and `tools/plugin-spike/run.sh` rebuilds and re-measures everything; `run-windows.ps1` is its Windows half.

### What was built

- **`wit/plugin.wit`**, the world: imports `vault` (`read(path) -> result<string, string>`, `modify(path, text) -> result<_, string>`) and `workspace` (`notice(message)`); exports `commands() -> list<command>` (a record of `id`, `name`, `has-check`), `check(id, active-path) -> bool` and `run(id, active-path) -> result<_, string>`.
- **Two guests, one small Rust crate each**, registering the same two commands: `count-words`, with a check callback (true when the active file ends in `.md`), which reads the note and posts a notice; and `append-line`, without one, which reads the note, writes it back with a line appended through `vault.modify`, and posts a notice. `guest-wit` uses `wit-bindgen` 0.63; `guest-extism` uses `extism-pdk` 1.4 with JSON in and out.
- **Three hosts**, all over a tempdir vault with one note, sharing `common/spike.rs` (the vault, the path rule that refuses anything but plain components, the timers): `host-baseline` runs the same commands as plain Rust; `host-wit` is wasmtime's component model with `bindgen!`, one cargo feature per compiler tier; `host-extism` is extism 1.41. Each checks that the note on disk ends with the appended line.

Both routes ran on wasmtime 49 (extism 1.41 depends on `wasmtime` ^49), built with Rust 1.100.0 from a rustup home inside the spike's target directory, because wasmtime 49 needs rustc 1.96 and Scoria's toolchain is 1.95 (see the toolchain notes below). Release profile with `strip = true` and otherwise Cargo's defaults, as the workspace's release profile is.

### Numbers, Linux

x86_64 NixOS, Threadripper PRO 3995WX (64 cores, 128 threads), local ZFS. Other agents were building the main workspace at the same time (load average 7 to 28), so build times are good to perhaps 10 percent. Sizes are the stripped binary; "added" is over the baseline host's 0.40 MB. Clean builds start from an empty target directory with the crates already downloaded; the incremental build follows a `touch` of `main.rs`.

| Host | Size | Added | Clean, `-j 128` | Clean, `-j 8` | Incremental |
| --- | --- | --- | --- | --- | --- |
| baseline (no runtime) | 0.40 MB | — | 0.4 s | 0.3 s | 0.3 s |
| WIT, wasmtime's default features | 18.59 MB | 18.19 MB | 51.9 s | 68.1 s | 1.5 s |
| WIT, Cranelift | 13.91 MB | 13.51 MB | 53.4 s | 64.0 s | 0.9 s |
| WIT, Cranelift + `parallel-compilation` | 14.08 MB | 13.68 MB | 54.3 s | 65.3 s | 0.8 s |
| WIT, Winch | 13.09 MB | 12.69 MB | 48.1 s | 53.0 s | 1.0 s |
| WIT, Cranelift targeting Pulley | 14.02 MB | 13.62 MB | 52.4 s | 59.5 s | 0.8 s |
| WIT, no compiler (loads precompiled native code) | 1.57 MB | 1.17 MB | 23.5 s | 25.5 s | 0.7 s |
| WIT, no compiler, Pulley interpreter | 1.68 MB | 1.28 MB | 23.6 s | 24.2 s | 0.8 s |
| extism, its default features | 22.33 MB | 21.93 MB | 61.6 s | 96.2 s | 1.4 s |
| extism, `default-features = false` | 18.45 MB | 18.05 MB | 61.1 s | 81.4 s | 1.1 s |

"Cranelift", "Winch" and "Pulley" are `wasmtime` with `default-features = false` and `std`, `runtime`, `component-model` plus that tier. Scoria's own release binary, stripped, is 54.1 MB (the 2026-11-10 build), so the recommended host adds about 25 percent to it. The clean build barely moves between 8 and 128 jobs because it is one long serial chain (`cranelift-codegen` and `wasmtime` itself); inside the workspace it would overlap with gpui's own long crates, which this spike did not measure.

Load and call times, from the same binaries. The guest is the 23 KB component (extism's is 120 KB, most of it `serde_json`). Compile is one component from bytes with wasmtime's cache off; instantiate is a new `Store` and instance from a pre-linked `InstancePre`, median of 200; check is one `check("count-words", "Notes/Spike.md")` from the host, median and 99th percentile of 100,000 calls, the extism one including the JSON both ways as the palette would pay it.

| Host | Engine | Compile | First instantiate | Instantiate | Check, median | Check, p99 |
| --- | --- | --- | --- | --- | --- | --- |
| baseline, a Rust function | — | — | — | — | 0.03 µs | 0.03 µs |
| WIT, default features | 0.16 ms | 15.4 ms | 86 µs | 19 µs | 0.79 µs | 1.19 µs |
| WIT, Cranelift | 0.13 ms | 28.2 ms | 74 µs | 16 µs | 0.39 µs | 0.44 µs |
| WIT, Cranelift + parallel | 0.12 ms | 16.8 ms | 74 µs | 16 µs | 0.37 µs | 0.48 µs |
| WIT, Winch | 0.15 ms | 10.0 ms | 62 µs | 16 µs | 0.41 µs | 0.67 µs |
| WIT, Pulley | 0.13 ms | 26.9 ms | 97 µs | 25 µs | 1.49 µs | 1.87 µs |
| WIT, no compiler, native | 0.05 ms | 0.20 ms (deserialize) | 82 µs | 16 µs | 0.39 µs | 0.62 µs |
| WIT, no compiler, Pulley | 0.04 ms | 0.19 ms (deserialize) | 72 µs | 24 µs | 1.52 µs | 1.82 µs |
| extism, default features | — | 38.0 ms | 378 µs | 133 µs | 5.26 µs | 8.47 µs |
| extism, minimal | — | 39.5 ms (2.1 ms from its disk cache) | 391 µs | 136 µs | 5.08 µs | 8.07 µs |

Budgets cost nothing measurable on the compiled tiers: with `epoch_interruption` or `consume_fuel` on, and the deadline or fuel re-armed before every call, the Cranelift check was 0.31 and 0.32 µs and the Winch one 0.33 and 0.37 µs, inside the noise of the unbudgeted runs. On Pulley they cost 18 and 30 percent (1.76 and 1.94 µs). A run of either command, which crosses into the host for a file read (and a write) and a notice, took 30 to 150 µs on every host, dominated by the file system; one Winch run of `append-line` took 7.3 ms once and is taken as a disk stall.

A keystroke at 60 Hz is 16.7 ms. The slowest check measured, extism's p99, would let one keystroke ask about 2,000 checked commands; the Cranelift one about 38,000.

### Numbers, Windows

`run-windows.ps1` on `w11`: Windows 11 in a VM with 16 virtual CPUs of a Threadripper PRO 3995WX, `x86_64-pc-windows-msvc`, Rust 1.100.0, a subset of the hosts, clean builds at cargo's default 16 jobs. The timer there ticks in 0.1 µs, so the check times are rounded to it. A run of a command takes 200 to 700 µs on Windows against 30 to 150 µs on Linux, which is the file system, not the runtime.

| Host | Size | Added | Clean | Incremental | Compile | Instantiate | Check, median | Check, p99 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline (no runtime) | 0.21 MB | — | 8.8 s | 6.0 s | — | — | 0.0 µs | 0.1 µs |
| WIT, Cranelift | 13.48 MB | 13.27 MB | 196.1 s | 2.4 s | 31.0 ms | 14 µs | 0.5 µs | 0.6 µs |
| WIT, Cranelift + parallel | 13.66 MB | 13.45 MB | 167.2 s | 2.5 s | 19.3 ms | 12 µs | 0.5 µs | 0.7 µs |
| WIT, Winch | 13.03 MB | 12.82 MB | 155.5 s | 2.8 s | 14.2 ms | 14 µs | 0.5 µs | 0.9 µs |
| WIT, no compiler, native | 1.30 MB | 1.09 MB | 41.6 s | 1.8 s | 0.48 ms (deserialize) | 14 µs | 0.5 µs | 0.8 µs |
| extism, minimal | 18.25 MB | 18.04 MB | 199.4 s | 3.5 s | 46.6 ms | 135 µs | 5.1 µs | 24.6 µs |

The sizes added match Linux's to within 0.3 MB. The clean builds take about three times as long as Linux's at `-j 8` (196 s against 64 s for Cranelift), on a VM and with MSVC's linker; the incremental ones stay within a few seconds.

### Numbers, macOS

Still to be taken, on the macmini: `run.sh` needs `rustup`, `wasm-tools` and `lld` on the path there, and GNU `stat` and `nproc` or two small edits. It is the first aarch64 measurement too; Cranelift and Winch both have aarch64 back ends, and none of the numbers above says how they do on one.

### Toolchains

- **Rust version.** wasmtime raises its minimum Rust with every monthly major: 48 needs 1.95, 49 needs 1.96, 50 needs 1.97 (crates.io's `rust_version` for each). Scoria builds with nixpkgs' rustc 1.95, so the newest wasmtime it can take today is 48; the same `host-wit` source builds on wasmtime 48 with that rustc (14.10 MB for Cranelift + parallel, check 0.49 µs median). Wasmtime 48 is also a long-term-support release: every major divisible by 12 is one, supported for 24 months ([release process](https://docs.wasmtime.dev/stability-release.html)). extism follows wasmtime a major behind and pins one (`^49` in 1.41, `^43` in 1.30), so on extism Scoria's wasmtime version would be extism's choice, and today it needs Rust 1.96.
- **Guest targets.** nixpkgs' rustc ships `wasm32-unknown-unknown` (and `wasm32v1-none`) but not `wasm32-wasip1` or `wasm32-wasip2`, and has no `lld` on the path, which rustc calls to link wasm. `tools/plugin-spike/shell.nix` adds `lld`, `wasm-tools` 1.249 and `rustup`; `rustup` with `RUSTUP_HOME` inside the spike's target directory installs `wasm32-wasip2` without touching the system or `~/.rustup`. `cargo-component` 0.21.1 is in nixpkgs as well, and was not needed.
- **The WIT guest needs no WASI.** Built for `wasm32-unknown-unknown` with `wit-bindgen`, then wrapped by `wasm-tools component new`, the component imports exactly the world's three interfaces. Built for `wasm32-wasip2`, the same source comes out as a 53 KB component that also imports fourteen `wasi:cli`, `wasi:io` and `wasi:clocks` interfaces, which Rust's standard library pulls in; a host would have to link `wasmtime-wasi` or stub them. The plugin kit should target `wasm32-unknown-unknown`, which keeps the capability list equal to the world.
- **The extism guest** builds for `wasm32-unknown-unknown` with `extism-pdk` and nothing else.

### What it settles

- **WIT components, not extism.** Both routes were equally short to write in Rust. Against extism: its host is 4.4 MB larger than the WIT host even with its default features off, because it fixes wasmtime's `cache`, `gc`, `coredump`, `wat` and WASI preview 1 on; its calls are 13 times slower (JSON both ways) and its instances 8 times slower to make; it ties Scoria's wasmtime version to its own, a major behind; it turns on wasmtime's disk cache by default, which writes `~/.cache/wasmtime` unless the host passes `with_cache_disabled`; and its contract is bytes, with nothing checking a plugin's JSON against the host's. What extism offers in return, plugin kits in many languages, matters after the Rust kit, and a component toolchain covers other languages too.
- **Cranelift, with `parallel-compilation`.** Compiling a plugin is a load-time cost paid once (16.8 ms for the spike's guest), and the host can keep `Component::serialize`'s output in Scoria's own cache directory, keyed by the wasmtime version, so a second launch deserializes in 0.2 ms. Winch compiles faster and calls as fast for this guest but saves only 0.8 MB, because it reuses Cranelift's code generator, and it gives up Cranelift's optimisation on plugins that do real work. Pulley calls four times slower and saves nothing while a compiler is linked; it is for targets Cranelift does not support, which Scoria does not ship. The compiler-less runtime (1.2 MB added) is the cheapest by far, but its precompiled code is specific to the platform and to the wasmtime version, which gives up the one-file plugin.
- **A cargo feature.** About 13.7 MB of binary and a near-minute serial chain in a clean build is enough to keep out of builds that do not want it, and the host is a self-contained crate. `scoria-plugin` should be an optional dependency of `scoria` behind a `plugins` feature, on in the default features so releases have it, off with `--no-default-features` for a lighter build; the incremental cost with it on is under a second. Pin wasmtime 48, the long-term-support release that builds with the toolchain Scoria already has, and move it by itself, one long-term-support release at a time, as the gpui pin moves.
- **Sync checks with a budget.** The check callback answers in under half a microsecond and an epoch deadline costs nothing measurable, so `check` can be a synchronous call on the palette's keystroke, guarded by an epoch deadline a ticker thread advances; `run` and anything that touches the vault can stay synchronous for milestone step 2 too, and become async when an API call can block (network, large reads).

## Open questions

- **Where Scoria plugins live and how they are enabled.** Dual plugins in `.obsidian/plugins/<id>/` sharing `community-plugins.json`, or a folder and list of Scoria's own? Probe what Obsidian does with a plugin folder that has a manifest and no `main.js`, and whether its updater keeps an unknown file in the folder.
- **Restricted mode.** Where Obsidian stores it, and whether Scoria should share the switch.
- **WIT or extism.** Settled 2026-11-16: WIT components. The spike of both on milestone step 2 found extism's host 4.4 MB larger, its calls 13 times slower and its wasmtime version a major behind and its own; see [[#What it settles]].
- **Async.** Zed calls extensions asynchronously; a `checkCallback` must answer within a palette keystroke. Partly settled 2026-11-16: a check is 0.4 µs and an epoch deadline costs nothing measurable, so checks are synchronous with a budget (see [[#What it settles]]). Which later calls go async (network, large reads) is still open.
- **What a post-processor sees.** The parser's spanned AST from `scoria-markdown`, the reading view's `RBlock`s, or only rendered Markdown in and out? The first is the most powerful and the most expensive to keep stable.
- **Editor extensions.** Whether decorations (a plugin colouring ranges, adding a widget at a position) can be offered without exposing `scoria_text` internals, and what Live Preview does with them.
- **Distribution.** A registry of Scoria plugins, or install from a URL or a folder only. Signing.
- **Versioning.** How the WIT world is versioned against Scoria's releases, and what `minAppVersion` means for a Scoria plugin.
- **Binary size and build time.** Measured on Linux and Windows 2026-11-16 (see [[#Spike, 2026-11-16]]): wasmtime with Cranelift adds about 13.7 MB stripped and a clean build of about a minute on Linux, so it goes behind a `plugins` cargo feature, on by default. The macOS numbers are still to be taken.

## Sources

- Obsidian developer docs: [Plugin API](https://docs.obsidian.md/Reference/TypeScript+API/Plugin), [Manifest](https://docs.obsidian.md/Reference/Manifest), [Commands](https://docs.obsidian.md/Plugins/User+interface/Commands), [Views](https://docs.obsidian.md/Plugins/User+interface/Views), [Settings](https://docs.obsidian.md/Plugins/User+interface/Settings), [Events](https://docs.obsidian.md/Plugins/Events), [Markdown post processing](https://docs.obsidian.md/Plugins/Editor/Markdown+post+processing), [Editor extensions](https://docs.obsidian.md/Plugins/Editor/Editor+extensions)
- Obsidian help: [Plugin security](https://obsidian.md/help/plugin-security)
- [obsidianmd/obsidian-api](https://github.com/obsidianmd/obsidian-api) (MIT): `obsidian.d.ts`, `package.json`
- Wasmtime: [docs](https://docs.wasmtime.dev/), [`wasmtime::component`](https://docs.wasmtime.dev/api/wasmtime/component/index.html), [interrupting Wasm](https://docs.wasmtime.dev/examples-interrupting-wasm.html), [release process](https://docs.wasmtime.dev/stability-release.html); [the component model](https://component-model.bytecodealliance.org/)
- [Extism](https://github.com/extism/extism) and its [runtime manifest](https://github.com/extism/extism/blob/main/runtime/Cargo.toml)
- [Zed decoded: extensions](https://zed.dev/blog/zed-decoded-extensions)
- [Rust reference: application binary interface](https://doc.rust-lang.org/reference/abi.html); [abi_stable](https://github.com/rodrimati1992/abi_stable_crates)
- [Rhai features](https://rhai.rs/book/about/features.html), [Rhai max operations](https://rhai.rs/book/safety/max-operations.html); [mlua](https://github.com/mlua-rs/mlua); [Boa](https://github.com/boa-dev/boa); [rquickjs](https://github.com/DelSkayn/rquickjs)
