# PDF fixtures

Generated once and committed; the tests read them as bytes.

- `pages.pdf` (16.6 KB) comes from `pages.typ`: `typst compile pages.typ pages.pdf`
  (typst 0.14.2). Five pages, A4 portrait except page 3 which is landscape, an outline
  from the headings, the title "Pages fixture", and a subset of typst's default font
  (Libertinus Serif) embedded.
- `rotated.pdf` (0.5 KB) and `blank.pdf` (0.4 KB) come from `python3 handmade.py`, which
  writes them by hand with a computed xref table. `rotated.pdf` is a 200 x 100 page with
  `/Rotate 90` and a black rectangle in the unrotated page's bottom-left quadrant;
  `blank.pdf` is an A4 page with no content stream.

Regenerate with the commands above from this directory.

The `probe-*` files are the fixtures of `tools/obsidian-probe/pdf-selection.mjs`
(2026-11-15), copied out of the probe's vault so the tests read the very bytes Obsidian was
observed on: `probe-doc.pdf` is that script's typst document (typst 0.14.2, tagged, so its
text sits in marked content), `probe-text.pdf` its hand-written content stream (standard 14
fonts, every way two text runs meet, colour swatches and an RGB inline image), and the three
`probe-locked-*`/`probe-owner-only` files are `probe-doc.pdf` encrypted by qpdf 12.3.2 (AES-256
and RC4-128 with the user password `secret`, and AES-256 with an owner password only).
`probe-items.json` is pdf.js's `getTextContent()` for the first two pages of the two text
files as Obsidian 1.15.0 returned it, marked content left out: `[str, hasEOL, x, y, width,
height]` per item, which is what a `#selection=` link counts in.
