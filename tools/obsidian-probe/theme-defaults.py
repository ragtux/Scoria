#!/usr/bin/env python3
"""Write crates/scoria/src/theme/defaults.rs from a theme-registry probe run.

The registry (crates/scoria/src/theme/registry.rs) names the tokens; this
script takes every custom property those tokens reference, transitively,
out of the probe's record of Obsidian's own declarations and writes them
with the selector each was declared under, for `theme::css::Cascade` to rank
as CSS does. Declarations under a platform or state selector (`.mod-macos`,
`body.is-frameless.in-progress`) are left out:
Linux is the reference the rest of the probes were run on, and a state the
app never enters has no value to carry.

It also writes, under `#[cfg(test)]`, the value Obsidian computed for every
token in each theme, which `theme::tests` holds the resolver to.

    tools/obsidian-probe/theme-defaults.py \\
        tools/obsidian-probe/results/2026-11-02-theme-registry.json

A token the record has no declaration for is an error: either the name is
wrong or the probe needs re-running on the Obsidian that introduced it.
"""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REGISTRY = ROOT / "crates/scoria/src/theme/registry.rs"
OUT = ROOT / "crates/scoria/src/theme/defaults.rs"

results = Path(sys.argv[1] if len(sys.argv) > 1 else HERE / "results/2026-11-02-theme-registry.json")
run = json.load(open(results))["themeRegistry"]["run"]
dark = run["steps"]["dark-01-vars"]["vars"]
light = run["steps"]["light-01-vars"]["vars"]
title = run.get("title", "")

names = re.findall(r'=\s*"(--[\w-]+)"\s*;', REGISTRY.read_text())
missing = [n for n in names if n not in dark]
if missing:
    print("tokens the probe record has no declaration for:", *missing, sep="\n  ", file=sys.stderr)
    sys.exit(1)

READ = {":root", "html", "body", ".theme-dark", ".theme-light", "body.theme-dark", "body.theme-light", ".theme-dark.theme-dark"}


def readable(selector):
    """The parts of a selector list the loader's scanner reads, joined."""
    parts = [p.strip() for p in selector.split(",") if p.strip() in READ]
    return ", ".join(parts)


# The closure: every name reachable through var() from a registry token.
seen = []
todo = list(names)
while todo:
    n = todo.pop(0)
    if n in seen:
        continue
    seen.append(n)
    for d in dark[n]["declared"]:
        for m in re.findall(r"var\((--[\w-]+)", d["value"]):
            if m not in seen and m in dark:
                todo.append(m)

# One row per declaration the scanner would read, in the sheet's order per
# name: the cascade in `theme::css` ranks them as CSS does.
rows = []
for n in sorted(seen):
    for d in dark[n]["declared"]:
        sel = readable(d["selector"])
        if sel:
            rows.append((sel, n, d["value"]))


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def observed(vars_):
    rows = []
    for n in names:
        c = vars_.get(n, {}).get("computed", "")
        rows.append((n, c))
    return rows


lines = [
    "//! The default theme's declarations, written by",
    f"//! `tools/obsidian-probe/theme-defaults.py` from `{results.relative_to(ROOT) if results.is_relative_to(ROOT) else results.name}`",
    f"//! ({title or 'Obsidian'}). Do not edit: re-run the probe and the script.",
    "//!",
    "//! Every entry is a custom property of Obsidian's default stylesheet as",
    "//! the probe read it off the running app, with the selector it was declared",
    "//! under, which `theme::css::Cascade` ranks as CSS does on the body.",
    "//! The chains are kept (`--text-muted` is `var(--color-base-70)`, not",
    "//! `#b3b3b3`) so an override of a base colour propagates as it does in",
    "//! Obsidian.",
    "",
    "/// Every declaration of the closure the scanner reads, as",
    "/// `(selector, property, value)`, in the sheet's order per property.",
    "pub const DEFAULTS: &[(&str, &str, &str)] = &[",
    *[f'    ("{esc(sel)}", "{n}", "{esc(v)}"),' for sel, n, v in rows],
    "];",
    "",
    "/// What Obsidian computed for each registry token on `body`, dark theme:",
    "/// the resolver is held to these in `theme::tests`.",
    "#[cfg(test)]",
    "pub const OBSERVED_DARK: &[(&str, &str)] = &[",
    *[f'    ("{n}", "{esc(c)}"),' for n, c in observed(dark)],
    "];",
    "",
    "/// The same for the light theme.",
    "#[cfg(test)]",
    "pub const OBSERVED_LIGHT: &[(&str, &str)] = &[",
    *[f'    ("{n}", "{esc(c)}"),' for n, c in observed(light)],
    "];",
    "",
]
OUT.write_text("\n".join(lines))
print(f"{OUT.relative_to(ROOT)}: {len(names)} tokens, {len(seen)} properties, {len(rows)} declarations")
