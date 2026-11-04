// The theme registry: which CSS custom property each surface Scoria paints
// resolves its colours from, and what every custom property computes to, in
// both base themes. This is the "do not guess, probe" step of
// docs/scoria/Theming.md: the variable a surface's colour comes from IS the
// token the matching Scoria call site wants.
//
// Three kinds of record:
//   vars      every `--*` declared under :root / body / .theme-dark /
//             .theme-light in Obsidian's own stylesheets, with the raw
//             declaration per selector (so `var()` chains are known) and the
//             value computed on <body> in each theme
//   surfaces  per surface element: its rect, the computed value of each
//             colour-ish property, and the winning cascade declaration for it
//             (selector, raw value with `var()`, specificity) — plus, when the
//             element sets no `color` of its own, the ancestor it inherits from
//   hover     a few surfaces read again under a real pointer
// Phases: fixtures | run   (node theme-registry.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';
import { LIB } from './theme-lib.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 600000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.themeRegistry = all.themeRegistry || {};
const ONLY = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
// A partial re-run (ONLY_STEPS) keeps the steps it does not touch.
const prev = all.themeRegistry[phase];
const out = { startedAt: new Date().toISOString(), steps: ONLY && prev ? prev.steps : {}, order: ONLY && prev ? prev.order.filter((n) => !ONLY.some((p) => n.startsWith(p))) : [] };
all.themeRegistry[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  return out.steps[name];
}
const vp = (rel) => path.join(VAULT, rel);

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('Folder'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), [
    '---', 'title: Alpha', 'tags:', '  - t/one', '  - two', 'count: 3', 'done: true', 'when: 2026-01-02', '---',
    '# Heading one', '', 'A paragraph with **bold**, *italic*, ~~struck~~, ==highlight==, `inline code`,',
    'a [[Beta]] link, a [[Nowhere]] unresolved link, an [external](https://example.com) link, a #tag1 and #t/one,',
    'a footnote[^1] and a block ^blk.', '', '## Heading two', '', '### Heading three', '', '#### Heading four', '',
    '##### Heading five', '', '###### Heading six', '', '> A blockquote', '', '> [!note] A callout', '> With a body.', '',
    '> [!warning]- A folded warning', '> Hidden.', '', '- [ ] an open task', '- [x] a done task', '- a bullet', '  - nested', '',
    '1. one', '2. two', '', '| a | b |', '| --- | --- |', '| 1 | 2 |', '', '---', '', '```js', 'const x = 1; // comment', '```', '',
    '$$e = mc^2$$', '', '![[Beta]]', '', '[^1]: The footnote.', '',
  ].join('\n'));
  fs.writeFileSync(vp('Beta.md'), '# Beta\n\nBack to [[Alpha]] with #tag1.\n');
  fs.writeFileSync(vp('Folder/Gamma.md'), '# Gamma\n\nSee [[Alpha#Heading two]].\n');
  fs.writeFileSync(vp('Folder/data.json'), '{}\n');
  out.wrote = fs.readdirSync(VAULT);
  save();
  process.exit(0);
}
if (phase === 'after-restart') { process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
out.title = await c.evaluate(`return document.title;`);
save();

// ---- the custom-property table ------------------------------------------
async function dumpVars(label) {
  return c.evaluate(LIB + `
    const decl = {};   // name -> [{selector, value, sheet}]
    for (const { r, sel, sheet } of allRules()) {
      const parts = sel.split(',').map((s) => s.trim());
      const scope = parts.filter((p) => /^(:root|html|body|\\.theme-dark|\\.theme-light|body\\.theme-dark|body\\.theme-light|\\.theme-dark\\.theme-dark|\\.mod-macos|\\.mod-linux|\\.mod-windows|body\\.mod-linux|body\\.is-mobile|body\\.is-phone)$/.test(p) || /^(:root|html|body)(\\.[\\w-]+)*$/.test(p) || /^\\.theme-(dark|light)(\\.[\\w-]+)*$/.test(p));
      if (!scope.length) continue;
      for (let i = 0; i < r.style.length; i++) {
        const name = r.style[i]; if (!name.startsWith('--')) continue;
        (decl[name] = decl[name] || []).push({ selector: scope.join(', '), value: r.style.getPropertyValue(name).trim(), sheet: String(sheet).slice(-40) });
      }
    }
    const cs = W.getComputedStyle(D.body);
    const vars = {};
    for (const name of Object.keys(decl).sort()) vars[name] = { computed: cs.getPropertyValue(name).trim(), declared: decl[name] };
    // Variables used anywhere but declared under no scope selector (set per component).
    const used = new Set();
    for (const { r } of allRules()) for (let i = 0; i < r.style.length; i++) { const v = r.style.getPropertyValue(r.style[i]); for (const m of v.matchAll(/var\\((--[\\w-]+)/g)) used.add(m[1]); }
    const undeclared = Array.from(used).filter((n) => !decl[n]).sort();
    return { label: ${JSON.stringify(label)}, bodyClass: document.body.className, count: Object.keys(vars).length, vars, undeclaredButUsed: undeclared, sheets: Array.from(document.styleSheets).map((s) => (s.href || (s.ownerNode && s.ownerNode.id) || 'inline') + ':' + (() => { try { return s.cssRules.length; } catch { return '?'; } })()) };
  `);
}

// ---- the surfaces ----------------------------------------------------------
// [key, selector, index?]  Keys are the names the registry note will use.
const SHELL = [
  ['body', 'body'],
  ['titlebar', '.titlebar'],
  ['titlebar.text', '.titlebar-text'],
  ['titlebar.button', '.titlebar-button'],
  ['titlebar.button-container', '.titlebar-button-container.mod-right'],
  ['ribbon', '.workspace-ribbon.mod-primary'],
  ['ribbon.action', '.side-dock-ribbon-action'],
  ['ribbon.action.svg', '.side-dock-ribbon-action svg'],
  ['ribbon.actions', '.side-dock-actions'],
  ['sidebar.toggle', '.sidebar-toggle-button'],
  ['sidebar.toggle.svg', '.sidebar-toggle-button svg'],
  ['split.left', '.mod-left-split'],
  ['split.right', '.mod-right-split'],
  ['split.root', '.mod-root'],
  ['split.left.resize-handle', '.mod-left-split .workspace-leaf-resize-handle'],
  ['split.left.tabs', '.workspace-split.mod-left-split .workspace-tabs'],
  ['split.left.leaf-content', '.mod-left-split .workspace-leaf-content'],
  ['split.left.tab-header-container', '.mod-left-split .workspace-tab-header-container'],
  ['split.left.tab-header-container-inner', '.mod-left-split .workspace-tab-header-container-inner'],
  ['split.left.tab-header.active', '.mod-left-split .workspace-tab-header.is-active'],
  ['split.left.tab-header.inactive', '.mod-left-split .workspace-tab-header:not(.is-active)'],
  ['split.left.tab-header.active.icon', '.mod-left-split .workspace-tab-header.is-active .workspace-tab-header-inner-icon'],
  ['split.left.tab-header.inactive.icon', '.mod-left-split .workspace-tab-header:not(.is-active) .workspace-tab-header-inner-icon'],
  ['split.left.tab-header.active.svg', '.mod-left-split .workspace-tab-header.is-active svg'],
  ['split.left.tab-header.inactive.svg', '.mod-left-split .workspace-tab-header:not(.is-active) svg'],
  ['split.left.tab-header-spacer', '.mod-left-split .workspace-tab-header-spacer'],
  ['root.tab-header-container', '.mod-root .workspace-tab-header-container'],
  ['root.tab-header-container-inner', '.mod-root .workspace-tab-header-container-inner'],
  ['root.tab-header.active', '.mod-root .workspace-tab-header.is-active'],
  ['root.tab-header.active.inner', '.mod-root .workspace-tab-header.is-active .workspace-tab-header-inner'],
  ['root.tab-header.active.title', '.mod-root .workspace-tab-header.is-active .workspace-tab-header-inner-title'],
  ['root.tab-header.active.close', '.mod-root .workspace-tab-header.is-active .workspace-tab-header-inner-close-button'],
  ['root.tab-header.active.status', '.mod-root .workspace-tab-header.is-active .workspace-tab-header-status-container'],
  ['root.tab-header.inactive', '.mod-root .workspace-tab-header:not(.is-active)'],
  ['root.tab-header.inactive.inner', '.mod-root .workspace-tab-header:not(.is-active) .workspace-tab-header-inner'],
  ['root.tab-header.inactive.title', '.mod-root .workspace-tab-header:not(.is-active) .workspace-tab-header-inner-title'],
  ['root.tab-header.inactive.close', '.mod-root .workspace-tab-header:not(.is-active) .workspace-tab-header-inner-close-button'],
  ['root.tab-header.new-tab', '.mod-root .workspace-tab-header-new-tab .clickable-icon'],
  ['root.tab-header.tab-list', '.mod-root .workspace-tab-header-tab-list .clickable-icon'],
  ['root.tab-header-spacer', '.mod-root .workspace-tab-header-spacer'],
  ['root.tabs', '.mod-root .workspace-tabs'],
  ['root.leaf', '.mod-root .workspace-leaf'],
  ['root.leaf-content', '.mod-root .workspace-leaf-content'],
  ['view-header', '.mod-root .view-header'],
  ['view-header.title-container', '.mod-root .view-header-title-container'],
  ['view-header.title', '.mod-root .view-header-title'],
  ['view-header.title-parent', '.mod-root .view-header-title-parent'],
  ['view-header.breadcrumb', '.mod-root .view-header-breadcrumb'],
  ['view-header.breadcrumb-separator', '.mod-root .view-header-breadcrumb-separator'],
  ['view-header.nav-button', '.mod-root .view-header-nav-buttons .clickable-icon'],
  ['view-header.nav-button.svg', '.mod-root .view-header-nav-buttons .clickable-icon svg'],
  ['view-header.action', '.mod-root .view-actions .view-action'],
  ['view-header.action.svg', '.mod-root .view-actions .view-action svg'],
  ['view-header.left', '.mod-root .view-header-left'],
  ['view-content', '.mod-root .view-content'],
  ['status-bar', '.status-bar'],
  ['status-bar.item', '.status-bar-item'],
  ['status-bar.item.plugin', '.status-bar-item.plugin-editor-status'],
  ['vault-profile', '.workspace-sidedock-vault-profile'],
  ['vault-profile.switcher', '.workspace-drawer-vault-switcher'],
  ['vault-profile.name', '.workspace-drawer-vault-name'],
  ['vault-profile.actions', '.workspace-drawer-vault-actions'],
  ['vault-profile.action', '.workspace-drawer-vault-actions .clickable-icon'],
  ['vault-profile.action.svg', '.workspace-drawer-vault-actions .clickable-icon svg'],
  ['clickable-icon', '.clickable-icon'],
  ['clickable-icon.svg', '.clickable-icon svg'],
  ['clickable-icon.active', '.clickable-icon.is-active'],
];
const EXPLORER = [
  ['nav.header', '.nav-header'],
  ['nav.buttons-container', '.nav-buttons-container'],
  ['nav.button', '.nav-buttons-container .clickable-icon'],
  ['nav.button.svg', '.nav-buttons-container .clickable-icon svg'],
  ['nav.files-container', '.nav-files-container'],
  ['nav.folder.root', '.nav-folder.mod-root'],
  ['nav.folder.root.title', '.nav-folder.mod-root > .nav-folder-title'],
  ['nav.folder.root.title-content', '.nav-folder.mod-root > .nav-folder-title .nav-folder-title-content'],
  ['nav.folder', '.nav-folder:not(.mod-root)'],
  ['nav.folder.title', '.nav-folder:not(.mod-root) > .nav-folder-title'],
  ['nav.folder.title-content', '.nav-folder:not(.mod-root) > .nav-folder-title .nav-folder-title-content'],
  ['nav.folder.collapse-indicator', '.nav-folder:not(.mod-root) > .nav-folder-title .collapse-icon, .nav-folder:not(.mod-root) > .nav-folder-title .nav-folder-collapse-indicator'],
  ['nav.folder.collapse-indicator.svg', '.nav-folder:not(.mod-root) > .nav-folder-title .collapse-icon svg, .nav-folder:not(.mod-root) > .nav-folder-title .nav-folder-collapse-indicator svg'],
  ['nav.folder.collapsed.collapse-indicator', '.nav-folder.is-collapsed:not(.mod-root) > .nav-folder-title .collapse-icon'],
  ['nav.file.nonmd', '.nav-file-title[data-path$=".json"]'],
  ['nav.file.nonmd.content', '.nav-file-title[data-path$=".json"] .nav-file-title-content'],
  ['nav.folder.children', '.nav-folder:not(.mod-root) > .nav-folder-children'],
  ['nav.file', '.nav-file'],
  ['nav.file.title', '.nav-file-title:not(.is-active)'],
  ['nav.file.title-content', '.nav-file-title:not(.is-active) .nav-file-title-content'],
  ['nav.file.title.active', '.nav-file-title.is-active'],
  ['nav.file.title.active-content', '.nav-file-title.is-active .nav-file-title-content'],
  ['nav.file.tag', '.nav-file-tag'],
  ['tree-item', '.tree-item'],
  ['tree-item.self', '.tree-item-self'],
  ['tree-item.inner', '.tree-item-inner'],
];
const READING = [
  ['preview', '.markdown-preview-view'],
  ['preview.sizer', '.markdown-preview-sizer'],
  ['preview.pusher', '.markdown-preview-pusher'],
  ['preview.inline-title', '.markdown-preview-view .inline-title'],
  ['preview.h1', '.markdown-preview-view h1:not(.inline-title)'],
  ['preview.h2', '.markdown-preview-view h2'],
  ['preview.h3', '.markdown-preview-view h3'],
  ['preview.h4', '.markdown-preview-view h4'],
  ['preview.h5', '.markdown-preview-view h5'],
  ['preview.h6', '.markdown-preview-view h6'],
  ['preview.h2.collapse-indicator', '.markdown-preview-view .heading-collapse-indicator'],
  ['preview.p', '.markdown-preview-view p'],
  ['preview.strong', '.markdown-preview-view strong'],
  ['preview.em', '.markdown-preview-view em'],
  ['preview.del', '.markdown-preview-view del'],
  ['preview.mark', '.markdown-preview-view mark'],
  ['preview.code', '.markdown-preview-view p code'],
  ['preview.internal-link', '.markdown-preview-view a.internal-link:not(.is-unresolved)'],
  ['preview.internal-link.unresolved', '.markdown-preview-view a.internal-link.is-unresolved'],
  ['preview.external-link', '.markdown-preview-view a.external-link'],
  ['preview.tag', '.markdown-preview-view a.tag'],
  ['preview.footnote-ref', '.markdown-preview-view .footnote-ref a, .markdown-preview-view sup.footnote-ref'],
  ['preview.footnote-link', '.markdown-preview-view .footnote-link'],
  ['preview.footnotes', '.markdown-preview-view .footnotes'],
  ['preview.footnote-backref', '.markdown-preview-view .footnote-backref'],
  ['preview.blockquote', '.markdown-preview-view blockquote'],
  ['preview.callout', '.markdown-preview-view .callout[data-callout="note"]'],
  ['preview.callout.title', '.markdown-preview-view .callout[data-callout="note"] .callout-title'],
  ['preview.callout.title-inner', '.markdown-preview-view .callout[data-callout="note"] .callout-title-inner'],
  ['preview.callout.icon', '.markdown-preview-view .callout[data-callout="note"] .callout-icon'],
  ['preview.callout.icon.svg', '.markdown-preview-view .callout[data-callout="note"] .callout-icon svg'],
  ['preview.callout.content', '.markdown-preview-view .callout[data-callout="note"] .callout-content'],
  ['preview.callout.warning', '.markdown-preview-view .callout[data-callout="warning"]'],
  ['preview.callout.warning.fold', '.markdown-preview-view .callout[data-callout="warning"] .callout-fold'],
  ['preview.callout.warning.fold.svg', '.markdown-preview-view .callout[data-callout="warning"] .callout-fold svg'],
  ['preview.ul', '.markdown-preview-view ul'],
  ['preview.li', '.markdown-preview-view ul > li:not(.task-list-item)'],
  ['preview.li.bullet', '.markdown-preview-view .list-bullet'],
  ['preview.li.collapse-indicator', '.markdown-preview-view .list-collapse-indicator'],
  ['preview.ol', '.markdown-preview-view ol'],
  ['preview.ol.li', '.markdown-preview-view ol > li'],
  ['preview.task', '.markdown-preview-view .task-list-item:not(.is-checked)'],
  ['preview.task.checkbox', '.markdown-preview-view .task-list-item:not(.is-checked) input.task-list-item-checkbox'],
  ['preview.task.checked', '.markdown-preview-view .task-list-item.is-checked'],
  ['preview.task.checked.checkbox', '.markdown-preview-view .task-list-item.is-checked input.task-list-item-checkbox'],
  ['preview.table', '.markdown-preview-view table'],
  ['preview.table.wrapper', '.markdown-preview-view .table-wrapper'],
  ['preview.th', '.markdown-preview-view th'],
  ['preview.td', '.markdown-preview-view td'],
  ['preview.tr', '.markdown-preview-view tbody tr'],
  ['preview.hr', '.markdown-preview-view hr'],
  ['preview.pre', '.markdown-preview-view pre'],
  ['preview.pre.code', '.markdown-preview-view pre > code'],
  ['preview.pre.copy', '.markdown-preview-view pre .copy-code-button'],
  ['preview.pre.flair', '.markdown-preview-view pre .code-block-flair'],
  ['preview.token.keyword', '.markdown-preview-view .token.keyword'],
  ['preview.token.comment', '.markdown-preview-view .token.comment'],
  ['preview.token.number', '.markdown-preview-view .token.number'],
  ['preview.token.operator', '.markdown-preview-view .token.operator'],
  ['preview.token.punctuation', '.markdown-preview-view .token.punctuation'],
  ['preview.math', '.markdown-preview-view .math'],
  ['preview.embed', '.markdown-preview-view .markdown-embed'],
  ['preview.embed.title', '.markdown-preview-view .markdown-embed-title'],
  ['preview.embed.link', '.markdown-preview-view .markdown-embed-link'],
  ['preview.embed.content', '.markdown-preview-view .markdown-embed-content'],
  ['preview.embed.h1', '.markdown-preview-view .markdown-embed h1'],
  ['preview.metadata', '.markdown-preview-view .metadata-container'],
  ['preview.metadata.properties-heading', '.markdown-preview-view .metadata-properties-heading'],
  ['preview.metadata.properties-title', '.markdown-preview-view .metadata-properties-title'],
  ['preview.metadata.collapse-indicator', '.markdown-preview-view .metadata-properties-heading .collapse-indicator'],
  ['preview.metadata.property', '.markdown-preview-view .metadata-property'],
  ['preview.metadata.property.key', '.markdown-preview-view .metadata-property-key'],
  ['preview.metadata.property.icon', '.markdown-preview-view .metadata-property-icon'],
  ['preview.metadata.property.icon.svg', '.markdown-preview-view .metadata-property-icon svg'],
  ['preview.metadata.property.key-input', '.markdown-preview-view .metadata-property-key-input'],
  ['preview.metadata.property.value', '.markdown-preview-view .metadata-property-value'],
  ['preview.metadata.input-text', '.markdown-preview-view .metadata-input-text'],
  ['preview.metadata.input-number', '.markdown-preview-view .metadata-input-number'],
  ['preview.metadata.checkbox', '.markdown-preview-view .metadata-input-checkbox, .markdown-preview-view .metadata-property[data-property-type="checkbox"] input'],
  ['preview.metadata.date', '.markdown-preview-view .metadata-property[data-property-type="date"] input'],
  ['preview.metadata.pill', '.markdown-preview-view .multi-select-pill'],
  ['preview.metadata.pill-content', '.markdown-preview-view .multi-select-pill-content'],
  ['preview.metadata.pill-remove', '.markdown-preview-view .multi-select-pill-remove-button'],
  ['preview.metadata.pill-input', '.markdown-preview-view .multi-select-input'],
  ['preview.metadata.add-button', '.markdown-preview-view .metadata-add-button'],
];
const EDITOR = [
  ['cm.editor', '.cm-editor'],
  ['cm.scroller', '.cm-scroller'],
  ['cm.sizer', '.cm-sizer'],
  ['cm.content', '.cm-content'],
  ['cm.contentContainer', '.cm-contentContainer'],
  ['cm.line', '.cm-line'],
  ['cm.line.active', '.cm-active.cm-line'],
  ['cm.cursor', '.cm-cursor'],
  ['cm.selection', '.cm-selectionBackground'],
  ['cm.gutters', '.cm-gutters'],
  ['cm.gutter.lineNumbers', '.cm-lineNumbers'],
  ['cm.gutter.fold', '.cm-foldGutter'],
  ['cm.h1', '.HyperMD-header-1'],
  ['cm.h1.text', '.cm-header-1'],
  ['cm.h2', '.HyperMD-header-2'],
  ['cm.h2.text', '.cm-header-2'],
  ['cm.h3.text', '.cm-header-3'],
  ['cm.h6.text', '.cm-header-6'],
  ['cm.formatting-header', '.cm-formatting-header'],
  ['cm.strong', '.cm-strong'],
  ['cm.em', '.cm-em'],
  ['cm.strikethrough', '.cm-strikethrough'],
  ['cm.highlight', '.cm-highlight'],
  ['cm.inline-code', '.cm-inline-code'],
  ['cm.formatting', '.cm-formatting'],
  ['cm.formatting-strong', '.cm-formatting-strong'],
  ['cm.formatting-link', '.cm-formatting-link'],
  ['cm.formatting-link-start', '.cm-formatting-link-start'],
  ['cm.hmd-internal-link', '.cm-hmd-internal-link'],
  ['cm.hmd-internal-link.unresolved', '.cm-hmd-internal-link.is-unresolved'],
  ['cm.link', '.cm-link'],
  ['cm.url', '.cm-url'],
  ['cm.hashtag', '.cm-hashtag'],
  ['cm.hashtag-begin', '.cm-hashtag-begin'],
  ['cm.footref', '.cm-footref'],
  ['cm.blockid', '.cm-blockid'],
  ['cm.quote', '.HyperMD-quote'],
  ['cm.quote.text', '.cm-quote'],
  ['cm.callout', '.HyperMD-callout'],
  ['cm.callout.widget', '.cm-callout'],
  ['cm.callout.widget.title', '.cm-callout .callout-title'],
  ['cm.list-line', '.HyperMD-list-line'],
  ['cm.list-bullet', '.cm-formatting-list .list-bullet'],
  ['cm.formatting-list', '.cm-formatting-list'],
  ['cm.task', '.HyperMD-task-line'],
  ['cm.task.checkbox', '.HyperMD-task-line .task-list-item-checkbox'],
  ['cm.task.checked', '.cm-line[data-task="x"]'],
  ['cm.task.checked.text', '.cm-line[data-task="x"] .cm-list-1'],
  ['cm.indent', '.cm-indent'],
  ['cm.hmd-table', '.HyperMD-table-row'],
  ['cm.table-widget', '.cm-table-widget'],
  ['cm.table-widget.th', '.cm-table-widget th'],
  ['cm.table-widget.td', '.cm-table-widget td'],
  ['cm.hr', '.HyperMD-hr'],
  ['cm.hr.text', '.cm-hr'],
  ['cm.codeblock', '.HyperMD-codeblock'],
  ['cm.codeblock.begin', '.HyperMD-codeblock-begin'],
  ['cm.codeblock.text', '.cm-hmd-codeblock'],
  ['cm.codeblock.flair', '.code-block-flair'],
  ['cm.codeblock.keyword', '.cm-keyword'],
  ['cm.codeblock.comment', '.cm-comment'],
  ['cm.codeblock.number', '.cm-number'],
  ['cm.frontmatter', '.HyperMD-frontmatter'],
  ['cm.frontmatter.text', '.cm-hmd-frontmatter'],
  ['cm.math', '.cm-math'],
  ['cm.math.block', '.HyperMD-math'],
  ['cm.embed', '.cm-embed-block'],
  ['cm.embed.title', '.cm-embed-block .markdown-embed-title'],
  ['cm.embed.edit', '.cm-embed-block .edit-block-button'],
  ['cm.fold-placeholder', '.cm-foldPlaceholder'],
  ['cm.metadata', '.cm-editor .metadata-container'],
  ['cm.metadata.property.key-input', '.cm-editor .metadata-property-key-input'],
  ['cm.metadata.pill', '.cm-editor .multi-select-pill'],
];
const SOURCE = [
  ['src.h1', '.HyperMD-header-1'],
  ['src.formatting-header', '.cm-formatting-header'],
  ['src.formatting-strong', '.cm-formatting-strong'],
  ['src.formatting-link', '.cm-formatting-link'],
  ['src.hmd-internal-link', '.cm-hmd-internal-link'],
  ['src.formatting-list', '.cm-formatting-list'],
  ['src.formatting-task', '.cm-formatting-task'],
  ['src.codeblock', '.HyperMD-codeblock'],
  ['src.codeblock.text', '.cm-hmd-codeblock'],
  ['src.formatting-code-block', '.cm-formatting-code-block'],
  ['src.frontmatter.text', '.cm-hmd-frontmatter'],
  ['src.quote', '.HyperMD-quote'],
  ['src.formatting-quote', '.cm-formatting-quote'],
  ['src.hr', '.HyperMD-hr'],
  ['src.table-row', '.HyperMD-table-row'],
  ['src.line', '.cm-line'],
  ['src.content', '.cm-content'],
];
const PANES = [
  ['pane.search.input-container', '.search-input-container'],
  ['pane.search.input', '.search-input-container input'],
  ['pane.search.clear', '.search-input-clear-button'],
  ['pane.search.toggle', '.search-input-container + .search-params .clickable-icon, .document-search-button'],
  ['pane.search.result-container', '.search-result-container'],
  ['pane.search.results-info', '.search-results-info'],
  ['pane.search.result-count', '.search-results-result-count'],
  ['pane.search.result', '.search-result'],
  ['pane.search.file-title', '.search-result-file-title'],
  ['pane.search.file-title-inner', '.search-result-file-title .tree-item-inner'],
  ['pane.search.file-flair', '.search-result-file-title .tree-item-flair'],
  ['pane.search.collapse-icon', '.search-result-file-title .collapse-icon'],
  ['pane.search.file-matches', '.search-result-file-matches'],
  ['pane.search.file-match', '.search-result-file-match'],
  ['pane.search.matched-text', '.search-result-file-matched-text'],
  ['pane.search.hover-button', '.search-result-hover-button'],
  ['pane.tags.tag', '.tag-pane-tag'],
  ['pane.tags.tag-self', '.tag-pane-tag .tree-item-self'],
  ['pane.tags.tag-text', '.tag-pane-tag .tree-item-inner-text, .tag-pane-tag .tree-item-inner'],
  ['pane.tags.tag-inner', '.tag-pane-tag .tree-item-inner'],
  ['pane.tags.tag-count', '.tag-pane-tag-count'],
  ['pane.tags.collapse-icon', '.tag-pane-tag .collapse-icon'],
  ['pane.outline.item', '.outline .tree-item-self, .outline-view .tree-item-self, [data-type="outline"] .tree-item-self'],
  ['pane.outline.inner', '.outline .tree-item-inner, .outline-view .tree-item-inner, [data-type="outline"] .tree-item-inner'],
  ['pane.outline.item.active', '[data-type="outline"] .tree-item-self.is-active, [data-type="outline"] .tree-item-self.mod-active'],
  ['pane.outline.collapse-icon', '[data-type="outline"] .collapse-icon'],
  ['pane.backlinks.header', '.backlink-pane .tree-item-self.is-clickable'],
  ['pane.backlinks.header-inner', '.backlink-pane .tree-item-inner'],
  ['pane.backlinks.count', '.backlink-pane .tree-item-flair'],
  ['pane.backlinks.file-title', '.backlink-pane .search-result-file-title'],
  ['pane.backlinks.matched-text', '.backlink-pane .search-result-file-matched-text'],
  ['pane.empty-state', '.pane-empty'],
  ['pane.nav-header', '.nav-header'],
];
const MENU = [
  ['menu', '.menu'],
  ['menu.item', '.menu-item:not(.selected)'],
  ['menu.item.selected', '.menu-item.selected'],
  ['menu.item.icon', '.menu-item-icon'],
  ['menu.item.icon.svg', '.menu-item-icon svg'],
  ['menu.item.title', '.menu-item-title'],
  ['menu.item.warning', '.menu-item.is-warning'],
  ['menu.item.warning.title', '.menu-item.is-warning .menu-item-title'],
  ['menu.item.submenu', '.menu-item.mod-submenu'],
  ['menu.item.submenu-icon', '.menu-item-submenu-icon'],
  ['menu.separator', '.menu-separator'],
  ['menu.scroll', '.menu-scroll'],
];
const PROMPT = [
  ['modal-container', '.modal-container'],
  ['modal-bg', '.modal-bg'],
  ['prompt', '.prompt'],
  ['prompt.input-container', '.prompt-input-container'],
  ['prompt.input', '.prompt-input'],
  ['prompt.input-cta', '.prompt-input-cta'],
  ['prompt.results', '.prompt-results'],
  ['prompt.item', '.suggestion-item:not(.is-selected)'],
  ['prompt.item.selected', '.suggestion-item.is-selected'],
  ['prompt.item.content', '.suggestion-item .suggestion-content'],
  ['prompt.item.title', '.suggestion-item .suggestion-title'],
  ['prompt.item.note', '.suggestion-item .suggestion-note'],
  ['prompt.item.aux', '.suggestion-item .suggestion-aux'],
  ['prompt.item.flair', '.suggestion-item .suggestion-flair'],
  ['prompt.item.hotkey', '.suggestion-item .suggestion-hotkey'],
  ['prompt.item.highlight', '.suggestion-item .suggestion-highlight'],
  ['prompt.item.icon', '.suggestion-item .suggestion-icon'],
  ['prompt.item.icon.svg', '.suggestion-item .suggestion-icon svg'],
  ['prompt.instructions', '.prompt-instructions'],
  ['prompt.instruction', '.prompt-instruction'],
  ['prompt.instruction-command', '.prompt-instruction-command'],
];
const SETTINGS = [
  ['settings.modal', '.modal.mod-settings'],
  ['settings.modal-bg', '.modal-bg'],
  ['settings.close', '.modal-close-button'],
  ['settings.sidebar', '.vertical-tab-header'],
  ['settings.group-title', '.vertical-tab-header-group-title'],
  ['settings.nav-item', '.vertical-tab-nav-item:not(.is-active)'],
  ['settings.nav-item.active', '.vertical-tab-nav-item.is-active'],
  ['settings.content', '.vertical-tab-content'],
  ['settings.content-container', '.vertical-tab-content-container'],
  ['settings.heading', '.setting-item.setting-item-heading'],
  ['settings.heading.name', '.setting-item.setting-item-heading .setting-item-name'],
  ['settings.item', '.setting-item:not(.setting-item-heading)'],
  ['settings.item.name', '.setting-item:not(.setting-item-heading) .setting-item-name'],
  ['settings.item.description', '.setting-item:not(.setting-item-heading) .setting-item-description'],
  ['settings.item.control', '.setting-item-control'],
  ['settings.toggle', '.checkbox-container:not(.is-enabled)'],
  ['settings.toggle.enabled', '.checkbox-container.is-enabled'],
  ['settings.select', 'select.dropdown'],
  ['settings.input', '.setting-item-control input[type="text"]'],
  ['settings.button', '.setting-item-control button:not(.mod-cta):not(.mod-warning)'],
  ['settings.button.cta', 'button.mod-cta'],
  ['settings.button.warning', 'button.mod-warning'],
  ['settings.slider', 'input.slider'],
  ['settings.search-input', '.setting-search-container input, .search-input-container input'],
  ['settings.extra-button', '.setting-item-control .clickable-icon'],
  ['settings.info', '.setting-item-info'],
  ['settings.hotkey', '.setting-hotkey'],
  ['settings.hotkey.empty', '.setting-hotkey.mod-empty'],
  ['settings.hotkey.icon', '.setting-hotkey-icon'],
  ['settings.command-name', '.setting-command-hotkeys .setting-item-name, .setting-item-name'],
];
const MISC = [
  ['notice', '.notice'],
  ['notice-container', '.notice-container'],
  ['tooltip', '.tooltip'],
  ['tooltip.arrow', '.tooltip-arrow'],
  ['popover', '.popover.hover-popover'],
  ['popover.embed', '.popover.hover-popover .markdown-embed'],
  ['empty-state', '.empty-state'],
  ['empty-state.title', '.empty-state-title'],
  ['empty-state.action', '.empty-state-action'],
  ['empty-state.container', '.empty-state-container'],
  ['suggestion-container', '.suggestion-container'],
  ['suggestion-container.item', '.suggestion-container .suggestion-item'],
  ['suggestion-container.item.selected', '.suggestion-container .suggestion-item.is-selected'],
  ['suggestion-container.highlight', '.suggestion-container .suggestion-highlight'],
  ['suggestion-container.note', '.suggestion-container .suggestion-note'],
  ['suggestion-container.flair', '.suggestion-container .suggestion-flair'],
];

// The editor keeps only the lines near the viewport in the DOM: read the list
// with the caret parked on each of `lines` in turn and keep the first answer.
async function describeManyScrolling(list, lines) {
  const merged = {};
  for (const line of lines) {
    await c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); if (v && v.editor) { v.editor.setCursor({ line: ${line}, ch: 0 }); v.editor.scrollIntoView({ from: { line: ${line}, ch: 0 }, to: { line: ${line}, ch: 0 } }, true); } return true;`);
    await sleep(350);
    const got = await describeMany(list);
    for (const [k, v] of Object.entries(got)) if (v && !v.error && !merged[k]) merged[k] = Object.assign({ atLine: line }, v);
  }
  for (const [k] of list) if (!merged[k]) merged[k] = null;
  return merged;
}
async function describeMany(list, docExpr = 'null') {
  return c.evaluate(`const __DOC = ${docExpr};` + LIB + `
    const list = ${JSON.stringify(list)};
    const o = {};
    for (const [key, sel, idx] of list) { try { const el = q(sel, idx || 0); o[key] = el ? describe(el) : null; if (o[key]) o[key].selector = sel; } catch (e) { o[key] = { error: String(e) }; } }
    return o;
  `);
}

// Hover a few real surfaces and read them again. Each entry: the selector
// to park the pointer on and the selectors to read while it is there.
async function hovers(list) {
  const o = {};
  for (const [key, over, read] of list) {
    try {
      await c.evaluate(`const el = document.querySelector(${JSON.stringify(over)}); if (el) el.scrollIntoView({ block: 'center' }); return true;`);
      await sleep(120);
      const r = await c.hover(over);
      if (!r) { o[key] = null; continue; }
      await sleep(250);
      o[key] = await c.evaluate(LIB + `
        const sels = ${JSON.stringify(read)}; const o = {};
        for (const [k, sel] of sels) { const el = q(sel); o[k] = el ? describe(el, { computed: ['color', 'background-color', 'border-color', 'border-bottom-color', 'opacity', 'fill', 'stroke', 'font-weight', 'box-shadow', 'text-decoration-color'] }) : null; }
        const hov = document.querySelectorAll(':hover'); o.hoverChain = Array.from(hov).slice(-4).map((e) => e.tagName.toLowerCase() + '.' + String(e.className).trim().split(/\\s+/).slice(0, 3).join('.'));
        return o;`);
    } catch (e) { o[key] = { error: String(e) }; }
    await c.mouseMove(2, 400);
    await sleep(120);
  }
  return o;
}

const HOVER_SHELL = [
  ['nav.file.hover', '.nav-file-title[data-path="Beta.md"]', [['title', '.nav-file-title:not(.is-active):hover'], ['content', '.nav-file-title:not(.is-active):hover .nav-file-title-content']]],
  ['nav.folder.hover', '.nav-folder:not(.mod-root) > .nav-folder-title', [['title', '.nav-folder-title:hover'], ['content', '.nav-folder-title:hover .nav-folder-title-content'], ['indicator', '.nav-folder-title:hover .nav-folder-collapse-indicator']]],
  ['root.tab.inactive.hover', '.mod-root .workspace-tab-header:not(.is-active)', [['tab', '.mod-root .workspace-tab-header:hover'], ['inner', '.mod-root .workspace-tab-header:hover .workspace-tab-header-inner'], ['title', '.mod-root .workspace-tab-header:hover .workspace-tab-header-inner-title'], ['close', '.mod-root .workspace-tab-header:hover .workspace-tab-header-inner-close-button']]],
  ['root.tab.active.hover', '.mod-root .workspace-tab-header.is-active', [['tab', '.mod-root .workspace-tab-header.is-active:hover'], ['inner', '.mod-root .workspace-tab-header.is-active:hover .workspace-tab-header-inner'], ['title', '.mod-root .workspace-tab-header.is-active:hover .workspace-tab-header-inner-title'], ['close', '.mod-root .workspace-tab-header.is-active:hover .workspace-tab-header-inner-close-button']]],
  ['dock.tab.inactive.hover', '.mod-left-split .workspace-tab-header:not(.is-active)', [['tab', '.mod-left-split .workspace-tab-header:hover'], ['inner', '.mod-left-split .workspace-tab-header:hover .workspace-tab-header-inner'], ['svg', '.mod-left-split .workspace-tab-header:hover svg']]],
  ['ribbon.action.hover', '.side-dock-ribbon-action', [['action', '.side-dock-ribbon-action:hover'], ['svg', '.side-dock-ribbon-action:hover svg']]],
  ['view-action.hover', '.mod-root .workspace-leaf.mod-active .view-actions .view-action', [['action', '.view-action:hover'], ['svg', '.view-action:hover svg']]],
  ['nav.button.hover', '.nav-buttons-container .clickable-icon', [['button', '.nav-buttons-container .clickable-icon:hover'], ['svg', '.nav-buttons-container .clickable-icon:hover svg']]],
  ['new-tab.hover', '.mod-root .workspace-tab-header-new-tab .clickable-icon', [['button', '.workspace-tab-header-new-tab .clickable-icon:hover']]],
  ['status-bar.item.hover', '.status-bar-item.mod-clickable', [['item', '.status-bar-item:hover']]],
  ['resize-handle.hover', '.mod-left-split .workspace-leaf-resize-handle', [['handle', '.workspace-leaf-resize-handle:hover']]],
  ['vault-switcher.hover', '.workspace-drawer-vault-switcher', [['switcher', '.workspace-drawer-vault-switcher:hover'], ['name', '.workspace-drawer-vault-switcher:hover .workspace-drawer-vault-name']]],
];
const HOVER_READING = [
  ['preview.internal-link.hover', '.markdown-preview-view a.internal-link:not(.is-unresolved)', [['link', '.markdown-preview-view a.internal-link:hover']]],
  ['preview.external-link.hover', '.markdown-preview-view a.external-link', [['link', '.markdown-preview-view a.external-link:hover']]],
  ['preview.tag.hover', '.markdown-preview-view a.tag', [['tag', '.markdown-preview-view a.tag:hover']]],
  ['preview.pre.hover', '.markdown-preview-view pre', [['pre', '.markdown-preview-view pre:hover'], ['copy', '.markdown-preview-view pre:hover .copy-code-button']]],
  ['preview.task.hover', '.markdown-preview-view .task-list-item:not(.is-checked) input.task-list-item-checkbox', [['checkbox', '.markdown-preview-view input.task-list-item-checkbox:hover']]],
  ['preview.metadata.property.hover', '.markdown-preview-view .metadata-property', [['property', '.markdown-preview-view .metadata-property:hover'], ['key', '.markdown-preview-view .metadata-property:hover .metadata-property-key'], ['icon', '.markdown-preview-view .metadata-property:hover .metadata-property-icon']]],
  ['preview.metadata.pill.hover', '.markdown-preview-view .multi-select-pill', [['pill', '.markdown-preview-view .multi-select-pill:hover'], ['remove', '.markdown-preview-view .multi-select-pill:hover .multi-select-pill-remove-button']]],
  ['preview.tr.hover', '.markdown-preview-view tbody tr', [['tr', '.markdown-preview-view tbody tr:hover'], ['td', '.markdown-preview-view tbody tr:hover td']]],
];
const HOVER_MENU = [
  ['menu.item.hover', '.menu-item:nth-child(3)', [['item', '.menu-item:hover'], ['title', '.menu-item:hover .menu-item-title'], ['icon', '.menu-item:hover .menu-item-icon'], ['svg', '.menu-item:hover .menu-item-icon svg']]],
];
const HOVER_SETTINGS = [
  ['settings.nav-item.hover', '.vertical-tab-nav-item:not(.is-active)', [['item', '.vertical-tab-nav-item:hover']]],
  ['settings.toggle.hover', '.checkbox-container', [['toggle', '.checkbox-container:hover']]],
  ['settings.button.hover', '.setting-item-control button', [['button', '.setting-item-control button:hover']]],
  ['settings.select.hover', 'select.dropdown', [['select', 'select.dropdown:hover']]],
  ['settings.close.hover', '.modal-close-button', [['close', '.modal-close-button:hover']]],
];

async function openNote(name, mode) {
  await c.evaluate(`
    const f = app.vault.getAbstractFileByPath(${JSON.stringify(name)});
    const leaf = app.workspace.getLeaf(false);
    await leaf.openFile(f, { state: { mode: ${JSON.stringify(mode === 'preview' ? 'preview' : 'source')}, source: ${mode === 'source'} } });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    return true;`);
  await sleep(900);
  return c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); return v ? { mode: v.getMode(), source: v.getState().source, file: v.file && v.file.path } : null;`);
}

async function setTheme(name) {
  await c.evaluate(`app.changeTheme ? app.changeTheme(${JSON.stringify(name)}) : app.setTheme(${JSON.stringify(name)}); return true;`);
  await sleep(900);
  return c.evaluate(`return document.body.className;`);
}

async function closeMenusAndModals() {
  await c.evaluate(`document.querySelectorAll('.menu').forEach((m) => m.remove()); return true;`);
  for (let i = 0; i < 3; i++) { if (!(await c.evaluate(`return !!document.querySelector('.modal-container');`))) break; await c.pressKey('Escape'); await sleep(250); }
  await c.evaluate(`if (app.setting && app.setting.close) app.setting.close(); return true;`);
  await sleep(200);
}

// Put the window in a known state: left dock open on Files with Folder
// expanded, right dock open, Alpha in reading view, Beta in a second tab.
async function arrange() {
  await c.evaluate(`
    app.workspace.leftSplit.expand(); app.workspace.rightSplit.expand();
    const files = app.workspace.getLeavesOfType('file-explorer')[0]; if (files) app.workspace.revealLeaf(files);
    return true;`);
  await sleep(400);
  await openNote('Beta.md', 'preview');
  await c.evaluate(`const leaf = app.workspace.getLeaf('tab'); const f = app.vault.getAbstractFileByPath('Alpha.md'); await leaf.openFile(f, { state: { mode: 'preview' } }); app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`);
  await sleep(1200);
  // Expand the folder row and unfold the callout state is as written.
  await c.evaluate(`const t = Array.from(document.querySelectorAll('.nav-folder:not(.mod-root)')).find((f) => f.classList.contains('is-collapsed')); if (t) t.querySelector('.nav-folder-title').click(); return true;`);
  await sleep(300);
  // Scroll the preview to the top and nudge every section into existence.
  await c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); if (v && v.previewMode) { v.previewMode.applyScroll(0); } return true;`);
  await sleep(500);
  return c.evaluate(LIB + `return { tabs: Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')).map((t) => ({ title: t.getAttribute('aria-label'), active: t.classList.contains('is-active') })), left: !!document.querySelector('.mod-left-split:not(.is-sidedock-collapsed)'), right: !!document.querySelector('.mod-right-split:not(.is-sidedock-collapsed)'), previewSections: document.querySelectorAll('.markdown-preview-section > div').length };`);
}

// One full pass in the current theme.
async function pass(theme) {
  await step(`${theme}-01-vars`, async () => dumpVars(theme));
  await step(`${theme}-02-shell`, async () => ({ surfaces: await describeMany(SHELL) }));
  await step(`${theme}-03-explorer`, async () => ({ surfaces: await describeMany(EXPLORER), html: await c.evaluate(LIB + `return { folder: html('.nav-folder:not(.mod-root)', 1500), file: html('.nav-file', 600), header: html('.nav-header', 1500) };`) }));
  await step(`${theme}-04-reading`, async (r) => {
    r.state = await openNote('Alpha.md', 'preview');
    // Make the reader build every section: scroll through it.
    await c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); if (v && v.previewMode) { for (let y = 0; y < 60; y += 10) { v.previewMode.applyScroll(y); await new Promise((r) => setTimeout(r, 60)); } v.previewMode.applyScroll(0); } return true;`);
    await sleep(500);
    r.surfaces = await describeMany(READING);
    return r;
  });
  await step(`${theme}-05-reading-hover`, async () => ({ hover: await hovers(HOVER_READING) }));
  await step(`${theme}-06-live-preview`, async (r) => {
    r.state = await openNote('Alpha.md', 'live');
    await c.clickElement('.cm-content');
    await sleep(300);
    r.surfaces = await describeManyScrolling(EDITOR, [0, 11, 20, 28, 36, 44, 50, 56]);
    // The selection wash, with a real selection.
    await c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); if (v && v.editor) v.editor.setSelection({ line: 11, ch: 2 }, { line: 11, ch: 12 }); return true;`);
    await sleep(300);
    r.selection = await describeMany([['cm.selection', '.cm-selectionBackground'], ['cm.selectionLayer', '.cm-selectionLayer'], ['cm.selectionMatch', '.cm-selectionMatch']]);
    return r;
  });
  await step(`${theme}-07-source`, async (r) => {
    r.state = await openNote('Alpha.md', 'source');
    await c.clickElement('.cm-content');
    await sleep(300);
    r.surfaces = await describeManyScrolling(SOURCE, [0, 11, 28, 36, 44, 50]);
    return r;
  });
  await step(`${theme}-08-shell-hover`, async (r) => {
    r.state = await openNote('Alpha.md', 'preview');
    r.hover = await hovers(HOVER_SHELL);
    return r;
  });
  await step(`${theme}-09-panes`, async (r) => {
    // Search for a word so the search pane has rows, then read every right-dock pane.
    await c.evaluate(`app.commands.executeCommandById('global-search:open'); return true;`);
    await sleep(500);
    await c.evaluate(`const l = app.workspace.getLeavesOfType('search')[0]; if (l) { l.view.setQuery('tag1'); } return true;`);
    await sleep(1200);
    await c.evaluate(`document.querySelectorAll('.suggestion-container').forEach((e) => e.remove()); const a = document.activeElement; if (a && a.blur) a.blur(); return true;`);
    await sleep(200);
    r.search = await describeMany(PANES.filter(([k]) => k.startsWith('pane.search')));
    r.searchHover = await hovers([['pane.search.match.hover', '.search-result-file-match', [['match', '.search-result-file-match:hover'], ['button', '.search-result-file-match:hover .search-result-hover-button']]], ['pane.search.title.hover', '.search-result-file-title', [['title', '.search-result-file-title:hover']]]]);
    for (const [type, keys] of [['tag', 'pane.tags'], ['outline', 'pane.outline'], ['backlink', 'pane.backlinks']]) {
      await c.evaluate(`const l = app.workspace.getLeavesOfType(${JSON.stringify(type)})[0]; if (l) app.workspace.revealLeaf(l); return true;`);
      await sleep(600);
      r[type] = await describeMany(PANES.filter(([k]) => k.startsWith(keys)));
      r[type + 'Html'] = await c.evaluate(LIB + `const l = app.workspace.getLeavesOfType(${JSON.stringify(type)})[0]; return l ? l.view.containerEl.outerHTML.slice(0, 2500) : null;`);
    }
    // Outline rows need an active markdown view; backlinks needs the file explorer not to steal.
    await c.evaluate(`const l = app.workspace.getLeavesOfType('tag')[0]; if (l) app.workspace.revealLeaf(l); return true;`);
    await sleep(400);
    r.tagHover = await hovers([['pane.tags.hover', '.tag-pane-tag .tree-item-self', [['self', '.tag-pane-tag .tree-item-self:hover'], ['inner', '.tag-pane-tag .tree-item-self:hover .tree-item-inner'], ['count', '.tag-pane-tag .tree-item-self:hover .tag-pane-tag-count']]]]);
    await c.evaluate(`const l = app.workspace.getLeavesOfType('file-explorer')[0]; if (l) app.workspace.revealLeaf(l); return true;`);
    return r;
  });
  await step(`${theme}-10-menu`, async (r) => {
    await c.rightClickElement('.nav-file-title:not(.is-active)');
    await sleep(400);
    r.items = await c.openMenuItems();
    await c.pressKey('ArrowDown'); await sleep(150);
    r.surfaces = await describeMany(MENU);
    r.html = await c.evaluate(LIB + `return html('.menu', 3000);`);
    r.hover = await hovers(HOVER_MENU);
    await closeMenusAndModals();
    return r;
  });
  await step(`${theme}-11-palette`, async (r) => {
    await c.evaluate(`app.commands.executeCommandById('command-palette:open'); return true;`);
    await sleep(500);
    await c.typeText('tog');
    await sleep(400);
    r.surfaces = await describeMany(PROMPT);
    r.hover = await hovers([['prompt.item.hover', '.suggestion-item:nth-child(3)', [['item', '.suggestion-item:hover'], ['title', '.suggestion-item:hover .suggestion-title']]]]);
    await closeMenusAndModals();
    return r;
  });
  await step(`${theme}-12-switcher`, async (r) => {
    await c.evaluate(`app.commands.executeCommandById('switcher:open'); return true;`);
    await sleep(500);
    await c.typeText('al');
    await sleep(400);
    r.surfaces = await describeMany(PROMPT);
    await closeMenusAndModals();
    return r;
  });
  await step(`${theme}-13-settings`, async (r) => {
    await c.evaluate(`app.setting.open(); app.setting.openTabById('editor'); return true;`);
    await sleep(900);
    // 1.14 opens settings in its own window on some platforms: read from wherever the tab's container lives.
    const SDOC = `(app.setting && app.setting.activeTab && app.setting.activeTab.containerEl.ownerDocument) || document`;
    r.where = await c.evaluate(`const t = app.setting.activeTab; const d = ${SDOC}; return { inThisDocument: d === document, bodyClass: d.body.className, sheets: Array.from(d.styleSheets).map((s) => (s.href || 'inline') + ':' + (() => { try { return s.cssRules.length; } catch { return '?'; } })()), modal: !!d.querySelector('.modal.mod-settings'), rootClasses: Array.from(d.querySelectorAll('body > *')).map((e) => e.className).slice(0, 8) };`);
    r.surfaces = await describeMany(SETTINGS, SDOC);
    r.html = await c.evaluate(`const __DOC = ${SDOC};` + LIB + `return { nav: html('.vertical-tab-header', 2500), item: html('.setting-item:not(.setting-item-heading)', 1500), toggle: html('.checkbox-container', 400) };`);
    await c.evaluate(`app.setting.openTabById('hotkeys'); return true;`);
    await sleep(900);
    r.hotkeys = await describeMany(SETTINGS.filter(([k]) => k.startsWith('settings.hotkey') || k === 'settings.command-name' || k === 'settings.search-input'), SDOC);
    await c.evaluate(`app.setting.openTabById('appearance'); return true;`);
    await sleep(900);
    r.appearance = await describeMany([['settings.color-input', 'input[type="color"]'], ['settings.slider', 'input.slider'], ['settings.theme-select', 'select.dropdown'], ['settings.description-link', '.setting-item-description a'], ['settings.accent', '.setting-item-control .accent-color-picker, .setting-item-control input[type="color"]']], SDOC);
    // The popout's own custom-property table, in case it differs from the vault window's.
    r.varsSample = await c.evaluate(`const d = ${SDOC}; const cs = d.defaultView.getComputedStyle(d.body); const o = {}; for (const n of ['--background-primary', '--background-secondary', '--text-normal', '--text-muted', '--interactive-accent', '--modal-background', '--setting-item-name-color']) o[n] = cs.getPropertyValue(n).trim(); return o;`);
    await closeMenusAndModals();
    return r;
  });
  await step(`${theme}-14-misc`, async (r) => {
    // The plugin API module is not reachable from the console, so the notice is
    // the real markup built by hand: computed styles follow the classes.
    await c.evaluate(`let nc = document.querySelector('.notice-container'); if (!nc) { nc = document.createElement('div'); nc.className = 'notice-container'; document.body.appendChild(nc); } const n = document.createElement('div'); n.className = 'notice'; n.textContent = 'A notice for the probe'; nc.appendChild(n); setTimeout(() => n.remove(), 4000); return true;`);
    await sleep(300);
    r.notice = await describeMany(MISC.filter(([k]) => k.startsWith('notice')));
    // A tooltip: hover a ribbon action and wait.
    await c.hover('.side-dock-ribbon-action');
    await sleep(1200);
    r.tooltip = await describeMany(MISC.filter(([k]) => k.startsWith('tooltip')));
    await c.mouseMove(2, 400);
    await sleep(300);
    // The empty tab.
    await c.evaluate(`const leaf = app.workspace.getLeaf('tab'); app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`);
    await sleep(600);
    r.empty = await describeMany(MISC.filter(([k]) => k.startsWith('empty-state')));
    r.emptyTab = await describeMany([['root.tab-header.active.empty', '.mod-root .workspace-tab-header.is-active'], ['view-header.empty', '.mod-root .workspace-leaf.mod-active .view-header'], ['view-content.empty', '.mod-root .workspace-leaf.mod-active .view-content']]);
    await c.evaluate(`app.workspace.activeLeaf.detach(); return true;`);
    await sleep(400);
    // The autocompletion popup: type [[ in Alpha.
    await openNote('Alpha.md', 'live');
    await c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); v.editor.setCursor({ line: 13, ch: 0 }); v.editor.focus(); return true;`);
    await sleep(300);
    await c.typeText('[[B');
    await sleep(700);
    r.suggest = await describeMany(MISC.filter(([k]) => k.startsWith('suggestion-container')));
    await c.pressKey('Escape');
    await sleep(200);
    await c.evaluate(`const v = (app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() === 'markdown' ? app.workspace.activeLeaf.view : null); const l = v.editor.getLine(13); if (l.startsWith('[[B')) v.editor.setLine(13, l.slice(3)); return true;`);
    // A hover popover over a link in the reader.
    await openNote('Alpha.md', 'preview');
    await c.evaluate(`app.vault.setConfig && null; return true;`);
    r.popoverTry = await (async () => { const rr = await c.rect('.markdown-preview-view a.internal-link:not(.is-unresolved)'); if (!rr) return null; await c.send && null; await c.mouseMove(rr.cx, rr.cy); await sleep(1500); const o = await describeMany(MISC.filter(([k]) => k.startsWith('popover'))); await c.mouseMove(2, 400); await sleep(300); return o; })();
    return r;
  });
}

await step('s00-arrange', async () => arrange());
await step('s00-theme-dark', async () => setTheme('obsidian'));
await pass('dark');
await step('s50-theme-light', async () => setTheme('moonstone'));
await pass('light');
await step('s99-theme-back', async () => setTheme('obsidian'));

save();
await c.quit();
process.exit(0);
