# Scoria

Native Rust/GPUI clone of Obsidian's desktop app, targeting byte-level vault compatibility.

## Ground rules

- The vault folder is the only source of truth. Never write a file Scoria did not change; preserve
  line endings, BOMs, and unknown JSON/YAML keys (`#[serde(flatten)] extra` everywhere).
- One Markdown parser (`scoria-markdown`) feeds the editor, reading view, cache, and link rewriting.
  Do not add a second tokenizer for any of those.
- Formats and behaviors are specified in `docs/spec.md`. When the spec and Obsidian disagree, test
  against a real vault and fix the spec.
- Command ids, config keys, and file names must match Obsidian's exactly (`scoria-commands/data/commands.txt`,
  `scoria-formats/src/config.rs`). Add new ones only with the observed string.
- Clean room: implement from the spec and public docs. Do not paste code, CSS, or icons from
  Obsidian's bundle.

## Layout

```
crates/scoria           GPUI app (bin)
crates/scoria-vault     folder scanning, .obsidian config
crates/scoria-markdown  OFM primitives
crates/scoria-cache     CachedMetadata, LinkResolver
crates/scoria-formats   FileKind, config types, canvas, base
crates/scoria-commands  command registry, default hotkeys
crates/scoria-bench     spike benchmarks: synthetic vault generator, index timer (bin)
docs/                   spec and design notes
```

## Commands

On NixOS run these inside `nix-shell` (`shell.nix` supplies gpui's X11, Wayland, and Vulkan libraries).
Known issue: on NVIDIA GPUs gpui 0.2.2's blade 0.7.1 requests ray-tracing extensions and `vkCreateDevice`
fails with `ERROR_INITIALIZATION_FAILED`; until gpui is moved past that, run the app on the software
driver: `VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json cargo run -p scoria -- <vault>`.

```
cargo test --workspace            # all library tests (fast)
cargo check -p scoria             # app, pulls gpui (slow first time)
cargo run -p scoria -- <vault>    # open a vault
cargo run --release -p scoria-bench -- generate <dir>   # synthetic 20k-note vault
cargo run --release -p scoria-bench -- index <vault>    # time scan/read/index/resolve, check the 2 s gate
cargo fmt --all && cargo clippy --workspace --all-targets
```

## Conventions

- Vault-relative paths use `/` and keep original case; comparisons are case-insensitive.
- Positions are zero-based line/column plus byte offset, like Obsidian's `Pos`.
- Tests that need a vault build one in a `tempfile::tempdir()`; never depend on the developer's vault.
