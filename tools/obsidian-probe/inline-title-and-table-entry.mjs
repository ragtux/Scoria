// Two gaps found in Scoria on 2026-11-08: (1) the note's inline title above the
// text is painted but not a field, and (2) arrowing down into a Live Preview
// table leaves the caret invisible. This probe records what Obsidian does.
//
//  - Table entry: with the caret on the line above a table, then the line
//    below one, which cell does each arrow key put it in, and what is the DOM
//    selection (is it inside the table widget)?
//  - Inline title: its markup and resting state, what hover paints, what a
//    click, a double click and Select All do, where ArrowDown goes from it,
//    whether ArrowUp from the first line enters it, and what Enter, Escape
//    and typing do, including whether Enter renames the file.
//
// Keys go through Input.dispatchKeyEvent and clicks through
// Input.dispatchMouseEvent. Generic content only.
//
// Phases: fixtures | run   (node inline-title-and-table-entry.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 420000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.titleAndTable = all.titleAndTable || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.titleAndTable[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
const ONLY = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now(); const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save(); return out.steps[name];
}
const vp = (rel) => path.join(VAULT, rel);

const BODY = 'intro line here\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\nafter line here\n';
const NOTES = { 'Probe Note.md': BODY, 'Other.md': 'other\n' };

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  for (const [n, t] of Object.entries(NOTES)) fs.writeFileSync(vp(n), t);
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'command-palette': true, switcher: true }, null, 2));
  save(); process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
save();

await c.evaluate(`
window.__t = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  leaf() { const a = app.workspace.activeLeaf; if (a && a.view && a.view.getViewType() === 'markdown') return a; return app.workspace.getLeavesOfType('markdown')[0] || app.workspace.getLeaf(false); },
  view() { return this.leaf().view; },
  ed() { return this.view().editor; },
  async open(file, mode) {
    const leaf = this.leaf();
    await leaf.openFile(app.vault.getAbstractFileByPath(file));
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(400);
    const st = leaf.getViewState();
    st.state = Object.assign({}, st.state, { mode: mode === 'reading' ? 'preview' : 'source', source: mode === 'source' });
    await leaf.setViewState(st);
    await this.sleep(500);
    return file;
  },
  async reseed(file, text, mode) {
    await this.open(file, mode || 'live');
    await app.vault.modify(this.view().file, text);
    await this.sleep(600);
    const ed = this.ed();
    if (ed.getValue() !== text) { ed.setValue(text); await this.sleep(200); }
    ed.focus(); ed.setCursor({ line: 0, ch: 0 });
    await this.sleep(300);
    return ed.getValue() === text;
  },
  describe(el) {
    if (!el) return null;
    const cls = (el.className && el.className.baseVal !== undefined ? el.className.baseVal : el.className) || '';
    return { tag: el.tagName, cls: String(cls).slice(0, 90), editable: el.isContentEditable,
      inTable: !!el.closest('table.table-editor'), inEditor: !!el.closest('.cm-editor'),
      inTitle: !!el.closest('.inline-title'), text: (el.textContent || '').slice(0, 30) };
  },
  sel() {
    const s = window.getSelection();
    const n = s && s.anchorNode; const el = n && (n.nodeType === 1 ? n : n.parentElement);
    const td = el && el.closest && el.closest('td, th');
    return { text: s ? s.toString() : null, collapsed: s ? s.isCollapsed : null, anchorOffset: s ? s.anchorOffset : null,
      anchorIn: el ? { tag: el.tagName, inTable: !!el.closest('table.table-editor'), inTitle: !!el.closest('.inline-title'),
        inEditor: !!el.closest('.cm-editor'), cell: td ? td.textContent : null } : null };
  },
  snap() {
    const ed = this.ed();
    const it = document.querySelector('.markdown-source-view .inline-title');
    return { cursor: ed.getCursor(), value: ed.getValue(), active: this.describe(document.activeElement), sel: this.sel(),
      title: it ? { text: it.textContent, editable: it.isContentEditable } : null,
      file: this.view().file && this.view().file.path };
  },
};
return true;`);

const snap = () => c.evaluate(`return window.__t.snap();`);
const reseed = (f, t, m) => c.evaluate(`return await window.__t.reseed(${JSON.stringify(f)}, ${JSON.stringify(t)}, ${JSON.stringify(m || 'live')});`);
const setCursor = (line, ch) => c.evaluate(`const ed = window.__t.ed(); ed.focus(); ed.setCursor({ line: ${line}, ch: ${ch} }); await window.__t.sleep(250); return true;`);

async function keys(list) {
  const rows = [];
  for (const k of list) {
    if (typeof k === 'string') await c.typeChar(k); else await c.pressKey(k.key, k.mods || {});
    await sleep(300);
    const s = await snap();
    s.key = typeof k === 'string' ? JSON.stringify(k) : k.key + (k.mods ? '+' + Object.keys(k.mods).join('+') : '');
    rows.push(s);
  }
  return rows;
}
const down = (n) => Array.from({ length: n }, () => ({ key: 'ArrowDown' }));
const up = (n) => Array.from({ length: n }, () => ({ key: 'ArrowUp' }));

await step('s00-context', async (r) => {
  await c.evaluate(`app.workspace.leftSplit.collapse(); return true;`);
  r.seedOk = await reseed('Probe Note.md', BODY);
  r.snapshot = await snap();
  r.config = await c.evaluate(`return { showInlineTitle: app.vault.getConfig('showInlineTitle') };`);
  return r;
});

// ---- (2) entering a table by arrow keys ------------------------------------
await step('s01-arrow-down-into-table-from-column-0', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  await setCursor(0, 0);
  r.start = await snap();
  r.after = await keys(down(6));
  return r;
});
await step('s02-arrow-down-into-table-from-column-8', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  await setCursor(0, 8);
  r.start = await snap();
  r.after = await keys(down(4));
  return r;
});
await step('s03-arrow-up-into-table-from-below', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  await setCursor(7, 3);
  r.start = await snap();
  r.after = await keys(up(5));
  return r;
});
await step('s04-typing-with-caret-in-table-cell-after-arrows', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  await setCursor(0, 0);
  const before = await keys(down(4));
  r.afterArrows = before[before.length - 1];
  r.typed = await keys(['Z']);
  return r;
});

// ---- (1) the inline title ---------------------------------------------------
await step('s10-inline-title-markup', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  r.styles = await c.evaluate(`
    const el = document.querySelector('.markdown-source-view .inline-title'); if (!el) return null;
    const cs = getComputedStyle(el);
    return { cls: el.className, tag: el.tagName, editable: el.isContentEditable, contenteditable: el.getAttribute('contenteditable'),
      tabindex: el.getAttribute('tabindex'), spellcheck: el.getAttribute('spellcheck'), enterkeyhint: el.getAttribute('enterkeyhint'),
      text: el.textContent, cursor: cs.cursor, bg: cs.backgroundColor, color: cs.color, fontSize: cs.fontSize,
      fontWeight: cs.fontWeight, outline: cs.outline, userSelect: cs.userSelect, parent: el.parentElement && el.parentElement.className,
      rect: (() => { const b = el.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; })() };`);
  r.readingStyles = await c.evaluate(`
    await window.__t.open('Probe Note.md', 'reading'); await window.__t.sleep(400);
    const el = document.querySelector('.markdown-reading-view .inline-title, .markdown-preview-view .inline-title'); if (!el) return null;
    const cs = getComputedStyle(el);
    return { cls: el.className, editable: el.isContentEditable, cursor: cs.cursor, text: el.textContent };`);
  await reseed('Probe Note.md', BODY);
  return r;
});
await step('s11-hover-and-click', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  const sel = '.markdown-source-view .inline-title';
  const base = await c.evaluate(`const el = document.querySelector(${JSON.stringify(sel)}); const cs = getComputedStyle(el); return { bg: cs.backgroundColor, outline: cs.outline, boxShadow: cs.boxShadow, cursor: cs.cursor };`);
  r.restingStyle = base;
  r.hoverRect = await c.hover(sel);
  await sleep(300);
  r.hoverStyle = await c.evaluate(`const el = document.querySelector(${JSON.stringify(sel)}); const cs = getComputedStyle(el); return { bg: cs.backgroundColor, outline: cs.outline, boxShadow: cs.boxShadow, cursor: cs.cursor };`);
  r.click = await c.clickElement(sel);
  await sleep(350);
  r.afterClick = await snap();
  r.afterClickStyle = await c.evaluate(`const el = document.querySelector(${JSON.stringify(sel)}); const cs = getComputedStyle(el); return { bg: cs.backgroundColor, outline: cs.outline, boxShadow: cs.boxShadow };`);
  return r;
});
await step('s12-double-click-and-select-all', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  const sel = '.markdown-source-view .inline-title';
  await c.clickElement(sel, { clickCount: 2 }); await sleep(350);
  r.afterDouble = await snap();
  await c.clickElement(sel); await sleep(250);
  r.selectAll = (await keys([{ key: 'a', mods: { ctrl: true } }]))[0];
  return r;
});
await step('s13-arrow-keys-in-title', async (r) => {
  const sel = '.markdown-source-view .inline-title';
  r.seedOk = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.down = await keys([{ key: 'ArrowDown' }, { key: 'ArrowDown' }]);
  r.seedOk2 = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.endDown = await keys([{ key: 'End' }, { key: 'ArrowDown' }]);
  r.seedOk3 = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.rightAtEnd = await keys([{ key: 'End' }, { key: 'ArrowRight' }]);
  r.seedOk4 = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.tab = await keys([{ key: 'Tab' }]);
  r.seedOk5 = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.enter = await keys([{ key: 'Enter' }]);
  return r;
});
await step('s14-arrow-up-from-first-line', async (r) => {
  r.seedOk = await reseed('Probe Note.md', BODY);
  await setCursor(0, 0);
  r.fromCol0 = await keys([{ key: 'ArrowUp' }]);
  r.seedOk2 = await reseed('Probe Note.md', BODY);
  await setCursor(0, 8);
  r.fromCol8 = await keys([{ key: 'ArrowUp' }]);
  r.then = await keys([{ key: 'ArrowDown' }]);
  return r;
});
await step('s15-typing-and-escape-and-enter-rename', async (r) => {
  const sel = '.markdown-source-view .inline-title';
  r.seedOk = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.typed = await keys(['X', { key: 'Escape' }]);
  r.files1 = await c.evaluate(`return app.vault.getMarkdownFiles().map((f) => f.path).sort();`);
  r.seedOk2 = await reseed('Probe Note.md', BODY);
  await c.clickElement(sel); await sleep(300);
  r.typed2 = await keys(['Y', { key: 'Enter' }]);
  await sleep(800);
  r.files2 = await c.evaluate(`return app.vault.getMarkdownFiles().map((f) => f.path).sort();`);
  r.afterEnterSnap = await snap();
  return r;
});

save();
await c.quit?.();
