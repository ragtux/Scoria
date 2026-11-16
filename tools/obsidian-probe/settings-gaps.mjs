// Four gaps the differences page listed, probed together on 2026-11-15:
//
//   s01  "Show tab title bar" (showViewHeader): which `.view-header`s it hides
//        (a note, the empty tab, a canvas, the docks, stacked tabs, a popout),
//        what a click on the row writes, and whether it takes at once
//   s02  "Always focus new tabs" (focusNewTab): every way a file opens in a new
//        tab, with the key on and off: which tab is active afterwards, where the
//        new tab sits in the strip, and where the keyboard is
//   s03  Ctrl+Enter in the split-file and extract-heading prompts: what it
//        creates, under which name, and what the source becomes
//   s04  File recovery's Settings buttons: what View opens on, Clear's question,
//        what Cancel and Clear leave in the snapshot store
//   s05  the template file picker on the Note composer, Unique note creator and
//        Daily notes tabs: the rows it lists (which files, in what order, how
//        many), its keys, an empty match, and what a pick writes
//
// Phases: fixtures | run.  ONLY=<regex> runs a subset of the steps.
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');

const vp = (rel) => path.join(VAULT, rel);
const disk = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
const write = (rel, s) => { fs.mkdirSync(path.dirname(vp(rel)), { recursive: true }); fs.writeFileSync(vp(rel), s); };
const tree = () => {
  const o = {};
  const walk = (d) => {
    for (const e of fs.readdirSync(path.join(VAULT, d), { withFileTypes: true })) {
      const rel = d ? d + '/' + e.name : e.name;
      if (rel === '.obsidian' || rel === '.trash') continue;
      if (e.isDirectory()) walk(rel); else o[rel] = fs.readFileSync(path.join(VAULT, rel), 'utf8');
    }
  };
  walk('');
  return o;
};
const diffTrees = (a, b) => {
  const d = {};
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) if (a[k] !== b[k]) d[k] = { before: a[k] ?? null, after: b[k] ?? null };
  return d;
};

if (phase === 'fixtures') {
  write('.obsidian/app.json', '{}');
  write('.obsidian/core-plugins.json', JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    'command-palette': true, 'file-recovery': true, canvas: true, 'note-composer': true, 'zk-prefixer': true,
    'daily-notes': true, bookmarks: true, 'page-preview': false, outline: true,
  }, null, 2));
  write('Links.md', 'Links\n\n[[Target]]\n\n[[Other]]\n');
  write('Target.md', '# Target\n\ntarget body\n');
  write('Other.md', 'other body\n');
  write('Alpha.md', 'alpha\n');
  write('Beta.md', 'beta\n');
  write('Board.canvas', '{"nodes":[],"edges":[]}');
  // s03
  for (const k of ['a', 'b', 'c', 'd', 'e', 'f']) write(`sp-${k}.md`, 'before\nPICK ME\nafter\n');
  write('Sub/sp-sub.md', 'before\nPICK ME\nafter\n');
  write('Existing target.md', 'existing target\n');
  write('hd-a.md', 'intro\n## Uniform\nsec body\n## Tango\nafter\n');
  write('hd-b.md', 'intro\n## Existing target\nsec body\n');
  write('hd-c.md', 'intro\n## Fresh heading\nsec body\n');
  // s05
  write('Templates/Composer.md', '{{content}}\n');
  write('Templates/Unique.md', '# {{title}}\n');
  write('Templates/Deep/Nested.md', 'nested\n');
  write('Templates/Sheet.canvas', '{"nodes":[],"edges":[]}');
  write('Templates/Pic.png', 'not really a png');
  write('Templates/Data.base', 'views: []\n');
  for (let i = 0; i < 40; i++) write(`Many/m${String(i).padStart(2, '0')}.md`, `m${i}\n`);
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

fs.mkdirSync(SHOTS, { recursive: true });
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
out.version = await c.evaluate(`return { ua: navigator.userAgent };`);

await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  notices: [],
  watch(doc) {
    const obs = new MutationObserver((ms) => { for (const m of ms) for (const n of m.addedNodes) {
      if (n.nodeType === 1 && n.classList && n.classList.contains('notice')) this.notices.push(n.textContent);
    } });
    obs.observe(doc.body, { childList: true, subtree: true });
  },
  takeNotices() { const n = this.notices; this.notices = []; return n; },
  vis(e) { if (!e) return false; const s = e.ownerDocument.defaultView.getComputedStyle(e); const r = e.getBoundingClientRect(); return s.display !== 'none' && s.visibility !== 'hidden' && r.height > 0; },
  headers(doc) {
    doc = doc || document;
    return Array.from(doc.querySelectorAll('.workspace-leaf')).map((l) => {
      const h = l.querySelector(':scope > .workspace-leaf-content > .view-header');
      const split = l.closest('.mod-root') ? 'root' : l.closest('.mod-left-split') ? 'left' : l.closest('.mod-right-split') ? 'right' : 'other';
      const leaf = l; const content = l.querySelector(':scope > .workspace-leaf-content');
      return { split, type: content ? content.getAttribute('data-type') : null, header: !!h, shown: this.vis(h), display: h ? doc.defaultView.getComputedStyle(h).display : null,
        height: h ? Math.round(h.getBoundingClientRect().height) : null, leafShown: this.vis(leaf),
        title: h ? ((h.querySelector('.view-header-title') || {}).textContent || null) : null,
        inlineTitleTop: (() => { const t = l.querySelector('.inline-title'); return t && this.vis(t) ? Math.round(t.getBoundingClientRect().top - l.getBoundingClientRect().top) : null; })() };
    });
  },
  tabs() {
    const a = app.workspace.activeLeaf;
    const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l));
    const order = [];
    for (const g of new Set(ls.map((l) => l.parent))) for (const ch of g.children) order.push({ file: ch.view && ch.view.file ? ch.view.file.path : ch.view.getViewType(), active: ch === a, groupActive: g.currentTab !== undefined ? g.children[g.currentTab] === ch : null, deferred: !!ch.isDeferred });
    return { order, active: a && a.view && a.view.file ? a.view.file.path : (a ? a.view.getViewType() : null), focus: this.describe(document.activeElement) };
  },
  describe(e) { return e ? { tag: e.tagName, cls: String(e.className).slice(0, 120) } : null; },
  async reset(files, activeIx) {
    const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l));
    for (const l of ls.slice(1)) l.detach();
    await this.sleep(300);
    let first = null;
    const leaves = [];
    for (let i = 0; i < files.length; i++) {
      const leaf = i === 0 ? (app.workspace.getLeavesOfType('markdown')[0] || app.workspace.getLeaf(false)) : app.workspace.getLeaf('tab');
      await leaf.openFile(app.vault.getAbstractFileByPath(files[i]));
      leaves.push(leaf);
      await this.sleep(250);
    }
    const leaf = leaves[activeIx];
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(400);
    return this.tabs();
  },
  async mode(m) {
    const leaf = app.workspace.activeLeaf;
    const st = leaf.getViewState();
    st.state = Object.assign({}, st.state, m === 'preview' ? { mode: 'preview' } : { mode: 'source', source: false });
    await leaf.setViewState(st); await this.sleep(500);
    return leaf.view.getMode();
  },
  modals() {
    return Array.from(document.querySelectorAll('.modal-container')).map((m) => {
      const root = m.querySelector('.modal') || m;
      return { cls: String(root.className), title: (root.querySelector('.modal-title') || {}).textContent || null,
        text: (root.textContent || '').trim().slice(0, 400),
        input: (() => { const i = root.querySelector('input'); return i ? { value: i.value, placeholder: i.placeholder } : null; })(),
        items: Array.from(root.querySelectorAll('.suggestion-item')).map((x) => ({ text: x.textContent.trim().slice(0, 120), sel: x.classList.contains('is-selected') })),
        buttons: Array.from(root.querySelectorAll('button')).map((b) => ({ text: b.textContent.trim(), cls: String(b.className) })) };
    });
  },
  closeModals() { let n = 0; for (const m of Array.from(document.querySelectorAll('.modal-container'))) { const b = m.querySelector('.modal-close-button'); if (b) b.click(); else m.remove(); n++; } return n; },
  ed() { const v = app.workspace.activeLeaf.view; return v && v.editor; },
};
window.__p.watch(document);
return true;`);
const P = (expr) => c.evaluate(`return await (async () => { const p = window.__p; ${expr} })();`);

async function step(name, fn, ms = 150000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now();
  const partial = {};
  out.steps[name] = { ok: null, partial };
  let timer;
  try {
    await Promise.race([fn(partial), new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); })]);
    out.steps[name] = { ok: true, ms: Date.now() - t, ...partial };
  } catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...partial }; }
  finally { clearTimeout(timer); }
  try { await P(`return p.closeModals();`); } catch { /* ignore */ }
  try { await c.evaluate(`document.querySelectorAll('.menu').forEach((m) => m.remove()); return true;`); } catch { /* ignore */ }
  save();
  console.error(`[settings-gaps] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + String(out.steps[name].error).split('\n')[0]}`);
  return out.steps[name];
}

async function shot(name, client = c) {
  try {
    const r = await client.send('Page.captureScreenshot', { format: 'png' });
    const f = path.join(SHOTS, name + '.png');
    fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
    return path.basename(f);
  } catch (e) { return 'shot failed: ' + e.message; }
}

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
  if (!url) return null;
  const s = await connect(url);
  await s.evaluate(`window.__q = {
    row(name) { return Array.from(document.querySelectorAll('.vertical-tab-content .setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === name) || null; },
    rect(e) { if (!e) return null; e.scrollIntoView({ block: 'center' }); const r = e.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2, w: r.width, h: r.height }; },
  }; return true;`);
  return s;
}
const closeSettings = async () => { await c.evaluate(`app.setting.close(); return true;`); await sleep(600); };

// ---------------------------------------------------------------------------
await step('s01-view-header', async (r) => {
  await P(`return await p.reset(['Links.md', 'Target.md'], 0);`);
  // An empty tab and a canvas beside the notes, and Outline in the right dock.
  await c.evaluate(`const l = app.workspace.getLeaf('tab'); await l.setViewState({ type: 'empty', state: {} }); return true;`);
  await c.evaluate(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Board.canvas')); return true;`);
  await c.evaluate(`const l = app.workspace.getLeavesOfType('markdown')[0]; app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  await sleep(600);
  r.appearanceBefore = disk('.obsidian/appearance.json');
  r.bodyBefore = await c.evaluate(`return Array.from(document.body.classList).filter((x) => /header|title|tab/.test(x));`);
  r.configBefore = await c.evaluate(`return app.vault.getConfig('showViewHeader');`);
  r.headersOn = await P(`return p.headers();`);
  r.shotOn = await shot('s01-header-on');
  // 1.15 keeps the row on the Interface tab.
  const s = await settings('interface');
  r.settingsOpen = !!s;
  if (!s) return;
  r.row = await s.evaluate(`const row = __q.row('Show tab title bar'); if (!row) return null; return { desc: (row.querySelector('.setting-item-description') || {}).textContent, on: !!row.querySelector('.checkbox-container.is-enabled'), tab: (document.querySelector('.vertical-tab-nav-item.is-active') || {}).textContent };`);
  if (!r.row) {
    // The row moved tab in some builds: look in every tab.
    r.tabs = await s.evaluate(`return Array.from(document.querySelectorAll('.vertical-tab-nav-item')).map((x) => x.textContent);`);
  }
  const t = await s.evaluate(`return __q.rect(__q.row('Show tab title bar') && __q.row('Show tab title bar').querySelector('.checkbox-container'));`);
  r.toggle = t;
  if (t) { await s.clickAt(t.cx, t.cy); await sleep(1500); }
  r.appearanceOff = disk('.obsidian/appearance.json');
  r.configOff = await c.evaluate(`return app.vault.getConfig('showViewHeader');`);
  r.bodyOff = await c.evaluate(`return Array.from(document.body.classList).filter((x) => /header|title|tab/.test(x));`);
  r.headersOffSettingsUp = await P(`return p.headers();`);
  s.close();
  await closeSettings();
  r.headersOff = await P(`return p.headers();`);
  r.shotOff = await shot('s01-header-off');
  // The tab strip, the inline title, and what the header's actions leave behind.
  r.stripOff = await c.evaluate(`const s = document.querySelector('.mod-root .workspace-tab-header-container'); return s ? { shown: window.__p.vis(s), h: Math.round(s.getBoundingClientRect().height) } : null;`);
  // Back/forward still work with no header? (commands)
  r.backCmd = await c.evaluate(`return app.commands.listCommands().some((x) => x.id === 'app:go-back');`);
  // Reading view of the same note.
  r.readingMode = await P(`return await p.mode('preview');`);
  r.headersOffReading = await P(`return p.headers();`);
  await P(`return await p.mode('source');`);
  // Stacked tabs with the header off.
  await c.evaluate(`app.commands.executeCommandById('workspace:toggle-stacked-tabs'); return true;`); await sleep(800);
  r.stackedOff = await P(`return p.headers();`);
  r.stackedTabHeaders = await c.evaluate(`return Array.from(document.querySelectorAll('.mod-root .workspace-tabs.mod-stacked .workspace-tab-header')).map((h) => ({ shown: window.__p.vis(h), title: h.getAttribute('aria-label') }));`);
  r.shotStacked = await shot('s01-stacked-off');
  await c.evaluate(`app.commands.executeCommandById('workspace:toggle-stacked-tabs'); return true;`); await sleep(800);
  // A popout with the header off.
  await c.evaluate(`const l = app.workspace.getLeavesOfType('markdown')[0]; app.workspace.setActiveLeaf(l, { focus: true }); app.commands.executeCommandById('workspace:open-in-new-window'); return true;`);
  await sleep(2500);
  r.popout = await c.evaluate(`const ch = app.workspace.floatingSplit.children; if (!ch.length) return null; const d = ch[0].doc; return { body: Array.from(d.body.classList).filter((x) => /header|title|tab/.test(x)), headers: window.__p.headers(d) };`);
  await c.evaluate(`for (const w of app.workspace.floatingSplit.children.slice()) w.win.close(); return true;`); await sleep(1200);
  // Set it back through the config API and see whether the leaves follow live.
  await c.evaluate(`app.vault.setConfig('showViewHeader', true); return true;`); await sleep(800);
  r.headersOnViaApi = (await P(`return p.headers();`)).filter((h) => h.split === 'root').map((h) => h.shown);
  r.appearanceEnd = disk('.obsidian/appearance.json');
});

// ---------------------------------------------------------------------------
// Each route opens Target.md in a new tab from Links.md, the middle of three tabs.
const routes = {
  async readingCtrlClick() {
    await P(`return await p.mode('preview');`);
    const r = await c.evaluate(`const a = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .markdown-reading-view a.internal-link')).find((x) => x.getAttribute('data-href') === 'Target'); if (!a) return null; const b = a.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!r) return 'no link';
    await c.clickAt(r.cx, r.cy, { mods: { ctrl: true } });
    return 'clicked';
  },
  async readingMiddleClick() {
    await P(`return await p.mode('preview');`);
    const r = await c.evaluate(`const a = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .markdown-reading-view a.internal-link')).find((x) => x.getAttribute('data-href') === 'Target'); if (!a) return null; const b = a.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!r) return 'no link';
    await c.clickAt(r.cx, r.cy, { button: 'middle' });
    return 'clicked';
  },
  async livePreviewCtrlClick() {
    await P(`return await p.mode('source');`);
    // Put the caret away from the link so it renders as a link.
    await c.evaluate(`const e = window.__p.ed(); e.setCursor({ line: 0, ch: 0 }); return true;`); await sleep(400);
    const r = await c.evaluate(`const a = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .cm-content .cm-hmd-internal-link')).find((x) => /Target/.test(x.textContent)); if (!a) return null; const b = a.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!r) return 'no link';
    await c.clickAt(r.cx, r.cy, { mods: { ctrl: true } });
    return 'clicked';
  },
  async openLinkCommand() {
    await P(`return await p.mode('source');`);
    await c.evaluate(`const e = window.__p.ed(); e.focus(); e.setCursor({ line: 2, ch: 4 }); return true;`); await sleep(300);
    return c.evaluate(`return app.commands.executeCommandById('editor:open-link-in-new-leaf');`);
  },
  async explorerCtrlClick() {
    await c.evaluate(`app.commands.executeCommandById('file-explorer:open'); return true;`); await sleep(500);
    const r = await c.evaluate(`const t = Array.from(document.querySelectorAll('.nav-file-title')).find((x) => x.getAttribute('data-path') === 'Target.md'); if (!t) return null; t.scrollIntoView({ block: 'center' }); const b = t.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!r) return 'no row';
    await c.clickAt(r.cx, r.cy, { mods: { ctrl: true } });
    return 'clicked';
  },
  async explorerMenuNewTab() {
    await c.evaluate(`app.commands.executeCommandById('file-explorer:open'); return true;`); await sleep(500);
    const r = await c.evaluate(`const t = Array.from(document.querySelectorAll('.nav-file-title')).find((x) => x.getAttribute('data-path') === 'Target.md'); if (!t) return null; const b = t.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!r) return 'no row';
    await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600);
    const m = await c.evaluate(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((x) => /^Open in new tab$/.test(x.textContent.trim())); if (!it) return Array.from(document.querySelectorAll('.menu .menu-item')).map((x) => x.textContent.trim()); const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!m || Array.isArray(m)) return { menu: m };
    await c.clickAt(m.cx, m.cy);
    return 'picked';
  },
  async switcherCtrlEnter() {
    await c.evaluate(`app.commands.executeCommandById('switcher:open'); return true;`); await sleep(700);
    await c.typeText('Target', 30); await sleep(500);
    await c.pressKey('Enter', { ctrl: true });
    return 'pressed';
  },
  async newTabCommand() {
    return c.evaluate(`return app.commands.executeCommandById('workspace:new-tab');`);
  },
};

for (const value of [true, false]) {
  for (const [name, route] of Object.entries(routes)) {
    await step(`s02-focus-${value ? 'on' : 'off'}-${name}`, async (r) => {
      r.config = await c.evaluate(`app.vault.setConfig('focusNewTab', ${value}); return app.vault.getConfig('focusNewTab');`);
      r.before = await P(`return await p.reset(['Alpha.md', 'Links.md', 'Beta.md'], 1);`);
      r.route = await route();
      await sleep(1500);
      r.after = await P(`return p.tabs();`);
      r.modals = await P(`return p.modals();`);
      if (r.modals.length) await c.pressKey('Escape');
    });
  }
}
await step('s02-app-json', async (r) => { r.app = disk('.obsidian/app.json'); await c.evaluate(`app.vault.setConfig('focusNewTab', true); return true;`); });

// ---------------------------------------------------------------------------
async function extract(r, src, cmd, caret, name, mods) {
  const before = tree();
  await P(`return await p.reset([${JSON.stringify(src)}], 0);`);
  await P(`return await p.mode('source');`);
  r.caret = await c.evaluate(`const e = window.__p.ed(); e.focus(); const s = ${JSON.stringify(caret)}; if (s.sel) e.setSelection(s.sel[0], s.sel[1]); else e.setCursor(s.cursor); return e.getSelection();`);
  await P(`p.takeNotices(); return true;`);
  r.ran = await c.evaluate(`return app.commands.executeCommandById(${JSON.stringify(cmd)});`);
  await sleep(700);
  r.prompt = await P(`return p.modals();`);
  if (name !== null) {
    await c.evaluate(`const i = document.querySelector('.modal-container input'); if (i) { i.focus(); i.select(); } return !!i;`);
    await c.pressKey('Backspace');
    if (name) await c.typeText(name, 25);
    await sleep(500);
  }
  r.typed = await P(`return p.modals();`);
  r.defaultPrevented = await c.evaluate(`window.__dp = null; const f = (e) => { if (e.key === 'Enter') setTimeout(() => { window.__dp = e.defaultPrevented; }, 0); }; document.addEventListener('keydown', f, { once: true }); return true;`);
  await c.pressKey('Enter', mods);
  await sleep(1000);
  r.keyPrevented = await c.evaluate(`return window.__dp;`);
  r.afterKey = await P(`return p.modals();`);
  await sleep(2600);
  r.notices = await P(`return p.takeNotices();`);
  r.tabs = await P(`return p.tabs();`);
  r.editor = await c.evaluate(`const e = window.__p.ed(); return e ? { value: e.getValue(), cursor: e.getCursor() } : null;`);
  r.changed = diffTrees(before, tree());
}
const PICK = { sel: [{ line: 1, ch: 0 }, { line: 1, ch: 7 }] };
await step('s03-split-ctrl-new-name', (r) => extract(r, 'sp-a.md', 'note-composer:split-file', PICK, 'brand-new', { ctrl: true }));
await step('s03-split-ctrl-matching', (r) => extract(r, 'sp-b.md', 'note-composer:split-file', PICK, 'Existing', { ctrl: true }));
await step('s03-split-ctrl-exact', (r) => extract(r, 'sp-c.md', 'note-composer:split-file', PICK, 'Existing target', { ctrl: true }));
await step('s03-split-ctrl-empty', (r) => extract(r, 'sp-d.md', 'note-composer:split-file', PICK, '', { ctrl: true }));
await step('s03-split-ctrl-nomatch', (r) => extract(r, 'sp-e.md', 'note-composer:split-file', PICK, 'zzqqxx', { ctrl: true }));
await step('s03-split-ctrl-shift', (r) => extract(r, 'sp-f.md', 'note-composer:split-file', PICK, 'shifted-new', { ctrl: true, shift: true }));
await step('s03-split-ctrl-sub', (r) => extract(r, 'Sub/sp-sub.md', 'note-composer:split-file', PICK, 'from-sub', { ctrl: true }));
await step('s03-heading-ctrl-default', (r) => extract(r, 'hd-a.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 3 } }, null, { ctrl: true }));
await step('s03-heading-ctrl-existing', (r) => extract(r, 'hd-b.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 3 } }, null, { ctrl: true }));
await step('s03-heading-ctrl-fresh', (r) => extract(r, 'hd-c.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 3 } }, null, { ctrl: true }));

// ---------------------------------------------------------------------------
await step('s04-recovery-buttons', async (r) => {
  // Snapshots of three notes.
  for (const f of ['Alpha.md', 'Beta.md', 'Other.md']) { await P(`return await p.reset([${JSON.stringify(f)}], 0);`); await sleep(400); }
  await c.evaluate(`await app.vault.modify(app.vault.getAbstractFileByPath('Alpha.md'), 'alpha changed\\n'); return true;`); await sleep(800);
  const count = () => c.evaluate(`const name = app.appId + '-backup'; const db = await new Promise((ok, no) => { const q = indexedDB.open(name); q.onsuccess = () => ok(q.result); q.onerror = () => no(q.error); }); const st = db.transaction('backups', 'readonly').objectStore('backups'); const n = await new Promise((ok) => { const q = st.count(); q.onsuccess = () => ok(q.result); }); db.close(); return n;`);
  r.dbName = await c.evaluate(`return app.appId + '-backup';`);
  r.countBefore = await count();
  await P(`return await p.reset(['Beta.md'], 0);`);
  const s = await settings('file-recovery');
  if (!s) { r.noSettings = true; return; }
  await s.evaluate(`window.__n = []; new MutationObserver((ms) => { for (const m of ms) for (const n of m.addedNodes) if (n.nodeType === 1 && n.classList.contains('notice')) window.__n.push(n.textContent); }).observe(document.body, { childList: true, subtree: true }); return true;`);
  const btn = (text) => s.evaluate(`const b = Array.from(document.querySelectorAll('.vertical-tab-content button')).find((x) => x.textContent.trim() === ${JSON.stringify(text)}); return __q.rect(b);`);
  const v = await btn('View');
  await s.clickAt(v.cx, v.cy); await sleep(1500);
  r.view = await s.evaluate(`const m = document.querySelector('.modal.mod-sync-history'); if (!m) return null; const i = m.querySelector('input[type=search], input'); return { title: (m.querySelector('.modal-title') || {}).textContent, search: i ? { value: i.value, placeholder: i.placeholder } : null, rows: Array.from(m.querySelectorAll('.modal-sidebar-list-item')).map((x) => x.textContent.trim()), right: (m.querySelector('.modal-content') || m).textContent.trim().slice(0, 300), focus: document.activeElement ? String(document.activeElement.className) : null };`);
  r.viewShot = await shot('s04-view', s);
  // The search: which files does it offer for "Al"?
  const sr = await s.evaluate(`const m = document.querySelector('.modal.mod-sync-history'); const i = m && m.querySelector('input'); return __q.rect(i);`);
  if (sr) {
    await s.clickAt(sr.cx, sr.cy); await sleep(200);
    await s.typeText('Al', 40); await sleep(800);
    r.viewSearch = await s.evaluate(`return { suggestions: Array.from(document.querySelectorAll('.suggestion-container .suggestion-item')).map((x) => x.textContent.trim()) };`);
    await s.pressKey('Enter'); await sleep(1000);
    r.viewPicked = await s.evaluate(`const m = document.querySelector('.modal.mod-sync-history'); return m ? { title: (m.querySelector('.modal-title') || {}).textContent, rows: Array.from(m.querySelectorAll('.modal-sidebar-list-item')).map((x) => x.textContent.trim()), text: (m.querySelector('textarea') || {}).value || null } : null;`);
    await s.pressKey('Escape'); await sleep(400);
  }
  await s.pressKey('Escape'); await sleep(800);
  r.settingsStillUp = await s.evaluate(`return !!document.querySelector('.vertical-tab-content');`).catch(() => false);
  const cl = await btn('Clear');
  await s.clickAt(cl.cx, cl.cy); await sleep(1000);
  r.clearAsk = await s.evaluate(`return Array.from(document.querySelectorAll('.modal-container')).map((m) => ({ title: (m.querySelector('.modal-title') || {}).textContent, text: (m.querySelector('.modal-content') || m).textContent.trim(), buttons: Array.from(m.querySelectorAll('button')).map((b) => ({ text: b.textContent.trim(), cls: String(b.className), focused: document.activeElement === b })), close: !!m.querySelector('.modal-close-button') }));`);
  r.clearShot = await shot('s04-clear-ask', s);
  // Escape first.
  await s.pressKey('Escape'); await sleep(600);
  r.afterEscape = { modals: await s.evaluate(`return document.querySelectorAll('.modal-container').length;`), count: await count() };
  await s.clickAt(cl.cx, cl.cy); await sleep(900);
  await s.evaluate(`const b = Array.from(document.querySelectorAll('.modal-container button')).find((x) => x.textContent.trim() === 'Cancel'); b.click(); return true;`); await sleep(600);
  r.afterCancel = await count();
  await s.clickAt(cl.cx, cl.cy); await sleep(900);
  // Enter on the question: does the focused button take it?
  await s.pressKey('Enter'); await sleep(1200);
  r.afterEnter = { modals: await s.evaluate(`return document.querySelectorAll('.modal-container').length;`), count: await count() };
  if (r.afterEnter.count) {
    await s.evaluate(`const b = Array.from(document.querySelectorAll('.modal-container button')).find((x) => x.textContent.trim() === 'Clear'); if (b) b.click(); return !!b;`); await sleep(1200);
  }
  r.afterClear = await count();
  r.noticesSettings = await s.evaluate(`return window.__n;`);
  r.noticesMain = await P(`return p.takeNotices();`);
  s.close();
  await closeSettings();
  // The command's modal after a clear, and whether opening a note snapshots it again.
  await c.evaluate(`app.commands.executeCommandById('file-recovery:open'); return true;`); await sleep(1200);
  r.modalAfterClear = await c.evaluate(`const m = document.querySelector('.modal.mod-sync-history'); return m ? { rows: Array.from(m.querySelectorAll('.modal-sidebar-list-item')).map((x) => x.textContent.trim()), text: m.textContent.trim().slice(0, 300) } : null;`);
  await c.pressKey('Escape'); await sleep(400);
  await P(`return await p.reset(['Target.md'], 0);`); await sleep(800);
  r.countAfterReopen = await count();
});

// ---------------------------------------------------------------------------
async function picker(r, tab, file) {
  r.fileBefore = disk('.obsidian/' + file);
  const s = await settings(tab);
  if (!s) { r.noSettings = true; return; }
  const btnRect = () => s.evaluate(`const row = __q.row('Template file location'); if (!row) return null; const b = row.querySelector('.combobox-button') || row.querySelector('.setting-item-control > *'); return Object.assign(__q.rect(b) || {}, { cls: String(b.className), html: row.querySelector('.setting-item-control').innerHTML.slice(0, 600) });`);
  r.button = await btnRect();
  if (!r.button) return;
  await s.clickAt(r.button.cx, r.button.cy); await sleep(900);
  const list = () => s.evaluate(`const x = document.querySelector('.suggestion-container'); if (!x) return null; const i = x.querySelector('input'); const b = x.getBoundingClientRect(); return { cls: String(x.className), input: i ? { placeholder: i.placeholder, value: i.value, focused: document.activeElement === i } : null, rect: [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)], items: Array.from(x.querySelectorAll('.suggestion-item')).map((it) => ({ text: it.textContent.trim(), html: it.innerHTML.slice(0, 300), sel: it.classList.contains('is-selected') })), empty: (x.querySelector('.suggestion-empty') || {}).textContent || null, scroll: (() => { const sc = x.querySelector('.suggestion'); return sc ? [sc.scrollHeight, sc.clientHeight] : null; })() };`);
  r.opened = await list();
  r.openShot = await shot(`s05-${tab}-open`, s);
  await s.pressKey('ArrowDown'); await sleep(200);
  await s.pressKey('ArrowDown'); await sleep(200);
  r.afterDown = (await list() || {}).items?.findIndex((x) => x.sel);
  await s.pressKey('ArrowUp'); await sleep(200);
  r.afterUp = (await list() || {}).items?.findIndex((x) => x.sel);
  await s.typeText('zzqqxx', 30); await sleep(500);
  r.noMatch = await list();
  await s.pressKey('Escape'); await sleep(600);
  r.afterEscape = { list: await list(), file: disk('.obsidian/' + file), settingsUp: await s.evaluate(`return !!document.querySelector('.vertical-tab-content');`).catch(() => false) };
  const b2 = await btnRect();
  await s.clickAt(b2.cx, b2.cy); await sleep(800);
  await s.typeText('nested', 30); await sleep(500);
  r.typed = await list();
  await s.pressKey('Enter'); await sleep(1500);
  r.picked = { file: disk('.obsidian/' + file), button: (await btnRect()) };
  r.pickedShot = await shot(`s05-${tab}-picked`, s);
  // Pick by mouse: open, type, click the second row.
  const b3 = await btnRect();
  await s.clickAt(b3.cx, b3.cy); await sleep(800);
  await s.typeText('Templates', 30); await sleep(500);
  r.typedFolder = await list();
  const row = await s.evaluate(`const it = document.querySelectorAll('.suggestion-container .suggestion-item')[1]; return __q.rect(it);`);
  if (row) { await s.clickAt(row.cx, row.cy); await sleep(1500); }
  r.clicked = { file: disk('.obsidian/' + file), button: await btnRect() };
  // The clear button.
  const clr = await s.evaluate(`const row = __q.row('Template file location'); const x = row && row.querySelector('.combobox-clear-button, .search-input-clear-button, [aria-label=Clear]'); return x ? Object.assign(__q.rect(x), { cls: String(x.className), aria: x.getAttribute('aria-label') }) : null;`);
  r.clearButton = clr;
  if (clr) { await s.clickAt(clr.cx, clr.cy); await sleep(1500); }
  r.cleared = { file: disk('.obsidian/' + file), button: await btnRect() };
  // A path that names no file: how the button shows it.
  s.close();
  await closeSettings();
}
await step('s05-picker-note-composer', (r) => picker(r, 'note-composer', 'note-composer.json'));
await step('s05-picker-zk-prefixer', (r) => picker(r, 'zk-prefixer', 'zk-prefixer.json'));
await step('s05-picker-daily-notes', (r) => picker(r, 'daily-notes', 'daily-notes.json'));
// What Enter writes on the row a query matching nothing offers, and what a
// typed query that names a file without its extension shows.
await step('s06-picker-free-text', async (r) => {
  const s = await settings('note-composer');
  const btnRect = () => s.evaluate(`const row = __q.row('Template file location'); const b = row.querySelector('.combobox-button'); return Object.assign(__q.rect(b) || {}, { html: row.querySelector('.setting-item-control').innerHTML.slice(0, 700) });`);
  const list = () => s.evaluate(`return Array.from(document.querySelectorAll('.suggestion-container .suggestion-item')).map((it) => ({ text: it.textContent.trim(), sel: it.classList.contains('is-selected') }));`);
  for (const q of ['zzqqxx', 'Folder/Nope', 'Templates/Composer', 'templates/composer.md']) {
    const b = await btnRect();
    await s.clickAt(b.cx, b.cy); await sleep(800);
    await s.typeText(q, 30); await sleep(500);
    const rows = await list();
    await s.pressKey('Enter'); await sleep(1500);
    r[q] = { rows, file: disk('.obsidian/note-composer.json'), button: (await btnRect()).html, open: await s.evaluate(`return !!document.querySelector('.suggestion-container');`) };
    if (r[q].open) { await s.pressKey('Escape'); await sleep(400); }
  }
  // A mouse click outside the open list.
  const b = await btnRect();
  await s.clickAt(b.cx, b.cy); await sleep(800);
  await s.clickAt(200, 600); await sleep(600);
  r.clickOutside = { open: await s.evaluate(`return !!document.querySelector('.suggestion-container');`), file: disk('.obsidian/note-composer.json') };
  s.close();
  await closeSettings();
});

await step('s05-missing-template', async (r) => {
  const cur = JSON.parse(disk('.obsidian/note-composer.json') || '{}');
  cur.template = 'Nowhere/Gone.md';
  write('.obsidian/note-composer.json', JSON.stringify(cur, null, 2));
  await c.evaluate(`const p = app.internalPlugins.plugins['note-composer'].instance; p.options.template = 'Nowhere/Gone.md'; return true;`);
  const s = await settings('note-composer');
  r.button = await s.evaluate(`const row = __q.row('Template file location'); return row ? row.querySelector('.setting-item-control').innerHTML.slice(0, 600) : null;`);
  s.close();
  await closeSettings();
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
