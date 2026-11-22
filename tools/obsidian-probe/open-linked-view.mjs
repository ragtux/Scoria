// The view header's "Open linked view" submenu and the linked leaves its rows
// make ([[Development#Open linked view, 2026-11-22]]).
//
//   s00  version; the commands of the linked views and their default hotkeys
//   s01  the submenu on a note, a canvas, a PDF, an image and a base: from the
//        view header's More options and from the tab header's right-click
//        menu, under real clicks and a real hover; each row's title and icon
//   s02  each row of the note's submenu, clicked from a lone Alpha tab: the
//        leaves it adds (type, state, group, place, title, icon), where focus
//        is, the note leaf's group, the new leaf's bytes in workspace.json,
//        its tab and view header; then the same row a second time
//   s03  following, per row: the note's leaf opens Beta, an unlinked tab
//        opens Delta, the note's leaf opens the canvas, the image, Gamma, then
//        the note's leaf closes and another file becomes active
//   s04  the linked Backlinks leaf's own controls (collapse, context, sort,
//        search, the two section folds): what its state keeps and whether
//        the dock's Backlinks leaf moves with it; the same for the linked
//        Outline and Outgoing links leaves; the linked leaf's own menus
//   s05  the commands with a note open: where each opens its view, and
//        whether it is linked
//   s07  a row clicked in a linked Backlinks, Outgoing links and Outline
//        view: which leaf opens the file; the command's unlinked leaf over
//        another note and over an empty tab
//   s06  a canvas's, a PDF's, an image's and a base's rows clicked (the views
//        they open); then every row on Alpha, and quit
//   after-restart: the linked leaves as reloaded
//
// Phases: fixtures | run | after-restart. ONLY=<regex> picks steps of `run`.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }

const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
const out = { startedAt: new Date().toISOString(), env: { ONLY: process.env.ONLY || null, vault: VAULT }, steps: {}, order: [] };
all[phase] = out;
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
save();
const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 240000) {
  if (ONLY && !ONLY.test(name)) return;
  out.order.push(name);
  const r = {};
  out.steps[name] = { ok: null, partial: r }; save();
  const t0 = Date.now();
  try { await withTimeout(fn(r), ms, name); out.steps[name] = { ok: true, ...r }; }
  catch (e) { out.steps[name] = { ok: false, error: String(e && e.stack ? e.stack : e), ...r }; }
  console.error(`[open-linked-view] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]} (${Date.now() - t0} ms)`);
  save();
}

const vp = (rel) => path.join(VAULT, rel);
const wsPath = vp('.obsidian/workspace.json');
const readWs = () => { try { return fs.readFileSync(wsPath, 'utf8'); } catch { return null; } };
// The raw text of the leaf object with this id, as written, by brace matching.
function rawLeaf(text, id) {
  if (!text || !id) return null;
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
// A one-page PDF with correct xref offsets.
function tinyPdf() {
  const objs = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
    null,
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
  ];
  const stream = 'BT /F1 18 Tf 20 100 Td (Hello) Tj ET';
  objs[3] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  let s = '%PDF-1.4\n';
  const offs = [];
  objs.forEach((o, i) => { offs.push(s.length); s += `${i + 1} 0 obj\n${o}\nendobj\n`; });
  const xref = s.length;
  s += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n` + offs.map((o) => String(o).padStart(10, '0') + ' 00000 n \n').join('');
  s += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return s;
}

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), '---\ntags: [x]\nstatus: draft\n---\n# Alpha\n\n## One\n\n[[Beta]] and [[Gamma]] and [[Nowhere]]\n\n## Two\n\nBeta is mentioned here.\n');
  fs.writeFileSync(vp('Beta.md'), '# Beta\n\n## Part\n\n[[Delta]]\n');
  fs.writeFileSync(vp('Gamma.md'), '# Gamma\n\n[[Beta]] and [[Alpha#One]]\n\nAlpha again, unlinked.\n');
  fs.writeFileSync(vp('Delta.md'), '# Delta\n\n[[Epsilon]] [[Beta]]\n');
  fs.writeFileSync(vp('Epsilon.md'), '---\nrating: 3\n---\n# Epsilon\n');
  fs.writeFileSync(vp('Board.canvas'), '{"nodes":[{"id":"a1","type":"file","file":"Alpha.md","x":0,"y":0,"width":400,"height":300}],"edges":[]}');
  fs.writeFileSync(vp('pic.png'), Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64'));
  fs.writeFileSync(vp('doc.pdf'), tinyPdf(), 'latin1');
  fs.writeFileSync(vp('Table.base'), 'views:\n  - type: table\n    name: Table\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  save();
  process.exit(0);
}

const c = await connect();
out.vaultPath = await c.vaultPath();
save();
const ev = (body) => c.evaluate(body);
await ev(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  avail(id) { return app.commands.listCommands().some((k) => k.id === id); },
  run(id) { let ran = null, err = null; try { ran = app.commands.executeCommandById(id); } catch (x) { err = String(x); } return { ran, err }; },
  where(l) {
    if (!l) return null;
    const root = l.getRoot();
    const split = root === app.workspace.rootSplit ? 'main' : root === app.workspace.rightSplit ? 'right' : root === app.workspace.leftSplit ? 'left' : 'other';
    const g = l.parent;
    const path = []; let n = l; while (n.parent) { const p = n.parent; path.unshift((p.type || '?') + '[' + p.children.indexOf(n) + ']' + (p.direction ? '(' + p.direction + ')' : '')); n = p; }
    return { split, path: path.join('/'), indexInGroup: g && g.children ? g.children.indexOf(l) : null, groupSize: g && g.children ? g.children.length : null,
      groupTypes: g && g.children ? g.children.map((x) => x.view.getViewType()) : null };
  },
  desc(l) {
    if (!l) return null; const v = l.view;
    const h = l.tabHeaderEl; const hsvg = h && h.querySelector('.workspace-tab-header-inner-icon svg');
    const vh = v.containerEl.querySelector('.view-header');
    return { id: l.id, type: v.getViewType(), file: v.file ? v.file.path : null, display: v.getDisplayText ? v.getDisplayText() : null,
      icon: v.getIcon ? v.getIcon() : null, group: l.group || null, where: this.where(l),
      state: JSON.parse(JSON.stringify(l.getViewState())), isActive: l === app.workspace.activeLeaf,
      tabTitle: h ? (h.querySelector('.workspace-tab-header-inner-title') || {}).textContent : null,
      tabAria: h ? h.getAttribute('aria-label') : null, tabIcon: hsvg ? hsvg.getAttribute('class') : null,
      viewHeader: vh ? { display: getComputedStyle(vh).display, title: (vh.querySelector('.view-header-title') || {}).textContent || null,
        actions: Array.from(vh.querySelectorAll('.view-action')).map((b) => b.getAttribute('aria-label')) } : null,
      text: v.containerEl.innerText.replace(/\\s+/g, ' ').slice(0, 400) };
  },
  // A walk of the tree itself: iterateAllLeaves missed a leaf a submenu row had just made.
  all() {
    const o = []; const seen = new Set();
    const walk = (n) => { if (!n) return; if (n.children) n.children.forEach(walk); else if (n.view && !seen.has(n)) { seen.add(n); o.push(n); } };
    const w = app.workspace; [w.rootSplit, w.leftSplit, w.rightSplit, w.floatingSplit].forEach(walk);
    app.workspace.iterateAllLeaves((l) => { if (!seen.has(l)) { seen.add(l); o.push(l); } });
    return o;
  },
  rootLeaves() { return this.all().filter((l) => l.getRoot() === app.workspace.rootSplit); },
  ids() { return this.all().map((l) => l.id); },
  mainDesc() { return this.rootLeaves().map((l) => this.desc(l)); },
  focusLeaf() { const a = document.activeElement; const l = this.all().find((x) => x.containerEl.contains(a)); return { tag: a ? a.tagName + '.' + String(a.className).slice(0, 60) : null, leafType: l ? l.view.getViewType() : null, leafId: l ? l.id : null }; },
  rightHeaders() { return Array.from(app.workspace.rightSplit.containerEl.querySelectorAll('.workspace-tab-header')).map((h) => ({ aria: h.getAttribute('aria-label'), dataType: h.getAttribute('data-type'), active: h.classList.contains('is-active') })); },
  menus() {
    return Array.from(document.querySelectorAll('.menu')).map((m) => Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => {
      if (x.classList.contains('menu-separator')) return '---';
      const svg = x.querySelector('.menu-item-icon svg');
      return ((x.querySelector('.menu-item-title') || x).textContent || '').trim() + (svg ? ' [' + svg.getAttribute('class').replace('svg-icon ', '') + ']' : '') + (x.classList.contains('is-disabled') ? ' (disabled)' : '') + (x.classList.contains('has-submenu') ? ' >' : '') + (x.dataset && x.dataset.section ? ' {' + x.dataset.section + '}' : '');
    }));
  },
  itemRect(title, menuIndex) {
    const ms = Array.from(document.querySelectorAll('.menu')); const m = menuIndex === undefined ? ms[ms.length - 1] : ms[menuIndex]; if (!m) return null;
    const it = Array.from(m.querySelectorAll('.menu-item')).find((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim() === title);
    if (!it) return null; const rc = it.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };
  },
  closeMenus() { document.querySelectorAll('.menu').forEach((m) => m.remove()); },
  moreRect(l) { const b = l && l.view.containerEl.querySelector('.view-action[aria-label="More options"]'); if (!b || !b.offsetParent) return null; const rc = b.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 }; },
  tabRect(l) { const h = l && l.tabHeaderEl; if (!h || !h.offsetParent) return null; const rc = h.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 }; },
  byId(id) { return this.all().find((l) => l.id === id) || null; },
  async reset(file) {
    for (const l of this.rootLeaves()) { l.setPinned && l.setPinned(false); l.detach(); }
    await this.sleep(400);
    const l = app.workspace.getLeaf(false);
    if (file) await l.openFile(app.vault.getAbstractFileByPath(file));
    app.workspace.setActiveLeaf(l, { focus: true });
    await this.sleep(700);
    return l.id;
  },
};
return true;`);
const flush = async () => { await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`); await sleep(2800); return readWs(); };
const menus = () => ev(`return window.__p.menus();`);
const escape = async () => { await c.pressKey('Escape'); await sleep(200); await ev(`window.__p.closeMenus(); return true;`); await sleep(200); };

// Open the submenu from the leaf's More options ('more') or its tab header ('tab').
async function openSubmenu(leafId, from) {
  const rc = await ev(`const l = window.__p.byId(${JSON.stringify(leafId)}); return ${from === 'more' ? 'window.__p.moreRect(l)' : 'window.__p.tabRect(l)'};`);
  if (!rc) return { opened: false };
  await c.clickAt(rc.x, rc.y, from === 'tab' ? { button: 'right' } : {});
  await sleep(600);
  const top = await menus();
  const it = await ev(`return window.__p.itemRect('Open linked view', 0);`);
  if (!it) return { opened: true, top, submenu: null };
  await c.mouseMove(it.x - 20, it.y); await sleep(150);
  await c.mouseMove(it.x, it.y);
  await sleep(900);
  const ms = await menus();
  return { opened: true, top, submenu: ms.length > 1 ? ms[ms.length - 1] : null };
}
// Click a submenu row (by title) after openSubmenu; the pointer travels right into the submenu first.
async function clickRow(title) {
  const it = await ev(`return window.__p.itemRect(${JSON.stringify(title)});`);
  if (!it) return false;
  // Straight across from the parent row into the submenu, then a real click.
  for (let k = 1; k <= 4; k++) { await c.mouseMove(it.x - 40 + k * 10, it.y); await sleep(60); }
  await sleep(200);
  const still = await ev(`return window.__p.itemRect(${JSON.stringify(title)});`);
  if (!still) return 'gone before click';
  await c.clickAt(still.x, still.y);
  await sleep(1500);
  const open = await ev(`return document.querySelectorAll('.menu').length;`);
  return open ? 'clicked; menus still open: ' + open : 'clicked';
}
const snapNew = async (beforeIds) => ev(`const before = new Set(${JSON.stringify(beforeIds)}); return window.__p.all().filter((l) => !before.has(l.id)).map((l) => window.__p.desc(l));`);
const ROWS = ['Open local graph', 'Open backlinks', 'Open outgoing links', 'Open file properties', 'Open outline'];

if (phase === 'run') {
  await step('s00-meta', async (r) => {
    r.title = await ev(`return document.title;`);
    r.userAgent = await ev(`return navigator.userAgent;`);
    await ev(`await window.__p.reset('Alpha.md'); return true;`);
    r.commands = await ev(`return app.commands.listCommands().filter((k) => /backlink|outgoing|outline|graph|propert|link/.test(k.id)).map((k) => ({ id: k.id, name: k.name, hotkeys: (app.hotkeyManager.getDefaultHotkeys(k.id) || []).map((h) => h.modifiers.join('+') + '+' + h.key) }));`);
    r.allCommandIdsWithLinked = await ev(`return Object.keys(app.commands.commands).filter((id) => /linked|for-current|open-local|open-backlinks/.test(id));`);
    r.rightHeaders = await ev(`return window.__p.rightHeaders();`);
    r.rightLeaves = await ev(`return window.__p.all().filter((l) => l.getRoot() === app.workspace.rightSplit).map((l) => ({ id: l.id, type: l.view.getViewType(), state: l.getViewState().state }));`);
    r.ws = await flush();
  });

  await step('s01-submenus', async (r) => {
    for (const f of ['Alpha.md', 'Board.canvas', 'doc.pdf', 'pic.png', 'Table.base']) {
      const id = await ev(`return await window.__p.reset(${JSON.stringify(f)});`);
      await sleep(800);
      const o = {};
      o.leaf = await ev(`const l = window.__p.byId(${JSON.stringify(id)}); return { type: l.view.getViewType(), file: l.view.file && l.view.file.path };`);
      o.more = await openSubmenu(id, 'more');
      await escape();
      o.tab = await openSubmenu(id, 'tab');
      await escape();
      // Inactive tab: a second tab active, right-click the first.
      if (f === 'Alpha.md') {
        await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Beta.md')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(600); return true;`);
        o.inactiveTab = await openSubmenu(id, 'tab');
        await escape();
      }
      r[f] = o;
    }
    // Reading view of the note.
    const id = await ev(`return await window.__p.reset('Alpha.md');`);
    await ev(`const l = window.__p.byId(${JSON.stringify(id)}); await l.setViewState({ type: 'markdown', state: { file: 'Alpha.md', mode: 'preview' } }); await window.__p.sleep(600); return true;`);
    r.reading = await openSubmenu(id, 'more');
    await escape();
    // An empty tab.
    const eid = await ev(`return await window.__p.reset(null);`);
    r.emptyTab = await openSubmenu(eid, 'tab');
    await escape();
  });

  for (const [i, row] of ROWS.entries()) {
    await step('s02-row-' + i, async (r) => {
      r.row = row;
      const id = await ev(`return await window.__p.reset('Alpha.md');`);
      r.noteLeafId = id;
      await ev(`app.workspace.rightSplit.collapse(); return true;`);
      await sleep(300);
      const before = await ev(`return window.__p.ids();`);
      r.rightCollapsedBefore = true;
      r.menu = await openSubmenu(id, 'more');
      r.clicked = await clickRow(row);
      await sleep(800);
      r.added = await snapNew(before);
      r.active = await ev(`return window.__p.desc(app.workspace.activeLeaf);`);
      r.focus = await ev(`return window.__p.focusLeaf();`);
      r.note = await ev(`return window.__p.desc(window.__p.byId(${JSON.stringify(id)}));`);
      r.rightCollapsedAfter = await ev(`return app.workspace.rightSplit.collapsed;`);
      r.rootChildren = await ev(`return app.workspace.rootSplit.children.map((g) => ({ type: g.type, direction: g.direction, n: g.children.length, types: g.children.map((x) => x.view ? x.view.getViewType() : x.type) }));`);
      r.rootDirection = await ev(`return app.workspace.rootSplit.direction;`);
      const ws = await flush();
      r.wsRaw = (r.added || []).map((d) => rawLeaf(ws, d.id));
      r.wsNoteRaw = rawLeaf(ws, id);
      r.wsMain = ws ? JSON.parse(ws).main : null;
      // The same row again, from the note.
      await ev(`app.workspace.setActiveLeaf(window.__p.byId(${JSON.stringify(id)}), { focus: true }); await window.__p.sleep(400); return true;`);
      const before2 = await ev(`return window.__p.ids();`);
      r.menu2 = await openSubmenu(id, 'more');
      r.clicked2 = await clickRow(row);
      await sleep(800);
      r.added2 = await snapNew(before2);
      r.active2 = await ev(`return window.__p.desc(app.workspace.activeLeaf);`);
      r.main2 = await ev(`return window.__p.mainDesc().map((d) => ({ id: d.id, type: d.type, group: d.group, path: d.where.path, title: d.tabTitle, active: d.isActive }));`);
      // From the linked view itself: the same row on its note?
      if ((r.added || []).length) {
        const lid = r.added[0].id;
        r.linkedMore = await openSubmenu(lid, 'more');
        await escape();
        r.linkedTab = await openSubmenu(lid, 'tab');
        await escape();
      }
      await ev(`app.workspace.rightSplit.expand(); return true;`);
    });
  }

  for (const [i, row] of ROWS.entries()) {
    if (i === 0) continue; // the local graph's following is in local-graph-pane.mjs
    await step('s03-follow-' + i, async (r) => {
      r.row = row;
      const id = await ev(`return await window.__p.reset('Alpha.md');`);
      const before = await ev(`return window.__p.ids();`);
      await openSubmenu(id, 'more');
      await clickRow(row);
      await sleep(800);
      const added = await snapNew(before);
      const lid = added[0] && added[0].id;
      r.start = added[0] || null;
      const linked = () => ev(`return window.__p.desc(window.__p.byId(${JSON.stringify(lid)}));`);
      const inNote = async (f) => { await ev(`const l = window.__p.byId(${JSON.stringify(id)}); app.workspace.setActiveLeaf(l, { focus: true }); await l.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(f)})); await window.__p.sleep(1200); return true;`); return linked(); };
      r.noteToBeta = await inNote('Beta.md');
      await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Delta.md')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(1200); return true;`);
      r.unlinkedDelta = await linked();
      r.noteToCanvas = await inNote('Board.canvas');
      r.wsCanvas = rawLeaf(await flush(), lid);
      r.noteToImage = await inNote('pic.png');
      r.noteToPdf = await inNote('doc.pdf');
      r.noteToGamma = await inNote('Gamma.md');
      // The linked view made active itself, then a click in it.
      await ev(`app.workspace.setActiveLeaf(window.__p.byId(${JSON.stringify(lid)}), { focus: true }); await window.__p.sleep(600); return true;`);
      r.linkedActive = await linked();
      // The note's tab closed.
      await ev(`window.__p.byId(${JSON.stringify(id)}).detach(); await window.__p.sleep(1200); return true;`);
      r.noteClosed = await linked();
      r.wsNoteClosed = rawLeaf(await flush(), lid);
      await ev(`const l = window.__p.rootLeaves().find((x) => x.view.getViewType() === 'markdown'); if (l) { await l.openFile(app.vault.getAbstractFileByPath('Epsilon.md')); app.workspace.setActiveLeaf(l, { focus: true }); } await window.__p.sleep(1200); return !!l;`);
      r.afterCloseEpsilon = await linked();
      r.mainAfter = await ev(`return window.__p.mainDesc().map((d) => ({ id: d.id, type: d.type, file: d.file, group: d.group, title: d.tabTitle, active: d.isActive }));`);
    });
  }

  await step('s04-own-state', async (r) => {
    // Backlinks linked to Beta (Alpha, Gamma and Delta link to it; Alpha mentions it unlinked).
    const id = await ev(`return await window.__p.reset('Beta.md');`);
    await ev(`app.workspace.rightSplit.expand(); const d = app.workspace.getLeavesOfType('backlink').find((l) => l.getRoot() === app.workspace.rightSplit); if (d) app.workspace.revealLeaf(d); await window.__p.sleep(500); return true;`);
    r.dockBefore = await ev(`const d = app.workspace.getLeavesOfType('backlink').find((l) => l.getRoot() === app.workspace.rightSplit); return d ? d.getViewState() : null;`);
    const before = await ev(`return window.__p.ids();`);
    await openSubmenu(id, 'more');
    await clickRow('Open backlinks');
    const added = await snapNew(before);
    const lid = added[0] && added[0].id;
    r.start = added[0] || null;
    r.buttons = await ev(`const l = window.__p.byId(${JSON.stringify(lid)}); return Array.from(l.view.containerEl.querySelectorAll('.nav-action-button, .clickable-icon')).filter((b) => b.offsetParent).map((b) => ({ aria: b.getAttribute('aria-label'), cls: String(b.className) }));`);
    const clickIn = async (aria) => {
      const rc = await ev(`const l = window.__p.byId(${JSON.stringify(lid)}); const b = Array.from(l.view.containerEl.querySelectorAll('[aria-label]')).find((x) => x.getAttribute('aria-label') === ${JSON.stringify(aria)} && x.offsetParent); if (!b) return null; const rc = b.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
      if (!rc) return false;
      await c.clickAt(rc.x, rc.y); await sleep(600); return true;
    };
    r.clicks = {};
    for (const a of ['Collapse results', 'Show more context']) r.clicks[a] = await clickIn(a);
    r.clicks.sort = await clickIn('Change sort order');
    r.sortMenu = await menus();
    const z = await ev(`return window.__p.itemRect('File name (Z to A)');`);
    if (z) { await c.clickAt(z.x, z.y); await sleep(600); }
    else await escape();
    r.clicks.search = await clickIn('Show search filter');
    await sleep(300);
    const inp = await ev(`const l = window.__p.byId(${JSON.stringify(lid)}); const i = l.view.containerEl.querySelector('input[type=search], .search-input-container input'); if (!i || !i.offsetParent) return null; const rc = i.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
    if (inp) { await c.clickAt(inp.x, inp.y); await c.typeText('Gam'); await sleep(900); }
    // Fold the unlinked mentions section open and the linked one shut.
    r.sectionHeads = await ev(`const l = window.__p.byId(${JSON.stringify(lid)}); return Array.from(l.view.containerEl.querySelectorAll('.tree-item-self')).filter((x) => x.offsetParent && /mentions/i.test(x.textContent)).map((x) => x.textContent.trim());`);
    const head = async (re) => ev(`const l = window.__p.byId(${JSON.stringify(lid)}); const h = Array.from(l.view.containerEl.querySelectorAll('.tree-item-self')).find((x) => x.offsetParent && ${re}.test(x.textContent)); if (!h) return null; const rc = h.getBoundingClientRect(); return { x: rc.x + 30, y: rc.y + rc.height / 2 };`);
    for (const re of ['/^Unlinked mentions/', '/^Linked mentions/']) { const h = await head(re); if (h) { await c.clickAt(h.x, h.y); await sleep(600); } }
    r.after = await ev(`return window.__p.desc(window.__p.byId(${JSON.stringify(lid)}));`);
    r.dockAfter = await ev(`const d = app.workspace.getLeavesOfType('backlink').find((l) => l.getRoot() === app.workspace.rightSplit); return d ? d.getViewState() : null;`);
    let ws = await flush();
    r.wsBacklink = rawLeaf(ws, lid);
    // The note moves on: what of that state travels?
    await ev(`const l = window.__p.byId(${JSON.stringify(id)}); app.workspace.setActiveLeaf(l, { focus: true }); await l.openFile(app.vault.getAbstractFileByPath('Delta.md')); await window.__p.sleep(1200); return true;`);
    r.afterNoteMoved = await ev(`return window.__p.desc(window.__p.byId(${JSON.stringify(lid)}));`);
    // Outline and Outgoing links linked, with their own controls.
    for (const [row, arias] of [['Open outline', ['Show search filter']], ['Open outgoing links', ['Collapse results']], ['Open file properties', []]]) {
      await ev(`const l = window.__p.byId(${JSON.stringify(id)}); await l.openFile(app.vault.getAbstractFileByPath('Alpha.md')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(800); return true;`);
      const b2 = await ev(`return window.__p.ids();`);
      await openSubmenu(id, 'more');
      await clickRow(row);
      const a2 = await snapNew(b2);
      const l2 = a2[0] && a2[0].id;
      const o = { start: a2[0] || null };
      o.buttons = await ev(`const l = window.__p.byId(${JSON.stringify(l2)}); return l ? Array.from(l.view.containerEl.querySelectorAll('.nav-action-button, .clickable-icon')).filter((b) => b.offsetParent).map((b) => ({ aria: b.getAttribute('aria-label'), cls: String(b.className) })) : null;`);
      for (const a of arias) {
        const rc = await ev(`const l = window.__p.byId(${JSON.stringify(l2)}); const b = l && Array.from(l.view.containerEl.querySelectorAll('[aria-label]')).find((x) => x.getAttribute('aria-label') === ${JSON.stringify(a)} && x.offsetParent); if (!b) return null; const rc = b.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
        if (rc) { await c.clickAt(rc.x, rc.y); await sleep(600); }
      }
      o.after = await ev(`return window.__p.desc(window.__p.byId(${JSON.stringify(l2)}));`);
      ws = await flush();
      o.ws = rawLeaf(ws, l2);
      r[row] = o;
    }
    r.wsAll = await flush();
  });

  await step('s05-commands', async (r) => {
    for (const cmd of ['backlink:open-backlinks', 'backlink:open', 'outgoing-links:open-for-current', 'outgoing-links:open', 'outline:open-for-current', 'outline:open', 'properties:open-local', 'properties:open', 'graph:open-local']) {
      const id = await ev(`return await window.__p.reset('Alpha.md');`);
      await ev(`app.workspace.rightSplit.collapse(); return true;`);
      await sleep(300);
      const before = await ev(`return window.__p.ids();`);
      const o = { available: await ev(`return window.__p.avail(${JSON.stringify(cmd)});`) };
      o.ran = await ev(`return window.__p.run(${JSON.stringify(cmd)});`);
      await sleep(1500);
      o.added = (await snapNew(before)).map((d) => ({ id: d.id, type: d.type, state: d.state, group: d.group, where: d.where, isActive: d.isActive, tabTitle: d.tabTitle }));
      o.active = await ev(`const d = window.__p.desc(app.workspace.activeLeaf); return d && { type: d.type, where: d.where, group: d.group, state: d.state };`);
      o.note = await ev(`const l = window.__p.byId(${JSON.stringify(id)}); return l ? { group: l.group || null } : null;`);
      o.rightCollapsed = await ev(`return app.workspace.rightSplit.collapsed;`);
      o.rightActive = await ev(`return window.__p.rightHeaders().filter((h) => h.active).map((h) => h.dataType);`);
      o.focus = await ev(`return window.__p.focusLeaf();`);
      r[cmd] = o;
    }
    await ev(`app.workspace.rightSplit.expand(); return true;`);
  });

  await step('s07-row-clicks', async (r) => {
    // A row clicked in a linked view: which leaf opens the file.
    const cases = [
      ['Beta.md', 'Open backlinks', 'Gamma'],
      ['Alpha.md', 'Open outgoing links', 'Gamma'],
      ['Alpha.md', 'Open outline', 'Two'],
    ];
    for (const [file, row, label] of cases) {
      const id = await ev(`return await window.__p.reset(${JSON.stringify(file)});`);
      const before = await ev(`return window.__p.ids();`);
      await openSubmenu(id, 'more');
      await clickRow(row);
      const added = await snapNew(before);
      const lid = added[0] && added[0].id;
      const o = { start: added[0] ? added[0].state : null };
      const rc = await ev(`const l = window.__p.byId(${JSON.stringify(lid)}); const it = l && Array.from(l.view.containerEl.querySelectorAll('.tree-item-self, .search-result-file-title')).find((x) => x.offsetParent && x.textContent.trim().startsWith(${JSON.stringify(label)})); if (!it) return null; const b = it.getBoundingClientRect(); return { x: b.x + Math.min(60, b.width / 2), y: b.y + b.height / 2, text: it.textContent.trim() };`);
      o.target = rc;
      if (rc) { await c.clickAt(rc.x, rc.y); await sleep(1500); }
      o.after = await ev(`return window.__p.mainDesc().map((d) => ({ id: d.id, type: d.type, file: d.file, group: d.group, path: d.where.path, active: d.isActive, title: d.tabTitle }));`);
      o.noteCursor = await ev(`const l = window.__p.byId(${JSON.stringify(id)}); const e = l && l.view.editor; return e ? e.getCursor() : null;`);
      o.focus = await ev(`return window.__p.focusLeaf();`);
      r[row] = o;
    }
    // The command's unlinked leaf: what it follows.
    await ev(`await window.__p.reset('Alpha.md'); return true;`);
    await ev(`return window.__p.run('backlink:open-backlinks');`);
    await sleep(1200);
    await ev(`const l = window.__p.rootLeaves().find((x) => x.view.getViewType() === 'markdown'); app.workspace.setActiveLeaf(l, { focus: true }); await l.openFile(app.vault.getAbstractFileByPath('Beta.md')); await window.__p.sleep(1200); return true;`);
    r.commandLeafOnBeta = await ev(`return window.__p.mainDesc().map((d) => ({ type: d.type, state: d.state.state, group: d.group, title: d.tabTitle }));`);
    await ev(`const l = app.workspace.getLeaf('tab'); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(1200); return true;`);
    r.commandLeafOnEmpty = await ev(`return window.__p.mainDesc().map((d) => ({ type: d.type, state: d.state.state, group: d.group, title: d.tabTitle }));`);
    r.wsOnEmpty = await flush();
  });

  await step('s06-other-kinds', async (r) => {
    for (const f of ['Board.canvas', 'doc.pdf', 'pic.png', 'Table.base']) {
      const id = await ev(`return await window.__p.reset(${JSON.stringify(f)});`);
      await sleep(600);
      const m = await openSubmenu(id, 'more');
      const o = { submenu: m.submenu, rows: {} };
      await escape();
      for (const row of (m.submenu || []).map((s) => s.replace(/ \[.*$/, '').replace(/ \{.*$/, ''))) {
        if (row === '---') continue;
        await ev(`await window.__p.reset(${JSON.stringify(f)}); return true;`);
        const nid = await ev(`return window.__p.rootLeaves()[0].id;`);
        const before = await ev(`return window.__p.ids();`);
        await openSubmenu(nid, 'more');
        await clickRow(row);
        o.rows[row] = (await snapNew(before)).map((d) => ({ type: d.type, state: d.state, group: d.group, path: d.where.path, title: d.tabTitle, display: d.display, isActive: d.isActive, text: d.text.slice(0, 200) }));
        o.rows[row].noteGroup = await ev(`const l = window.__p.byId(${JSON.stringify(nid)}); return l ? l.group || null : null;`);
      }
      r[f] = o;
    }
    // Leave a set for the relaunch: Alpha with every row linked.
    const id = await ev(`return await window.__p.reset('Alpha.md');`);
    for (const row of ROWS.slice(1)) {
      await ev(`app.workspace.setActiveLeaf(window.__p.byId(${JSON.stringify(id)}), { focus: true }); await window.__p.sleep(300); return true;`);
      await openSubmenu(id, 'more');
      await clickRow(row);
    }
    r.beforeQuit = await ev(`return window.__p.mainDesc().map((d) => ({ id: d.id, type: d.type, state: d.state, group: d.group, path: d.where.path, title: d.tabTitle }));`);
    r.wsFinal = await flush();
    await c.quit();
  }, 300000);
}

if (phase === 'after-restart') {
  await step('r00-reloaded', async (r) => {
    await sleep(2500);
    r.title = await ev(`return document.title;`);
    r.main = await ev(`return window.__p.mainDesc().map((d) => ({ id: d.id, type: d.type, state: d.state, group: d.group, path: d.where.path, title: d.tabTitle, display: d.display, text: d.text.slice(0, 200) }));`);
    r.wsAtStart = readWs();
    // Following still works after the relaunch?
    await ev(`const l = window.__p.rootLeaves().find((x) => x.view.getViewType() === 'markdown'); await l.openFile(app.vault.getAbstractFileByPath('Beta.md')); await window.__p.sleep(1500); return true;`);
    r.afterBeta = await ev(`return window.__p.mainDesc().map((d) => ({ type: d.type, state: d.state, group: d.group, title: d.tabTitle }));`);
  });
  c.close();
  process.exit(0);
}
c.close();
process.exit(0);
