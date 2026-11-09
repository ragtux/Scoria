// The Live Preview table's row and column handles: the grips beside a focused
// cell that open the row/column menu and drag to reorder, and the "+" buttons
// that add a row or a column. [[Specification#Observed table behavior]] records
// the widget's markup and the table commands but not these.
//
// Records, with a cell focused and with none: every element in the widget that
// is not a cell (class, rect, computed style, icon), the widget's markup,
// hover, a click on each handle and the menu it opens, a click on each "+",
// and real drags of a row handle and a column handle, reading the note back.
// Screenshots go to SHOTS when it is set.
//
// Phases: fixtures | run   (node table-handles.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || '';
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 420000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.tableHandles = all.tableHandles || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.tableHandles[phase] = out;
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

// Three columns and three body rows, so a move has somewhere to go both ways.
const BODY = 'intro\n\n| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n| 7 | 8 | 9 |\n\nafter\n';
const NOTES = { 'Handles.md': BODY };

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  for (const [n, t] of Object.entries(NOTES)) fs.writeFileSync(vp(n), t);
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'command-palette': true, switcher: true }, null, 2));
  if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true });
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
  },
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
  // Everything in the table widget that is not a plain cell, with its box.
  widget() {
    const root = document.querySelector('.markdown-source-view .cm-table-widget, .markdown-source-view .table-wrapper, .markdown-source-view .table-editor');
    if (!root) return null;
    const wrap = root.closest('.cm-embed-block') || root.closest('.cm-table-widget') || root;
    const table = wrap.querySelector('table.table-editor') || wrap.querySelector('table');
    const rect = (el) => { const b = el.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; };
    const cells = Array.from(wrap.querySelectorAll('td, th'));
    const cellSet = new Set(cells);
    const els = Array.from(wrap.querySelectorAll('*')).filter((e) => !cellSet.has(e) && !['TABLE', 'THEAD', 'TBODY', 'TR', 'TD', 'TH', 'COLGROUP', 'COL'].includes(e.tagName) && !e.closest('td, th'));
    return {
      wrapClass: wrap.className, tableRect: table ? rect(table) : null,
      extras: els.map((e) => {
        const cs = getComputedStyle(e);
        return { tag: e.tagName, cls: String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className).slice(0, 120),
          label: e.getAttribute('aria-label'), icon: (e.querySelector('svg') && String(e.querySelector('svg').getAttribute('class'))) || null,
          rect: rect(e), display: cs.display, visibility: cs.visibility, opacity: cs.opacity, cursor: cs.cursor,
          bg: cs.backgroundColor, color: cs.color, radius: cs.borderRadius, parent: e.parentElement && String(e.parentElement.className).slice(0, 60) };
      }),
      html: wrap.outerHTML.slice(0, 6000),
      cellRects: cells.map((e) => ({ text: e.textContent, tag: e.tagName, ...rect(e) })),
    };
  },
  handles() {
    const rect = (el) => { const b = el.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; };
    const desc = (kind) => Array.from(document.querySelectorAll('.markdown-source-view .table-' + kind + '-drag-handle')).map((h) => {
      const cs = getComputedStyle(h); const cell = h.closest('td, th');
      const svg = h.querySelector('svg');
      return { kind, cell: cell ? cell.textContent : null, cellTag: cell ? cell.tagName : null, cellClass: cell ? cell.className : null,
        rect: rect(h), opacity: cs.opacity, visibility: cs.visibility, display: cs.display, cursor: cs.cursor, bg: cs.backgroundColor,
        color: cs.color, radius: cs.borderRadius, pos: cs.position, icon: svg ? svg.getAttribute('class') : null, svgRect: svg ? rect(svg) : null,
        borderColor: cs.borderColor, borderWidth: cs.borderWidth, zIndex: cs.zIndex };
    });
    const wrap = document.querySelector('.markdown-source-view .cm-table-widget');
    return { widgetClass: wrap ? wrap.className : null, cols: desc('col'), rows: desc('row') };
  },
  snap() { const ed = this.ed(); return { value: ed.getValue(), cursor: ed.getCursor() }; },
  menu() {
    const m = document.querySelector('.menu');
    if (!m) return null;
    return { items: Array.from(m.querySelectorAll('.menu-item')).map((i) => ({ title: (i.querySelector('.menu-item-title') || {}).textContent, icon: (i.querySelector('svg') && String(i.querySelector('svg').getAttribute('class'))) || null, disabled: i.classList.contains('is-disabled'), warning: i.classList.contains('is-warning') })),
      separators: m.querySelectorAll('.menu-separator').length, rect: (() => { const b = m.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; })() };
  },
};
return true;`);

const widget = () => c.evaluate(`return window.__t.widget();`);
const snap = () => c.evaluate(`return window.__t.snap();`);
const menu = () => c.evaluate(`return window.__t.menu();`);
const reseed = () => c.evaluate(`return await window.__t.reseed('Handles.md', ${JSON.stringify(BODY)});`);
async function shot(name) {
  if (!SHOTS) return null;
  const r = await c.send('Page.captureScreenshot', { format: 'png' });
  const file = path.join(SHOTS, name + '.png');
  fs.writeFileSync(file, Buffer.from(r.data, 'base64'));
  return file;
}
const cellSel = (row, col) => `.markdown-source-view table.table-editor tbody tr:nth-child(${row}) td:nth-child(${col})`;
async function clickCell(row, col) {
  const r = await c.rect(cellSel(row, col));
  if (!r) return { missing: cellSel(row, col) };
  await c.clickAt(r.cx, r.cy);
  await sleep(450);
  return r;
}
const lines = (v) => v.split('\n');

await step('s00-no-cell-focused', async (r) => {
  await c.evaluate(`app.workspace.leftSplit.collapse(); return true;`);
  r.seedOk = await reseed();
  await sleep(300);
  r.widget = await widget();
  r.shot = await shot('01-no-focus');
  return r;
});

await step('s01-cell-focused', async (r) => {
  r.seedOk = await reseed();
  r.click = await clickCell(2, 2);
  r.widget = await widget();
  r.shot = await shot('02-focused-middle');
  r.firstCell = null;
  return r;
});

await step('s02-other-cells-and-edges', async (r) => {
  r.seedOk = await reseed();
  await clickCell(1, 1);
  r.firstBody = await widget(); r.shot1 = await shot('03-focused-first');
  r.seedOk2 = await reseed();
  await clickCell(3, 3);
  r.lastBody = await widget(); r.shot2 = await shot('04-focused-last');
  return r;
});

const mouse = (type, x, y, extra = {}) => c.send('Input.dispatchMouseEvent', { type, x: Math.round(x), y: Math.round(y), ...extra });
const handles = () => c.evaluate(`return window.__t.handles();`);
const pick = (h, kind, cell) => (h[kind === 'col' ? 'cols' : 'rows']).find((x) => x.cell === cell);

await step('s03-handle-geometry-and-state', async (r) => {
  r.seedOk = await reseed();
  r.unfocused = await handles();
  await clickCell(2, 2);               // the cell "5"
  r.focusedOn5 = await handles();
  r.shot = await shot('05-focused-on-5');
  r.seedOk2 = await reseed();
  await clickCell(1, 1);               // the cell "1"
  r.focusedOn1 = await handles();
  r.seedOk3 = await reseed();
  await clickCell(3, 3);               // the cell "9"
  r.focusedOn9 = await handles();
  return r;
});

await step('s04-hover-each-handle', async (r) => {
  r.seedOk = await reseed();
  await clickCell(2, 2);
  const h = await handles();
  const col = pick(h, 'col', 'B'), row = pick(h, 'row', '4');
  r.colB = col; r.row4 = row;
  r.hover = {};
  for (const [name, hd] of [['colB', col], ['row4', row]]) {
    if (!hd) continue;
    await c.mouseMove(hd.rect.x + hd.rect.w / 2, hd.rect.y + hd.rect.h / 2);
    await sleep(350);
    const hh = await handles();
    r.hover[name] = pick(hh, name === 'colB' ? 'col' : 'row', name === 'colB' ? 'B' : '4');
    r.hover[name + 'Shot'] = await shot('06-hover-' + name);
  }
  return r;
});

await step('s05-click-each-handle', async (r) => {
  r.clicks = {};
  for (const [name, kind, cell] of [['colB', 'col', 'B'], ['row4', 'row', '4'], ['colA', 'col', 'A'], ['row1', 'row', '1'], ['colC', 'col', 'C'], ['row7', 'row', '7']]) {
    r.seedOk = await reseed();
    await clickCell(2, 2);
    const hd = pick(await handles(), kind, cell);
    if (!hd) { r.clicks[name] = { missing: true }; continue; }
    await c.clickAt(hd.rect.x + hd.rect.w / 2, hd.rect.y + hd.rect.h / 2);
    await sleep(500);
    r.clicks[name] = { handle: hd.rect, menu: await menu(), after: await snap(), shot: await shot('07-click-' + name) };
    await c.pressKey('Escape'); await sleep(200);
  }
  return r;
});

// Real drags: a row handle down one row, a column handle right one column.
await step('s06-drag-row-handle', async (r) => {
  r.cases = {};
  for (const [name, from, toCell, frac] of [['row4-down-below-7', '4', '7', 0.75], ['row4-up-above-1', '4', '1', 0.25], ['row1-down-below-4', '1', '4', 0.75], ['row7-up-above-4', '7', '4', 0.25]]) {
    r.seedOk = await reseed();
    await clickCell(2, 2);
    const w = await widget();
    const hd = pick(await handles(), 'row', from);
    const target = w.cellRects.find((x) => x.text === toCell);
    if (!hd || !target) { r.cases[name] = { missing: true }; continue; }
    const sx = hd.rect.x + hd.rect.w / 2, sy = hd.rect.y + hd.rect.h / 2;
    const during = [];
    await mouse('mouseMoved', sx, sy);
    await mouse('mousePressed', sx, sy, { button: 'left', clickCount: 1 });
    const ty = target.y + target.h * frac;
    for (let i = 1; i <= 14; i++) {
      await mouse('mouseMoved', sx, sy + ((ty - sy) * i) / 14, { button: 'left' });
      await sleep(60);
      if (i === 7) during.push(await c.evaluate(`const w = document.querySelector('.cm-table-widget'); return { cls: w.className, drag: Array.from(w.querySelectorAll('[class*=drag], [class*=drop], [class*=mod-]')).map((e) => e.className).slice(0, 12) };`));
    }
    r.shotMid = r.shotMid || await shot('08-drag-row-mid');
    await mouse('mouseReleased', sx, ty, { button: 'left', clickCount: 1 });
    await sleep(600);
    r.cases[name] = { before: BODY, during, after: await snap(), shot: await shot('09-after-' + name) };
  }
  return r;
});
await step('s07-drag-col-handle', async (r) => {
  r.cases = {};
  for (const [name, from, toCell, frac] of [['colB-right-of-C', 'B', 'C', 0.75], ['colB-left-of-A', 'B', 'A', 0.25], ['colA-right-of-B', 'A', 'B', 0.75], ['colC-left-of-B', 'C', 'B', 0.25]]) {
    r.seedOk = await reseed();
    await clickCell(2, 2);
    const w = await widget();
    const hd = pick(await handles(), 'col', from);
    const target = w.cellRects.find((x) => x.text === toCell);
    if (!hd || !target) { r.cases[name] = { missing: true }; continue; }
    const sx = hd.rect.x + hd.rect.w / 2, sy = hd.rect.y + hd.rect.h / 2;
    const during = [];
    await mouse('mouseMoved', sx, sy);
    await mouse('mousePressed', sx, sy, { button: 'left', clickCount: 1 });
    const tx = target.x + target.w * frac;
    for (let i = 1; i <= 14; i++) {
      await mouse('mouseMoved', sx + ((tx - sx) * i) / 14, sy, { button: 'left' });
      await sleep(60);
      if (i === 7) during.push(await c.evaluate(`const w = document.querySelector('.cm-table-widget'); return { cls: w.className, drag: Array.from(w.querySelectorAll('[class*=drag], [class*=drop], [class*=mod-]')).map((e) => e.className).slice(0, 12) };`));
    }
    r.shotMid = r.shotMid || await shot('10-drag-col-mid');
    await mouse('mouseReleased', tx, sy, { button: 'left', clickCount: 1 });
    await sleep(600);
    r.cases[name] = { during, after: await snap(), shot: await shot('11-after-' + name) };
  }
  return r;
});

// The "+" buttons: click each and read the bytes.
await step('s08-add-buttons', async (r) => {
  r.seedOk = await reseed();
  await clickCell(2, 2);
  const w = await widget();
  const adds = (w.extras || []).filter((e) => /add|plus|new|insert/i.test(e.cls + ' ' + (e.label || '')) && e.rect.w > 0);
  r.adds = adds.map((e) => ({ cls: e.cls, label: e.label, rect: e.rect }));
  r.clicks = [];
  for (let i = 0; i < adds.length; i++) {
    r.seedOk2 = await reseed();
    await clickCell(2, 2);
    const w2 = await widget();
    const list = (w2.extras || []).filter((e) => /add|plus|new|insert/i.test(e.cls + ' ' + (e.label || '')) && e.rect.w > 0);
    const e = list[i];
    if (!e) { r.clicks.push({ i, missing: true }); continue; }
    await c.clickAt(e.rect.x + e.rect.w / 2, e.rect.y + e.rect.h / 2);
    await sleep(600);
    r.clicks.push({ i, cls: e.cls, label: e.label, after: await snap() });
  }
  return r;
});

save();
await c.quit?.();
