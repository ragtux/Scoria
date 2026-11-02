#!/usr/bin/env python3
"""Cut every mark out of Anya's draft sheet into its own flat SVG.

`docs/img/marks/Scoria_draft3.pdf` is the sheet Anya sent on 2026-10-29: the scoria clast in
three colourways and four single-colour cuts, a rounder pebble in the same three colourways
and four single-colour cuts, two shaded studies, and the wordmark. This writes one SVG per
mark into `docs/img/marks/`, and six lockups that set the wordmark beside a mark the way the
sheet does. Run it when the sheet changes:

    just marks                         # or, without just:
    tools/marks/cut.py [sheet.pdf]

Inkscape imports the PDF (its own importer, not poppler's, which keeps the fills as `#rrggbb`
rather than ICC profiles). Every path then arrives with a `matrix(4/3, 0, 0, -4/3, tx, ty)` of
its own - the PDF's points to pixels, y flipped - and that transform is baked into the path data
here so each cut is a plain list of filled paths with no transform, no style block and no
stroke, like the master it replaces. Commands are the four the sheet uses (move, line, cubic,
close); an arc would be an error, and none occurs.

What is fixed on the way through, and nothing else:

  * one facet of the flat red clast is `#9e1c20` where its thirteen neighbours are `#a01d21`,
    a slipped digit, and is written as `#a01d21`;
  * the flat red clast's small dark speck (the fourth `#221f1f` path the other clasts carry,
    at the right-hand edge) had been dragged up into the wordmark above it on the sheet, and is
    left out rather than put back by guesswork.

The marks are identified by Inkscape's path ids, which are the PDF's drawing order; the ranges
below were read off the import and hold for this sheet only.
"""

import os
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SHEET = ROOT / "docs/img/marks/Scoria_draft3.pdf"
OUT = ROOT / "docs/img/marks"

NUMBER = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"


def rng(a, b, *, skip=()):
    """`pathA`..`pathB` inclusive, minus `skip`."""
    return [f"path{n}" for n in range(a, b + 1) if f"path{n}" not in skip]


# name -> the paths that make it, in drawing order.
MARKS = [
    ("clast-orange", rng(124, 148)),
    ("clast-red", rng(99, 123)),
    ("clast-brown", rng(249, 272)),
    ("clast-flat-orange", rng(224, 248)),
    ("clast-flat-red", rng(199, 223, skip=("path221",))),
    ("clast-flat-brown", rng(149, 173)),
    ("clast-flat-rust", rng(174, 198)),
    ("clast-orange-shaded", rng(61, 96)),
    ("clast-red-shaded", rng(4, 60, skip=("path28",))),
    ("pebble-orange", rng(300, 324)),
    ("pebble-red", rng(325, 349)),
    ("pebble-brown", rng(274, 299, skip=("path298",))),
    ("pebble-flat-orange", rng(376, 400)),
    ("pebble-flat-red", rng(351, 375)),
    ("pebble-flat-brown", rng(426, 450)),
    ("pebble-flat-sienna", rng(401, 425)),
    ("wordmark", ["path3"]),
]
# Measured off the sheet: where the wordmark's box sits relative to the mark's. The clast
# lockups put the word's left edge 2.4 px past the clast's and its top 139.6 px below the
# clast's top; the pebble lockups leave a 52.8 px gap and drop the word 121.2 px.
LOCKUPS = [
    ("lockup-clast-orange", "clast-orange", (2.4, 139.6)),
    ("lockup-clast-red", "clast-red", (2.4, 139.6)),
    ("lockup-clast-brown", "clast-brown", (2.4, 139.6)),
    ("lockup-pebble-orange", "pebble-orange", (52.8, 121.2)),
    ("lockup-pebble-red", "pebble-red", (52.8, 121.2)),
    ("lockup-pebble-brown", "pebble-brown", (52.8, 121.2)),
]
FIX = {"#9e1c20": "#a01d21"}


def matrix(text):
    m = re.search(r"matrix\(([^)]*)\)", text or "")
    if m is None:
        return (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    return tuple(float(v) for v in re.split(r"[ ,]+", m.group(1).strip()))


def compose(outer, inner):
    """`outer` applied after `inner`."""
    a1, b1, c1, d1, e1, f1 = outer
    a2, b2, c2, d2, e2, f2 = inner
    return (
        a1 * a2 + c1 * b2, b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2, b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1, b1 * e2 + d1 * f2 + f1,
    )


def apply(t, x, y):
    a, b, c, d, e, f = t
    return (a * x + c * y + e, b * x + d * y + f)


def import_sheet(pdf):
    """Inkscape's import of the sheet, parsed: paths and gradients by id."""
    with tempfile.TemporaryDirectory() as tmp:
        svg = Path(tmp) / "sheet.svg"
        subprocess.run(
            ["inkscape", "--export-type=svg", f"--export-filename={svg}", str(pdf)],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        root = ET.parse(svg).getroot()

    paths, gradients = {}, {}

    def walk(el, t):
        t = compose(t, matrix(el.get("transform")))
        tag = el.tag.rsplit("}", 1)[-1]
        if tag == "path":
            fill = re.search(r"fill:(#[0-9a-fA-F]{6}|url\(#[^)]+\))", el.get("style", ""))
            paths[el.get("id")] = (fill.group(1) if fill else None, el.get("d"), t)
        elif tag == "linearGradient":
            stops = []
            for stop in el:
                colour = re.search(r"stop-color:(#[0-9a-fA-F]{6})", stop.get("style", ""))
                stops.append((float(stop.get("offset")), colour.group(1)))
            gradients[el.get("id")] = {
                "x1": float(el.get("x1")), "y1": float(el.get("y1")),
                "x2": float(el.get("x2")), "y2": float(el.get("y2")),
                "transform": matrix(el.get("gradientTransform")), "stops": stops,
            }
        for child in el:
            walk(child, t)

    walk(root, (1.0, 0.0, 0.0, 1.0, 0.0, 0.0))
    return paths, gradients


def absolute(d, t):
    """Path data as absolute M/L/C/Z segments in page pixels."""
    tokens = re.findall(r"[MLHVCSQTAZmlhvcsqtaz]|" + NUMBER, d)
    segments, cur, start, cmd, i = [], (0.0, 0.0), (0.0, 0.0), None, 0
    while i < len(tokens):
        tok = tokens[i]
        if tok.isalpha():
            cmd, i = tok, i + 1
            if cmd in "Zz":
                segments.append(("Z", []))
                cur = start
            continue
        if cmd.upper() not in "MLHVC":
            sys.exit(f"path command {cmd!r} is not one the sheet uses")
        need = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6}[cmd.upper()]
        values = [float(v) for v in tokens[i:i + need]]
        i += need
        relative = cmd.islower()
        if cmd.upper() == "H":
            cur = ((cur[0] if relative else 0.0) + values[0], cur[1])
            segments.append(("L", [cur]))
        elif cmd.upper() == "V":
            cur = (cur[0], (cur[1] if relative else 0.0) + values[0])
            segments.append(("L", [cur]))
        else:
            ox, oy = cur if relative else (0.0, 0.0)
            points = [(ox + values[k], oy + values[k + 1]) for k in range(0, need, 2)]
            segments.append((cmd.upper(), points))
            cur = points[-1]
            if cmd.upper() == "M":
                start = cur
                cmd = "l" if relative else "L"
    return [(c, [apply(t, *p) for p in pts]) for c, pts in segments]


def bounds(segments):
    xs, ys, cur = [], [], None
    for cmd, points in segments:
        if cmd == "C":
            p0, (p1, p2, p3) = cur, points
            for k in range(21):
                s = k / 20
                r = 1 - s
                xs.append(r * r * r * p0[0] + 3 * r * r * s * p1[0] + 3 * r * s * s * p2[0] + s * s * s * p3[0])
                ys.append(r * r * r * p0[1] + 3 * r * r * s * p1[1] + 3 * r * s * s * p2[1] + s * s * s * p3[1])
        for p in points:
            xs.append(p[0])
            ys.append(p[1])
        if points:
            cur = points[-1]
    return min(xs), min(ys), max(xs), max(ys)


def num(v):
    s = f"{v:.4f}".rstrip("0").rstrip(".")
    return "0" if s in ("", "-0") else s


def path_data(segments, dx, dy):
    """An absolute move per subpath, then relative lines and curves, each rounded before the
    difference is taken so nothing drifts along a long path."""
    out, cur = [], None

    def at(p):
        return (round(p[0] + dx, 4), round(p[1] + dy, 4))

    for cmd, points in segments:
        if cmd == "Z":
            out.append("z")
        elif cmd == "M":
            cur = at(points[0])
            out.append(f"M{num(cur[0])} {num(cur[1])}")
        elif cmd == "L":
            p = at(points[0])
            ddx, ddy = round(p[0] - cur[0], 4), round(p[1] - cur[1], 4)
            out.append(f"h{num(ddx)}" if ddy == 0 else f"v{num(ddy)}" if ddx == 0 else f"l{num(ddx)} {num(ddy)}")
            cur = p
        else:
            ps = [at(p) for p in points]
            out.append("c" + " ".join(f"{num(round(p[0] - cur[0], 4))} {num(round(p[1] - cur[1], 4))}" for p in ps))
            cur = ps[-1]
    return "".join(out)


def document(name, pieces, gradients):
    """One SVG: `pieces` are (fill, segments, transform) in drawing order, in page pixels."""
    boxes = [bounds(segments) for _, segments, _ in pieces]
    x0, y0 = min(b[0] for b in boxes), min(b[1] for b in boxes)
    w = round(max(b[2] for b in boxes) - x0, 4)
    h = round(max(b[3] for b in boxes) - y0, 4)
    defs, body = [], []
    for fill, segments, t in pieces:
        d = path_data(segments, -x0, -y0)
        if fill.startswith("url("):
            gid = fill[5:-1]
            g = gradients[gid]
            gt = compose((1.0, 0.0, 0.0, 1.0, -x0, -y0), compose(t, g["transform"]))
            stops = "".join(f'\n      <stop offset="{num(o)}" stop-color="{c}"/>' for o, c in g["stops"])
            defs.append(
                f'    <linearGradient id="{gid}" gradientUnits="userSpaceOnUse"'
                f' x1="{num(g["x1"])}" y1="{num(g["y1"])}" x2="{num(g["x2"])}" y2="{num(g["y2"])}"'
                f' gradientTransform="matrix({" ".join(num(round(v, 6)) for v in gt)})">{stops}\n    </linearGradient>'
            )
            body.append(f'  <path fill="url(#{gid})" d="{d}"/>')
        else:
            body.append(f'  <path fill="{FIX.get(fill, fill)}" d="{d}"/>')
    lines = [
        '<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Scoria"',
        f'     width="{num(w)}" height="{num(h)}" viewBox="0 0 {num(w)} {num(h)}">',
        "  <title>Scoria</title>",
        f"  <!-- {name}: cut from Scoria_draft3.pdf by tools/marks/cut.py. Do not edit. -->",
    ]
    if defs:
        lines.append("  <defs>\n" + "\n".join(defs) + "\n  </defs>")
    lines += body
    lines.append("</svg>")
    return "\n".join(lines) + "\n", (x0, y0, w, h)


def main():
    sheet = Path(sys.argv[1]) if len(sys.argv) > 1 else SHEET
    if not sheet.exists():
        sys.exit(f"missing {sheet}")
    paths, gradients = import_sheet(sheet)
    pieces, boxes = {}, {}
    for name, ids in MARKS:
        # The ids a range skips over are the gradient stops Inkscape numbered in sequence;
        # the range's two ends must be paths, or the sheet is not the one the ids were read off.
        for end in (ids[0], ids[-1]):
            if end not in paths:
                sys.exit(f"{name}: {sheet.name} has no {end}; the sheet changed, re-read the ids")
        ids = [i for i in ids if i in paths]
        pieces[name] = [(paths[i][0], absolute(paths[i][1], paths[i][2]), paths[i][2]) for i in ids]
        text, box = document(name, pieces[name], gradients)
        (OUT / f"{name}.svg").write_text(text)
        boxes[name] = box
        fills = sorted({FIX.get(f, f) for f, _, _ in pieces[name] if not f.startswith("url(")})
        print(f"{name:22s} {box[2]:8.2f} x {box[3]:7.2f}  {len(ids):2d} paths  {' '.join(fills)}")

    word = pieces["wordmark"]
    wx, wy = boxes["wordmark"][0], boxes["wordmark"][1]
    for name, mark, (gap, drop) in LOCKUPS:
        mx, my = boxes[mark][0], boxes[mark][1]
        # Move the word so its box lands at (mark right + gap, mark top + drop).
        dx = (mx + boxes[mark][2] + gap) - wx
        dy = (my + drop) - wy
        shifted = [(f, [(c, [(x + dx, y + dy) for x, y in pts]) for c, pts in segs], t) for f, segs, t in word]
        text, box = document(name, pieces[mark] + shifted, gradients)
        (OUT / f"{name}.svg").write_text(text)
        print(f"{name:22s} {box[2]:8.2f} x {box[3]:7.2f}")


if __name__ == "__main__":
    main()
