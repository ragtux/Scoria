# The marks

Anya's marks, every one cut from her sheet `Scoria_draft3.pdf` (2026-10-29) by
`tools/marks/cut.py`; `just marks` re-cuts them all. Nothing here is edited by hand: change the
sheet, re-cut. Each file is flat - filled paths only, no transform, no style block, no stroke -
and sized to its own drawing in pixels at 96 dpi, which is the sheet's 4/3 of a point.

Two shapes. The **clast** is the logo's lumpy 7:5 chunk: a heavy `#221f1f` outline, fourteen
facets in three inks, seven `#231f20` vesicles, one crack mark and a small chip off its top right
edge. The **pebble** is a rounder 1:1 lump with six facets, nineteen holes and no outline. Each
comes three-toned in the three families and as single-ink cuts; the `-shaded` pair are the
sheet's two studies with gradient-shaded vesicles, kept for the record and not used anywhere.

| file | shape | inks |
| --- | --- | --- |
| `clast-orange.svg` | clast, three-tone | `#ee3300` `#ff5721` `#ff6f21` - **the logo**; `crates/scoria/assets/logo.svg` and `site/sites/scoria/assets/logo.svg` are byte copies |
| `clast-red.svg` | clast, three-tone | `#861e1a` `#a82223` `#ca2b27` |
| `clast-brown.svg` | clast, three-tone | `#6f290e` `#86361e` `#a6462c` |
| `clast-flat-orange.svg` | clast, one ink | `#ee3300` - the app icons at 32 px and below, and the favicon |
| `clast-flat-red.svg` | clast, one ink | `#a01d21` |
| `clast-flat-brown.svg` | clast, one ink | `#92391f` |
| `clast-flat-rust.svg` | clast, one ink | `#d55227` |
| `clast-orange-shaded.svg` | clast, study | the orange three-tone with gradient vesicles |
| `clast-red-shaded.svg` | clast, study | the red three-tone with gradient vesicles |
| `pebble-orange.svg` | pebble, three-tone | `#ee3300` `#ff5721` `#ff6f21` |
| `pebble-red.svg` | pebble, three-tone | `#861e1a` `#a82223` `#ca2b27` |
| `pebble-brown.svg` | pebble, three-tone | `#6f290e` `#86361e` `#a6462c` |
| `pebble-flat-orange.svg` | pebble, one ink | `#ff5721` |
| `pebble-flat-red.svg` | pebble, one ink | `#a82223` |
| `pebble-flat-brown.svg` | pebble, one ink | `#86361e` |
| `pebble-flat-sienna.svg` | pebble, one ink | `#a6462c` |
| `wordmark.svg` | the word, as outlines | `#ffffff`, as the sheet sets it on its `#231f20` ground; recolour the one path for paper |
| `lockup-clast-{orange,red,brown}.svg` | clast + wordmark | the sheet's spacing: the word's box 2.4 px past the clast's, 139.6 px down |
| `lockup-pebble-{orange,red,brown}.svg` | pebble + wordmark | the sheet's spacing: a 52.8 px gap, 121.2 px down |

The palette these inks make, and the three the interface uses, are in
`docs/scoria/Scoria.md` under "Brand colors".

Two things the cut corrects on the way through, both recorded in `tools/marks/cut.py`: one
facet of the flat red clast is `#9e1c20` on the sheet where its neighbours are `#a01d21`, a
slipped digit, and is written as `#a01d21`; and the flat red clast's chip had been dragged up into
the wordmark above it, so that cut has no chip rather than one put back by guesswork. The brown
clast has no chip on the sheet either.

The orange clast is the drawing `anya_v0_base.svg` is of, set 7% taller on this sheet; the
procedural generator in `docs/anya_scoria_gen` still measures the earlier file.
