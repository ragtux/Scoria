# Frozen config fixtures

`workspace.obsidian.json` is a real `workspace.json` **as Obsidian wrote it**, copied from
`docs/scoria-demo/.obsidian/` on 2026-11-04 (Obsidian 1.14.5 wrote it at 20:58 that evening;
Scoria had not opened that vault). It is frozen here on purpose: every vault in the repository
gitignores its own `workspace.json`, because that file is live per-user state, so without a copy
there is nothing for a test to round-trip on a clean checkout.

It is kept because of what it holds, not because it is tidy:

- `315.1974411010742`-style dock widths. Not because of `f32` narrowing — that is the *typed*
  configs' problem, and this file goes through as a `serde_json::Value`, which holds every float
  as `f64` — but because they make the test sensitive to how a number is *spelled*. Verified by
  hand on 2026-11-04: respell that width as `315.19744110107420` or `3.151974411010742e2` and the
  round trip fails, because `serde_json` re-renders both as `315.1974411010742`. So if Obsidian
  ever writes a number in a form `serde_json` does not reproduce, this fixture is what catches
  it. (Substituting a plain `320` passes, correctly: that is the same bytes either way.)
- An empty object, no trailing newline, and Obsidian's two-space indent.
- `left-ribbon.hiddenItems`, leaf `icon`/`title`, and three `lastOpenFiles` entries pointing at
  Excalidraw notes that no longer exist — keys and values Scoria does not model, which is
  exactly what the merge has to preserve byte for byte.

Do not regenerate it to tidy it up. If it is ever replaced, replace it with a file Obsidian
wrote last, and say so here — a fixture Scoria wrote makes the test agree with itself.
