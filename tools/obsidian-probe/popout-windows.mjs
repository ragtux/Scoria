// Popout windows, the thing every "Open in new window" row and the four
// `-window` commands wait on: what Obsidian 1.15.0 opens, what the popout
// shows (ribbon? sidebars? which chrome), what `workspace.json` records for
// it while it is open and after it closes, and what each command does.
//
//   s01  `workspace:move-to-new-window` on the only tab: the floating split
//        tree, the popout's document (body classes, chrome present), its
//        window bounds and title, the main window's remaining layout, and
//        workspace.json once Obsidian has written it.
//   s02  `workspace:open-in-new-window` from the main window's tab: a copy
//        in a second popout, or the same popout?
//   s03  `editor:open-link-in-new-window` with the caret on a link.
//   s04  `workspace:new-window`: an empty window — what it holds.
//   s05  the popout's tab header menu, for the rows that differ from the
//        main window's (Move to new window, Open in new window).
//   s06  `workspace:close-window` with the popout's leaf active, then
//        `win.close()` on another: what workspace.json holds afterwards,
//        and where the tabs went.
//
// The popout's document is reached from the vault window through
// `app.workspace.floatingSplit.children[i].win` — Electron opens popouts
// with window.open, so the main renderer holds a reference — rather than
// through a second CDP target.
//
// Phases: fixtures | run   (node popout-windows.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 300000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.popoutWindows = all.popoutWindows || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.popoutWindows[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn) {
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), '# Alpha\n\nA link to [[Beta]] and one to [[Gamma]].\n');
  fs.writeFileSync(vp('Beta.md'), '# Beta\n\nBack to [[Alpha]].\n');
  fs.writeFileSync(vp('Gamma.md'), '# Gamma\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, backlink: true, 'command-palette': true, switcher: true,
  }, null, 2));
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
save();

await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  // The layout tree as Obsidian's own serializer sees it, trimmed: ids,
  // types, children, and the state a leaf carries.
  tree(item) {
    if (!item) return null;
    const o = { type: item.type };
    if (item.id) o.id = item.id;
    if (item.view) o.view = { type: item.view.getViewType(), file: item.view.file ? item.view.file.path : null };
    if (item.children) o.children = item.children.map((ch) => this.tree(ch));
    for (const k of ['direction', 'width', 'height', 'x', 'y', 'maximize', 'zoom', 'collapsed']) if (item[k] !== undefined) o[k] = item[k];
    if (item.win && item.win !== window) {
      const w = item.win, d = item.doc || w.document;
      o.win = {
        title: d.title,
        bodyClass: d.body.className,
        inner: [w.innerWidth, w.innerHeight], outer: [w.outerWidth, w.outerHeight], screen: [w.screenX, w.screenY],
        chrome: {
          ribbon: !!d.querySelector('.workspace-ribbon'),
          leftSplit: !!d.querySelector('.workspace-split.mod-left-split'),
          rightSplit: !!d.querySelector('.workspace-split.mod-right-split'),
          rootSplit: !!d.querySelector('.workspace-split.mod-root'),
          titlebar: !!d.querySelector('.titlebar'),
          statusBar: !!d.querySelector('.status-bar'),
          tabHeaders: Array.from(d.querySelectorAll('.workspace-tab-header')).map((t) => ({ label: t.getAttribute('aria-label'), type: t.getAttribute('data-type') })),
          tabContainerButtons: Array.from(d.querySelectorAll('.workspace-tab-header-tab-list, .workspace-tab-header-new-tab, .sidebar-toggle-button, .workspace-tab-header-container-inner')).map((b) => ({ cls: b.className, label: b.getAttribute('aria-label') })),
          rootChildren: Array.from(d.body.children).map((e) => e.tagName + '.' + e.className),
          appContainerChildren: Array.from((d.querySelector('.app-container') || { children: [] }).children).map((e) => e.className),
          workspaceChildren: Array.from((d.querySelector('.workspace') || { children: [] }).children).map((e) => e.className),
        },
      };
    }
    return o;
  },
  layout() {
    const ws = app.workspace;
    return {
      root: this.tree(ws.rootSplit),
      floating: this.tree(ws.floatingSplit),
      floatingCount: ws.floatingSplit ? ws.floatingSplit.children.length : 0,
      active: ws.activeLeaf ? { type: ws.activeLeaf.view.getViewType(), file: ws.activeLeaf.view.file ? ws.activeLeaf.view.file.path : null, popout: !!(ws.activeLeaf.getContainer && ws.activeLeaf.getContainer().win !== window) } : null,
      serialized: ws.getLayout(),
    };
  },
  popout(i) {
    const fs = app.workspace.floatingSplit;
    return fs && fs.children[i] ? fs.children[i] : null;
  },
  async open(file) {
    const f = app.vault.getAbstractFileByPath(file);
    const ls = app.workspace.getLeavesOfType('markdown').filter((l) => !(l.getContainer && l.getContainer().win !== window));
    const leaf = ls.length ? ls[0] : app.workspace.getLeaf(false);
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(300);
    return { file: leaf.view.file.path };
  },
  describeMenu(doc) {
    const m = (doc || document).querySelector('.menu');
    if (!m) return null;
    return Array.from(m.children).map((x) => x.classList.contains('menu-separator') ? '---' : ((x.querySelector('.menu-item-title') || x).textContent.trim()));
  },
};
return true;`);

const layout = () => c.evaluate(`return window.__p.layout();`);
const readCfg = (rel) => c.evaluate(`return await app.vault.adapter.read(${JSON.stringify(rel)});`);
const cmd = (id) => c.evaluate(`return app.commands.executeCommandById(${JSON.stringify(id)});`);
// Obsidian writes workspace.json on a debounce; wait it out, then read.
const workspaceJson = async () => { await sleep(2500); return readCfg('.obsidian/workspace.json'); };

await step('s00-context', async (r) => {
  r.opened = await c.evaluate(`app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse(); return await window.__p.open('Alpha.md');`);
  r.commands = await c.evaluate(`
    const ids = ['workspace:move-to-new-window', 'workspace:open-in-new-window', 'workspace:new-window', 'workspace:close-window', 'editor:open-link-in-new-window', 'window:toggle-always-on-top', 'window:zoom-in', 'window:zoom-out', 'window:reset-zoom'];
    const o = {}; for (const id of ids) { const c = app.commands.commands[id]; o[id] = c ? { name: c.name, hotkeys: (app.hotkeyManager.defaultKeys || {})[id] || null, check: !!(c.checkCallback || c.editorCheckCallback) } : null; } return o;`);
  r.before = await layout();
  r.workspaceJsonBefore = await workspaceJson();
  return r;
});

await step('s01-move-to-new-window', async (r) => {
  r.ran = await cmd('workspace:move-to-new-window');
  await sleep(1500);
  r.after = await layout();
  r.workspaceJson = await workspaceJson();
  r.mainActiveElement = await c.evaluate(`return document.activeElement ? document.activeElement.className : null;`);
  return r;
});

await step('s02-open-in-new-window-from-main', async (r) => {
  // The main window has an empty tab now (or none); open Beta there.
  r.opened = await c.evaluate(`return await window.__p.open('Beta.md');`);
  r.ran = await cmd('workspace:open-in-new-window');
  await sleep(1500);
  r.after = await layout();
  r.workspaceJson = await workspaceJson();
  return r;
});

await step('s03-open-link-in-new-window', async (r) => {
  r.opened = await c.evaluate(`return await window.__p.open('Alpha.md');`);
  r.caret = await c.evaluate(`
    const ed = app.workspace.activeLeaf.view.editor; ed.focus();
    const text = ed.getValue(); const ix = text.indexOf('[[Gamma]]') + 3;
    const pos = ed.offsetToPos(ix); ed.setCursor(pos); return pos;`);
  r.ran = await cmd('editor:open-link-in-new-window');
  await sleep(1500);
  r.after = await layout();
  r.workspaceJson = await workspaceJson();
  return r;
});

await step('s04-new-window', async (r) => {
  r.ran = await cmd('workspace:new-window');
  await sleep(1500);
  r.after = await layout();
  r.workspaceJson = await workspaceJson();
  return r;
});

await step('s05-popout-tab-menu', async (r) => {
  r.menu = await c.evaluate(`
    const p = window.__p.popout(0); if (!p) return null;
    const d = p.doc || p.win.document;
    const h = d.querySelector('.workspace-tab-header'); if (!h) return 'no tab header';
    const rc = h.getBoundingClientRect();
    h.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: rc.x + 10, clientY: rc.y + 10, view: p.win }));
    await window.__p.sleep(400);
    const items = window.__p.describeMenu(d) || window.__p.describeMenu(document);
    (d.querySelector('.menu') || document.querySelector('.menu') || { remove() {} }).remove();
    return items;`);
  r.mainMenu = await c.evaluate(`
    const h = document.querySelector('.mod-root .workspace-tab-header'); if (!h) return 'no tab header';
    const rc = h.getBoundingClientRect();
    h.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: rc.x + 10, clientY: rc.y + 10 }));
    await window.__p.sleep(400);
    const items = window.__p.describeMenu(document);
    (document.querySelector('.menu') || { remove() {} }).remove();
    return items;`);
  return r;
});

await step('s06-close-windows', async (r) => {
  r.before = await layout();
  // Activate the first popout's leaf and run close-window.
  r.activated = await c.evaluate(`
    const p = window.__p.popout(0); if (!p) return null;
    let leaf = null; app.workspace.iterateAllLeaves((l) => { if (!leaf && l.getContainer && l.getContainer() === p) leaf = l; });
    if (!leaf) return 'no leaf';
    app.workspace.setActiveLeaf(leaf, { focus: true }); await window.__p.sleep(200);
    return { type: leaf.view.getViewType(), file: leaf.view.file ? leaf.view.file.path : null };`);
  r.ranClose = await cmd('workspace:close-window');
  await sleep(1500);
  r.afterCloseCommand = await layout();
  // Close the rest with win.close().
  r.closedRest = await c.evaluate(`
    const fs = app.workspace.floatingSplit; const n = fs ? fs.children.length : 0;
    for (const ch of [...(fs ? fs.children : [])]) { try { ch.win.close(); } catch (e) {} }
    await window.__p.sleep(1200);
    return { had: n, left: app.workspace.floatingSplit ? app.workspace.floatingSplit.children.length : 0 };`);
  r.after = await layout();
  r.workspaceJson = await workspaceJson();
  return r;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
