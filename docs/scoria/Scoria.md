---
aliases:
  - Home
tags:
  - scoria/meta
---
# Scoria

Scoria is a native Rust/GPUI knowledge base that opens [Obsidian](https://obsidian.md) vaults
unchanged. This folder is itself an Obsidian vault: open `docs/scoria` in Obsidian, and once Scoria can
browse a vault, open it in Scoria too. Anything the two apps disagree on here is a bug or a spec gap.

## Notes

- [[Specification]] — what the app implements and where the full specification lives.
- [[Development]] — machine setup, build commands, pinned dependencies, and benchmark results.
- [[Interaction probes]] — the hands-on Obsidian observations the automated probe could not make.
- [[GPUI Kit]] — the component framework Scoria does not use, and when adopting part of it would pay.
- [[Theming]] — how far a Scoria theme can go, and the token registry it needs first.

## Brand colors

The palette is the marks' own inks, read off Anya's sheet of 2026-10-29 (`docs/img/marks/Scoria_draft3.pdf`)
and adopted on 2026-11-02 in place of the three interface oranges picked on 2026-10-17. Three
families, each a shadow, a body and a lit facet - the lit facet is not lighter than the body, it is
the same lightness turned toward yellow - plus the single-colour inks of the flat cuts and the two
near-blacks.

| Family | Shadow | Body | Lit facet | Flat cuts |
| --- | --- | --- | --- | --- |
| Orange, the logo | `#ee3300` | `#ff5721` | `#ff6f21` | `#ee3300` (clast), `#ff5721` (pebble) |
| Red | `#861e1a` | `#a82223` | `#ca2b27` | `#a01d21` (clast), `#a82223` (pebble) |
| Brown | `#6f290e` | `#86361e` | `#a6462c` | `#92391f` (clast), `#86361e` and `#a6462c` (pebble) |
| Rust, a flat clast only | | | | `#d55227` |

| Ink | Hex |
| --- | --- |
| Outline | `#221f1f` |
| Vesicles, and the sheet's ground | `#231f20` |
| Wordmark | `#ffffff` |
| Paper, the plate under any mark on a tile | `#faf9f7` |

Where the interface needs one colour, these are the roles (the site's `scoria.css` tokens and the
README badge read from this table):

| Role | Hex | Contrast |
| --- | --- | --- |
| Brand, large marks, the glow on the social card | `#ff5721` | 6.2:1 on `#0b0b0c` |
| Text and links on the dark theme, hover states | `#ff6f21` | 7.1:1 on `#0b0b0c` |
| Text and links on paper | `#a6462c` | 5.7:1 on `#fbfaf8`, 5.9:1 on white |

No orange of the family clears AA on paper - `#ee3300`, the darkest, is 3.9:1 - which is why the
light theme's accent is the brown family's lit facet, the nearest hue that does.

Inside the app the accent is a vault setting (`accentColor` in `appearance.json`) and a vault that
sets one gets it. A vault that sets none got Obsidian's purple until 2026-11-02; it now gets
Scoria's own brick, `hsl(14, 62%, 46%)` = `#be4f2d`, a tone between `#a6462c` and `#d55227` that
the theme registry declares after Obsidian's purple (`theme::ACCENT`), so a community theme's own
accent still wins over it. Obsidian's derivations then make the dark theme's links `#d3573b` and
the light theme's buttons `#cd512f`. It is brick rather than red so it stays apart from the error
and failure colour, `--color-red` (`#fb464c` dark, `#e93147` light), which is pink-leaning where
the brick is orange-leaning.

## The marks

The marks are Anya's, cut from her sheet by `tools/marks/cut.py` into `docs/img/marks/` (`just marks`);
the table there lists every file. There are two shapes: the **clast**, a lumpy 7:5 chunk with a
heavy outline, fourteen facets and seven vesicles, and the **pebble**, a rounder 1:1 lump with six
facets and nineteen holes and no outline. Each comes in the three families and as flat single-ink
cuts, and the sheet sets the wordmark beside both.

The logo is `docs/img/marks/clast-orange.svg`, the orange three-tone clast, adopted on 2026-11-02 in
place of the shaded rock the project started with (and, the same day, in place of her earlier cut
of the same drawing, `anya_v0_base.svg`, which this one stretches 7% taller). It is flat - five
fills, no gradients and no strokes - and carries its own five inks.

Neither near-black is `#000`, and the outline is the reason anything that sets the mark on a
tile (the app icons, the site's favicon, the social card) uses paper (`#faf9f7`) and not the
chrome's dark: at `#221f1f` the outline disappears into a dark plate and the clast reads as a torn blob.
Over a background, with no tile, the mark is left bare and holds on both themes. Below 48 px the
tiles switch to Anya's flat cut of the clast, `clast-flat-orange.svg`, whose one ink and seven
holes survive where the facets do not.

## Rules for this vault

- Write notes the way Obsidian would: wikilinks, YAML frontmatter, `#tags`, callouts.
- Keep `.obsidian/` committed except `workspace.json`, so the vault exercises Scoria's config reader.
- Project rules for contributors and agents stay in the repository root `CLAUDE.md`.
