// The tab header's right-click menu, under a real right click
// (Input.dispatchMouseEvent: a dispatched `contextmenu` event only gets a
// bare "Close"), and what its rows do.
//
//   s00  context: version, commands
//   s01  a markdown tab alone in its group; every submenu row hovered
//   s02  the same tab with three tabs in the group
//   s03  Close others on the middle tab while the last is active
//   s04  Close all: what is left
//   s05  Pin: pinned state, the header's pin element, the menu again,
//        Ctrl+W and workspace:close on the pinned tab, then Unpin
//   s06  Link with tab...: the UI it raises, then Escape
//   s07  an empty "New tab" tab
//   s08  a canvas tab and a base tab
//   s09  a sidebar tab (Backlinks, Outline in the right dock)
//   s10  Move to new window from the menu, then the popout tab's menu
//        (a real right click on the popout's own CDP target)
//
// Each menu row records title, data-section, icon class, disabled, warning,
// checked, submenu, separators (and whether each is drawn). `ONLY_STEPS=s05`
// re-runs a step.
//
// Phases: fixtures | run   (node tab-header-menu.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.tabHeaderMenu = all.tabHeaderMenu || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && process.env.ONLY_STEPS && all.tabHeaderMenu.run) {
  const prev = all.tabHeaderMenu.run;
  Object.assign(out, { steps: prev.steps || {}, order: prev.order || [], version: prev.version });
}
all.tabHeaderMenu[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const ONLY = process.env.ONLY_STEPS && phase === 'run' ? process.env.ONLY_STEPS.split(',') : null;
const STEP_MS = Number(process.env.STEP_MS || 90000);
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), STEP_MS); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[tabmenu] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('Folder'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), '# Alpha\n\nA link to [[Beta]] and one to [[Gamma]].\n\n## Section\n\nText.\n');
  fs.writeFileSync(vp('Beta.md'), '# Beta\n\nBack to [[Alpha]].\n');
  fs.writeFileSync(vp('Folder/Gamma.md'), '# Gamma\n\nSee [[Alpha]].\n');
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify({ nodes: [{ id: 'a1', type: 'text', text: 'Hello', x: 0, y: 0, width: 250, height: 60 }], edges: [] }, null, '\t'));
  fs.writeFileSync(vp('Table.base'), 'views:\n  - type: table\n    name: Table\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    outline: true, canvas: true, bases: true, bookmarks: true, 'command-palette': true, 'file-recovery': true,
    'page-preview': true, 'tag-pane': true, properties: true,
  }, null, 2));
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

const HELPERS = `
window.__t = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  menus(doc) {
    doc = doc || document;
    return Array.from(doc.querySelectorAll('.menu')).map((m) => {
      const r = m.getBoundingClientRect();
      return { cls: this.cls(m), rect: [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)],
        rows: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((e) => {
          if (e.classList.contains('menu-separator')) return { sep: true, drawn: e.offsetHeight > 0 && !!e.offsetParent };
          const t = e.querySelector('.menu-item-title');
          const svg = e.querySelector('.menu-item-icon svg');
          return {
            title: t ? t.textContent.trim() : (e.textContent || '').trim(),
            section: e.getAttribute('data-section'),
            icon: svg ? svg.getAttribute('class') : null,
            cls: this.cls(e),
            disabled: e.classList.contains('is-disabled'),
            warning: e.classList.contains('is-warning'),
            checked: e.classList.contains('mod-checked') || !!e.querySelector('.mod-checked'),
            submenu: e.classList.contains('has-submenu') || !!e.querySelector('.menu-item-icon.mod-submenu, .mod-submenu'),
            extra: Array.from(e.children).map((ch) => this.cls(ch)).filter((x) => !/menu-item-(icon|title)$/.test(x || '')),
            drawn: e.offsetHeight > 0 && !!e.offsetParent,
          };
        }) };
    });
  },
  row(doc, title, menuIndex) {
    doc = doc || document;
    const ms = Array.from(doc.querySelectorAll('.menu'));
    const m = ms[menuIndex === undefined ? ms.length - 1 : menuIndex]; if (!m) return null;
    const it = Array.from(m.querySelectorAll('.menu-item')).find((e) => ((e.querySelector('.menu-item-title') || e).textContent || '').trim() === title);
    if (!it) return null;
    const r = it.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2 };
  },
  leafInfo(l) {
    const v = l.view; const vs = l.getViewState();
    return { type: v.getViewType(), deferred: !!l.isDeferred, file: v.file ? v.file.path : (vs.state && vs.state.file) || null, title: l.getDisplayText(), pinned: !!vs.pinned, group: l.group || null,
      active: app.workspace.activeLeaf === l, popout: !!(l.getContainer && l.getContainer().win !== window) };
  },
  rootLeaves() {
    const o = []; app.workspace.iterateRootLeaves((l) => { o.push(this.leafInfo(l)); }); return o;
  },
  allLeaves() {
    const o = []; app.workspace.iterateAllLeaves((l) => { o.push(this.leafInfo(l)); }); return o;
  },
  headers(doc) {
    doc = doc || document;
    return Array.from(doc.querySelectorAll('.mod-root .workspace-tab-header, .workspace-tab-header')).filter((h, i, a) => a.indexOf(h) === i).map((h) => ({
      label: h.getAttribute('aria-label'), type: h.getAttribute('data-type'), cls: this.cls(h),
      inRoot: !!h.closest('.mod-root'), inSide: !!h.closest('.mod-sidedock'),
      status: Array.from(h.querySelectorAll('.workspace-tab-header-status-container > *')).map((e) => ({ cls: this.cls(e), svg: e.querySelector('svg') ? e.querySelector('svg').getAttribute('class') : null, aria: e.getAttribute('aria-label') })),
      closeBtn: !!h.querySelector('.workspace-tab-header-inner-close-button'),
      closeBtnShown: (() => { const b = h.querySelector('.workspace-tab-header-inner-close-button'); return b ? getComputedStyle(b).display !== 'none' : null; })(),
    }));
  },
  // Reset the root to one leaf holding the given files as tabs (one group).
  async setup(files, activeIndex) {
    app.workspace.leftSplit.collapse();
    const leaves = []; app.workspace.iterateRootLeaves((l) => { leaves.push(l); });
    for (const l of leaves) { const vs = l.getViewState(); if (vs.pinned) l.setPinned(false); }
    const first = leaves[0] || app.workspace.getLeaf(false);
    for (const l of leaves.slice(1)) l.detach();
    await this.sleep(200);
    const keep = []; app.workspace.iterateRootLeaves((l) => { keep.push(l); });
    let leaf = keep[0];
    for (const l of keep.slice(1)) l.detach();
    const made = [];
    for (let i = 0; i < files.length; i++) {
      const target = i === 0 ? leaf : app.workspace.createLeafInParent(leaf.parent, leaf.parent.children.length);
      if (files[i] === null) await target.setViewState({ type: 'empty', state: {} });
      else await target.openFile(app.vault.getAbstractFileByPath(files[i]));
      made.push(target);
    }
    const act = made[activeIndex === undefined ? made.length - 1 : activeIndex];
    app.workspace.setActiveLeaf(act, { focus: true });
    await this.sleep(500);
    return this.rootLeaves();
  },
  headerRectOf(index) {
    const hs = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header'));
    const h = hs[index]; if (!h) return null; h.scrollIntoView && h.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const r = h.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2, label: h.getAttribute('aria-label') };
  },
  ui(doc) {
    doc = doc || document;
    return {
      menus: doc.querySelectorAll('.menu').length,
      modals: Array.from(doc.querySelectorAll('.modal-container')).map((m) => ({ cls: this.cls(m), title: (m.querySelector('.modal-title') || {}).textContent || null, text: (m.textContent || '').trim().slice(0, 300) })),
      prompts: Array.from(doc.querySelectorAll('.prompt')).map((m) => ({ cls: this.cls(m), text: (m.textContent || '').trim().slice(0, 300) })),
      notices: Array.from(doc.querySelectorAll('.notice')).map((n) => n.textContent.trim()),
      bodyCls: doc.body.className,
      focus: doc.activeElement ? doc.activeElement.tagName.toLowerCase() + '.' + this.cls(doc.activeElement) : null,
    };
  },
};
return true;`;
await c.evaluate(HELPERS);

const ev = (s) => c.evaluate(s);
const T = 'const T = window.__t;';
const dismiss = async (cl = c) => {
  await cl.pressKey('Escape'); await sleep(250);
  await cl.evaluate(`for (const m of document.querySelectorAll('.menu')) m.remove(); return true;`);
  await sleep(150);
};

// Right-click a header (rect) on client `cl`, read every menu; hover every
// submenu row and read its submenu too.
async function menuAt(r, cl = c, { hoverSubmenus = true } = {}) {
  await cl.mouseMove(r.cx, r.cy); await sleep(150);
  await cl.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(700);
  const menus = await cl.evaluate(`return window.__t.menus();`);
  const res = { menus };
  if (hoverSubmenus && menus.length) {
    res.submenus = {};
    for (const row of menus[0].rows.filter((x) => x.submenu)) {
      const rr = await cl.evaluate(`return window.__t.row(document, ${JSON.stringify(row.title)}, 0);`);
      if (!rr) continue;
      await cl.mouseMove(rr.x + 8, rr.cy); await sleep(150);
      await cl.mouseMove(rr.cx, rr.cy); await sleep(800);
      const ms = await cl.evaluate(`return window.__t.menus();`);
      res.submenus[row.title] = ms.slice(1);
    }
  }
  return res;
}
async function clickRow(title, cl = c, menuIndex) {
  const rr = await cl.evaluate(`return window.__t.row(document, ${JSON.stringify(title)}${menuIndex === undefined ? '' : ', ' + menuIndex});`);
  if (!rr) return { error: 'row not found: ' + title };
  await cl.mouseMove(rr.cx, rr.cy); await sleep(150);
  await cl.clickAt(rr.cx, rr.cy); await sleep(900);
  return { clicked: title };
}
const rightClickHeader = async (i) => {
  const r = await ev(`${T} return T.headerRectOf(${i});`);
  if (!r) throw new Error('no header ' + i);
  await c.mouseMove(r.cx, r.cy); await sleep(150);
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(700);
  return r;
};

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, apiVersion: (typeof apiVersion !== 'undefined' ? apiVersion : (window.apiVersion || null)), ua: navigator.userAgent, win: [innerWidth, innerHeight] };`);
  const cmds = await ev(`const ids = ['workspace:close', 'workspace:close-others', 'workspace:close-tab-group', 'workspace:close-others-tab-group', 'workspace:toggle-pin', 'workspace:move-to-new-window', 'workspace:open-in-new-window', 'workspace:split-vertical', 'workspace:split-horizontal', 'workspace:copy-path', 'workspace:copy-full-path', 'workspace:copy-url', 'workspace:edit-file-title', 'file-explorer:reveal-active-file', 'open-with-default-app:open', 'open-with-default-app:show'];
    const o = {}; for (const id of ids) { const x = app.commands.commands[id]; o[id] = x ? { name: x.name, keys: (app.hotkeyManager.defaultKeys || {})[id] || null } : null; } return o;`);
  return { version: out.version, commands: cmds };
});

await step('s01-single-md', async () => {
  const leaves = await ev(`${T} return await T.setup(['Alpha.md'], 0);`);
  const r = await ev(`${T} return T.headerRectOf(0);`);
  const m = await menuAt(r);
  await dismiss();
  return { leaves, header: r, ...m };
});

await step('s02-three-md', async () => {
  const leaves = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md', 'Folder/Gamma.md'], 1);`);
  const r = await ev(`${T} return T.headerRectOf(1);`);
  const m = await menuAt(r, c, { hoverSubmenus: false });
  await dismiss();
  // Also a right click on an inactive tab (index 0).
  const r0 = await ev(`${T} return T.headerRectOf(0);`);
  const inactive = await menuAt(r0, c, { hoverSubmenus: false });
  const activeAfterRightClick = await ev(`${T} return T.rootLeaves().filter((l) => l.active).map((l) => l.file);`);
  await dismiss();
  return { leaves, ...m, inactiveTab: inactive, activeAfterRightClickOnInactive: activeAfterRightClick };
});

await step('s03-close-others', async () => {
  const before = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md', 'Folder/Gamma.md'], 2);`);
  await rightClickHeader(1);
  const clicked = await clickRow('Close others');
  const after = await ev(`${T} return T.rootLeaves();`);
  const ui = await ev(`${T} return T.ui();`);
  return { before, clicked, after, ui };
});

await step('s04-close-all', async () => {
  const before = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md', 'Folder/Gamma.md'], 1);`);
  await rightClickHeader(1);
  const clicked = await clickRow('Close all');
  await sleep(500);
  const after = await ev(`${T} return T.rootLeaves();`);
  const headers = await ev(`${T} return T.headers().filter((h) => h.inRoot);`);
  const ui = await ev(`${T} return T.ui();`);
  return { before, clicked, after, headers, ui };
});

await step('s05-pin', async () => {
  const before = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md'], 0);`);
  await rightClickHeader(0);
  const clicked = await clickRow('Pin');
  const afterPin = await ev(`${T} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); return { leaves: T.rootLeaves(), viewState: ls[0].getViewState(), headers: T.headers().filter((h) => h.inRoot),
    pinHtml: (() => { const h = document.querySelectorAll('.mod-root .workspace-tab-header')[0]; const s = h.querySelector('.workspace-tab-header-status-container'); return s ? s.outerHTML.replace(/<path[^>]*>/g, '').slice(0, 1500) : null; })() };`);
  const r = await ev(`${T} return T.headerRectOf(0);`);
  const menuPinned = await menuAt(r, c, { hoverSubmenus: false });
  await dismiss();
  // Hover the pin icon: tooltip / clickable?
  const pinIcon = await ev(`const h = document.querySelectorAll('.mod-root .workspace-tab-header')[0]; const p = h.querySelector('.workspace-tab-header-status-icon, .mod-pinned'); if (!p) return null; const r = p.getBoundingClientRect(); return { cls: p.className && p.className.baseVal !== undefined ? p.getAttribute('class') : p.className, aria: p.getAttribute('aria-label'), rect: [r.x, r.y, r.width, r.height], cx: r.x + r.width / 2, cy: r.y + r.height / 2, cursor: getComputedStyle(p).cursor };`);
  // Ctrl+W on the pinned active tab.
  await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); app.workspace.setActiveLeaf(ls[0], { focus: true }); return true;`); await sleep(300);
  await c.pressKey('w', { ctrl: true }); await sleep(900);
  const afterCtrlW = await ev(`${T} return { leaves: T.rootLeaves(), ui: T.ui() };`);
  await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); const pl = ls.find((l) => l.getViewState().pinned); if (pl) app.workspace.setActiveLeaf(pl, { focus: true }); return !!pl;`); await sleep(300);
  const ranClose = await ev(`return app.commands.executeCommandById('workspace:close');`); await sleep(900);
  const afterCloseCmd = await ev(`${T} return { leaves: T.rootLeaves(), ui: T.ui() };`);
  // Middle click / close button on the pinned tab (if still there).
  let afterCloseButton = null;
  const cb = await ev(`const hs = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')); const h = hs.find((x) => x.querySelector('.workspace-tab-header-status-container > *')); if (!h) return null; const b = h.querySelector('.workspace-tab-header-inner-close-button'); const r = h.getBoundingClientRect(); return { headerCx: r.x + r.width / 2, headerCy: r.y + r.height / 2, closeDisplay: b ? getComputedStyle(b).display : null };`);
  if (cb) {
    await c.clickAt(cb.headerCx, cb.headerCy, { button: 'middle' }); await sleep(900);
    afterCloseButton = { closeDisplay: cb.closeDisplay, afterMiddleClick: await ev(`${T} return T.rootLeaves();`) };
  }
  // Unpin through the menu again.
  let unpin = null;
  const idx = await ev(`const hs = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')); return hs.findIndex((x) => x.querySelector('.workspace-tab-header-status-container > *'));`);
  if (idx >= 0) {
    await rightClickHeader(idx);
    const titles = (await ev(`${T} return T.menus();`))[0].rows.map((x) => x.title || '---');
    const cl = await clickRow(titles.includes('Unpin') ? 'Unpin' : 'Pin');
    unpin = { titles, clicked: cl, after: await ev(`${T} return { leaves: T.rootLeaves(), headers: T.headers().filter((h) => h.inRoot) };`) };
  }
  await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); for (const l of ls) l.setPinned(false); return true;`);
  return { before, clicked, afterPin, menuPinned, pinIcon, afterCtrlW, ranClose, afterCloseCmd, afterCloseButton, unpin };
});

await step('s06-link-with-tab', async () => {
  const before = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md'], 0);`);
  await rightClickHeader(0);
  const clicked = await clickRow('Link with tab...');
  await sleep(400);
  const ui = await ev(`${T} return T.ui();`);
  const dom = await ev(`
    const bodyCls = document.body.className;
    const marked = Array.from(document.querySelectorAll('[class*=link], [class*=linking], [class*=overlay], .workspace-leaf, .workspace-tab-header')).filter((e) => /link|overlay|stack|select|mod-/.test(e.className && e.className.baseVal === undefined ? e.className : '')).slice(0, 60)
      .map((e) => ({ tag: e.tagName.toLowerCase(), cls: e.className, text: (e.textContent || '').trim().slice(0, 80) }));
    const novel = Array.from(document.body.children).map((e) => e.tagName.toLowerCase() + '.' + (e.className && e.className.baseVal === undefined ? e.className : ''));
    const overlays = Array.from(document.querySelectorAll('.workspace-leaf-overlay, .workspace-fake-target-overlay, .workspace-drop-overlay, .mod-linking, .is-linking, .tooltip, .notice')).map((e) => ({ cls: e.className, text: (e.textContent || '').trim().slice(0, 200), rect: (() => { const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })(), style: (() => { const s = getComputedStyle(e); return { bg: s.backgroundColor, outline: s.outline, border: s.border, opacity: s.opacity }; })() }));
    return { bodyCls, novel, overlays, leafCls: Array.from(document.querySelectorAll('.mod-root .workspace-leaf')).map((e) => e.className), tooltips: Array.from(document.querySelectorAll('.tooltip')).map((e) => e.textContent.trim()) };`);
  // Hover the other leaf's content.
  const other = await ev(`const ls = Array.from(document.querySelectorAll('.mod-root .workspace-leaf')); const l = ls[ls.length - 1]; const r = l.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2 };`);
  await c.mouseMove(other.cx, other.cy); await sleep(400);
  const hovered = await ev(`${T} return { ui: T.ui(), overlays: Array.from(document.querySelectorAll('.workspace-leaf-overlay, .workspace-fake-target-overlay, .mod-linking, .is-linking, .tooltip')).map((e) => ({ cls: e.className, text: (e.textContent || '').trim().slice(0, 200) })), leafCls: Array.from(document.querySelectorAll('.mod-root .workspace-leaf')).map((e) => e.className) };`);
  await c.pressKey('Escape'); await sleep(500);
  const afterEscape = await ev(`${T} return { ui: T.ui(), leaves: T.rootLeaves(), overlays: document.querySelectorAll('.workspace-leaf-overlay, .workspace-fake-target-overlay, .mod-linking, .is-linking').length };`);
  return { before, clicked, ui, dom, hovered, afterEscape };
});

await step('s07-empty-tab', async () => {
  const leaves = await ev(`${T} return await T.setup([null], 0);`);
  const r = await ev(`${T} return T.headerRectOf(0);`);
  const m = await menuAt(r);
  await dismiss();
  const leaves2 = await ev(`${T} return await T.setup(['Alpha.md', null], 1);`);
  const r2 = await ev(`${T} return T.headerRectOf(1);`);
  const m2 = await menuAt(r2, c, { hoverSubmenus: false });
  await dismiss();
  return { leaves, header: r, ...m, withOthers: { leaves: leaves2, ...m2 } };
});

await step('s08-canvas-base', async () => {
  const lc = await ev(`${T} return await T.setup(['Board.canvas'], 0);`);
  const canvas = await menuAt(await ev(`${T} return T.headerRectOf(0);`));
  await dismiss();
  const lb = await ev(`${T} return await T.setup(['Table.base'], 0);`);
  const base = await menuAt(await ev(`${T} return T.headerRectOf(0);`));
  await dismiss();
  return { canvas: { leaves: lc, ...canvas }, base: { leaves: lb, ...base } };
});

await step('s09-sidebar', async () => {
  await ev(`${T} await T.setup(['Alpha.md'], 0); return true;`);
  const setup = await ev(`
    const rs = app.workspace.rightSplit; rs.expand();
    const types = ['backlink', 'outline'];
    for (const t of types) { if (!app.workspace.getLeavesOfType(t).length) { const l = app.workspace.getRightLeaf(false); await l.setViewState({ type: t, active: true }); } }
    await new Promise((r) => setTimeout(r, 800));
    return Array.from(document.querySelectorAll('.mod-right-split .workspace-tab-header')).map((h) => ({ label: h.getAttribute('aria-label'), type: h.getAttribute('data-type'), cls: h.className }));`);
  const res = { setup };
  for (const type of ['backlink', 'outline']) {
    const r = await ev(`const h = document.querySelector('.mod-right-split .workspace-tab-header[data-type="${type}"]'); if (!h) return null; const r = h.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2 };`);
    if (!r) { res[type] = 'no header'; continue; }
    res[type] = await menuAt(r);
    await dismiss();
  }
  // Left dock's Files tab too.
  const lr = await ev(`app.workspace.leftSplit.expand(); await new Promise((r) => setTimeout(r, 600)); const h = document.querySelector('.mod-left-split .workspace-tab-header[data-type="file-explorer"]'); if (!h) return null; const r = h.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2, w: r.width };`);
  if (lr) { res.fileExplorer = await menuAt(lr, c, { hoverSubmenus: false }); await dismiss(); }
  await ev(`app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse(); return true;`);
  return res;
});

await step('s10-move-to-new-window', async () => {
  const before = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md'], 0);`);
  const targetsBefore = await pageTargets();
  await rightClickHeader(0);
  const clicked = await clickRow('Move to new window');
  await sleep(2500);
  const after = await ev(`${T} return { all: T.allLeaves(), root: T.rootLeaves(), floating: app.workspace.floatingSplit ? app.workspace.floatingSplit.children.length : 0 };`);
  const targetsAfter = await pageTargets();
  const popUrl = targetsAfter.find((u) => !targetsBefore.includes(u));
  const res = { before, clicked, after, popoutTarget: !!popUrl };
  if (!popUrl) return res;
  const p = await connect(popUrl);
  await p.evaluate(HELPERS.replace(/app\./g, 'window.opener.app.'));
  res.popTitle = await p.evaluate(`return document.title;`);
  // Add a second tab in the popout so group rows can show too.
  const r = await p.evaluate(`const h = document.querySelector('.workspace-tab-header'); const r = h.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2, label: h.getAttribute('aria-label') };`);
  res.popoutMenu = await menuAt(r, p, { hoverSubmenus: true });
  await dismiss(p);
  // Click "Move to new window" in the popout too, if it exists.
  const titles = (res.popoutMenu.menus[0] || { rows: [] }).rows.map((x) => x.title);
  if (titles.includes('Move to new window')) {
    const tb = await pageTargets();
    await menuAt(r, p, { hoverSubmenus: false });
    res.popoutMoveClick = await clickRow('Move to new window', p);
    await sleep(2500);
    const ta = await pageTargets();
    res.popoutMoveAfter = { newTargets: ta.filter((u) => !tb.includes(u)).length, floating: await ev(`return app.workspace.floatingSplit.children.length;`), leaves: await ev(`${T} return T.allLeaves();`) };
  }
  try { p.close(); } catch { /* ignore */ }
  // Compare with the command on the main window: same result shape?
  await ev(`for (const ch of [...app.workspace.floatingSplit.children]) { try { ch.win.close(); } catch (e) {} } await new Promise((r) => setTimeout(r, 1500)); return true;`);
  const before2 = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md'], 0);`);
  await ev(`return app.commands.executeCommandById('workspace:move-to-new-window');`); await sleep(2500);
  res.viaCommand = { before: before2, after: await ev(`${T} return { all: T.allLeaves(), root: T.rootLeaves(), floating: app.workspace.floatingSplit.children.length };`) };
  await ev(`for (const ch of [...app.workspace.floatingSplit.children]) { try { ch.win.close(); } catch (e) {} } await new Promise((r) => setTimeout(r, 1500)); return true;`);
  return res;
});

await step('s11-pin-behaviour', async () => {
  const res = {};
  const wsLeaf = async () => {
    await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`); await sleep(2500);
    const raw = await ev(`return await app.vault.adapter.read('.obsidian/workspace.json');`);
    // The raw text of every leaf object in the main split, as written.
    const a = raw.indexOf('"main"'), b = raw.indexOf('"left"');
    return { mainRaw: raw.slice(a, b) };
  };
  const pinnedIdx = () => ev(`const hs = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')); return hs.findIndex((x) => x.querySelector('.workspace-tab-header-status-container > *'));`);
  // (a) bytes of a pinned leaf, then unpinned.
  res.setup = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md'], 0);`);
  await rightClickHeader(0); await clickRow('Pin');
  res.pinnedWorkspace = await wsLeaf();
  await rightClickHeader(0); res.unpinClick = await clickRow('Unpin');
  res.unpinnedWorkspace = await wsLeaf();
  // (c) the pinned header's DOM and a click on the pin icon.
  await rightClickHeader(0); await clickRow('Pin');
  res.pinnedHeader = await ev(`const h = document.querySelectorAll('.mod-root .workspace-tab-header')[0];
    const desc = (e, d = 0) => ({ tag: e.tagName.toLowerCase(), cls: e.getAttribute('class'), aria: e.getAttribute('aria-label'), display: getComputedStyle(e).display, children: d < 4 ? Array.from(e.children).filter((c) => c.tagName.toLowerCase() !== 'path' && c.tagName.toLowerCase() !== 'line' && c.tagName.toLowerCase() !== 'circle').map((c) => desc(c, d + 1)) : [] });
    return desc(h);`);
  const pin = await ev(`const h = document.querySelectorAll('.mod-root .workspace-tab-header')[0]; const p = h.querySelector('.workspace-tab-header-status-container > *'); if (!p) return null; const r = p.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2, cursor: getComputedStyle(p).cursor };`);
  if (pin) {
    await c.mouseMove(pin.cx, pin.cy); await sleep(800);
    res.pinHoverTooltip = await ev(`return Array.from(document.querySelectorAll('.tooltip')).map((t) => t.textContent.trim());`);
    await c.clickAt(pin.cx, pin.cy); await sleep(700);
    res.afterPinIconClick = { cursor: pin.cursor, pinned: await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); return ls.map((l) => [l.getDisplayText(), !!l.getViewState().pinned]);`) };
  }
  // (b) with Alpha pinned and active: follow a link, quick switcher.
  res.setup2 = await ev(`${T} const r = await T.setup(['Alpha.md'], 0); const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); ls[0].setPinned(true); await T.sleep(300); return T.rootLeaves();`);
  res.followLink = await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); const leaf = ls[0]; app.workspace.setActiveLeaf(leaf, { focus: true });
    const ed = leaf.view.editor; const t = ed.getValue(); ed.setCursor(ed.offsetToPos(t.indexOf('[[Beta]]') + 3)); await new Promise((r) => setTimeout(r, 300));
    return app.commands.executeCommandById('editor:follow-link');`);
  await sleep(1200);
  res.afterFollowLink = await ev(`${T} return T.rootLeaves();`);
  // Quick switcher from the pinned tab.
  await ev(`${T} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); for (const l of ls.slice(1)) l.detach(); app.workspace.setActiveLeaf(ls[0], { focus: true }); await T.sleep(300); return true;`);
  await ev(`return app.commands.executeCommandById('switcher:open');`); await sleep(700);
  await c.typeText('Gamma'); await sleep(500);
  await c.pressKey('Enter'); await sleep(1200);
  res.afterQuickSwitcher = await ev(`${T} return T.rootLeaves();`);
  // A click on a link in reading view.
  await ev(`${T} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); for (const l of ls.slice(1)) l.detach(); const leaf = ls[0]; app.workspace.setActiveLeaf(leaf, { focus: true }); const vs = leaf.getViewState(); vs.state.mode = 'preview'; await leaf.setViewState(vs); await T.sleep(800); return leaf.getViewState().pinned;`);
  const link = await c.rect('.mod-root .workspace-leaf.mod-active .markdown-reading-view a.internal-link[data-href="Beta"]');
  if (link) { await c.clickAt(link.cx, link.cy); await sleep(1200); res.afterReadingLinkClick = await ev(`${T} return T.rootLeaves();`); }
  // Close all / Close others with a pinned tab: Alpha pinned, Beta, Gamma unpinned.
  await ev(`${T} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); for (const l of ls) l.setPinned(false); return true;`);
  res.setup3 = await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Folder/Gamma.md'], 1); const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); ls[0].setPinned(true); await T.sleep(300); return T.rootLeaves();`);
  await rightClickHeader(1); res.closeOthersClick = await clickRow('Close others');
  res.afterCloseOthersWithPinned = await ev(`${T} return T.rootLeaves();`);
  await ev(`${T} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); for (const l of ls) l.setPinned(false); return true;`);
  res.setup4 = await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Folder/Gamma.md'], 1); const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); ls[0].setPinned(true); await T.sleep(300); return T.rootLeaves();`);
  await rightClickHeader(1); res.closeAllClick = await clickRow('Close all');
  res.afterCloseAllWithPinned = await ev(`${T} return T.rootLeaves();`);
  // Right-click the pinned tab itself: Close others there.
  await ev(`${T} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); for (const l of ls) l.setPinned(false); return true;`);
  return res;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
