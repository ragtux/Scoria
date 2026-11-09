// The view header's bookmark action on Obsidian 1.15.0: whether it is drawn
// on a bookmarked and an unbookmarked note, its label, icon, fill and
// computed colours, where it sits among the header's actions, and what a
// click on it does (the modal it opens, the bytes it writes) — plus whether
// it follows a bookmark made or removed while the note is open.
// Phases: fixtures | run   (node view-header-bookmark.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
all.viewHeaderBookmark = all.viewHeaderBookmark || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.viewHeaderBookmark[phase] = out;
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
async function step(name, fn) {
  out.order.push(name);
  try { const v = await fn(); out.steps[name] = { ok: true, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, error: String(e && e.stack || e) }; }
  save();
}
const vp = (rel) => path.join(VAULT, rel);
const BM = '.obsidian/bookmarks.json';
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch { return null; } };

if (phase === 'fixtures') {
  fs.writeFileSync(vp('Marked.md'), '# Marked\n\nBookmarked note.\n');
  fs.writeFileSync(vp('Plain.md'), '# Plain\n\nNot bookmarked.\n');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'global-search': true, bookmarks: true, 'command-palette': true, switcher: true, outline: true, graph: true, backlink: true }, null, 2));
  fs.writeFileSync(vp(BM), JSON.stringify({ items: [{ type: 'file', ctime: 1792373000000, path: 'Marked.md' }] }, null, 2));
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
const HEADER = `
const leaf = app.workspace.getLeavesOfType('markdown')[0];
const header = leaf.view.containerEl.querySelector('.view-actions');
const rect = (el) => { const r = el.getBoundingClientRect(); return [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; };
const actions = Array.from(header.children).map(a => {
  const svg = a.querySelector('svg');
  const s = getComputedStyle(a);
  return { cls: a.className, label: a.getAttribute('aria-label'), icon: svg && svg.getAttribute('class'), rect: rect(a),
    display: s.display, color: s.color, bg: s.backgroundColor, opacity: s.opacity,
    svg: svg && { fill: getComputedStyle(svg).fill, stroke: getComputedStyle(svg).stroke, color: getComputedStyle(svg).color, opacity: getComputedStyle(svg).opacity, w: getComputedStyle(svg).width } };
});
`;
const header = () => c.evaluate(HEADER + `return actions;`);
const modal = () => c.evaluate(`const m = document.querySelector('.modal-container .modal'); if (!m) return null; return { title: (m.querySelector('.modal-title') || {}).textContent, text: m.innerText.slice(0, 400), buttons: Array.from(m.querySelectorAll('button')).map(b => b.textContent.trim()) };`);
const open = (p) => c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath(${JSON.stringify(p)})); return true;`);

await step('s01-marked', async () => { await open('Marked.md'); await sleep(800); return { actions: await header(), shot: await shot('01-marked') }; });
await step('s02-plain', async () => { await open('Plain.md'); await sleep(800); return { actions: await header(), shot: await shot('02-plain') }; });
await step('s03-hover', async () => {
  await open('Marked.md'); await sleep(600);
  const r = await c.evaluate(HEADER + `const a = Array.from(header.children).find(x => /bookmark/i.test((x.getAttribute('aria-label') || '') + (x.querySelector('svg') || {getAttribute: () => ''}).getAttribute('class'))); if (!a) return null; a.setAttribute('data-cdp', 'bm'); const b = a.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (!r) return { missing: true };
  await c.mouseMove(r.x, r.y); await sleep(1100);
  return { tooltip: await c.evaluate(`return Array.from(document.querySelectorAll('.tooltip')).map(t => t.textContent.trim());`), hovered: await c.evaluate(`const a = document.querySelector('[data-cdp="bm"]'); const s = getComputedStyle(a); return { bg: s.backgroundColor, color: s.color, svgFill: getComputedStyle(a.querySelector('svg')).fill };`), shot: await shot('03-hover') };
});
await step('s04-click', async () => {
  const before = bytes(BM);
  const r = await c.rect('[data-cdp="bm"]');
  if (!r) return { missing: true };
  await c.clickAt(r.cx, r.cy); await sleep(800);
  const o = { modal: await modal(), menu: await c.openMenuItems(), shot: await shot('04-click') };
  await c.pressKey('Escape'); await sleep(400);
  o.changed = bytes(BM) !== before;
  return o;
});
await step('s05-unbookmark-while-open', async () => {
  await c.evaluate(`await app.commands.executeCommandById('bookmarks:unbookmark-current-view'); return true;`); await sleep(800);
  return { actions: await header(), file: bytes(BM) };
});
await step('s06-bookmark-while-open', async () => {
  await c.evaluate(`app.commands.executeCommandById('bookmarks:bookmark-current-view'); return true;`); await sleep(700);
  const m = await modal();
  await c.pressKey('Enter'); await sleep(900);
  return { modal: m, actions: await header() };
});
c.close();
save();
process.exit(0);
