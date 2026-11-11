// Seven commands phase 4 left unrunnable for want of an observation
// ([[Development#The unrunnable commands, 2026-11-09]]):
//
//   s00  version; each id's name, icon, callback kinds, default hotkeys
//   s01  editor:toggle-spellcheck from no key and from false, twice each:
//        app.json's bytes, the editor's spellcheck attribute, notices
//   s02  editor:attach-file: the file input it raises, then for each
//        attachmentFolderPath ("/", "./", "./att", "Attach") where the chosen
//        file is copied, the text inserted, the caret; a second copy of the
//        same name; a selection; Markdown links; a file already in the vault
//   s03  editor:download-attachments: availability, the modal, which links
//        it offers, the names written, the rewritten links, collisions, each
//        attachment folder, Markdown links, a deselected item, a 404, Cancel,
//        a note with nothing to download
//   s04  app:migrate-frontmatter: the modal, then every fixture's bytes
//        before and after Start conversion, the notices
//   s05  app:show-debug-info: the text with its values, under a second
//        configuration; Copy to clipboard, Sandbox vault (blocked), Done
//   s06  app:show-release-notes offline (the main process's session put
//        offline), and what is kept between runs
//   s07  app:open-file with the main process's open dialog answered: a note
//        inside the vault, a note outside it (and an edit to it), an image
//        outside it, a cancel
//
// The file input is answered over CDP (Page.setInterceptFileChooserDialog,
// DOM.setFileInputFiles), so no picker is drawn. Obsidian's open dialog is
// shown by the main process: with OBSIDIAN_ARGS=--inspect=<port> (MAIN_PORT)
// `electron.dialog.showOpenDialog` is replaced there by a recorder that
// answers with the path the step names. That is a stub at the dialog
// boundary, not a person choosing a file in GTK. Images for s03 come from
// a local server (MEDIA_BASE, `serve.py` beside the run).
//
// Phases: fixtures | run. ONLY=<regex> picks steps.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep, pageTargets } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const OUTSIDE = process.env.OUTSIDE;
const MEDIA = process.env.MEDIA_BASE || 'http://127.0.0.1:9343';
const MAIN_PORT = process.env.MAIN_PORT || '9342';
const MEDIA_LOG = process.env.MEDIA_LOG;
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
if (!VAULT || !RESULTS || !OUTSIDE) { console.error('VAULT, RESULTS and OUTSIDE are required'); process.exit(2); }

const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const vp = (rel) => path.join(VAULT, rel);

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.mkdirSync(OUTSIDE, { recursive: true });
  fs.writeFileSync(path.join(OUTSIDE, 'pic one.png'), PNG);
  fs.writeFileSync(path.join(OUTSIDE, 'doc.txt'), 'hello attach\n');
  fs.writeFileSync(path.join(OUTSIDE, 'Outside Note.md'), 'outside body\n');
  fs.writeFileSync(path.join(OUTSIDE, 'outside.png'), PNG);
  process.exit(0);
}

const T0 = Date.now();
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
const out = { startedAt: new Date().toISOString(), env: { ONLY: process.env.ONLY || null, vault: VAULT, outside: OUTSIDE }, steps: {}, order: [] };
all[phase + (process.env.RUN_LABEL ? ':' + process.env.RUN_LABEL : '')] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now(); const r = {};
  out.steps[name] = { ok: null, partial: r }; save();
  try { await withTimeout(fn(r), ms, name); out.steps[name] = { ok: true, ms: Date.now() - t, ...r }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...r }; }
  save();
}

// ---- the disk, read on the host -------------------------------------------
function listVault() {
  const o = {};
  const walk = (d, rel) => {
    for (const n of fs.readdirSync(d).sort()) {
      if (n === '.obsidian') continue;
      const p = path.join(d, n); const r = rel ? rel + '/' + n : n;
      if (fs.statSync(p).isDirectory()) { o[r + '/'] = 'dir'; walk(p, r); } else o[r] = fs.statSync(p).size;
    }
  };
  walk(VAULT, '');
  return o;
}
const readOr = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } };
const mediaLog = () => (MEDIA_LOG ? (readOr(MEDIA_LOG) || '').split('\n').filter(Boolean) : []);
function clearVault() {
  for (const n of fs.readdirSync(VAULT)) if (n !== '.obsidian') fs.rmSync(vp(n), { recursive: true, force: true });
}

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

await mainEval(`
  const e = require('electron');
  global.__dlg = []; global.__pick = null;
  for (const n of ['showOpenDialog', 'showOpenDialogSync']) {
    e.dialog[n] = (...a) => {
      const opts = a.find((x) => x && typeof x === 'object' && (x.properties || x.filters)) || null;
      global.__dlg.push({ n, opts: opts ? JSON.parse(JSON.stringify(opts)) : null });
      const p = global.__pick;
      if (n.endsWith('Sync')) return p || undefined;
      return Promise.resolve(p ? { canceled: false, filePaths: p } : { canceled: true, filePaths: [] });
    };
  }
  return true;`);
const pick = (paths) => mainEval(`global.__dlg = []; global.__pick = ${JSON.stringify(paths)}; return true;`);
const dialogs = () => mainEval(`return global.__dlg;`);

await ev(`
const e = require('electron');
window.__rec = [];
const ipc = e.ipcRenderer;
const BLOCK = new Set(['sandbox', 'open-url']);
const origSend = ipc.send.bind(ipc);
ipc.send = (ch, ...a) => { if (ch !== 'update-menu-items') window.__rec.push({ kind: (BLOCK.has(ch) ? 'BLOCKED ' : '') + 'ipc.send ' + ch, args: JSON.parse(JSON.stringify(a)) }); if (BLOCK.has(ch)) return; return origSend(ch, ...a); };
const origSendSync = ipc.sendSync.bind(ipc);
ipc.sendSync = (ch, ...a) => { if (!String(ch).startsWith('REMOTE_')) window.__rec.push({ kind: 'ipc.sendSync ' + ch, args: JSON.parse(JSON.stringify(a)) }); return origSendSync(ch, ...a); };
window.__inputs = [];
const oc = HTMLInputElement.prototype.click;
HTMLInputElement.prototype.click = function () { if (this.type === 'file') { window.__lastFileInput = this; window.__inputs.push({ type: this.type, accept: this.accept, multiple: this.multiple, inDoc: document.contains(this) }); } return oc.call(this); };
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  take() { const r = window.__rec; window.__rec = []; return r; },
  notices() { return Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent); },
  clearNotices() { document.querySelectorAll('.notice').forEach((n) => n.remove()); },
  avail(id) { return app.commands.listCommands().some((k) => k.id === id); },
  run(id) { let ran = null, err = null; try { ran = app.commands.executeCommandById(id); } catch (x) { err = String(x); } return { ran, err }; },
  leaf() { const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l)); return ls; },
  async only(file) {
    for (const l of this.leaf().slice(1)) l.detach();
    const leaf = this.leaf()[0];
    if (file) await leaf.openFile(app.vault.getAbstractFileByPath(file)); else await leaf.setViewState({ type: 'empty' });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(500);
    return leaf;
  },
  editor() { return app.workspace.activeLeaf.view.editor; },
  modal() {
    const m = document.querySelector('.modal-container .modal'); if (!m) return null;
    const strip = (h) => h.replace(/<svg[\\s\\S]*?<\\/svg>/g, '<svg/>');
    return { cls: m.className, title: (m.querySelector('.modal-title') || {}).textContent || null, html: strip(m.outerHTML).slice(0, 6000),
      buttons: Array.from(m.querySelectorAll('button')).map((b) => ({ text: b.textContent, cls: b.className })) };
  },
  dismiss() { document.querySelectorAll('.modal-container').forEach((m) => { const b = m.querySelector('.mod-cancel'); if (b) b.click(); }); document.querySelectorAll('.modal-container').forEach((m) => m.remove()); },
  async settle(path, want) { for (let i = 0; i < 40; i++) { const f = app.vault.getAbstractFileByPath(path); if (f && (want === undefined || (await app.vault.read(f)) === want)) return true; await this.sleep(100); } return false; },
};
return true;`);
const take = () => ev(`return window.__p.take();`);
const notices = () => ev(`return window.__p.notices();`);
const clearNotices = () => ev(`window.__p.clearNotices(); return true;`);
const avail = (id) => ev(`return window.__p.avail(${JSON.stringify(id)});`);
const run = (id) => ev(`return window.__p.run(${JSON.stringify(id)});`);
const modal = () => ev(`return window.__p.modal();`);
const dismiss = () => ev(`window.__p.dismiss(); return true;`);
const setConfig = (k, v) => ev(`app.vault.setConfig(${JSON.stringify(k)}, ${JSON.stringify(v)}); return app.vault.getConfig(${JSON.stringify(k)});`);
const write = (rel, text) => ev(`const p = ${JSON.stringify(rel)}; const dir = p.split('/').slice(0, -1).join('/'); if (dir && !(await app.vault.adapter.exists(dir))) await app.vault.createFolder(dir);
  const f = app.vault.getAbstractFileByPath(p); if (f) await app.vault.modify(f, ${JSON.stringify(text)}); else await app.vault.create(p, ${JSON.stringify(text)});
  return await window.__p.settle(p, ${JSON.stringify(text)});`);
const editorState = () => ev(`const ed = window.__p.editor(); return { text: ed.getValue(), cursor: ed.getCursor(), anchor: ed.getCursor('anchor'), selection: ed.getSelection() };`);
async function reset() {
  await dismiss(); await clearNotices();
  await ev(`await window.__p.only(null); return true;`);
  for (const f of await ev(`return app.vault.getAllLoadedFiles().filter((f) => f.path !== '/').map((f) => f.path);`)) {
    await ev(`const f = app.vault.getAbstractFileByPath(${JSON.stringify(f)}); if (f) { try { await app.vault.delete(f, true); } catch (e) {} } return true;`);
  }
  await sleep(300);
}

await step('s00-version', async (r) => {
  r.title = await ev(`return document.title;`);
  r.version = await ev(`return require('electron').ipcRenderer.sendSync('version');`);
  r.userAgent = await ev(`return navigator.userAgent;`);
  r.commands = await ev(`return ${JSON.stringify(['editor:toggle-spellcheck', 'editor:attach-file', 'editor:download-attachments', 'app:migrate-frontmatter', 'app:show-debug-info', 'app:show-release-notes', 'app:open-file'])}.map((id) => { const c = app.commands.commands[id];
    return [id, c ? { name: c.name, icon: c.icon || null, kinds: ['callback', 'checkCallback', 'editorCallback', 'editorCheckCallback'].filter((k) => c[k]), hotkeys: app.hotkeyManager.getDefaultHotkeys(id) || null } : null]; });`);
});

// ---- s01 spellcheck --------------------------------------------------------
await step('s01-spellcheck', async (r) => {
  await reset();
  await write('Note.md', 'line one\n');
  await ev(`await window.__p.only('Note.md'); return true;`);
  r.runs = [];
  for (const start of ['{}', '{\n  "spellcheck": false\n}']) {
    fs.writeFileSync(vp('.obsidian/app.json'), start);
    await ev(`await app.vault.reloadConfig(); return true;`).catch(() => null);
    await sleep(800);
    const seq = { start, startConfig: await ev(`return app.vault.getConfig('spellcheck');`), runs: [] };
    for (let i = 0; i < 2; i++) {
      await clearNotices();
      const before = readOr(vp('.obsidian/app.json'));
      seq.runs.push({ avail: await avail('editor:toggle-spellcheck'), run: await run('editor:toggle-spellcheck') });
      const t = Date.now(); let after = before;
      while (Date.now() - t < 4000 && after === before) { await sleep(100); after = readOr(vp('.obsidian/app.json')); }
      await sleep(600);
      Object.assign(seq.runs[i], { writtenAfterMs: after === before ? null : Date.now() - t, appJson: readOr(vp('.obsidian/app.json')),
        config: await ev(`return app.vault.getConfig('spellcheck');`),
        cmAttr: await ev(`const e = document.querySelector('.cm-content'); return e ? e.getAttribute('spellcheck') : null;`),
        webContents: await ev(`try { return require('electron').remote.getCurrentWebContents().session.isSpellCheckerEnabled(); } catch (e) { return 'n/a: ' + e.message; }`),
        notices: await notices(), calls: await take() });
    }
    r.runs.push(seq);
  }
  r.noFile = await ev(`await window.__p.only(null); return window.__p.avail('editor:toggle-spellcheck');`);
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
});

// ---- s02 attach-file -------------------------------------------------------
await c.send('Page.enable');
await c.send('DOM.enable');
await c.send('Page.setInterceptFileChooserDialog', { enabled: true });
async function attach(file, { setup = '' } = {}) {
  const o = {};
  await ev(`window.__inputs = []; return true;`);
  await clearNotices();
  const chooser = c.once('Page.fileChooserOpened', 6000).catch((e) => ({ error: String(e) }));
  if (setup) await ev(`const ed = window.__p.editor(); ${setup}; return true;`);
  o.before = await editorState();
  // A file input opens its chooser only with a user activation, which a
  // plain Runtime.evaluate does not carry: run the command with one.
  const ranWith = await c.send('Runtime.evaluate', { expression: `window.__p.run('editor:attach-file')`, userGesture: true, returnByValue: true });
  o.run = ranWith.result && ranWith.result.value;
  const ch = await chooser;
  o.chooser = { mode: ch.mode || null, error: ch.error || null };
  o.inputs = await ev(`return window.__inputs;`);
  if (ch.backendNodeId && file) await c.send('DOM.setFileInputFiles', { files: [file], backendNodeId: ch.backendNodeId });
  else if (file) {
    // No chooser event: hand the files to the input the command clicked.
    const h = await c.send('Runtime.evaluate', { expression: 'window.__lastFileInput' });
    o.fallback = !!(h.result && h.result.objectId);
    if (o.fallback) await c.send('DOM.setFileInputFiles', { files: [file], objectId: h.result.objectId });
  }
  await sleep(1800);
  o.after = await editorState();
  o.vault = listVault();
  o.notices = await notices();
  o.calls = await take();
  return o;
}
const PIC = path.join(OUTSIDE, 'pic one.png');
const TXT = path.join(OUTSIDE, 'doc.txt');
await step('s02-attach-file', async (r) => {
  r.cases = {};
  const base = async (folder, extra = {}) => {
    await reset();
    await setConfig('attachmentFolderPath', folder);
    await setConfig('useMarkdownLinks', extra.md || false);
    await setConfig('newLinkFormat', extra.format || 'shortest');
    await write('Sub/Note.md', 'line one\nline two\n');
    await ev(`await window.__p.only('Sub/Note.md'); return true;`);
  };
  const caret = `ed.focus(); ed.setCursor({ line: 0, ch: 4 })`;
  await base('/');
  r.availability = { note: await avail('editor:attach-file') };
  await ev(`const l = app.workspace.activeLeaf; const s = l.getViewState(); s.state.mode = 'preview'; await l.setViewState(s); await window.__p.sleep(300); return true;`);
  r.availability.reading = await avail('editor:attach-file');
  await ev(`await window.__p.only(null); return true;`);
  r.availability.empty = await avail('editor:attach-file');

  await base('/');
  r.cases.rootPng = await attach(PIC, { setup: caret });
  r.cases.rootPngAgain = await attach(PIC, { setup: `ed.focus(); ed.setCursor({ line: 1, ch: 8 })` });
  r.cases.rootTxt = await attach(TXT, { setup: `ed.focus(); ed.setCursor({ line: 1, ch: 0 })` });
  r.cases.cancel = await attach(null, { setup: caret });
  await base('./');
  r.cases.sameFolderPng = await attach(PIC, { setup: caret });
  await base('./att');
  r.cases.subfolderPng = await attach(PIC, { setup: caret });
  await base('Attach');
  r.cases.namedFolderPng = await attach(PIC, { setup: caret });
  await base('/');
  r.cases.selection = await attach(PIC, { setup: `ed.focus(); ed.setSelection({ line: 0, ch: 5 }, { line: 0, ch: 8 })` });
  await base('/', { md: true });
  r.cases.markdownLinks = await attach(PIC, { setup: caret });
  await base('/', { md: true, format: 'relative' });
  r.cases.markdownRelative = await attach(PIC, { setup: caret });
  await base('/');
  fs.writeFileSync(vp('Sub/inside.png'), PNG);
  await sleep(800);
  r.cases.alreadyInVault = await attach(vp('Sub/inside.png'), { setup: caret });
  await setConfig('attachmentFolderPath', '/'); await setConfig('useMarkdownLinks', false); await setConfig('newLinkFormat', 'shortest');
});
await c.send('Page.setInterceptFileChooserDialog', { enabled: false });

// ---- s03 download-attachments ----------------------------------------------
const M = MEDIA;
const DL_NOTE = [
  `a ![](${M}/img/photo.png) b`,
  `![alt](${M}/img/two%20words.png)`,
  `[link](${M}/img/pic.jpg)`,
  `![x](${M}/png-noext)`,
  `![j](${M}/jpeg-noext)`,
  `![q](${M}/img/photo.png?size=2)`,
  `![sized|100](${M}/img/pic.jpg)`,
  `![t](${M}/img/again/photo.png "a title")`,
  `<img src="${M}/img/pic.jpg">`,
  `![[${M}/img/photo.png]]`,
  `![pdf](${M}/doc.pdf)`,
  '',
].join('\n');
async function download(note, { before = null, click = null, button = 'mod-cta' } = {}) {
  const o = {};
  await ev(`await window.__p.only(${JSON.stringify(note)}); return true;`);
  await clearNotices();
  if (before) await before();
  o.mediaBefore = mediaLog().length;
  o.textBefore = readOr(vp(note));
  o.avail = await avail('editor:download-attachments');
  o.run = await run('editor:download-attachments');
  await sleep(1500);
  o.modal = await modal();
  o.items = await ev(`return Array.from(document.querySelectorAll('.download-attachment-item')).map((i) => ({ cls: i.className, label: i.getAttribute('aria-label'), checked: (i.querySelector('input') || {}).checked, img: (i.querySelector('img') || {}).src || null, html: i.innerHTML.slice(0, 300) }));`);
  o.noticesOnOpen = await notices();
  if (click !== null && o.modal) {
    await ev(`document.querySelectorAll('.download-attachment-item')[${click}].click(); return true;`);
    await sleep(300);
    o.itemsAfterClick = await ev(`return Array.from(document.querySelectorAll('.download-attachment-item')).map((i) => ({ cls: i.className, checked: (i.querySelector('input') || {}).checked }));`);
  }
  if (o.modal) {
    await ev(`const b = document.querySelector('.modal-container .modal button.${button}'); if (b) b.click(); return !!b;`);
    await sleep(3500);
  }
  o.modalAfter = await modal();
  o.textAfter = readOr(vp(note));
  o.editorText = await ev(`const v = app.workspace.activeLeaf.view; return v.editor ? v.editor.getValue() : null;`);
  o.vault = listVault();
  o.notices = await notices();
  o.requests = mediaLog().slice(o.mediaBefore);
  o.calls = await take();
  await dismiss();
  return o;
}
await step('s03-download-attachments', async (r) => {
  r.cases = {};
  const base = async (folder, extra = {}) => {
    await reset();
    await setConfig('attachmentFolderPath', folder);
    await setConfig('useMarkdownLinks', extra.md || false);
    await setConfig('newLinkFormat', extra.format || 'shortest');
    await write('Sub/Page.md', extra.text || DL_NOTE);
  };
  await base('/');
  r.cases.root = await download('Sub/Page.md');
  await base('/');
  fs.writeFileSync(vp('photo.png'), Buffer.from('existing'));
  await sleep(800);
  r.cases.collision = await download('Sub/Page.md');
  await base('./att');
  r.cases.subfolder = await download('Sub/Page.md');
  await base('./');
  r.cases.sameFolder = await download('Sub/Page.md');
  await base('Attach');
  r.cases.namedFolder = await download('Sub/Page.md');
  await base('/', { md: true });
  r.cases.markdownLinks = await download('Sub/Page.md');
  await base('/', { text: `![a](${M}/img/photo.png)\n![b](${M}/img/two%20words.png)\n` });
  r.cases.deselectSecond = await download('Sub/Page.md', { click: 1 });
  await base('/', { text: `![a](${M}/img/photo.png)\n![gone](${M}/missing.png)\n` });
  r.cases.notFound = await download('Sub/Page.md');
  await base('/', { text: `![a](${M}/img/photo.png)\n` });
  r.cases.cancel = await download('Sub/Page.md', { button: 'mod-cancel' });
  await base('/', { text: `no images here [x](${M}/img/pic.jpg) ![[local.png]]\n` });
  r.cases.nothing = await download('Sub/Page.md');
  await base('/', { text: `![a](${M}/img/photo.png)\n![b](${M}/img/photo.png)\n` });
  r.cases.sameUrlTwice = await download('Sub/Page.md');
  await ev(`await window.__p.only(null); return true;`);
  r.emptyTabAvail = await avail('editor:download-attachments');
  await setConfig('attachmentFolderPath', '/'); await setConfig('useMarkdownLinks', false);
});

await step('s03b-download-edges', async (r) => {
  r.cases = {};
  const base = async (text) => { await reset(); await setConfig('attachmentFolderPath', '/'); await setConfig('useMarkdownLinks', false); await setConfig('newLinkFormat', 'shortest'); await write('Sub/Page.md', text); };
  await base(`![gone](${M}/missing.png)\n![a](${M}/img/photo.png)\n`);
  r.cases.notFoundFirst = await download('Sub/Page.md');
  await base(`![g](${M}/gif-noext)\n![w](${M}/webp-noext)\n![s](${M}/svg-noext)\n![h](${M}/html-noext)\n![o](${M}/octet-noext)\n`);
  r.cases.contentTypes = await download('Sub/Page.md');
  await base(`![u](${M}/img/UPPER.PNG)\n![d](${M}/img/dir/)\n![e](${M}/img/a%23b.png)\n![s](https://127.0.0.1:9/nope.png)\n`);
  r.cases.oddNames = await download('Sub/Page.md');
  await base(`- ![a](${M}/img/photo.png)\n> ![b](${M}/img/pic.jpg)\n| ![c](${M}/png-noext) |\n| --- |\n\`![d](${M}/img/photo.png)\`\n`);
  r.cases.contexts = await download('Sub/Page.md');
  await base(`---\nimg: ${M}/img/photo.png\n---\n![a](<${M}/img/two%20words.png>)\n![b](${M}/img/photo.png#frag)\n`);
  r.cases.misc = await download('Sub/Page.md');
});

await step('s03c-download-names', async (r) => {
  r.cases = {};
  const base = async (text) => { await reset(); await setConfig('attachmentFolderPath', '/'); await setConfig('useMarkdownLinks', false); await setConfig('newLinkFormat', 'shortest'); await write('Sub/Page.md', text); };
  await base(`![m](${M}/img/mismatch.png)\n![h](${M}/img/x.webp)\n![j](${M}/img/name.jpeg)\n![d](${M}/img/dots.min.png)\n![g](${M}/img/Grand%C3%A9.gif)\n`);
  r.cases.extensions = await download('Sub/Page.md');
  await base(`---\nsource: ${M}/page.html\nlist:\n  - ${M}/img/photo.png\nquoted: "${M}/img/pic.jpg"\ntext: see ${M}/img/photo.png\n---\nbody\n`);
  r.cases.frontmatter = await download('Sub/Page.md');
  await base(`![a](${M}/img/photo.png)\n`);
  await setConfig('useMarkdownLinks', true); await setConfig('newLinkFormat', 'absolute');
  r.cases.absoluteMarkdown = await download('Sub/Page.md');
  await setConfig('useMarkdownLinks', false); await setConfig('newLinkFormat', 'shortest');
});

// ---- s04 migrate-frontmatter -----------------------------------------------
const FM = process.env.FM_SET === 'b' ? {
  'n01 aliases spaces.md': '---\naliases: New York\n---\nbody\n',
  'n02 aliases comma spaces.md': '---\naliases: New York, LA\n---\nbody\n',
  'n03 tags comma and spaces.md': '---\ntags: a b, c\n---\nbody\n',
  'n04 cssclasses comma.md': '---\ncssclasses: wide, narrow\n---\nbody\n',
  'n05 tags duplicate.md': '---\ntags: a a\n---\nbody\n',
  'n06 tags trailing comma.md': '---\ntags: a,\n---\nbody\n',
  'n07 tags with hash unquoted list.md': '---\ntags:\n  - "#a"\n---\nbody\n',
  'n08 Tags capital.md': '---\nTags: a b\n---\nbody\n',
  'n09 tags bool.md': '---\ntags: true\n---\nbody\n',
  'n10 alias list.md': '---\nalias:\n  - x\n---\nbody\n',
  'n11 cssclass and cssclasses.md': '---\ncssclass: a\ncssclasses: b\n---\nbody\n',
  'n12 nested other.md': '---\nmeta:\n  k: v\ntags: a\n---\nbody\n',
  'n13 flow other.md': '---\nlist: [1, 2]\ntags: a\n---\nbody\n',
  'n14 date and quotes.md': "---\nd: 2024-01-02\ns: 'single'\ntags: a\n---\nbody\n",
  'n15 blank line.md': '---\na: 1\n\ntags: a\n---\nbody\n',
  'n16 tag empty.md': '---\ntag:\n---\nbody\n',
  'n17 aliases empty string.md': '---\naliases: ""\n---\nbody\n',
} : {
  'm01 tags comma.md': '---\ntags: a, b\n---\nbody\n',
  'm02 tags spaces.md': '---\ntags: "#a #b"\n---\nbody\n',
  'm03 tag singular.md': '---\ntag: x\n---\nbody\n',
  'm04 tags flow.md': '---\ntags: [a, b]\n---\nbody\n',
  'm05 tags block.md': '---\ntags:\n  - a\n  - b\n---\nbody\n',
  'm06 aliases scalar.md': '---\naliases: foo\n---\nbody\n',
  'm07 alias singular.md': '---\nalias: foo, bar\n---\nbody\n',
  'm08 cssclass.md': '---\ncssclass: wide\n---\nbody\n',
  'm09 cssclasses string.md': '---\ncssclasses: wide narrow\n---\nbody\n',
  'm10 tags hash.md': '---\ntags: "#a"\n---\nbody\n',
  'm11 no frontmatter.md': 'tags: a\nbody\n',
  'm12 crlf.md': '---\r\ntitle: T\r\ntags: a\r\nother: 1\r\n---\r\nbody\r\n',
  'm13 tags empty.md': '---\ntags:\n---\nbody\n',
  'm14 tags number.md': '---\ntags: 123\n---\nbody\n',
  'm15 both tag and tags.md': '---\ntag: x\ntags: [y]\n---\nbody\n',
  'm16 order kept.md': '---\nz: 1\ntags: a\ny: 2\naliases: b\nx: 3\n---\nbody\n',
  'Sub/m17 nested.md': '---\ntags: n1 n2\n---\nbody\n',
  'm18 tags one word.md': '---\ntags: solo\n---\nbody\n',
  'm19 aliases list.md': '---\naliases:\n  - one\n  - two\n---\nbody\n',
  'm20 tags quoted comma.md': "---\ntags: 'a, b'\n---\nbody\n",
  'm21 alias and aliases.md': '---\nalias: x\naliases: y\n---\nbody\n',
  'm22 tags flow hash.md': '---\ntags: ["#a", b]\n---\nbody\n',
  'm23 cssclass list.md': '---\ncssclass:\n  - wide\n---\nbody\n',
  'm24 tags comma no space.md': '---\ntags: a,b,c\n---\nbody\n',
  'm25 bom.md': '﻿---\ntags: a\n---\nbody\n',
  'm26 comment.md': '---\ntags: a # note\nk: v # keep\n---\nbody\n',
  'm27 tags nested.md': '---\ntags: a/b c/d\n---\nbody\n',
  'm28 other keys only.md': '---\ntitle: plain\n---\nbody\n',
};
await step('s04-migrate-frontmatter', async (r) => {
  await reset();
  for (const [p, t] of Object.entries(FM)) { const d = path.dirname(vp(p)); fs.mkdirSync(d, { recursive: true }); fs.writeFileSync(vp(p), t); }
  fs.writeFileSync(vp('Board.canvas'), '{"nodes":[],"edges":[]}');
  await sleep(2500);
  const mt = () => Object.fromEntries([...Object.keys(FM), 'Board.canvas'].map((p) => [p, fs.statSync(vp(p)).mtimeMs]));
  const m0 = mt();
  r.avail = await avail('app:migrate-frontmatter');
  r.run = await run('app:migrate-frontmatter');
  await sleep(800);
  r.modal = await modal();
  r.cancel = await ev(`const b = document.querySelector('.modal-container .modal .mod-cancel'); if (b) b.click(); await window.__p.sleep(500); return { clicked: !!b, modalLeft: !!document.querySelector('.modal-container') };`);
  r.afterCancelChanged = Object.entries(mt()).filter(([p, t]) => t !== m0[p]).map(([p]) => p);
  await run('app:migrate-frontmatter');
  await sleep(800);
  await ev(`document.querySelector('.modal-container .modal .mod-cta').click(); return true;`);
  const seen = [];
  for (let i = 0; i < 30; i++) { await sleep(300); const m = await modal(); const n = await notices(); seen.push({ t: i * 300, modal: m ? { title: m.title, text: m.html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').slice(0, 400), buttons: m.buttons } : null, notices: n }); }
  r.progress = seen.filter((s, i) => i === 0 || JSON.stringify(s.modal) + JSON.stringify(s.notices) !== JSON.stringify(seen[i - 1].modal) + JSON.stringify(seen[i - 1].notices));
  r.files = {};
  const m1 = mt();
  for (const [p, t] of Object.entries(FM)) { const now = fs.readFileSync(vp(p), 'utf8'); r.files[p] = { before: t, after: now, changed: now !== t, touched: m1[p] !== m0[p] }; }
  r.canvasTouched = m1['Board.canvas'] !== m0['Board.canvas'];
  r.finishedModal = await modal();
  r.calls = await take();
  // Done, then the notice's Undo.
  r.done = await ev(`const b = Array.from(document.querySelectorAll('.modal-container .modal button')).find((b) => b.textContent === 'Done'); if (b) b.click(); await window.__p.sleep(500); return { clicked: !!b, modalLeft: !!document.querySelector('.modal-container'), notices: window.__p.notices(), noticeHtml: Array.from(document.querySelectorAll('.notice')).map((n) => n.outerHTML) };`);
  r.undo = await ev(`const n = Array.from(document.querySelectorAll('.notice')).find((n) => /Undo/.test(n.textContent)); if (!n) return null; const b = Array.from(n.querySelectorAll('*')).reverse().find((e) => /^Undo$/.test(e.textContent.trim())) || n; b.click(); await window.__p.sleep(2500); return { clicked: b.tagName + '.' + b.className, notices: window.__p.notices(), modal: window.__p.modal() };`);
  r.afterUndo = {};
  for (const [p, t] of Object.entries(FM)) { const now = fs.readFileSync(vp(p), 'utf8'); r.afterUndo[p] = now === t ? 'original' : now === r.files[p].after ? 'converted' : now; }
  await dismiss(); await clearNotices();
  // A second conversion over what the first left (the undo state aside).
  for (const [p, t] of Object.entries(FM)) fs.writeFileSync(vp(p), r.files[p].after);
  await sleep(2000);
  const m2 = mt();
  await run('app:migrate-frontmatter'); await sleep(800);
  await ev(`document.querySelector('.modal-container .modal .mod-cta').click(); return true;`);
  await sleep(4000);
  r.second = { modal: await modal(), notices: await notices(), touched: Object.entries(mt()).filter(([p, t]) => t !== m2[p]).map(([p]) => p) };
  await dismiss();
});

// ---- s05 debug info --------------------------------------------------------
const debugText = () => ev(`const t = document.querySelector('.modal-container .modal textarea'); return t ? t.value : null;`);
await step('s05-debug-info', async (r) => {
  await reset();
  await write('Note.md', 'x\n');
  await ev(`await window.__p.only('Note.md'); return true;`);
  await run('app:show-debug-info'); await sleep(800);
  r.modal = await modal();
  r.text = await debugText();
  r.calls = await take();
  r.copy = await ev(`const b = Array.from(document.querySelectorAll('.modal-container .modal button')).find((b) => b.textContent === 'Copy to clipboard'); b.click(); await window.__p.sleep(600);
    let clip = null; try { clip = require('electron').clipboard.readText(); } catch (e) { clip = 'err ' + e.message; }
    return { clip, notices: window.__p.notices(), modalLeft: !!document.querySelector('.modal-container'), buttonText: b.textContent };`);
  r.copy.equalsText = r.copy.clip === r.text;
  r.copy.calls = await take();
  await clearNotices(); await dismiss();
  await run('app:show-debug-info'); await sleep(800); await take();
  r.sandbox = await ev(`const b = Array.from(document.querySelectorAll('.modal-container .modal button')).find((b) => b.textContent === 'Sandbox vault'); if (!b) return null; b.click(); await window.__p.sleep(1000);
    return { notices: window.__p.notices(), modalLeft: !!document.querySelector('.modal-container'), modal: window.__p.modal() };`);
  r.sandbox.calls = await take();
  r.sandbox.sandboxPath = await ev(`try { return require('electron').ipcRenderer.sendSync('get-sandbox-vault-path'); } catch (e) { return 'err ' + e.message; }`);
  await dismiss();
  await run('app:show-debug-info'); await sleep(800);
  r.done = await ev(`const b = Array.from(document.querySelectorAll('.modal-container .modal button')).find((b) => b.textContent === 'Done'); b.click(); await window.__p.sleep(500); return { modalLeft: !!document.querySelector('.modal-container'), notices: window.__p.notices() };`);
  await run('app:show-debug-info'); await sleep(800);
  r.escape = await (async () => { await c.pressKey('Escape'); await sleep(500); return { modalLeft: !!(await modal()) }; })();
  await dismiss();
  // A second configuration: Source mode default, a light base theme, a
  // community theme and snippet named, community plugins on with one plugin.
  fs.mkdirSync(vp('.obsidian/themes/Probe Theme'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/themes/Probe Theme/manifest.json'), '{"name":"Probe Theme","version":"1.0.0","minAppVersion":"1.0.0","author":"probe"}');
  fs.writeFileSync(vp('.obsidian/themes/Probe Theme/theme.css'), 'body{}');
  fs.mkdirSync(vp('.obsidian/snippets'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/snippets/probe-snip.css'), 'body{}');
  fs.mkdirSync(vp('.obsidian/plugins/probe-noop'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/plugins/probe-noop/manifest.json'), '{"id":"probe-noop","name":"Probe Noop","version":"0.0.1","minAppVersion":"1.0.0","description":"","author":"probe","isDesktopOnly":false}');
  fs.writeFileSync(vp('.obsidian/plugins/probe-noop/main.js'), 'const { Plugin } = require("obsidian"); module.exports = class extends Plugin {};');
  r.config2 = await ev(`
    app.vault.setConfig('livePreview', false);
    app.changeTheme && app.changeTheme('moonstone'); app.vault.setConfig('theme', 'moonstone');
    try { await app.customCss.setTheme('Probe Theme'); } catch (e) {}
    try { app.customCss.setCssEnabledStatus('probe-snip', true); } catch (e) {}
    let plug = null;
    try { await app.plugins.setEnable(true); await app.plugins.loadManifests(); await app.plugins.enablePluginAndSave('probe-noop'); plug = Object.keys(app.plugins.plugins); } catch (e) { plug = 'err ' + e.message; }
    await window.__p.sleep(1000);
    return { plug, theme: app.vault.getConfig('theme'), css: app.vault.getConfig('cssTheme'), restricted: app.plugins.isEnabled ? !app.plugins.isEnabled() : null };`);
  await dismiss();
  await run('app:show-debug-info'); await sleep(800);
  r.text2 = await debugText();
  await dismiss();
  r.config2.undo = await ev(`try { await app.plugins.disablePluginAndSave('probe-noop'); await app.plugins.setEnable(false); } catch (e) {} try { await app.customCss.setTheme(''); } catch (e) {} try { app.customCss.setCssEnabledStatus('probe-snip', false); } catch (e) {} app.vault.setConfig('livePreview', true); app.vault.setConfig('theme', 'obsidian'); return true;`);
});

// ---- s06 release notes offline ---------------------------------------------
await step('s06-release-notes-offline', async (r) => {
  await reset();
  const ls = () => ev(`const o = {}; for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); if (/release|changelog|version|whats/i.test(k)) o[k] = String(localStorage.getItem(k)).slice(0, 200); } return o;`);
  r.localStorageBefore = await ls();
  const show = async () => {
    await clearNotices();
    await run('app:show-release-notes');
    await sleep(4000);
    const v = await ev(`const l = app.workspace.activeLeaf; return { viewType: l.view.getViewType(), state: l.getViewState(), title: l.view.getDisplayText(),
      content: (l.view.contentEl.innerText || '').slice(0, 600), html: l.view.contentEl.innerHTML.replace(/<svg[\\s\\S]*?<\\/svg>/g, '<svg/>').slice(0, 1500), tabs: document.querySelectorAll('.mod-root .workspace-tab-header').length };`);
    v.notices = await notices();
    v.calls = (await take()).filter((x) => /request-url/.test(x.kind)).map((x) => x.kind + ' ' + JSON.stringify(x.args).slice(0, 300));
    return v;
  };
  r.online = await show();
  await ev(`window.__p.leaf().slice(1).forEach((l) => l.detach()); return true;`);
  r.localStorageAfterOnline = await ls();
  // Network emulation does not reach the main process's requests (net.fetch
  // still answered 200 under it), so the request-url handler is wrapped to ask
  // a closed local port instead: a real connection failure, as offline gives.
  r.offlineSet = await mainEval(`const { ipcMain } = require('electron'); const ls = ipcMain.listeners('request-url'); global.__reqUrl = ls; ipcMain.removeAllListeners('request-url');
    ipcMain.on('request-url', (ev, id, req) => { const u = String(req && req.url || ''); const r2 = Object.assign({}, req, { url: u.replace(/^https?:\\/\\/[^/]+/, 'https://127.0.0.1:1') }); for (const l of ls) l(ev, id, r2); }); return ls.length;`);
  r.offline = await show();
  await ev(`window.__p.leaf().slice(1).forEach((l) => l.detach()); return true;`);
  await mainEval(`const { ipcMain } = require('electron'); ipcMain.removeAllListeners('request-url'); for (const l of global.__reqUrl) ipcMain.on('request-url', l); return true;`);
  r.backOnline = await show();
  r.localStorageAfter = await ls();
});

// ---- s07 open-file ---------------------------------------------------------
await step('s07-open-file', async (r) => {
  await reset();
  await write('Note A.md', 'alpha\n');
  await write('Note B.md', 'bravo\n');
  await write('Sub/Deep.md', 'deep\n');
  fs.writeFileSync(vp('img.png'), PNG);
  await sleep(800);
  const st = () => ev(`const l = app.workspace.activeLeaf; const tabs = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')).map((h) => h.getAttribute('aria-label') + (h.classList.contains('is-active') ? '*' : ''));
    return { type: l.view.getViewType(), file: l.view.file ? l.view.file.path : null, fileName: l.view.file ? l.view.file.name : null, title: l.view.getDisplayText(), state: l.getViewState(), tabs, mode: l.view.getMode ? l.view.getMode() : null,
      inVault: l.view.file ? !!app.vault.getAbstractFileByPath(l.view.file.path) : null, headerTitle: (l.view.titleEl || {}).textContent || null, breadcrumbs: Array.from(l.view.containerEl.querySelectorAll('.view-header-breadcrumb')).map((b) => b.textContent),
      editorText: l.view.editor ? l.view.editor.getValue() : null };`);
  const one = async (label, target, from) => {
    const o = { target };
    await clearNotices();
    await ev(`await window.__p.only(${JSON.stringify(from)}); return true;`);
    o.before = await st();
    o.vaultBefore = listVault();
    await pick(target ? [target] : null);
    o.run = await run('app:open-file');
    await sleep(2500);
    o.dialogs = await dialogs();
    o.after = await st();
    o.vaultAfter = listVault();
    o.targets = (await pageTargets()).length;
    o.notices = await notices();
    o.calls = (await take()).map((x) => x.kind + ' ' + JSON.stringify(x.args).slice(0, 400));
    r[label] = o;
  };
  await one('insideFromNote', vp('Note B.md'), 'Note A.md');
  await one('insideFromEmpty', vp('Note B.md'), null);
  await one('insideSubFromNote', vp('Sub/Deep.md'), 'Note A.md');
  await one('insideAlreadyOpen', vp('Note A.md'), 'Note A.md');
  await one('outsideNoteFromNote', path.join(OUTSIDE, 'Outside Note.md'), 'Note A.md');
  // An edit in the outside note's tab: does it reach the outside file?
  r.outsideEdit = await ev(`const l = app.workspace.activeLeaf; const ed = l.view.editor; if (!ed) return { editor: false };
    ed.focus(); ed.setCursor({ line: 0, ch: 0 }); ed.replaceRange('EDIT ', { line: 0, ch: 0 }); await window.__p.sleep(3500);
    return { editor: true, text: ed.getValue(), path: l.view.file ? l.view.file.path : null };`);
  r.outsideEdit.disk = readOr(path.join(OUTSIDE, 'Outside Note.md'));
  r.outsideEdit.vault = listVault();
  await one('outsideNoteFromEmpty', path.join(OUTSIDE, 'Outside Note.md'), null);
  await one('outsideImage', path.join(OUTSIDE, 'outside.png'), 'Note A.md');
  await one('insideImage', vp('img.png'), 'Note A.md');
  await one('cancel', null, 'Note A.md');
  r.workspaceJson = readOr(vp('.obsidian/workspace.json'));
});

save();
c.close();
process.exit(0);
