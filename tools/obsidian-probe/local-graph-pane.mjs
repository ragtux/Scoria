// The local graph as a side pane: what `graph:open-local` makes and how the
// leaf lives ([[Development#Local graph pane, 2026-11-22]]).
//
//   s00  version; the commands offered with a note open
//   s01  graph:open-local from a note: which split and tab group the leaf
//        lands in, its index, whether it is active and the dock expanded,
//        its view state, display text, icon, the tab header's label and icon,
//        and the leaf's bytes in workspace.json
//   s02  graph:open-local a second time (from the note, and with the dock
//        collapsed): a new leaf or the same one revealed
//   s03  the linked pane following its note's leaf: another note in that
//        leaf, an unlinked tab, a canvas, an image, the linked leaf closed
//   s04  a localgraph leaf in the right sidebar (a real tab drag first, then
//        one made through the API): its bytes, and following the active
//        file over a note, a canvas, an image, an empty tab, no leaf at all,
//        itself active; graph:open-local with it open
//   s05  both tabs' right-click menus, their view headers and More options,
//        the graph controls' buttons, the note's own More options
//   s06  Ctrl+G with either pane open, from the note and from the pane
//   s07  a real click and a Ctrl+click on a node of either pane
//   s08  the sidebar pane's own options: the Filters rows under real clicks,
//        the Depth slider, what workspace.json holds afterwards; then quit
//   after-restart: the leaves and their options as reloaded
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
// Every localgraph leaf in a workspace.json text, with its place.
function localLeaves(text) {
  if (!text) return [];
  const found = [];
  const walk = (node, where) => {
    if (!node || typeof node !== 'object') return;
    if (node.type === 'leaf' && node.state && node.state.type === 'localgraph') found.push({ where, leaf: node });
    (node.children || []).forEach((ch, i) => walk(ch, where + '/' + (node.type || '?') + '[' + i + ']'));
  };
  const j = JSON.parse(text);
  for (const k of ['main', 'left', 'right']) walk(j[k], k);
  return { found, active: j.active, lastOpenFiles: j.lastOpenFiles };
}
// The raw text of the localgraph leaf object as written, by brace matching.
function rawLeafText(text) {
  if (!text) return null;
  const at = text.indexOf('"type": "localgraph"');
  if (at < 0) return null;
  let depth = 0, start = at;
  // Walk back to the `{` that opens the leaf (two levels up: state, leaf).
  let need = 2;
  for (let i = at; i >= 0; i--) {
    if (text[i] === '}') depth++;
    if (text[i] === '{') { if (depth === 0) { need--; if (need === 0) { start = i; break; } } else depth--; }
  }
  depth = 0;
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
  fs.writeFileSync(vp('Delta.md'), '# Delta\n\n[[Epsilon]]\n');
  fs.writeFileSync(vp('Epsilon.md'), '# Epsilon\n');
  fs.writeFileSync(vp('Lonely.md'), '# Lonely\n');
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
    return { split, indexInGroup: g && g.children ? g.children.indexOf(l) : null, groupSize: g && g.children ? g.children.length : null,
      groupType: g && g.type, groupCurrentTab: g && g.currentTab,
      groupTypes: g && g.children ? g.children.map((x) => x.view.getViewType()) : null,
      groupIndexInSplit: g && g.parent && g.parent.children ? g.parent.children.indexOf(g) : null };
  },
  desc(l) {
    if (!l) return null; const v = l.view;
    return { type: v.getViewType(), file: v.file ? v.file.path : null, display: v.getDisplayText ? v.getDisplayText() : null,
      icon: v.getIcon ? v.getIcon() : null, pinned: !!l.pinned, group: l.group || null, where: this.where(l),
      state: JSON.parse(JSON.stringify(l.getViewState())), isActive: l === app.workspace.activeLeaf };
  },
  local() { return app.workspace.getLeavesOfType('localgraph'); },
  localDesc() { return this.local().map((l) => this.desc(l)); },
  pick(which) { const ls = this.local(); if (!which) return ls[0]; const root = which === 'right' ? app.workspace.rightSplit : app.workspace.rootSplit; return ls.find((l) => l.getRoot() === root); },
  view(which) {
    const l = this.pick(which); if (!l) return null; const v = l.view; const el = v.containerEl;
    const r = v.renderer;
    return {
      headerTitle: (el.querySelector('.view-header-title') || {}).textContent || null,
      empty: (el.querySelector('.pane-empty, .graph-empty, .empty-state') || {}).textContent || null,
      text: el.innerText.slice(0, 300),
      nodes: r && r.nodes ? r.nodes.map((n) => n.id).sort() : null,
      links: r && r.links ? r.links.map((k) => k.source.id + '->' + k.target.id).sort() : null,
      visible: !!el.offsetParent,
    };
  },
  dockTab(which) {
    const l = this.pick(which); if (!l) return null;
    const h = l.tabHeaderEl; if (!h) return null;
    const r = h.getBoundingClientRect();
    const svg = h.querySelector('svg');
    return { aria: h.getAttribute('aria-label'), dataType: h.getAttribute('data-type'), cls: h.className,
      innerTitle: (h.querySelector('.workspace-tab-header-inner-title') || {}).textContent || null,
      icon: svg ? svg.getAttribute('class') : null, rect: { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2 },
      isActiveHeader: h.classList.contains('is-active') };
  },
  rightHeaders() {
    return Array.from(app.workspace.rightSplit.containerEl.querySelectorAll('.workspace-tab-header')).map((h) => ({ aria: h.getAttribute('aria-label'), dataType: h.getAttribute('data-type'), active: h.classList.contains('is-active') }));
  },
  rootLeaves() { const out = []; app.workspace.iterateAllLeaves((l) => { if (l.getRoot() === app.workspace.rootSplit) out.push(l); }); return out; },
  mainDesc() { return this.rootLeaves().map((l) => ({ type: l.view.getViewType(), file: l.view.file ? l.view.file.path : null, active: l === app.workspace.activeLeaf, group: app.workspace.rootSplit.children.indexOf(l.parent) })); },
  menu() {
    const m = document.querySelector('.menu'); if (!m) return null;
    return Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => {
      if (x.classList.contains('menu-separator')) return '---';
      const svg = x.querySelector('.menu-item-icon svg');
      return ((x.querySelector('.menu-item-title') || x).textContent || '').trim() + (svg ? ' [' + svg.getAttribute('class') + ']' : '') + (x.classList.contains('is-disabled') ? ' (disabled)' : '');
    });
  },
  closeMenus() { document.querySelectorAll('.menu').forEach((m) => m.remove()); },
};
return true;`);
const run = (id) => ev(`return window.__p.run(${JSON.stringify(id)});`);
const openIn = (file, how = false) => ev(`const l = app.workspace.getLeaf(${JSON.stringify(how)}); ${file ? `await l.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(file)}));` : ''} app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(800); return true;`);
const snap = async () => ({
  leaves: await ev(`return window.__p.localDesc();`),
  view: await ev(`return window.__p.view('main');`),
  sideView: await ev(`return window.__p.view('right');`),
  active: await ev(`return window.__p.desc(app.workspace.activeLeaf);`),
  main: await ev(`return window.__p.mainDesc();`),
  rightCollapsed: await ev(`return app.workspace.rightSplit.collapsed;`),
});
const flush = async () => { await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`); await sleep(3000); return readWs(); };

if (phase === 'run') {
  await step('s00-meta', async (r) => {
    r.title = await ev(`return document.title;`);
    r.apiVersion = await ev(`return require('obsidian') ? null : null;`).catch(() => null);
    r.appVersion = await ev(`try { return require('electron').ipcRenderer.sendSync('version'); } catch (e) { return String(e); }`);
    await openIn('Alpha.md');
    r.commands = await ev(`return app.commands.listCommands().filter((k) => /graph/.test(k.id)).map((k) => ({ id: k.id, name: k.name, hotkeys: (app.hotkeyManager.getDefaultHotkeys(k.id) || []).map((h) => h.modifiers.join('+') + '+' + h.key) }));`);
    r.offeredOn = {};
    for (const f of ['Board.canvas', 'pic.png', 'Alpha.md']) {
      await openIn(f);
      r.offeredOn[f] = await ev(`return window.__p.avail('graph:open-local');`);
    }
    r.wsBefore = await flush();
    r.rightHeadersBefore = await ev(`return window.__p.rightHeaders();`);
  });

  await step('s01-open-local', async (r) => {
    r.ran = await run('graph:open-local');
    await sleep(1500);
    Object.assign(r, await snap());
    r.dockTab = await ev(`return window.__p.dockTab();`);
    r.rightHeaders = await ev(`return window.__p.rightHeaders();`);
    r.focusInPane = await ev(`const l = window.__p.local()[0]; return l ? l.containerEl.contains(document.activeElement) : null;`);
    const ws = await flush();
    r.ws = localLeaves(ws);
    r.wsRawLeaf = rawLeafText(ws);
    r.wsRight = ws ? JSON.parse(ws).right : null;
  });

  await step('s02-open-again', async (r) => {
    await openIn('Alpha.md');
    r.again = await run('graph:open-local');
    await sleep(1000);
    r.afterAgain = await snap();
    await ev(`app.workspace.rightSplit.collapse(); await window.__p.sleep(500); return true;`);
    await openIn('Alpha.md');
    r.collapsedRun = await run('graph:open-local');
    await sleep(1000);
    r.afterCollapsed = await snap();
    const ws = await flush();
    r.wsMain = ws ? JSON.parse(ws).main : null;
    // Keep the first linked leaf only.
    await ev(`const ls = window.__p.local(); ls.slice(1).forEach((l) => l.detach()); await window.__p.sleep(500); return true;`);
    r.kept = await snap();
  });

  await step('s03-follow-linked', async (r) => {
    // The note's own leaf moves on: does the linked pane follow?
    await ev(`const l = app.workspace.getLeavesOfType('markdown')[0]; app.workspace.setActiveLeaf(l, { focus: true }); await l.openFile(app.vault.getAbstractFileByPath('Beta.md')); await window.__p.sleep(1000); return true;`);
    r.linkedLeafToBeta = await snap();
    // A second, unlinked tab active on another note.
    await openIn('Delta.md', 'tab');
    r.unlinkedTabDelta = await snap();
    // The linked leaf on a canvas.
    await ev(`const l = window.__p.rootLeaves().find((x) => x.group && x.view.getViewType() !== 'localgraph'); if (l) { await l.openFile(app.vault.getAbstractFileByPath('Board.canvas')); app.workspace.setActiveLeaf(l, { focus: true }); } await window.__p.sleep(1000); return !!l;`);
    r.linkedCanvas = await snap();
    r.wsCanvas = localLeaves(await flush());
    // The linked leaf on an image.
    await ev(`const l = window.__p.rootLeaves().find((x) => x.group && x.view.getViewType() !== 'localgraph'); if (l) { await l.openFile(app.vault.getAbstractFileByPath('pic.png')); app.workspace.setActiveLeaf(l, { focus: true }); } await window.__p.sleep(1000); return !!l;`);
    r.linkedImage = await snap();
    // The linked leaf closed: what does the pane show?
    await ev(`const l = window.__p.rootLeaves().find((x) => x.group && x.view.getViewType() !== 'localgraph'); if (l) l.detach(); await window.__p.sleep(1000); return !!l;`);
    r.linkedClosed = await snap();
    r.wsLinkedClosed = localLeaves(await flush());
    await ev(`app.workspace.detachLeavesOfType('localgraph'); for (const l of window.__p.rootLeaves()) { l.setPinned(false); l.detach(); } await window.__p.sleep(600); return true;`);
  });

  await step('s04-sidebar-leaf', async (r) => {
    await openIn('Alpha.md');
    // A real drag of the linked pane's tab header into the right sidebar's strip, first.
    await run('graph:open-local');
    await sleep(1200);
    await ev(`app.workspace.rightSplit.expand(); await window.__p.sleep(500); return true;`);
    const from = await ev(`return window.__p.dockTab('main');`);
    const to = await ev(`const h = app.workspace.rightSplit.containerEl.querySelector('.workspace-tab-header-container'); if (!h) return null; const rc = h.getBoundingClientRect(); return { x: rc.x + rc.width - 20, y: rc.y + rc.height / 2 };`);
    r.dragFrom = from; r.dragTo = to;
    if (from && to) r.drag = await c.dragAndDrop(from.rect.cx, from.rect.cy, to.x, to.y);
    await sleep(1200);
    r.afterDrag = await snap();
    r.droppedInRight = await ev(`return !!window.__p.pick('right');`);
    if (r.droppedInRight) {
      const ws = await flush();
      r.wsAfterDrag = localLeaves(ws);
      r.wsAfterDragRaw = rawLeafText(ws);
    }
    // A clean sidebar leaf for the rest: made through the API on the file the drag would carry.
    await ev(`app.workspace.detachLeavesOfType('localgraph'); await window.__p.sleep(300); const l = app.workspace.getRightLeaf(false); await l.setViewState({ type: 'localgraph', state: { file: 'Alpha.md' }, active: false }); app.workspace.revealLeaf(l); await window.__p.sleep(1200); return true;`);
    r.made = await snap();
    r.sideTab = await ev(`return window.__p.dockTab('right');`);
    r.rightHeaders = await ev(`return window.__p.rightHeaders();`);
    const ws = await flush();
    r.ws = localLeaves(ws);
    r.wsRawLeaf = rawLeafText(ws);
    // Following the active file from the sidebar.
    await openIn('Beta.md');
    r.sideBeta = await snap();
    r.wsSideBeta = localLeaves(await flush());
    await openIn('Board.canvas');
    r.sideCanvas = await snap();
    await openIn('pic.png');
    r.sideImage = await snap();
    await openIn('Gamma.md');
    await ev(`const l = app.workspace.getLeaf('tab'); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(1000); return true;`);
    r.sideEmptyTab = await snap();
    await ev(`for (const l of window.__p.rootLeaves()) { l.setPinned(false); l.detach(); } await window.__p.sleep(1000); return true;`);
    r.sideNoMain = await snap();
    const wsNo = await flush();
    r.wsSideNoMain = localLeaves(wsNo);
    r.wsSideNoMainRaw = rawLeafText(wsNo);
    await openIn('Delta.md');
    r.sideAfterReopen = await snap();
    // The pane made active itself.
    await ev(`const l = window.__p.pick('right'); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(800); return true;`);
    r.sideActive = await snap();
    // graph:open-local with the sidebar pane open.
    await openIn('Alpha.md');
    r.openLocalWithSide = await run('graph:open-local');
    await sleep(1200);
    r.afterOpenLocalWithSide = await snap();
    await ev(`const l = window.__p.pick('main'); if (l) l.detach(); await window.__p.sleep(500); return true;`);
  });

  await step('s05-tab-menus', async (r) => {
    for (const which of ['right', 'main']) {
      if (which === 'main') { await openIn('Alpha.md'); await run('graph:open-local'); await sleep(1200); }
      await ev(`const l = window.__p.pick(${JSON.stringify(which)}); if (l) app.workspace.revealLeaf(l); await window.__p.sleep(500); return true;`);
      const t = await ev(`return window.__p.dockTab(${JSON.stringify(which)});`);
      r[which + 'Tab'] = t;
      if (t) {
        await c.clickAt(t.rect.cx, t.rect.cy, { button: 'right' });
        await sleep(600);
        r[which + 'Menu'] = await ev(`return window.__p.menu();`);
        await c.pressKey('Escape');
        await sleep(300);
        await ev(`window.__p.closeMenus(); return true;`);
      }
      r[which + 'Header'] = await ev(`const l = window.__p.pick(${JSON.stringify(which)}); if (!l) return null; const h = l.containerEl.querySelector('.view-header'); return h ? { display: getComputedStyle(h).display, title: (h.querySelector('.view-header-title') || {}).textContent, actions: Array.from(h.querySelectorAll('.view-action')).map((b) => b.getAttribute('aria-label')) } : null;`);
      const more = await ev(`const l = window.__p.pick(${JSON.stringify(which)}); if (!l) return null; const b = l.containerEl.querySelector('.view-action[aria-label="More options"]'); if (!b || !b.offsetParent) return null; const rc = b.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
      if (more) {
        await c.clickAt(more.x, more.y);
        await sleep(600);
        r[which + 'MoreOptions'] = await ev(`return window.__p.menu();`);
        await c.pressKey('Escape');
        await ev(`window.__p.closeMenus(); return true;`);
      }
      r[which + 'Controls'] = await ev(`const l = window.__p.pick(${JSON.stringify(which)}); if (!l) return null; return Array.from(l.containerEl.querySelectorAll('.graph-controls .clickable-icon')).map((b) => ({ aria: b.getAttribute('aria-label'), cls: String(b.className), visible: !!b.offsetParent }));`);
    }
    // The note's own pane menu: is there an "Open linked view" with the local graph?
    const pm = await ev(`const l = app.workspace.getLeavesOfType('markdown')[0]; if (!l) return null; app.workspace.setActiveLeaf(l, { focus: true }); const b = l.view.containerEl.querySelector('.view-action[aria-label="More options"]'); if (!b) return null; const rc = b.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
    if (pm) {
      await c.clickAt(pm.x, pm.y);
      await sleep(600);
      r.noteMoreOptions = await ev(`return window.__p.menu();`);
      const lv = await ev(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((x) => /Open linked view/.test(x.textContent)); if (!it) return null; const rc = it.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
      if (lv) {
        await c.mouseMove(lv.x, lv.y);
        await sleep(700);
        r.linkedViewSubmenu = await ev(`const ms = Array.from(document.querySelectorAll('.menu')); const m = ms[ms.length - 1]; return ms.length > 1 ? Array.from(m.querySelectorAll('.menu-item')).map((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim()) : null;`);
      }
      await c.pressKey('Escape');
      await ev(`window.__p.closeMenus(); return true;`);
    }
    await ev(`const l = window.__p.pick('main'); if (l) l.detach(); await window.__p.sleep(500); return true;`);
  });

  await step('s06-ctrl-g', async (r) => {
    for (const where of ['side', 'linked']) {
      await ev(`app.workspace.detachLeavesOfType('graph'); for (const l of window.__p.rootLeaves()) { if (l.view.getViewType() !== 'localgraph') { l.setPinned(false); l.detach(); } } return true;`);
      await openIn('Alpha.md');
      if (where === 'linked') { await run('graph:open-local'); await sleep(1200); await ev(`const l = app.workspace.getLeavesOfType('markdown')[0]; app.workspace.setActiveLeaf(l, { focus: true }); l.view.editor && l.view.editor.focus(); return true;`); }
      await sleep(400);
      r[where + 'Before'] = await snap();
      await c.pressKey('g', { ctrl: true });
      await sleep(1500);
      r[where + 'AfterCtrlG'] = await snap();
      r[where + 'Global'] = await ev(`return app.workspace.getLeavesOfType('graph').map((l) => window.__p.desc(l).where);`);
      // From the local pane, active.
      await ev(`app.workspace.detachLeavesOfType('graph'); await window.__p.sleep(400); return true;`);
      await openIn('Alpha.md');
      await ev(`const l = window.__p.pick(${JSON.stringify(where === 'side' ? 'right' : 'main')}); if (l) app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(500); return true;`);
      await c.pressKey('g', { ctrl: true });
      await sleep(1500);
      r[where + 'FromPane'] = await snap();
      r[where + 'GlobalFromPane'] = await ev(`return app.workspace.getLeavesOfType('graph').map((l) => window.__p.desc(l).where);`);
      await ev(`app.workspace.detachLeavesOfType('graph'); await window.__p.sleep(400); return true;`);
    }
    await ev(`const l = window.__p.pick('main'); if (l) l.detach(); return true;`);
  });

  const nodePoint = (which, id) => ev(`
    const v = window.__p.pick(${JSON.stringify(which)}).view; const rd = v.renderer;
    const n = rd.nodes.find((x) => x.id === ${JSON.stringify(id)}); if (!n) return null;
    const el = rd.interactiveEl || v.containerEl.querySelector('canvas');
    const rc = el.getBoundingClientRect(); const dpr = window.devicePixelRatio || 1;
    let g = null; try { const p = n.circle.getGlobalPosition(); g = { x: p.x / dpr + rc.x, y: p.y / dpr + rc.y }; } catch (e) {}
    return g || { x: (n.x * rd.scale + rd.panX) / dpr + rc.x, y: (n.y * rd.scale + rd.panY) / dpr + rc.y };`);

  await step('s07-node-click', async (r) => {
    for (const which of ['right', 'main']) {
      await ev(`for (const l of window.__p.rootLeaves()) { if (l.view.getViewType() !== 'localgraph') { l.setPinned(false); l.detach(); } } return true;`);
      await openIn('Alpha.md');
      if (which === 'main') { await run('graph:open-local'); await sleep(1200); }
      await ev(`const l = window.__p.pick(${JSON.stringify(which)}); app.workspace.revealLeaf(l); await window.__p.sleep(3000); return true;`);
      r[which + 'Before'] = await snap();
      const pt = await nodePoint(which, 'Gamma.md');
      r[which + 'Point'] = pt;
      if (!pt) continue;
      await c.mouseMove(pt.x, pt.y);
      await sleep(500);
      r[which + 'Hovered'] = await ev(`const rd = window.__p.pick(${JSON.stringify(which)}).view.renderer; return rd.highlightNode ? rd.highlightNode.id : null;`);
      await c.clickAt(pt.x, pt.y);
      await sleep(1500);
      r[which + 'AfterClick'] = await snap();
      // Ctrl+click on Beta.
      await ev(`const l = window.__p.pick(${JSON.stringify(which)}); app.workspace.revealLeaf(l); await window.__p.sleep(2500); return true;`);
      const p2 = await nodePoint(which, 'Beta.md');
      if (p2) {
        await c.mouseMove(p2.x, p2.y); await sleep(400);
        await c.clickAt(p2.x, p2.y, { mods: { ctrl: true } });
        await sleep(1500);
        r[which + 'AfterCtrlClick'] = await snap();
      }
    }
    await ev(`const l = window.__p.pick('main'); if (l) l.detach(); return true;`);
  });

  await step('s08-options', async (r) => {
    await ev(`for (const l of window.__p.rootLeaves()) { l.setPinned(false); l.detach(); } return true;`);
    await openIn('Alpha.md');
    await ev(`const l = window.__p.pick('right'); app.workspace.revealLeaf(l); await window.__p.sleep(1500); return true;`);
    r.stateBefore = await ev(`return window.__p.pick('right').getViewState();`);
    r.controls = await ev(`
      const el = window.__p.pick('right').view.containerEl;
      const gc = el.querySelector('.graph-controls');
      if (!gc) return null;
      const closed = gc.classList.contains('is-close');
      if (closed) { const b = gc.querySelector('.graph-controls-button.mod-open'); if (b) b.click(); await window.__p.sleep(400); }
      const filt = Array.from(gc.querySelectorAll('.tree-item.graph-control-section')).find((s) => /Filters/.test(s.textContent));
      if (filt && filt.classList.contains('is-collapsed')) { filt.querySelector('.tree-item-self').click(); await window.__p.sleep(400); }
      return { wasClosed: closed, rows: Array.from(gc.querySelectorAll('.setting-item')).filter((s) => s.offsetParent).map((s) => ({ name: (s.querySelector('.setting-item-name') || {}).textContent, control: s.querySelector('.checkbox-container') ? 'toggle' : s.querySelector('input[type=range]') ? 'slider' : s.querySelector('input') ? 'input' : null, on: s.querySelector('.checkbox-container') ? s.querySelector('.checkbox-container').classList.contains('is-enabled') : null, value: s.querySelector('input[type=range]') ? s.querySelector('input[type=range]').value : null, min: s.querySelector('input[type=range]') ? s.querySelector('input[type=range]').min : null, max: s.querySelector('input[type=range]') ? s.querySelector('input[type=range]').max : null, step: s.querySelector('input[type=range]') ? s.querySelector('input[type=range]').step : null })) };`);
    const toggleRect = (name) => ev(`
      const gc = window.__p.pick('right').view.containerEl.querySelector('.graph-controls');
      const s = Array.from(gc.querySelectorAll('.setting-item')).find((x) => (x.querySelector('.setting-item-name') || {}).textContent === ${JSON.stringify(name)});
      if (!s) return null; const b = s.querySelector('.checkbox-container'); b.scrollIntoView({ block: 'center' }); const rc = b.getBoundingClientRect(); return { x: rc.x + rc.width / 2, y: rc.y + rc.height / 2 };`);
    for (const name of ['Neighbor links', 'Incoming links']) {
      const t = await toggleRect(name);
      if (t) { await c.clickAt(t.x, t.y); await sleep(500); }
    }
    r.slider = await ev(`
      const gc = window.__p.pick('right').view.containerEl.querySelector('.graph-controls');
      const s = Array.from(gc.querySelectorAll('.setting-item')).find((x) => (x.querySelector('.setting-item-name') || {}).textContent === 'Depth');
      if (!s) return null; const i = s.querySelector('input[type=range]'); i.scrollIntoView({ block: 'center' }); const rc = i.getBoundingClientRect();
      return { x: rc.x, y: rc.y + rc.height / 2, w: rc.width, value: i.value, min: i.min, max: i.max, step: i.step };`);
    if (r.slider) {
      const lo = Number(r.slider.min), hi = Number(r.slider.max);
      const x0 = r.slider.x + r.slider.w * ((Number(r.slider.value) - lo) / (hi - lo));
      const x1 = r.slider.x + r.slider.w * ((3 - lo) / (hi - lo));
      await c.drag(x0, r.slider.y, x1, r.slider.y);
      await sleep(800);
    }
    r.stateAfter = await ev(`return window.__p.pick('right').getViewState();`);
    r.viewAfter = await ev(`return window.__p.view('right');`);
    const ws = await flush();
    r.ws = localLeaves(ws);
    r.wsRawLeaf = rawLeafText(ws);
    r.graphJson = (() => { try { return fs.readFileSync(vp('.obsidian/graph.json'), 'utf8'); } catch { return null; } })();
    await openIn('Beta.md');
    await sleep(800);
    r.stateOnBeta = await ev(`return window.__p.pick('right').getViewState();`);
    // A linked pane opened now: does it start from the sidebar pane's options?
    await run('graph:open-local');
    await sleep(1200);
    r.linkedStateAfterSideEdit = await ev(`const l = window.__p.pick('main'); return l ? l.getViewState() : null;`);
    await openIn('Alpha.md');
    await flush();
    r.wsFinal = readWs();
    await c.quit();
  }, 180000);
}

if (phase === 'after-restart') {
  await step('r00-reloaded', async (r) => {
    await sleep(2500);
    r.title = await ev(`return document.title;`);
    r.leaves = await ev(`return window.__p.localDesc();`);
    r.view = await ev(`return window.__p.view('main');`);
    r.sideView = await ev(`return window.__p.view('right');`);
    r.active = await ev(`return window.__p.desc(app.workspace.activeLeaf);`);
    r.sideTab = await ev(`return window.__p.dockTab('right');`);
    r.rightHeaders = await ev(`return window.__p.rightHeaders();`);
    r.wsAtStart = readWs();
    r.ws = localLeaves(r.wsAtStart);
  });
  c.close();
  process.exit(0);
}
c.close();
process.exit(0);
