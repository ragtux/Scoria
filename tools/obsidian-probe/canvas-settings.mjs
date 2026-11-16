// The Canvas plugin's settings (`canvas.json`) and what each does to an open
// canvas, for Scoria's canvas view: the two keys the 2026-10-30 pass could
// not reach (the folder row and the zoom threshold slider), where
// `canvas:new-file` puts its canvas under each location, what the wheel does
// under each wheel behavior and modifier, what "Show card names" does to a
// file card's name, which zoom the threshold slider moves, what a Ctrl+drag
// on the background does under each "Default Ctrl + Drag behavior", and what
// snapping to the grid and to objects snap to, at what distance, on which
// axis, with which guides, for a drag and a resize.
//
// Every option is set through the Settings tab's own controls, as a person
// would, and acted on in a canvas that stays open: none needed a reopen.
// Fixtures are reset through `app.vault.modify` (writing the file from the
// host while a canvas leaf saves races it into a file that does not parse).
// Run it in a headless sway with OBSIDIAN_ARGS=--ozone-platform=x11, like
// settings-gaps.mjs. ONLY_STEPS=s03,s08 runs a subset.
//
// Phases: fixtures | run   (node canvas-settings.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const only = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run'].includes(phase)) { console.error('phase must be fixtures|run'); process.exit(2); }

const T0 = Date.now();
const load = () => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } };
const all = load();
all.canvasSettings = all.canvasSettings || {};
const out = all.canvasSettings[phase] && only ? all.canvasSettings[phase] : { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.canvasSettings[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn) {
  if (only && !only.some((o) => name.startsWith(o))) return out.steps[name];
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  try { const v = await fn(); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' && !Array.isArray(v) ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e) }; }
  save();
  console.error(`[${name}] ${out.steps[name].ok ? 'ok' : 'FAIL'} ${out.steps[name].ms}ms`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
const canvases = () => {
  const o = [];
  const walk = (d) => { for (const e of fs.readdirSync(vp(d), { withFileTypes: true })) { if (e.name.startsWith('.')) continue; const p = d ? `${d}/${e.name}` : e.name; if (e.isDirectory()) walk(p); else if (p.endsWith('.canvas')) o.push(p); } };
  walk('');
  return o.sort();
};

// ---- fixtures -------------------------------------------------------------
const FIXTURE = {
  nodes: [
    { id: 'a', type: 'text', text: 'A', x: 0, y: 300, width: 100, height: 100 },
    { id: 'b', type: 'text', text: 'B', x: 400, y: 0, width: 200, height: 100 },
    { id: 'f', type: 'file', file: 'Plan.md', x: -400, y: 0, width: 300, height: 200 },
    { id: 'g', type: 'group', label: 'Grp', x: 900, y: 300, width: 300, height: 300 },
  ],
  edges: [],
};

if (phase === 'fixtures') {
  await step('fixtures', async () => {
    for (const d of ['.obsidian', 'Sub', 'Canvases']) fs.mkdirSync(vp(d), { recursive: true });
    fs.writeFileSync(vp('.obsidian/app.json'), '{}');
    fs.writeFileSync(vp('Plan.md'), '# Plan\n\nRoot note.\n');
    fs.writeFileSync(vp('Sub/Note.md'), '# Note\n\nIn Sub.\n');
    fs.writeFileSync(vp('Fixture.canvas'), JSON.stringify(FIXTURE, null, '\t'));
    return { vaultFiles: fs.readdirSync(VAULT).sort() };
  });
  process.exit(0);
}

// ---- run ------------------------------------------------------------------
const c = await connect();
const vault = await c.vaultPath();
if (vault.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault open: ' + vault); c.close(); process.exit(3); }
out.vault = vault;
out.title = await c.evaluate(`return document.title`);

const ev = (s) => c.evaluate(s);
const C = `const leaf = app.workspace.getLeavesOfType('canvas')[0]; if (!leaf) throw new Error('no canvas leaf'); const cv = leaf.view.canvas;`;
const options = () => ev(`return app.internalPlugins.plugins.canvas.instance.options`);

// The Canvas tab's rows by index: 0 location, 1 folder, 2 wheel, 3 Ctrl+drag,
// 4 card names, 5 snap to grid, 6 snap to objects, 7 zoom threshold.
async function setOpt(key, value) {
  const ROW = `const rows = app.setting.activeTab.containerEl.querySelectorAll('.setting-item');`;
  await ev(`app.setting.open(); app.setting.openTabById('canvas'); return true;`); await sleep(400);
  const select = { newFileLocation: 0, defaultWheelBehavior: 2, defaultModDragBehavior: 3, cardLabelVisibility: 4 }[key];
  if (select !== undefined) {
    await ev(`${ROW} const s = rows[${select}].querySelector('select'); s.value = ${JSON.stringify(value)}; s.dispatchEvent(new Event('change', { bubbles: true })); return true`);
  } else if (key === 'zoomBreakpoint') {
    await ev(`${ROW} const s = rows[7].querySelector('input[type=range]'); s.value = ${JSON.stringify(String(value))}; s.dispatchEvent(new Event('input', { bubbles: true })); s.dispatchEvent(new Event('change', { bubbles: true })); return true`);
  } else if (key === 'newFileFolderPath') {
    await ev(`${ROW} const i = rows[1].querySelector('input'); i.value = ${JSON.stringify(value)}; i.dispatchEvent(new Event('input', { bubbles: true })); return true`);
  } else {
    // A toggle's state is its container's `is-enabled`, not the input's `checked`.
    const ri = { snapToGrid: 5, snapToObjects: 6 }[key];
    await ev(`${ROW} const box = rows[${ri}].querySelector('.checkbox-container'); if (box.classList.contains('is-enabled') !== ${!!value}) box.click(); return true`);
  }
  await sleep(500);
  await ev(`app.setting.close(); return true`); await sleep(300);
  const o = await options();
  if (key !== 'zoomBreakpoint' && o[key] !== value) throw new Error(`setting ${key}=${value} left ${JSON.stringify(o)}`);
  return o;
}

async function openFixture(fixture = FIXTURE) {
  await ev(`for (const l of app.workspace.getLeavesOfType('canvas')) l.detach(); app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse(); return true`);
  await sleep(800);
  await ev(`await app.vault.modify(app.vault.getAbstractFileByPath('Fixture.canvas'), ${JSON.stringify(JSON.stringify(fixture, null, '\t'))}); return true`);
  await sleep(300);
  await ev(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Fixture.canvas')); return true`);
  await sleep(1200);
}
const view = () => ev(`${C} return { x: cv.x, y: cv.y, zoom: cv.zoom }`);
const setView = async (x, y, z) => { await ev(`${C} cv.deselectAll(); cv.setViewport(${x}, ${y}, ${z}); return true`); await sleep(400); };
const scr = (x, y) => ev(`${C} const r = cv.wrapperEl.getBoundingClientRect(); return { x: r.left + r.width / 2 + (${x} - cv.x) * cv.scale, y: r.top + r.height / 2 + (${y} - cv.y) * cv.scale };`);
const nodes = () => ev(`${C} return Array.from(cv.nodes.values()).map(n => { const d = n.getData(); return { id: n.id, x: n.x, y: n.y, w: n.width, h: n.height, type: d.type, text: d.text, file: d.file, label: d.label }; });`);
const node = async (id) => (await nodes()).find((n) => n.id === id);
const place = async (id, x, y, w, h) => { await ev(`${C} const n = cv.nodes.get(${JSON.stringify(id)}); n.moveAndResize({ x: ${x}, y: ${y}, width: ${w}, height: ${h} }); cv.requestSave(); return true`); await sleep(150); };
const M = (type, p, mods = 0, extra = {}) => c.send('Input.dispatchMouseEvent', { type, x: p.x, y: p.y, modifiers: mods, ...extra });
const GUIDES = `${C} const svg = cv.wrapperEl.querySelector('svg.canvas-snaps'); if (!svg) return null;
  return { lines: Array.from(svg.querySelectorAll('line')).map(l => [l.getAttribute('x1'), l.getAttribute('y1'), l.getAttribute('x2'), l.getAttribute('y2')].map(Number)),
    dots: Array.from(svg.querySelectorAll('circle')).map(e => { const b = e.getBoundingClientRect(); return [b.width, b.height]; }),
    stroke: svg.querySelector('line') ? getComputedStyle(svg.querySelector('line')).stroke : null,
    fill: svg.querySelector('circle') ? getComputedStyle(svg.querySelector('circle')).fill : null };`;
/** Press at p, move to q in eight steps (modifiers held throughout), read the guides, release. */
async function gesture(p, q, mods = 0) {
  await M('mouseMoved', { x: p.x - 30, y: p.y - 30 }); await sleep(60);
  await M('mouseMoved', p, mods); await sleep(150);
  await M('mousePressed', p, mods, { button: 'left', clickCount: 1 });
  for (let i = 1; i <= 8; i++) { await M('mouseMoved', { x: p.x + (q.x - p.x) * i / 8, y: p.y + (q.y - p.y) * i / 8 }, mods, { button: 'left', buttons: 1 }); await sleep(30); }
  await sleep(120);
  const guides = await ev(GUIDES);
  await M('mouseReleased', q, mods, { button: 'left', clickCount: 1 }); await sleep(250);
  return guides;
}
/** Drag card `id` by a canvas-space offset, grabbing it 20 units in from its top left. */
async function dragBy(id, dx, dy, mods = 0) {
  const n = await node(id); const s = await ev(`${C} return cv.scale`);
  const p = await scr(n.x + 20, n.y + 20);
  const guides = await gesture(p, { x: p.x + dx * s, y: p.y + dy * s }, mods);
  const m = await node(id);
  return { from: [n.x, n.y], to: [m.x, m.y], guides };
}
/** Drag one side or corner of card `id` by a canvas-space offset (the resizer appears under the pointer). */
async function resizeBy(id, side, d) {
  const n = await node(id); const s = await ev(`${C} return cv.scale`);
  const p = side === 'right' ? await scr(n.x + n.w, n.y + 25) : side === 'left' ? await scr(n.x, n.y + 25) : side === 'bottom' ? await scr(n.x + 25, n.y + n.h) : await scr(n.x + n.w, n.y + n.h);
  const q = side === 'bottom' ? { x: p.x, y: p.y + d * s } : side === 'br' ? { x: p.x + d * s, y: p.y + d * s } : { x: p.x + d * s, y: p.y };
  const guides = await gesture(p, q);
  const m = await node(id);
  return { rect: [m.x, m.y, m.w, m.h], guides };
}

await step('s01-tab-and-keys', async () => {
  const before = bytes('.obsidian/canvas.json');
  await ev(`app.setting.open(); app.setting.openTabById('canvas'); return true;`); await sleep(800);
  const rows = await ev(`return Array.from(app.setting.activeTab.containerEl.querySelectorAll('.setting-item')).map(s => ({
    name: s.querySelector('.setting-item-name')?.textContent, desc: s.querySelector('.setting-item-description')?.textContent || '', hidden: s.offsetParent === null,
    controls: Array.from(s.querySelectorAll('input,select')).filter(e => !e.classList.contains('is-measuring')).map(e => ({ tag: e.tagName, type: e.type, value: e.value, min: e.min, max: e.max, step: e.step, placeholder: e.placeholder, options: e.options ? Array.from(e.options).map(o => o.value) : undefined })) }));`);
  await ev(`app.setting.close(); return true`);
  await setOpt('zoomBreakpoint', 0.5);
  const afterSlider = bytes('.obsidian/canvas.json');
  await setOpt('newFileLocation', 'folder');
  await setOpt('newFileFolderPath', 'Canvases');
  const afterFolder = bytes('.obsidian/canvas.json');
  await setOpt('zoomBreakpoint', 2.4);
  const max = (await options()).zoomBreakpoint;
  await setOpt('zoomBreakpoint', 0);
  return { before, rows, afterSlider, afterFolder, sliderAt24: max, final: bytes('.obsidian/canvas.json') };
});

await step('s02-new-file-location', async () => {
  const res = {};
  const run = async (name, active) => {
    const before = canvases();
    if (active) { await ev(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath(${JSON.stringify(active)})); return true`); await sleep(600); }
    else { await ev(`app.workspace.getLeaf(false).setViewState({ type: 'empty' }); return true`); await sleep(400); }
    await ev(`app.commands.executeCommandById('canvas:new-file'); return true`); await sleep(1500);
    res[name] = { added: canvases().filter((x) => !before.includes(x)), active: await ev(`return app.workspace.getActiveFile()?.path`) };
  };
  await setOpt('newFileLocation', 'root'); await run('root, Sub/Note.md open', 'Sub/Note.md');
  await setOpt('newFileLocation', 'current'); await run('current, Sub/Note.md open', 'Sub/Note.md'); await run('current, Plan.md open', 'Plan.md'); await run('current, nothing open', null);
  await setOpt('newFileLocation', 'folder');
  for (const folder of ['Canvases', 'Nope/Deep', '', '/Canvases/']) { await setOpt('newFileFolderPath', folder); await run(`folder ${JSON.stringify(folder)}`, 'Sub/Note.md'); }
  res.nopeCreated = fs.existsSync(vp('Nope'));
  res.untitledBytes = bytes('Untitled.canvas');
  await setOpt('newFileLocation', 'root');
  return res;
});

await step('s03-wheel', async () => {
  const res = {};
  const wheel = async (dx, dy, mods) => {
    await setView(0, 0, 0);
    const p = await scr(0, 0); const at = { x: Math.round(p.x) + 300, y: Math.round(p.y) + 200 };
    const before = await view();
    await M('mouseMoved', at); await M('mouseWheel', at, mods, { deltaX: dx, deltaY: dy }); await sleep(1100);
    const after = await view();
    return { dx: +(after.x - before.x).toFixed(2), dy: +(after.y - before.y).toFixed(2), dzoom: +(after.zoom - before.zoom).toFixed(4) };
  };
  for (const mode of ['pan', 'zoom']) {
    await setOpt('defaultWheelBehavior', mode);
    await openFixture();
    res[mode] = {
      down: await wheel(0, 100, 0), up: await wheel(0, -100, 0), sideways: await wheel(100, 0, 0),
      ctrl: await wheel(0, 100, 2), ctrlUp: await wheel(0, -100, 2), ctrlSideways: await wheel(100, 0, 2),
      shift: await wheel(0, 100, 8), ctrlShift: await wheel(0, 100, 10), alt: await wheel(0, 100, 1), meta: await wheel(0, 100, 4),
    };
  }
  await setOpt('defaultWheelBehavior', 'pan');
  return res;
});

await step('s04-card-names', async () => {
  const res = {};
  const read = () => ev(`${C} const o = {}; for (const id of ['f', 'g']) { const n = cv.nodes.get(id); const l = n.nodeEl.querySelector('.canvas-node-label') || n.labelEl; const cs = l && getComputedStyle(l);
    o[id] = l ? { cls: l.className, text: l.textContent, display: cs.display, opacity: cs.opacity } : null; } o.zoomedOut = cv.wrapperEl.classList.contains('mod-zoomed-out'); return o;`);
  for (const mode of ['always', 'hover', 'never']) {
    await setOpt('cardLabelVisibility', mode);
    await openFixture(); await setView(0, 200, 0);
    const far = await scr(200, 800); const over = await scr(-250, 100);
    const r = {};
    await c.mouseMove(far.x, far.y); await sleep(500); r.pointerAway = await read();
    await c.mouseMove(over.x, over.y); await sleep(500); r.pointerOverCard = await read();
    await c.clickAt(over.x, over.y); await sleep(300); await c.mouseMove(far.x, far.y); await sleep(500); r.selectedPointerAway = await read();
    await setView(0, 200, -2); r.zoomedOut = await read();
    res[mode] = r;
  }
  await setOpt('cardLabelVisibility', 'always');
  return res;
});

await step('s05-zoom-threshold', async () => {
  const res = {};
  const fixture = { nodes: [{ id: 'm', type: 'text', text: 'mid', x: 0, y: 0, width: 400, height: 200 }, { id: 'f', type: 'file', file: 'Plan.md', x: 500, y: 0, width: 400, height: 200 }], edges: [] };
  await openFixture(fixture);
  const at = async (z) => { await ev(`${C} cv.setViewport(450, 100, ${z}); return true`); await sleep(250); return ev(`${C} return { zoomedOut: cv.wrapperEl.classList.contains('mod-zoomed-out'), m: cv.nodes.get('m').isContentMounted, f: cv.nodes.get('f').isContentMounted, label: getComputedStyle(cv.nodes.get('f').labelEl).display }`); };
  for (const bp of [0, 0.5, -0.7, 2.4]) {
    const opt = (await setOpt('zoomBreakpoint', bp)).zoomBreakpoint;
    const r = { stored: opt };
    for (const key of ['zoomedOut', 'm']) {
      let lo = -4, hi = 1;
      const want = key === 'zoomedOut';
      const atLo = (await at(lo))[key], atHi = (await at(hi))[key];
      if (atLo !== want || atHi === want) { r[key] = { atMin: atLo, atMax: atHi }; continue; }
      for (let i = 0; i < 13; i++) { const mid = (lo + hi) / 2; if ((await at(mid))[key] === want) lo = mid; else hi = mid; }
      r[key] = +((lo + hi) / 2).toFixed(3);
    }
    r.labelAtMinus12 = (await at(-1.2)).label;
    r.labelAtMinus2 = (await at(-2)).label;
    res[String(bp)] = r;
  }
  await setOpt('zoomBreakpoint', 0);
  return res;
});

await step('s06-ctrl-drag', async () => {
  const res = {};
  const ui = () => ev(`${C} return { menus: Array.from(document.querySelectorAll('.menu')).map(m => Array.from(m.querySelectorAll('.menu-item-title')).map(e => e.textContent)),
    modals: Array.from(document.querySelectorAll('.modal-container')).map(m => ({ cls: m.className, placeholder: m.querySelector('input')?.placeholder, title: m.querySelector('.modal-title')?.textContent })),
    editing: Array.from(cv.nodes.values()).filter(n => n.isEditing).map(n => n.id), selection: Array.from(cv.selection).map(n => n.id),
    rectShown: !!cv.wrapperEl.querySelector('.canvas-selection') };`);
  for (const mode of ['menu', 'card', 'note', 'media', 'webpage', 'group']) {
    await setOpt('defaultModDragBehavior', mode);
    await openFixture(); await setView(300, 600, 0);
    const before = (await nodes()).map((n) => n.id);
    const p = await scr(103, 707), q = await scr(407, 913);
    await gesture(p, q, 2); await sleep(700);
    const r = { ui: await ui(), added: (await nodes()).filter((n) => !before.includes(n.id)) };
    await c.pressKey('Escape'); await sleep(300); await c.pressKey('Escape'); await sleep(300);
    res[mode] = r;
  }
  await setOpt('defaultModDragBehavior', 'card');
  await openFixture(); await setView(300, 600, 0);
  { const before = (await nodes()).map((n) => n.id); const p = await scr(150, 750); await c.clickAt(p.x, p.y, { mods: { ctrl: true } }); await sleep(700); res.ctrlClickNoDrag = { ui: await ui(), added: (await nodes()).filter((n) => !before.includes(n.id)) }; }
  await setOpt('defaultModDragBehavior', 'menu');
  return res;
});

await step('s07-snap-grid', async () => {
  await setOpt('snapToGrid', true); await setOpt('snapToObjects', false);
  await openFixture(); await setView(300, 200, 0);
  const res = { drags: [] };
  for (const [dx, dy] of [[30, 0], [33, 0], [9, 0], [0, 9], [30, 2], [0, 2], [2, 0]]) { await place('a', 7, 307, 100, 100); res.drags.push({ by: [dx, dy], ...(await dragBy('a', dx, dy)) }); }
  await place('a', 7, 307, 100, 100); res.alt = await dragBy('a', 30, 0, 1);
  for (const [side, d] of [['right', 25], ['left', -25], ['bottom', 25], ['br', 25]]) { await place('a', 7, 307, 103, 107); res['resize ' + side] = await resizeBy('a', side, d); }
  await setOpt('snapToGrid', false);
  await place('a', 7, 307, 100, 100); res.neither = await dragBy('a', 30.4, 2.6);
  await place('a', 7, 307, 103, 107); res.neitherResize = await resizeBy('a', 'right', 25.4);
  await setOpt('snapToGrid', true); await setOpt('snapToObjects', true);
  return res;
});

await step('s08-snap-objects', async () => {
  await setOpt('snapToGrid', false); await setOpt('snapToObjects', true);
  const res = {};
  // A (100 square) dragged sideways from x 0 to near B's lines (B 400..600, centre 500).
  for (const zoom of [0, 1, -1, -2]) {
    await openFixture(); await setView(300, 200, zoom);
    const xs = { 0: [395, 412, 413.5, 414.5, 415.5, 434.5, 435.5, 445, 455, 495, 597, 296, 304], 1: [405, 407.5, 408, 409], '-1': [413, 421, 424, 427, 430, 440], '-2': [440, 455, 470] }[zoom];
    const r = [];
    for (const x of xs) { await place('a', 0, 300, 100, 100); r.push([x, (await dragBy('a', x, 0)).to[0]]); }
    res['scale ' + Math.pow(2, zoom)] = r;
  }
  await openFixture(); await setView(300, 200, 0);
  await place('a', 0, 300, 100, 100); res.guidesBelow = await dragBy('a', 404, -150);
  await place('a', 0, 300, 100, 100); res.guidesOnRow = await dragBy('a', 404, -297);
  await place('a', 405, 300, 100, 100); res.straightUp = await dragBy('a', 0, -40);
  await place('a', 405, 300, 100, 100); res.sideways = await dragBy('a', 40, 0);
  await place('a', 405, 300, 100, 100); res.groupNotATarget = await dragBy('a', 492, 100);
  await place('a', 0, 300, 100, 100); res.alt = await dragBy('a', 403, 0, 1);
  await place('a', 100, 200, 100, 100); res.resizeRightNearB = await resizeBy('a', 'right', 196);
  await setOpt('snapToGrid', true);
  await openFixture(); await setView(300, 200, 0);
  res.both = [];
  for (const x of [393, 397, 403, 407, 413]) { await place('a', 0, 300, 100, 100); res.both.push([x, (await dragBy('a', x, 0)).to[0]]); }
  return res;
});

save();
c.close();
process.exit(0);
