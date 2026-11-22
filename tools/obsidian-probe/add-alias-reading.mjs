// `markdown:add-alias` in reading view: what the 2026-11-09 run (`SCEN=al`
// of panes-workspaces-headings.mjs) left open there.
//
//   s00  versions; the command's entry; the palette's rows in reading view
//        over a note with no frontmatter and one with an aliases list
//   s01  propertiesInDocument "visible", no frontmatter / a list / other keys,
//        the File properties pane closed: focus, the key's timing, the bytes
//        after the first keystroke, after the rest, after Enter, after Escape,
//        and where a second Escape puts the keyboard
//   s02  the same with the pane open behind Backlinks
//   s03  propertiesInDocument "hidden" and "source", pane closed and open
//   s04  the reading view's properties block collapsed (its heading clicked)
//   s05  an alias typed and left by a click in the note's body (blur)
//   s06  Enter with nothing typed, over a list and over no frontmatter
//
// Phases: fixtures | run   (node add-alias-reading.mjs <phase>)
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
const BUDGET_MS = 600000;
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
async function pollChange(rel, prev, timeout = 3000) {
  const t = Date.now();
  while (Date.now() - t < timeout) {
    const b = bytes(rel);
    if (b !== prev) { const ms = Date.now() - t; await sleep(300); return { changed: true, ms, bytes: bytes(rel) }; }
    await sleep(100);
  }
  return { changed: false, waitedMs: timeout, bytes: bytes(rel) };
}

const SHAPES = {
  none: 'Body only.\n',
  list: '---\naliases:\n  - one\n  - two\n---\nBody.\n',
  otherKeys: '---\ntitle: T\ntags: x\n---\nBody.\n',
};

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
  fs.writeFileSync(vp('Alias.md'), SHAPES.none);
  fs.writeFileSync(vp('Listed.md'), SHAPES.list);
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
    return { tag: el.tagName, cls: typeof el.className === 'string' ? el.className : (el.className && el.className.baseVal), text: (el.textContent || '').trim().slice(0, 160) };
  },
  active() {
    const a = document.activeElement; const d = this.describe(a);
    if (a) {
      const leafEl = a.closest('.workspace-leaf-content');
      d.inLeaf = leafEl ? leafEl.getAttribute('data-type') : null;
      d.inRightSplit = !!a.closest('.mod-right-split');
      d.inReading = !!a.closest('.markdown-reading-view');
      d.inSource = !!a.closest('.markdown-source-view');
      const row = a.closest('.metadata-property'); d.rowKey = row ? row.getAttribute('data-property-key') : null;
      if (a.isContentEditable) { d.contentEditable = true; d.typed = a.textContent; }
      const b = a.getBoundingClientRect(); d.rect = [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)];
      d.visible = !!(a.offsetWidth || a.offsetHeight);
    }
    const l = app.workspace.activeLeaf; d.activeLeaf = l ? l.view.getViewType() : null;
    return d;
  },
  reading() {
    const v = this.mdLeaf().view;
    const el = v.previewMode && v.previewMode.containerEl;
    const m = el && el.querySelector('.metadata-container');
    const fm = el && el.querySelector('.frontmatter, pre.frontmatter, .el-pre.mod-frontmatter');
    return { mode: v.getMode(), block: m ? { cls: m.className, shown: !!m.offsetParent, collapsed: m.classList.contains('is-collapsed'), text: (m.innerText || '').trim().slice(0, 300), rows: Array.from(m.querySelectorAll('.metadata-property')).map(r => r.getAttribute('data-property-key')), pills: Array.from(m.querySelectorAll('.multi-select-pill')).map(x => x.textContent) } : null,
      yaml: fm ? { cls: fm.className, shown: !!fm.offsetParent, text: (fm.innerText || '').slice(0, 200) } : null };
  },
  pane() {
    const l = app.workspace.getLeavesOfType('file-properties')[0]; if (!l) return null;
    const el = l.view.containerEl;
    return { file: l.view.file ? l.view.file.path : null, shown: !!el.offsetParent, rows: Array.from(el.querySelectorAll('.metadata-property')).map(r => r.getAttribute('data-property-key')),
      pills: Array.from(el.querySelectorAll('.multi-select-pill')).map(x => x.textContent), active: app.workspace.activeLeaf === l };
  },
  dock() {
    return { collapsed: app.workspace.rightSplit.collapsed, fpLeaves: app.workspace.getLeavesOfType('file-properties').length,
      tabs: Array.from(document.querySelectorAll('.mod-right-split .workspace-tab-header')).map(h => (h.classList.contains('is-active') ? '*' : '') + h.getAttribute('data-type')) };
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
}
const runCmd = (id) => ev(`return app.commands.executeCommandById(${JSON.stringify(id)});`);
const listed = () => ev(`return app.commands.listCommands().some(x => x.id === 'markdown:add-alias');`);
const setPid = async (v) => { await ev(`app.vault.setConfig('propertiesInDocument', ${JSON.stringify(v)}); return true;`); await sleep(600); };
const snap = async () => ({ active: await P(`return p.active();`), reading: await P(`return p.reading();`), pane: await P(`return p.pane();`), dock: await P(`return p.dock();`) });

async function palette(query) {
  await runCmd('command-palette:open'); await sleep(400);
  await c.typeText(query, 20); await sleep(500);
  const rows = await ev(`return Array.from(document.querySelectorAll('.prompt .suggestion-item')).map(x => (x.innerText || '').trim());`);
  await c.pressKey('Escape'); await sleep(300);
  return rows;
}

/** Prepare: `file` seeded, reading view, the pane open behind Backlinks or closed with the dock collapsed. */
async function prepare(r, { file = 'Alias.md', seed, pane, pid = 'visible' }) {
  await setPid(pid);
  await P(`return await p.open(${JSON.stringify(file)}, 'live');`);
  await P(`return await p.seed(${JSON.stringify(file)}, ${JSON.stringify(seed)});`);
  if (pane) { await runCmd('properties:open-local'); await sleep(500); await ev(`const l = app.workspace.getLeavesOfType('backlink')[0]; if (l) app.workspace.revealLeaf(l); return true;`); }
  else await ev(`for (const l of app.workspace.getLeavesOfType('file-properties')) l.detach(); app.workspace.rightSplit.collapse(); return true;`);
  await sleep(400);
  r.mode = await P(`return await p.mode('preview');`);
  await P(`const l = p.mdLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); return true;`); await sleep(1500);
  r.listed = await listed();
  r.before = await snap();
  r.diskBefore = bytes(file);
}

/** Run the command and type `value` one key at a time, reading the disk after the first and the last key. */
async function aliasCase(r, opts) {
  const file = opts.file || 'Alias.md';
  await prepare(r, opts);
  const before = bytes(file);
  r.ran = await runCmd('markdown:add-alias'); await sleep(400);
  r.afterRun = await snap();
  r.diskAfterRun = await pollChange(file, before, 2000);
  if (opts.value === undefined) return;
  const v = opts.value;
  let prev = bytes(file);
  if (v.length) {
    await c.typeText(v[0], 30); await sleep(200);
    r.diskAfterFirstKey = await pollChange(file, prev, 1500);
    prev = bytes(file);
    await c.typeText(v.slice(1), 30); await sleep(200);
    r.activeAfterType = await P(`return p.active();`);
    r.diskAfterTyping = await pollChange(file, prev, 2500);
    prev = bytes(file);
  }
  if (opts.leave === 'blur') {
    const pt = await ev(`const v = window.__p.mdLeaf().view; const e = Array.from(v.previewMode.containerEl.querySelectorAll('p')).pop(); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 10, y: b.y + b.height / 2 };`);
    r.blurAt = pt;
    if (pt) await c.clickAt(pt.x, pt.y);
    await sleep(400);
    r.afterBlur = await snap();
    r.diskAfterBlur = await pollChange(file, prev, 3000);
    return;
  }
  await c.pressKey('Enter'); await sleep(400);
  r.afterEnter = await snap();
  r.diskAfterEnter = await pollChange(file, prev, 2500);
  prev = bytes(file);
  await c.pressKey('Escape'); await sleep(400);
  r.afterEscape = await snap();
  r.diskAfterEscape = await pollChange(file, prev, 1500);
  await c.pressKey('Escape'); await sleep(400);
  r.afterSecondEscape = await snap();
  r.diskFinal = bytes(file);
}

// ---------------------------------------------------------------------------
await step('s00-context', async (r) => {
  r.versions = await ev(`return { title: document.title, apiVersion: String(app.apiVersion), ua: navigator.userAgent };`);
  r.cmd = await ev(`const x = app.commands.commands['markdown:add-alias']; return x ? { name: x.name, icon: x.icon || null, check: !!x.checkCallback, editor: !!(x.editorCallback || x.editorCheckCallback) } : null;`);
  await P(`return await p.open('Alias.md', 'preview');`); await sleep(500);
  r.readingNone = { listed: await listed(), palette: await palette('add alias') };
  await P(`return await p.open('Listed.md', 'preview');`); await sleep(500);
  r.readingList = { listed: await listed(), palette: await palette('add alias') };
  for (const pid of ['hidden', 'source']) {
    await setPid(pid);
    await P(`return await p.open('Alias.md', 'preview');`); await sleep(500);
    r['reading-' + pid] = { listed: await listed(), palette: await palette('add alias'), reading: await P(`return p.reading();`) };
  }
  await setPid('visible');
  return r;
});

for (const [k, seed] of Object.entries(SHAPES)) {
  await step('s01-visible-closed-' + k, async (r) => { await aliasCase(r, { seed, pane: false, value: 'NewAlias' }); return r; });
}
await step('s02-visible-open-none', async (r) => { await aliasCase(r, { seed: SHAPES.none, pane: true, value: 'NewAlias' }); return r; });
await step('s02-visible-open-list', async (r) => { await aliasCase(r, { seed: SHAPES.list, pane: true, value: 'NewAlias' }); return r; });
for (const pid of ['hidden', 'source']) {
  await step(`s03-${pid}-closed-none`, async (r) => { await aliasCase(r, { seed: SHAPES.none, pane: false, pid, value: 'NewAlias' }); return r; });
  await step(`s03-${pid}-open-list`, async (r) => { await aliasCase(r, { seed: SHAPES.list, pane: true, pid, value: 'NewAlias' }); return r; });
}
await step('s04-collapsed-list', async (r) => {
  await prepare(r, { seed: SHAPES.list, pane: false });
  r.headClick = await c.clickElement('.workspace-leaf.mod-active .markdown-reading-view .metadata-properties-heading');
  await sleep(500);
  r.afterFold = await snap();
  r.wsFold = await ev(`return window.__p.mdLeaf().getViewState().state;`);
  const before = bytes('Alias.md');
  r.ran = await runCmd('markdown:add-alias'); await sleep(400);
  r.afterRun = await snap();
  r.diskAfterRun = await pollChange('Alias.md', before, 1500);
  await c.typeText('NewAlias', 30); await c.pressKey('Enter'); await sleep(400);
  r.afterEnter = await snap();
  r.diskAfterEnter = await pollChange('Alias.md', before, 2500);
  await c.pressKey('Escape'); await sleep(300); await c.pressKey('Escape'); await sleep(300);
  // Unfold again for later steps.
  const st = await P(`return p.reading();`);
  if (st.block && st.block.collapsed) { await c.clickElement('.workspace-leaf.mod-active .markdown-reading-view .metadata-properties-heading'); await sleep(300); }
  return r;
});
await step('s04-collapsed-none', async (r) => {
  // A fold then the key added: does the new block arrive folded?
  await prepare(r, { seed: SHAPES.otherKeys, pane: false });
  r.headClick = await c.clickElement('.workspace-leaf.mod-active .markdown-reading-view .metadata-properties-heading');
  await sleep(500);
  r.afterFold = await snap();
  const before = bytes('Alias.md');
  r.ran = await runCmd('markdown:add-alias'); await sleep(400);
  r.afterRun = await snap();
  r.diskAfterRun = await pollChange('Alias.md', before, 1500);
  await c.pressKey('Escape'); await sleep(300); await c.pressKey('Escape'); await sleep(300);
  const st = await P(`return p.reading();`);
  if (st.block && st.block.collapsed) { await c.clickElement('.workspace-leaf.mod-active .markdown-reading-view .metadata-properties-heading'); await sleep(300); }
  return r;
});
await step('s05-blur-visible-list', async (r) => { await aliasCase(r, { seed: SHAPES.list, pane: false, value: 'Blurred', leave: 'blur' }); return r; });
await step('s05-blur-visible-none', async (r) => { await aliasCase(r, { seed: SHAPES.none, pane: false, value: 'Blurred', leave: 'blur' }); return r; });
await step('s06-empty-enter-list', async (r) => { await aliasCase(r, { seed: SHAPES.list, pane: false, value: '' }); return r; });
await step('s06-empty-enter-none', async (r) => { await aliasCase(r, { seed: SHAPES.none, pane: false, value: '' }); return r; });
await step('s06-run-only-none-blur', async (r) => {
  // The command, nothing typed, a click away: is the bare key kept?
  await aliasCase(r, { seed: SHAPES.none, pane: false, value: '', leave: 'blur' });
  return r;
});

await setPid('visible');
save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
