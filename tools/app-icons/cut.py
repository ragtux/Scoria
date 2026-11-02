#!/usr/bin/env python3
"""Cut the executables' icons and the site's favicon from the logo master.

`docs/img/marks/clast-orange.svg` is the master every other copy of the logo comes from, and
this is the cut each desktop reads: `crates/scoria/resources/windows/scoria.ico`, which
`build.rs` embeds as resource 1 of `scoria.exe`; `crates/scoria/resources/macos/scoria.icns`,
which `tools/macos-app/bundle.sh` puts in `Scoria.app`; and the hicolor theme under
`crates/scoria/resources/linux/icons`, which `tools/linux-desktop/install.sh` copies into the
XDG data directory beside `scoria.desktop`. It also writes `site/sites/scoria/assets/favicon.svg`,
which is the same tile the small icons are. Run it after changing the master, the way the two
SVG copies are re-cut:

    nix-shell -p imagemagick --run tools/app-icons/cut.py

Two sources, because one drawing cannot cover 16 px and 1024 px:

  * at 48 px and up the master itself, centred on a rounded square of paper, so the mark keeps
    its outline, its facets and all seven vesicles. The plate is paper rather than the chrome's
    near-black because the mark's own outline is #221f1f: on a dark tile the clast loses its
    edge and reads as a torn blob;
  * at 32 px and below `docs/img/marks/clast-flat-orange.svg`, Anya's own flat cut of the same
    clast - one orange, the outline and the vesicles, no facets - for exactly this reason: the
    full drawing turns to mud by 16 px. The favicon is this cut on the same plate.

The macOS entries are additionally inset to Apple's 824/1024 content box, which is what keeps a
Dock icon the same visual size as its neighbours; the Windows and hicolor entries are full-bleed,
which is what Explorer and the GNOME and KDE shells draw. Linux also gets the scalable SVG, which
is the plate with the master nested inside it rather than anything redrawn.
"""

import io
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MASTER = ROOT / "docs/img/marks/clast-orange.svg"
FLAT = ROOT / "docs/img/marks/clast-flat-orange.svg"
ICO = ROOT / "crates/scoria/resources/windows/scoria.ico"
ICNS = ROOT / "crates/scoria/resources/macos/scoria.icns"
HICOLOR = ROOT / "crates/scoria/resources/linux/icons/hicolor"
FAVICON = ROOT / "site/sites/scoria/assets/favicon.svg"

# The plate behind the mark: paper, with a 7/32 corner radius, on a 1024 grid.
PLATE_COLOUR = "#faf9f7"
PLATE_RADIUS = 224
PLATE = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024">
  <rect width="1024" height="1024" rx="{PLATE_RADIUS}" fill="{PLATE_COLOUR}"/>
</svg>""".encode()
# Of 1024, measured across the mark, which is landscape: 880 leaves 72 px either side and the
# tall margins the 7:5 clast makes for itself. Wider than this and the outline touches the
# plate's corner radius at 16 px.
MARK_WIDTH = 880
FLAT_BELOW = 48  # under this many pixels the flat cut is drawn instead
APPLE_BOX = 824 / 1024  # macOS content box; the rest of the canvas is transparent margin

# What Windows asks the shell for: the four list and tile sizes, then 256 for the preview pane.
ICO_SIZES = [16, 20, 24, 32, 48, 64, 128, 256]
# The modern icns set: `icp4`/`icp5` and `ic07`-`ic10` are the 1x icons, `ic11`-`ic14` the
# retina ones. 256 and 512 appear twice because 256 @1x and 128 @2x are different entries of
# the same pixel size, and the Finder reads both.
ICNS_ENTRIES = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
]
# The hicolor sizes a Linux desktop looks for, biggest last. 512 is what GNOME's shell and the
# KDE task switcher reach for; the scalable SVG sits beside them for anything larger.
HICOLOR_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]


def magick(args, stdin=None):
    """Run ImageMagick and hand back what it wrote to stdout."""
    out = subprocess.run(
        ["magick", *args], input=stdin, stdout=subprocess.PIPE, check=True
    )
    return out.stdout


def plate_png(mark):
    """`mark` on its plate, 1024 px square, as PNG bytes."""
    # Everything goes through files: ImageMagick hands SVG to an external renderer (librsvg
    # or Inkscape), neither of which reads a pipe, and a composite wants two inputs where
    # only one of them could be stdin anyway.
    with tempfile.TemporaryDirectory() as tmp:
        plate = Path(tmp) / "plate.svg"
        plate.write_bytes(PLATE)
        layers = []
        for name, args in (
            ("plate.png", ["-background", "none", str(plate)]),
            (
                "mark.png",
                ["-background", "none", "-density", "1200", str(mark),
                 "-trim", "+repage", "-resize", f"{MARK_WIDTH}x{MARK_WIDTH}"],
            ),
        ):
            path = Path(tmp) / name
            magick([*args, f"png32:{path}"])
            layers.append(str(path))
        return magick([*layers, "-gravity", "center", "-composite", "png32:-"])


def render(size, plates, inset=False):
    """One square PNG of `size` pixels, from whichever drawing reads at that size."""
    box = round(size * APPLE_BOX) if inset else size
    source = plates["flat"] if size < FLAT_BELOW else plates["full"]
    art = magick(
        ["-background", "none", "png32:-", "-filter", "Lanczos",
         "-resize", f"{box}x{box}", "png32:-"],
        stdin=source,
    )
    if not inset:
        return art
    return magick(
        ["-background", "none", "png32:-", "-gravity", "center",
         "-extent", f"{size}x{size}", "png32:-"],
        stdin=art,
    )


def nested_svg(mark):
    """The plate with `mark` nested inside it, for `hicolor/scalable` and the favicon.

    An SVG may contain another SVG with its own viewBox and viewport, so the mark goes in
    whole - every path of it - rather than being redrawn or traced. Only its outer tag's
    `width` and `height` change, into the box the raster cuts centre the mark in.
    """
    text = mark.read_text()
    open_tag = re.search(r"<svg\b[^>]*>", text, re.S)
    if open_tag is None:
        sys.exit(f"no <svg> element in {mark.relative_to(ROOT)}")

    view_box = re.search(r'viewBox="([^"]+)"', open_tag.group(0))
    if view_box is None:
        sys.exit(f"no viewBox on {mark.relative_to(ROOT)}'s <svg>")
    _, _, vw, vh = (float(n) for n in view_box.group(1).split())

    # The same box the raster sizes put the mark in: MARK_WIDTH wide, centred on the plate.
    width = MARK_WIDTH
    height = width * vh / vw
    x = (1024 - width) / 2
    y = (1024 - height) / 2

    inner = re.sub(r'\s+(?:width|height|x|y)="[^"]*"', "", open_tag.group(0))
    # Dropping those attributes can leave a line of the opening tag empty.
    inner = re.sub(r"\n\s*(?=\n)", "", inner)
    inner = inner[:-1].rstrip() + (
        f'\n   x="{x:g}" y="{y:g}" width="{width:g}" height="{height:g}">'
    )
    # Anything before the opening tag is a declaration or a comment, and is dropped.
    body = inner + text[open_tag.end():]

    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f"<!-- Cut from {mark.relative_to(ROOT)} by tools/app-icons/cut.py. Do not edit. -->\n"
        '<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024"'
        ' viewBox="0 0 1024 1024">\n'
        f'  <rect width="1024" height="1024" rx="{PLATE_RADIUS}" fill="{PLATE_COLOUR}"/>\n'
        f"{body}\n"
        "</svg>\n"
    )


def write_ico(frames):
    """An ICONDIR of PNG-compressed entries: every Windows since Vista reads these."""
    out = io.BytesIO()
    out.write(struct.pack("<HHH", 0, 1, len(frames)))
    offset = 6 + 16 * len(frames)
    for size, png in frames:
        # 0 in the width and height bytes means 256; everything else fits in a byte.
        out.write(
            struct.pack(
                "<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), offset
            )
        )
        offset += len(png)
    for _, png in frames:
        out.write(png)
    return out.getvalue()


def write_icns(entries):
    """The icns container: a typed, length-prefixed chunk per entry, PNG payloads throughout."""
    body = b"".join(
        kind + struct.pack(">I", len(png) + 8) + png for kind, png in entries
    )
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def main():
    if shutil.which("magick") is None:
        sys.exit("ImageMagick is not on PATH; try: nix-shell -p imagemagick --run tools/app-icons/cut.py")
    for source in (MASTER, FLAT):
        if not source.exists():
            sys.exit(f"missing {source.relative_to(ROOT)}")

    plates = {"full": plate_png(MASTER), "flat": plate_png(FLAT)}

    ICO.write_bytes(write_ico([(s, render(s, plates)) for s in ICO_SIZES]))
    print(f"{ICO.relative_to(ROOT)}: {len(ICO_SIZES)} sizes, {ICO.stat().st_size} bytes")

    sized = {}
    for kind, size in ICNS_ENTRIES:
        if size not in sized:
            sized[size] = render(size, plates, inset=True)
    ICNS.write_bytes(write_icns([(kind, sized[size]) for kind, size in ICNS_ENTRIES]))
    print(f"{ICNS.relative_to(ROOT)}: {len(ICNS_ENTRIES)} entries, {ICNS.stat().st_size} bytes")

    # hicolor wants one file per size in its own directory, and the theme is installed by
    # copying the tree, so the layout on disk is the layout the spec asks for.
    for size in HICOLOR_SIZES:
        png = HICOLOR / f"{size}x{size}/apps/scoria.png"
        png.parent.mkdir(parents=True, exist_ok=True)
        png.write_bytes(render(size, plates))
    svg = HICOLOR / "scalable/apps/scoria.svg"
    svg.parent.mkdir(parents=True, exist_ok=True)
    svg.write_text(nested_svg(MASTER))
    print(
        f"{HICOLOR.relative_to(ROOT)}: {len(HICOLOR_SIZES)} sizes"
        f" ({', '.join(str(n) for n in HICOLOR_SIZES)}) and scalable"
    )

    FAVICON.write_text(nested_svg(FLAT))
    print(f"{FAVICON.relative_to(ROOT)}: the flat cut on the plate")


if __name__ == "__main__":
    main()
