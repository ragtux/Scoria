# Scoria task runner. `just` lists the recipes; `just <recipe>` runs one.
#
# On NixOS run the gpui recipes (check, build, run, ...) inside `nix-shell`,
# which supplies the X11, Wayland and Vulkan libraries: `nix-shell --run 'just run <vault>'`.
# On macOS a repo on a network share is mounted with the quarantine flag, and
# every build script cargo writes there triggers a Gatekeeper prompt, so the
# target directory defaults to a local cache unless CARGO_TARGET_DIR is set.

export CARGO_TARGET_DIR := env("CARGO_TARGET_DIR", if os() == "macos" { home_directory() / ".cache/cargo-target/scoria" } else { justfile_directory() / "target" })

docs_vault := justfile_directory() / "docs/scoria"

# List the recipes
default:
    @just --list --unsorted

# ---- Tests ---------------------------------------------------------------

# All tests in the workspace, the gpui app included
test:
    cargo test --workspace

# The library tests only: every crate but the gpui app (fast, no Metal or Vulkan)
test-lib:
    cargo test --workspace --exclude scoria

# The Writer gate: round-trip the test corpus and a real vault through the text model
roundtrip vault:
    SCORIA_ROUNDTRIP_VAULT="{{vault}}" cargo test -p scoria-text --test writer_roundtrip

# ---- Build ---------------------------------------------------------------

# Type-check the app crate (pulls gpui; slow the first time)
check:
    cargo check -p scoria

# Release build of every binary: scoria, scoria-mcp, scoria-bench
build:
    cargo build --release --workspace
    @echo "binaries in $CARGO_TARGET_DIR/release"

# Release build of the MCP server alone, for the entry in .mcp.json
mcp:
    cargo build --release -p scoria-mcp
    @echo "$CARGO_TARGET_DIR/release/scoria-mcp"

# Remove the build output
clean:
    cargo clean

# ---- Run -----------------------------------------------------------------

# Open a vault (release: the editor reparses on every edit). Extra args go to the app.
run vault *args:
    cargo run --release -p scoria -- "{{vault}}" {{args}}

# Open a vault with the desktop's own title bar instead of the hidden frame
run-native vault *args:
    cargo run --release -p scoria -- "{{vault}}" --frame native {{args}}

# Open a vault at a note, optionally at a heading: `just note <vault> "Name#Heading"`
note vault note *args:
    cargo run --release -p scoria -- "{{vault}}" --note "{{note}}" {{args}}

# Open the autocompletion popup in a note's source view for a screenshot, e.g. `just popup <vault> Welcome '[[al'`
popup vault note typed *args:
    cargo run --release -p scoria -- "{{vault}}" --note "{{note}}" --source --type "{{typed}}" {{args}}

# Dogfood: open the docs/scoria vault, the spec and design notes
docs *args:
    cargo run --release -p scoria -- "{{docs_vault}}" {{args}}

# Editor spike: self-driven edits over a synthetic 50k-note vault, prints timings
stress vault:
    cargo run --release -p scoria -- "{{vault}}" --synthetic 50000 --stress

# ---- Benchmarks ----------------------------------------------------------

# Generate a synthetic 20k-note vault into a directory
generate dir:
    cargo run --release -p scoria-bench -- generate "{{dir}}"

# Time scan, read, index and resolve over a vault and check the 2 s gate
index vault:
    cargo run --release -p scoria-bench -- index "{{vault}}"

# ---- Lint ----------------------------------------------------------------

# Format the workspace
fmt:
    cargo fmt --all

# Lint the workspace, tests included
clippy:
    cargo clippy --workspace --all-targets

# The CI lint job, step for step: formatting, the library tests, clippy without the app
ci:
    cargo fmt --all -- --check
    cargo test --workspace --exclude scoria
    cargo clippy --workspace --exclude scoria --all-targets

# ---- Site ----------------------------------------------------------------

# Build scoria.cc into site/dist/scoria (exits 1 on a dead link)
site:
    cd site && npm install && npm run build

# Build scoria.cc and serve it on http://localhost:4321/
site-serve:
    cd site && npm install && npm run serve

# Serve scoria.cc and rebuild on change
site-watch:
    cd site && npm install && npm run watch
