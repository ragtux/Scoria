#!/usr/bin/env python3
"""Write crates/scoria/src/theme/defaults.rs from a theme-registry probe run.

The registry (crates/scoria/src/theme/registry.rs) names the tokens; this
script takes every custom property those tokens reference, transitively,
out of the probe's record of Obsidian's own declarations and writes them as
three tables: the declarations shared by both themes (`:root`, then `body`),
the dark theme's and the light theme's. Declarations under a platform or
state selector (`.mod-macos`, `body.is-frameless.in-progress`) are left out:
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

COMMON_SEL = {":root": 0, "html": 0, "body": 1}
DARK_SEL = {".theme-dark", "body.theme-dark", ".theme-dark.theme-dark"}
LIGHT_SEL = {".theme-light", "body.theme-light"}


def scope(selector):
    parts = [p.strip() for p in selector.split(",")]
    for p in parts:
        if p in COMMON_SEL:
            return "common", COMMON_SEL[p]
        if p in DARK_SEL:
            return "dark", 0
        if p in LIGHT_SEL:
            return "light", 0
    return None, 0


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

tables = {"common": [], "dark": [], "light": []}
for n in sorted(seen):
    per = {}
    for d in dark[n]["declared"]:
        s, rank = scope(d["selector"])
        if s is None:
            continue
        # The last declaration in a scope wins, as it does in the sheet.
        per[(s, rank)] = d["value"]
    for (s, rank), v in sorted(per.items(), key=lambda kv: kv[0][1]):
        tables[s].append((n, v, rank))


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def table(name, rows, doc):
    out = [f"/// {doc}", f"pub const {name}: &[(&str, &str)] = &["]
    for n, v, _ in rows:
        out.append(f'    ("{n}", "{esc(v)}"),')
    out.append("];")
    return "\n".join(out)


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
    "//! the probe read it off the running app, in the three scopes the registry",
    "//! layers: `COMMON` (`:root`, then `body`), then `DARK` or `LIGHT` over it.",
    "//! The chains are kept (`--text-muted` is `var(--color-base-70)`, not",
    "//! `#b3b3b3`) so an override of a base colour propagates as it does in",
    "//! Obsidian.",
    "",
    table("COMMON", tables["common"], "Declarations shared by both themes, `:root`'s before `body`'s."),
    "",
    table("DARK", tables["dark"], "The dark theme's (`.theme-dark`)."),
    "",
    table("LIGHT", tables["light"], "The light theme's (`.theme-light`)."),
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
print(f"{OUT.relative_to(ROOT)}: {len(names)} tokens, {len(seen)} properties; "
      f"common {len(tables['common'])}, dark {len(tables['dark'])}, light {len(tables['light'])}")
