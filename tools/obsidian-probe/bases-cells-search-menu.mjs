// Two leftovers of differences.md, in one block: the Bases table's cell
// selection (shift-click and shift+arrow ranges, what Ctrl+C writes to the
// clipboard, what Ctrl+V writes into each note's frontmatter over a range,
// a paste larger than the range, Delete over a range, the selection's look)
// and the Search pane's right-click menus (a file row's and a match row's
// rows, and what each row does).
//
// Fixtures: `cells/` holds five notes with typed properties (text, number,
// checkbox, date, list, a second text) and `Cells.base` a table over them,
// sorted by name (`Grouped.base` the same, grouped by `done`); `search/`
// holds three notes with `needle` in them. Every case reseeds the notes
// through `app.vault.modify` first.
//
//   s00  context: version, window
//   s01  the cursor and a keyboard range: classes, computed look, shift+arrows
//        growing, shrinking and crossing the anchor, a plain arrow, Escape,
//        Ctrl+A
//   s02  shift-click: from the cursor to a checkbox cell's margin, to a name
//        cell's padding, then shift+arrow; a plain click; onto a text field
//   s03  copy: every clipboard type the copy event carries for a range, the
//        whole table, one cell, values with a tab, a newline and a quote; cut
//   s04  Delete and Backspace over a range and a single cell, undo and redo
//   s05  paste: 32 cases of external text and of the table's own copies over
//        cursors and ranges of every type, larger and smaller than the target
//   s06  a grouped table: shift+down across a group heading, a paste across it
//   s07  the Search pane: a file row's and a match row's right-click menus
//   s08  what each row of those menus does
//
// HOLD=1 keeps the run phase alive after the fixtures are open, until the
// file `<results>.stop` appears, so a person can explore over CDP_PORT.
// ONLY_STEPS=<regex> narrows the run. Screenshots go to <results>-shots/.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });
const vp = (rel) => path.join(VAULT, rel);

const NOTES = {
  'cells/Alpha.md': '---\nstatus: todo\nprice: 1.5\ndone: false\ndue: 2026-01-02\nitems:\n  - red\n  - green\nnote: plain\n---\nAlpha body.\n',
  'cells/Bravo.md': '---\nstatus: doing\nprice: 2\ndone: true\ndue: 2026-02-03\nitems:\n  - blue\nnote: second\n---\nBravo body.\n',
  'cells/Charlie.md': '---\nstatus: done\nprice: 30\ndone: false\ndue: 2026-03-04\nitems: []\n---\nCharlie body.\n',
  'cells/Delta.md': '---\nstatus: "with\ttab"\nprice: 4\ndone: true\nnote: "multi word, comma"\n---\nDelta body.\n',
  'cells/Echo.md': 'Echo has no frontmatter.\n',
  'search/One.md': 'The needle is here.\nSecond line without it.\nA needle again.\n',
  'search/Two.md': '# Two\n\nneedle in two.\n',
  'search/Three needle.md': 'Nothing on the first line.\n\nneedle three.\n',
};
const BASE = [
  'filters:',
  '  and:',
  '    - file.inFolder("cells")',
  'views:',
  '  - type: table',
  '    name: Table',
  '    order:',
  '      - file.name',
  '      - status',
  '      - price',
  '      - done',
  '      - due',
  '      - items',
  '      - note',
  '    sort:',
  '      - property: file.name',
  '        direction: ASC',
  '',
].join('\n');
const TYPES = { types: { status: 'text', price: 'number', done: 'checkbox', due: 'date', items: 'multitext', note: 'text' } };

function writeFixtures() {
  for (const [rel, text] of Object.entries(NOTES)) {
    fs.mkdirSync(path.dirname(vp(rel)), { recursive: true });
    fs.writeFileSync(vp(rel), text);
  }
  fs.writeFileSync(vp('Cells.base'), BASE);
  fs.writeFileSync(vp('Grouped.base'), BASE.replace('    sort:', '    groupBy:\n      property: done\n      direction: ASC\n    sort:'));
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/types.json'), JSON.stringify(TYPES, null, 2));
}

if (phase === 'fixtures') { writeFixtures(); process.exit(0); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[cells] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const c = await connect();
const want = VAULT.replace(/\/+$/, '');
const got = (await c.vaultPath()).replace(/\/+$/, '');
if (got !== want) { console.error(`wrong vault: ${got}`); process.exit(3); }

if (process.env.HOLD) {
  console.error('[cells] holding; touch ' + RESULTS + '.stop to finish');
  while (!fs.existsSync(RESULTS + '.stop')) await sleep(1000);
  c.close(); process.exit(0);
}

// ---- helpers ---------------------------------------------------------------
const COLS = ['file.name', 'note.status', 'note.price', 'note.done', 'note.due', 'note.items', 'note.note'];
const ROWS = ['Alpha', 'Bravo', 'Charlie', 'Delta', 'Echo'];
const shot = async (name, clip) => {
  const s = await c.send('Page.captureScreenshot', { format: 'png', ...(clip ? { clip: { ...clip, scale: 1 } } : {}) });
  fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(s.data, 'base64'));
};
const TABLE_CLIP = { x: 344, y: 120, width: 760, height: 190 };
/** The table view object's own state: its cursor (`activeCell`) and range (`selection`). */
const tableState = () => c.evaluate(`
  const v = app.workspace.activeLeaf.view.controller.view;
  const sel = v.selectionEl;
  const box = (e) => { if (!e || !e.isConnected) return null; const b = e.getBoundingClientRect(); return [b.x, b.y, b.width, b.height]; };
  return {
    active: v.activeCell ? { row: v.activeCell.row, column: v.activeCell.column, group: v.activeCell.groupIdx } : null,
    selection: v.selection ? { top: v.selection.top, bottom: v.selection.bottom, start: v.selection.start, end: v.selection.end, group: v.selection.groupIdx } : null,
    activeCellEl: v.activeCellEl ? v.activeCellEl.className : null,
    selectionBox: box(sel),
    focus: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className),
  };`);
const styles = () => c.evaluate(`
  const v = app.workspace.activeLeaf.view.controller.view;
  const cs = (e) => { if (!e || !e.isConnected) return null; const s = getComputedStyle(e); const b = e.getBoundingClientRect(); return { cls: e.className, rect: [b.x, b.y, b.width, b.height], bg: s.backgroundColor, boxShadow: s.boxShadow, radius: s.borderRadius, blend: s.mixBlendMode, pointer: s.pointerEvents }; };
  return { selection: cs(v.selectionEl), activeCell: cs(v.activeCellEl) };`);
const tdRect = (row, prop) => c.evaluate(`
  const e = document.querySelectorAll('.bases-tr')[${row + 1}].querySelector('.bases-td[data-property="${prop}"]');
  const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height, cy: b.y + b.height / 2 };`);
const BASE_NOTES = Object.fromEntries(Object.entries(NOTES).filter(([k]) => k.startsWith('cells/')));
const reseed = (extra = {}) => c.evaluate(`
  const N = ${JSON.stringify(BASE_NOTES)}; Object.assign(N, ${JSON.stringify(extra)});
  for (const [p, t] of Object.entries(N)) { const f = app.vault.getAbstractFileByPath(p); await app.vault.modify(f, t); }
  await new Promise(r => setTimeout(r, 1500)); return true;`);
const readAll = () => c.evaluate(`
  const o = {}; for (const n of ${JSON.stringify(ROWS)}) o[n] = await app.vault.adapter.read('cells/' + n + '.md'); return o;`);
/** Only the notes whose bytes differ from the seed. */
const changed = async (extra = {}) => {
  const now = await readAll(); const seed = { ...BASE_NOTES, ...extra }; const out = {};
  for (const n of ROWS) if (now[n] !== seed['cells/' + n + '.md']) out[n] = now[n];
  return out;
};
const openBase = async (p = 'Cells.base') => {
  await c.evaluate(`
    app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown');
    await new Promise(r => setTimeout(r, 800));
    const f = app.vault.getAbstractFileByPath(${JSON.stringify(p)});
    const leaf = app.workspace.getLeaf('tab'); await leaf.openFile(f); app.workspace.setActiveLeaf(leaf, { focus: true });
    await new Promise(r => setTimeout(r, 2000)); return true;`);
};
/** The cursor on (row, column), reached the way a person would: a click into
 *  Alpha's status cell, Escape twice (the suggestions, then the field), then
 *  the arrows. A click straight on a date cell would hit its Open daily note
 *  button, and one on a checkbox would flip it. */
const cursorTo = async (row, col) => {
  const a = await tdRect(0, 'note.status');
  await c.clickAt(a.x + a.w - 4, a.cy); await sleep(400);
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(250);
  for (let i = 0; i < row; i++) { await c.pressKey('ArrowDown'); await sleep(80); }
  const dc = col - 1;
  for (let i = 0; i < Math.abs(dc); i++) { await c.pressKey(dc > 0 ? 'ArrowRight' : 'ArrowLeft'); await sleep(80); }
};
const selectRange = async (row, col, dr = 0, dc = 0) => {
  await cursorTo(row, col);
  for (let i = 0; i < dr; i++) { await c.pressKey('ArrowDown', { shift: true }); await sleep(100); }
  for (let i = 0; i < dc; i++) { await c.pressKey('ArrowRight', { shift: true }); await sleep(100); }
  return tableState();
};
const recordClipboardEvents = () => c.evaluate(`
  if (!window.__cellsRec) {
    window.__cellsRec = [];
    for (const kind of ['copy', 'cut']) window.addEventListener(kind, (e) => {
      const d = {}; for (const t of e.clipboardData.types) d[t] = e.clipboardData.getData(t);
      window.__cellsRec.push({ kind, types: [...e.clipboardData.types], data: d, prevented: e.defaultPrevented });
    });
  }
  window.__cellsRec.length = 0; return true;`);
const clipboardEvents = () => c.evaluate(`return window.__cellsRec.slice();`);
const notices = () => c.evaluate(`return [...document.querySelectorAll('.notice')].map(n => n.textContent);`);

// ---- s00: context -------------------------------------------------------------
await step('s00-context', async () => {
  await openBase();
  return c.evaluate(`return { title: document.title, ua: navigator.userAgent, w: innerWidth, h: innerHeight };`);
});

// ---- s01: the cursor and a keyboard range, and their look ------------------
await step('s01-keys', async (p) => {
  await reseed(); await openBase();
  const a = await tdRect(0, 'note.status');
  await c.clickAt(a.x + a.w - 4, a.cy); await sleep(400);
  p.click = await tableState(); p.clickStyles = await styles();
  await c.pressKey('Escape'); await sleep(200);
  p.escape1 = await tableState();
  await c.pressKey('Escape'); await sleep(300);
  p.escape2 = await tableState(); p.cursorStyles = await styles();
  await shot('s01-cursor', TABLE_CLIP);
  for (const k of ['ArrowDown', 'ArrowDown', 'ArrowRight']) { await c.pressKey(k, { shift: true }); await sleep(150); }
  p.range = await tableState(); p.rangeStyles = await styles();
  await shot('s01-range', TABLE_CLIP);
  await c.pressKey('ArrowUp', { shift: true }); await sleep(200);
  p.shrink = await tableState();
  await c.pressKey('ArrowLeft', { shift: true }); await sleep(150);
  await c.pressKey('ArrowLeft', { shift: true }); await sleep(200);
  p.pastAnchor = await tableState();
  await c.pressKey('ArrowDown'); await sleep(200);
  p.plainArrow = await tableState();
  await c.pressKey('ArrowDown', { shift: true }); await sleep(200);
  p.again = await tableState();
  await c.pressKey('Escape'); await sleep(200);
  p.escapeOnRange = await tableState();
  await c.pressKey('Escape'); await sleep(200);
  p.escapeAgain = await tableState();
  await cursorTo(0, 1);
  await c.pressKey('a', { ctrl: true }); await sleep(300);
  p.ctrlA = await tableState();
  p.files = await changed();
  return p;
});

// ---- s02: shift-click ---------------------------------------------------------
await step('s02-shift-click', async (p) => {
  await reseed(); await openBase();
  await cursorTo(0, 1);
  p.start = await tableState();
  const d = await tdRect(2, 'note.done');
  await c.clickAt(d.x + 6, d.cy, { mods: { shift: true } }); await sleep(400);
  p.toCheckboxMargin = await tableState();
  await shot('s02-shift-click', TABLE_CLIP);
  const n = await tdRect(3, 'file.name');
  await c.clickAt(n.x + n.w - 6, n.cy, { mods: { shift: true } }); await sleep(400);
  p.toNamePadding = await tableState();
  await c.pressKey('ArrowRight', { shift: true }); await sleep(200);
  p.thenShiftArrow = await tableState();
  await c.clickAt(n.x + n.w - 6, n.cy); await sleep(400);
  p.plainClickOnName = await tableState();
  p.markdownLeaves = await c.evaluate(`return app.workspace.getLeavesOfType('markdown').length;`);
  // A shift-click straight onto a text cell's field.
  await cursorTo(2, 2);
  const s = await tdRect(0, 'note.status');
  await c.clickAt(s.x + s.w - 4, s.cy, { mods: { shift: true } }); await sleep(400);
  p.ontoTextField = await tableState();
  await shot('s02-shift-click-field', TABLE_CLIP);
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(150);
  p.files = await changed();
  return p;
});

// ---- s03: copy ---------------------------------------------------------------
const QUOTED = { 'cells/Alpha.md': '---\nstatus: |-\n  l1\n  l2\nprice: 1.5\nnote: say "hi"\n---\nA\n' };
await step('s03-copy', async (p) => {
  await reseed(); await openBase(); await recordClipboardEvents();
  await selectRange(0, 1, 2, 1);
  await c.pressKey('c', { ctrl: true }); await sleep(400);
  p.range = await clipboardEvents();
  p.rangeClipboard = await c.readClipboard();
  await recordClipboardEvents();
  await cursorTo(0, 0);
  for (let i = 0; i < 4; i++) { await c.pressKey('ArrowDown', { shift: true }); await sleep(80); }
  for (let i = 0; i < 6; i++) { await c.pressKey('ArrowRight', { shift: true }); await sleep(80); }
  p.wholeSelection = await tableState();
  await c.pressKey('c', { ctrl: true }); await sleep(400);
  p.whole = await clipboardEvents();
  await recordClipboardEvents();
  await cursorTo(1, 1);
  await c.pressKey('c', { ctrl: true }); await sleep(400);
  p.singleCell = await clipboardEvents();
  await recordClipboardEvents();
  await reseed(QUOTED); await openBase();
  await selectRange(0, 1, 0, 5);
  await c.pressKey('c', { ctrl: true }); await sleep(400);
  p.quoted = await clipboardEvents();
  await recordClipboardEvents();
  await c.writeClipboard('SENTINEL');
  await selectRange(0, 1, 1, 0);
  await c.pressKey('x', { ctrl: true }); await sleep(1500);
  p.cut = { events: await clipboardEvents(), clipboard: await c.readClipboard(), files: await changed(QUOTED) };
  p.notices = await notices();
  return p;
});

// ---- s04: Delete and Backspace -------------------------------------------------
await step('s04-delete', async (p) => {
  for (const [name, sel, key] of [
    ['deleteRange', [0, 1, 2, 1], 'Delete'],
    ['backspaceRange', [0, 1, 1, 1], 'Backspace'],
    ['deleteSingle', [0, 1, 0, 0], 'Delete'],
    ['backspaceSingle', [1, 2, 0, 0], 'Backspace'],
    ['deleteWholeTable', [0, 0, 4, 6], 'Delete'],
  ]) {
    await reseed(); await openBase();
    const o = { selection: await selectRange(...sel) };
    await c.pressKey(key); await sleep(1500);
    o.after = await tableState(); o.files = await changed();
    if (name === 'deleteRange') {
      await c.pressKey('z', { ctrl: true }); await sleep(1500);
      o.undo = await changed();
      await c.pressKey('z', { ctrl: true, shift: true }); await sleep(1500);
      o.redo = await changed();
    }
    p[name] = o;
  }
  return p;
});

// ---- s05: paste ------------------------------------------------------------------
// [name, target [row, col, dr, dc], clipboard text, or { copy: [row, col, dr, dc] }
// to paste what the table itself copied (its obsidian/bases-cells payload)].
const PASTES = [
  ['singleOverCursor', [0, 1, 0, 0], 'Q'],
  ['singleOverColumn', [0, 1, 2, 0], 'Q'],
  ['singleOverBlock', [0, 1, 1, 1], 'F'],
  ['linesOverCursor', [3, 1, 0, 0], 'r1\nr2\nr3'],
  ['crlfOverCursor', [0, 1, 0, 0], 'c1\r\nc2\r\n'],
  ['markdownTableOverCursor', [0, 1, 0, 0], '| h1 | h2 |\n| --- | --- |\n| m1 | m2 |'],
  ['linesOverColumn', [0, 2, 1, 0], 'abc\n12,5'],
  ['shortColumnOverLonger', [0, 1, 3, 0], 'm\nn'],
  ['trailingNewlineOverColumn', [0, 5, 3, 0], 'a, b\n[[Alpha]]\n"q, r"\n'],
  ['tsvAtCursor', [2, 1, 0, 0], 'x1\ty1\nx2\ty2'],
  ['tsvAtCursorCol1', [0, 1, 0, 0], 'a\tb'],
  ['tsvAtCursorCol2', [0, 2, 0, 0], 'p\tq\tr\ts'],
  ['tsvAtCursorCol3', [0, 3, 0, 0], 'u\tv\tw\tx'],
  ['tsvLargerThanCursor', [0, 1, 0, 0], 'a\tb\tc\nd\te\tf\ng\th\ti'],
  ['tsvSmallerThanRange', [0, 1, 3, 1], 'a\t7\nb\t8'],
  ['tsvPastLastColumn', [0, 5, 0, 0], 'i1\tn1\textra'],
  ['tsvQuoted', [1, 1, 0, 0], '"x\ty"\tz\n"multi\nline"\t"say ""hi"""'],
  ['tsvIntoNoFrontmatter', [4, 1, 0, 1], 'fresh\t9'],
  ['numberColumn', [0, 2, 3, 0], '1e3\n-0.50\n 7 \n0x10'],
  ['checkboxColumn', [0, 3, 4, 0], 'true\nyes\n1\nfalse\nx'],
  ['dateColumn', [0, 4, 3, 0], '2026-05-06\nMay 6, 2026\n2026-05-06T10:20\nnope'],
  ['nameColumn', [0, 0, 0, 0], 'Renamed'],
  ['copiedListIntoText', [0, 6, 0, 0], { copy: [0, 5, 0, 0] }],
  ['copiedNumbersIntoText', [2, 6, 0, 0], { copy: [0, 2, 1, 0] }],
  ['copiedTextIntoNumber', [0, 2, 0, 0], { copy: [0, 1, 0, 0] }],
  ['copiedCheckboxesIntoText', [0, 6, 1, 0], { copy: [0, 3, 1, 0] }],
  ['copiedNamesIntoText', [0, 6, 1, 0], { copy: [0, 0, 1, 0] }],
  ['copiedDateIntoText', [0, 6, 0, 0], { copy: [0, 4, 0, 0] }],
  ['copiedEmptyIntoText', [0, 6, 0, 0], { copy: [4, 1, 0, 0] }],
  ['copiedSingleOverColumn', [0, 6, 2, 0], { copy: [0, 1, 0, 0] }],
  ['copiedBlockAtCursor', [2, 1, 0, 0], { copy: [0, 1, 1, 1] }],
  ['copiedBlockOverRange', [2, 1, 1, 1], { copy: [0, 1, 1, 1] }],
];
await step('s05-paste', async (p) => {
  for (const [name, target, clip] of PASTES) {
    try {
      await reseed(); await openBase();
      const o = {};
      if (clip && clip.copy) {
        o.copyFrom = await selectRange(...clip.copy);
        await c.pressKey('c', { ctrl: true }); await sleep(300);
        o.copied = await c.readClipboard();
        await c.pressKey('Escape'); await sleep(150);
      } else {
        await c.writeClipboard(clip);
        o.clipboard = clip;
      }
      o.target = await selectRange(...target);
      await c.pressKey('v', { ctrl: true }); await sleep(1800);
      o.after = await tableState();
      o.files = await changed();
      o.notices = await notices();
      o.paths = await c.evaluate(`return app.vault.getMarkdownFiles().map(f => f.path).filter(p => p.startsWith('cells/')).sort();`);
      p[name] = o;
    } catch (e) { p[name] = { error: String(e) }; }
  }
  return p;
}, 900000);

// ---- s06: a grouped table -------------------------------------------------------
await step('s06-grouped', async (p) => {
  await reseed(); await openBase('Grouped.base');
  p.groups = await c.evaluate(`return [...document.querySelectorAll('.bases-group-heading, .bases-table-group-heading')].map(e => e.textContent.trim());`);
  p.rows = await c.evaluate(`return [...document.querySelectorAll('.bases-tr')].map(r => (r.querySelector('[data-property="file.name"]') || r).textContent.trim());`);
  const a = await c.evaluate(`const e = [...document.querySelectorAll('.bases-td[data-property="note.status"]')][0]; const b = e.getBoundingClientRect(); return { x: b.x + b.width - 4, y: b.y + b.height / 2 };`);
  await c.clickAt(a.x, a.y); await sleep(400);
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(200);
  p.start = await tableState();
  const moves = [];
  for (let i = 0; i < 5; i++) { await c.pressKey('ArrowDown', { shift: true }); await sleep(150); moves.push(await tableState()); }
  p.shiftDown = moves;
  await shot('s06-grouped', { x: 344, y: 120, width: 760, height: 330 });
  await c.writeClipboard('g1\ng2\ng3\ng4\ng5');
  await c.pressKey('Escape'); await sleep(150);
  await c.clickAt(a.x, a.y); await sleep(400);
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(200);
  for (let i = 0; i < 4; i++) { await c.pressKey('ArrowDown'); await sleep(100); }
  p.cursorAfterDown4 = await tableState();
  await c.pressKey('v', { ctrl: true }); await sleep(1800);
  p.pasteAcrossGroups = { after: await tableState(), files: await changed() };
  return p;
});

// ---- s07, s08: the Search pane's row menus --------------------------------------
const menuDump = () => c.evaluate(`
  return [...document.querySelectorAll('.menu')].map(m => ({
    rect: (() => { const b = m.getBoundingClientRect(); return [b.x, b.y, b.width, b.height]; })(),
    items: [...m.querySelectorAll('.menu-item, .menu-separator')].map(e => e.classList.contains('menu-separator') ? { separator: true, drawn: e.getBoundingClientRect().height > 0 } : {
      title: (e.querySelector('.menu-item-title') || e).textContent.trim(),
      section: e.dataset.section || null,
      cls: e.className,
      icon: e.querySelector('.menu-item-icon svg') ? [...e.querySelector('.menu-item-icon svg').classList].join(' ') : null,
    }),
  }));`);
const openSearch = async (q) => c.evaluate(`
  app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown');
  app.internalPlugins.getPluginById('global-search').instance.openGlobalSearch(${JSON.stringify(q)});
  await new Promise(r => setTimeout(r, 2500)); return true;`);
const rightClickRow = async (selector, index) => {
  await openSearch('needle');
  const r = await c.rect(selector, index);
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600);
};
const pickRow = async (title) => {
  const r = await c.evaluate(`const it = [...document.querySelectorAll('.menu .menu-item')].find(e => e.textContent.trim() === ${JSON.stringify(title)}); if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
  if (!r) return false;
  await c.clickAt(r.cx, r.cy); await sleep(1200);
  return true;
};
const closeAll = async () => {
  await c.pressKey('Escape'); await sleep(300);
  await c.evaluate(`document.querySelectorAll('.modal-close-button').forEach(b => b.click()); return true;`); await sleep(300);
};
const leaves = () => c.evaluate(`
  const out = []; app.workspace.iterateAllLeaves(l => { const st = l.getViewState(); if (st.type !== 'markdown') return; const e = l.view.editor; out.push({ file: st.state.file, active: l === app.workspace.activeLeaf, window: l.view.containerEl.win === window ? 'main' : 'popout', cursor: e ? e.getCursor() : null }); });
  return { markdown: out, activeType: app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() };`);
const surface = () => c.evaluate(`
  const m = document.querySelector('.modal-container'); const pr = document.querySelector('.prompt');
  return { modal: m ? m.innerText.slice(0, 300) : null, modalCls: m && m.querySelector('.modal') ? m.querySelector('.modal').className : null, prompt: pr ? pr.innerText.slice(0, 200) : null,
    focus: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className),
    explorerFocus: (document.querySelector('.nav-file-title.has-focus') || {}).textContent || null };`);

await step('s07-search-menus', async (p) => {
  await openSearch('needle');
  p.rows = await c.evaluate(`return [...document.querySelectorAll('.search-result-file-title, .search-result-file-match')].map(e => ({ cls: e.className, text: e.textContent.trim().slice(0, 60) }));`);
  const title = await c.rect('.search-result-file-title');
  await c.clickAt(title.cx, title.cy, { button: 'right' }); await sleep(700);
  p.fileRow = await menuDump();
  p.rowClassesAfter = await c.evaluate(`return [...document.querySelectorAll('.search-result-file-title, .search-result-file-match')].map(e => e.className);`);
  p.focusAfter = await c.evaluate(`return document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className.slice(0, 40));`);
  await shot('s07-file-row');
  await c.pressKey('Escape'); await sleep(300);
  const match = await c.rect('.search-result-file-match');
  await c.clickAt(match.cx, match.cy, { button: 'right' }); await sleep(700);
  p.matchRow = await menuDump();
  await shot('s07-match-row');
  await c.pressKey('Escape'); await sleep(300);
  return p;
});

await step('s08-search-menu-rows', async (p) => {
  for (const [row, ix, label] of [['.search-result-file-match', 1, 'match'], ['.search-result-file-title', 0, 'file']]) {
    for (const title of ['Open in new tab', 'Open to the right', 'Open in new window']) {
      await c.evaluate(`app.workspace.detachLeavesOfType('markdown'); return true;`); await sleep(500);
      await rightClickRow(row, ix);
      const ok = await pickRow(title); await sleep(800);
      p[label + ': ' + title] = { ok, ...(await leaves()) };
      await c.evaluate(`for (const s of app.workspace.floatingSplit ? app.workspace.floatingSplit.children : []) { try { s.win.close(); } catch (e) {} } return true;`); await sleep(800);
    }
  }
  await rightClickRow('.search-result-file-match', 0);
  const cp = await c.evaluate(`const it = [...document.querySelectorAll('.menu .menu-item')].find(e => e.textContent.trim() === 'Copy path'); const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
  await c.mouseMove(cp.cx, cp.cy); await sleep(800);
  const menus = await menuDump();
  p.copyPathSubmenu = menus[1] || null;
  p.copyPath = {};
  for (const s of (p.copyPathSubmenu ? p.copyPathSubmenu.items : []).filter((i) => i.title).map((i) => i.title)) {
    await closeAll();
    await rightClickRow('.search-result-file-match', 0);
    await c.mouseMove(cp.cx, cp.cy); await sleep(800);
    await c.writeClipboard('SENTINEL');
    const at = await c.evaluate(`const ms = document.querySelectorAll('.menu'); const it = [...ms[ms.length - 1].querySelectorAll('.menu-item')].find(e => e.textContent.trim() === ${JSON.stringify(s)}); if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (at) { await c.mouseMove(at.cx - 20, at.cy); await sleep(200); await c.clickAt(at.cx, at.cy); await sleep(500); }
    p.copyPath[s] = (await c.readClipboard()).split(VAULT).join('<vault>').replace(/vault=[^&]+/, 'vault=<name>');
  }
  for (const title of ['Rename...', 'Move file to...', 'Bookmark...', 'Reveal file in navigation']) {
    await closeAll();
    await rightClickRow('.search-result-file-match', 0);
    await pickRow(title); await sleep(600);
    p[title] = await surface();
  }
  await closeAll();
  await rightClickRow('.search-result-file-title', 2);
  await pickRow('Delete file'); await sleep(600);
  p['Delete file'] = await surface();
  await closeAll();
  p.filesAfter = await c.evaluate(`return app.vault.getMarkdownFiles().map(f => f.path).sort();`);
  return p;
});

c.close();
process.exit(0);
