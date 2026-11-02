#!/usr/bin/env bash
# Drive the reference loop: build rocks, measure marks, score them.
#
# Three external tools, none of them a dependency of the generator itself:
# blender renders the reference and decodes images, inkscape rasterises an
# SVG, python3 scores. The crate builds and runs with none of them installed.
#
#   reference/run.sh rocks [N]        re-render the reference set and re-aggregate
#   reference/run.sh check FILE.svg.. rasterise, measure and score marks
#
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$here")"
work="${SCORIA_REF_WORK:-$(mktemp -d)}"

have() { command -v "$1" >/dev/null 2>&1 || { echo "reference/run.sh: need $1 on PATH" >&2; exit 2; }; }

cmd="${1:-}"; shift || true

case "$cmd" in
rocks)
  have blender
  n="${1:-6}"
  echo "[rocks] rendering $n reference rocks into $work"
  for s in $(seq 1 "$n"); do
    blender --background --python "$here/model.py" -- \
      --seed "$s" --out "$work/rock$s" --res 1000 --samples 32 >/dev/null 2>&1 &
  done
  wait
  blender --background --python "$here/measure.py" -- \
    "$work"/rock*/colour.png --json "$work/rocks.json" >/dev/null 2>&1
  python3 "$here/compare.py" aggregate "$work/rocks.json" -o "$here/targets.json"
  ;;

check)
  have blender; have inkscape
  [ $# -gt 0 ] || { echo "reference/run.sh check: give at least one SVG" >&2; exit 2; }
  # Fresh every run: with SCORIA_REF_WORK pointing at a reused directory the
  # scorer measured every stale PNG left from earlier binaries alongside the
  # marks asked for, and reported their failures as this run's.
  rm -rf "$work/marks"
  mkdir -p "$work/marks"
  for f in "$@"; do
    # -y 0 is load bearing: without it inkscape writes an OPAQUE background
    # and every measurement below describes the image rectangle instead of
    # the mark. `-b none` does not do it -- an unparseable colour leaves the
    # background on.
    inkscape "$f" -w 1000 -y 0 -o "$work/marks/$(basename "${f%.svg}").png" 2>/dev/null
  done
  blender --background --python "$here/measure.py" -- \
    "$work"/marks/*.png --json "$work/marks.json" >/dev/null 2>&1
  strict=()
  # Quoting "${STRICT:+--strict}" would pass an EMPTY argument when STRICT is
  # unset, which argparse rejects. An array expands to nothing instead.
  [ -n "${STRICT:-}" ] && strict=(--strict)
  python3 "$here/compare.py" check "$work/marks.json" -t "$here/targets.json" "${strict[@]}"
  ;;

*)
  sed -n '2,12p' "$0" >&2
  exit 2
  ;;
esac
