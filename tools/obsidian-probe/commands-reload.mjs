// The last ids Scoria does not run, and app:reload.
//
//   s00  version; for each id the 2026-11-15 count left: registered on this
//        desktop build (`app.commands.commands`), its callback kind, whether
//        the palette offers it with a note in Source mode and the caret in a
//        table cell; every registered id; the internal plugin ids
//   s01  the main process's application menu (label, role, accelerator): what
//        Ctrl+= / Ctrl+- / Ctrl+0 reach, if anything
//   s02  app:reload: two groups, three tabs, a history, a scroll and a caret,
//        the search pane's query, a zoom step, a font-size step, a popout; an
//        external edit of app.json and appearance.json (a snippet turned on)
//        while it runs; then the command, and the same reads afterwards
//   s03  app:reload two hundred ms after an edit: does the edit reach the disk,
//        and does Ctrl+Z still undo it afterwards
//
// Needs OBSIDIAN_ARGS="--inspect=<MAIN_PORT>" and MAIN_PORT for s01.
// Phases: fixtures | run. ONLY=<regex> picks steps.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep, pageTargets, waitForObsidian } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const MAIN_PORT = process.env.MAIN_PORT;
const UDD = process.env.USER_DATA_DIR || path.join(process.env.HOME, '.config/obsidian');
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all[phase + (process.env.RUN_LABEL ? ':' + process.env.RUN_LABEL : '')] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now();
  const r = {};
  try { await withTimeout(fn(r), ms, name); out.steps[name] = { ok: true, ms: Date.now() - t, ...r }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...r }; }
  save();
  console.error(`[reload] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}
const vp = (rel) => path.join(VAULT, rel);
const readOr = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } };

const LONG = Array.from({ length: 200 }, (_, i) => `Line ${i + 1} of the long note, with some words.`).join('\n') + '\n';
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian/snippets'), { recursive: true });
  fs.writeFileSync(vp('Long.md'), LONG);
  fs.writeFileSync(vp('Table.md'), '# Table\n\n| a | b |\n| :-- | --: |\n| 1 | 2 |\n| 3 | 4 |\n');
  fs.writeFileSync(vp('Other.md'), '# Other\n\nThe other note with a needle in it.\n');
  fs.writeFileSync(vp('Third.md'), 'Third\n');
  fs.writeFileSync(vp('Pop.md'), 'In the popout\n');
  fs.writeFileSync(vp('Edit.md'), 'Before\n');
  fs.writeFileSync(vp('.obsidian/snippets/probe.css'), 'body { --probe-var: 7px; }\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/appearance.json'), '{}');
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

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

let c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { out.error = 'wrong vault'; save(); process.exit(3); }
const ev = (body) => c.evaluate(body);

const IDS = [
  'app:reload', 'app:show-tab-switcher', 'editor:configure-toolbar', 'editor:copy', 'editor:cut', 'editor:paste',
  'editor:undo', 'editor:redo', 'editor:go-start', 'editor:go-end', 'editor:move-caret-up', 'editor:move-caret-down',
  'editor:move-caret-left', 'editor:move-caret-right', 'editor:toggle-keyboard', 'editor:table-col-duplicate',
  'editor:table-row-duplicate', 'editor:table-row-copy', 'editor:table-col-copy', 'markdown-importer:open',
  'mobile:quick-action', 'window:zoom-in', 'window:zoom-out', 'window:reset-zoom', 'window:toggle-always-on-top',
];

await step('s00-registry', async (r) => {
  r.title = await ev(`return document.title;`);
  r.apiVersion = await ev(`try { return require('obsidian').apiVersion; } catch { return null; }`).catch(() => null);
  r.isMobile = await ev(`return app.isMobile;`);
  await ev(`const f = app.vault.getAbstractFileByPath('Table.md'); const l = app.workspace.getLeaf(false); await l.openFile(f, { state: { mode: 'source', source: true } });
    app.workspace.setActiveLeaf(l, { focus: true }); await new Promise((r) => setTimeout(r, 800));
    const e = app.workspace.activeEditor.editor; e.focus(); e.setCursor({ line: 4, ch: 2 }); await new Promise((r) => setTimeout(r, 300)); return true;`);
  r.ids = await ev(`const ids = ${JSON.stringify(IDS)}; const listed = new Set(app.commands.listCommands().map((c) => c.id));
    return ids.map((id) => { const c = app.commands.commands[id] || app.commands.editorCommands?.[id];
      return { id, registered: !!app.commands.commands[id], inEditorCommands: !!(app.commands.editorCommands && app.commands.editorCommands[id]),
        name: c ? c.name : null, kinds: c ? ['callback', 'checkCallback', 'editorCallback', 'editorCheckCallback'].filter((k) => typeof c[k] === 'function') : [],
        listed: listed.has(id), mobileOnly: c ? !!c.mobileOnly : null, defaultKeys: app.hotkeyManager.defaultKeys[id] || null }; });`);
  r.commandsOwnKeys = await ev(`return Object.keys(app.commands);`);
  r.allIds = await ev(`return Object.keys(app.commands.commands).sort();`);
  r.internalPlugins = await ev(`return Object.keys(app.internalPlugins.plugins).sort();`);
});

await step('s01-menu', async (r) => {
  if (!MAIN_PORT) { r.skipped = 'no MAIN_PORT'; return; }
  r.menu = await mainEval(`const { Menu, BrowserWindow } = require('electron');
    const walk = (m) => m ? m.items.map((i) => ({ label: i.label, role: i.role || null, accelerator: i.accelerator || null, id: i.id || null, visible: i.visible, sub: i.submenu ? walk(i.submenu) : null })) : null;
    const wins = BrowserWindow.getAllWindows().map((w) => ({ title: w.getTitle(), menuBarVisible: w.isMenuBarVisible(), autoHide: w.isMenuBarAutoHide(), zoom: w.webContents.getZoomLevel() }));
    return { app: walk(Menu.getApplicationMenu()), wins };`);
});

const LAYOUT = `const groups = []; const seen = new Set();
  app.workspace.iterateRootLeaves((l) => { const p = l.parent; if (!seen.has(p)) { seen.add(p); groups.push(p); } });
  const leafInfo = (l) => { const s = l.getViewState(); const v = l.view; let ed = null;
    if (v && v.editor) { const e = v.editor; ed = { cursor: e.getCursor(), scroll: e.getScrollInfo().top }; }
    let preview = null; if (v && v.previewMode && v.getMode && v.getMode() === 'preview') preview = v.previewMode.getScroll();
    return { type: s.type, file: s.state && s.state.file, mode: s.state && s.state.mode, ed, preview, active: app.workspace.activeLeaf === l,
      backHistory: l.history ? l.history.backHistory.map((h) => h.state && h.state.state && h.state.state.file) : null, ephemeral: v && v.getEphemeralState ? v.getEphemeralState() : null }; };
  const gs = groups.map((g) => ({ tabs: g.children.map(leafInfo), current: g.currentTab }));
  const search = app.workspace.getLeavesOfType('search')[0];
  const fl = app.workspace.floatingSplit;
  return { groups: gs, search: search ? { query: search.view.getQuery ? search.view.getQuery() : null, visible: search.view.containerEl.isShown() } : null,
    leftWidth: app.workspace.leftSplit.size, leftCollapsed: app.workspace.leftSplit.collapsed, rightCollapsed: app.workspace.rightSplit.collapsed,
    popouts: fl ? fl.children.length : 0, zoom: require('electron').webFrame.getZoomLevel(), dpr: window.devicePixelRatio,
    font: getComputedStyle(document.body).getPropertyValue('--font-text-size'), probeVar: getComputedStyle(document.body).getPropertyValue('--probe-var'),
    showLineNumber: app.vault.getConfig('showLineNumber'), snippets: app.customCss.enabledSnippets ? [...app.customCss.enabledSnippets] : null,
    lineNumbersInDom: !!document.querySelector('.cm-lineNumbers'), lastOpen: app.workspace.getLastOpenFiles().slice(0, 5) };`;

async function reconnectAfterReload() {
  try { c.close(); } catch { /* gone */ }
  await sleep(3000);
  const url = await waitForObsidian(60000);
  c = await connect(url);
  await sleep(2500);
}

await step('s02-reload', async (r) => {
  await ev(`const open = async (l, p, st) => { await l.openFile(app.vault.getAbstractFileByPath(p), st ? { state: st } : undefined); };
    const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l)); for (const l of leaves.slice(1)) l.detach();
    const a = app.workspace.getLeaf(false);
    await open(a, 'Third.md', { mode: 'source', source: true });
    await open(a, 'Long.md', { mode: 'source', source: true });
    const t = app.workspace.getLeaf('tab'); await open(t, 'Third.md');
    const s = app.workspace.getLeaf('split', 'vertical'); await open(s, 'Other.md', { mode: 'preview' });
    app.workspace.setActiveLeaf(a, { focus: true }); await new Promise((r) => setTimeout(r, 600));
    const e = a.view.editor; e.setCursor({ line: 120, ch: 5 }); e.scrollTo(null, 2000); await new Promise((r) => setTimeout(r, 400));
    const p = app.workspace.openPopoutLeaf(); await open(p, 'Pop.md'); await new Promise((r) => setTimeout(r, 1500));
    app.workspace.setActiveLeaf(a, { focus: true });
    return true;`);
  await ev(`const sl = app.workspace.getLeavesOfType('search')[0]; if (sl) { app.workspace.revealLeaf(sl); sl.view.setQuery('needle'); } return !!sl;`);
  await ev(`window.focus(); app.commands.executeCommandById('window:zoom-in'); app.commands.executeCommandById('app:increase-font-size'); await new Promise((r) => setTimeout(r, 600)); return true;`);
  await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); await new Promise((r) => setTimeout(r, 2500)); return true;`);
  r.before = await ev(LAYOUT);
  r.workspaceBefore = readOr(vp('.obsidian/workspace.json'));
  // External edits while the app runs.
  fs.writeFileSync(vp('.obsidian/app.json'), JSON.stringify({ showLineNumber: true }, null, 2));
  fs.writeFileSync(vp('.obsidian/appearance.json'), JSON.stringify({ enabledCssSnippets: ['probe'] }, null, 2));
  await sleep(3000);
  r.beforeReloadAfterExternal = await ev(LAYOUT);
  r.workspaceAtReload = readOr(vp('.obsidian/workspace.json'));
  r.run = await ev(`setTimeout(() => app.commands.executeCommandById('app:reload'), 100); return true;`).catch((e) => String(e));
  await reconnectAfterReload();
  r.targets = (await pageTargets()).length;
  r.after = await ev(LAYOUT);
  r.workspaceAfter = readOr(vp('.obsidian/workspace.json'));
  r.workspaceChangedByReload = r.workspaceAfter !== r.workspaceAtReload;
  r.appJsonAfter = readOr(vp('.obsidian/app.json'));
  r.appearanceAfter = readOr(vp('.obsidian/appearance.json'));
  r.windowFile = (() => { try { const reg = JSON.parse(readOr(path.join(UDD, 'obsidian.json'))).vaults; for (const [id, v] of Object.entries(reg)) if (v.path.replace(/\/+$/, '') === VAULT.replace(/\/+$/, '')) return readOr(path.join(UDD, id + '.json')); } catch { /* none */ } return null; })();
});

await step('s03-reload-unsaved', async (r) => {
  await ev(`const fl = app.workspace.floatingSplit; for (const w of [...(fl ? fl.children : [])]) { const win = w.win; setTimeout(() => win.close(), 50); } await new Promise((r) => setTimeout(r, 1500)); window.focus(); return true;`);
  r.popoutsLeft = await ev(`return app.workspace.floatingSplit ? app.workspace.floatingSplit.children.length : 0;`);
  await ev(`const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l)); for (const l of leaves.slice(1)) l.detach();
    const a = app.workspace.getLeaf(false); await a.openFile(app.vault.getAbstractFileByPath('Edit.md'), { state: { mode: 'source', source: true } });
    app.workspace.setActiveLeaf(a, { focus: true }); await new Promise((r) => setTimeout(r, 1500)); return true;`);
  r.diskBefore = readOr(vp('Edit.md'));
  await ev(`const e = app.workspace.activeEditor.editor; e.focus(); e.setCursor({ line: 0, ch: 6 }); return true;`);
  await c.typeText(' typed', 20);
  r.bufferBeforeReload = await ev(`return { file: app.workspace.activeEditor.file.path, text: app.workspace.activeEditor.editor.getValue() };`);
  r.diskJustBefore = readOr(vp('Edit.md'));
  await ev(`setTimeout(() => app.commands.executeCommandById('app:reload'), 50); return true;`).catch((e) => String(e));
  await reconnectAfterReload();
  r.diskAfter = readOr(vp('Edit.md'));
  r.bufferAfter = await ev(`const a = app.workspace.activeEditor; return a && a.editor ? { file: a.file.path, text: a.editor.getValue() } : null;`);
  await ev(`const e = app.workspace.activeEditor.editor; e.focus(); return true;`);
  await c.pressKey('z', { ctrl: true });
  await sleep(400);
  r.bufferAfterCtrlZ = await ev(`const e = app.workspace.activeEditor && app.workspace.activeEditor.editor; return e ? e.getValue() : null;`);
  await sleep(2500);
  r.diskAtEnd = readOr(vp('Edit.md'));
});

await step('s04-menu-items', async (r) => {
  if (!MAIN_PORT) { r.skipped = 'no MAIN_PORT'; return; }
  await ev(`require('electron').webFrame.setZoomLevel(0); window.focus(); return true;`);
  const click = (path) => mainEval(`const { Menu, BrowserWindow } = require('electron');
    let m = Menu.getApplicationMenu(); let item = null;
    for (const label of ${JSON.stringify(path)}) { item = m.items.find((i) => i.label === label); m = item && item.submenu; }
    const w = BrowserWindow.getAllWindows().find((x) => x.getTitle().includes('Obsidian')) || BrowserWindow.getFocusedWindow();
    item.click(undefined, w, w.webContents);
    await new Promise((r) => setTimeout(r, 400));
    return { level: w.webContents.getZoomLevel(), onTop: w.isAlwaysOnTop() };`);
  r.zoomIn = [];
  for (let i = 0; i < 8; i++) r.zoomIn.push(await click(['&View', 'Zoom In']));
  r.rendererLevel = await ev(`return require('electron').webFrame.getZoomLevel();`);
  r.zoomOut = [];
  for (let i = 0; i < 14; i++) r.zoomOut.push(await click(['&View', 'Zoom Out']));
  r.actual = await click(['&View', 'Actual Size']);
  r.onTop1 = await click(['&Window', 'Always on Top']);
  r.onTop2 = await click(['&Window', 'Always on Top']);
});

save();
c.close();
process.exit(0);
