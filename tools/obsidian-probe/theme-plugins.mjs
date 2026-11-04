// The theme registry's second pass: the Bases, canvas, graph and PDF chrome.
// theme-registry.mjs read the shell, the editor, the panes and the popups;
// these four surfaces were left out, and Scoria's call sites for them read
// the token each old short field happened to be rather than the one
// Obsidian paints the surface from. This answers the same question for them:
// for every element of the view, which custom property its colour, background
// and border win from, `var()` intact (the CSSOM walk in theme-lib.mjs).
//
// Rather than a hand-picked selector per element, each state walks the view's
// whole subtree and reads one element per distinct tag-and-class signature,
// so a surface nobody thought to name is still recorded. A state is the
// view at rest, a hover under a real pointer, a selection, or an open menu or
// panel, in both base themes.
//
// Phases: fixtures | run   (node theme-plugins.mjs <phase>)
// ONLY_STEPS=dark-03,light-03 re-runs a step or two. SHOTS names the folder
// the per-state screenshots go to.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { connect, sleep } from './cdp.mjs';
import { LIB } from './theme-lib.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || '';
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 1200000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.themePlugins = all.themePlugins || {};
const ONLY = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
const prev = all.themePlugins[phase];
const out = { startedAt: new Date().toISOString(), steps: ONLY && prev ? prev.steps : {}, order: ONLY && prev ? prev.order.filter((n) => !ONLY.some((p) => n.startsWith(p))) : [] };
all.themePlugins[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' && !Array.isArray(v) ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  console.error(`[theme-plugins] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + String(out.steps[name].error).split('\n')[0]} (${out.steps[name].ms} ms)`);
  save();
  return out.steps[name];
}
const vp = (rel) => path.join(VAULT, rel);

// ---- fixtures -------------------------------------------------------------
/** A tiny valid PNG (w x h, one flat colour), for the cards cover and the canvas image card. */
function png(w, h, [r, g, b]) {
  const crc = (buf) => { let c = ~0; for (const x of buf) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); } return (~c) >>> 0; };
  const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const cc = Buffer.alloc(4); cc.writeUInt32BE(crc(td)); return Buffer.concat([len, td, cc]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  const rows = [];
  for (let y = 0; y < h; y++) { const row = Buffer.alloc(1 + w * 3); for (let x = 0; x < w; x++) { row[1 + x * 3] = r; row[2 + x * 3] = g; row[3 + x * 3] = b; } rows.push(row); }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(Buffer.concat(rows))), chunk('IEND', Buffer.alloc(0))]);
}
const note = (fm, body) => `---\n${fm}\n---\n\n${body}\n`;
const NOTES = {
  'Scoria.md': note('rating: 4\ndue: 2026-03-14\ndone: false\nkinds:\n  - basalt\n  - frothy\nrelated: "[[Pumice]]"\ntags:\n  - rock\nstatus: todo\ncover: "[[red.png]]"', '# Scoria\n\nFrothed basalt, see [[Pumice]] and [[Basalt]]. ![[red.png]]'),
  'Pumice.md': note('rating: 5\ndue: 2026-04-01\ndone: true\nkinds:\n  - felsic\nrelated: "[[Scoria]]"\ntags:\n  - rock\nstatus: done\ncover: "[[blue.png]]"', '# Pumice\n\nFloats. Back to [[Scoria]], and a [[Missing note]].'),
  'Basalt.md': note('rating: 3\ndue: 2026-05-20\ndone: false\nkinds:\n  - mafic\nrelated: "[[Tuff]]"\ntags:\n  - rock\nstatus: doing', '# Basalt\n\nDense. #volcanic'),
  'Tuff.md': note('rating: 2\ndue: 2026-07-07\ndone: false\nkinds:\n  - ash\ntags:\n  - rock\nstatus: todo', '# Tuff\n\nWelded ash, like [[Basalt]].'),
};
const ROCKS = `formulas:
  score: 'rating * 2'
summaries:
  customMean: 'values.mean().round(2)'
views:
  - type: table
    name: Table
    order:
      - file.name
      - status
      - rating
      - due
      - done
      - kinds
      - related
      - formula.score
    groupBy:
      property: status
      direction: ASC
    summaries:
      rating: Average
  - type: cards
    name: Cards
    order:
      - file.name
      - status
      - rating
    image: note.cover
  - type: list
    name: List
    order:
      - file.name
      - status
  - type: kanban
    name: Board
    order:
      - file.name
      - rating
    groupBy:
      property: status
      direction: ASC
`;
const CANVAS = {
  nodes: [
    { id: 'g1', type: 'group', x: -60, y: -80, width: 900, height: 620, label: 'A group' },
    { id: 't1', type: 'text', text: '# A card\n\nSome **text** and a [[Scoria]] link.', x: 0, y: 0, width: 260, height: 160 },
    { id: 't2', type: 'text', text: 'A red card', x: 360, y: 0, width: 220, height: 80, color: '1' },
    { id: 't3', type: 'text', text: 'A hex card', x: 620, y: 0, width: 160, height: 80, color: '#2a9d8f' },
    { id: 'f1', type: 'file', file: 'Scoria.md', x: 0, y: 240, width: 320, height: 220 },
    { id: 'f2', type: 'file', file: 'red.png', x: 380, y: 240, width: 160, height: 120 },
    { id: 'f3', type: 'file', file: 'doc.pdf', x: 600, y: 240, width: 200, height: 260 },
  ],
  edges: [
    { id: 'e1', fromNode: 't1', fromSide: 'right', toNode: 't2', toSide: 'left', label: 'an edge label' },
    { id: 'e2', fromNode: 't1', fromSide: 'bottom', toNode: 'f1', toSide: 'top', color: '4' },
  ],
};
const EMBEDS = '# Embeds\n\n![[Rocks.base]]\n\n![[doc.pdf]]\n';

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  if (!fs.existsSync(vp('.obsidian/app.json'))) fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  for (const [name, text] of Object.entries(NOTES)) fs.writeFileSync(vp(name), text);
  fs.writeFileSync(vp('red.png'), png(64, 48, [200, 60, 40]));
  fs.writeFileSync(vp('blue.png'), png(48, 64, [40, 80, 200]));
  fs.writeFileSync(vp('Rocks.base'), ROCKS);
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify(CANVAS, null, '\t'));
  fs.writeFileSync(vp('Embeds.md'), EMBEDS);
  // A real multi-page PDF from Scoria's own fixtures, so pdf.js has text and thumbnails.
  const here = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
  fs.copyFileSync(path.join(here, '../../crates/scoria-pdf/tests/fixtures/pages.pdf'), vp('doc.pdf'));
  if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true });
  out.wrote = fs.readdirSync(VAULT).sort();
  save();
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

// ---- run ------------------------------------------------------------------
const c = await connect();
out.vaultPath = await c.vaultPath();
out.title = await c.evaluate(`return document.title;`);
out.ua = await c.evaluate(`return navigator.userAgent;`);
save();
const ev = (js) => c.evaluate(js);

// Walk a root's subtree and describe one element per signature (tag plus its
// sorted classes, with the SVG `class` attribute read too). The pdf.js text
// layer and the canvas's many identical nodes collapse to a handful of rows.
const WALK = `
  const sigOf = (el) => { const cl = String(el.getAttribute('class') || '').trim().split(/\\s+/).filter(Boolean).sort(); return el.tagName.toLowerCase() + (cl.length ? '.' + cl.join('.') : ''); };
  const COMPACT = ['color', 'background-color', 'background-image', 'border-top-color', 'border-bottom-color', 'border-left-color', 'border-right-color', 'border-top-width', 'border-left-width', 'outline-color', 'fill', 'stroke', 'opacity', 'font-size', 'font-weight', 'box-shadow', 'display', 'visibility'];
  const walk = (root, max) => {
    const o = {}; let n = 0;
    if (!root) return { missing: true };
    for (const el of [root, ...root.querySelectorAll('*')]) {
      const tag = el.tagName.toLowerCase();
      // Inside an icon only the <svg> itself matters; pdf.js's page canvases and text spans are content, not chrome.
      if (el.closest('svg') && tag !== 'svg' && !(el.getAttribute('class') || '').match(/canvas|graph|edge|path/)) continue;
      if (el.closest('.textLayer, .annotationLayer') && el !== root) continue;
      const sig = sigOf(el);
      if (o[sig]) { o[sig].count++; continue; }
      if (n++ >= max) { o['…truncated'] = { count: (o['…truncated'] || { count: 0 }).count + 1 }; continue; }
      const d = describe(el, { computed: COMPACT });
      delete d.matched;
      const parent = el.parentElement ? sigOf(el.parentElement) : null;
      o[sig] = { count: 1, parent, text: (el.childElementCount === 0 ? (el.textContent || '').trim().slice(0, 40) : undefined), cls: d.cls, rect: d.rect, computed: d.computed, own: d.own, inherited: d.inherited };
    }
    return o;
  };
`;
const walkOf = (rootExpr, max = 260) => ev(LIB + WALK + `return walk(${rootExpr}, ${max});`);
const ACTIVE = `(document.querySelector('.workspace-leaf.mod-active .workspace-leaf-content') || document.querySelector('.mod-root .workspace-leaf-content'))`;
// The floating surfaces a click opens: menus, popovers, suggestion lists, modals.
const floating = () => ev(LIB + WALK + `
  const o = {};
  const els = Array.from(document.querySelectorAll('.menu, .popover, .suggestion-container, .modal-container, .canvas-menu-container, .tooltip')).filter((e) => e.offsetParent || getComputedStyle(e).position === 'fixed');
  els.forEach((e, i) => { o[i + ':' + sigOf(e)] = walk(e, 120); });
  return o;`);

let shotN = 0;
async function shot(name) {
  if (!SHOTS) return null;
  try {
    fs.mkdirSync(SHOTS, { recursive: true });
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = path.join(SHOTS, `${String(++shotN).padStart(3, '0')}-${name}.png`);
    fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
    return path.basename(f);
  } catch (e) { return 'error: ' + e.message; }
}

// Park the pointer on `over` and walk `root` (default: the hovered element's
// own subtree) while it is there.
async function hoverWalk(over, rootSel, index = 0) {
  await ev(`const el = document.querySelectorAll(${JSON.stringify(over)})[${index}]; if (el) el.scrollIntoView({ block: 'nearest' }); return true;`);
  await sleep(150);
  const r = await c.hover(over, index);
  if (!r) return { missing: over };
  await sleep(450);
  const w = await ev(LIB + WALK + `
    const hov = Array.from(document.querySelectorAll(':hover'));
    const root = ${rootSel ? `document.querySelector(${JSON.stringify(rootSel)})` : 'hov[hov.length - 1]'};
    return { hoverChain: hov.slice(-5).map(sigOf), walk: walk(root, 60) };`);
  await c.mouseMove(2, 2);
  await sleep(200);
  return w;
}
async function dismiss() {
  await ev(`document.querySelectorAll('.menu, .suggestion-container').forEach((m) => m.remove()); return true;`);
  for (let i = 0; i < 2; i++) { await c.pressKey('Escape'); await sleep(150); }
}
async function openPath(p, state) {
  await dismiss();
  // One root leaf kept and every other closed, the kept one emptied first:
  // opening over the view a graph or canvas left behind sometimes left a
  // PDF's or a note's leaf reading `empty` for good. Up to three tries, since
  // the view is built lazily and a loaded machine can miss the first.
  let got = null;
  for (let attempt = 0; attempt < 3; attempt++) {
    got = await ev(`
      let f = null;
      for (let i = 0; i < 40 && !f; i++) { f = app.vault.getAbstractFileByPath(${JSON.stringify(p)}); if (!f) await new Promise((r) => setTimeout(r, 500)); }
      if (!f) throw new Error('missing ' + ${JSON.stringify(p)});
      const keep = app.workspace.getLeaf(false);
      const others = []; app.workspace.iterateRootLeaves((l) => { if (l !== keep) others.push(l); });
      for (const l of others) l.detach();
      await keep.setViewState({ type: 'empty' });
      await keep.openFile(f${state ? `, { state: ${JSON.stringify(state)} }` : ''});
      app.workspace.setActiveLeaf(keep, { focus: true });
      for (let i = 0; i < 40; i++) { if (keep.view.file && keep.view.file.path === ${JSON.stringify(p)}) break; await new Promise((r) => setTimeout(r, 250)); }
      await new Promise((r) => setTimeout(r, 1500));
      const l = app.workspace.activeLeaf; return { type: l.view.getViewType(), file: l.view.file && l.view.file.path, attempt: ${attempt} };`);
    if (got && got.file === p) break;
    await sleep(1000);
  }
  return got;
}
async function setTheme(name) {
  await ev(`app.changeTheme ? app.changeTheme(${JSON.stringify(name)}) : app.setTheme(${JSON.stringify(name)}); return true;`);
  await sleep(900);
  return ev(`return document.body.className;`);
}

// ---- Bases ------------------------------------------------------------------
const openPanel = async (cls) => {
  for (let attempt = 0; attempt < 3; attempt++) {
    await c.clickElement('.workspace-leaf.mod-active .' + cls);
    for (let i = 0; i < 8; i++) { await sleep(150); if (await ev(`return !!document.querySelector('.menu.bases-toolbar-menu');`)) return { opened: true, attempt }; }
    await dismiss();
  }
  return { opened: false };
};
const switchView = async (name) => {
  await dismiss();
  await openPanel('bases-toolbar-views-menu');
  const r = await ev(`
    const rows = Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-item'));
    const row = rows.find((e) => ((e.querySelector('.bases-toolbar-menu-item-name') || {}).textContent || '').trim() === ${JSON.stringify(name)});
    if (!row) return { found: false };
    const info = row.querySelector('.bases-toolbar-menu-item-info') || row; const b = info.getBoundingClientRect();
    return { found: true, x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (r.found) { await c.clickAt(r.x, r.y); await sleep(2000); }
  await dismiss();
  return { ...r, now: await ev(`try { return app.workspace.activeLeaf.view.controller.viewName; } catch (e) { return null; }`) };
};

async function basesPass(theme) {
  await step(`${theme}-01-bases-table`, async (r) => {
    r.open = await openPath('Rocks.base');
    r.view = await switchView('Table');
    r.walk = await walkOf(ACTIVE);
    r.shot = await shot(`${theme}-bases-table`);
    return r;
  });
  await step(`${theme}-02-bases-hover`, async (r) => {
    r.row = await hoverWalk('.workspace-leaf.mod-active .bases-tbody .bases-tr', null, 1);
    r.rowWhole = await hoverWalk('.workspace-leaf.mod-active .bases-tbody .bases-tr .bases-td', '.workspace-leaf.mod-active .bases-tbody .bases-tr:hover', 3);
    r.header = await hoverWalk('.workspace-leaf.mod-active .bases-thead .bases-table-header', null, 2);
    r.toolbarItem = await hoverWalk('.workspace-leaf.mod-active .bases-toolbar .bases-toolbar-item', null, 1);
    r.groupHeading = await hoverWalk('.workspace-leaf.mod-active .bases-group-heading, .workspace-leaf.mod-active .bases-table-group-header, .workspace-leaf.mod-active .bases-group-value', null, 0);
    r.summary = await hoverWalk('.workspace-leaf.mod-active .bases-table-group-summary-row .bases-td, .workspace-leaf.mod-active .bases-table-footer .bases-td', null, 2);
    return r;
  });
  await step(`${theme}-03-bases-selected`, async (r) => {
    // A single click selects a cell; read the table with it selected.
    const cell = await c.rect('.workspace-leaf.mod-active .bases-tbody .bases-tr .bases-td', 6);
    if (cell) { await c.clickAt(cell.x + cell.w - 6, cell.cy); await sleep(500); }
    r.selected = await ev(LIB + WALK + `const t = document.querySelector('.workspace-leaf.mod-active .bases-tbody'); const sel = t && t.querySelectorAll('.bases-td.is-selected, .bases-td.is-focused, .bases-td:focus, .bases-td:focus-within, .bases-tr.is-selected'); return { active: document.activeElement ? sigOf(document.activeElement) : null, selected: Array.from(sel || []).slice(0, 4).map((e) => ({ sig: sigOf(e), d: (() => { const d = describe(e); delete d.matched; return d; })() })) };`);
    r.shot = await shot(`${theme}-bases-selected`);
    await dismiss();
    return r;
  });
  await step(`${theme}-04-bases-panels`, async (r) => {
    const panels = await ev(`return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-toolbar .bases-toolbar-item')).map((e) => String(e.className).split(/\\s+/).find((k) => /^bases-toolbar-.*-menu$|^bases-toolbar-result-count$|^bases-toolbar-search$/.test(k)) || String(e.className));`);
    r.panels = panels;
    for (const cls of panels) {
      if (!/-menu$/.test(cls)) continue;
      await dismiss();
      const o = await openPanel(cls);
      await sleep(300);
      const surf = await floating();
      // The panel's second row under the pointer.
      const rowHover = await hoverWalk('.menu.bases-toolbar-menu .bases-toolbar-menu-item, .menu.bases-toolbar-menu .menu-item', null, 1);
      r[cls] = { opened: o.opened, surfaces: surf, rowHover, shot: await shot(`${theme}-${cls}`) };
    }
    await dismiss();
    return r;
  });
  for (const [name, key] of [['Cards', 'cards'], ['List', 'list'], ['Board', 'kanban']]) {
    await step(`${theme}-05-bases-${key}`, async (r) => {
      r.view = await switchView(name);
      r.walk = await walkOf(ACTIVE);
      const item = { cards: '.bases-cards-item', list: '.bases-list-item', kanban: '.bases-kanban-card' }[key];
      r.hover = await hoverWalk(`.workspace-leaf.mod-active ${item}`, null, 0);
      if (key === 'kanban') r.columnHover = await hoverWalk('.workspace-leaf.mod-active .bases-kanban-column-header', null, 0);
      r.shot = await shot(`${theme}-bases-${key}`);
      return r;
    });
  }
  await step(`${theme}-06-bases-embed`, async (r) => {
    await openPath('Rocks.base');
    await switchView('Table');
    r.open = await openPath('Embeds.md', { mode: 'preview' });
    await sleep(1500);
    r.walk = await walkOf(`document.querySelector('.workspace-leaf.mod-active .markdown-preview-view .bases-embed, .workspace-leaf.mod-active .markdown-preview-view .internal-embed[src$=".base"]')`);
    r.shot = await shot(`${theme}-bases-embed`);
    return r;
  });
}

// ---- Canvas -----------------------------------------------------------------
async function canvasPass(theme) {
  await step(`${theme}-10-canvas`, async (r) => {
    r.open = await openPath('Board.canvas');
    // The nodes are mounted a frame or two after the view: wait for them.
    await ev(`const v = app.workspace.getLeavesOfType('canvas')[0].view; if (v.canvas) { v.canvas.zoomToFit(); } for (let i = 0; i < 40 && v.containerEl.querySelectorAll('.canvas-node').length < 7; i++) await new Promise((r) => setTimeout(r, 250)); return true;`);
    await sleep(1500);
    r.walk = await walkOf(ACTIVE, 320);
    r.shot = await shot(`${theme}-canvas`);
    return r;
  });
  await step(`${theme}-11-canvas-hover`, async (r) => {
    r.node = await hoverWalk('.workspace-leaf.mod-active .canvas-node:not(.canvas-node-group) .canvas-node-container', '.workspace-leaf.mod-active .canvas-node:hover', 0);
    r.group = await hoverWalk('.workspace-leaf.mod-active .canvas-group-label', null, 0);
    r.control = await hoverWalk('.workspace-leaf.mod-active .canvas-controls .canvas-control-item', null, 0);
    r.cardMenu = await hoverWalk('.workspace-leaf.mod-active .canvas-card-menu .canvas-card-menu-button', null, 0);
    r.edgeLabel = await hoverWalk('.workspace-leaf.mod-active .canvas-path-label', null, 0);
    return r;
  });
  await step(`${theme}-12-canvas-selected`, async (r) => {
    await ev(`const v = app.workspace.getLeavesOfType('canvas')[0].view; const n = v.canvas.nodes.get('t1'); v.canvas.deselectAll(); v.canvas.select(n); v.canvas.requestFrame && v.canvas.requestFrame(); return true;`);
    await sleep(700);
    r.walk = await walkOf(ACTIVE, 320);
    r.floating = await floating();
    r.menuHover = await hoverWalk('.workspace-leaf.mod-active .canvas-menu button, .workspace-leaf.mod-active .canvas-menu .clickable-icon', null, 0);
    r.shot = await shot(`${theme}-canvas-selected`);
    // An edge selected.
    await ev(`const v = app.workspace.getLeavesOfType('canvas')[0].view; const e = v.canvas.edges.get('e1'); v.canvas.deselectAll(); v.canvas.select(e); return true;`);
    await sleep(600);
    r.edge = await walkOf(`document.querySelector('.workspace-leaf.mod-active .canvas-edges')`, 40);
    r.edgeLabelSel = await walkOf(`document.querySelector('.workspace-leaf.mod-active .canvas-path-label-wrapper') || document.querySelector('.workspace-leaf.mod-active .canvas-path-label')`, 20);
    r.shotEdge = await shot(`${theme}-canvas-edge-selected`);
    await ev(`app.workspace.getLeavesOfType('canvas')[0].view.canvas.deselectAll(); return true;`);
    return r;
  });
}

// ---- Graph ------------------------------------------------------------------
async function graphPass(theme) {
  await step(`${theme}-20-graph`, async (r) => {
    await dismiss();
    await ev(`app.workspace.detachLeavesOfType('graph'); app.commands.executeCommandById('graph:open'); return true;`);
    await sleep(2500);
    // Open the controls and unfold every section.
    await ev(`
      const v = app.workspace.getLeavesOfType('graph')[0].view; const el = v.containerEl;
      const ctl = el.querySelector('.graph-controls'); if (ctl && ctl.classList.contains('is-close')) { const b = ctl.querySelector('.graph-controls-button.mod-open, .clickable-icon'); if (b) b.click(); }
      await new Promise((r) => setTimeout(r, 300));
      for (const s of Array.from(el.querySelectorAll('.graph-controls .tree-item.is-collapsed'))) { const h = s.querySelector('.tree-item-self'); if (h) h.click(); await new Promise((r) => setTimeout(r, 200)); }
      return true;`);
    await sleep(800);
    r.walk = await walkOf(`app.workspace.getLeavesOfType('graph')[0].view.containerEl`, 320);
    // The colours the renderer paints nodes and links with are read off hidden
    // elements by class; record each with its winning declaration.
    r.colors = await ev(LIB + `
      const el = app.workspace.getLeavesOfType('graph')[0].view.containerEl;
      const o = {};
      for (const e of Array.from(el.querySelectorAll('[class*="color-"]'))) { const d = describe(e, { computed: ['color', 'opacity', 'display'] }); delete d.matched; o[String(e.getAttribute('class'))] = d; }
      return o;`);
    // Those elements are made and dropped while the renderer reads them, so
    // the rules themselves are the record: every rule naming a colour class.
    r.colorRules = await ev(LIB + `
      return allRules().filter(({ sel }) => /\\.color-(fill|line|arrow|circle|text)|graph-view\\.color/.test(sel)).map(({ r, sel, sheet }) => ({ sel, css: r.style.cssText, sheet: String(sheet).slice(-40) }));`);
    r.renderer = await ev(`try { const v = app.workspace.getLeavesOfType('graph')[0].view; const rd = v.renderer; const col = rd.colors || {}; const o = {}; for (const k of Object.keys(col)) { const x = col[k]; o[k] = x && typeof x === 'object' ? { rgb: x.rgb, a: x.a } : x; } return { keys: Object.keys(rd).filter((k) => /colo/i.test(k)), colors: o }; } catch (e) { return { error: e.message }; }`);
    r.shot = await shot(`${theme}-graph`);
    return r;
  });
  await step(`${theme}-21-graph-hover`, async (r) => {
    r.section = await hoverWalk('.graph-controls .tree-item-self', null, 0);
    r.close = await hoverWalk('.graph-controls .graph-controls-button', null, 0);
    r.toggle = await hoverWalk('.graph-controls .checkbox-container', null, 0);
    r.button = await hoverWalk('.graph-controls button', null, 0);
    r.colorGroup = await hoverWalk('.graph-controls .graph-color-group, .graph-controls .graph-color-button-container', null, 0);
    // The panel closed: the cog button.
    await ev(`const el = app.workspace.getLeavesOfType('graph')[0].view.containerEl; const b = el.querySelector('.graph-controls .graph-controls-button.mod-close'); if (b) b.click(); return true;`);
    await sleep(400);
    r.closed = await walkOf(`app.workspace.getLeavesOfType('graph')[0].view.containerEl.querySelector('.graph-controls')`, 40);
    r.cog = await hoverWalk('.graph-controls .graph-controls-button.mod-open, .graph-controls.is-close .clickable-icon', null, 0);
    r.shot = await shot(`${theme}-graph-closed`);
    return r;
  });
}

// ---- PDF --------------------------------------------------------------------
async function pdfPass(theme) {
  await step(`${theme}-30-pdf`, async (r) => {
    r.open = await openPath('doc.pdf');
    await sleep(1500);
    r.walk = await walkOf(ACTIVE, 260);
    r.vars = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .pdf-container') || document.body; const cs = getComputedStyle(e); const o = {}; for (const n of ['--pdf-background', '--pdf-page-background', '--pdf-sidebar-background', '--pdf-toolbar-background', '--pdf-shadow', '--pdf-thumbnail-shadow', '--pdf-spread-color']) o[n] = cs.getPropertyValue(n).trim(); return o;`);
    r.shot = await shot(`${theme}-pdf`);
    return r;
  });
  await step(`${theme}-31-pdf-hover`, async (r) => {
    r.toolbarButton = await hoverWalk('.workspace-leaf.mod-active .pdf-toolbar .clickable-icon', null, 1);
    r.pageInput = await hoverWalk('.workspace-leaf.mod-active .pdf-toolbar .pdf-page-input', null, 0);
    return r;
  });
  await step(`${theme}-32-pdf-sidebar`, async (r) => {
    await ev(`const t = document.querySelector('.workspace-leaf.mod-active .pdf-toolbar [aria-label="Toggle sidebar"]'); if (t && !document.querySelector('.workspace-leaf.mod-active .pdf-content-container.sidebarOpen')) t.click(); return true;`);
    await sleep(1500);
    r.walk = await walkOf(`document.querySelector('.workspace-leaf.mod-active .pdf-sidebar-container')`, 80);
    r.toolbar = await walkOf(`document.querySelector('.workspace-leaf.mod-active .pdf-toolbar')`, 60);
    r.thumbHover = await hoverWalk('.workspace-leaf.mod-active .pdf-thumbnail-view .thumbnail', null, 1);
    r.shot = await shot(`${theme}-pdf-sidebar`);
    // The outline tab, if the toolbar offers one.
    r.outline = await ev(`const b = document.querySelector('.workspace-leaf.mod-active .pdf-sidebar-container [aria-label*="utline"], .workspace-leaf.mod-active .pdf-toolbar [aria-label*="utline"]'); if (b) { b.click(); return true; } return false;`);
    if (r.outline) { await sleep(700); r.outlineWalk = await walkOf(`document.querySelector('.workspace-leaf.mod-active .pdf-sidebar-container')`, 60); }
    await ev(`const t = document.querySelector('.workspace-leaf.mod-active .pdf-toolbar [aria-label="Toggle sidebar"]'); if (t && document.querySelector('.workspace-leaf.mod-active .pdf-content-container.sidebarOpen')) t.click(); return true;`);
    await sleep(500);
    return r;
  });
  await step(`${theme}-33-pdf-find`, async (r) => {
    await c.clickElement('.workspace-leaf.mod-active .pdf-viewer-container');
    await sleep(200);
    await c.pressKey('f', { ctrl: true });
    await sleep(600);
    await c.typeText('page');
    await sleep(1200);
    r.walk = await walkOf(`document.querySelector('.workspace-leaf.mod-active .pdf-findbar')`, 60);
    r.highlights = await walkOf(`document.querySelector('.workspace-leaf.mod-active .textLayer .highlight') && document.querySelector('.workspace-leaf.mod-active .textLayer .highlight').parentElement`, 10);
    r.highlightEls = await ev(LIB + `const o = {}; for (const s of ['.textLayer .highlight', '.textLayer .highlight.selected']) { const e = document.querySelector('.workspace-leaf.mod-active ' + s); if (e) { const d = describe(e); delete d.matched; o[s] = d; } } return o;`);
    r.shot = await shot(`${theme}-pdf-find`);
    await c.pressKey('Escape');
    await sleep(300);
    return r;
  });
  await step(`${theme}-34-pdf-options`, async (r) => {
    await dismiss();
    const b = await c.rect('.workspace-leaf.mod-active .pdf-toolbar [aria-label="Display options"]');
    if (b) { await c.clickAt(b.cx, b.cy); await sleep(600); }
    r.floating = await floating();
    r.shot = await shot(`${theme}-pdf-options`);
    await dismiss();
    return r;
  });
  await step(`${theme}-35-pdf-embed`, async (r) => {
    r.open = await openPath('Embeds.md', { mode: 'preview' });
    await ev(`const e = document.querySelector('.workspace-leaf.mod-active .pdf-embed'); if (e) e.scrollIntoView({ block: 'start' }); return true;`);
    await sleep(2000);
    r.walk = await walkOf(`document.querySelector('.workspace-leaf.mod-active .pdf-embed')`, 120);
    r.shot = await shot(`${theme}-pdf-embed`);
    return r;
  });
}

await step('s00-window', async () => ev(`return { w: window.innerWidth, h: window.innerHeight, dpr: window.devicePixelRatio, platform: document.body.className.match(/mod-(windows|macos|linux)/) && document.body.className.match(/mod-(windows|macos|linux)/)[0] };`));
// Every rule of the app's sheets that names one of the four surfaces, with
// its declarations: the states the walks above cannot all reach (a focused
// canvas node, a resizer, a coloured edge, a dragged kanban card) are in here.
await step('s01-rules', async () => ev(LIB + `
  const re = /bases|canvas|graph|pdf|thumbnail|textLayer|findbar/;
  return allRules().filter(({ sel }) => re.test(sel)).map(({ r, sel, sheet }) => ({ sel, css: r.style.cssText, sheet: String(sheet).slice(-40) }));`));
for (const [theme, name] of [['dark', 'obsidian'], ['light', 'moonstone']]) {
  await step(`${theme}-00-theme`, async () => ({ body: await setTheme(name) }));
  await basesPass(theme);
  await canvasPass(theme);
  await graphPass(theme);
  await pdfPass(theme);
}
await step('s99-theme-back', async () => setTheme('obsidian'));

save();
await c.quit();
process.exit(0);
