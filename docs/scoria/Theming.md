---
tags:
  - scoria/dev
  - scoria/design
created: 2026-10-30
---
# Theming

[[Development#Parity (phase 4)]] lists "The themes subset, URI and CLI" as not started. This note is the design for the themes half of that line: what Scoria can be made to theme, what it cannot, and the order the work goes in. It was written after measuring the app on 2026-10-30, and the measurements come first because the conclusion follows from them.

## Where the app stands

**Token discipline is already good.** Of 73 colour literals in `crates/scoria/src`, 57 are in `theme.rs`. The other sixteen are singletons — seven in `pdf/view.rs`, two each in `workspace.rs` and `graph/view.rs`, one each in `settings.rs`, `search.rs`, `prompt.rs`, `math.rs` and `canvas/view.rs`. The thing that normally turns theming into a rewrite, colours inlined at the call site, did not happen here, and fixing those sixteen is an afternoon.

**The vocabulary is the problem.** `Theme` carries fifteen shell colours, three font families and a base size, plus `CanvasColors`'s ten colours, six presets and three alphas. About 750 `theme.*` references paint from them, and nine tokens carry nearly all of it:

| Token | Paint sites |
| --- | --- |
| `text_muted` | 128 |
| `text` | 123 |
| `border` | 100 |
| `bg_alt` | 99 |
| `accent` | 49 |
| `text_faint` | 47 |
| `bg_secondary` | 44 |
| `bg_active` | 34 |
| `bg` | 26 |

`text_muted` alone is the explorer's filenames, the tab strip, the status bar, settings labels, search context lines, the graph legend and the Bases toolbar. A theme that wants the sidebar dimmer than the status bar cannot say so: those two are the same value by accident of sharing a token, not by design. That is a parity-bug generator quite apart from theming, and [[UI parity backlog]] holds entries that are really this.

**Three things are missing outright.**

- **No syntax highlighting.** Neither `syntect` nor `tree-sitter` appears in any manifest; a fenced block renders as mono text on `code_bg`. The input to a highlighter does exist — `scoria-markdown`'s `FencedCode` has carried `info: Option<Cow<str>>`, the unescaped info string, since the fence-language work in `0c15404` — it is simply never read for colour. In a VS Code theme file `tokenColors` is the bulk of the document, so this is a missing axis rather than a missing corner.
- **Headings have no colour of their own.** `theme::heading_style(level)` answers size, weight and line height; `reading/view.rs:631` and `:936` take those three and let colour fall through to plain `theme.text`. Obsidian themes set `--h1-color` through `--h6-color`, and it is among the first things anyone changes.
- **Nothing reloads.** `main.rs:436` already does `cx.set_global(Theme::from_appearance(..))` followed by `cx.notify()`, so the repaint path exists and is exercised when the window answers the system appearance. What is absent is a trigger: `watch.rs` does not watch `.obsidian/`.

The config side, by contrast, is done. `AppearanceConfig` has parsed `cssTheme` (`config.rs:205`) and `enabledCssSnippets` (`:242`) all along, and `Theme::from_appearance` already resolves `obsidian`, `moonstone` and `system` and pulls the three font families out of the same file.

## The ceiling

GPUI has no DOM, no cascade and no selectors. A theme can therefore substitute a **value** into a token the app already paints from; it can never add a **rule**. That is a hard architectural limit, and it decides which bar is reachable.

- **VS Code is reachable.** Its theme surface is large but enumerable — on the order of 600 named keys in the theme-colour reference, plus TextMate and semantic token scopes — and it deliberately forbids custom CSS. It is value substitution into a fixed table, which is exactly what GPUI does.
- **Zed is reachable, and nearer:** a hot-reloaded JSON file of named tokens, on the order of 150 for the UI and 45 for syntax.
- **Obsidian is not reachable.** This is the inversion worth stating plainly, because it is the opposite of what the project's premise suggests. An Obsidian theme is arbitrary CSS: it restyles `.nav-file-title`, adds `::before` content, changes layout. Scoria cannot run that and will not be able to.

So the target is: as themeable as Zed, reaching toward VS Code, and permanently less themeable than the app being cloned.

> [!warning] The ceiling is not a phase
> No amount of registry work gets a selector-level theme working. A Scoria theme is a value table, now and for good. The registry decides *how many* values there are; nothing decides *whether rules are allowed*, because that answer is no.

## The registry: use Obsidian's variable names

Expand the 31 colours to roughly 250, and **take Obsidian's CSS custom-property names as the keys** rather than inventing a scheme.

Three reasons, heaviest first:

1. **Importing a theme becomes a lookup instead of a mapping table.** A community theme's `theme.css` declares `--background-primary`, `--text-muted`, `--tab-text-color-focused-active`. If those strings are the registry's keys, the loader is a dictionary assignment and the correspondence needs no maintenance. Invent a scheme instead and the project owns a 250-row translation table forever, with a new row for every variable Obsidian adds.
2. **It is the compatibility-correct answer.** The ground rule in `CLAUDE.md` is that command ids, config keys and file names match Obsidian's exactly, and that new ones are added only with the observed string. A theme variable is a config key by another name.
3. **`theme.rs` already does this informally.** Its doc comments name `--background-primary-alt`, `--tab-divider-color`, `--background-modifier-border-focus`, `--canvas-dot-pattern`, `--callout-color`, `--font-monospace` and `--h1-size`. The scheme is half-written in prose; the work is promoting it to the identifier and finishing it.

It also ends the naming argument before it starts, which on a 250-token table is worth something by itself.

Keep the Rust side a struct of named fields rather than a `HashMap<String, Hsla>`: a typo should fail to compile. The strings belong in one generated table mapping a variable name to a field, and that table is the loader's only knowledge of CSS. `editor.rs:270`'s `Palette` is derived from `Theme` through `from_theme`, so it regenerates for free and is not a second problem.

## Mapping the call sites

This is the expensive part, and the reason this note exists: about 750 references each have to become the *right* specific token, and that is a judgement per site, not a regular expression.

**Do not guess — probe.** `tools/obsidian-probe/` can read the computed value of a CSS custom property off the real element Obsidian renders for a surface; that is where `theme.rs`'s present values came from (`frame-typography.mjs`, 2026-10-20, recorded in [[Specification]] under "Shell typography and chrome"). The same technique answers the mapping question directly: ask Obsidian which variable `.nav-file-title`'s colour resolves to, and that variable *is* the token the corresponding Scoria call site wants. The method turns 750 judgements into 750 lookups.

The loop runs per surface, not per call site:

1. Name the surface and the Obsidian element that corresponds to it — explorer row, tab, status bar, settings label, search context, Bases toolbar.
2. Probe that element for the custom property each painted colour resolves to, in both default themes.
3. Add those variables to the registry, with the probed values as their defaults.
4. Repoint that surface's call sites, and compare a screenshot before and after.

Step 4 is the test, and it is a strong one: the defaults are the values already being painted, so a correct repoint changes **nothing** on screen. The refactor is complete when it is invisible, and anything that does move is either a mapping error or an already-wrong colour found by accident.

Expect the second kind of finding often. Two surfaces sharing `text_muted` today where Obsidian uses two different variables is precisely the class of bug [[UI parity backlog]] is full of, and this pass surfaces them in bulk rather than one screenshot at a time.

## The loader

Small, once the registry exists.

- Read `.obsidian/themes/<name>/theme.css` for the theme `cssTheme` names, then each file in `.obsidian/snippets/` that `enabledCssSnippets` lists, in that order.
- Scan **custom-property declarations only**, under `:root`, `body`, `.theme-dark` and `.theme-light`. This is a property scanner of a couple of hundred lines, not a CSS engine: every selector that is not one of those four, and every declaration whose property does not begin `--`, is skipped without being understood. Refusing to grow a cascade here is the whole reason it stays small, and that refusal needs defending every time someone asks for one more selector.
- Parse each value as a colour (hex, `rgb()`, `rgba()`, `hsl()`, `hsla()`), a length, or a font list — `first_font` already covers the last case. A `var()` reference resolves against the table being built; anything else is dropped.
- Add `.obsidian/` to `watch.rs`. A changed `theme.css` then rebuilds the table and re-runs the existing `set_global` and `notify`, which is hot reload and live preview at no further cost. (Built: the watcher was already recursive, so this became a filter in the workspace's change handler.)

A native format falls out of this for nothing: the same registry serialised as JSON is a Scoria theme file, Zed-shaped, for anyone who would rather write one than write CSS.

## Syntax highlighting

A separate, self-contained project. Schedule it when code blocks start mattering, not as part of the registry.

The compatibility target is **Prism's token classes** — not TextMate scopes, and not tree-sitter capture names. Obsidian highlights code with Prism.js, so a community theme styles `.token.keyword`, `.token.string`, `.token.comment` and about twenty siblings, and those class names are therefore this axis's registry keys by the same argument as above.

`syntect` ships soonest. `tree-sitter` is the better engine and the one Zed uses, but it adds a per-language grammar build to a project with no other reason to want one. Either way the engine's own token names are mapped onto the Prism set, and the Prism set is what a theme addresses.

## What will not work

Variable-driven themes — Minimal, Things, AnuPpuccin — come out mostly right once the registry and the loader land. Mostly, not wholly: each of them also does selector-level work, and all of that is dropped. The honest figure is three quarters to four fifths of the intended look, and it should be published next to a statement that the remainder is architectural rather than unfinished.

Dropped specifically: any rule with a selector, `::before` and `::after` content, layout and spacing overrides not expressed as a variable, anything animated, and webfonts a theme loads itself.

## Order of work

1. **The registry, ~31 colours to ~250.** Weeks, and the bulk of the entire effort. Worth doing on its own merits even if a theme never loads: a vocabulary that distinguishes the surfaces Obsidian distinguishes is simply more correct than one that does not, and the probe pass finds parity bugs as a side effect.
2. **The loader.** Days, on top of 1. Nothing in it is hard, and the config half already exists.
3. **Syntax highlighting.** One to two weeks, independent of 1 and 2.

Zed's bar is passed at the end of 3. VS Code's is passed at roughly the same moment, because most of the ~350 keys it has beyond that address surfaces Scoria has no equivalent of — the terminal, the debug console, notebooks, peek views.

Do 1 first, and alone. A loader shipped against 31 tokens would produce themes that visibly do not work, and would teach everyone that Scoria theming is broken — a harder impression to repair than a missing feature is to add.

## Progress

**2026-11-02: the registry exists, the probe answered the mapping, and nothing moved on screen.** Step 1's first half is in. `crates/scoria/src/theme/` is now a module: `registry.rs` declares 313 tokens under Obsidian's own property names (every colour the probe saw a shell, editor, reading, pane, menu, prompt or settings surface read, plus the canvas, graph and code tokens the app already painted by name), each with a kind that decides its type; `value.rs` parses and resolves the CSS value grammar those declarations are written in (`var()` with fallback, `hsl()` with `calc()` channels, `rgba()`, `color-mix(in oklch, …)`, `calc()`, lengths, keywords, shadow lists); `defaults.rs` is **generated** by `tools/obsidian-probe/theme-defaults.py` from the probe's record of the declarations under `:root`, `body`, `.theme-dark` and `.theme-light`, chains intact, so overriding `--color-base-70` moves `--text-muted` and all 20 tokens downstream of it exactly as it does in Obsidian. `theme::tests` holds every token to the value Obsidian computed, in both themes.

The mapping question was answered by `theme-registry.mjs` rather than by judgement: for each surface element it reports the cascade-winning declaration of each painted property with its `var()` intact, found by walking the CSSOM, and the condensed table is in [[Specification#Observed theme tokens, Obsidian 1.14.5 (installer build 1.13.7)]]. Two things the measurement in "Where it stands" could not see fell out: the light theme's accent is `--color-accent-1`, a shade lighter than the dark theme's, where Scoria had painted one value in both; and the highlight wash is `--color-yellow` at 30 percent, not a flat gold. One delta is Scoria's own, since 2026-11-02: the accent a vault gets when `appearance.json` sets none is the brick `hsl(14, 62%, 46%)` (`theme::ACCENT`, see [[Scoria#Brand colors]]) rather than Obsidian's purple, declared on `body` after Obsidian's so a theme's `--accent-*` and the inline `accentColor` both still beat it.

The screenshot-equal test from "Mapping the call sites" was run on the docs vault in both themes with the binary from before the change beside the one after: zero differing pixels. The short fields (`text_muted`, `bg_alt`, …) still exist and are now derived from the registry, so the roughly 750 call sites are untouched; **what remains of step 1 is repointing them surface by surface** to the token the table names (`nav_item_color` where it says `--nav-item-color`), with the same screenshot check per surface. `appearance.json`'s `accentColor` now lands on `--accent-h/s/l`, which is how Obsidian applies it, so the brand orange recolours the whole accent chain.

The probe ran under a data directory of its own (`USER_DATA_DIR` in `run-block.sh`), beside the desktop's Obsidian rather than instead of it — Obsidian's single-instance lock is per `--user-data-dir`. Every probe from here on can run that way.

**The same day, the shell, the editor's palette, the context menu, the command palette and the reading view's blocks were repointed** (the list is in [[Development#The theme registry, 2026-11-02]]). Each pass was captured against the pre-registry binary, and every region that moved was a colour the table showed to be wrong — a dozen small parity fixes, icon opacity and tab-title weight among them, found in bulk exactly as "Mapping the call sites" said they would be. By the end of the day every call site read a token and the short fields were deleted: the pane rows, the search pane, Settings, the prompts and the text fields by their records, the Bases, canvas, graph and PDF chrome by the mechanical name of the field they had used, to be refined when a probe reaches them. **Step 1 is done.**

**Step 2, the loader, landed the same night** (`crates/scoria/src/theme/css.rs`; the record is in [[Development#The theme registry, 2026-11-02]]). It is the property scanner described under "The loader" above, with one thing that section did not say and that building it settled: the five selectors are not equal. `.theme-dark` outranks `body` on the body element, so Obsidian's own `.theme-dark { --interactive-accent }` beats a theme's `body { --interactive-accent }`, exactly as it does in Obsidian; the cascade ranks a declaration by scope (inherited from `:root`, or on the body), then `!important`, specificity and order, and Obsidian's defaults are the first sheet in it. A theme's `theme.css` and its enabled snippets are read in that order, the accent setting lands on top as an inline declaration, and `.obsidian/` changes rebuild the registry live.

**The Appearance tab's pickers landed on 2026-11-02 too**, which is what makes steps 1 and 2 reachable without a text editor. They were built to a probe of their own, `appearance-tab.mjs` ([[Specification#Observed Appearance tab, Obsidian 1.14.5 (installer build 1.13.7)]]), and the probe answered three things the plan had not asked:

- **A theme folder counts as installed only when its `manifest.json` names the folder.** A folder whose manifest says something else, and a folder with no manifest, are both dropped by Obsidian — they do not appear in the dropdown and cannot be picked. So `cssTheme` is the folder name, the manifest's name and the dropdown's value all at once, and `themes/<cssTheme>/theme.css` always resolves.
- **Enabled snippets cascade in the snippets folder's order, not in `enabledCssSnippets`' order.** The list in the file is the order the toggles were switched on, but Obsidian rebuilds every `<style>` whenever one is toggled, walking its sorted list of files, so the alphabetically later snippet wins a conflict however the two were enabled. The loader shipped on the night of the 2nd had this wrong and read them in the file's order; `theme::sheets_of` now sorts, and a test pins it.
- **The tab is a drill-in in 1.14.5.** "CSS snippets" and the three font rows are `.setting-item.mod-navigable.tappable` that push a `.setting-page` with a back chevron and a title of its own; the snippets are not rows on the tab at all. Escape on such a page backs out to the tab and leaves Settings open.

Built: the Themes dropdown over `Vault::css_themes()` with the "Open themes folder" button beside it; the "Current community themes" row that counts them; the accent swatch with its "Restore default", which writes a lowercase `#rrggbb` and the empty string the way Obsidian does; and the CSS snippets page with its reload and folder buttons, its search box and a toggle per snippet. Every one of them writes `appearance.json`, which the loader already watches, so a pick recolours the app as it is made — verified on a scratch vault with two themes and three snippets.

Not built, and now in [[UI parity backlog]]: the "Manage" button (Obsidian's community theme browser, which Scoria will not have), the three font pages (Scoria still shows a text field), and the "Quick font size adjustment" and "Custom app icon" rows, whose config keys the probe did not record.

**Step 3, syntax highlighting, landed on 2026-11-02**, and the probe changed the plan's
recommendation before a line was written. `code-highlight.mjs`
([[Specification#Observed code highlighting, Obsidian 1.14.5 (installer build 1.13.7)]]) settled
three things:

- **Obsidian runs two highlighters over one palette.** Prism in the reading view, CodeMirror in
  the editor, both resolving to the same eleven `--code-*`. The section above said the Prism set
  is what a theme addresses; what it did not say is that the *editor* has its own token names and
  that the two disagree — a JSON key is `--code-property` rendered and `--code-function` in the
  editor. One tokenizer is the right answer for Scoria, and it is Prism's, because Prism's names
  are what a theme styles.
- **Thirty-three classes are styled and everything else inherits `--code-normal`.** The rules are
  not reachable through `document.styleSheets`, so each class was put to the renderer directly.
  The registry already had all eleven properties from step 1, so no new tokens were needed.
- **`syntect` is not the shortcut this note assumed.** Its bundled set is 75 syntaxes against
  Obsidian's 171, and it has no TypeScript, TOML, Kotlin, Swift, Nix, Dockerfile or GraphQL.
  Breadth was the whole argument for it, and the breadth is not there.

So Prism was **ported** rather than wrapped: `crates/scoria-highlight` is `matchGrammar` from
`prism-core.js` with the grammars written out from the same upstream source — the move the
project already makes for Lucide's icons. That buys exactness, and the exactness is testable:
`tests/corpus/obsidian-1.14.5.json` holds Obsidian's own `innerHTML` for a sample per language,
and a ported grammar is correct when Scoria's HTML is **identical**, which compares every token's
name, aliases, nesting and boundaries at once. Eight languages match byte for byte today.

What remains is breadth, one grammar at a time — a language with no port renders plain, which is
what Obsidian does for a name Prism has not got. See [[Development#Syntax highlighting, 2026-11-02]].

## See also

- [[Development#Parity (phase 4)]] for the line this note plans, and [[Development#Environment]] for the GPUI pin the ceiling follows from.
- [[UI parity backlog]] for the surfaces still being matched against Obsidian; a surface still in flight is a bad one to repoint first.
- [[Interaction probes]] for how the probe scripts are driven.
- [[Specification]] under "Shell typography and chrome" for the measured values `theme.rs` was written to.
