#!/usr/bin/env bash
# The plugin runtime spike (docs/scoria/Plugin SDK.md, "Spike"). Run inside the
# spike's shell, from anywhere:
#
#   nix-shell tools/plugin-spike/shell.nix --run 'tools/plugin-spike/run.sh all'
#
# Steps: toolchain | guests | builds | bench | all. Everything lands in
# tools/plugin-spike/target (cargo, and a rustup home of its own, so nothing
# outside the folder changes) and tools/plugin-spike/out (the guests, the
# precompiled components, results.txt).
#
# The toolchain is rustup's, not the system rustc: wasmtime 49 needs rustc
# 1.96 and wasmtime 50 needs 1.97 (scoria builds with 1.95), and extism 1.41
# depends on wasmtime 49, so both routes are built on one wasmtime major.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
T="$here/target"
OUT="$here/out"
TOOLCHAIN="${SPIKE_TOOLCHAIN:-1.100.0}"
export RUSTUP_HOME="$T/rustup"
mkdir -p "$T" "$OUT"

cargo() { rustup run "$TOOLCHAIN" cargo "$@"; }

# name | crate dir | cargo feature flags
VARIANTS=(
  "baseline|host-baseline|"
  "wit-default|host-wit|--features wasmtime-default"
  "wit-cranelift|host-wit|--features cranelift"
  "wit-cranelift-parallel|host-wit|--features cranelift-parallel"
  "wit-winch|host-wit|--features winch"
  "wit-pulley|host-wit|--features pulley"
  "wit-runtime|host-wit|--features runtime"
  "wit-runtime-pulley|host-wit|--features runtime-pulley"
  "extism-default|host-extism|"
  "extism-minimal|host-extism|--no-default-features"
)

toolchain() {
  rustup toolchain install "$TOOLCHAIN" --profile minimal \
    -t wasm32-unknown-unknown -t wasm32-wasip2 --no-self-update
  rustup run "$TOOLCHAIN" rustc --version
}

guests() {
  local tgt="$T/guests"
  (cd "$here/guest-wit" && CARGO_TARGET_DIR="$tgt" cargo build --release --target wasm32-unknown-unknown)
  (cd "$here/guest-extism" && CARGO_TARGET_DIR="$tgt" cargo build --release --target wasm32-unknown-unknown)
  local rel="$tgt/wasm32-unknown-unknown/release"
  wasm-tools component new "$rel/spike_guest_wit.wasm" -o "$OUT/guest-wit.component.wasm"
  cp -f "$rel/spike_guest_extism.wasm" "$OUT/guest-extism.wasm"
  ls -l "$OUT"/*.wasm
}

secs() { python3 -c "import sys;print(f'{float(sys.argv[2])-float(sys.argv[1]):.1f}')" "$1" "$2"; }

# Clean build (its own fresh target dir), then an incremental one after
# touching main.rs, at full parallelism and at -j 8. Appends to builds.txt;
# `run.sh builds <name>` builds one variant.
builds() {
    for d in host-baseline host-wit host-extism; do (cd "$here/$d" && cargo fetch -q); done
  for v in "${VARIANTS[@]}"; do
    IFS='|' read -r name dir flags <<<"$v"
    [[ -n "$ONLY" && "$name" != "$ONLY" ]] && continue
    for jobs in "$(nproc)" 8; do
      local tgt="$T/hosts-j$jobs/$name"
      rm -rf "$tgt"
      local t0 t1 t2
      t0=$(date +%s.%N)
      # shellcheck disable=SC2086
      (cd "$here/$dir" && CARGO_TARGET_DIR="$tgt" cargo build --release -j "$jobs" $flags -q)
      t1=$(date +%s.%N)
      touch "$here/$dir/src/main.rs"
      # shellcheck disable=SC2086
      (cd "$here/$dir" && CARGO_TARGET_DIR="$tgt" cargo build --release -j "$jobs" $flags -q)
      t2=$(date +%s.%N)
      local bin size
      bin="$tgt/release/spike-$dir"
      size=$(stat -c %s "$bin")
      local line="$name jobs=$jobs size=$size clean_s=$(secs "$t0" "$t1") incr_s=$(secs "$t1" "$t2")"
      echo "$line" | tee -a "$OUT/builds.txt"
    done
  done
}

bin() { echo "$T/hosts-j$(nproc)/$1/release/spike-$2"; }

bench() {
  : >"$OUT/bench.txt"
  # Bring every host up to date with its sources (a no-op after `builds`), and
  # record the sizes the bench ran with.
  for v in "${VARIANTS[@]}"; do
    IFS='|' read -r name dir flags <<<"$v"
    # shellcheck disable=SC2086
    (cd "$here/$dir" && CARGO_TARGET_DIR="$T/hosts-j$(nproc)/$name" cargo build --release $flags -q)
    echo "size $name $(stat -c %s "$(bin "$name" "$dir")")" | tee -a "$OUT/bench.txt"
  done
  "$(bin wit-cranelift host-wit)" "$OUT/guest-wit.component.wasm" --precompile "$OUT/guest-wit.cwasm"
  "$(bin wit-pulley host-wit)" "$OUT/guest-wit.component.wasm" --precompile "$OUT/guest-wit.pulley.cwasm"
  ls -l "$OUT"/*.cwasm | tee -a "$OUT/bench.txt"
  for v in "${VARIANTS[@]}"; do
    IFS='|' read -r name dir _ <<<"$v"
    local guest
    case "$name" in
      baseline) guest="" ;;
      wit-runtime) guest="$OUT/guest-wit.cwasm" ;;
      wit-runtime-pulley) guest="$OUT/guest-wit.pulley.cwasm" ;;
      wit-*) guest="$OUT/guest-wit.component.wasm" ;;
      extism-*) guest="$OUT/guest-extism.wasm" ;;
    esac
    echo "== $name" | tee -a "$OUT/bench.txt"
    "$(bin "$name" "$dir")" $guest | tee -a "$OUT/bench.txt"
    case "$name" in
      extism-minimal)
        echo "== $name cache=system (second run hits it)" | tee -a "$OUT/bench.txt"
        SPIKE_EXTISM_CACHE=1 "$(bin "$name" "$dir")" $guest >/dev/null
        SPIKE_EXTISM_CACHE=1 "$(bin "$name" "$dir")" $guest | grep -E 'cache|compile|instantiate_first' | tee -a "$OUT/bench.txt" ;;
      wit-cranelift|wit-winch|wit-pulley)
        for budget in epoch fuel; do
          echo "== $name budget=$budget" | tee -a "$OUT/bench.txt"
          SPIKE_BUDGET=$budget "$(bin "$name" "$dir")" $guest | grep -E 'budget|check_' | tee -a "$OUT/bench.txt"
        done ;;
    esac
  done
}

ONLY="${2:-}"
case "${1:-all}" in
  toolchain) toolchain ;;
  guests) guests ;;
  builds) builds ;;
  bench) bench ;;
  all) toolchain && guests && rm -f "$OUT/builds.txt" && builds && bench ;;
  *) echo "usage: run.sh [toolchain|guests|builds|bench|all]" >&2; exit 2 ;;
esac
