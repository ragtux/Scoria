// Two gaps at once (2026-11-16):
//
// The font size's persistence on 1.15: what `app:increase-font-size`,
// `app:decrease-font-size`, Ctrl+wheel (Quick font size adjustment) and the
// Appearance tab's Font size slider and its Restore default each write, to
// `appearance.json` and to local storage, and which wins on a relaunch.
//   f01  the boot: `appearance.json` holds `baseFontSize: 20` and no local
//        storage key exists yet -- the size drawn, the key afterwards
//   f02  Increase once: the size, the key, `appearance.json`
//   f03  Ctrl+wheel one notch up (a WheelEvent on the note)
//   f04  the Font size slider moved to 14, then Restore default
//   f05  `appearance.json` rewritten from outside with `baseFontSize: 24`
//   f06  Increase twice, then quit; after-restart reads the size back
//
// The backlinks block at the foot of a note (`backlink:toggle-backlinks-in-document`):
// its own "Change sort order" menu and "Show search filter", what each
// draws and what it persists.
//   b01  the block's nav buttons and the sort menu under a real click: rows,
//        icons, checks, separators, where the menu opens against the button
//   b02  each of the six orders picked from that menu: the groups' order,
//        the leaf's `backlinkOpts.sortOrder`, workspace.json, backlink.json,
//        and the dock pane's own sortOrder
//   b03  Show search filter: the field, its focus, the button's state; a
//        query typed with real keys and what it filters; the leaf's state
//   b04  Escape in the field, Clear search, and the filter hidden again with
//        a query in it
//   b05  sortOrder, showSearch and searchQuery left set, then quit:
//        after-restart reads the block back
//
// Phases: fixtures | run | after-restart   (node font-size-backlinks-block.mjs <phase>)
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
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
/** The markdown leaves of a workspace.json text, their inner state only. */
function wsLeaves(text) {
  let ws; try { ws = JSON.parse(text); } catch { return null; }
  const o = [];
  const walk = (n) => { if (!n) return; if (n.type === 'leaf') { if (n.state && n.state.type) o.push({ type: n.state.type, state: n.state.state }); } (n.children || []).forEach(walk); };
  walk(ws && ws.main); walk(ws && ws.right);
  return o;
}

// ---------------------------------------------------------------------------
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  const core = {
    'file-explorer': true, 'global-search': true, switcher: true, graph: false, backlink: true,
    'outgoing-link': true, 'tag-pane': true, properties: true, 'page-preview': true,
    'command-palette': true, 'editor-status': true, outline: true,
  };
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify(core, null, 2));
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/appearance.json'), '{\n  "baseFontSize": 20,\n  "baseFontSizeAction": true\n}');
  fs.writeFileSync(vp('Target.md'), '# Target\n\nThe target.\n');
  // Three linkers whose name order, modified order and created order differ.
  fs.writeFileSync(vp('Zulu.md'), 'Zulu points at [[Target]] once.\n');
  await sleep(1100);
  fs.writeFileSync(vp('Alpha.md'), 'Alpha says [[Target]] here.\n\nAnd again [[Target|alias]] there.\n');
  await sleep(1100);
  fs.writeFileSync(vp('Mike.md'), 'Mike links [[Target]] with a kiwi.\n');
  fs.writeFileSync(vp('Mention.md'), 'Target is named here without a link.\n');
  // Modified: Alpha newest, then Zulu, then Mike oldest.
  const now = Date.now() / 1000;
  fs.utimesSync(vp('Mike.md'), now - 3000, now - 3000);
  fs.utimesSync(vp('Zulu.md'), now - 2000, now - 2000);
  fs.utimesSync(vp('Alpha.md'), now - 1000, now - 1000);
  out.files = fs.readdirSync(VAULT);
  out.birth = Object.fromEntries(['Zulu.md', 'Alpha.md', 'Mike.md'].map((f) => [f, fs.statSync(vp(f)).birthtimeMs]));
  save();
  process.exit(0);
}

const c = await connect();
out.vaultPath = await c.vaultPath();
out.title = await c.evaluate(`return document.title;`);
out.appId = await c.evaluate(`return app.appId;`);
save();

await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  font() {
    const ls = {}; for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); if (/font/i.test(k)) ls[k] = localStorage.getItem(k); }
    return { cssVar: getComputedStyle(document.body).getPropertyValue('--font-text-size').trim(), inline: document.body.style.getPropertyValue('--font-text-size'),
      rootFont: document.documentElement.style.getPropertyValue('font-size'), config: app.vault.getConfig('baseFontSize'), ls };
  },
  mdLeaf() {
    let leaf = null; app.workspace.iterateRootLeaves((l) => { if (!leaf && l.view.getViewType() === 'markdown') leaf = l; });
    if (!leaf) app.workspace.iterateRootLeaves((l) => { if (!leaf) leaf = l; });
    return leaf || app.workspace.getLeaf(false);
  },
  async open(file) {
    let f = null;
    for (let i = 0; i < 40 && !f; i++) { f = app.vault.getAbstractFileByPath(file); if (!f) await this.sleep(150); }
    const leaf = this.mdLeaf();
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(600);
    return leaf.getViewState().state;
  },
  block() {
    const leaf = this.mdLeaf();
    const el = leaf.view.containerEl.querySelector('.embedded-backlinks');
    if (!el) return null;
    const btns = Array.from(el.querySelectorAll('.nav-header .nav-action-button')).map((b) => ({ aria: b.getAttribute('aria-label'), cls: b.className, icon: (b.querySelector('svg') || {}).getAttribute ? b.querySelector('svg').getAttribute('class') : null, rect: (() => { const r = b.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })() }));
    const sic = el.querySelector('.search-input-container');
    const input = sic && sic.querySelector('input');
    const pane = el.querySelector('.backlink-pane');
    const sections = pane ? Array.from(pane.querySelectorAll(':scope > .tree-item-self')).map((s) => ({ text: (s.querySelector('.tree-item-inner') || s).textContent.trim(), flair: ((s.querySelector('.tree-item-flair') || {}).textContent || '').trim(), collapsed: s.classList.contains('is-collapsed') })) : [];
    const containers = pane ? Array.from(pane.querySelectorAll(':scope > .search-result-container')) : [];
    const groups = containers.map((cn) => ({ shown: !!cn.offsetParent, files: Array.from(cn.querySelectorAll('.tree-item.search-result')).map((g) => ({ name: ((g.querySelector('.search-result-file-title .tree-item-inner') || {}).textContent || '').trim(), flair: ((g.querySelector('.search-result-file-title .tree-item-flair') || {}).textContent || '').trim(), matches: Array.from(g.querySelectorAll('.search-result-file-match')).map((m) => m.textContent.trim().slice(0, 80)) })) }));
    const r = el.getBoundingClientRect();
    let inputRect = null; if (input) { const ir = input.getBoundingClientRect(); inputRect = [ir.x, ir.y, ir.width, ir.height]; }
    let sicStyle = null; if (sic) { const cs = getComputedStyle(sic); sicStyle = { display: cs.display, margin: cs.margin, width: cs.width, height: cs.height }; }
    return { rect: [r.x, r.y, r.width, r.height], display: getComputedStyle(el).display, btns,
      search: sic ? { shown: !!sic.offsetParent, html: sic.outerHTML.slice(0, 600), value: input && input.value, placeholder: input && input.placeholder, inputRect, style: sicStyle,
        clear: (() => { const b = sic.querySelector('.search-input-clear-button'); return b ? { aria: b.getAttribute('aria-label'), shown: !!b.offsetParent, display: getComputedStyle(b).display } : null; })() } : null,
      sections, groups, empty: ((el.querySelector('.search-empty-state') || {}).textContent || '').trim() || null };
  },
  leafState() { return this.mdLeaf().getViewState().state; },
  dockState() { const l = app.workspace.getLeavesOfType('backlink')[0]; return l ? l.getViewState().state : null; },
  menus() {
    return Array.from(document.querySelectorAll('.menu')).map((m) => { const r = m.getBoundingClientRect(); return { rect: [r.x, r.y, r.width, r.height], items: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((it) => it.classList.contains('menu-separator') ? '---' : {
      title: ((it.querySelector('.menu-item-title') || it).textContent || '').trim(),
      icon: (() => { const s = it.querySelector('.menu-item-icon svg'); return s ? s.getAttribute('class') : null; })(),
      checked: it.classList.contains('mod-checked'), cls: it.className, section: it.getAttribute('data-section'),
    }) }; });
  },
  active() { const a = document.activeElement; if (!a) return null; return { tag: a.tagName, cls: String(a.className), placeholder: a.placeholder, inBlock: !!a.closest('.embedded-backlinks'), inDock: !!a.closest('.mod-right-split'), cm: !!a.closest('.cm-content') }; },
};
return true;`);

const ev = (body) => c.evaluate(body);
const P = (body) => c.evaluate(`return await (async () => { const p = window.__p; ${body} })();`);
const font = () => P(`return p.font();`);
const run = (id) => ev(`return app.commands.executeCommandById(${JSON.stringify(id)});`);
const btnSel = (aria) => `.workspace-leaf.mod-active .embedded-backlinks .nav-action-button[aria-label="${aria}"]`;

if (phase === 'after-restart') {
  await sleep(1500);
  await step('after-font', async () => ({ font: await font(), appearance: bytes('.obsidian/appearance.json') }));
  await step('after-block', async () => {
    const r = {};
    r.leafState = await P(`return p.leafState();`);
    r.block = await P(`return p.block();`);
    r.workspace = wsLeaves(bytes('.obsidian/workspace.json'));
    r.backlinkJson = bytes('.obsidian/backlink.json');
    return r;
  });
  save();
  console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
  c.close();
  process.exit(0);
}

// ---------------------------------------------------------------------------
await step('f01-boot', async () => ({ font: await font(), appearance: bytes('.obsidian/appearance.json') }));

await step('f02-increase', async () => {
  const r = { before: await font() };
  r.ran = await run('app:increase-font-size');
  await sleep(2500);
  r.after = await font();
  r.appearance = bytes('.obsidian/appearance.json');
  return r;
});

await step('f03-wheel', async () => {
  await P(`await p.open('Target.md'); return true;`);
  await sleep(700);
  const r = { before: await font() };
  r.prevented = await ev(`const el = document.querySelector('.workspace-leaf.mod-active .view-content');
    const b = el.getBoundingClientRect();
    const target = document.elementFromPoint(b.x + b.width / 2, b.y + 40) || el;
    const e = new WheelEvent('wheel', { deltaY: -120, deltaMode: 0, ctrlKey: true, clientX: b.x + b.width / 2, clientY: b.y + 40, bubbles: true, cancelable: true });
    target.dispatchEvent(e); return e.defaultPrevented;`);
  await sleep(2500);
  r.after = await font();
  r.appearance = bytes('.obsidian/appearance.json');
  return r;
});

await step('f04-slider', async () => {
  const r = {};
  await ev(`app.setting.open(); app.setting.openTabById('appearance'); return true;`);
  await sleep(2000);
  const findSlider = `const pane = app.setting.activeTab && app.setting.activeTab.containerEl;
    const it = pane && Array.from(pane.querySelectorAll('.setting-item')).find((x) => ((x.querySelector('.setting-item-name') || {}).textContent || '').trim() === 'Font size');`;
  r.row = await ev(`${findSlider} if (!it) return null; const s = it.querySelector('input.slider');
    return { value: s && s.value, min: s && s.min, max: s && s.max, buttons: Array.from(it.querySelectorAll('.clickable-icon, button')).map((b) => b.getAttribute('aria-label')) };`);
  r.setTo14 = await ev(`${findSlider} const s = it.querySelector('input.slider'); const W = s.ownerDocument.defaultView;
    s.value = '14'; s.dispatchEvent(new W.Event('input', { bubbles: true })); s.dispatchEvent(new W.Event('change', { bubbles: true })); return s.value;`);
  await sleep(2500);
  r.after14 = await font();
  r.appearance14 = bytes('.obsidian/appearance.json');
  r.restore = await ev(`${findSlider} const b = Array.from(it.querySelectorAll('.clickable-icon, button')).find((x) => /restore/i.test(x.getAttribute('aria-label') || '')); if (!b) return false; b.click(); return true;`);
  await sleep(2500);
  r.afterRestore = await font();
  r.appearanceRestore = bytes('.obsidian/appearance.json');
  r.sliderAfter = await ev(`${findSlider} const s = it && it.querySelector('input.slider'); return s && s.value;`);
  await ev(`app.setting.close(); return true;`);
  await sleep(800);
  return r;
});

await step('f05-external', async () => {
  const r = { before: await font() };
  fs.writeFileSync(vp('.obsidian/appearance.json'), '{\n  "baseFontSize": 24,\n  "baseFontSizeAction": true\n}');
  await sleep(3500);
  r.after = await font();
  r.appearance = bytes('.obsidian/appearance.json');
  return r;
});

// ---------------------------------------------------------------------------
await step('b01-menu', async () => {
  const r = {};
  r.open = await P(`return await p.open('Target.md');`);
  r.toggled = await run('backlink:toggle-backlinks-in-document');
  await sleep(1500);
  r.state = await P(`return p.leafState();`);
  r.block = await P(`return p.block();`);
  r.dock = await P(`return p.dockState();`);
  await ev(`const el = document.querySelector('.workspace-leaf.mod-active .embedded-backlinks'); el && el.scrollIntoView({ block: 'center' }); return true;`);
  await sleep(400);
  r.btn = await c.rect(btnSel('Change sort order'));
  r.click = await c.clickElement(btnSel('Change sort order'));
  await sleep(600);
  r.menus = await P(`return p.menus();`);
  r.activeWithMenu = await P(`return p.active();`);
  await c.pressKey('Escape'); await sleep(400);
  r.menusAfterEscape = await P(`return p.menus();`);
  return r;
});

const ORDERS = ['File name (A to Z)', 'File name (Z to A)', 'Modified time (new to old)', 'Modified time (old to new)', 'Created time (new to old)', 'Created time (old to new)'];
await step('b02-orders', async () => {
  const r = { picks: [] };
  for (const title of [...ORDERS.slice(1), ORDERS[0], ORDERS[2]]) {
    const prevWs = bytes('.obsidian/workspace.json');
    await c.clickElement(btnSel('Change sort order'));
    await sleep(500);
    const at = await ev(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim() === ${JSON.stringify(title)});
      if (!it) return null; const b = it.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (at) await c.clickAt(at.x, at.y);
    await sleep(2500);
    const ws = bytes('.obsidian/workspace.json');
    const blk = await P(`return p.block();`);
    r.picks.push({ title, clicked: !!at, order: blk && blk.groups[0] ? blk.groups[0].files.map((f) => f.name) : null,
      leaf: await P(`return p.leafState();`), dock: await P(`return p.dockState();`),
      wsChanged: ws !== prevWs, wsLeaves: wsLeaves(ws), backlinkJson: bytes('.obsidian/backlink.json'),
      menusAfter: await P(`return p.menus().length;`) });
  }
  return r;
});

await step('b03-filter', async () => {
  const r = {};
  r.beforeBlock = await P(`return p.block();`);
  r.click = await c.clickElement(btnSel('Show search filter'));
  await sleep(700);
  r.blockShown = await P(`return p.block();`);
  r.active = await P(`return p.active();`);
  r.leafShown = await P(`return p.leafState();`);
  await sleep(2000);
  r.wsShown = wsLeaves(bytes('.obsidian/workspace.json'));
  await c.typeText('kiwi', 60);
  await sleep(1200);
  r.blockKiwi = await P(`return p.block();`);
  r.leafKiwi = await P(`return p.leafState();`);
  await sleep(2000);
  r.wsKiwi = wsLeaves(bytes('.obsidian/workspace.json'));
  // A file-name query, then a query matching nothing.
  for (let i = 0; i < 4; i++) { await c.pressKey('Backspace'); await sleep(40); }
  await c.typeText('Zulu', 60); await sleep(1200);
  r.blockZulu = await P(`return p.block();`);
  await c.pressKey('a', { ctrl: true }); await c.typeText('nomatchhere', 30); await sleep(1200);
  r.blockNone = await P(`return p.block();`);
  r.dock = await P(`return p.dockState();`);
  return r;
});

await step('b04-escape-clear-hide', async () => {
  const r = {};
  await c.pressKey('a', { ctrl: true }); await c.typeText('Alpha', 40); await sleep(1000);
  r.beforeEscape = { block: await P(`return p.block();`), active: await P(`return p.active();`) };
  await c.pressKey('Escape'); await sleep(800);
  r.afterEscape = { block: await P(`return p.block();`), active: await P(`return p.active();`), leaf: await P(`return p.leafState();`) };
  // Clear search, by its button.
  await ev(`const i = document.querySelector('.workspace-leaf.mod-active .embedded-backlinks .search-input-container input'); i && i.focus(); return true;`);
  await sleep(200);
  r.clearRect = await c.rect('.workspace-leaf.mod-active .embedded-backlinks .search-input-clear-button');
  if (r.clearRect) await c.clickElement('.workspace-leaf.mod-active .embedded-backlinks .search-input-clear-button');
  await sleep(800);
  r.afterClear = { block: await P(`return p.block();`), active: await P(`return p.active();`), leaf: await P(`return p.leafState();`) };
  // A query in the field, then the filter hidden by its button.
  await ev(`const i = document.querySelector('.workspace-leaf.mod-active .embedded-backlinks .search-input-container input'); i && i.focus(); return true;`);
  await c.typeText('Mike', 50); await sleep(1000);
  r.withMike = await P(`return p.block();`);
  await c.clickElement(btnSel('Show search filter'));
  await sleep(900);
  r.hidden = { block: await P(`return p.block();`), leaf: await P(`return p.leafState();`), active: await P(`return p.active();`) };
  await c.clickElement(btnSel('Show search filter'));
  await sleep(900);
  r.reshown = { block: await P(`return p.block();`), leaf: await P(`return p.leafState();`), active: await P(`return p.active();`) };
  return r;
});

await step('b05-leave-and-quit', async () => {
  const r = {};
  // Leave: Modified time (new to old) from b02, the filter shown with "Zulu".
  await ev(`const i = document.querySelector('.workspace-leaf.mod-active .embedded-backlinks .search-input-container input'); i && i.focus(); return true;`);
  await c.pressKey('a', { ctrl: true }); await c.typeText('Zulu', 50); await sleep(1500);
  r.leaf = await P(`return p.leafState();`);
  r.block = await P(`return p.block();`);
  await sleep(2500);
  r.ws = wsLeaves(bytes('.obsidian/workspace.json'));
  return r;
});

await step('f06-increase-and-quit', async () => {
  const r = { before: await font() };
  await run('app:increase-font-size'); await sleep(200); await run('app:increase-font-size');
  await sleep(1500);
  r.after = await font();
  r.appearance = bytes('.obsidian/appearance.json');
  return r;
});

save();
out.quitting = true; save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
try { await c.quit(); out.quit = true; } catch (e) { out.quit = String(e); }
save();
c.close();
process.exit(0);
