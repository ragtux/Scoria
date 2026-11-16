// Undo and redo of a Bases table's cell edits: what Ctrl+Z and its redo keys
// do after a paste over a range, a Delete, a typed cell and a checkbox, whether
// each action is one step, where focus has to be, how long the history lives,
// what an undo writes when the note changed in between, and what the note's own
// editor history holds afterwards.
//
// Fixtures: the five notes and `Cells.base` of bases-cells-search-menu.mjs
// (`cells/`, typed properties, a table sorted by name). Every case reseeds the
// notes through `app.vault.modify` first.
//
//   u00  context: version, and every method, field and command whose name
//        speaks of undo or history on the view, its controller and the app
//   u01  paste one value over a column: Ctrl+Z, Ctrl+Shift+Z, Ctrl+Z, Ctrl+Y
//   u02  Delete over a column where one note lacks the key and one has no
//        frontmatter: are the bytes restored exactly?
//   u03  a typed text cell (Enter commits): Ctrl+Z inside the field, after
//        Escape, and redo
//   u04  a checkbox clicked: Ctrl+Z with focus where the click left it, then
//        on the table container
//   u05  three actions in a row (Delete, paste, Delete): Ctrl+Z three times,
//        then redo three times; a fourth Ctrl+Z
//   u06  focus elsewhere: the Files pane, a note's editor, an open field
//   u07  the note changed between the paste and the undo
//   u08  the note open beside the table: its editor's own Ctrl+Z after a
//        table paste, and after a table undo
//   u09  the history across closing and reopening the base, and across a
//        second view of the same base
//   u10  a new action after an undo: is the redo dropped?
//
// HOLD=1 keeps the run phase alive after the fixtures are open, until the
// file `<results>.stop` appears. ONLY_STEPS=<regex> narrows the run.
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
const vp = (rel) => path.join(VAULT, rel);

const NOTES = {
  'cells/Alpha.md': '---\nstatus: todo\nprice: 1.5\ndone: false\ndue: 2026-01-02\nitems:\n  - red\n  - green\nnote: plain\n---\nAlpha body.\n',
  'cells/Bravo.md': '---\nstatus: doing\nprice: 2\ndone: true\ndue: 2026-02-03\nitems:\n  - blue\nnote: second\n---\nBravo body.\n',
  'cells/Charlie.md': '---\nstatus: done\nprice: 30\ndone: false\ndue: 2026-03-04\nitems: []\n---\nCharlie body.\n',
  'cells/Delta.md': '---\nstatus: "with\ttab"\nprice: 4\ndone: true\nnote: "multi word, comma"\n---\nDelta body.\n',
  'cells/Echo.md': 'Echo has no frontmatter.\n',
  'Other.md': 'Another note.\n',
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
  '  - type: table',
  '    name: Second',
  '    order:',
  '      - file.name',
  '      - status',
  '      - price',
  '    sort:',
  '      - property: file.name',
  '        direction: ASC',
  '',
].join('\n');
const TYPES = { types: { status: 'text', price: 'number', done: 'checkbox', due: 'date', items: 'multitext', note: 'text' } };

if (phase === 'fixtures') {
  for (const [rel, text] of Object.entries(NOTES)) {
    fs.mkdirSync(path.dirname(vp(rel)), { recursive: true });
    fs.writeFileSync(vp(rel), text);
  }
  fs.writeFileSync(vp('Cells.base'), BASE);
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/types.json'), JSON.stringify(TYPES, null, 2));
  process.exit(0);
}

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 180000) {
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
  console.error(`[undo] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const c = await connect();
const want = VAULT.replace(/\/+$/, '');
const got = (await c.vaultPath()).replace(/\/+$/, '');
if (got !== want) { console.error(`wrong vault: ${got}`); process.exit(3); }

if (process.env.HOLD) {
  console.error('[undo] holding; touch ' + RESULTS + '.stop to finish');
  while (!fs.existsSync(RESULTS + '.stop')) await sleep(1000);
  c.close(); process.exit(0);
}

// ---- helpers ---------------------------------------------------------------
const ROWS = ['Alpha', 'Bravo', 'Charlie', 'Delta', 'Echo'];
const baseView = `(app.workspace.getLeavesOfType('bases')[0].view.controller.view)`;
const tableState = () => c.evaluate(`
  const v = ${baseView};
  return {
    active: v.activeCell ? { row: v.activeCell.row, column: v.activeCell.column } : null,
    selection: v.selection ? { top: v.selection.top, bottom: v.selection.bottom, start: v.selection.start, end: v.selection.end } : null,
    focus: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className),
    activeLeaf: app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType(),
  };`);
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
  const now = await readAll(); const seed = { ...BASE_NOTES, ...extra }; const o = {};
  for (const n of ROWS) if (now[n] !== seed['cells/' + n + '.md']) o[n] = now[n];
  return o;
};
const openBase = async (p = 'Cells.base') => {
  await c.evaluate(`
    app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown');
    await new Promise(r => setTimeout(r, 800));
    const f = app.vault.getAbstractFileByPath(${JSON.stringify(p)});
    const leaf = app.workspace.getLeaf('tab'); await leaf.openFile(f); app.workspace.setActiveLeaf(leaf, { focus: true });
    await new Promise(r => setTimeout(r, 2000)); return true;`);
};
/** The cursor on (row, column): a click into Alpha's status field, Escape
 *  twice (the suggestions, then the field), then the arrows. */
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
const undo = async () => { await c.pressKey('z', { ctrl: true }); await sleep(1500); };
const redo = async () => { await c.pressKey('z', { ctrl: true, shift: true }); await sleep(1500); };
const redoY = async () => { await c.pressKey('y', { ctrl: true }); await sleep(1500); };
const paste = async (text) => { await c.writeClipboard(text); await c.pressKey('v', { ctrl: true }); await sleep(1800); };
const notices = () => c.evaluate(`return [...document.querySelectorAll('.notice')].map(n => n.textContent);`);
const clearNotices = () => c.evaluate(`document.querySelectorAll('.notice').forEach(n => n.remove()); return true;`);
/** What the table view object's own undo machinery holds, whatever it is called. */
const history = () => c.evaluate(`
  const v = ${baseView};
  const seen = new WeakSet();
  const safe = (x, d = 0) => {
    if (x === null || typeof x !== 'object') return typeof x === 'function' ? '[fn]' : x;
    if (seen.has(x) || d > 4) return '[…]'; seen.add(x);
    if (x instanceof Node) return '[node]';
    if (x.constructor && x.constructor.name === 'TFile') return { file: x.path };
    if (Array.isArray(x)) return x.map(y => safe(y, d + 1));
    const o = {}; for (const k of Object.keys(x)) o[k] = safe(x[k], d + 1); return o;
  };
  return { undo: safe(v.undoStack), redo: safe(v.redoStack), workspaceUndo: safe(app.workspace.undoHistory) };`);

// ---- u00: context and introspection ------------------------------------------
await step('u00-context', async () => {
  await openBase();
  return c.evaluate(`
    const names = (o) => { const s = new Set(); let p = o; while (p && p !== Object.prototype) { for (const k of Object.getOwnPropertyNames(p)) if (/undo|redo|histor/i.test(k)) s.add(k); p = Object.getPrototypeOf(p); } return [...s]; };
    const leaf = app.workspace.getLeavesOfType('bases')[0];
    return {
      title: document.title, ua: navigator.userAgent, w: innerWidth, h: innerHeight,
      view: names(leaf.view.controller.view), controller: names(leaf.view.controller), leafView: names(leaf.view),
      app: names(app), workspace: names(app.workspace), fileManager: names(app.fileManager), vault: names(app.vault),
      sources: { undoTransaction: String(leaf.view.controller.view.undoTransaction), redoTransaction: String(leaf.view.controller.view.redoTransaction),
        pushUndoHistory: String(app.workspace.pushUndoHistory), hasUndoHistory: String(app.workspace.hasUndoHistory), notifyForBulkUndo: String(app.fileManager.notifyForBulkUndo) },
      commands: Object.keys(app.commands.commands).filter(k => /undo|redo/i.test(k)),
      hotkeys: Object.fromEntries(Object.entries(app.hotkeyManager.defaultKeys || {}).filter(([k]) => /undo|redo/i.test(k))),
    };`);
});

// ---- u01: paste, undo, redo by both keys --------------------------------------
await step('u01-paste', async (p) => {
  await reseed(); await openBase(); await clearNotices();
  p.target = await selectRange(0, 1, 2, 0);
  await paste('Q');
  p.pasted = await changed(); p.historyAfterPaste = await history();
  await undo(); p.undo = await changed(); p.undoState = await tableState(); p.historyAfterUndo = await history();
  await redo(); p.redoShiftZ = await changed(); p.redoState = await tableState();
  await undo(); p.undo2 = await changed();
  await redoY(); p.redoCtrlY = await changed();
  await undo(); p.undo3 = await changed();
  p.notices = await notices();
  return p;
});

// ---- u02: Delete adding keys and a block; exact bytes back? --------------------
await step('u02-delete-adds', async (p) => {
  await reseed(); await openBase();
  p.target = await selectRange(2, 6, 2, 0); // note column, Charlie (no key), Delta, Echo (no frontmatter)
  await c.pressKey('Delete'); await sleep(1500);
  p.deleted = await changed();
  await undo(); p.undo = await changed(); p.undoState = await tableState();
  await redo(); p.redo = await changed();
  // A value of every type deleted and restored: list, number, checkbox, date, quoted string.
  await reseed(); await openBase();
  p.target2 = await selectRange(0, 1, 3, 5);
  await c.pressKey('Delete'); await sleep(1500);
  p.deletedAll = await changed();
  await undo(); p.undoAll = await changed();
  return p;
});

// ---- u03: a typed text cell ---------------------------------------------------
await step('u03-typed-cell', async (p) => {
  await reseed(); await openBase();
  const a = await tdRect(0, 'note.status');
  await c.clickAt(a.x + a.w - 4, a.cy); await sleep(400);
  await c.pressKey('a', { ctrl: true }); await sleep(100);
  await c.typeText('edited'); await sleep(300);
  p.typing = { state: await tableState(), files: await changed() };
  await c.pressKey('Enter'); await sleep(1500);
  p.afterEnter = { state: await tableState(), files: await changed() };
  await undo();
  p.undoWhereEnterLeft = { state: await tableState(), files: await changed() };
  await redo();
  p.redoWhereEnterLeft = { state: await tableState(), files: await changed() };
  // Again, leaving the field by Escape first.
  await reseed(); await openBase();
  const b = await tdRect(1, 'note.status');
  await c.clickAt(b.x + b.w - 4, b.cy); await sleep(400);
  await c.pressKey('a', { ctrl: true }); await sleep(100);
  await c.typeText('typed'); await sleep(300);
  await c.pressKey('Escape'); await sleep(200); await c.pressKey('Escape'); await sleep(800);
  p.afterEscapes = { state: await tableState(), files: await changed() };
  await undo();
  p.undoOnContainer = { state: await tableState(), files: await changed() };
  await redo();
  p.redoOnContainer = { state: await tableState(), files: await changed() };
  // Ctrl+Z inside an open field that was typed into but not committed.
  await reseed(); await openBase();
  const d = await tdRect(2, 'note.status');
  await c.clickAt(d.x + d.w - 4, d.cy); await sleep(400);
  await c.typeText('xyz'); await sleep(300);
  await c.pressKey('z', { ctrl: true }); await sleep(800);
  p.undoInsideField = { state: await tableState(), files: await changed(),
    fieldText: await c.evaluate(`const e = document.activeElement; return e ? (e.value !== undefined ? e.value : e.textContent) : null;`) };
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(800);
  p.afterLeaving = await changed();
  return p;
});

// ---- u04: a checkbox ------------------------------------------------------------
await step('u04-checkbox', async (p) => {
  await reseed(); await openBase();
  const d = await tdRect(0, 'note.done');
  const box = await c.evaluate(`
    const e = document.querySelectorAll('.bases-tr')[1].querySelector('.bases-td[data-property="note.done"] input[type=checkbox]');
    const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.clickAt(box.x, box.y); await sleep(1500);
  p.clicked = { state: await tableState(), files: await changed() };
  await undo();
  p.undoWhereClickLeft = { state: await tableState(), files: await changed() };
  await c.clickAt(d.x + d.w - 4, d.cy); await sleep(300); // the cell's margin: the cursor, no flip
  p.cursorOnCell = { state: await tableState(), files: await changed() };
  await undo();
  p.undoOnContainer = { state: await tableState(), files: await changed() };
  await redo();
  p.redo = { state: await tableState(), files: await changed() };
  // Space on the cursor over a checkbox cell, then undo.
  await reseed(); await openBase();
  await cursorTo(1, 3);
  await c.pressKey('Space'); await sleep(1200);
  p.space = { state: await tableState(), files: await changed() };
  await undo(); p.spaceUndo = await changed();
  return p;
});

// ---- u05: three actions, three undos ---------------------------------------------
await step('u05-steps', async (p) => {
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 0); await c.pressKey('Delete'); await sleep(1500);
  p.a1 = await changed();
  await c.pressKey('Escape'); await sleep(150);
  await c.pressKey('ArrowDown'); await sleep(150);
  await paste('P');
  p.a2 = await changed();
  await c.pressKey('ArrowRight'); await sleep(150);
  await c.pressKey('Delete'); await sleep(1500);
  p.a3 = await changed(); p.state = await tableState(); p.history = await history();
  const undos = []; for (let i = 0; i < 4; i++) { await undo(); undos.push({ files: await changed(), state: await tableState() }); }
  p.undos = undos;
  const redos = []; for (let i = 0; i < 4; i++) { await redo(); redos.push({ files: await changed(), state: await tableState() }); }
  p.redos = redos;
  return p;
});

// ---- u06: focus elsewhere ---------------------------------------------------------
await step('u06-focus', async (p) => {
  // The Files pane.
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('F');
  p.pasted = await changed();
  await c.evaluate(`const l = app.workspace.getLeavesOfType('file-explorer')[0]; if (l) { app.workspace.revealLeaf(l); app.workspace.setActiveLeaf(l, { focus: true }); } return true;`);
  await sleep(500);
  p.explorerFocus = await tableState();
  await undo(); p.undoFromExplorer = await changed();
  // Back to the table by a click on a name cell's padding.
  const n = await tdRect(3, 'file.name');
  await c.clickAt(n.x + n.w - 6, n.cy); await sleep(500);
  p.backOnTable = await tableState();
  await undo(); p.undoBackOnTable = await changed();
  // An open field elsewhere in the table.
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('G');
  p.pasted2 = await changed();
  const f = await tdRect(3, 'note.note');
  await c.clickAt(f.x + f.w - 4, f.cy); await sleep(400);
  p.fieldFocus = await tableState();
  await undo(); p.undoInOtherField = await changed();
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(400);
  p.leftField = await tableState();
  await undo(); p.undoAfterLeaving = await changed();
  // The command palette's own undo, if there is one, with the table active.
  p.commands = await c.evaluate(`return Object.keys(app.commands.commands).filter(k => /undo|redo/i.test(k));`);
  // The cursor dropped (Escape twice): the container still focused?
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('H');
  await c.pressKey('Escape'); await sleep(150); await c.pressKey('Escape'); await sleep(300);
  p.noCursor = await tableState();
  await undo(); p.undoNoCursor = await changed();
  return p;
});

// ---- u07: the note changed in between ---------------------------------------------
await step('u07-external', async (p) => {
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 1); await paste('E');
  p.pasted = await changed();
  // The body edited, and another key of the frontmatter changed, from outside.
  await c.evaluate(`const f = app.vault.getAbstractFileByPath('cells/Alpha.md'); const t = await app.vault.read(f);
    await app.vault.modify(f, t.replace('Alpha body.', 'Alpha body, edited.').replace('note: plain', 'note: changed')); await new Promise(r => setTimeout(r, 1500)); return true;`);
  p.external = await changed();
  await c.pressKey('Escape'); await sleep(100);
  const n = await tdRect(2, 'file.name'); await c.clickAt(n.x + n.w - 6, n.cy); await sleep(400);
  await undo(); p.undo = await changed();
  await redo(); p.redo = await changed();
  // A pasted cell edited again from outside: whose value does undo restore?
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 0); await paste('P1');
  await c.evaluate(`const f = app.vault.getAbstractFileByPath('cells/Alpha.md'); const t = await app.vault.read(f);
    await app.vault.modify(f, t.replace('status: P1', 'status: outside')); await new Promise(r => setTimeout(r, 1500)); return true;`);
  p.overwritten = await changed();
  const m = await tdRect(2, 'file.name'); await c.clickAt(m.x + m.w - 6, m.cy); await sleep(400);
  await undo(); p.undoOverwritten = await changed();
  // Only the body edited from outside: the frontmatter is as the paste left it.
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 0); await paste('P2');
  await c.evaluate(`const f = app.vault.getAbstractFileByPath('cells/Alpha.md'); const t = await app.vault.read(f);
    await app.vault.modify(f, t.replace('Alpha body.', 'Alpha body, edited.')); await new Promise(r => setTimeout(r, 1500)); return true;`);
  p.bodyOnly = await changed();
  const q = await tdRect(2, 'file.name'); await c.clickAt(q.x + q.w - 6, q.cy); await sleep(400);
  await undo(); p.undoBodyOnly = await changed();
  await redo(); p.redoBodyOnly = await changed();
  // The frontmatter reformatted from outside, same values (a quoted string).
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 0); await paste('P3');
  await c.evaluate(`const f = app.vault.getAbstractFileByPath('cells/Alpha.md'); const t = await app.vault.read(f);
    await app.vault.modify(f, t.replace('status: P3', 'status: "P3"')); await new Promise(r => setTimeout(r, 1500)); return true;`);
  p.reformatted = await changed();
  const q2 = await tdRect(2, 'file.name'); await c.clickAt(q2.x + q2.w - 6, q2.cy); await sleep(400);
  await undo(); p.undoReformatted = await changed();
  p.notices = await notices();
  return p;
});

// ---- u08: the note open beside the table --------------------------------------------
const openBeside = () => c.evaluate(`
  const baseLeaf = app.workspace.getLeavesOfType('bases')[0];
  const leaf = app.workspace.getLeaf('split', 'vertical'); await leaf.openFile(app.vault.getAbstractFileByPath('cells/Alpha.md'));
  await leaf.setViewState({ type: 'markdown', state: { file: 'cells/Alpha.md', mode: 'source', source: true } });
  await new Promise(r => setTimeout(r, 1200));
  app.workspace.setActiveLeaf(baseLeaf, { focus: true });
  return true;`);
const noteEditor = () => c.evaluate(`
  const l = app.workspace.getLeavesOfType('markdown')[0]; const e = l.view.editor;
  return { text: e.getValue(), undoDepth: (() => { try { const cm = e.cm; const h = cm.state.values.find(v => v && v.done && v.undone); return h ? { done: h.done.length, undone: h.undone.length } : null; } catch (x) { return String(x); } })() };`);
const focusNote = async () => {
  const r = await c.evaluate(`const l = app.workspace.getLeavesOfType('markdown')[0]; const b = l.view.contentEl.querySelector('.cm-content').getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height - 10 };`);
  await c.clickAt(r.x, r.y); await sleep(400);
};
await step('u08-note-open', async (p) => {
  await reseed(); await openBase(); await openBeside();
  p.before = await noteEditor();
  await selectRange(0, 1, 0, 0); await paste('S');
  p.pasted = { files: await changed(), editor: await noteEditor() };
  await focusNote();
  p.noteFocus = await tableState();
  await c.pressKey('z', { ctrl: true }); await sleep(1500);
  p.editorUndo = { files: await changed(), editor: await noteEditor() };
  await c.pressKey('z', { ctrl: true, shift: true }); await sleep(1500);
  p.editorRedo = { files: await changed(), editor: await noteEditor() };
  // Back to the table: undo there, then look at the note's editor history.
  const n = await tdRect(2, 'file.name'); await c.clickAt(n.x + n.w - 6, n.cy); await sleep(400);
  await undo();
  p.tableUndo = { files: await changed(), editor: await noteEditor() };
  await focusNote();
  await c.pressKey('z', { ctrl: true }); await sleep(1500);
  p.editorUndoAfterTableUndo = { files: await changed(), editor: await noteEditor() };
  // A typed edit in the note, then a table paste, then the table's undo: does
  // the note's own edit survive?
  await reseed(); await openBase(); await openBeside();
  await focusNote(); await c.pressKey('End', { ctrl: true }); await c.typeText(' typed'); await sleep(2500);
  p.typedInNote = { files: await changed(), editor: await noteEditor() };
  const t = await tdRect(2, 'file.name'); await c.clickAt(t.x + t.w - 6, t.cy); await sleep(400);
  await selectRange(0, 1, 0, 0); await paste('T');
  p.pastedAfterTyping = { files: await changed(), editor: await noteEditor() };
  const t2 = await tdRect(2, 'file.name'); await c.clickAt(t2.x + t2.w - 6, t2.cy); await sleep(400);
  await undo();
  p.tableUndoAfterTyping = { files: await changed(), editor: await noteEditor() };
  return p;
});

// ---- u09: the history's lifetime --------------------------------------------------------
await step('u09-lifetime', async (p) => {
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('L');
  p.pasted = await changed();
  // Closed and reopened.
  await openBase();
  await cursorTo(3, 1);
  await undo(); p.undoAfterReopen = await changed();
  // Another view of the same base.
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('V');
  await c.evaluate(`const v = app.workspace.getLeavesOfType('bases')[0].view; const ctl = v.controller; if (ctl.selectView) ctl.selectView('Second'); else if (v.setViewState) {} return true;`);
  await sleep(1200);
  p.viewName = await c.evaluate(`const ctl = app.workspace.getLeavesOfType('bases')[0].view.controller; return ctl.viewName || (ctl.view && ctl.view.config && ctl.view.config.name) || null;`);
  await cursorTo(3, 1);
  await undo(); p.undoAfterViewSwitch = await changed();
  // Another note opened in the same tab and back.
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('W');
  await c.evaluate(`const l = app.workspace.getLeavesOfType('bases')[0]; await l.openFile(app.vault.getAbstractFileByPath('Other.md')); await new Promise(r => setTimeout(r, 800)); await l.openFile(app.vault.getAbstractFileByPath('Cells.base')); await new Promise(r => setTimeout(r, 1500)); app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  await cursorTo(3, 1);
  await undo(); p.undoAfterNavigation = await changed();
  // A second base leaf: is the history shared between two leaves of one file?
  await reseed(); await openBase();
  await selectRange(0, 1, 1, 0); await paste('X');
  await c.evaluate(`const leaf = app.workspace.getLeaf('split', 'vertical'); await leaf.openFile(app.vault.getAbstractFileByPath('Cells.base')); app.workspace.setActiveLeaf(leaf, { focus: true }); await new Promise(r => setTimeout(r, 1500)); return true;`);
  const r = await c.evaluate(`const leaf = app.workspace.activeLeaf; const e = leaf.view.containerEl.querySelectorAll('.bases-tr')[4].querySelector('.bases-td[data-property="file.name"]'); const b = e.getBoundingClientRect(); return { x: b.x + b.width - 6, y: b.y + b.height / 2 };`);
  await c.clickAt(r.x, r.y); await sleep(400);
  await undo(); p.undoInSecondLeaf = await changed();
  return p;
});

// ---- u10: a new action after an undo -----------------------------------------------------
await step('u10-branch', async (p) => {
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 0); await paste('B1');
  await c.pressKey('Escape'); await sleep(100); await c.pressKey('ArrowDown'); await sleep(100);
  await paste('B2');
  p.two = await changed();
  await undo(); p.undoOne = await changed();
  await c.pressKey('ArrowDown'); await sleep(100);
  await paste('B3');
  p.newAction = await changed();
  await redo(); p.redoAfterNew = await changed();
  await undo(); p.undoNew = await changed();
  await undo(); p.undoFirst = await changed();
  // Paste of the same value the cell already holds: is it a step?
  await reseed(); await openBase();
  await selectRange(0, 1, 0, 0); await paste('todo');
  p.sameValue = await changed();
  await selectRange(1, 1, 0, 0); await paste('Z');
  await undo(); p.undoAfterSame1 = await changed();
  await undo(); p.undoAfterSame2 = await changed();
  return p;
});

save();
c.close();
process.exit(0);
