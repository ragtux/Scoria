---
tags:
  - scoria/dev
  - scoria/design
created: 2026-11-15
---
# Accessibility

Scoria exposes nothing to assistive technology today: no screen reader can read a note, a file
row or a button. Obsidian is an Electron app and gets much of this from Chromium. This is a parity
gap, and a large one. This note records where the support would come from, what it costs, and how
to start.

## Where it comes from: GPUI, not a component kit

[gpui-kit's accessibility guide](https://gpui-kit.com/docs/accessibility/#accessibility) (read
2026-11-15, kit 0.9.1) was the prompt for this note. Its key sentence: "GPUI sends an
accessibility tree to platform assistive technology through [AccessKit](https://accesskit.dev/)."
The element API it documents (`.role(…)`, `.aria_label(…)`, `.aria_description(…)`,
`.aria_selected(…)`, `.aria_expanded(…)`, `.aria_toggled(…)`, `.aria_value(…)`,
`.on_a11y_action(…)`, `accessibility_id(…)`) belongs to GPUI. The kit only applies it to its own
stock components.

So adopting gpui-kit would buy Scoria almost nothing here. Scoria draws its own elements, and the
reasons it does not use the kit's components still hold (see [[GPUI Kit]]). The kit would also
replace our git pin with its `gpui-pre` snapshot (`0.3.12` at the time of reading). What Scoria
needs is the GPUI that has the API.

## The pin is older than the API

`gpui` is pinned to Zed `4ce1874`, the v1.3.6 bump of 2026-05-21 (see the note in the root
`Cargo.toml` and [[Development#Environment]]). AccessKit landed in GPUI a week later:

| Zed PR | Merged | What it brought |
| --- | --- | --- |
| [#56065](https://github.com/zed-industries/zed/pull/56065) | 2026-05-27 | AccessKit support in gpui (`accesskit` 0.24, `accesskit_macos`, `accesskit_unix`, `accesskit_windows`) |
| [#57885](https://github.com/zed-industries/zed/pull/57885) | 2026-05-28 | a panic on invalid focus ids fixed |
| [#57954](https://github.com/zed-industries/zed/pull/57954) | 2026-05-28 | `Application::inaccessible()` |
| [#60397](https://github.com/zed-industries/zed/pull/60397) | 2026-07-13 | landmarks and menu improvements |
| [#61926](https://github.com/zed-industries/zed/pull/61926) | 2026-08-07 | accessibility identifiers for platform clients (UIA `AutomationId`, `AXIdentifier`) |
| [#64214](https://github.com/zed-industries/zed/pull/64214) | after 2026-08 | `aria_current` and its node propagation |

The first stable Zed release containing #56065 is **v1.6.3** (2026-06-10). The newest stable on
2026-11-15 is v1.28.1, about 26 minor releases past the pin.

## What the kit's guide says the API cannot do

These limits are worth knowing before the work starts:
- A node needs both an element id and a non-empty role. `Role::GenericContainer` is filtered out.
- There is no general `.aria_disabled(…)` builder, and no labelledby, describedby or controls
  relations.
- "Tree properties alone do not implement keyboard behavior": focus order, Tab and Shift+Tab, and
  Enter and Space are still the element's own job.
- An input's `SetValue` action is not blocked by a read-only flag; handle it explicitly.
- Linux goes through AT-SPI and "depends on the deployed adapter". Expect it to be the weakest of
  the three platforms.
- "A passing headless tree assertion cannot prove what a screen reader announces." Testing needs a
  real screen reader on each platform: Orca, VoiceOver, NVDA or Narrator.

## The cost

1. **The pin bump.** It is its own piece of work and comes first. Follow the standing rule: one
   release at a time, `cargo run -p scoria` on the NVIDIA machine before each commit, and `w11` and
   the Mac before the last. Re-check the GPUI traps recorded in memory and in [[Development]]: the
   dispatch order (actions before `capture_key_down`), the variable-height `list` reset, a notify
   dropped during paint, and the variable-font bold. The `libc` pin may also be droppable by then.
   Do not bump while branches built on the old pin are still open.
2. **The tree, surface by surface.** Every Scoria element is hand-drawn, so each needs ids, roles,
   names, states and actions added by hand. In rough order of value and difficulty:
   - the ribbon, tab headers, buttons and toggles (cheap: buttons, tabs, switches);
   - the Files, Bookmarks, Outline, Tags and Search trees (`tree.rs`: a tree role with expanded
     and selected states);
   - the command palette, quick switcher and suggestion popups (list box, active descendant);
   - the Settings window (form controls against `settings.rs`'s rows);
   - modals and notices (dialog, status; the guide warns that a status role alone may not be
     spoken);
   - the reading view (document structure: headings, lists, links, tables, from `scoria-markdown`'s
     tree, never a second parser);
   - **the editor**, the hardest: a text role exposing the text, the caret and the selection, and
     live updates while typing. `editor.rs` and `scoria-text` own this.
   - canvas, graph, Bases and PDF last, each needing a design of its own.
3. **Matching Obsidian.** Record what Obsidian's Chromium tree exposes for each surface (CDP's
   `Accessibility.getFullAXTree` in a probe, the same harness as `tools/obsidian-probe`) and hold
   Scoria's roles and names to it, the way the rest of phase 4 holds to observations.

## How to start

1. Bump the pin to v1.6.3 on a scratch branch, alone, and see what breaks.
2. Give one cheap surface (the ribbon and the tab headers) roles and names, and check with Orca in
   a real session that they are announced.
3. Probe Obsidian's accessibility tree for the same surfaces, and write a `## Observed
   accessibility tree, Obsidian <version>` section in [[Specification]].
4. Only then plan the editor.

See also [[GPUI Kit]] for why the kit's components stay out, [[Development]] for the pin, and
[[Plugin SDK]] for the other large piece of work after phase 4.
