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
docs/                   spec and design notes
```

## Commands

```
cargo test --workspace            # all library tests (fast)
cargo check -p scoria             # app, pulls gpui (slow first time)
cargo run -p scoria -- <vault>    # open a vault
cargo fmt --all && cargo clippy --workspace --all-targets
```

## Conventions

- Vault-relative paths use `/` and keep original case; comparisons are case-insensitive.
- Positions are zero-based line/column plus byte offset, like Obsidian's `Pos`.
- Tests that need a vault build one in a `tempfile::tempdir()`; never depend on the developer's vault.
