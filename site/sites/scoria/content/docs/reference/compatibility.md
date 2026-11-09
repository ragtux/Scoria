---
title: Obsidian compatibility
description: Scoria's four compatibility tiers, what each one promises, the conformance numbers behind tier 1, and where the project currently stands against each.
---

# Obsidian compatibility

Compatibility with Obsidian is Scoria's reason to exist, so it is worth being precise about
what is promised and what is merely intended. The project uses four tiers.

| Tier | Goal | Status |
|---|---|---|
| **1** | Vault and format compatibility — no conversion, no data loss | In scope, and first |
| **2** | Core feature parity with Obsidian's app and its 29 core plugins | In scope |
| **3** | Theme compatibility | A variable-level subset: the custom properties, not the rules |
| **4** | Community plugin compatibility | **Out of scope.** A new native plugin API instead |

## Tier 1: the vault and the formats

This is the tier that actually matters, because it is the one whose failure loses your work.
It means: Scoria reads your vault without converting anything, and writes it without
damaging anything.

The evidence for it is two conformance suites, both **ratchets** in CI — they are not
allowed to go down:

| Suite | Result |
|---|---|
| CommonMark 0.31.2 | **652 of 652** spec examples render byte-identical HTML |
| Obsidian 1.13.7 reading view | **1,008 of 1,009** corpus notes render the same canonical HTML |

The corpus is a real 1,009-note vault rendered by Obsidian itself, kept as expected output.
A third suite renames every note in it under every link format, moves its folders, and
renames its attachments, checking each time that every resolved link still resolves to the
same file, that no link appears or disappears, and that nothing outside the links changed.

### The note that differs

Honesty about a ratchet means naming what it does not cover. One corpus note does not match:
`edge/Probe Inlines.md`, a hand-written probe of inline edge cases, on two kinds of construct:

- How runs of `*` and `_` pair up when they are ambiguous on purpose: `_a_b _a_`,
  `*a **b* c**`, `****a****`, `**a*b**c*`, `~~*a*~~`.
- `==== four ==x===y==`, a highlight run longer than its delimiter.

These are degenerate cases that no one writes on purpose. They are tracked rather than
hidden, and the suite is set at 1,008 so that a regression elsewhere cannot hide behind
them. (A wikilink inside Markdown link text, which used to be the third, now matches.)

### What tier 1 covers beyond Markdown

| | |
|---|---|
| File kinds and extensions | Notes, images, audio, video, PDF, Canvas, Base |
| Forbidden file-name characters | `* " \ / < > : \| ?`, and no leading dot |
| Frontmatter | Line 1 only, BOM and CRLF tolerated |
| Link resolution | Extensions, `/absolute`, `./relative`, ambiguous basenames in Obsidian's order |
| The metadata cache | Links, embeds, tags, headings, sections, list items with Obsidian's `parent` encoding, blocks, footnotes, frontmatter links with key paths |
| Config files | Sparse, optional, unknown keys round-trip |
| JSON Canvas 1.0 | Fields, z-order by array order, tab-indented output |
| Bases | Top-level keys and recursive filters |
| Command ids | All 244 Obsidian registers, with the names of the 227 it shows on desktop and its default hotkeys |

## Tier 2: feature parity

The app's own features and the 29 core plugins on Obsidian's Core plugins tab. This is the
tier currently being built, phase by phase:

| Phase | Gate | Status |
|---|---|---|
| 1. Spike | 50k-line editor, parser conformance corpus, 2 s vault index | **Done** |
| 2. Reader | A daily user browses their real vault with no missing content | **Done** |
| 3. Writer | The vault round-trips byte-identical after a week of mixed use in both apps | **Done**, 9 November 2026, in 0.5.0 |
| 4. Parity | Core plugin parity | **In progress**: the canvas view and writer, Bases, the PDF viewer, math (KaTeX-compatible, not MathJax), the themes subset, `obsidian://` URIs, the command line and popout windows are done; Note composer, Page preview, Slash commands, Footnotes view, Workspaces, Slides, Web viewer, Audio recorder, File recovery, Random note and Unique note creator are not |
| 5. Ecosystem | Native plugin SDK | Not started |

Note the shape of those gates: each is a claim about a person using the program, not a
checklist of features. The phase-2 gate was checked by opening every note of a real vault
and reading it, which is how two fidelity bugs — equal-width table columns and wrapping code
blocks — were found and fixed. The phase-3 gate was a week of mixed use on a real vault, half
in Scoria and half in Obsidian, with the vault diffed every day: every byte that moved was a
person's edit in one of the two apps.

What is in and out today is [what differs from
Obsidian](/docs/reference/differences/).

## Tier 3: themes

Obsidian themes are CSS over 1,047 documented custom properties. Scoria is not a browser, so
a theme's CSS cannot simply be applied. What it does instead is a **variable-level subset**:
the app paints from a registry of about 350 tokens named after Obsidian's own custom
properties (`--background-primary`, `--text-muted`, `--nav-item-color`, …), with Obsidian's
default values and their `var()` chains, so overriding `--color-base-70` moves everything
Obsidian derives from it.

Today `appearance.json`'s base theme, accent color, three font families, font size, CSS theme
(`cssTheme`) and enabled CSS snippets are all honored; its "Show tab title bar" and "Native
menus" switches are not. A theme's `theme.css` and the snippets are read for their
custom-property declarations under `:root`, `html`, `body`, `.theme-dark` and `.theme-light`,
the snippets in the folder's order as Obsidian applies them, and a change under `.obsidian/`
restyles the app while it runs. The Appearance tab picks the theme, the snippets and the
fonts. Fenced code is highlighted by a port of Prism, the highlighter Obsidian uses, so a
theme's `--code-*` colors apply; 85 languages are ported, answering to 122 of the 171 names
Obsidian's Prism knows, and a block in any other language renders plain.

What a theme does with **selectors** is dropped, and that is architectural, not unfinished:
a rule aimed at `.nav-file-title`, `::before` and `::after` content, layout and spacing that
is not a variable, animations, and webfonts a theme loads itself. A theme built on variables
comes out mostly right; one that restyles components does not. Browsing and downloading
community themes is not part of Scoria.

## Tier 4: community plugins

**Obsidian's community plugins cannot run in Scoria, and making them run is not a goal.**

They are JavaScript modules written against an Electron application's API, with access to
Node and the DOM. Scoria is a native binary with no JavaScript runtime and no DOM. Embedding
one to host them would mean rebuilding the thing whose absence is the point.

Instead, tier 4 is a **native plugin API of Scoria's own** — the last planned phase. The
core plugins are being implemented as part of the application rather than as plugins.

Your `community-plugins.json` and your plugins' manifests are still read and shown in
[Settings](/docs/settings/), so the tab tells you truthfully what your vault has installed.

## Next

- [What differs from Obsidian](/docs/reference/differences/) — the current gap list.
- [How Scoria works](/docs/reference/architecture/)
