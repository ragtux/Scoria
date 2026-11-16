// The File properties side pane (`properties:open-local`, view type
// `file-properties`) as a leaf: what the earlier runs left open
// ([[Specification#Observed add-alias behavior, Obsidian 1.15.0]] and the
// properties blocks).
//
//   s01  properties:open-local the first time: the leaf, its place, focus,
//        the dock, workspace.json
//   s02  properties:open-local again with another right-dock tab showing:
//        a second leaf, or the first revealed
//   s03  what the pane shows and is bound to under each active file: a note
//        with properties, one with none, a canvas, an image, a PDF, and no
//        file at all (every main leaf closed)
//   s04  the leaf's tab header menu under a real right click, then its Close:
//        the leaves, the tab the dock shows next, workspace.json
//   s05  properties:open-local after the close: where the new leaf goes
//   s06  workspace:close and Ctrl+W with the File properties leaf active
//   s07  markdown:add-alias in Source mode with the leaf open and with it
//        closed: where focus lands, typing, Enter, Escape, the bytes
//   s08  editing in the pane: a text value retyped, Add property with a key,
//        Tab and a value, a list value appended -- the bytes each writes
//
// Phases: fixtures | run   (node file-properties.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 420000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 90000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  out.steps[name] = { running: true, partial };
  try { const v = await withTimeout(fn(partial), ms, name); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  try { await cleanup(); } catch { /* ignore */ }
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
async function pollChange(rel, prev, timeout = 6000) {
  const t = Date.now();
  while (Date.now() - t < timeout) {
    const b = bytes(rel);
    if (b !== prev) { const ms = Date.now() - t; await sleep(400); return { changed: true, ms, bytes: bytes(rel) }; }
    await sleep(100);
  }
  return { changed: false, waitedMs: timeout, bytes: bytes(rel) };
}
/** The right dock of a workspace.json text. */
function wsRight(text) {
  let ws; try { ws = JSON.parse(text); } catch { return null; }
  if (!ws) return null;
  return { right: ws.right, active: ws.active };
}

const PROPS = '---\ntitle: Old title\ntags:\n  - one\nrelated: "[[Target]]"\n---\nBody.\n';
// A 1x1 PNG.
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
const PDF = '%PDF-1.1\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n';

// ---------------------------------------------------------------------------
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  const core = {
    'file-explorer': true, 'global-search': true, switcher: true, graph: false, backlink: true,
    'outgoing-link': true, 'tag-pane': true, properties: true, 'page-preview': true,
    'command-palette': true, 'editor-status': true, outline: true, 'file-recovery': true,
  };
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify(core, null, 2));
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('Props.md'), PROPS);
  fs.writeFileSync(vp('Target.md'), '# Target\n\nThe target.\n');
  fs.writeFileSync(vp('Plain.md'), 'No properties here.\n');
  fs.writeFileSync(vp('Alias.md'), 'Body only.\n');
  fs.writeFileSync(vp('Edit.md'), PROPS);
  fs.writeFileSync(vp('Board.canvas'), '{"nodes":[],"edges":[]}');
  fs.writeFileSync(vp('pic.png'), PNG);
  fs.writeFileSync(vp('doc.pdf'), PDF);
  out.files = fs.readdirSync(VAULT);
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

// ---------------------------------------------------------------------------
const c = await connect();
out.vaultPath = await c.vaultPath();
save();

await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  mdLeaf() {
    const ls = app.workspace.getLeavesOfType('markdown');
    if (ls.length) return ls[0];
    let main = null; app.workspace.iterateRootLeaves((l) => { if (!main) main = l; });
    return main || app.workspace.getLeaf(false);
  },
  async open(file, mode) {
    let f = null;
    for (let i = 0; i < 40 && !f; i++) { f = app.vault.getAbstractFileByPath(file); if (!f) await this.sleep(150); }
    let leaf = null; app.workspace.iterateRootLeaves((l) => { if (!leaf) leaf = l; });
    leaf = leaf || app.workspace.getLeaf(false);
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(500);
    if (mode && leaf.view.getViewType() === 'markdown') await this.mode(mode, leaf);
    return { file: leaf.view.file && leaf.view.file.path, type: leaf.view.getViewType(), state: leaf.getViewState().state };
  },
  async mode(m, leaf) {
    leaf = leaf || this.mdLeaf();
    const st = leaf.getViewState();
    const s = m === 'preview' ? { mode: 'preview' } : { mode: 'source', source: m === 'source' };
    st.state = Object.assign({}, st.state, s);
    await leaf.setViewState(st);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(350);
    return leaf.getViewState().state;
  },
  async seed(file, content) {
    const f = app.vault.getAbstractFileByPath(file);
    await app.vault.modify(f, content);
    await this.sleep(900);
    return true;
  },
  describe(el) {
    if (!el) return null;
    return { tag: el.tagName, cls: typeof el.className === 'string' ? el.className : (el.className && el.className.baseVal), text: (el.textContent || '').trim().slice(0, 160), aria: el.getAttribute && el.getAttribute('aria-label') };
  },
  active() {
    const a = document.activeElement; const d = this.describe(a);
    if (a) {
      const leafEl = a.closest('.workspace-leaf-content');
      d.inLeaf = leafEl ? leafEl.getAttribute('data-type') : null;
      d.inRightSplit = !!a.closest('.mod-right-split');
      const row = a.closest('.metadata-property'); d.rowKey = row ? row.getAttribute('data-property-key') : null;
      if ('selectionStart' in a) { try { d.selStart = a.selectionStart; d.selEnd = a.selectionEnd; d.value = a.value; } catch {} }
      if (a.isContentEditable) { const s = window.getSelection(); d.contentEditable = true; d.selText = s && s.toString(); }
    }
    return d;
  },
  leaves() {
    const o = [];
    app.workspace.iterateAllLeaves((l) => {
      const root = l.getRoot && l.getRoot();
      const where = root === app.workspace.leftSplit ? 'left' : root === app.workspace.rightSplit ? 'right' : root === app.workspace.rootSplit ? 'main' : 'other';
      const p = l.parent;
      o.push({ id: l.id, type: l.view.getViewType(), where, index: p && p.children ? p.children.indexOf(l) : null,
        groupActive: p && p.children ? p.children.indexOf(p.children.find(x => x.containerEl && x.containerEl.isShown && x.containerEl.isShown()) ) : null,
        display: l.getDisplayText(), icon: l.view.getIcon && l.view.getIcon(), state: l.getViewState(),
        isActive: app.workspace.activeLeaf === l, shown: !!(l.containerEl && l.containerEl.offsetParent) });
    });
    return o;
  },
  fp() { return app.workspace.getLeavesOfType('file-properties'); },
  pane() {
    const l = this.fp()[0]; if (!l) return null;
    const el = l.view.containerEl;
    const vis = (s) => { const e = el.querySelector(s); return e ? { present: true, shown: !!e.offsetParent, text: (e.innerText || '').trim().slice(0, 200) } : { present: false }; };
    return { file: l.view.file ? l.view.file.path : null, state: l.getViewState().state, text: (el.innerText || '').trim().slice(0, 600),
      rows: Array.from(el.querySelectorAll('.metadata-property')).map(r => r.getAttribute('data-property-key')),
      add: vis('.metadata-add-button'), empty: vis('.pane-empty'), heading: vis('.metadata-properties-heading'),
      container: vis('.metadata-container'), shown: !!el.offsetParent };
  },
  menus() {
    return Array.from(document.querySelectorAll('.menu')).map((m) => Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((it) => it.classList.contains('menu-separator') ? '---' : {
      title: ((it.querySelector('.menu-item-title') || it).textContent || '').trim(),
      icon: (() => { const s = it.querySelector('.menu-item-icon svg'); return s ? s.getAttribute('class') : null; })(),
      disabled: it.classList.contains('is-disabled'), section: it.getAttribute('data-section'),
    }));
  },
  rightTabs() {
    return Array.from(document.querySelectorAll('.mod-right-split .workspace-tab-header')).map(h => ({ type: h.getAttribute('data-type'), active: h.classList.contains('is-active'), aria: h.getAttribute('aria-label') }));
  },
};
return true;`);

const ev = (body) => c.evaluate(body);
const P = (body) => c.evaluate(`return await (async () => { const p = window.__p; ${body} })();`);
async function cleanup() {
  for (let i = 0; i < 3; i++) {
    const n = await ev(`return document.querySelectorAll('.modal-container, .menu, .suggestion-container').length;`);
    if (!n) return;
    await c.pressKey('Escape'); await sleep(200);
  }
  await ev(`document.querySelectorAll('.menu').forEach(m => m.remove()); return true;`);
}
const runCmd = (id) => ev(`return app.commands.executeCommandById(${JSON.stringify(id)});`);
const saveLayout = async () => { const before = bytes('.obsidian/workspace.json'); await ev(`app.workspace.requestSaveLayout(); return true;`); return pollChange('.obsidian/workspace.json', before, 4000); };
const snapshot = async (r, tag) => {
  r[tag] = {
    leaves: await P(`return p.leaves().filter(l => l.where !== 'left');`),
    rightTabs: await P(`return p.rightTabs();`),
    rightCollapsed: await ev(`return app.workspace.rightSplit.collapsed;`),
    active: await P(`return p.active();`),
    activeLeafType: await ev(`const l = app.workspace.activeLeaf; return l ? l.view.getViewType() : null;`),
    pane: await P(`return p.pane();`),
  };
};

// ---------------------------------------------------------------------------
await step('s00-context', async (r) => {
  r.versions = await ev(`return { title: document.title, apiVersion: String(app.apiVersion), ua: navigator.userAgent };`);
  r.cmds = await ev(`const o = {}; for (const id of ['properties:open-local','markdown:add-alias','workspace:close']) { const x = app.commands.commands[id] || (app.commands.editorCommands || {})[id]; o[id] = x ? { name: x.name, icon: x.icon || null, check: !!x.checkCallback, editor: !!(x.editorCallback || x.editorCheckCallback), keys: (app.hotkeyManager.defaultKeys || {})[id] || null } : null; } return o;`);
  r.listed = await ev(`return app.commands.listCommands().filter(x => /properties:open-local|add-alias/.test(x.id)).map(x => x.id);`);
  await snapshot(r, 'start');
  r.ws = wsRight(bytes('.obsidian/workspace.json'));
  return r;
});

await step('s01-open-first', async (r) => {
  r.opened = await P(`return await p.open('Props.md', 'live');`);
  await ev(`app.workspace.rightSplit.collapse(); return true;`); await sleep(300);
  r.ran = await runCmd('properties:open-local'); await sleep(800);
  await snapshot(r, 'after');
  r.ws = await saveLayout();
  r.wsRight = wsRight(r.ws.bytes);
  return r;
});

await step('s02-open-again', async (r) => {
  // Show Backlinks in the dock, then ask for File properties again.
  await ev(`const l = app.workspace.getLeavesOfType('backlink')[0]; if (l) app.workspace.revealLeaf(l); return !!l;`); await sleep(500);
  await snapshot(r, 'before');
  r.ran = await runCmd('properties:open-local'); await sleep(800);
  await snapshot(r, 'after');
  r.count = await ev(`return app.workspace.getLeavesOfType('file-properties').length;`);
  return r;
});

await step('s03-bound-file', async (r) => {
  for (const f of ['Props.md', 'Plain.md', 'Board.canvas', 'pic.png', 'doc.pdf', 'Props.md']) {
    const o = await P(`return await p.open(${JSON.stringify(f)}, 'live');`); await sleep(900);
    r[f + (r[f] ? '#2' : '')] = { opened: o, pane: await P(`return p.pane();`), listed: await ev(`return app.commands.listCommands().some(x => x.id === 'properties:open-local');`) };
  }
  // No file: every main leaf detached, so the root holds one empty leaf.
  await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l)); for (const l of ls) l.detach(); return ls.length;`); await sleep(900);
  r.noFile = { pane: await P(`return p.pane();`), mainLeaves: await ev(`const o = []; app.workspace.iterateRootLeaves((l) => o.push(l.view.getViewType())); return o;`), listed: await ev(`return app.commands.listCommands().some(x => x.id === 'properties:open-local');`) };
  r.ranNoFile = await runCmd('properties:open-local'); await sleep(600);
  r.noFileAfterRun = await P(`return p.pane();`);
  r.ws = wsRight((await saveLayout()).bytes);
  await P(`return await p.open('Props.md', 'live');`); await sleep(800);
  r.reopened = await P(`return p.pane();`);
  return r;
});

await step('s04-tab-menu-close', async (r) => {
  await ev(`const l = app.workspace.getLeavesOfType('file-properties')[0]; app.workspace.revealLeaf(l); return true;`); await sleep(500);
  const sel = '.mod-right-split .workspace-tab-header[data-type="file-properties"]';
  r.hit = await c.rightClickElement(sel); await sleep(500);
  r.menus = await P(`return p.menus();`);
  // Close from the menu.
  r.closeClick = await ev(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find(x => (x.textContent || '').trim() === 'Close'); if (!it) return null; const b = it.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (r.closeClick) { await c.clickAt(r.closeClick.x, r.closeClick.y); await sleep(800); }
  await snapshot(r, 'after');
  r.ws = await saveLayout();
  r.wsRight = wsRight(r.ws.bytes);
  // The menu on another side tab, for comparison.
  r.backlinkHit = await c.rightClickElement('.mod-right-split .workspace-tab-header[data-type="backlink"]'); await sleep(500);
  r.backlinkMenus = await P(`return p.menus();`);
  return r;
});

await step('s05-reopen', async (r) => {
  await P(`return await p.open('Props.md', 'live');`);
  await ev(`const l = app.workspace.getLeavesOfType('outline')[0]; if (l) app.workspace.revealLeaf(l); return !!l;`); await sleep(400);
  await snapshot(r, 'before');
  r.ran = await runCmd('properties:open-local'); await sleep(800);
  await snapshot(r, 'after');
  r.ws = await saveLayout();
  r.wsRight = wsRight(r.ws.bytes);
  return r;
});

await step('s06-workspace-close', async (r) => {
  // The leaf made active through a click in its empty space, then Ctrl+W.
  await ev(`const l = app.workspace.getLeavesOfType('file-properties')[0]; app.workspace.revealLeaf(l); return true;`); await sleep(400);
  const box = await ev(`const l = app.workspace.getLeavesOfType('file-properties')[0]; const b = l.view.containerEl.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height - 30 };`);
  await c.clickAt(box.x, box.y); await sleep(400);
  await snapshot(r, 'beforeKey');
  await c.pressKey('w', { ctrl: true }); await sleep(800);
  await snapshot(r, 'afterCtrlW');
  // And by id, with the leaf made active through the API.
  await runCmd('properties:open-local'); await sleep(600);
  await ev(`const l = app.workspace.getLeavesOfType('file-properties')[0]; app.workspace.setActiveLeaf(l, { focus: true }); return true;`); await sleep(300);
  r.activeBeforeCmd = await ev(`const l = app.workspace.activeLeaf; return l ? l.view.getViewType() : null;`);
  r.ranClose = await runCmd('workspace:close'); await sleep(800);
  await snapshot(r, 'afterCmd');
  return r;
});

const aliasCase = async (r, haveLeaf) => {
  await P(`return await p.open('Alias.md', 'source');`);
  await P(`return await p.seed('Alias.md', 'Body only.\\n');`);
  if (haveLeaf) { await runCmd('properties:open-local'); await sleep(500); await ev(`const l = app.workspace.getLeavesOfType('backlink')[0]; if (l) app.workspace.revealLeaf(l); return true;`); }
  else await ev(`for (const l of app.workspace.getLeavesOfType('file-properties')) l.detach(); app.workspace.rightSplit.collapse(); return true;`);
  await sleep(500);
  await P(`const l = p.mdLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); l.view.editor.focus(); l.view.editor.setCursor({ line: 0, ch: 0 }); return true;`); await sleep(300);
  await snapshot(r, 'before');
  const before = bytes('Alias.md');
  r.ran = await runCmd('markdown:add-alias'); await sleep(500);
  await snapshot(r, 'afterRun');
  r.diskAfterRun = await pollChange('Alias.md', before, 2500);
  await c.typeText('Al2', 30); await sleep(250);
  r.activeAfterType = await P(`return p.active();`);
  const mid = bytes('Alias.md');
  await c.pressKey('Enter'); await sleep(400);
  r.activeAfterEnter = await P(`return p.active();`);
  r.diskAfterEnter = await pollChange('Alias.md', mid, 3000);
  await c.pressKey('Escape'); await sleep(400);
  r.activeAfterEscape = await P(`return p.active();`);
  await c.pressKey('Escape'); await sleep(400);
  r.activeAfterSecondEscape = await P(`return p.active();`);
  r.activeLeafAfter = await ev(`const l = app.workspace.activeLeaf; return l ? l.view.getViewType() : null;`);
};
await step('s07-alias-source-leaf-open', async (r) => { await aliasCase(r, true); return r; });
await step('s07-alias-source-leaf-closed', async (r) => { await aliasCase(r, false); return r; });

await step('s08-edit-in-pane', async (r) => {
  await P(`return await p.open('Edit.md', 'source');`);
  await P(`return await p.seed('Edit.md', ${JSON.stringify(PROPS)});`);
  await runCmd('properties:open-local'); await sleep(600);
  const fpSel = '.mod-right-split .workspace-leaf-content[data-type="file-properties"]';
  // A text value retyped.
  let prev = bytes('Edit.md');
  r.valueClick = await c.clickElement(fpSel + ' .metadata-property[data-property-key="title"] .metadata-property-value');
  await sleep(300);
  r.activeOnValue = await P(`return p.active();`);
  await c.pressKey('a', { ctrl: true }); await c.typeText('New title', 25); await sleep(200);
  await c.pressKey('Enter'); await sleep(300);
  r.activeAfterValueEnter = await P(`return p.active();`);
  r.diskValue = await pollChange('Edit.md', prev, 3000);
  // A tag appended to the list.
  prev = bytes('Edit.md');
  r.tagsClick = await c.clickElement(fpSel + ' .metadata-property[data-property-key="tags"] .multi-select-input');
  await sleep(300);
  r.activeOnTags = await P(`return p.active();`);
  await c.typeText('two', 25); await c.pressKey('Enter'); await sleep(300);
  r.diskTags = await pollChange('Edit.md', prev, 3000);
  await c.pressKey('Escape'); await sleep(300);
  // Add property: a key, Tab, a value, Enter.
  prev = bytes('Edit.md');
  r.addClick = await c.clickElement(fpSel + ' .metadata-add-button');
  await sleep(400);
  r.activeOnAdd = await P(`return p.active();`);
  await c.typeText('status', 25); await sleep(400);
  r.suggestOpen = await ev(`return document.querySelectorAll('.suggestion-container').length;`);
  await c.pressKey('Tab'); await sleep(300);
  r.activeAfterKeyTab = await P(`return p.active();`);
  r.diskAfterKey = await pollChange('Edit.md', prev, 2000);
  prev = r.diskAfterKey.bytes;
  await c.typeText('done', 25); await c.pressKey('Enter'); await sleep(300);
  r.activeAfterValueEnter2 = await P(`return p.active();`);
  r.diskAdd = await pollChange('Edit.md', prev, 3000);
  r.pane = await P(`return p.pane();`);
  // The type icon's menu in the pane.
  r.iconClick = await c.clickElement(fpSel + ' .metadata-property[data-property-key="status"] .metadata-property-icon');
  await sleep(400);
  r.iconMenus = await P(`return p.menus();`);
  await c.pressKey('Escape'); await sleep(300);
  return r;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
