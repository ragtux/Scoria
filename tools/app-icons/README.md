# App icons

The icon inside the executables and the site's favicon, cut from `docs/img/marks/clast-orange.svg`
the way the two SVG copies of the logo are:

```
just icons                                             # or, without just:
nix-shell -p imagemagick --run tools/app-icons/cut.py
```

It writes three committed icon sets, because no platform can be handed an SVG, and the favicon:

| file | what reads it |
| --- | --- |
| `crates/scoria/resources/windows/scoria.ico` | `crates/scoria/build.rs` embeds it as resource 1 of `scoria.exe`, which is where gpui's window class looks for the window and taskbar icon and where Explorer looks for the file's |
| `crates/scoria/resources/macos/scoria.icns` | `tools/macos-app/bundle.sh` puts it in `Scoria.app/Contents/Resources`, and `Info.plist`'s `CFBundleIconFile` names it |
| `crates/scoria/resources/linux/icons/hicolor/` | `tools/linux-desktop/install.sh` copies the theme into the XDG data directory, where `scoria.desktop`'s `Icon=scoria` finds it |
| `site/sites/scoria/assets/favicon.svg` | every page of scoria.cc links it as its icon |

Two drawings go in, because no single one survives the range from 16 px to 1024 px. At 48 px and
up it is the master itself on a rounded square of paper — the same 7/32 corner radius at every
size — keeping the outline, the facets and all seven vesicles. At 32 px and below it is Anya's
own flat cut of the same clast, `docs/img/marks/clast-flat-orange.svg`: one orange, the outline
and the vesicles and no facets, for exactly this reason — the full drawing turns to mud by 16
px. The favicon is that flat cut on the same plate, nested whole.

The plate is paper and not the chrome's near-black because the mark's own outline is `#221f1f`.
On a dark tile that outline vanishes and the clast reads as a torn orange blob; on paper it reads
as the sticker it is drawn as. The mark is landscape, seven across to five down, so `MARK_WIDTH`
is measured across it and the tall margins are what the shape leaves behind.

The macOS entries are additionally inset to Apple's 824/1024 content box, which is what makes a
Dock icon the same visual size as the ones either side of it; the Windows entries are full-bleed,
which is what Explorer draws.

The `.ico` carries 16, 20, 24, 32, 48, 64, 128 and 256; the `.icns` carries the modern PNG entry
set (`icp4`, `icp5`, `ic07`–`ic14`); hicolor gets 16, 24, 32, 48, 64, 128, 256 and 512 plus the
scalable SVG, which is the plate with the master nested inside it rather than anything redrawn,
so it stays one drawing at any size. Both binary containers are written here rather than by
ImageMagick or `iconutil`, so the cut runs on any machine and not only on a Mac, and
`crates/scoria/tests/icons.rs` parses all three committed sets back.

The cut is reproducible in pixels, not in bytes: librsvg and Inkscape render the master
identically, but two ImageMagick builds will compress the PNGs differently, so re-cutting with no
change to the master can still show a diff.
