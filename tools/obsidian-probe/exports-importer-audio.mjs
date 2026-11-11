// The five ids nothing had observed: workspace:export-pdf,
// canvas:export-as-image, markdown-importer:open, audio-recorder:start and
// audio-recorder:stop.
//
//   s00  context: version; each id's name, icon, hotkey and kind; when the
//        palette offers it (a note in each mode, a canvas, an empty tab);
//        the two plugins' state and options; the ribbon's rows
//   s01  Export to PDF: the modal's DOM and controls with app.json as it
//        starts, the save dialog's options (stubbed in the main process), the
//        file written (size, pages, MediaBox, Title, Producer); then each
//        control changed and exported again, app.json after each
//   s02  Export as image on a canvas: the modal, its controls, where the image
//        goes (a save dialog or a download), its name, type and pixel size, a
//        canvas config written anywhere; a selection exported
//   s03  Format converter: the modal, its toggles and buttons, the bytes of
//        every fixture note before and after a run with each toggle on alone,
//        and the plugin's data file
//   s04  Audio recorder: start and stop in a note (the file's name, folder and
//        bytes' type, the text written and where), the ribbon and status bar
//        while recording, with an attachment folder, with an empty tab, stop
//        when not recording
//
// Needs OBSIDIAN_ARGS="--inspect=<MAIN_PORT> --use-fake-device-for-media-stream
// --use-fake-ui-for-media-stream" and MAIN_PORT; OUT_DIR is where the stubbed
// dialogs point. ONLY=<regex> picks steps.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const MAIN_PORT = process.env.MAIN_PORT;
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('bad phase'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
const OUT_DIR = process.env.OUT_DIR || RESULTS.replace(/\.json$/, '-out');
fs.mkdirSync(SHOTS, { recursive: true });
fs.mkdirSync(OUT_DIR, { recursive: true });

const T0 = Date.now();
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
const out = { startedAt: new Date().toISOString(), steps: { ...((all[phase] || {}).steps || {}) }, order: [] };
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 240000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now(); const partial = {}; let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...v }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[exports] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
}
// A bounded sub-step: a hang costs that line, not the step.
async function bounded(partial, key, fn, ms = 15000) {
  let timer;
  try { partial[key] = await Promise.race([fn(), new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('timeout')), ms); })]); }
  catch (e) { partial[key] = { error: String(e.message || e) }; }
  finally { clearTimeout(timer); }
  return partial[key];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
function listVault(dir = '', acc = []) {
  for (const n of fs.readdirSync(vp(dir)).sort()) {
    if (n === '.obsidian' || n === '.trash') continue;
    const rel = dir ? `${dir}/${n}` : n;
    const st = fs.statSync(vp(rel));
    if (st.isDirectory()) listVault(rel, acc); else acc.push({ path: rel, size: st.size });
  }
  return acc;
}
function snapshotNotes() {
  const o = {};
  for (const f of listVault()) if (/\.(md|canvas)$/.test(f.path)) o[f.path] = bytes(f.path);
  return o;
}
function pdfFacts(file) {
  const b = fs.readFileSync(file);
  const s = b.toString('latin1');
  const boxes = [...s.matchAll(/\/MediaBox\s*\[\s*([\d.\s-]+)\]/g)].map((m) => m[1].trim().split(/\s+/).map(Number));
  const str = (k) => { const m = s.match(new RegExp(`/${k}\\s*\\(((?:\\\\.|[^\\\\)])*)\\)`)); if (m) return m[1]; const h = s.match(new RegExp(`/${k}\\s*<([0-9A-Fa-f]+)>`)); return h ? Buffer.from(h[1], 'hex').toString('utf16le') : null; };
  return { size: b.length, head: s.slice(0, 8), pages: (s.match(/\/Type\s*\/Page[^s]/g) || []).length, mediaBoxes: [...new Set(boxes.map((x) => x.join(' ')))], title: str('Title'), producer: str('Producer'), creator: str('Creator'), outlines: /\/Outlines/.test(s), links: (s.match(/\/Subtype\s*\/Link/g) || []).length };
}
function pngFacts(file) {
  const b = fs.readFileSync(file);
  if (b.slice(1, 4).toString() === 'PNG') return { type: 'png', size: b.length, width: b.readUInt32BE(16), height: b.readUInt32BE(20), colorType: b[25] };
  return { type: b.slice(0, 4).toString('hex'), size: b.length };
}

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, 'command-palette': true, canvas: true,
    'markdown-importer': true, 'audio-recorder': true, 'page-preview': true, outline: true,
  }, null, 2));
  const long = Array.from({ length: 60 }, (_, i) => `Paragraph ${i + 1}. ` + 'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '.repeat(3)).join('\n\n');
  fs.writeFileSync(vp('Note.md'), `---\ntags: [a]\n---\n# Export me\n\nSome **bold** text and a [[Other]] link and [a web link](https://example.com).\n\n## Second\n\n![[pic.png]]\n\n${long}\n`);
  fs.writeFileSync(vp('Other.md'), 'Other note.\n');
  fs.writeFileSync(vp('Short.md'), 'One line.\n');
  // A 4x4 red PNG.
  fs.writeFileSync(vp('pic.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEklEQVR4nGP4z8CAB+GTG8HSALfKY52fRZqhAAAAAElFTkSuQmCC', 'base64'));
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify({ nodes: [
    { id: 'a1', type: 'text', text: 'Hello canvas', x: 0, y: 0, width: 250, height: 60 },
    { id: 'b2', type: 'file', file: 'Other.md', x: 400, y: 100, width: 400, height: 300 },
    { id: 'c3', type: 'group', label: 'Group', x: -50, y: 300, width: 300, height: 200, color: '4' },
  ], edges: [{ id: 'e1', fromNode: 'a1', fromSide: 'right', toNode: 'b2', toSide: 'left', label: 'to' }] }, null, '\t'));
  // The Format converter's inputs: Bear, Roam and the older Obsidian forms.
  fs.mkdirSync(vp('Convert'), { recursive: true });
  const conv = {
    'Convert/Highlights.md': 'A ^^roam highlight^^ and ==obsidian==.\nAnd ::bear highlight:: here.\n',
    'Convert/Bear tags.md': '#tag# and #multi word tag# and #nested/tag# and #plain\n',
    'Convert/Roam.md': '- [[Page]] and #[[Long Tag]] and #tag\n- {{[[TODO]]}} task\n- {{[[DONE]]}} done\n- ((block-ref))\n- `[[not]]` and __italic__\n',
    'Convert/Links.md': '[[Other.md]] and [[Other.md|alias]] and [x](Other.md) and [y](Other%20Note.md) and ![](pic.png)\n',
    'Convert/Frontmatter.md': '---\ntag: a\nalias: b\ncssclass: c\n---\nBody\n',
    'Convert/Plain.md': 'Nothing to convert.\n',
  };
  for (const [k, v] of Object.entries(conv)) fs.writeFileSync(vp(k), v);
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

// ---- the main process, through Node's inspector -----------------------------
async function mainEval(body) {
  const ts = await (await fetch(`http://127.0.0.1:${MAIN_PORT}/json`)).json();
  const ws = new WebSocket(ts[0].webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  const v = await new Promise((res, rej) => {
    ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === 1) { const r = m.result || {}; r.exceptionDetails ? rej(new Error(JSON.stringify(r.exceptionDetails).slice(0, 800))) : res(r.result && r.result.value); } };
    const expression = `(async () => { const require = process.mainModule.require.bind(process.mainModule); ${body} })()`;
    ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
    setTimeout(() => rej(new Error('main eval timeout')), 15000);
  });
  ws.close();
  return v;
}

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { out.error = 'wrong vault'; save(); process.exit(3); }
const ev = (body) => c.evaluate(body);

out.mainStub = MAIN_PORT ? await mainEval(`
  const e = require('electron');
  global.__dlg = []; global.__save = null; global.__pick = null;
  for (const n of ['showSaveDialog', 'showSaveDialogSync', 'showOpenDialog', 'showOpenDialogSync']) {
    e.dialog[n] = (...a) => {
      const opts = a.find((x) => x && typeof x === 'object' && !x.webContents && (x.defaultPath !== undefined || x.filters || x.properties || x.title)) || null;
      global.__dlg.push({ n, opts: opts ? JSON.parse(JSON.stringify(opts)) : null });
      const save = n.startsWith('showSave');
      const p = save ? global.__save : global.__pick;
      if (n.endsWith('Sync')) return save ? (p || undefined) : (p || undefined);
      return Promise.resolve(save ? (p ? { canceled: false, filePath: p } : { canceled: true, filePath: '' }) : (p ? { canceled: false, filePaths: p } : { canceled: true, filePaths: [] }));
    };
  }
  // shell calls, so nothing opens on a desk.
  global.__shell = [];
  for (const n of ['openPath', 'showItemInFolder', 'openExternal']) { e.shell[n] = (...a) => { global.__shell.push({ n, a }); return Promise.resolve(''); }; }
  return true;`).catch((e) => 'stub failed: ' + e.message) : 'no MAIN_PORT';
const setSave = (p) => mainEval(`global.__dlg = []; global.__save = ${JSON.stringify(p)}; return true;`);
const dialogs = () => mainEval(`return { dlg: global.__dlg, shell: global.__shell };`);

// Downloads (an <a download> or a blob) land in OUT_DIR/dl.
const DL = path.join(OUT_DIR, 'dl'); fs.mkdirSync(DL, { recursive: true });
await c.send('Browser.setDownloadBehavior', { behavior: 'allowAndName', downloadPath: DL, eventsEnabled: true }).catch(() => c.send('Page.setDownloadBehavior', { behavior: 'allow', downloadPath: DL }).catch(() => null));
const downloads = [];
try { c.on('Browser.downloadWillBegin', (p) => downloads.push({ ev: 'begin', url: String(p.url).slice(0, 120), suggestedFilename: p.suggestedFilename })); c.on('Browser.downloadProgress', (p) => { if (p.state !== 'inProgress') downloads.push({ ev: p.state, guid: p.guid, total: p.totalBytes }); }); } catch { /* no event support */ }

await ev(`
window.__r = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; },
  skeleton(e, d = 0, max = 9) {
    if (!e) return null;
    const tag = e.tagName.toLowerCase();
    if (['path', 'line', 'circle', 'rect', 'polyline', 'polygon'].includes(tag)) return null;
    const o = { tag, cls: e.getAttribute('class') || undefined };
    for (const a of ['aria-label', 'type', 'placeholder', 'value', 'role', 'min', 'max', 'step']) { const v = e.getAttribute(a); if (v !== null) o[a] = v; }
    if (tag === 'input') { o.prop = e.type === 'checkbox' ? e.checked : e.value; }
    if (tag === 'select') { o.prop = e.value; o.options = Array.from(e.options).map((x) => [x.value, x.textContent]); }
    if (!e.children.length || tag === 'select') { const t = (e.textContent || '').trim(); if (t && tag !== 'select') o.text = t.slice(0, 200); }
    if (d < max && e.children.length && tag !== 'select') o.children = Array.from(e.children).map((c) => this.skeleton(c, d + 1, max)).filter(Boolean);
    return o;
  },
  offered(id) { return app.commands.listCommands().some((x) => x.id === id); },
  async open(p, mode) {
    const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l));
    for (const l of ls.slice(1)) l.detach();
    const leaf = ls[0] || app.workspace.getLeaf(false);
    if (p === null) await leaf.setViewState({ type: 'empty', state: {} });
    else {
      await leaf.openFile(app.vault.getAbstractFileByPath(p));
      if (mode) { const st = leaf.getViewState(); st.state.mode = mode === 'reading' ? 'preview' : 'source'; st.state.source = mode === 'source'; await leaf.setViewState(st); }
    }
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(700);
    return leaf.view.getViewType();
  },
  modal() { const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; return m ? this.skeleton(m, 0, 12) : null; },
  modals() { return Array.from(document.querySelectorAll('.modal-container')).map((m) => ({ title: ((m.querySelector('.modal-title') || {}).textContent || '').trim(), text: (m.textContent || '').trim().slice(0, 600) })); },
  notices() { return Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent); },
  controls() {
    const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; if (!m) return null;
    return Array.from(m.querySelectorAll('.setting-item')).map((s) => ({
      name: ((s.querySelector('.setting-item-name') || {}).textContent || '').trim(),
      desc: ((s.querySelector('.setting-item-description') || {}).textContent || '').trim(),
      selects: Array.from(s.querySelectorAll('select')).map((x) => ({ value: x.value, options: Array.from(x.options).map((o) => [o.value, o.textContent]) })),
      toggles: Array.from(s.querySelectorAll('.checkbox-container')).map((x) => x.classList.contains('is-enabled')),
      inputs: Array.from(s.querySelectorAll('input:not([type=checkbox])')).map((x) => ({ type: x.type, value: x.value, min: x.min, max: x.max, step: x.step })),
      buttons: Array.from(s.querySelectorAll('button')).map((b) => ({ text: b.textContent.trim(), cls: b.className })),
    }));
  },
  buttons() { const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; return m ? Array.from(m.querySelectorAll('button')).map((b) => ({ text: b.textContent.trim(), cls: b.className })) : []; },
  click(text) { const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; if (!m) return null; const b = Array.from(m.querySelectorAll('button')).find((x) => x.textContent.trim() === text); if (!b) return null; b.click(); return true; },
  setSelect(row, value) { const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; const s = Array.from(m.querySelectorAll('.setting-item'))[row]; const x = s && s.querySelector('select'); if (!x) return null; x.value = value; x.dispatchEvent(new Event('change', { bubbles: true })); return x.value; },
  setInput(row, value) { const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; const s = Array.from(m.querySelectorAll('.setting-item'))[row]; const x = s && s.querySelector('input:not([type=checkbox])'); if (!x) return null; x.value = value; x.dispatchEvent(new Event('input', { bubbles: true })); x.dispatchEvent(new Event('change', { bubbles: true })); return x.value; },
  toggle(row) { const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; const s = Array.from(m.querySelectorAll('.setting-item'))[row]; const x = s && s.querySelector('.checkbox-container'); if (!x) return null; x.click(); return x.classList.contains('is-enabled'); },
  closeModals() { for (let i = 0; i < 4; i++) { const ms = Array.from(document.querySelectorAll('.modal-container')); if (!ms.length) break; const b = ms[ms.length - 1].querySelector('.modal-close-button, .modal-header-button'); if (b) b.click(); else break; } return document.querySelectorAll('.modal-container').length; },
};
return true;`);
const R = 'const R = window.__r;';
let shotN = 0;
async function shot(name) {
  try { const r = await c.send('Page.captureScreenshot', { format: 'png' }); const f = `${String(++shotN).padStart(2, '0')}-${name}.png`; fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64')); return f; }
  catch (e) { return 'shot failed: ' + e.message; }
}
async function waitFile(file, ms = 20000) {
  const t = Date.now(); let last = -1;
  while (Date.now() - t < ms) {
    if (fs.existsSync(file)) { const s = fs.statSync(file).size; if (s > 0 && s === last) return { ms: Date.now() - t, size: s }; last = s; }
    await sleep(400);
  }
  return null;
}
async function waitDir(dir, before, ms = 20000) {
  const t = Date.now();
  while (Date.now() - t < ms) {
    const now = fs.readdirSync(dir).filter((n) => !before.includes(n) && !n.endsWith('.crdownload'));
    if (now.length) { await sleep(800); return now; }
    await sleep(400);
  }
  return [];
}
const IDS = ['workspace:export-pdf', 'canvas:export-as-image', 'markdown-importer:open', 'audio-recorder:start', 'audio-recorder:stop'];

await step('s00-context', async (p) => {
  p.version = await ev(`return { title: document.title, ua: navigator.userAgent, electron: process.versions && process.versions.electron };`);
  p.cmds = await ev(`const o = {}; for (const id of ${JSON.stringify(IDS)}) { const x = app.commands.commands[id]; o[id] = x ? { name: x.name, icon: x.icon || null, keys: (app.hotkeyManager.defaultKeys || {})[id] || null, kinds: ['callback', 'checkCallback', 'editorCallback', 'editorCheckCallback'].filter((k) => x[k]) } : null; } return o;`);
  p.otherIds = await ev(`return Object.keys(app.commands.commands).filter((k) => /^(markdown-importer|audio-recorder):|export|pdf/.test(k));`);
  p.plugins = await ev(`const o = {}; for (const id of ['markdown-importer', 'audio-recorder', 'canvas']) { const pl = app.internalPlugins.getPluginById(id); const i = pl && pl.instance; o[id] = pl ? { enabled: pl.enabled, name: i && i.name, desc: i && i.description, options: i && i.options ? JSON.parse(JSON.stringify(i.options)) : null, keys: i ? Object.keys(i).sort() : null } : null; } return o;`);
  p.offered = {};
  for (const [label, file, mode] of [['live', 'Note.md', 'live'], ['source', 'Note.md', 'source'], ['reading', 'Note.md', 'reading'], ['canvas', 'Board.canvas'], ['image', 'pic.png'], ['empty', null]]) {
    p.offered[label] = await ev(`${R} const vt = await R.open(${JSON.stringify(file)}, ${JSON.stringify(mode || null)}); const o = { view: vt }; for (const id of ${JSON.stringify(IDS)}) o[id] = R.offered(id); return o;`);
  }
  p.ribbon = await ev(`return Array.from(document.querySelectorAll('.side-dock-ribbon-action')).map((e) => ({ aria: e.getAttribute('aria-label'), icon: (e.querySelector('svg') || {}).classList ? Array.from(e.querySelector('svg').classList).join(' ') : null, hidden: e.style.display === 'none' }));`);
  p.app = bytes('.obsidian/app.json');
  return p;
});

await step('s01-export-pdf', async (p) => {
  await ev(`${R} await R.open('Note.md', 'live'); return true;`);
  p.runs = [];
  await bounded(p, 'open', async () => {
    const ran = await ev(`return app.commands.executeCommandById('workspace:export-pdf');`); await sleep(2500);
    return { ran, modal: await ev(`${R} return R.modal();`), controls: await ev(`${R} return R.controls();`), buttons: await ev(`${R} return R.buttons();`), modals: await ev(`${R} return R.modals();`), shot: await shot('pdf-modal'), app: bytes('.obsidian/app.json'), config: await ev(`return app.vault.getConfig('pdfExportSettings') || null;`) };
  }, 20000);
  const exportOnce = async (label, change) => {
    const r = { label };
    if (change) r.change = await ev(`${R} ${change}`);
    await sleep(800);
    r.controls = await ev(`${R} return R.controls();`);
    r.appBeforeExport = bytes('.obsidian/app.json');
    const target = path.join(OUT_DIR, `${label}.pdf`);
    await setSave(target);
    const btn = await ev(`${R} const bs = R.buttons(); return bs.map((b) => b.text);`);
    r.buttonsSeen = btn;
    r.clicked = await ev(`${R} return R.click('Export to PDF') || R.click('Export') || null;`);
    r.file = await waitFile(target, 30000);
    r.dialogs = await dialogs();
    if (r.file) r.pdf = pdfFacts(target);
    await sleep(1500);
    r.notices = await ev(`${R} return R.notices();`);
    r.modalsAfter = await ev(`${R} return R.modals();`);
    r.app = bytes('.obsidian/app.json');
    r.config = await ev(`return app.vault.getConfig('pdfExportSettings') || null;`);
    r.dlDir = fs.readdirSync(DL);
    p.runs.push(r);
    save();
    return r;
  };
  await bounded(p, 'first', () => exportOnce('default'), 45000);
  // Each control in turn: reopen the modal, change one thing, export.
  const ctl = (p.open && p.open.controls) || [];
  const plans = [];
  ctl.forEach((row, i) => {
    if (row.selects.length) for (const [v] of row.selects[0].options) if (v !== row.selects[0].value) plans.push([`r${i}-${v}`.replace(/[^\w-]/g, '_'), `return R.setSelect(${i}, ${JSON.stringify(v)});`]);
    if (row.toggles.length) plans.push([`r${i}-toggle`, `return R.toggle(${i});`]);
    if (row.inputs.length) plans.push([`r${i}-input`, `return R.setInput(${i}, '50');`]);
  });
  p.plans = plans.map((x) => x[0]);
  for (const [label, change] of plans.slice(0, 14)) {
    await bounded(p, 'plan-' + label, async () => {
      await ev(`${R} R.closeModals(); return true;`); await sleep(500);
      await ev(`return app.commands.executeCommandById('workspace:export-pdf');`); await sleep(1800);
      return exportOnce(label, change);
    }, 45000);
  }
  // Cancel at the dialog.
  await bounded(p, 'cancel', async () => {
    await ev(`${R} R.closeModals(); return true;`); await sleep(500);
    await ev(`return app.commands.executeCommandById('workspace:export-pdf');`); await sleep(1800);
    await setSave(null);
    await ev(`${R} return R.click('Export to PDF') || R.click('Export');`); await sleep(3000);
    return { dialogs: await dialogs(), modals: await ev(`${R} return R.modals();`), notices: await ev(`${R} return R.notices();`) };
  }, 30000);
  // A short note, and the reading view.
  await bounded(p, 'short-reading', async () => {
    await ev(`${R} R.closeModals(); await R.open('Short.md', 'reading'); return true;`);
    await ev(`return app.commands.executeCommandById('workspace:export-pdf');`); await sleep(1800);
    return exportOnce('short-reading');
  }, 45000);
  await ev(`${R} R.closeModals(); return true;`);
  return p;
}, 600000);

await step('s02-canvas-image', async (p) => {
  await ev(`${R} await R.open('Board.canvas'); return true;`); await sleep(1500);
  p.canvasBefore = bytes('Board.canvas');
  p.runs = [];
  const exportOnce = async (label, change) => {
    const r = { label };
    const before = fs.readdirSync(DL);
    const target = path.join(OUT_DIR, `${label}.png`);
    await setSave(target);
    downloads.length = 0;
    r.ran = await ev(`return app.commands.executeCommandById('canvas:export-as-image');`); await sleep(2000);
    r.modal = await ev(`${R} return R.modal();`);
    r.controls = await ev(`${R} return R.controls();`);
    r.buttons = await ev(`${R} return R.buttons();`);
    r.shot = await shot('canvas-modal-' + label);
    if (change) { r.change = await ev(`${R} ${change}`); await sleep(1200); r.controlsAfter = await ev(`${R} return R.controls();`); r.shotAfter = await shot('canvas-modal-' + label + '-changed'); }
    r.preview = await ev(`const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; if (!m) return null; return Array.from(m.querySelectorAll('img, canvas')).map((e) => ({ tag: e.tagName, w: e.naturalWidth || e.width, h: e.naturalHeight || e.height, src: (e.src || '').slice(0, 40) }));`);
    const names = r.buttons.map((b) => b.text);
    const go = names.find((t) => /^(Export|Save|Copy)/i.test(t));
    r.clicked = go ? await ev(`${R} return R.click(${JSON.stringify(go)});`) : null;
    r.clickedText = go;
    r.file = await waitFile(target, 12000);
    if (r.file) r.image = pngFacts(target);
    r.newDownloads = await waitDir(DL, before, r.file ? 1500 : 12000);
    r.downloadFacts = r.newDownloads.map((n) => ({ name: n, ...pngFacts(path.join(DL, n)) }));
    r.downloads = [...downloads];
    r.dialogs = await dialogs();
    r.notices = await ev(`${R} return R.notices();`);
    r.modalsAfter = await ev(`${R} return R.modals();`);
    r.app = bytes('.obsidian/app.json');
    r.canvasJson = bytes('.obsidian/canvas.json');
    r.clipboardHasImage = await mainEval(`const { clipboard } = require('electron'); const i = clipboard.readImage(); return i.isEmpty() ? null : i.getSize();`).catch((e) => 'err ' + e.message);
    p.runs.push(r); save();
    await ev(`${R} R.closeModals(); return true;`); await sleep(600);
    return r;
  };
  await bounded(p, 'default', () => exportOnce('default'), 40000);
  const ctl = (p.runs[0] && p.runs[0].controls) || [];
  const plans = [];
  ctl.forEach((row, i) => {
    if (row.selects.length) for (const [v] of row.selects[0].options) if (v !== row.selects[0].value) plans.push([`r${i}-${v}`.replace(/[^\w-]/g, '_'), `return R.setSelect(${i}, ${JSON.stringify(v)});`]);
    if (row.toggles.length) plans.push([`r${i}-toggle`, `return R.toggle(${i});`]);
  });
  p.plans = plans.map((x) => x[0]);
  for (const [label, change] of plans.slice(0, 10)) await bounded(p, 'plan-' + label, () => exportOnce(label, change), 40000);
  // A selection of one node.
  await bounded(p, 'selection', async () => {
    await ev(`const v = app.workspace.activeLeaf.view; const cv = v.canvas; const n = cv.nodes.get('a1'); cv.deselectAll(); cv.select(n); return cv.selection.size;`);
    await sleep(500);
    return exportOnce('selection');
  }, 40000);
  // Over a note, where it is not offered: what a run does.
  await bounded(p, 'over-note', async () => { await ev(`${R} await R.open('Note.md', 'live'); return true;`); return { ran: await ev(`return app.commands.executeCommandById('canvas:export-as-image');`), modals: await ev(`${R} await R.sleep(1200); return R.modals();`) }; }, 15000);
  await ev(`${R} R.closeModals(); return true;`);
  p.canvasAfter = bytes('Board.canvas');
  return p;
}, 600000);

await step('s03-format-converter', async (p) => {
  await ev(`${R} await R.open('Short.md', 'live'); return true;`);
  p.before = snapshotNotes();
  p.dataBefore = bytes('.obsidian/markdown-importer.json');
  p.ran = await ev(`return app.commands.executeCommandById('markdown-importer:open');`); await sleep(1500);
  p.modal = await ev(`${R} return R.modal();`);
  p.controls = await ev(`${R} return R.controls();`);
  p.buttons = await ev(`${R} return R.buttons();`);
  p.shot = await shot('converter-modal');
  p.runs = [];
  const n = (p.controls || []).filter((x) => x.toggles.length).length;
  const togRows = (p.controls || []).map((x, i) => (x.toggles.length ? i : -1)).filter((i) => i >= 0);
  // Every toggle off first, then each on alone, then all on.
  const setAll = async (on) => ev(`${R} const ms = Array.from(document.querySelectorAll('.modal-container')); const m = ms[ms.length - 1]; for (const t of m.querySelectorAll('.checkbox-container')) if (t.classList.contains('is-enabled') !== ${on}) t.click(); return Array.from(m.querySelectorAll('.checkbox-container')).map((t) => t.classList.contains('is-enabled'));`);
  const reset = () => { for (const [k, v] of Object.entries(p.before)) if (k.startsWith('Convert/')) fs.writeFileSync(vp(k), v); };
  const runOnce = async (label, rows) => {
    reset(); await sleep(1500);
    if (!(await ev(`return !!document.querySelector('.modal-container');`))) { await ev(`return app.commands.executeCommandById('markdown-importer:open');`); await sleep(1200); }
    const r = { label, rows };
    r.off = await setAll(false);
    for (const i of rows) await ev(`${R} return R.toggle(${i});`);
    r.state = await ev(`${R} return R.controls().map((x) => x.toggles);`);
    const go = (await ev(`${R} return R.buttons();`)).map((b) => b.text).find((t) => /start|convert|run/i.test(t));
    r.go = go;
    if (go) await ev(`${R} return R.click(${JSON.stringify(go)});`);
    await sleep(4000);
    r.modalsAfter = await ev(`${R} return R.modals();`);
    r.notices = await ev(`${R} return R.notices();`);
    r.shot = await shot('converter-' + label);
    const now = snapshotNotes();
    r.changed = Object.fromEntries(Object.entries(now).filter(([k, v]) => p.before[k] !== v));
    r.created = listVault().map((f) => f.path).filter((f) => !(f in p.before) && !/\.(png)$/.test(f));
    r.data = bytes('.obsidian/markdown-importer.json');
    p.runs.push(r); save();
    return r;
  };
  for (const i of togRows) await bounded(p, 'row' + i, () => runOnce('row' + i, [i]), 30000);
  await bounded(p, 'all', () => runOnce('all', togRows), 30000);
  await bounded(p, 'none', () => runOnce('none', []), 30000);
  await ev(`${R} R.closeModals(); return true;`);
  reset();
  p.toggleRows = togRows; p.toggleCount = n;
  return p;
}, 600000);

await step('s04-audio', async (p) => {
  const statusBar = () => ev(`return Array.from(document.querySelectorAll('.status-bar-item')).map((e) => ({ cls: e.className, text: e.textContent.trim(), aria: e.getAttribute('aria-label') }));`);
  const ribbon = () => ev(`return Array.from(document.querySelectorAll('.side-dock-ribbon-action')).filter((e) => /record/i.test(e.getAttribute('aria-label') || '')).map((e) => ({ aria: e.getAttribute('aria-label'), cls: e.className, icon: Array.from((e.querySelector('svg') || { classList: [] }).classList).join(' ') }));`);
  const record = async (label, setup, secs = 3) => {
    const r = { label };
    if (setup) r.setup = await ev(`${R} ${setup}`);
    await sleep(800);
    const before = listVault().map((f) => f.path);
    r.noteBefore = bytes('Short.md');
    r.offeredBefore = await ev(`${R} return { start: R.offered('audio-recorder:start'), stop: R.offered('audio-recorder:stop') };`);
    r.start = await ev(`return app.commands.executeCommandById('audio-recorder:start');`);
    await sleep(1200);
    r.offeredDuring = await ev(`${R} return { start: R.offered('audio-recorder:start'), stop: R.offered('audio-recorder:stop') };`);
    r.ribbonDuring = await ribbon(); r.statusDuring = await statusBar(); r.notices = await ev(`${R} return R.notices();`);
    r.shotDuring = await shot('audio-' + label);
    await sleep(secs * 1000);
    r.stop = await ev(`return app.commands.executeCommandById('audio-recorder:stop');`);
    await sleep(3500);
    r.offeredAfter = await ev(`${R} return { start: R.offered('audio-recorder:start'), stop: R.offered('audio-recorder:stop') };`);
    r.ribbonAfter = await ribbon(); r.statusAfter = await statusBar(); r.noticesAfter = await ev(`${R} return R.notices();`);
    const now = listVault();
    r.created = now.filter((f) => !before.includes(f.path)).map((f) => ({ ...f, head: fs.readFileSync(vp(f.path)).slice(0, 4).toString('hex') }));
    r.noteAfter = bytes('Short.md');
    r.cursor = await ev(`const v = app.workspace.activeLeaf.view; return v.editor ? { cursor: v.editor.getCursor(), viewType: v.getViewType(), file: v.file && v.file.path } : { viewType: v.getViewType(), file: v.file && v.file.path };`);
    r.tabs = await ev(`return app.workspace.getLeavesOfType('audio').length + ' audio leaves; active ' + app.workspace.activeLeaf.view.getViewType();`);
    p.runs.push(r); save();
    return r;
  };
  p.runs = [];
  p.media = await ev(`try { const s = await navigator.mediaDevices.getUserMedia({ audio: true }); const t = s.getAudioTracks().map((x) => x.label); s.getTracks().forEach((x) => x.stop()); return { ok: true, tracks: t, types: ['audio/webm', 'audio/ogg', 'audio/mp4', 'audio/webm;codecs=opus'].filter((m) => MediaRecorder.isTypeSupported(m)) }; } catch (e) { return { ok: false, err: String(e) }; }`);
  p.ribbonIdle = await ribbon();
  await bounded(p, 'caret', () => record('caret', `await R.open('Short.md', 'source'); const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor(0, 3); return ed.getCursor();`), 30000);
  await bounded(p, 'selection', () => record('selection', `await R.open('Short.md', 'source'); const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setSelection({ line: 0, ch: 0 }, { line: 0, ch: 3 }); return ed.getSelection();`), 30000);
  await bounded(p, 'second-within-minute', () => record('second-within-minute', `await R.open('Short.md', 'source'); const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor(ed.lastLine(), 0); return ed.getCursor();`, 1), 30000);
  await bounded(p, 'reading', () => record('reading', `await R.open('Short.md', 'reading'); return true;`), 30000);
  await bounded(p, 'folder', () => record('folder', `app.vault.setConfig('attachmentFolderPath', 'Attachments'); await R.open('Short.md', 'source'); const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor(ed.lastLine(), 0); return app.vault.getConfig('attachmentFolderPath');`), 30000);
  await bounded(p, 'subfolder', () => record('subfolder', `app.vault.setConfig('attachmentFolderPath', './'); await R.open('Convert/Plain.md', 'source'); const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor(ed.lastLine(), 0); return app.vault.getConfig('attachmentFolderPath');`), 30000);
  await bounded(p, 'empty-tab', () => record('empty-tab', `app.vault.setConfig('attachmentFolderPath', '/'); await R.open(null); return true;`), 30000);
  await bounded(p, 'canvas', () => record('canvas', `await R.open('Board.canvas'); return true;`), 30000);
  p.convertPlain = bytes('Convert/Plain.md');
  p.canvasAfter = bytes('Board.canvas');
  // Stop with nothing recording.
  await bounded(p, 'stop-idle', async () => ({ ran: await ev(`return app.commands.executeCommandById('audio-recorder:stop');`), notices: await ev(`${R} await R.sleep(800); return R.notices();`) }), 10000);
  // The ribbon button: click it twice.
  await bounded(p, 'ribbon-click', async () => {
    await ev(`${R} await R.open('Short.md', 'source'); return true;`);
    const before = listVault().map((f) => f.path);
    const r = await ev(`const e = Array.from(document.querySelectorAll('.side-dock-ribbon-action')).find((x) => /record/i.test(x.getAttribute('aria-label') || '')); if (!e) return null; const b = e.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2, aria: e.getAttribute('aria-label') };`);
    if (!r) return null;
    await c.clickAt(r.cx, r.cy); await sleep(1500);
    const during = await ribbon(); const shotD = await shot('audio-ribbon-during');
    await sleep(2000);
    await c.clickAt(r.cx, r.cy); await sleep(3500);
    return { button: r, during, shotD, after: await ribbon(), created: listVault().filter((f) => !before.includes(f.path)), note: bytes('Short.md') };
  }, 30000);
  p.app = bytes('.obsidian/app.json');
  return p;
}, 600000);

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
