// Two gaps at once (2026-11-16): what the Files pane's "New folder" does
// (the name it picks and its sequence, where the folder lands from the
// header button, the command, a folder's menu, the empty space's menu and a
// file's, what Enter, Escape, an empty name, a taken name and a click away do
// to the inline rename, and whether the folder survives a cancelled one), and
// the order of Obsidian's file pickers with no query: the template file
// picker's combobox and the Note composer's "Merge with" modal, against
// `vault.getFiles()` and the folder's own readdir order, at load and after a
// create, a modify and a rename.
// Phases: fixtures | run   (node new-folder-picker-order.mjs <phase>)
// Run with `OBSIDIAN_ARGS=--ozone-platform=x11` in a headless sway.
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const vp = (rel) => path.join(VAULT, rel);
const write = (rel, s) => { fs.mkdirSync(path.dirname(vp(rel)), { recursive: true }); fs.writeFileSync(vp(rel), s); };
const listVault = () => {
  const o = [];
  const walk = (dir, rel) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      if (e.name.startsWith('.')) continue;
      const r = rel ? rel + '/' + e.name : e.name;
      o.push(e.isDirectory() ? r + '/' : r);
      if (e.isDirectory()) walk(path.join(dir, e.name), r);
    }
  };
  walk(VAULT, '');
  return o;
};

// Created in this order, which is neither path order nor its reverse, with
// a pause between so birth times differ.
const CREATED = ['m.md', 'B/y.md', 'a.md', 'Z.md', 'B/c.md', 'Templates/T.md', 'Untitled.md', 'Templates/Deep/N.md', 'x.png', 'Proj/Plan.md', 'k.md'];

if (phase === 'fixtures') {
  write('.obsidian/app.json', '{}');
  write('.obsidian/core-plugins.json', JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true,
    'command-palette': true, 'note-composer': true, 'page-preview': false, outline: true,
  }, null, 2));
  for (const rel of CREATED) {
    write(rel, rel.endsWith('.png') ? 'not really a png' : `# ${path.basename(rel, '.md')}\n`);
    const d = Date.now(); while (Date.now() - d < 40) { /* spin */ }
  }
  fs.mkdirSync(vp('Empty'), { recursive: true });
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
const out = all['run' + (ONLY ? ':' + process.env.ONLY : '')] = { startedAt: new Date().toISOString(), steps: {}, order: [] };
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const c = await connect();
out.vaultPath = await c.evaluate(`return app.vault.adapter.basePath;`);
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }
out.title = await c.evaluate(`return document.title;`);
out.version = await c.evaluate(`return { ua: navigator.userAgent, api: (window.require && require('obsidian').apiVersion) || null };`).catch(() => null);
out.readdir = Object.fromEntries(['', 'B', 'Templates', 'Proj'].map((d) => [d || '/', fs.readdirSync(vp(d))]));

async function step(name, fn, ms = 60000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  try {
    await Promise.race([fn(partial), new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); })]);
    out.steps[name] = { ok: true, ms: Date.now() - t, ...partial };
  } catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...partial }; }
  finally { clearTimeout(timer); }
  try { await c.pressKey('Escape'); } catch { /* ignore */ }
  try { await c.evaluate(`document.querySelectorAll('.menu').forEach((m) => m.remove()); return true;`); } catch { /* ignore */ }
  save();
  console.error(`[new-folder] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + String(out.steps[name].error).split('\n')[0]}`);
}

const EXP = `
  const pane = () => app.workspace.getLeavesOfType('file-explorer')[0].view.containerEl;
  const rows = () => Array.from(pane().querySelectorAll('.nav-files-container .tree-item-self')).filter((s) => s.getBoundingClientRect().height > 0).map((s) => ({ path: s.getAttribute('data-path'), text: (s.querySelector('.tree-item-inner') || {}).textContent, folder: s.classList.contains('nav-folder-title'), renaming: s.classList.contains('is-being-renamed'), focus: s.classList.contains('has-focus'), active: s.classList.contains('is-active') }));
  const editing = () => { const e = pane().querySelector('.is-being-renamed .tree-item-inner'); if (!e) return null; const sel = window.getSelection(); return { text: e.textContent, editable: e.getAttribute('contenteditable'), focused: document.activeElement === e, selected: sel ? sel.toString() : null, path: e.closest('.tree-item-self').getAttribute('data-path') }; };
`;
const snap = () => c.evaluate(EXP + `return { rows: rows(), editing: editing(), active: document.activeElement ? document.activeElement.className + ' ' + document.activeElement.tagName : null, notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent) };`);
const button = (aria) => `.workspace-leaf-content[data-type="file-explorer"] .nav-action-button[aria-label="${aria}"]`;
const row = (p) => `.workspace-leaf-content[data-type="file-explorer"] .tree-item-self[data-path="${p}"]`;
const folders = () => listVault().filter((f) => f.endsWith('/'));
const newOnes = (before) => listVault().filter((f) => !before.includes(f));
const openNote = async (p) => { await c.evaluate(`const l = app.workspace.getLeavesOfType('markdown')[0] || app.workspace.getLeaf(false); await l.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(p)})); app.workspace.setActiveLeaf(l, { focus: true }); return true;`); await sleep(500); };
const clearNotices = () => c.evaluate(`document.querySelectorAll('.notice').forEach((n) => n.remove()); return true;`);

// ---------------------------------------------------------------- picker order
const FILES = `return { getFiles: app.vault.getFiles().map((f) => f.path), loaded: app.vault.getAllLoadedFiles().map((f) => f.path), md: app.vault.getMarkdownFiles().map((f) => f.path), stat: Object.fromEntries(app.vault.getFiles().map((f) => [f.path, [f.stat.ctime, f.stat.mtime]])) };`;

/** Open Settings on tab `id` and connect to its window (a page target of its own). */
async function settings(id) {
  const before = await pageTargets();
  await c.evaluate(`app.setting.open(); app.setting.openTabById(${JSON.stringify(id)}); return true;`);
  await sleep(1800);
  const after = await pageTargets();
  let url = after.find((u) => !before.includes(u));
  if (!url) {
    for (const u of after) {
      const s = await connect(u);
      const t = await s.evaluate(`return document.title;`).catch(() => null);
      if (t === 'Settings') { url = u; s.close(); break; }
      s.close();
    }
  }
  if (!url) return { s: c, popout: false };
  const s = await connect(url);
  return { s, popout: true };
}
async function pickerRows() {
  const { s, popout } = await settings('note-composer');
  const R = `const row = Array.from(document.querySelectorAll('.vertical-tab-content .setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === 'Template file location'); const b = row && row.querySelector('.combobox-button'); if (!b) return null; b.scrollIntoView({ block: 'center' }); const r = b.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2 };`;
  const b = await s.evaluate(R);
  let items = null;
  if (b) {
    await s.clickAt(b.cx, b.cy); await sleep(900);
    items = await s.evaluate(`const x = document.querySelector('.suggestion-container'); return x ? Array.from(x.querySelectorAll('.suggestion-item')).map((it) => it.textContent.trim()) : null;`);
    await s.pressKey('Escape'); await sleep(400);
  }
  if (popout) s.close();
  await c.evaluate(`app.setting.close(); return true;`); await sleep(600);
  return { popout, items };
}
async function mergeRows() {
  await openNote('k.md');
  await c.evaluate(`app.commands.executeCommandById('note-composer:merge-file'); return true;`); await sleep(900);
  const items = await c.evaluate(`const m = document.querySelector('.modal-container .prompt'); return m ? Array.from(m.querySelectorAll('.suggestion-item')).map((it) => it.textContent.trim()) : null;`);
  await c.pressKey('Escape'); await sleep(400);
  return items;
}
async function switcherRows() {
  await c.evaluate(`app.commands.executeCommandById('switcher:open'); return true;`); await sleep(900);
  const items = await c.evaluate(`const m = document.querySelector('.modal-container .prompt'); return m ? Array.from(m.querySelectorAll('.suggestion-item')).map((it) => it.textContent.trim()) : null;`);
  await c.pressKey('Escape'); await sleep(400);
  return items;
}

await step('s01-order-at-load', async (r) => {
  r.files = await c.evaluate(FILES);
  r.picker = await pickerRows();
  r.merge = await mergeRows();
  r.switcher = await switcherRows();
});

await step('s02-order-after-changes', async (r) => {
  await c.evaluate(`await app.vault.create('Zeta new.md', 'zeta\\n'); return true;`); await sleep(400);
  await c.evaluate(`await app.vault.create('Aaa new.md', 'aaa\\n'); return true;`); await sleep(400);
  await c.evaluate(`await app.vault.create('B/Mid new.md', 'mid\\n'); return true;`); await sleep(400);
  r.afterCreate = { files: (await c.evaluate(FILES)).getFiles, picker: (await pickerRows()).items };
  await c.evaluate(`await app.vault.modify(app.vault.getAbstractFileByPath('a.md'), '# a modified\\n'); return true;`); await sleep(400);
  r.afterModify = { files: (await c.evaluate(FILES)).getFiles, picker: (await pickerRows()).items };
  await c.evaluate(`await app.fileManager.renameFile(app.vault.getAbstractFileByPath('m.md'), 'm renamed.md'); return true;`); await sleep(600);
  r.afterRename = { files: (await c.evaluate(FILES)).getFiles, picker: (await pickerRows()).items };
  await c.evaluate(`await app.fileManager.renameFile(app.vault.getAbstractFileByPath('B'), 'B moved'); return true;`); await sleep(800);
  r.afterFolderRename = { files: (await c.evaluate(FILES)).getFiles, picker: (await pickerRows()).items, merge: await mergeRows() };
  // A file written behind Obsidian's back (the watcher's path, not the API's).
  fs.writeFileSync(vp('Outside.md'), 'outside\n'); await sleep(1500);
  r.afterOutside = { files: (await c.evaluate(FILES)).getFiles, picker: (await pickerRows()).items };
});

// ---------------------------------------------------------------- new folder
await step('s03-header-button', async (r) => {
  await openNote('Proj/Plan.md');
  r.config = await c.evaluate(`return { newFileLocation: app.vault.getConfig('newFileLocation'), newFileFolderPath: app.vault.getConfig('newFileFolderPath') };`);
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  r.snap = await snap();
  // The sequence: Escape, then again, and again.
  await c.pressKey('Escape'); await sleep(600);
  r.afterEscape = { created: newOnes(b0), snap: await snap() };
  await c.clickElement(button('New folder')); await sleep(900);
  r.second = { created: newOnes(b0), editing: (await snap()).editing };
  await c.pressKey('Escape'); await sleep(600);
  await c.clickElement(button('New folder')); await sleep(900);
  r.third = { created: newOnes(b0), editing: (await snap()).editing };
  await c.pressKey('Escape'); await sleep(600);
  r.folders = folders();
});

await step('s04-type-and-enter', async (r) => {
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  r.editing = (await snap()).editing;
  await c.typeText('Fresh'); await sleep(200);
  r.whileTyping = (await snap()).editing;
  await c.pressKey('Enter'); await sleep(800);
  r.after = { created: newOnes(b0), snap: await snap() };
});

await step('s05-empty-name', async (r) => {
  await clearNotices();
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  await c.pressKey('Backspace'); await sleep(200);
  r.emptied = (await snap()).editing;
  await c.pressKey('Enter'); await sleep(900);
  r.after = { created: newOnes(b0), snap: await snap() };
  await c.pressKey('Escape'); await sleep(500);
  r.afterEscape = { created: newOnes(b0), snap: await snap() };
});

await step('s06-taken-name', async (r) => {
  await clearNotices();
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  await c.typeText('Proj'); await sleep(200);
  await c.pressKey('Enter'); await sleep(1000);
  r.after = { created: newOnes(b0), snap: await snap(), modals: await c.evaluate(`return Array.from(document.querySelectorAll('.modal-container')).map((m) => m.textContent.trim().slice(0, 200));`) };
  await c.pressKey('Escape'); await sleep(500);
  r.afterEscape = { created: newOnes(b0), snap: await snap() };
});

await step('s07-click-away', async (r) => {
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  await c.typeText('Blurred'); await sleep(200);
  // A click in the note's editor.
  const ed = await c.rect('.workspace-leaf.mod-active .cm-content, .mod-root .cm-content');
  if (ed) { await c.clickAt(ed.cx, ed.cy); await sleep(900); }
  r.after = { created: newOnes(b0), snap: await snap() };
});

await step('s08-escape-after-typing', async (r) => {
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  await c.typeText('Dropped'); await sleep(200);
  await c.pressKey('Escape'); await sleep(800);
  r.after = { created: newOnes(b0), snap: await snap() };
});

await step('s09-folder-menu', async (r) => {
  // Collapse Proj first: does New folder inside it expand it?
  await c.evaluate(`const v = app.workspace.getLeavesOfType('file-explorer')[0].view; const it = v.fileItems['Proj']; if (it && !it.collapsed) it.setCollapsed(true); return true;`); await sleep(300);
  const b0 = listVault();
  await c.rightClickElement(row('Proj')); await sleep(600);
  r.menu = await c.openMenuItems();
  await c.evaluate(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((i) => (i.textContent || '').trim() === 'New folder'); if (it) it.click(); return !!it;`);
  await sleep(900);
  r.created = newOnes(b0);
  r.snap = await snap();
  await c.pressKey('Escape'); await sleep(500);
  // A second one in the same folder.
  await c.rightClickElement(row('Proj')); await sleep(600);
  await c.evaluate(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((i) => (i.textContent || '').trim() === 'New folder'); if (it) it.click(); return !!it;`);
  await sleep(900);
  r.second = { created: newOnes(b0), editing: (await snap()).editing };
  await c.pressKey('Escape'); await sleep(500);
});

await step('s10-file-menu', async (r) => {
  await c.rightClickElement(row('Proj/Plan.md')); await sleep(600);
  r.menu = await c.openMenuItems();
  await c.pressKey('Escape'); await sleep(300);
});

await step('s11-blank-menu', async (r) => {
  const b0 = listVault();
  const box = await c.evaluate(`const el = app.workspace.getLeavesOfType('file-explorer')[0].view.containerEl.querySelector('.nav-files-container'); const b = el.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height - 20 };`);
  await c.clickAt(box.x, box.y, { button: 'right' }); await sleep(600);
  r.menu = await c.openMenuItems();
  await c.evaluate(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((i) => (i.textContent || '').trim() === 'New folder'); if (it) it.click(); return !!it;`);
  await sleep(900);
  r.created = newOnes(b0);
  r.editing = (await snap()).editing;
  await c.pressKey('Escape'); await sleep(500);
});

await step('s12-command', async (r) => {
  await openNote('Proj/Plan.md');
  const b0 = listVault();
  r.ran = await c.evaluate(`return app.commands.executeCommandById('file-explorer:new-folder');`); await sleep(900);
  r.created = newOnes(b0);
  r.snap = await snap();
  await c.pressKey('Escape'); await sleep(500);
  // With the explorer collapsed away (another left tab active)?
});

await step('s13-new-file-location', async (r) => {
  await c.evaluate(`app.vault.setConfig('newFileLocation', 'current'); return true;`); await sleep(300);
  await openNote('Proj/Plan.md');
  let b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.current = { created: newOnes(b0), editing: (await snap()).editing };
  await c.pressKey('Escape'); await sleep(500);
  b0 = listVault();
  r.currentCommand = await c.evaluate(`return app.commands.executeCommandById('file-explorer:new-folder');`); await sleep(900);
  r.currentCommandCreated = newOnes(b0);
  await c.pressKey('Escape'); await sleep(500);
  await c.evaluate(`app.vault.setConfig('newFileLocation', 'folder'); app.vault.setConfig('newFileFolderPath', 'Templates'); return true;`); await sleep(300);
  b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.folder = { created: newOnes(b0), editing: (await snap()).editing };
  await c.pressKey('Escape'); await sleep(500);
  await c.evaluate(`app.vault.setConfig('newFileLocation', 'root'); return true;`);
});

await step('s14-focused-folder-row', async (r) => {
  // The tree has the keyboard on a folder row: does the header's New folder go there?
  await c.evaluate(`const v = app.workspace.getLeavesOfType('file-explorer')[0].view; app.workspace.setActiveLeaf(app.workspace.getLeavesOfType('file-explorer')[0], { focus: true }); return true;`);
  await c.clickElement(row('Templates')); await sleep(500);
  r.before = (await snap()).rows.filter((x) => x.focus || x.active);
  const b0 = listVault();
  await c.clickElement(button('New folder')); await sleep(900);
  r.created = newOnes(b0);
  r.editing = (await snap()).editing;
  await c.pressKey('Escape'); await sleep(500);
});

out.finishedAt = new Date().toISOString();
out.vaultAtEnd = listVault();
save();
c.close();
process.exit(0);
