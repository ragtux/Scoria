// Link with tab... and Unlink tab: the pick mode the menu row starts, the
// `group` id it writes, what linked leaves do, and the tab header's link icon
// ([[Development#Link with tab, 2026-11-22]]).
//
//   s00  version; every registered command whose id or name mentions a link,
//        an unlink or a group
//   s01  the menu on an unlinked note tab (active, and inactive), then "Link
//        with tab..." clicked: what changes before the pointer moves (body
//        classes, new elements, cursor, notices), then hovering the other
//        group's content, the source itself, an inactive tab's header, the
//        Files view, the ribbon: the overlay's box and paint
//   s02  the targets: every sidebar leaf (revealed in turn) and an empty tab,
//        hovered at four points each
//   s03  ending the mode without a link: Escape, a click on the ribbon, a
//        click on the source itself
//   s04  the link made by a click on the other group: the ids, which leaf is
//        active, the files, the headers' link icon (markup, box, colour,
//        hover: icon and the leaves highlighted), both menus' row, and the
//        bytes of the linked leaves in workspace.json
//   s05  following: a real link click in either tab, an API open in an
//        inactive one, back, each tab's mode, a canvas and an image, a
//        wheel scroll, an open at a line, an empty tab linked
//   s06  an unlinked inactive tab joining the pair through its short menu: the
//        id it gets, the three following
//   s07  Pin from the menu on a linked tab
//   s08  Unlink tab from the menu, then the header icon clicked: what the
//        others keep, the last one left
//   s09  a sidebar leaf linked: Backlinks picked as the target, what it
//        follows, its leaf's bytes; a sidebar tab's own menu
//   s10  the local graph: graph:open-local's pair in the menus, the graph
//        tab as a source picking a note
//   s11  closing a tab of a pair; then quit
//   after-restart: the links as reloaded and their icons
//
// Phases: fixtures | run | after-restart. ONLY=<regex> picks steps of `run`.
// SHOTS=<dir> saves Page.captureScreenshot PNGs of the mode and the hover.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || null;
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }

const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
const out = { startedAt: new Date().toISOString(), env: { ONLY: process.env.ONLY || null, vault: VAULT }, steps: {}, order: [] };
all[phase] = out;
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
save();
const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return;
  out.order.push(name);
  const r = {};
  out.steps[name] = { ok: null, partial: r }; save();
  try { await withTimeout(fn(r), ms, name); out.steps[name] = { ok: true, ...r }; }
  catch (e) { out.steps[name] = { ok: false, error: String(e && e.stack ? e.stack : e), ...r }; }
  save();
}

const vp = (rel) => path.join(VAULT, rel);
const wsPath = vp('.obsidian/workspace.json');
const readWs = () => { try { return fs.readFileSync(wsPath, 'utf8'); } catch { return null; } };
// Every leaf of a workspace.json text that carries a `group`, with its place
// and its raw text as written (found by its id, brace-matched).
function groupedLeaves(text) {
  if (!text) return null;
  const found = [];
  const walk = (node, where) => {
    if (!node || typeof node !== 'object') return;
    if (node.type === 'leaf' && node.group) found.push({ where, id: node.id, group: node.group, keys: Object.keys(node), stateType: node.state && node.state.type, raw: rawLeaf(text, node.id) });
    (node.children || []).forEach((ch, i) => walk(ch, where + '/' + (node.type || '?') + '[' + i + ']'));
  };
  const j = JSON.parse(text);
  for (const k of ['main', 'left', 'right']) walk(j[k], k);
  return found;
}
function rawLeaf(text, id) {
  const at = text.indexOf('"id": "' + id + '"');
  if (at < 0) return null;
  let start = text.lastIndexOf('{', at);
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    if (text[i] === '{') depth++;
    if (text[i] === '}') { depth--; if (depth === 0) return text.slice(start, i + 1); }
  }
  return null;
}

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), '# Alpha\n\n[[Beta]] and [[Gamma]]\n');
  fs.writeFileSync(vp('Beta.md'), '# Beta\n\n[[Delta]]\n');
  fs.writeFileSync(vp('Gamma.md'), '# Gamma\n\n[[Beta]]\n');
  fs.writeFileSync(vp('Delta.md'), '# Delta\n\n[[Alpha]]\n');
  fs.writeFileSync(vp('Long.md'), '# Long\n\n' + Array.from({ length: 300 }, (_, i) => 'Line ' + i + '\n').join(''));
  fs.writeFileSync(vp('Board.canvas'), '{"nodes":[{"id":"a1","type":"file","file":"Alpha.md","x":0,"y":0,"width":400,"height":300}],"edges":[]}');
  fs.writeFileSync(vp('pic.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64'));
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  save();
  process.exit(0);
}

const c = await connect();
out.vaultPath = await c.vaultPath();
save();
const ev = (body) => c.evaluate(body);
const shot = async (name) => {
  if (!SHOTS) return null;
  fs.mkdirSync(SHOTS, { recursive: true });
  const r = await c.send('Page.captureScreenshot', { format: 'png' });
  const p = path.join(SHOTS, name + '.png');
  fs.writeFileSync(p, Buffer.from(r.data, 'base64'));
  return p;
};
await ev(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  f: (p) => app.vault.getAbstractFileByPath(p),
  centre(el) { if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, w: r.width, h: r.height, left: r.x, top: r.y }; },
  roots() { const o = []; app.workspace.iterateRootLeaves((l) => { o.push(l); }); return o; },
  byFile(p) { return this.roots().find((l) => l.view.file && l.view.file.path === p); },
  desc(l) {
    const v = l.view;
    return { id: l.id, type: v.getViewType(), file: v.file ? v.file.path : null, group: l.group || null,
      mode: v.getMode ? v.getMode() : null, pinned: !!l.pinned, active: l === app.workspace.activeLeaf,
      title: l.getDisplayText ? l.getDisplayText() : null,
      linkIcon: !!(l.tabHeaderEl && l.tabHeaderEl.querySelector('.workspace-tab-header-status-icon.mod-linked')),
      back: l.history ? l.history.backHistory.map((h) => (h.state && h.state.state && h.state.state.file) || (h.state && h.state.type)) : null,
      fwd: l.history ? l.history.forwardHistory.map((h) => (h.state && h.state.state && h.state.state.file) || (h.state && h.state.type)) : null,
      root: l.getRoot() === app.workspace.rootSplit ? 'main' : l.getRoot() === app.workspace.rightSplit ? 'right' : l.getRoot() === app.workspace.leftSplit ? 'left' : 'other' };
  },
  all() { const o = []; app.workspace.iterateAllLeaves((l) => { o.push(this.desc(l)); }); return o; },
  main() { return this.roots().map((l) => this.desc(l)); },
  linked() { return this.all().filter((d) => d.group); },
  overlay() {
    const o = document.querySelector('.workspace-drop-overlay');
    if (!o) return { present: false };
    const cs = getComputedStyle(o); const b = getComputedStyle(o, '::before'); const r = o.getBoundingClientRect();
    return { present: true, shown: cs.display !== 'none', rect: [r.x, r.y, r.width, r.height], position: cs.position, zIndex: cs.zIndex, transition: cs.transition,
      before: { bg: b.backgroundColor, opacity: b.opacity, radius: b.borderRadius, inset: b.inset } };
  },
  overlayLeaf() {
    const o = document.querySelector('.workspace-drop-overlay');
    if (!o || getComputedStyle(o).display === 'none') return null;
    const r = o.getBoundingClientRect(); let hit = null;
    app.workspace.iterateAllLeaves((l) => { const b = l.containerEl.getBoundingClientRect(); if (Math.abs(b.x - r.x) < 2 && Math.abs(b.y - r.y) < 2 && Math.abs(b.width - r.width) < 2 && l.containerEl.offsetParent) hit = hit || this.desc(l); });
    return hit;
  },
  menu() {
    const m = document.querySelector('.menu'); if (!m) return null;
    return Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => {
      if (x.classList.contains('menu-separator')) return '---';
      const svg = x.querySelector('.menu-item-icon svg'); const r = x.getBoundingClientRect();
      return { title: ((x.querySelector('.menu-item-title') || x).textContent || '').trim(), icon: svg ? svg.getAttribute('class').replace('svg-icon ', '') : null,
        section: x.getAttribute('data-section'), disabled: x.classList.contains('is-disabled'), x: r.x + r.width / 2, y: r.y + r.height / 2 };
    });
  },
  paneRows() { const m = this.menu(); return m ? m.filter((x) => x !== '---' && x.section === 'pane').map((x) => x.title + ' [' + x.icon + ']') : null; },
  rowAt(title) { const m = this.menu(); const x = m && m.find((r) => r !== '---' && r.title === title); return x ? { x: x.x, y: x.y } : null; },
  closeMenus() { document.querySelectorAll('.menu').forEach((m) => m.remove()); },
  header(l) { return this.centre(l.tabHeaderEl); },
  linkIcon(l) {
    const s = l.tabHeaderEl && l.tabHeaderEl.querySelector('.workspace-tab-header-status-icon.mod-linked'); if (!s) return null;
    const svg = s.querySelector('svg'); const cs = getComputedStyle(s); const r = s.getBoundingClientRect();
    return { aria: s.getAttribute('aria-label'), cls: s.className, svg: svg && svg.getAttribute('class'), svgSize: svg && getComputedStyle(svg).width,
      box: [r.x, r.y, r.width, r.height], color: cs.color, bg: cs.backgroundColor, radius: cs.borderRadius, cursor: cs.cursor,
      order: Array.from(l.tabHeaderEl.querySelector('.workspace-tab-header-inner').children).map((e) => e.className),
      closeShown: (() => { const x = l.tabHeaderEl.querySelector('.workspace-tab-header-inner-close-button'); return x ? getComputedStyle(x).display !== 'none' : null; })(),
      x: r.x + r.width / 2, y: r.y + r.height / 2 };
  },
  highlighted() {
    const o = []; app.workspace.iterateAllLeaves((l) => { if (l.containerEl.classList.contains('is-highlighted')) o.push(l.id); });
    const el = document.querySelector('.workspace-leaf.is-highlighted');
    const b = el ? getComputedStyle(el, '::before') : null;
    return { ids: o, before: b ? { bg: b.backgroundColor, inset: b.inset, zIndex: b.zIndex, content: b.content } : null };
  },
};
return true;`);

const P = (expr) => ev(`return window.__p.${expr};`);
const fresh = async () => {
  // Main area: Alpha and Beta tabs in one group, Gamma in a group to its right,
  // nothing linked, Alpha active.
  await ev(`
    app.workspace.iterateAllLeaves((l) => { if (l.group) l.setGroup(null); if (l.pinned) l.setPinned(false); });
    for (const l of window.__p.roots()) l.detach();
    await window.__p.sleep(300);
    const a = app.workspace.getLeaf(false); await a.openFile(window.__p.f('Alpha.md'));
    const b = app.workspace.getLeaf('tab'); await b.openFile(window.__p.f('Beta.md'));
    app.workspace.setActiveLeaf(a, { focus: true });
    const g = app.workspace.getLeaf('split', 'vertical'); await g.openFile(window.__p.f('Gamma.md'));
    app.workspace.setActiveLeaf(a, { focus: true });
    await window.__p.sleep(800);
    window.__L = { a, b, g };
    return true;`);
};
const leafPoint = (which, where = 'content') => ev(`const l = window.__L[${JSON.stringify(which)}]; return ${where === 'header' ? 'window.__p.header(l)' : 'window.__p.centre(l.containerEl)'};`);
const menuOn = async (which) => {
  await ev(`window.__p.closeMenus(); return true;`);
  const h = await leafPoint(which, 'header');
  await c.clickAt(h.x, h.y, { button: 'right' });
  await sleep(500);
  return P('menu()');
};
const clickRow = async (title) => {
  const at = await ev(`return window.__p.rowAt(${JSON.stringify(title)});`);
  if (!at) return false;
  await c.clickAt(at.x, at.y);
  await sleep(500);
  return true;
};
const startPick = async (which) => { await menuOn(which); return clickRow('Link with tab...'); };
const hover = async (x, y) => { await c.mouseMove(22, 500); await sleep(150); await c.mouseMove(x, y); await sleep(400); return { overlay: await P('overlay()'), on: await P('overlayLeaf()') }; };
const ribbon = async () => ev(`return window.__p.centre(document.querySelector('.workspace-ribbon'));`);
const flush = async () => { await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`); await sleep(2500); return readWs(); };

if (phase === 'run') {
  await step('s00-meta', async (r) => {
    r.title = await ev(`return document.title;`);
    r.commands = await ev(`return Object.values(app.commands.commands).filter((k) => /link|group|unlink/i.test(k.id + ' ' + k.name)).map((k) => k.id + ' | ' + k.name);`);
    r.window = await ev(`return [innerWidth, innerHeight];`);
  });

  await step('s01-pick-mode', async (r) => {
    await fresh();
    r.menuActive = await menuOn('a');
    r.menuInactive = await menuOn('b');
    await ev(`window.__p.closeMenus(); return true;`);
    await ev(`window.__before = new Set(document.querySelectorAll('*')); window.__bodyCls = document.body.className; return true;`);
    await menuOn('a');
    r.clicked = await clickRow('Link with tab...');
    r.beforeMove = await ev(`
      const added = Array.from(document.querySelectorAll('*')).filter((e) => !window.__before.has(e) && !e.closest('svg')).map((e) => e.tagName + '.' + e.className);
      return { bodyBefore: window.__bodyCls, body: document.body.className, added, cursor: getComputedStyle(document.body).cursor,
        notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent), modals: document.querySelectorAll('.modal').length,
        menus: document.querySelectorAll('.menu').length, activeLeaf: app.workspace.activeLeaf.id, focus: document.activeElement.className };`);
    r.beforeMove.overlay = await P('overlay()');
    const g = await leafPoint('g');
    r.hoverOther = await hover(g.x, g.y);
    r.hoverOther.cursor = await ev(`const e = document.elementFromPoint(${g.x}, ${g.y}); return getComputedStyle(e).cursor;`);
    r.hoverOther.shot = await shot('link-pick-hover');
    const a = await leafPoint('a');
    r.hoverSelf = await hover(a.x, a.y);
    const bh = await leafPoint('b', 'header');
    r.hoverInactiveHeader = await hover(bh.x, bh.y);
    const gh = await leafPoint('g', 'header');
    r.hoverOtherHeader = await hover(gh.x, gh.y);
    const files = await ev(`const l = app.workspace.getLeavesOfType('file-explorer')[0]; return l ? window.__p.centre(l.containerEl) : null;`);
    if (files) r.hoverFiles = await hover(files.x, files.y);
    const rb = await ribbon();
    r.hoverRibbon = await hover(rb.x, rb.y);
    r.leaves = await P('main()');
  });

  await step('s02-targets', async (r) => {
    // Still in the mode from s01 (a hover does not end it).
    r.sidebars = {};
    for (const t of ['file-explorer', 'search', 'bookmarks', 'backlink', 'outgoing-link', 'tag', 'all-properties', 'outline']) {
      const pts = await ev(`const l = app.workspace.getLeavesOfType(${JSON.stringify(t)})[0]; if (!l) return null;
        const root = l.getRoot(); if (root.collapsed) root.expand(); app.workspace.revealLeaf(l); await window.__p.sleep(500);
        const b = l.containerEl.getBoundingClientRect(); const h = l.tabHeaderEl.getBoundingClientRect();
        return [[b.x + b.width / 2, b.y + 60], [b.x + b.width / 2, b.y + b.height / 2], [b.x + b.width / 2, b.y + b.height - 30], [h.x + h.width / 2, h.y + h.height / 2]];`);
      if (!pts) { r.sidebars[t] = null; continue; }
      r.sidebars[t] = [];
      for (const [x, y] of pts) { const h = await hover(x, y); r.sidebars[t].push(h.on ? h.on.type : (h.overlay.shown ? 'overlay on no leaf' : 'none')); }
    }
    // An empty tab in a third group.
    await ev(`const e = app.workspace.getLeaf('split', 'vertical'); window.__L.e = e; await window.__p.sleep(500); return true;`);
    const e = await leafPoint('e');
    const he = await hover(e.x, e.y);
    r.emptyTab = he.on ? he.on.type : 'none';
    r.stillPicking = r.emptyTab !== 'none';
    await ev(`window.__L.e.detach(); app.workspace.leftSplit.expand(); app.workspace.revealLeaf(app.workspace.getLeavesOfType('file-explorer')[0]); app.workspace.rightSplit.collapse(); await window.__p.sleep(400); return true;`);
  });

  await step('s03-cancel', async (r) => {
    const g = await leafPoint('g');
    // Escape (twice), then a hover.
    await c.pressKey('Escape'); await sleep(300); await c.pressKey('Escape'); await sleep(300);
    r.afterEscape = (await hover(g.x, g.y)).on;
    // A click on the ribbon's empty part.
    const rb = await ribbon();
    await c.clickAt(rb.x, rb.y + 200); await sleep(400);
    r.afterRibbonClick = (await hover(g.x, g.y)).on;
    r.linkedAfterRibbon = await P('linked()');
    // Again, ended by a click on the source itself.
    await startPick('a');
    const a = await leafPoint('a');
    await hover(a.x, a.y);
    await c.clickAt(a.x, a.y); await sleep(400);
    r.afterSelfClick = (await hover(g.x, g.y)).on;
    r.linkedAfterSelf = await P('linked()');
    // A right click on another tab during the mode.
    await startPick('a');
    const gh = await leafPoint('g', 'header');
    await c.clickAt(gh.x, gh.y, { button: 'right' }); await sleep(400);
    r.rightClickMenu = await P('paneRows()');
    await c.pressKey('Escape'); await sleep(300);
    r.afterRightClick = (await hover(g.x, g.y)).on;
    r.linkedAfterRightClick = await P('linked()');
    await c.clickAt(rb.x, rb.y + 200); await sleep(300);
  });

  await step('s04-link', async (r) => {
    await fresh();
    r.before = await P('main()');
    await startPick('a');
    const g = await leafPoint('g');
    await hover(g.x, g.y);
    await c.clickAt(g.x, g.y); await sleep(700);
    r.after = await P('main()');
    r.overlayAfter = await P('overlay()');
    r.iconA = await ev(`return window.__p.linkIcon(window.__L.a);`);
    r.iconG = await ev(`return window.__p.linkIcon(window.__L.g);`);
    r.headerHtml = await ev(`return window.__L.a.tabHeaderEl.outerHTML.replace(/<path[^>]*>|<\\/path>|<line[^>]*>|<\\/line>|<circle[^>]*>|<\\/circle>|<polyline[^>]*>|<\\/polyline>|<rect[^>]*>|<\\/rect>/g, '');`);
    // Hover the icon.
    if (r.iconA) {
      await c.mouseMove(r.iconA.x, r.iconA.y); await sleep(900);
      r.iconHover = await ev(`return window.__p.linkIcon(window.__L.a);`);
      r.highlighted = await P('highlighted()');
      r.tooltip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
      r.iconHover.shot = await shot('link-icon-hover');
      await c.mouseMove(800, 600); await sleep(300);
      r.highlightedAfter = await P('highlighted()');
    }
    r.menuA = (await menuOn('a')) && await P('paneRows()');
    r.menuG = (await menuOn('g')) && await P('paneRows()');
    r.menuB = (await menuOn('b')) && await P('paneRows()');
    await ev(`window.__p.closeMenus(); return true;`);
    const ws = await flush();
    r.ws = groupedLeaves(ws);
  });

  await step('s05-follow', async (r) => {
    // A real link click in Alpha (Live Preview): [[Beta]].
    const link = await ev(`const l = window.__L.a; app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(300);
      const el = Array.from(l.view.containerEl.querySelectorAll('.cm-hmd-internal-link, .internal-link')).find((x) => /Beta/.test(x.textContent)); return window.__p.centre(el);`);
    if (link) { await c.clickAt(link.x, link.y); await sleep(1200); }
    r.clickInA = await P('main()');
    // A real link click in Gamma's tab, now on Beta: [[Delta]].
    const link2 = await ev(`const l = window.__L.g; app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(300);
      const el = Array.from(l.view.containerEl.querySelectorAll('.cm-hmd-internal-link, .internal-link')).find((x) => /Delta/.test(x.textContent)); return window.__p.centre(el);`);
    if (link2) { await c.clickAt(link2.x, link2.y); await sleep(1200); }
    r.clickInG = await P('main()');
    // An open in the tab that is not active.
    await ev(`app.workspace.setActiveLeaf(window.__L.b, { focus: true }); await window.__p.sleep(300); await window.__L.a.openFile(window.__p.f('Gamma.md')); await window.__p.sleep(1200); return true;`);
    r.openInInactive = await P('main()');
    // Back in one.
    await ev(`app.workspace.setActiveLeaf(window.__L.a, { focus: true }); await window.__L.a.history.back(); await window.__p.sleep(1200); return true;`);
    r.back = await P('main()');
    // Modes: g in Reading view, a in Source mode, then a opens Alpha.
    await ev(`const g = window.__L.g; await g.setViewState({ type: 'markdown', state: { ...g.getViewState().state, mode: 'preview' } });
      const a = window.__L.a; await a.setViewState({ type: 'markdown', state: { ...a.getViewState().state, mode: 'source', source: true } });
      await window.__p.sleep(500); await a.openFile(window.__p.f('Alpha.md')); await window.__p.sleep(1200); return true;`);
    r.modes = await ev(`return [window.__L.a, window.__L.g].map((l) => ({ file: l.view.file.path, mode: l.view.getMode(), source: l.getViewState().state.source }));`);
    // A canvas, an image, back to a note.
    r.kinds = [];
    for (const p of ['Board.canvas', 'pic.png', 'Alpha.md']) {
      await ev(`await window.__L.a.openFile(window.__p.f(${JSON.stringify(p)})); await window.__p.sleep(1200); return true;`);
      r.kinds.push(await ev(`return [window.__L.a, window.__L.g].map((l) => l.view.getViewType() + ' ' + (l.view.file ? l.view.file.path : '') + ' ' + (l.view.getMode ? l.view.getMode() : ''));`));
    }
    // Scroll: a wheel over g on Long.md.
    await ev(`app.workspace.setActiveLeaf(window.__L.g, { focus: true }); await window.__L.g.openFile(window.__p.f('Long.md')); await window.__p.sleep(1500); return true;`);
    r.scrollBefore = await ev(`return [window.__L.a, window.__L.g].map((l) => l.view.currentMode.getScroll());`);
    const gp = await leafPoint('g');
    await c.mouseMove(gp.x, gp.y);
    for (let i = 0; i < 3; i++) { await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: Math.round(gp.x), y: Math.round(gp.y), deltaX: 0, deltaY: 600 }); await sleep(250); }
    await sleep(1000);
    r.scrollAfterWheel = await ev(`return [window.__L.a, window.__L.g].map((l) => l.view.currentMode.getScroll());`);
    // An open at a line (an ephemeral state).
    await ev(`window.__L.g.setEphemeralState({ line: 200 }); await window.__p.sleep(1000); return true;`);
    r.scrollAfterLine = await ev(`return [window.__L.a, window.__L.g].map((l) => l.view.currentMode.getScroll());`);
    // A wheel over a with g in Reading view.
    await ev(`const g = window.__L.g; await g.setViewState({ type: 'markdown', state: { ...g.getViewState().state, mode: 'preview' } }); await window.__p.sleep(800); return true;`);
    const ap = await leafPoint('a');
    await c.mouseMove(ap.x, ap.y);
    for (let i = 0; i < 3; i++) { await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: Math.round(ap.x), y: Math.round(ap.y), deltaX: 0, deltaY: 600 }); await sleep(250); }
    await sleep(1000);
    r.scrollMixedModes = await ev(`return [window.__L.a, window.__L.g].map((l) => [l.view.getMode(), l.view.currentMode.getScroll()]);`);
    await ev(`const g = window.__L.g; await g.setViewState({ type: 'markdown', state: { ...g.getViewState().state, mode: 'source' } }); await window.__p.sleep(500); return true;`);
    // An empty tab put in the link.
    await ev(`const e = app.workspace.getLeaf('tab'); window.__L.e = e; await window.__p.sleep(300); e.setGroup(window.__L.a.group);
      await window.__L.a.openFile(window.__p.f('Beta.md')); await window.__p.sleep(1200); return true;`);
    r.emptyLinked = await P('main()');
    await ev(`await window.__L.e.openFile(window.__p.f('Gamma.md')); await window.__p.sleep(1200); return true;`);
    r.emptyOpens = await P('main()');
    await ev(`window.__L.e.detach(); await window.__p.sleep(400); await window.__L.a.openFile(window.__p.f('Alpha.md')); await window.__p.sleep(800); return true;`);
    r.end = await P('main()');
  });

  await step('s06-join', async (r) => {
    // Beta's tab (inactive, behind Alpha) linked with Gamma, which is linked already.
    r.menu = (await menuOn('b')) && await P('paneRows()');
    await clickRow('Link with tab...');
    const g = await leafPoint('g');
    await hover(g.x, g.y);
    await c.clickAt(g.x, g.y); await sleep(800);
    r.after = await P('main()');
    await ev(`await window.__L.g.openFile(window.__p.f('Delta.md')); await window.__p.sleep(1200); return true;`);
    r.threeFollow = await P('main()');
    // Linking to the tab shown behind an inactive header: Gamma's group header
    // picked from a new unlinked split.
    const ws = await flush();
    r.ws = groupedLeaves(ws);
  });

  await step('s07-pin', async (r) => {
    await menuOn('a');
    await clickRow('Pin');
    await sleep(300);
    r.afterPin = await P('main()');
    r.headerG = await ev(`return Array.from(window.__L.g.tabHeaderEl.querySelectorAll('.workspace-tab-header-status-icon')).map((e) => e.className + ' ' + e.getAttribute('aria-label'));`);
    await menuOn('g');
    r.menuG = await P('paneRows()');
    await clickRow('Unpin');
    await sleep(300);
    r.afterUnpin = await P('main()');
  });

  await step('s08-unlink', async (r) => {
    r.menuB = (await menuOn('b')) && await P('paneRows()');
    await clickRow('Unlink tab');
    r.afterUnlinkB = await P('main()');
    // The header icon, hovered first (the hover swaps its glyph), read again, clicked once.
    let icon = await ev(`return window.__p.linkIcon(window.__L.a);`);
    if (icon) { await c.mouseMove(icon.x, icon.y); await sleep(600); icon = await ev(`return window.__p.linkIcon(window.__L.a);`); }
    r.iconClickedAt = icon;
    if (icon) { await c.clickAt(icon.x, icon.y); await sleep(600); }
    r.afterIconA = await P('main()');
    const ws = await flush();
    r.ws = groupedLeaves(ws);
  });

  await step('s09-sidebar', async (r) => {
    await fresh();
    await ev(`app.workspace.rightSplit.expand(); app.workspace.revealLeaf(app.workspace.getLeavesOfType('backlink')[0]); await window.__p.sleep(600); return true;`);
    await startPick('g');
    const bl = await ev(`return window.__p.centre(app.workspace.getLeavesOfType('backlink')[0].containerEl);`);
    await hover(bl.x, bl.y);
    await c.clickAt(bl.x, bl.y); await sleep(800);
    r.linked = await P('linked()');
    r.activeAfter = await ev(`return window.__p.desc(app.workspace.activeLeaf);`);
    await ev(`app.workspace.setActiveLeaf(window.__L.a, { focus: true }); await window.__L.a.openFile(window.__p.f('Delta.md')); await window.__p.sleep(2500); return true;`);
    r.unlinkedActiveMoves = await ev(`return [window.__p.desc(app.workspace.getLeavesOfType('backlink')[0]), window.__p.desc(window.__L.g)];`);
    await ev(`await window.__L.g.openFile(window.__p.f('Beta.md')); await window.__p.sleep(1500); return true;`);
    r.linkedNoteMoves = await ev(`const b = app.workspace.getLeavesOfType('backlink')[0]; return [window.__p.desc(b), b.getViewState()];`);
    r.sidebarHeader = await ev(`return window.__p.linkIcon(app.workspace.getLeavesOfType('backlink')[0]);`);
    const ws = await flush();
    r.ws = groupedLeaves(ws);
    // A sidebar tab's own menu (Outline, unlinked) and Backlinks' (linked).
    const oh = await ev(`const o = app.workspace.getLeavesOfType('outline')[0]; return window.__p.header(o);`);
    await c.clickAt(oh.x, oh.y, { button: 'right' }); await sleep(400);
    r.outlineMenu = await P('paneRows()');
    await ev(`window.__p.closeMenus(); app.workspace.revealLeaf(app.workspace.getLeavesOfType('backlink')[0]); await window.__p.sleep(400); return true;`);
    const bh = await ev(`return window.__p.header(app.workspace.getLeavesOfType('backlink')[0]);`);
    await c.clickAt(bh.x, bh.y, { button: 'right' }); await sleep(400);
    r.backlinkMenu = await P('paneRows()');
    await clickRow('Unlink tab');
    r.afterUnlink = await P('linked()');
    await ev(`app.workspace.rightSplit.collapse(); return true;`);
  });

  await step('s10-local-graph', async (r) => {
    await fresh();
    await ev(`app.workspace.setActiveLeaf(window.__L.a, { focus: true }); app.commands.executeCommandById('graph:open-local'); await window.__p.sleep(1500);
      window.__L.lg = app.workspace.getLeavesOfType('localgraph')[0]; return true;`);
    r.pair = await P('linked()');
    r.menuNote = (await menuOn('a')) && await P('paneRows()');
    r.menuGraph = (await menuOn('lg')) && await P('paneRows()');
    r.iconGraph = await ev(`return window.__p.linkIcon(window.__L.lg);`);
    await ev(`window.__p.closeMenus(); return true;`);
    // Unlink the note: the graph is left alone in its link.
    await menuOn('a');
    await clickRow('Unlink tab');
    r.afterUnlinkNote = await P('linked()');
    r.graphAfter = await ev(`return window.__p.desc(window.__L.lg);`);
    // The graph tab as the source, Gamma the target.
    r.menuGraphUnlinked = (await menuOn('lg')) && await P('paneRows()');
    await clickRow('Link with tab...');
    const g = await leafPoint('g');
    await hover(g.x, g.y);
    await c.clickAt(g.x, g.y); await sleep(1000);
    r.graphLinkedToGamma = await P('linked()');
    r.graphState = await ev(`return window.__L.lg.getViewState().state.file;`);
    await ev(`await window.__L.g.openFile(window.__p.f('Delta.md')); await window.__p.sleep(1500); return true;`);
    r.graphFollows = await ev(`return [window.__L.lg.getViewState().state.file, window.__L.lg.getDisplayText(), window.__p.desc(window.__L.g).file];`);
    // The graph's own file never drives the note (it has no open of its own here).
  });

  await step('s11-close', async (r) => {
    await fresh();
    await ev(`window.__L.a.setGroup('0123456789abcdef'); window.__L.g.setGroup('0123456789abcdef'); await window.__p.sleep(300); return true;`);
    r.setGroupApi = await P('linked()');
    await ev(`window.__L.g.detach(); await window.__p.sleep(600); return true;`);
    r.afterClose = await P('main()');
    // Leave a linked pair for the relaunch.
    await ev(`const g = app.workspace.getLeaf('split', 'vertical'); await g.openFile(window.__p.f('Gamma.md')); window.__L.g = g; app.workspace.setActiveLeaf(window.__L.a, { focus: true }); await window.__p.sleep(600); return true;`);
    await startPick('a');
    const g = await leafPoint('g');
    await hover(g.x, g.y);
    await c.clickAt(g.x, g.y); await sleep(800);
    // The earlier leaf made active before the quit: which file the pair comes back on.
    await ev(`app.workspace.setActiveLeaf(window.__L.a, { focus: true }); await window.__p.sleep(400); return true;`);
    r.final = await P('main()');
    r.ws = groupedLeaves(await flush());
    r.wsFinal = readWs();
    await c.quit();
  }, 180000);
}

if (phase === 'after-restart') {
  await step('r00-reloaded', async (r) => {
    await sleep(2500);
    r.title = await ev(`return document.title;`);
    r.main = await P('main()');
    r.icons = await ev(`return window.__p.roots().map((l) => window.__p.linkIcon(l) && window.__p.linkIcon(l).aria);`);
    r.ws = groupedLeaves(readWs());
  });
  c.close();
  process.exit(0);
}
c.close();
process.exit(0);
