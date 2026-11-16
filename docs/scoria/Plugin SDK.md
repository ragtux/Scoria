---
tags:
  - scoria/dev
  - scoria/design
created: 2026-11-15
---
# Plugin SDK

The roadmap's last row is "5. Ecosystem | Native plugin SDK | Not started", and the compatibility tiers promise "a new native plugin API; existing JS plugins are out of scope". This note is the design study to start that phase from: what Obsidian's plugin API offers, how much of it Scoria already models, the runtimes a native SDK could sit on, what plugins mean for the vault's bytes, and a recommendation with a first milestone. Nothing here is built. Claims about outside projects carry their source; claims about Scoria name the crate and type, as of `c9479a5` (0.6.0).

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

**WebAssembly components on wasmtime, with a WIT world of Scoria's own; Rust first.** It is the only option that is sandboxed, ABI-stable and one-file cross-platform at once, it hot-reloads, and it is the architecture gpui's own editor chose for the same problem. Extism is worth a spike as the host layer if JSON payloads and its multi-language kits outweigh WIT's typed bindings; the WIT route is the one with a typed contract and Zed's precedent. A JS kit can come later as a component that embeds a JS engine, which gives Obsidian authors a familiar language without promising their existing plugins run.

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

## Open questions

- **Where Scoria plugins live and how they are enabled.** Dual plugins in `.obsidian/plugins/<id>/` sharing `community-plugins.json`, or a folder and list of Scoria's own? Probe what Obsidian does with a plugin folder that has a manifest and no `main.js`, and whether its updater keeps an unknown file in the folder.
- **Restricted mode.** Where Obsidian stores it, and whether Scoria should share the switch.
- **WIT or extism.** Typed WIT bindings with the component toolchain, or extism's byte-and-JSON calls with its broader plugin kits. A spike of each on milestone step 2 settles it.
- **Async.** Zed calls extensions asynchronously; a `checkCallback` must answer within a palette keystroke. Which calls are sync with a budget and which are async?
- **What a post-processor sees.** The parser's spanned AST from `scoria-markdown`, the reading view's `RBlock`s, or only rendered Markdown in and out? The first is the most powerful and the most expensive to keep stable.
- **Editor extensions.** Whether decorations (a plugin colouring ranges, adding a widget at a position) can be offered without exposing `scoria_text` internals, and what Live Preview does with them.
- **Distribution.** A registry of Scoria plugins, or install from a URL or a folder only. Signing.
- **Versioning.** How the WIT world is versioned against Scoria's releases, and what `minAppVersion` means for a Scoria plugin.
- **Binary size and build time.** wasmtime with Cranelift is a heavy dependency; measure it on the three platforms before committing, and whether it can be a cargo feature.

## Sources

- Obsidian developer docs: [Plugin API](https://docs.obsidian.md/Reference/TypeScript+API/Plugin), [Manifest](https://docs.obsidian.md/Reference/Manifest), [Commands](https://docs.obsidian.md/Plugins/User+interface/Commands), [Views](https://docs.obsidian.md/Plugins/User+interface/Views), [Settings](https://docs.obsidian.md/Plugins/User+interface/Settings), [Events](https://docs.obsidian.md/Plugins/Events), [Markdown post processing](https://docs.obsidian.md/Plugins/Editor/Markdown+post+processing), [Editor extensions](https://docs.obsidian.md/Plugins/Editor/Editor+extensions)
- Obsidian help: [Plugin security](https://obsidian.md/help/plugin-security)
- [obsidianmd/obsidian-api](https://github.com/obsidianmd/obsidian-api) (MIT): `obsidian.d.ts`, `package.json`
- Wasmtime: [docs](https://docs.wasmtime.dev/), [`wasmtime::component`](https://docs.wasmtime.dev/api/wasmtime/component/index.html), [interrupting Wasm](https://docs.wasmtime.dev/examples-interrupting-wasm.html); [the component model](https://component-model.bytecodealliance.org/)
- [Extism](https://github.com/extism/extism) and its [runtime manifest](https://github.com/extism/extism/blob/main/runtime/Cargo.toml)
- [Zed decoded: extensions](https://zed.dev/blog/zed-decoded-extensions)
- [Rust reference: application binary interface](https://doc.rust-lang.org/reference/abi.html); [abi_stable](https://github.com/rodrimati1992/abi_stable_crates)
- [Rhai features](https://rhai.rs/book/about/features.html), [Rhai max operations](https://rhai.rs/book/safety/max-operations.html); [mlua](https://github.com/mlua-rs/mlua); [Boa](https://github.com/boa-dev/boa); [rquickjs](https://github.com/DelSkayn/rquickjs)
