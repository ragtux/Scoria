// Tab drags between windows, the last step of multi-window: what Obsidian
// does when a tab is dragged from the vault window into a popout, from a
// popout into the vault window, a popout's last tab out of it, and a tab
// dropped outside every window (see [[Development#Multi-window, the
// design, 2026-11-09]]).
//
// Setup: the vault window holds Alpha and Gamma in one group, a popout
// holds Beta.
//   s01  Alpha's tab from the vault window onto the popout's tab strip,
//        after Beta: the drop overlay while over it, the layout right
//        after the drop, and again once the source's drag has ended.
//   s02  Alpha (now in the popout) onto the vault window's content, near
//        its right edge: a split there, or a tab?
//   s03  Beta, the popout's last tab, onto the vault window's tab strip:
//        does the popout close?
//   s04  Gamma dragged in the vault window and let go outside it (a drag
//        cancelled off the page): a new popout, and where?
//
// A drag is started for real in the window it leaves (Input.setInterceptDrags
// hands its data back) and played into the window it enters with
// Input.dispatchDragEvent on that window's own page target, the way the
// compositor carries an OS drag across. The source's drag is then ended
// with a dragCancel, whose dragend says "none" where a real drop would say
// "move": the layout is read before and after it so the two are told apart.
//
// Phases: fixtures | run   (node drag-between-windows.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
all.dragBetweenWindows = all.dragBetweenWindows || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.dragBetweenWindows[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
async function step(name, fn) {
  out.order.push(name);
  const t = Date.now();
  const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v || {}) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
}
const vp = (rel) => path.join(VAULT, rel);

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  for (const n of ['Alpha', 'Beta', 'Gamma']) fs.writeFileSync(vp(n + '.md'), `# ${n}\n\nBody of ${n}.\n`);
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const main = await connect();
out.vaultPath = await main.vaultPath();
out.title = await main.evaluate('return document.title;');
await main.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  tree(item) {
    if (!item) return null;
    const o = { type: item.type };
    if (item.view) o.file = item.view.file ? item.view.file.path : item.view.getViewType();
    if (item.children) o.children = item.children.map((ch) => this.tree(ch));
    if (item.direction) o.direction = item.direction;
    if (item.win && item.win !== window) o.win = { inner: [item.win.innerWidth, item.win.innerHeight], screen: [item.win.screenX, item.win.screenY], title: item.doc.title };
    return o;
  },
  layout() {
    const ws = app.workspace, a = ws.activeLeaf;
    return { root: this.tree(ws.rootSplit), floating: this.tree(ws.floatingSplit),
      floatingCount: ws.floatingSplit ? ws.floatingSplit.children.length : 0,
      active: a ? { file: a.view.file ? a.view.file.path : null, popout: a.getContainer().win !== window } : null,
      mainScreen: [window.screenX, window.screenY, window.outerWidth, window.outerHeight] };
  },
};
return true;`);
const layout = () => main.evaluate('return window.__p.layout();');

// The popout's own page target, found by a mark set on its window.
async function popoutClient(index) {
  await main.evaluate(`app.workspace.floatingSplit.children[${index}].win.__probeMark = 'popout-${index}'; return true;`);
  for (const url of await pageTargets()) {
    const c = await connect(url);
    const mark = await c.evaluate('return window.__probeMark || null;').catch(() => null);
    if (mark === `popout-${index}`) return c;
    c.close();
  }
  throw new Error('no page target for popout ' + index);
}

// The centre of a tab header, the right end of a tab strip, or a point of
// a group's content, in the page's own coordinates.
const tabCentre = (c, label) => c.evaluate(`
  const t = Array.from(document.querySelectorAll('.workspace-tab-header')).find((e) => e.getAttribute('aria-label') === ${JSON.stringify(label)} && e.offsetParent);
  if (!t) return null; const r = t.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2];`);
const stripEnd = (c) => c.evaluate(`
  const s = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header-container')).find((e) => e.offsetParent);
  const r = s.getBoundingClientRect(); return [r.x + r.width * 0.6, r.y + r.height / 2];`);
const contentAt = (c, fx, fy) => c.evaluate(`
  const s = Array.from(document.querySelectorAll('.mod-root .workspace-tab-container')).find((e) => e.offsetParent);
  const r = s.getBoundingClientRect(); return [r.x + r.width * ${fx}, r.y + r.height * ${fy}];`);
// What the target page paints while a drag is over it.
const overlay = (c) => c.evaluate(`
  const pick = (sel) => Array.from(document.querySelectorAll(sel)).filter((e) => e.offsetParent || getComputedStyle(e).display !== 'none').map((e) => {
    const r = e.getBoundingClientRect(); return { cls: e.className, rect: [r.x, r.y, r.width, r.height].map(Math.round), display: getComputedStyle(e).display };
  });
  return { overlay: pick('.workspace-drop-overlay'), indicator: pick('.workspace-tab-header-container .drop-indicator, .workspace-drop-indicator'), dragOver: pick('.is-drag-over, .mod-drag-over').slice(0, 6), bodyClass: document.body.className };`);

// Start a drag on `from` at `p`; play it into `to` along to `q`; drop there
// (or cancel off the page when `q` is null); then end the source's drag.
async function crossDrag(from, p, to, q, rec) {
  await from.send('Input.setInterceptDrags', { enabled: true });
  const got = from.once('Input.dragIntercepted', 6000);
  await from.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(p[0]), y: Math.round(p[1]) });
  await from.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: Math.round(p[0]), y: Math.round(p[1]), button: 'left', clickCount: 1 });
  for (let i = 1; i <= 4; i++) await from.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(p[0] + i * 4), y: Math.round(p[1] + i * 3), button: 'left' });
  const { data } = await got;
  rec.dragData = { mimeTypes: data.items ? data.items.map((i) => i.mimeType) : null, items: data.items ? data.items.map((i) => ({ mime: i.mimeType, data: String(i.data).slice(0, 200) })) : null, dragOperationsMask: data.dragOperationsMask };
  rec.draggable = await main.evaluate(`const d = app.dragManager && app.dragManager.draggable; return d ? { type: d.type, title: d.title || null, file: d.file ? d.file.path : null } : null;`);
  const fd = (type, x, y) => from.send('Input.dispatchDragEvent', { type, x: Math.round(x), y: Math.round(y), data });
  await fd('dragEnter', p[0], p[1]);
  await fd('dragOver', p[0] + 20, p[1] + 10);
  await sleep(150);
  if (q && to !== from) {
    const td = (type, x, y) => to.send('Input.dispatchDragEvent', { type, x: Math.round(x), y: Math.round(y), data });
    await td('dragEnter', q[0] - 40, q[1]);
    for (let i = 1; i <= 6; i++) { await td('dragOver', q[0] - 40 + (40 * i) / 6, q[1]); await sleep(80); }
    rec.overTarget = await overlay(to);
    await td('drop', q[0], q[1]);
    await sleep(400);
    rec.afterDrop = await layout();
    // The source's window may be gone by now (its last tab left it).
    await Promise.race([fd('dragCancel', -300, -300), sleep(2000)]).catch((e) => { rec.sourceCancel = String(e); });
  } else if (q) {
    for (let i = 1; i <= 6; i++) { await fd('dragOver', p[0] + ((q[0] - p[0]) * i) / 6, p[1] + ((q[1] - p[1]) * i) / 6); await sleep(80); }
    rec.overTarget = await overlay(from);
    await fd('drop', q[0], q[1]);
  } else {
    // Off the page: CDP clamps nothing, so the drag goes over a point
    // left of the window and ends there.
    for (let i = 1; i <= 6; i++) { await fd('dragOver', p[0] - i * 80, p[1]); await sleep(80); }
    rec.overOffPage = await overlay(from);
    await fd('dragCancel', -400, p[1]);
  }
  await Promise.race([from.send('Input.setInterceptDrags', { enabled: false }), sleep(2000)]).catch(() => {});
  await Promise.race([from.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: -400, y: Math.round(p[1]), button: 'left', clickCount: 1 }), sleep(2000)]).catch(() => {});
  await sleep(1200);
  rec.after = await layout();
}

await step('s00-setup', async () => {
  await main.evaluate(`
    const f = (n) => app.vault.getAbstractFileByPath(n);
    const leaf = app.workspace.getLeaf(false); await leaf.openFile(f('Alpha.md'));
    const g = app.workspace.getLeaf('tab'); await g.openFile(f('Gamma.md'));
    const p = app.workspace.openPopoutLeaf(); await p.openFile(f('Beta.md'));
    await window.__p.sleep(1500); return true;`);
  return { layout: await layout() };
});

let pop = await popoutClient(0);
const only = (process.env.ONLY_STEPS || '').split(',').filter(Boolean);
await step('s01-main-tab-to-popout-strip', async (rec) => {
  const p = await tabCentre(main, 'Alpha');
  const q = await stripEnd(pop);
  rec.from = p; rec.to = q;
  await crossDrag(main, p, pop, q, rec);
  return rec;
});
await step('s02-popout-tab-to-main-right-edge', async (rec) => {
  const p = await tabCentre(pop, 'Alpha');
  const q = await contentAt(main, 0.95, 0.5);
  rec.from = p; rec.to = q;
  if (!p) throw new Error('Alpha is not in the popout');
  await crossDrag(pop, p, main, q, rec);
  return rec;
});
await step('s03-popout-last-tab-to-main-strip', async (rec) => {
  const p = await tabCentre(pop, 'Beta');
  const q = await stripEnd(main);
  rec.from = p; rec.to = q;
  rec.before = await layout();
  if (rec.before.floatingCount !== 1) throw new Error('expected one popout before s03');
  await crossDrag(pop, p, main, q, rec);
  return rec;
});
await step('s04-main-tab-dropped-outside', async (rec) => {
  const p = await tabCentre(main, 'Gamma');
  rec.from = p;
  rec.before = await layout();
  await crossDrag(main, p, null, null, rec);
  return rec;
});
// The same drag ended by a dragend synthesized on the dragged header with
// the pointer left of the window, in client and screen coordinates, the
// way a desktop's drop on nothing ends an OS drag.
await step('s05-synthetic-dragend-outside', async (rec) => {
  const p = await tabCentre(main, 'Beta');
  rec.from = p;
  rec.before = await layout();
  await main.send('Input.setInterceptDrags', { enabled: true });
  const got = main.once('Input.dragIntercepted', 6000);
  await main.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(p[0]), y: Math.round(p[1]) });
  await main.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: Math.round(p[0]), y: Math.round(p[1]), button: 'left', clickCount: 1 });
  for (let i = 1; i <= 4; i++) await main.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(p[0] + i * 4), y: Math.round(p[1] + i * 3), button: 'left' });
  await got;
  rec.dispatched = await main.evaluate(`
    const t = Array.from(document.querySelectorAll('.workspace-tab-header')).find((e) => e.getAttribute('aria-label') === 'Beta' && e.offsetParent);
    const dt = new DataTransfer(); try { dt.dropEffect = 'none'; } catch {}
    const ok = t.dispatchEvent(new DragEvent('dragend', { bubbles: true, cancelable: true, clientX: -300, clientY: 100, screenX: window.screenX - 300, screenY: window.screenY + 100, dataTransfer: dt }));
    return { ok, dropEffect: dt.dropEffect };`);
  await main.send('Input.setInterceptDrags', { enabled: false });
  await main.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: Math.round(p[0]), y: Math.round(p[1]), button: 'left', clickCount: 1 });
  await sleep(1500);
  rec.after = await layout();
  return rec;
});

// workspace.json once Obsidian has written it.
await sleep(2500);
out.workspaceJson = (() => { try { return JSON.parse(fs.readFileSync(vp('.obsidian/workspace.json'), 'utf8')); } catch (e) { return String(e); } })();
save();
pop.close?.();
main.close();
process.exit(0);
