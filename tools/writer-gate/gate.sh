#!/usr/bin/env bash
#
# The Writer gate, phase 3: "the vault round-trips byte-identical after a week
# of mixed use in both apps" (docs/scoria/Development.md, [[#Writer (phase 3)]]).
#
# The harness half -- every path from a keystroke to a file's bytes -- is
# `just roundtrip`. This script is the other half: the baseline snapshots and
# the diffs that make a week of real use measurable rather than remembered.
# A snapshot is a copy, not a list of hashes, because a hash says a file moved
# and the next question is always *which bytes*.
#
# Two vaults, for the reason recorded in [[Writer gate run]]:
#
#   PLMCRZY    the deciding vault. Only Scoria and Obsidian ever write to it,
#              so a moved byte has exactly two suspects. Snapshot and diff.
#   docs/scoria the vault that is actually lived in, watched through git --
#              free evidence, but repo tooling writes there too, so a moved
#              byte has three suspects and the verdict never rests on it.
#
# `workspace.json` is excluded throughout: Scoria neither models nor writes it
# and Obsidian rewrites it constantly.
#
#   gate.sh baseline [date]        take today's snapshot of both vaults
#   gate.sh check [snapshot]       diff both vaults (default: newest snapshot)
#   gate.sh check --from-first     diff against the first snapshot of the run
#   gate.sh status                 what snapshots exist, and where the run is
#
set -euo pipefail

GATE_DIR="${SCORIA_GATE_DIR:-$HOME/bstore2/raga/scoria-gate}"
VAULT="${SCORIA_GATE_VAULT:-$HOME/bstore2/raga/plmcrzy/PLMCRZY}"
REPO="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
DOCS="$REPO/docs/scoria"
EXCLUDE=workspace.json
NAME="$(basename "$VAULT")"
SLUG="$(printf %s "$NAME" | tr "[:upper:] " "[:lower:]-")"

die() { printf '%s\n' "$*" >&2; exit 1; }
rule() { printf '\n== %s ==\n' "$*"; }

# sha256 of every file but the excluded one, paths relative to the vault root,
# in the same shape as the manifests already in the gate directory.
manifest() {
    ( cd "$1" && find . -type f -not -name "$EXCLUDE" | sort | xargs -d '\n' sha256sum )
}

snapshots() { find "$GATE_DIR" -maxdepth 1 -type d -name 'snapshot-*' | sort; }
newest()    { snapshots | tail -1; }
first()     { snapshots | head -1; }

cmd_baseline() {
    local date="${1:-$(date +%F)}"
    local dest="$GATE_DIR/snapshot-$date"
    [ -d "$VAULT" ] || die "no vault at $VAULT"
    mkdir -p "$GATE_DIR"
    [ -e "$dest" ] && die "$dest exists already; pass another date or remove it"

    rsync -a --exclude "$EXCLUDE" "$VAULT"/ "$dest"/
    manifest "$VAULT" > "$GATE_DIR/$SLUG-$date.sha256"
    printf 'snapshot  %s  (%s files)\n' "$dest" "$(find "$dest" -type f | wc -l)"
    printf 'manifest  %s\n' "$GATE_DIR/$SLUG-$date.sha256"

    # docs/scoria needs no copy: git holds every byte. Record the commit the
    # day started on, so a later diff has something to name.
    local head
    head="$(git -C "$REPO" rev-parse --short HEAD)"
    printf '%s  %s  docs/scoria at %s\n' "$date" "$(date +%T)" "$head" \
        >> "$GATE_DIR/docs-scoria-heads.txt"
    printf 'docs vault  at %s' "$head"
    if [ -n "$(git -C "$REPO" status --porcelain -- "$DOCS")" ]; then
        printf ', uncommitted changes present:\n'
        git -C "$REPO" status --porcelain -- "$DOCS"
        printf 'commit or stash them, or the week cannot tell an app write from a tool write\n'
    else
        printf ', working tree clean\n'
    fi
}

cmd_check() {
    local base
    case "${1:-}" in
        --from-first) base="$(first)" ;;
        '')           base="$(newest)" ;;
        *)            base="$1" ;;
    esac
    [ -n "$base" ] && [ -d "$base" ] || die "no snapshot to compare against; run: gate.sh baseline"

    rule "$NAME against $(basename "$base")"
    local moved=0
    diff -r -q --exclude "$EXCLUDE" "$base" "$VAULT" || moved=1
    if [ "$moved" = 0 ]; then
        echo "no file moved"
    else
        rule "the bytes that moved"
        diff -r --exclude "$EXCLUDE" "$base" "$VAULT" || true
        cat <<'NOTE'

Each file above is the gate's evidence, and is not a failure by itself. For
every one, answer in [[Writer gate run]]:
  - which app moved it, and
  - whether the move was an edit someone made, or an app rewriting bytes
    nobody touched.
Only the second kind fails the gate.
NOTE
    fi

    rule "docs/scoria (git)"
    if [ -n "$(git -C "$REPO" status --porcelain -- "$DOCS")" ]; then
        git -C "$REPO" status --porcelain -- "$DOCS"
        git -C "$REPO" diff --stat -- "$DOCS" || true
    else
        echo "working tree clean"
    fi

    rule "next"
    echo "attribute anything above, then: just gate-baseline"
}

cmd_status() {
    rule "snapshots in $GATE_DIR"
    snapshots | while read -r s; do
        printf '%s  %s files\n' "$(basename "$s")" "$(find "$s" -type f | wc -l)"
    done
    rule "the run"
    local f n
    f="$(first)"; n="$(newest)"
    [ -n "$f" ] && printf 'first   %s\n' "$(basename "$f")"
    [ -n "$n" ] && printf 'newest  %s\n' "$(basename "$n")"
    rule "is the vault registered in Obsidian?"
    if grep -qF "$VAULT" "$HOME/.config/obsidian/obsidian.json" 2>/dev/null; then
        echo "yes -- the mixed-use half can run"
    else
        echo "NO -- open it once in Obsidian with 'Open folder as vault' first"
    fi
}

case "${1:-}" in
    baseline) shift; cmd_baseline "$@" ;;
    check)    shift; cmd_check "$@" ;;
    status)   shift; cmd_status "$@" ;;
    *)        awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' \
                  "${BASH_SOURCE[0]}"; exit 1 ;;
esac
