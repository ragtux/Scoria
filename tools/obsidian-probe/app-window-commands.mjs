// The app and window commands Scoria may not implement until they are watched
// ([[Development#The unrunnable commands, 2026-11-09]], "Waiting on a probe"):
//
//   s00  version, every command's palette name, callback kind and hotkeys;
//        the shell/dialog stubs installed and checked
//   s01  app:increase-font-size / app:decrease-font-size: the step from 16 px,
//        25 runs each way, where the size is kept, baseFontSizeAction off/on
//   s02  theme:toggle-light-dark from the fixture's theme (THEME_START), twice
//   s03  app:toggle-ribbon, twice: the files it writes, the ribbon's DOM
//   s04  app:toggle-default-new-pane-mode from no key, then from source, twice
//   s05  workspace:copy-path / copy-full-path / copy-url on `Sub/My Note.md`
//   s06  workspace:copy-url on a name with `&`, `#`, a non-ASCII letter, a space
//   s07  open-with-default-app:open / :show for a note, an image, no file
//   s08  workspace:show-trash under each trashOption, `.trash` absent and present
//   s09  app:show-tab-switcher and Ctrl+Tab with three tabs
//   s10  app:show-debug-info's modal
//   s11  app:show-release-notes
//   s12  app:open-file (the ipc message it sends, blocked so no dialog opens)
//   s13  editor:context-menu: the menu's place against the caret, its rows
//   s14  editor:focus from Files, a canvas, an empty tab, a reading-view note
//   s15  window:toggle-always-on-top in the vault window and in a popout
//   s16  window:zoom-in with a popout active, and the popout's `zoom` in workspace.json
//   s17  window:zoom-* in the vault window: 20 presses each way, what is written;
//        leaves zoom +1 and the font at 18 and quits
//   after-restart: whether the zoom and the font size survived
//
// Nothing may reach the desktop: electron.shell's and remote.shell's opening
// calls, remote dialogs, `window.open(_, "_external")` and the ipc messages
// `open-file`, `open-url` and `trash` are replaced by recorders before any step.
//
// Phases: fixtures | run | after-restart. ONLY=<regex> picks steps of `run`
// (fixtures always run whole). THEME_START=obsidian|moonstone|system|none sets
// the fixture's appearance.json `theme` (default none: no key).
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep, pageTargets } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const UDD = process.env.USER_DATA_DIR || path.join(process.env.HOME, '.config/obsidian');
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
const THEME_START = process.env.THEME_START || 'none';
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 540000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.appWindow = all.appWindow || {};
const runKey = phase + (process.env.RUN_LABEL ? ':' + process.env.RUN_LABEL : '');
const out = { startedAt: new Date().toISOString(), env: { ONLY: process.env.ONLY || null, THEME_START, vault: VAULT, udd: UDD }, steps: {}, order: [] };
all.appWindow[runKey] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 90000) {
  if (phase === 'run' && ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const r = {};
  out.steps[name] = { ok: null, partial: r }; save();
  try { await withTimeout(fn(r), ms, name); out.steps[name] = { ok: true, ms: Date.now() - t, ...r }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...r }; }
  save();
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const ODD = 'Sub/Café & Bar #2.md';

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('Sub'), { recursive: true });
  fs.mkdirSync(vp('img'), { recursive: true });
  fs.writeFileSync(vp('Sub/My Note.md'), '# My Note\n\nplain text here and a [[Note B]] link and more words\n\nsecond paragraph\n');
  fs.writeFileSync(vp(ODD), 'odd name\n');
  fs.writeFileSync(vp('Note A.md'), 'Alpha body\n');
  fs.writeFileSync(vp('Note B.md'), 'Bravo body\n');
  fs.writeFileSync(vp('Note C.md'), 'Charlie body\n');
  fs.writeFileSync(vp('Board.canvas'), '{"nodes":[],"edges":[]}');
  fs.writeFileSync(vp('img/pic.png'), PNG);
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  const app = THEME_START === 'none' ? {} : { theme: THEME_START };
  fs.writeFileSync(vp('.obsidian/appearance.json'), JSON.stringify(app, null, 2));
  save();
  process.exit(0);
}

// ---- disk snapshots (read on the host, never through Obsidian) -------------
const readOr = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } };
function snapshot() {
  const s = {};
  let names = [];
  try { names = fs.readdirSync(vp('.obsidian')); } catch { /* none */ }
  for (const n of names) {
    const p = vp('.obsidian/' + n);
    try { if (fs.statSync(p).isFile()) s['.obsidian/' + n] = readOr(p); } catch { /* gone */ }
  }
  s['UDD/obsidian.json'] = readOr(path.join(UDD, 'obsidian.json'));
  // The per-vault window file `<vault id>.json` of this run's vault only.
  try {
    const reg = JSON.parse(s['UDD/obsidian.json']).vaults || {};
    for (const [id, v] of Object.entries(reg)) if (v.path.replace(/\/+$/, '') === VAULT.replace(/\/+$/, '')) s['UDD/<vault id>.json'] = readOr(path.join(UDD, id + '.json'));
  } catch { /* none */ }
  // Chromium keeps per-host zoom in Preferences; keep only the zoom-related parts.
  const pref = readOr(path.join(UDD, 'Preferences'));
  if (pref) {
    try {
      const j = JSON.parse(pref);
      s['UDD/Preferences#zoom'] = JSON.stringify({ partition: j.partition || null, default_zoom_level: (j.profile || {}).default_zoom_level ?? null });
    } catch { s['UDD/Preferences#zoom'] = 'unparsable'; }
  }
  return s;
}
function diff(a, b) {
  const d = {};
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) if (a[k] !== b[k]) d[k] = { before: a[k] ?? null, after: b[k] ?? null };
  return d;
}
// Poll up to `ms` for any of `files` (or any file, when omitted) to change;
// then wait `settle` more and diff everything.
async function waitWrite(before, files, ms = 4000, settle = 800) {
  const t = Date.now();
  let changedAt = null;
  while (Date.now() - t < ms) {
    const now = snapshot();
    const d = diff(before, now);
    const keys = Object.keys(d).filter((k) => !files || files.includes(k));
    if (keys.length) { changedAt = Date.now() - t; break; }
    await sleep(100);
  }
  if (changedAt !== null) await sleep(settle);
  const after = snapshot();
  return { firstChangeMs: changedAt, waitedMs: Date.now() - t, diff: diff(before, after) };
}

if (phase === 'after-restart') {
  const c = await connect();
  out.vaultPath = await c.vaultPath();
  await step('r01-after-restart', async (r) => {
    await sleep(1500);
    r.state = await c.evaluate(`
      const e = require('electron');
      return { zoomLevel: e.webFrame.getZoomLevel(), zoomFactor: e.webFrame.getZoomFactor(),
        fontVar: getComputedStyle(document.body).getPropertyValue('--font-text-size'),
        fontInline: document.body.style.getPropertyValue('--font-text-size'),
        baseFontSize: app.getBaseFontSize(), localStorageBase: app.loadLocalStorage('base-font-size'),
        bodyCls: document.body.className, theme: app.vault.getConfig('theme') };`);
    r.disk = snapshot();
  });
  c.close();
  process.exit(0);
}

// ---- run ------------------------------------------------------------------
const c = await connect();
out.vaultPath = await c.vaultPath();
save();

// The helpers and the stubs. Every opening call is recorded into
// window.__rec and never passed on.
await c.evaluate(`
const e = require('electron');
window.__rec = [];
const rec = (kind, args) => { window.__rec.push({ kind, args: JSON.parse(JSON.stringify(args, (k, v) => typeof v === 'function' ? '[fn]' : v)), t: Date.now() }); };
const shellNames = ['openPath', 'openExternal', 'showItemInFolder', 'trashItem', 'beep'];
for (const n of shellNames) { try { if (e.shell && e.shell[n]) { e.shell[n] = (...a) => { rec('shell.' + n, a); return Promise.resolve(''); }; } } catch (err) { rec('stub-fail shell.' + n, [String(err)]); } }
// remote.* reach the main process through ipcRenderer.sendSync; catch the
// member calls by name there, which covers remote.shell and remote.dialog
// however the proxies are cached.
const BLOCK_MEMBERS = new Set(['showItemInFolder', 'openPath', 'openExternal', 'trashItem', 'showOpenDialog', 'showOpenDialogSync', 'showSaveDialog', 'showSaveDialogSync', 'showMessageBox', 'showMessageBoxSync', 'showErrorBox']);
const ipc = e.ipcRenderer;
const origSendSync = ipc.sendSync.bind(ipc);
ipc.sendSync = (ch, ...a) => {
  if (ch === 'REMOTE_BROWSER_MEMBER_CALL' && BLOCK_MEMBERS.has(a[2])) { rec('remote.' + a[2], (a[3] || []).map((x) => x && x.value !== undefined ? x.value : x)); return { type: 'value', value: a[2].endsWith('Sync') ? undefined : undefined }; }
  if (ch && String(ch).startsWith('REMOTE_')) { if (window.__logRemote) rec('remote-pass ' + ch, [a[2]]); }
  else rec('ipc.sendSync ' + ch, a);
  return origSendSync(ch, ...a);
};
const BLOCK_SEND = new Set(['open-file', 'open-url', 'trash', 'open-external', 'show-item-in-folder']);
const origSend = ipc.send.bind(ipc);
ipc.send = (ch, ...a) => { const block = BLOCK_SEND.has(ch) && !(ch === 'open-file' && window.__allowOpenFile); rec((block ? 'BLOCKED ' : '') + 'ipc.send ' + ch, a); if (block) return; return origSend(ch, ...a); };
if (ipc.invoke) { const origInvoke = ipc.invoke.bind(ipc); ipc.invoke = (ch, ...a) => { rec('ipc.invoke ' + ch, a); return origInvoke(ch, ...a); }; }
const origOpen = window.open.bind(window);
window.open = (url, target, feat) => { if (target === '_external' || /^(file:|\\/)/.test(String(url))) { rec('window.open', [url, target, feat ?? null]); return null; } rec('window.open passed', [url, target, feat ?? null]); return origOpen(url, target, feat); };
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  take() { const r = window.__rec; window.__rec = []; return r; },
  notices(doc) { return Array.from((doc || document).querySelectorAll('.notice')).map((n) => n.textContent); },
  clearNotices(doc) { (doc || document).querySelectorAll('.notice').forEach((n) => n.remove()); },
  font() { return { fontVar: getComputedStyle(document.body).getPropertyValue('--font-text-size').trim(), fontInline: document.body.style.getPropertyValue('--font-text-size'), base: app.getBaseFontSize(), ls: app.loadLocalStorage('base-font-size') }; },
  avail(id) { return app.commands.listCommands().some((k) => k.id === id); },
  run(id) { let ran = null, err = null; try { ran = app.commands.executeCommandById(id); } catch (x) { err = String(x); } return { ran, err }; },
  async open(file, leaf) {
    const f = app.vault.getAbstractFileByPath(file);
    leaf = leaf || app.workspace.getLeaf(false);
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(400);
    return leaf;
  },
  describe(el) { return el ? { tag: el.tagName, cls: el.className, text: (el.textContent || '').trim().slice(0, 160), label: el.getAttribute && el.getAttribute('aria-label') } : null; },
  active() {
    const l = app.workspace.activeLeaf; const ae = document.activeElement;
    return { leafType: l ? l.view.getViewType() : null, file: l && l.view.file ? l.view.file.path : null, mode: l && l.view.getMode ? l.view.getMode() : null,
      activeElement: ae ? { tag: ae.tagName, cls: String(ae.className).slice(0, 120), inCm: !!(ae.closest && ae.closest('.cm-editor')), inExplorer: !!(ae.closest && ae.closest('.nav-files-container, .workspace-leaf-content[data-type=file-explorer]')), leafType: ae.closest && ae.closest('.workspace-leaf-content') ? ae.closest('.workspace-leaf-content').getAttribute('data-type') : null } : null };
  },
  menu() {
    const m = document.querySelector('.menu'); if (!m) return null;
    const r = m.getBoundingClientRect();
    return { rect: { x: r.x, y: r.y, w: r.width, h: r.height }, cls: m.className,
      rows: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => x.classList.contains('menu-separator') ? (x.offsetHeight ? '---' : '(hidden ---)') :
        { title: ((x.querySelector('.menu-item-title') || x).textContent || '').trim(), icon: (x.querySelector('.menu-item-icon svg') || {}).classList ? Array.from(x.querySelector('.menu-item-icon svg').classList).join(' ') : null, section: x.getAttribute('data-section'), disabled: x.classList.contains('is-disabled'), sub: !!x.querySelector('.menu-item-icon.mod-submenu') || x.classList.contains('has-submenu') }) };
  },
  closeMenus() { document.querySelectorAll('.menu').forEach((m) => m.remove()); },
};
return true;`);

const ev = (body) => c.evaluate(body);
const take = () => ev(`return window.__p.take();`);
const notices = () => ev(`return window.__p.notices();`);
const clearNotices = () => ev(`window.__p.clearNotices(); return true;`);
const avail = (id) => ev(`return window.__p.avail(${JSON.stringify(id)});`);
const run = (id) => ev(`return window.__p.run(${JSON.stringify(id)});`);
const font = () => ev(`return window.__p.font();`);
const open = (file) => ev(`await window.__p.open(${JSON.stringify(file)}); return window.__p.active();`);
const active = () => ev(`return window.__p.active();`);
const modalCount = () => ev(`return document.querySelectorAll('.modal-container').length;`);
const dismiss = async () => {
  await ev(`window.__p.closeMenus(); return true;`);
  for (let i = 0; i < 3 && (await modalCount()) > 0; i++) { await c.pressKey('Escape'); await sleep(250); }
  const left = await modalCount();
  if (left) await ev(`document.querySelectorAll('.modal-container').forEach((m) => m.remove()); return true;`);
};
// Run a command once and record what it did around it.
async function once(id, { waitFiles, ms = 4000 } = {}) {
  await clearNotices(); await take();
  const before = snapshot();
  const a = await avail(id);
  const r = await run(id);
  await sleep(250);
  const n = await notices();
  const w = waitFiles === false ? null : await waitWrite(before, waitFiles, ms);
  return { available: a, ...r, notices: n, calls: await take(), write: w };
}

const IDS = ['app:increase-font-size', 'app:decrease-font-size', 'window:zoom-in', 'window:zoom-out', 'window:reset-zoom',
  'window:toggle-always-on-top', 'theme:toggle-light-dark', 'app:toggle-ribbon', 'app:toggle-default-new-pane-mode',
  'workspace:copy-path', 'workspace:copy-full-path', 'workspace:copy-url', 'workspace:show-trash',
  'open-with-default-app:open', 'open-with-default-app:show', 'app:show-tab-switcher', 'app:show-debug-info',
  'app:show-release-notes', 'app:open-file', 'editor:context-menu', 'editor:focus'];

await step('s00-meta', async (r) => {
  r.version = await ev(`return { app: require('electron').ipcRenderer.sendSync('version'), ua: navigator.userAgent, title: document.title, platform: process.platform };`);
  await take();
  r.commands = await ev(`
    const ids = ${JSON.stringify(IDS)}; const o = {};
    for (const id of ids) { const k = app.commands.commands[id];
      o[id] = k ? { name: k.name, icon: k.icon || null, callback: !!k.callback, checkCallback: !!k.checkCallback, editorCallback: !!k.editorCallback, editorCheckCallback: !!k.editorCheckCallback,
        defaultHotkeys: (app.hotkeyManager.defaultKeys || {})[id] || null, customHotkeys: (app.hotkeyManager.customKeys || {})[id] || null,
        inPaletteNow: window.__p.avail(id) } : null; }
    return o;`);
  r.ctrlTabOwner = await ev(`const d = app.hotkeyManager.defaultKeys || {}; return Object.entries(d).filter(([id, ks]) => ks.some((k) => k.key === 'Tab')).map(([id, ks]) => ({ id, ks }));`);
  r.stubCheck = await ev(`const e = require('electron'); return { shellOpenPathStubbed: !String(e.shell.openPath).includes('native'), sendSyncWrapped: String(e.ipcRenderer.sendSync).includes('REMOTE_BROWSER_MEMBER_CALL') };`);
  r.disk = snapshot();
});

await step('s01-font-size', async (r) => {
  await open('Note A.md');
  r.start = await font();
  r.appearanceStart = readOr(vp('.obsidian/appearance.json'));
  r.lsKeys = await ev(`return Object.keys(localStorage).filter((k) => /font/i.test(k));`);
  r.firstIncrease = await once('app:increase-font-size', { ms: 4000 });
  r.afterFirst = await font();
  r.lsKeysAfter = await ev(`return Object.keys(localStorage).filter((k) => /font/i.test(k)).map((k) => [k, localStorage.getItem(k)]);`);
  r.up = [];
  for (let i = 0; i < 25; i++) { await run('app:increase-font-size'); await sleep(60); r.up.push((await font()).fontVar); }
  r.down = [];
  for (let i = 0; i < 25; i++) { await run('app:decrease-font-size'); await sleep(60); r.down.push((await font()).fontVar); }
  const before = snapshot();
  for (let i = 0; i < 6; i++) await run('app:increase-font-size');
  r.backTo = await font();
  r.writeAfterRuns = await waitWrite(before, ['.obsidian/appearance.json'], 4000);
  // baseFontSizeAction off, then on.
  for (const v of [false, true]) {
    const b = snapshot();
    await ev(`app.vault.setConfig('baseFontSizeAction', ${v}); return true;`);
    await waitWrite(b, ['.obsidian/appearance.json'], 3000);
    const k = 'action_' + v;
    r[k] = { appearance: readOr(vp('.obsidian/appearance.json')), before: await font() };
    r[k].inc = await once('app:increase-font-size', { waitFiles: false });
    r[k].afterInc = await font();
    r[k].dec = await once('app:decrease-font-size', { waitFiles: false });
    r[k].afterDec = await font();
  }
  r.appearanceEnd = readOr(vp('.obsidian/appearance.json'));
});

await step('s02-theme-toggle', async (r) => {
  const state = () => ev(`return { theme: app.vault.getConfig('theme') ?? null, bodyDark: document.body.classList.contains('theme-dark'), bodyLight: document.body.classList.contains('theme-light'), isDark: app.customCss.isDarkMode() };`);
  r.start = THEME_START;
  r.appearanceStart = readOr(vp('.obsidian/appearance.json'));
  r.stateStart = await state();
  r.runs = [];
  for (let i = 0; i < 2; i++) {
    const o = await once('theme:toggle-light-dark', { waitFiles: ['.obsidian/appearance.json'] });
    o.appearance = readOr(vp('.obsidian/appearance.json'));
    o.state = await state();
    r.runs.push(o);
  }
});

await step('s03-ribbon', async (r) => {
  await sleep(2500);
  const dom = () => ev(`const els = Array.from(document.querySelectorAll('.workspace-ribbon'));
    return { showRibbonCfg: app.vault.getConfig('showRibbon'), bodyShowRibbon: document.body.classList.contains('show-ribbon'), bodyCls: document.body.className,
      ribbons: els.map((el) => { const cs = getComputedStyle(el); const r = el.getBoundingClientRect(); return { cls: el.className, display: cs.display, visibility: cs.visibility, x: r.x, w: r.width, h: r.height, children: el.children.length }; }) };`);
  r.start = await dom();
  r.runs = [];
  for (let i = 0; i < 2; i++) { const o = await once('app:toggle-ribbon'); o.dom = await dom(); r.runs.push(o); }
});

await step('s04-default-new-pane-mode', async (r) => {
  const cfg = () => ev(`return app.vault.getConfig('defaultViewMode') ?? null;`);
  r.start = { cfg: await cfg(), appJson: readOr(vp('.obsidian/app.json')) };
  r.fromAbsent = await once('app:toggle-default-new-pane-mode');
  r.fromAbsent.cfg = await cfg();
  const b = snapshot();
  await ev(`app.vault.setConfig('defaultViewMode', 'source'); return true;`);
  r.seedSource = await waitWrite(b, ['.obsidian/app.json'], 3000);
  r.runs = [];
  for (let i = 0; i < 2; i++) { const o = await once('app:toggle-default-new-pane-mode', { waitFiles: ['.obsidian/app.json'] }); o.cfg = await cfg(); o.appJson = readOr(vp('.obsidian/app.json')); r.runs.push(o); }
});

const clip = async (id) => {
  await c.writeClipboard('SENTINEL-' + id);
  const o = await once(id, { waitFiles: false });
  o.clipboard = await c.readClipboard();
  return o;
};

await step('s05-copy-paths', async (r) => {
  r.active = await open('Sub/My Note.md');
  r.vaultName = await ev(`return app.vault.getName();`);
  for (const id of ['workspace:copy-path', 'workspace:copy-full-path', 'workspace:copy-url']) r[id] = await clip(id);
});

await step('s06-copy-url-odd-name', async (r) => {
  r.active = await open(ODD);
  r['workspace:copy-url'] = await clip('workspace:copy-url');
  r['workspace:copy-path'] = await clip('workspace:copy-path');
  r['workspace:copy-full-path'] = await clip('workspace:copy-full-path');
  // A file at the vault root and an image, for the extension rule.
  r.rootNote = { active: await open('Note A.md'), url: await clip('workspace:copy-url') };
  r.image = { active: await open('img/pic.png'), url: await clip('workspace:copy-url'), path: await clip('workspace:copy-path') };
});

await step('s07-open-with-default-app', async (r) => {
  for (const [k, f] of [['note', 'Sub/My Note.md'], ['image', 'img/pic.png']]) {
    r[k] = { active: await open(f) };
    for (const id of ['open-with-default-app:open', 'open-with-default-app:show']) r[k][id] = await once(id, { waitFiles: false });
  }
  await ev(`app.workspace.getLeaf(true); await window.__p.sleep(400); return true;`);
  r.none = { active: await active() };
  for (const id of ['open-with-default-app:open', 'open-with-default-app:show', 'workspace:copy-path', 'workspace:copy-full-path', 'workspace:copy-url']) r.none[id] = await once(id, { waitFiles: false });
  await ev(`app.workspace.activeLeaf.detach(); return true;`);
});

await step('s08-show-trash', async (r) => {
  await open('Note A.md');
  r.defaultTrashOption = await ev(`return app.vault.getConfig('trashOption') ?? null;`);
  r.trashExistsAtStart = fs.existsSync(vp('.trash'));
  r.atDefault = await once('workspace:show-trash', { waitFiles: false });
  for (const opt of ['system', 'local', 'none']) {
    const b = snapshot();
    await ev(`app.vault.setConfig('trashOption', ${JSON.stringify(opt)}); return true;`);
    await waitWrite(b, ['.obsidian/app.json'], 3000);
    r[opt] = { appJson: readOr(vp('.obsidian/app.json')), noTrashFolder: await once('workspace:show-trash', { waitFiles: false }) };
    if (opt === 'local') {
      await ev(`await app.vault.adapter.mkdir('.trash'); return true;`);
      await sleep(400);
      r[opt].withTrashFolder = await once('workspace:show-trash', { waitFiles: false });
      await ev(`await app.vault.adapter.rmdir('.trash', true); return true;`);
      await sleep(300);
    }
  }
  r.restore = await ev(`app.vault.setConfig('trashOption', 'system'); return true;`);
});

await step('s09-tab-switcher', async (r) => {
  r.registered = await ev(`return !!app.commands.commands['app:show-tab-switcher'];`);
  r.anyIdLike = await ev(`return Object.keys(app.commands.commands).filter((k) => /switcher/i.test(k));`);
  r.ran = await run('app:show-tab-switcher');
  // Three tabs in the root, the middle one active.
  await ev(`
    const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l));
    const a = leaves[0]; for (const l of leaves.slice(1)) l.detach();
    app.workspace.setActiveLeaf(a, { focus: true });
    await a.openFile(app.vault.getAbstractFileByPath('Note A.md'));
    const b = app.workspace.getLeaf('tab'); await b.openFile(app.vault.getAbstractFileByPath('Note B.md'));
    const c = app.workspace.getLeaf('tab'); await c.openFile(app.vault.getAbstractFileByPath('Note C.md'));
    // Strip order A B C; most recent C, then A, then B (active), so the strip's
    // next (C) and the most recent other tab (A) differ.
    app.workspace.setActiveLeaf(c, { focus: true }); await window.__p.sleep(300);
    app.workspace.setActiveLeaf(a, { focus: true }); await window.__p.sleep(300);
    app.workspace.setActiveLeaf(b, { focus: true }); await window.__p.sleep(300);
    return true;`);
  const st = () => ev(`const l = app.workspace.activeLeaf; return { active: l.view.file ? l.view.file.path : l.view.getViewType(),
    strip: Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')).map((h) => h.getAttribute('aria-label') + (h.classList.contains('is-active') ? '*' : '')),
    overlays: Array.from(document.querySelectorAll('.modal-container, .menu, .suggestion-container, [class*=switcher]')).map((e) => e.className) };`);
  r.start = await st();
  // Ctrl held across two Tabs, then released.
  const send = (type, key, code, vk, mods) => c.send('Input.dispatchKeyEvent', { type, key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk, modifiers: mods });
  await send('rawKeyDown', 'Control', 'ControlLeft', 17, 2);
  await send('rawKeyDown', 'Tab', 'Tab', 9, 2); await send('keyUp', 'Tab', 'Tab', 9, 2); await sleep(300);
  r.afterCtrlTab1 = await st();
  await send('rawKeyDown', 'Tab', 'Tab', 9, 2); await send('keyUp', 'Tab', 'Tab', 9, 2); await sleep(300);
  r.afterCtrlTab2 = await st();
  await send('keyUp', 'Control', 'ControlLeft', 17, 0); await sleep(300);
  r.afterRelease = await st();
  await send('rawKeyDown', 'Control', 'ControlLeft', 17, 10);
  await send('rawKeyDown', 'Tab', 'Tab', 9, 10); await send('keyUp', 'Tab', 'Tab', 9, 10); await sleep(300);
  await send('keyUp', 'Control', 'ControlLeft', 17, 0); await sleep(300);
  r.afterCtrlShiftTab = await st();
  await dismiss();
});

await step('s10-debug-info', async (r) => {
  r.run = await run('app:show-debug-info');
  await sleep(600);
  r.modal = await ev(`const m = document.querySelector('.modal-container .modal'); if (!m) return null;
    const t = m.querySelector('.modal-title'); const content = m.querySelector('.modal-content');
    const ta = content && content.querySelector('textarea'); const text = ta ? ta.value : (content ? content.innerText : '');
    return { cls: m.className, title: t ? t.textContent : null,
      labels: text.split('\\n').map((l) => l.replace(/:.*$/, ':').trim()).filter(Boolean),
      lineCount: text.split('\\n').length,
      structure: content ? Array.from(content.querySelectorAll('*')).slice(0, 60).map((e) => e.tagName + '.' + e.className) : [],
      buttons: Array.from(m.querySelectorAll('button')).map((b) => ({ text: b.textContent, cls: b.className })),
      closeButton: !!m.querySelector('.modal-close-button') };`);
  // Values that cannot be private: the version line and the plugin lines.
  r.textareaShape = await ev(`const t = document.querySelector('.modal-container .modal textarea'); if (!t) return null; const v = t.value;
    return { lines: v.split('\\n').length, endsWithNewline: v.endsWith('\\n'), readOnly: t.readOnly, rows: t.rows, indentedLines: v.split('\\n').filter((l) => /^\\s/.test(l)).map((l) => l.replace(/:.*$/, ':')),
      versionLines: v.split('\\n').filter((l) => /version/i.test(l)) };`);
  r.calls = await take();
  await dismiss();
  r.modalsLeft = await modalCount();
});

await step('s11-release-notes', async (r) => {
  const strip = `return Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')).map((h) => (h.getAttribute('data-type') || '') + ':' + h.getAttribute('aria-label') + (h.classList.contains('is-active') ? '*' : ''));`;
  const before = await ev(strip);
  r.run = await run('app:show-release-notes');
  await sleep(2500);
  r.leaf = await ev(`const l = app.workspace.activeLeaf; const s = l.getViewState();
    return { viewType: l.view.getViewType(), state: s, displayText: l.view.getDisplayText(), icon: l.view.getIcon ? l.view.getIcon() : null,
      tabTitle: (l.tabHeaderInnerTitleEl || {}).textContent || null, headerTitle: (l.view.titleEl || {}).textContent || null,
      contentHead: (l.view.contentEl.innerText || '').slice(0, 300), contentCls: l.view.contentEl.className };`);
  r.leavesBefore = before;
  r.leavesAfter = await ev(strip);
  r.calls = await take();
  r.notices = await notices();
  await ev(`app.workspace.activeLeaf.detach(); return true;`);
});

await step('s12-open-file', async (r) => {
  await open('Note A.md');
  r.once = await once('app:open-file', { waitFiles: false });
  r.modalsAfter = await modalCount();
  await dismiss();
});

// s12b: only with OPEN_FILE_REAL=1 and WTYPE naming a wtype binary, in a
// private headless compositor with no session bus (so Electron falls back to
// its GTK dialog, which opens in that compositor and nowhere else). The
// dialog is answered with real keys: Ctrl+L, the path, Enter. On 2026-11-09
// the GTK 4 dialog ("Open File", filter "Supported files") opened and took
// Ctrl+L and the path, but never acted on Return from wtype's virtual
// keyboard, so what an accepted path does stayed unobserved. The screenshots
// (GRIM, SHOTS) show the dialog's file list, i.e. a folder of the person's:
// keep them out of the repository.
const host = await import('node:child_process');
const wtype = (args) => host.execFileSync(process.env.WTYPE, args, { env: { ...process.env }, timeout: 10000 });
const grim = (name) => { if (!process.env.GRIM || !process.env.SHOTS) return null; const f = path.join(process.env.SHOTS, name); try { host.execFileSync(process.env.GRIM, [f], { timeout: 10000 }); return f; } catch (e) { return String(e); } };
if (process.env.OPEN_FILE_REAL && process.env.WTYPE) {
  const outside = path.join(path.dirname(VAULT), 'Outside Folder');
  fs.mkdirSync(outside, { recursive: true });
  fs.writeFileSync(path.join(outside, 'Outside Note.md'), 'outside body\n');
  fs.writeFileSync(path.join(outside, 'outside.png'), PNG);
  const listVault = () => { const o = []; const walk = (d, rel) => { for (const n of fs.readdirSync(d)) { if (n === '.obsidian') continue; const p = path.join(d, n); const r = rel ? rel + '/' + n : n; if (fs.statSync(p).isDirectory()) walk(p, r); else o.push(r); } }; walk(VAULT, ''); return o.sort(); };
  for (const [label, target] of [['inside', vp('Note B.md')], ['outsideNote', path.join(outside, 'Outside Note.md')], ['outsideImage', path.join(outside, 'outside.png')]]) {
    await step('s12b-open-file-' + label, async (r) => {
      await ev(`window.__allowOpenFile = true; return true;`);
      await open('Note A.md');
      r.target = target;
      r.before = { vault: listVault(), targets: (await pageTargets()).length, disk: snapshot() };
      r.run = await run('app:open-file');
      await sleep(2500);
      r.shotDialog = grim('open-file-' + label + '-dialog.png');
      // One wtype invocation: a fresh virtual keyboard per call lost the
      // first key or the last one to GTK's dialog in earlier tries.
      wtype(['-s', '300', '-M', 'ctrl', '-k', 'l', '-m', 'ctrl', '-s', '400', target, '-s', '400', '-k', 'Return', '-s', '300']);
      await sleep(3000);
      r.shotTyped = null;
      r.shotAfter = grim('open-file-' + label + '-after.png');
      r.after = { vault: listVault(), targets: (await pageTargets()).length,
        active: await ev(`const l = app.workspace.activeLeaf; return { type: l.view.getViewType(), file: l.view.file ? l.view.file.path : null, title: l.view.getDisplayText(), state: l.getViewState() };`),
        strip: await ev(`return Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')).map((h) => h.getAttribute('aria-label') + (h.classList.contains('is-active') ? '*' : ''));`),
        notices: await notices(), modals: await modalCount() };
      r.diskDiff = diff(r.before.disk, snapshot()); delete r.before.disk;
      r.calls = await take();
      await ev(`window.__allowOpenFile = false; return true;`);
      await dismiss();
    });
  }
}

await step('s13-context-menu', async (r) => {
  await ev(`const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l)); for (const l of leaves.slice(1)) l.detach(); return true;`);
  await open('Sub/My Note.md');
  const setMode = (mode, source) => ev(`const l = app.workspace.activeLeaf; const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: ${JSON.stringify(mode)}, source: ${source} }); await l.setViewState(s); await window.__p.sleep(400); return l.view.getMode();`);
  const caseRun = async (label, setup) => {
    await dismiss();
    const o = {};
    o.setup = await ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ${setup}; await window.__p.sleep(200);
      const cm = ed.cm; const head = cm.state.selection.main.head; const co = cm.coordsAtPos(head);
      return { cursor: ed.getCursor(), from: ed.getCursor('from'), to: ed.getCursor('to'), sel: ed.getSelection(), coords: co ? { left: co.left, right: co.right, top: co.top, bottom: co.bottom } : null, zoom: require('electron').webFrame.getZoomLevel() };`);
    o.available = await avail('editor:context-menu');
    o.run = await run('editor:context-menu');
    await sleep(700);
    o.menu = await ev(`return window.__p.menu();`);
    o.after = await ev(`const ed = app.workspace.activeLeaf.view.editor; return { cursor: ed.getCursor(), sel: ed.getSelection() };`);
    o.calls = await take();
    await dismiss();
    r[label] = o;
  };
  r.modeLP = await setMode('source', false);
  await caseRun('lpNoSelection', `ed.setCursor({ line: 2, ch: 3 })`);
  await caseRun('lpSelection', `ed.setSelection({ line: 2, ch: 0 }, { line: 2, ch: 10 })`);
  await caseRun('lpInLink', `ed.setCursor({ line: 2, ch: 26 })`);
  await caseRun('lpEndOfDoc', `ed.setCursor({ line: 4, ch: 16 })`);
  r.modeSource = await setMode('source', true);
  await caseRun('sourceNoSelection', `ed.setCursor({ line: 2, ch: 3 })`);
  r.modePreview = await setMode('preview', false);
  r.preview = { available: await avail('editor:context-menu'), run: await run('editor:context-menu') };
  await sleep(500);
  r.preview.menu = await ev(`return window.__p.menu();`);
  await dismiss();
  await setMode('source', false);
});

await step('s14-editor-focus', async (r) => {
  const blur = `document.activeElement && document.activeElement.blur && document.activeElement.blur();`;
  const fcase = async (label) => {
    const o = { before: await active(), available: await avail('editor:focus') };
    o.run = await run('editor:focus');
    await sleep(300);
    o.after = await active();
    o.mdFocus = await ev(`return app.workspace.getLeavesOfType('markdown').map((l) => ({ file: l.view.file && l.view.file.path, hasFocus: l.view.editor ? l.view.editor.hasFocus() : null, visible: !!l.view.containerEl.offsetParent }));`);
    r[label] = o;
  };
  // Note A in the one root tab, focus moved to the Files pane.
  await ev(`const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l)); for (const l of leaves.slice(1)) l.detach(); return true;`);
  await open('Note A.md');
  await ev(`app.workspace.leftSplit.expand(); return true;`);
  await run('file-explorer:reveal-active-file');
  await sleep(400);
  // Real DOM focus in the tree: a click on the empty space below the rows.
  const fr = await c.rect('.nav-files-container');
  if (fr) await c.clickAt(fr.cx, fr.y + fr.h - 10);
  await sleep(300);
  r.explorerFocused = await active();
  await fcase('fromFiles');
  // A canvas in a second tab, active, with Note A behind it.
  await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Board.canvas')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(500); return true;`);
  await fcase('fromCanvas');
  // An empty tab active, Note A behind it.
  await ev(`const l = app.workspace.getLeaf('tab'); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(400); return true;`);
  await fcase('fromEmptyTab');
  // Only empty tabs and a canvas: no markdown leaf anywhere.
  await ev(`for (const l of app.workspace.getLeavesOfType('markdown')) l.detach(); await window.__p.sleep(300); return true;`);
  r.markdownLeavesLeft = await ev(`return app.workspace.getLeavesOfType('markdown').length;`);
  await fcase('noMarkdownLeaf');
  // A reading-view note active.
  await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Note B.md')); const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'preview' }); await l.setViewState(s); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(500); ${blur} return l.view.getMode();`);
  await fcase('fromReadingView');
  // Two markdown leaves, the older one in another tab: which gets focus.
  await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Note C.md')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(300);
    const cv = app.workspace.getLeavesOfType('canvas')[0]; app.workspace.setActiveLeaf(cv, { focus: true }); await window.__p.sleep(400); return true;`);
  await fcase('canvasWithTwoMarkdownBehind');
  await ev(`for (const t of ['canvas', 'empty']) for (const l of app.workspace.getLeavesOfType(t)) l.detach(); return true;`);
});

// ---- popouts ----------------------------------------------------------------
async function popoutClient() {
  for (const url of await pageTargets()) {
    try {
      const pc = await connect(url);
      const isPop = await pc.evaluate(`return document.body.classList.contains('is-popout-window');`).catch(() => false);
      if (isPop) return pc;
      pc.close();
    } catch { /* next */ }
  }
  return null;
}

await step('s15-always-on-top', async (r) => {
  await ev(`const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l)); for (const l of leaves.slice(1)) l.detach(); return true;`);
  await open('Note A.md');
  r.mainWindow = await once('window:toggle-always-on-top', { waitFiles: false });
  r.mainState = await ev(`return { isAlwaysOnTop: electronWindow.isAlwaysOnTop(), isMaximized: electronWindow.isMaximized(), activeIsMain: activeWindow === window };`);
  // A popout with Note C in it.
  await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Note C.md')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(300); return true;`);
  r.move = await run('workspace:move-to-new-window');
  await sleep(2500);
  r.floating = await ev(`const fs = app.workspace.floatingSplit; return fs ? fs.children.length : 0;`);
  const pc = await popoutClient();
  r.popoutTarget = !!pc;
  if (pc) {
    const pr = await pc.rect('.cm-content');
    if (pr) await pc.clickAt(pr.cx, pr.cy);
    await sleep(500);
    pc.close();
  }
  r.activeAfterClick = await ev(`const w = app.workspace.floatingSplit.children[0].win; return { activeIsPopout: activeWindow === w, activeIsMain: activeWindow === window, hasFocus: w.document.hasFocus() };`);
  if (!r.activeAfterClick.activeIsPopout) { await ev(`app.workspace.floatingSplit.children[0].win.focus(); await window.__p.sleep(500); return true;`); r.activeAfterFocus = await ev(`const w = app.workspace.floatingSplit.children[0].win; return { activeIsPopout: activeWindow === w };`); }
  r.popoutBefore = await ev(`const w = app.workspace.floatingSplit.children[0].win; const ew = w.electronWindow; return { isAlwaysOnTop: ew.isAlwaysOnTop(), isMaximized: ew.isMaximized(), sameAsMain: ew === electronWindow };`);
  r.popoutRuns = [];
  for (let i = 0; i < 2; i++) {
    const o = await once('window:toggle-always-on-top', { ms: 3000 });
    o.state = await ev(`const w = app.workspace.floatingSplit.children[0].win; return { popout: w.electronWindow.isAlwaysOnTop(), main: electronWindow.isAlwaysOnTop() };`);
    o.popoutNotices = await ev(`return window.__p.notices(app.workspace.floatingSplit.children[0].win.document);`);
    r.popoutRuns.push(o);
  }
  // A maximized popout: is the command still offered?
  r.maximize = await ev(`const w = app.workspace.floatingSplit.children[0].win; w.electronWindow.maximize(); await window.__p.sleep(800);
    return { isMaximized: w.electronWindow.isMaximized(), available: window.__p.avail('window:toggle-always-on-top'), activeIsPopout: activeWindow === w };`).catch((e) => String(e));
  await ev(`const w = app.workspace.floatingSplit.children[0].win; w.electronWindow.unmaximize(); await window.__p.sleep(500); return true;`).catch(() => null);
});

await step('s16-popout-zoom', async (r) => {
  const z = () => ev(`const fs = app.workspace.floatingSplit; const w = fs && fs.children[0] ? fs.children[0].win : null;
    return { main: require('electron').webFrame.getZoomLevel(), mainFactor: require('electron').webFrame.getZoomFactor(),
      popout: w && w.electron ? w.electron.webFrame.getZoomLevel() : (w ? 'no electron on popout' : 'no popout'),
      popoutDpr: w ? w.devicePixelRatio : null, mainDpr: window.devicePixelRatio, activeIsPopout: !!w && activeWindow === w };`);
  r.start = await z();
  const b = snapshot();
  r.run = await run('window:zoom-in');
  await sleep(500);
  r.afterCommand = await z();
  r.calls = await take();
  r.write = await waitWrite(b, ['.obsidian/workspace.json'], 4000);
  await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`);
  await sleep(2500);
  const wj = readOr(vp('.obsidian/workspace.json'));
  r.workspaceFloating = (() => { try { return JSON.parse(wj).floating || null; } catch { return 'unparsable'; } })();
  // The hotkey pressed inside the popout itself.
  const pc = await popoutClient();
  if (pc) {
    const k = await pc.evaluate(`return null;`);
    await pc.pressKey('=', { ctrl: true }); await sleep(500);
    r.afterCtrlEqualInPopout = await z();
    await pc.pressKey('0', { ctrl: true }); await sleep(500);
    r.afterCtrl0InPopout = await z();
    pc.close();
  }
  await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`);
  await sleep(2500);
  try { r.workspaceFloatingAfterKeys = JSON.parse(readOr(vp('.obsidian/workspace.json'))).floating || null; } catch { r.workspaceFloatingAfterKeys = 'unparsable'; }
  // Not a command: the popout's own frame zoomed through its electronWindow,
  // to see what the window node's `zoom` records.
  r.direct = await ev(`const w = app.workspace.floatingSplit.children[0].win; w.electronWindow.setFrameZoomLevel(0.5); await window.__p.sleep(500);
    return { popoutDpr: w.devicePixelRatio, mainDpr: window.devicePixelRatio };`).catch((e) => String(e));
  await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`);
  await sleep(2500);
  try { const f = JSON.parse(readOr(vp('.obsidian/workspace.json'))).floating; r.workspaceFloatingAfterDirect = f ? f.children.map((x) => ({ zoom: x.zoom, x: x.x, y: x.y, width: x.width, height: x.height, maximize: x.maximize })) : null; } catch { r.workspaceFloatingAfterDirect = 'unparsable'; }
  await ev(`const w = app.workspace.floatingSplit.children[0].win; w.electronWindow.setFrameZoomLevel(0); return true;`).catch(() => null);
  // Back to level 0 in both, close the popout.
  await ev(`window.focus(); return true;`);
  r.reset = await run('window:reset-zoom');
  await ev(`const w = app.workspace.floatingSplit.children[0] && app.workspace.floatingSplit.children[0].win; if (w && w.electron) w.electronWindow.setFrameZoomLevel(0); return true;`).catch(() => null);
  await sleep(400);
  r.afterReset = await z();
  await ev(`const w = app.workspace.floatingSplit.children[0].win; w.close(); await window.__p.sleep(1000); return true;`).catch(() => null);
  r.floatingLeft = await ev(`return app.workspace.floatingSplit.children.length;`);
});

await step('s17-zoom-main', async (r) => {
  await ev(`window.focus(); return true;`);
  await open('Note A.md');
  const z = () => ev(`const w = require('electron').webFrame; return [w.getZoomLevel(), Math.round(w.getZoomFactor() * 10000) / 10000];`);
  r.start = await z();
  await sleep(2500);
  const s0 = snapshot();
  r.windowFileAtStart = s0['UDD/<vault id>.json'];
  r.firstIn = await once('window:zoom-in', { ms: 4000 });
  r.afterFirstIn = await z();
  r.in = [];
  for (let i = 0; i < 20; i++) { await run('window:zoom-in'); await sleep(120); r.in.push(await z()); }
  r.out = [];
  for (let i = 0; i < 40; i++) { await run('window:zoom-out'); await sleep(120); r.out.push(await z()); }
  r.reset = await once('window:reset-zoom', { waitFiles: false });
  r.afterReset = await z();
  // The hotkeys, from the note.
  const keyZ = [];
  for (const [k, m] of [['=', { ctrl: true }], ['=', { ctrl: true }], ['-', { ctrl: true }], ['0', { ctrl: true }]]) { await c.pressKey(k, m); await sleep(300); keyZ.push([k, await z()]); }
  r.keys = keyZ;
  r.writeAll = diff(s0, snapshot());
  // Leave zoom +1.0 and the font at 18, then quit for after-restart.
  await run('window:zoom-in'); await run('window:zoom-in');
  await run('app:increase-font-size'); await run('app:increase-font-size');
  await sleep(300);
  r.beforeQuit = { zoom: await z(), font: await font() };
  const s1 = snapshot();
  await sleep(3000);
  r.writeBeforeQuit = diff(s1, snapshot());
  r.diskBeforeQuit = snapshot();
  r.windowFileBeforeQuit = r.diskBeforeQuit['UDD/<vault id>.json'];
}, 120000);

save();
if (!ONLY || ONLY.test('s17-zoom-main')) {
  if (process.env.NO_QUIT) { c.close(); process.exit(0); }
  out.quitting = true; save();
  try { await c.quit(); out.quit = true; } catch (e) { out.quit = String(e); }
  await sleep(1500);
  out.diskAfterQuit = snapshot();
  save();
  process.exit(0);
}
c.close();
process.exit(0);
