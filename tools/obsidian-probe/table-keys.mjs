// Table cell keyboard in Live Preview: what Enter, Escape, the arrows, Tab and
// Shift+Enter do from a cell, above all from the LAST cell of the last row, and
// where the caret and focus are afterwards. [[Specification#Observed table
// behavior]] records the table commands and the widget's markup, and says in
// so many words that the keyboard from inside a cell is unprobed; Scoria's
// cell field commits on Enter in the last row and nothing else, so a person
// cannot leave a table that ends the note.
//
// Keys go through Input.dispatchKeyEvent and clicks through
// Input.dispatchMouseEvent, so they carry focus and take a person's path.
// The note is seeded through the vault with a settle before any click, as the
// other typing probes do.
//
// Phases: fixtures | run   (node table-keys.mjs <phase>)
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
all.tableKeys = all.tableKeys || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.tableKeys[phase] = out;
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

// Generic content only: a two-column table, then (or not) a paragraph after it.
const TABLE = '| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n';
const NOTES = {
  'AtEnd.md': 'intro\n\n' + TABLE,                  // the table is the last thing in the file
  'AtEndBlank.md': 'intro\n\n' + TABLE + '\n',      // one blank line after it
  'Followed.md': 'intro\n\n' + TABLE + '\nafter\n', // text after it
};

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  for (const [name, text] of Object.entries(NOTES)) fs.writeFileSync(vp(name), text);
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
  async open(file) {
    const leaf = this.leaf();
    await leaf.openFile(app.vault.getAbstractFileByPath(file));
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(400);
    const st = leaf.getViewState();
    st.state = Object.assign({}, st.state, { mode: 'source', source: false });
    await leaf.setViewState(st);
    await this.sleep(500);
    return file;
  },
  // Reset the note to its fixture through the vault, let the file event land,
  // then focus the editor with the caret at the top (a file event landing
  // after a click moves the caret under the keystroke).
  async reseed(file, text) {
    await this.open(file);
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
    return {
      tag: el.tagName, cls: String(cls).slice(0, 80), editable: el.isContentEditable,
      inTable: !!el.closest('table.table-editor'), inEditor: !!el.closest('.cm-editor'),
      text: (el.textContent || '').slice(0, 30),
    };
  },
  snap() {
    const ed = this.ed();
    const cells = Array.from(document.querySelectorAll('.markdown-source-view table.table-editor tbody tr'))
      .map((tr) => Array.from(tr.querySelectorAll('td')).map((td) => td.textContent));
    return {
      value: ed.getValue(), cursor: ed.getCursor(), sel: ed.getSelection(), lines: ed.lineCount(),
      active: this.describe(document.activeElement),
      widgetRows: cells.length, widgetCells: cells,
      widgetPresent: !!document.querySelector('.markdown-source-view table.table-editor'),
    };
  },
};
return true;`);

const snap = () => c.evaluate(`return window.__t.snap();`);
const reseed = (file, text) => c.evaluate(`return await window.__t.reseed(${JSON.stringify(file)}, ${JSON.stringify(text)});`);

// Click one cell of the live table. row counts body rows from 1.
async function clickCell(row, col) {
  const sel = `.markdown-source-view table.table-editor tbody tr:nth-child(${row}) td:nth-child(${col})`;
  const r = await c.rect(sel);
  if (!r) return { missing: sel };
  await c.clickAt(r.cx, r.cy);
  await sleep(350);
  return { clicked: sel, rect: r };
}

// A key is {key, mods?} or a single character.
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
// Reseed, click a cell, then press the keys; the answer is every snapshot.
async function trial(file, row, col, list) {
  const seedOk = await reseed(file, NOTES[file]);
  const click = await clickCell(row, col);
  const before = await snap();
  const after = await keys(list);
  return { file, cell: [row, col], seedOk, click, before, after };
}

await step('s00-context', async (r) => {
  r.ua = await c.evaluate(`return navigator.userAgent;`);
  r.version = await c.evaluate(`return (app.appVersion || (typeof electron !== 'undefined' && electron.remote && electron.remote.app.getVersion()) || null);`);
  await c.evaluate(`app.workspace.leftSplit.collapse(); return true;`);
  r.seedOk = await reseed('AtEnd.md', NOTES['AtEnd.md']);
  r.snapshot = await snap();
  // What does one click on the last row's last cell do to focus and caret?
  r.clickLast = await clickCell(2, 2);
  r.afterClick = await snap();
  return r;
});

// The user's case: the table is the last thing in the file.
await step('s01-enter-last-cell-table-at-end', async (r) => {
  r.enterOnce = await trial('AtEnd.md', 2, 2, [{ key: 'Enter' }]);
  r.enterTwice = await trial('AtEnd.md', 2, 2, [{ key: 'Enter' }, { key: 'Enter' }]);
  r.enterThrice = await trial('AtEnd.md', 2, 2, [{ key: 'Enter' }, { key: 'Enter' }, { key: 'Enter' }]);
  return r;
});

await step('s02-enter-last-cell-other-endings', async (r) => {
  r.blankAfter = await trial('AtEndBlank.md', 2, 2, [{ key: 'Enter' }, { key: 'Enter' }]);
  r.textAfter = await trial('Followed.md', 2, 2, [{ key: 'Enter' }, { key: 'Enter' }]);
  return r;
});

await step('s03-enter-middle-and-first-row', async (r) => {
  r.firstRowFirstCol = await trial('AtEnd.md', 1, 1, [{ key: 'Enter' }]);
  r.firstRowSecondCol = await trial('AtEnd.md', 1, 2, [{ key: 'Enter' }]);
  r.lastRowFirstCol = await trial('AtEnd.md', 2, 1, [{ key: 'Enter' }]);
  return r;
});

await step('s04-escape-and-arrows-from-last-cell', async (r) => {
  r.escape = await trial('AtEnd.md', 2, 2, [{ key: 'Escape' }]);
  r.arrowDown = await trial('AtEnd.md', 2, 2, [{ key: 'ArrowDown' }]);
  r.arrowDownTwice = await trial('AtEnd.md', 2, 2, [{ key: 'ArrowDown' }, { key: 'ArrowDown' }]);
  r.arrowDownFollowed = await trial('Followed.md', 2, 2, [{ key: 'ArrowDown' }]);
  r.arrowRight = await trial('AtEnd.md', 2, 2, [{ key: 'ArrowRight' }]);
  r.arrowUpFirstRow = await trial('AtEnd.md', 1, 1, [{ key: 'ArrowUp' }]);
  r.arrowUpFirstRowFollowed = await trial('Followed.md', 1, 1, [{ key: 'ArrowUp' }]);
  return r;
});

await step('s05-tab-and-shift-keys', async (r) => {
  r.tabLastCell = await trial('AtEnd.md', 2, 2, [{ key: 'Tab' }]);
  r.tabLastCellTwice = await trial('AtEnd.md', 2, 2, [{ key: 'Tab' }, { key: 'Tab' }]);
  r.tabFirstCell = await trial('AtEnd.md', 1, 1, [{ key: 'Tab' }]);
  r.shiftTabFirstCell = await trial('AtEnd.md', 1, 1, [{ key: 'Tab', mods: { shift: true } }]);
  r.shiftEnter = await trial('AtEnd.md', 2, 2, [{ key: 'Enter', mods: { shift: true } }]);
  r.ctrlEnter = await trial('AtEnd.md', 2, 2, [{ key: 'Enter', mods: { ctrl: true } }]);
  return r;
});

// The other half of the report: the field stays open and eats the typing.
await step('s06-click-cell-then-click-away', async (r) => {
  const seedOk = await reseed('Followed.md', NOTES['Followed.md']);
  r.seedOk = seedOk;
  r.clickCell = await clickCell(1, 1);
  r.inCell = await snap();
  // Click the paragraph under the table.
  const p = await c.rect('.markdown-source-view .cm-line:not(.cm-embed-block)', 5);
  r.afterTextRect = p;
  const lines = await c.evaluate(`return Array.from(document.querySelectorAll('.markdown-source-view .cm-line')).map((l) => l.textContent);`);
  r.cmLines = lines;
  const target = await c.evaluate(`
    const ls = Array.from(document.querySelectorAll('.markdown-source-view .cm-line'));
    const l = ls.find((x) => x.textContent === 'after'); if (!l) return null;
    const b = l.getBoundingClientRect(); return { x: b.x + 6, y: b.y + b.height / 2 };`);
  r.target = target;
  if (target) { await c.clickAt(target.x, target.y); await sleep(350); }
  r.afterClickAway = await snap();
  // Does typing land in the cell, or in the paragraph?
  r.typed = await keys(['Z']);
  return r;
});

await step('s07-type-in-cell-then-leave', async (r) => {
  // Type into a cell, then leave by Tab, Enter and Escape; does the typed text commit?
  r.typeThenEscape = await trial('AtEnd.md', 1, 1, ['Z', { key: 'Escape' }]);
  r.typeThenEnter = await trial('AtEnd.md', 1, 1, ['Z', { key: 'Enter' }]);
  r.typeThenTab = await trial('AtEnd.md', 1, 1, ['Z', { key: 'Tab' }]);
  return r;
});

await step('s08-typing-after-leaving', async (r) => {
  // After each way out of the last cell, does a typed character land in the note, and where?
  r.afterEscape = await trial('AtEnd.md', 2, 2, [{ key: 'Escape' }, 'Q']);
  r.afterEnter = await trial('AtEnd.md', 2, 2, [{ key: 'Enter' }, 'Q']);
  r.afterArrowDown = await trial('AtEnd.md', 2, 2, [{ key: 'ArrowDown' }, 'Q']);
  return r;
});

save();
await c.quit?.();
