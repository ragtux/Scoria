// The Files pane while a drag is held, for look-alike parity (the
// 2026-10-25 explorer-dnd pass recorded what a drop does and which classes
// appear, not how they look): the dragged row, the drag ghost's markup and
// text, and the drop target's computed styles held over a folder row, over a
// file row inside a folder, over a file row at the root, over the space below
// the last row, and with a folder dragged — each with a screenshot (SHOTS).
// Nothing is dropped: every drag ends in dragCancel.
// Phases: fixtures | run   (node explorer-drag-look.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
all.explorerDragLook = all.explorerDragLook || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.explorerDragLook[phase] = out;
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
async function step(name, fn) {
  out.order.push(name);
  try { const v = await fn(); out.steps[name] = { ok: true, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, error: String(e && e.stack || e) }; }
  save();
}
const vp = (rel) => path.join(VAULT, rel);

if (phase === 'fixtures') {
  for (const d of ['Dest', 'Inbox', 'Inbox/Deep']) fs.mkdirSync(vp(d), { recursive: true });
  fs.writeFileSync(vp('Root.md'), '# Root\n');
  fs.writeFileSync(vp('Other.md'), '# Other\n');
  fs.writeFileSync(vp('Dest/Keep.md'), '# Keep\n');
  fs.writeFileSync(vp('Inbox/Item.md'), '# Item\n');
  fs.writeFileSync(vp('Inbox/Deep/Leaf.md'), '# Leaf\n');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'global-search': true, bookmarks: true, 'command-palette': true }, null, 2));
  save();
  process.exit(0);
}

const c = await connect();
const shot = async (name) => {
  if (!SHOTS) return null;
  fs.mkdirSync(SHOTS, { recursive: true });
  const r = await c.send('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  return name + '.png';
};
const LOOK = `
const cs = (el, props) => { const s = getComputedStyle(el); const o = {}; for (const p of props) o[p] = s.getPropertyValue(p); return o; };
const rect = (el) => { const r = el.getBoundingClientRect(); return [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; };
const P = ['background-color','color','border','border-radius','box-shadow','outline','opacity'];
const marked = Array.from(document.querySelectorAll('.is-being-dragged, .is-being-dragged-over, .is-selected')).map(e => ({ cls: e.className, rect: rect(e), style: cs(e, P),
  title: e.querySelector(':scope > .tree-item-self') && { cls: e.querySelector(':scope > .tree-item-self').className, style: cs(e.querySelector(':scope > .tree-item-self'), P), rect: rect(e.querySelector(':scope > .tree-item-self')) } }));
const ghost = document.querySelector('.drag-ghost');
return { marked, ghost: ghost && { html: ghost.outerHTML.slice(0, 1200), text: ghost.innerText, rect: rect(ghost), style: cs(ghost, ['background-color','color','padding','border-radius','box-shadow','font-size','width','max-width']),
  kids: Array.from(ghost.querySelectorAll('*')).map(k => ({ cls: k.className && (k.className.baseVal !== undefined ? k.className.baseVal : k.className), text: Array.from(k.childNodes).filter(n => n.nodeType === 3).map(n => n.textContent).join('').trim(), style: cs(k, ['color','font-size','font-weight','margin','padding','display','opacity']) })) } };`;
const row = (p) => c.rect(`.nav-file-title[data-path="${p}"], .nav-folder-title[data-path="${p}"]`);

await step('s00-open', async () => {
  await c.evaluate(`app.workspace.leftSplit.expand(); const l = app.workspace.getLeavesOfType('file-explorer')[0]; app.workspace.revealLeaf(l);
    for (const f of ['Dest', 'Inbox']) { const it = l.view.fileItems[f]; if (it && it.collapsed !== undefined && it.setCollapsed) it.setCollapsed(false); }
    await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Other.md')); return true;`);
  await sleep(800);
  return { shot: await shot('00-tree') };
});

async function hold(name, from, to, toY) {
  const a = await row(from);
  const b = to === 'below' ? await c.evaluate(`const b = document.querySelector('.nav-files-container').getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.bottom - 40, y: b.bottom - 40, h: 0 };`) : await row(to);
  if (!a || !b) return { missing: { a, b } };
  const y = toY === undefined ? b.cy : b.y + toY;
  const d = await c.dragOverOnly(a.cx, a.cy, b.cx, y);
  await sleep(500);
  const look = await c.evaluate(LOOK);
  const s = await shot(name);
  if (d.finish) await d.finish();
  await sleep(300);
  return { started: d.started, payload: d.data && d.data.items, look, shot: s };
}

await step('s01-file-over-folder', () => hold('01-file-over-folder', 'Root.md', 'Dest'));
await step('s02-file-over-file-in-folder', () => hold('02-file-over-file-in-folder', 'Root.md', 'Inbox/Item.md'));
await step('s03-file-over-root-file', () => hold('03-file-over-root-file', 'Inbox/Item.md', 'Other.md'));
await step('s04-file-below-rows', () => hold('04-file-below-rows', 'Inbox/Item.md', 'below'));
await step('s05-folder-over-folder', () => hold('05-folder-over-folder', 'Inbox/Deep', 'Dest'));
await step('s06-file-over-own-folder', () => hold('06-file-over-own-folder', 'Inbox/Item.md', 'Inbox'));
await step('s07-file-over-editor', async () => {
  const a = await row('Root.md');
  const b = await c.rect('.workspace-leaf.mod-active .cm-content, .workspace-split.mod-root .cm-content');
  if (!a || !b) return { missing: true };
  const d = await c.dragOverOnly(a.cx, a.cy, b.cx, b.y + 20);
  await sleep(500);
  const look = await c.evaluate(LOOK);
  const s = await shot('07-file-over-editor');
  if (d.finish) await d.finish();
  return { look, shot: s };
});
c.close();
save();
process.exit(0);
