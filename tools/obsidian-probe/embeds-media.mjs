// A canvas embedded in a note, and the audio and video players: what Obsidian
// draws for `![[Board.canvas]]`, `![[x.mp3]]` and `![[x.mp4]]` in the reading
// view and Live Preview, and for an audio or video file opened in a tab.
// Real input over CDP (Input.dispatchMouseEvent / dispatchKeyEvent).
//
//   s00  context: version, window
//   s01  the canvas embed in reading view: markup, rects, styles, the viewport
//        the embedded canvas fitted to, its nodes' screen rects; a screenshot
//   s02  the embed's sizes: `|400`, `|400x300`, `|x300`, and a subpath
//   s01b the minimap's SVG: every element's attributes and computed paint
//   s01c the same in the light theme
//   s01d input: the cursor over it, a click on the minimap and on the title,
//        Ctrl+click, the wheel
//   s03b what shows on hover
//   s04  the canvas embed in Live Preview: markup, rects; a click on it (does
//        it reveal the source?), the wheel, a drag
//   s05  nested: a note embedding a note that embeds the canvas, the canvas
//        embedding itself through a file card, a canvas whose file card is a
//        note embedding the canvas
//   s06  audio files opened in a tab: the view type, the markup, the
//        `<audio>` element's attributes, rects; canPlayType and whether each
//        format loads (duration, error); a screenshot
//   s07  audio embeds `![[x.mp3]]` in reading view and Live Preview: markup,
//        rects; a click in Live Preview; the keys on a focused player
//   s08  video files opened in a tab and embedded: markup, attributes,
//        videoWidth/Height, duration, errors per format; a screenshot
//   s09  the workspace leaf states for an audio and a video tab
//
// MEDIA_DIR holds tone.{wav,mp3,m4a,flac,ogg,oga,opus,3gp} and
// clip.{mp4,webm,mov,mkv,ogv}, copied into the vault by `fixtures`.
// Screenshots go to <results>-shots/. ONLY_STEPS=<regex> narrows the run.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const MEDIA = process.env.MEDIA_DIR;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 240000) {
  if (ONLY && !ONLY.test(name)) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[embeds] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

const AUDIO = ['wav', 'mp3', 'm4a', 'flac', 'ogg', 'oga', 'opus', '3gp'];
const VIDEO = ['mp4', 'webm', 'mov', 'mkv', 'ogv'];

const BOARD = {
  nodes: [
    { id: 'g1', type: 'group', x: -60, y: -60, width: 760, height: 420, label: 'Group' },
    { id: 't1', type: 'text', x: 0, y: 0, width: 250, height: 60, text: 'Text card' },
    { id: 't2', type: 'text', x: 400, y: 0, width: 250, height: 120, text: '# Heading\n\nSecond card', color: '4' },
    { id: 'f1', type: 'file', x: 0, y: 160, width: 300, height: 160, file: 'Other.md' },
    { id: 'l1', type: 'link', x: 400, y: 200, width: 250, height: 100, url: 'https://example.com' },
  ],
  edges: [{ id: 'e1', fromNode: 't1', fromSide: 'right', toNode: 't2', toSide: 'left', label: 'edge' }],
};

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify(BOARD, null, '\t'));
  fs.writeFileSync(vp('Tall.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'text', x: 0, y: 0, width: 200, height: 1200, text: 'Tall' },
  ], edges: [] }, null, '\t'));
  fs.writeFileSync(vp('Wide.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'text', x: 0, y: 0, width: 3000, height: 100, text: 'Wide' },
  ], edges: [] }, null, '\t'));
  fs.writeFileSync(vp('Empty.canvas'), '');
  fs.writeFileSync(vp('Edges.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'text', x: 0, y: 0, width: 100, height: 100, text: 'A' },
    { id: 'b', type: 'text', x: 1000, y: 500, width: 100, height: 100, text: 'B', color: '#ff0000' },
    { id: 'c', type: 'text', x: 0, y: 500, width: 100, height: 100, text: 'C', color: '1' },
    { id: 'd', type: 'text', x: 1000, y: 0, width: 100, height: 100, text: 'D', color: '6' },
  ], edges: [
    { id: 'e1', fromNode: 'a', fromSide: 'bottom', toNode: 'c', toSide: 'top' },
    { id: 'e2', fromNode: 'a', toNode: 'b', color: '1' },
    { id: 'e3', fromNode: 'c', fromSide: 'right', toNode: 'b', toSide: 'left', toEnd: 'none' },
    { id: 'e4', fromNode: 'd', fromSide: 'left', toNode: 'a', toSide: 'right', fromEnd: 'arrow' },
  ] }, null, '\t'));
  fs.writeFileSync(vp('Edges2.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'text', x: 0, y: 0, width: 100, height: 100, text: 'A' },
    { id: 'b', type: 'text', x: 1000, y: 500, width: 100, height: 100, text: 'B', color: '#ff0000' },
    { id: 'c', type: 'text', x: 0, y: 500, width: 100, height: 100, text: 'C', color: '1' },
    { id: 'd', type: 'text', x: 1000, y: 0, width: 100, height: 100, text: 'D', color: '6' },
  ], edges: [
    { id: 'e1', fromNode: 'a', fromSide: 'bottom', toNode: 'c', toSide: 'top' },
    { id: 'e2', fromNode: 'a', fromSide: 'right', toNode: 'b', toSide: 'top', color: '1' },
    { id: 'e3', fromNode: 'c', fromSide: 'right', toNode: 'b', toSide: 'left', toEnd: 'none' },
    { id: 'e4', fromNode: 'd', fromSide: 'left', toNode: 'a', toSide: 'right', fromEnd: 'arrow' },
  ] }, null, '\t'));
  fs.writeFileSync(vp('Edges3.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'text', x: 0, y: 0, width: 100, height: 100, text: 'A' },
    { id: 'b', type: 'text', x: 1000, y: 500, width: 100, height: 100, text: 'B', color: '#ff0000' },
  ], edges: [] }, null, '\t'));
  fs.writeFileSync(vp('Small.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'text', x: 0, y: 0, width: 100, height: 40, text: 'A' },
    { id: 'b', type: 'text', x: 200, y: 0, width: 100, height: 40, text: 'B' },
  ], edges: [{ id: 'e', fromNode: 'a', fromSide: 'right', toNode: 'b', toSide: 'left' }] }, null, '\t'));
  fs.writeFileSync(vp('Edges.md'), '# Edges\n\n![[Edges.canvas]]\n\n![[Small.canvas]]\n\n![[Edges2.canvas]]\n\n![[Edges3.canvas]]\n');
  fs.writeFileSync(vp('Self.canvas'), JSON.stringify({ nodes: [
    { id: 'a', type: 'file', x: 0, y: 0, width: 400, height: 300, file: 'Self.canvas' },
    { id: 'b', type: 'file', x: 500, y: 0, width: 400, height: 300, file: 'Loop.md' },
  ], edges: [] }, null, '\t'));
  fs.writeFileSync(vp('Other.md'), '# Other\n\nThe other note.\n');
  fs.writeFileSync(vp('Loop.md'), 'Loop embeds the canvas:\n\n![[Self.canvas]]\n');
  fs.writeFileSync(vp('Inner.md'), 'Inner embeds the board:\n\n![[Board.canvas]]\n');
  const filler = Array.from({ length: 30 }, (_, i) => `Filler line ${i + 1}.`).join('\n\n');
  fs.writeFileSync(vp('Note.md'), `# Note\n\nBefore the embed.\n\n![[Board.canvas]]\n\nAfter the embed.\n\n${filler}\n`);
  fs.writeFileSync(vp('Sizes.md'), '# Sizes\n\n![[Board.canvas|400]]\n\n![[Board.canvas|400x300]]\n\n![[Board.canvas|x300]]\n\n![[Tall.canvas]]\n\n![[Wide.canvas]]\n\n![[Empty.canvas]]\n\n![[Board.canvas#t2]]\n\n![[Missing.canvas]]\n');
  fs.writeFileSync(vp('Nested.md'), '# Nested\n\n![[Inner]]\n\n![[Loop]]\n');
  if (MEDIA) {
    for (const f of fs.readdirSync(MEDIA)) fs.copyFileSync(path.join(MEDIA, f), vp(f));
  }
  fs.writeFileSync(vp('Audio.md'), '# Audio\n\n' + AUDIO.map((e) => `![[tone.${e}]]`).join('\n\n') + '\n\n![[tone.mp3|300]]\n\n![[missing.mp3]]\n');
  fs.writeFileSync(vp('Video.md'), '# Video\n\n' + VIDEO.map((e) => `![[clip.${e}]]`).join('\n\n') + '\n\n![[clip.mp4|200]]\n\n![[clip.mp4|200x100]]\n\n![[missing.mp4]]\n');
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

await c.evaluate(`
window.__s = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x * 10) / 10, Math.round(r.y * 10) / 10, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10]; },
  skeleton(e, d = 0, max = 6, kids = 8) {
    if (!e) return null;
    const tag = e.tagName.toLowerCase();
    if (['path', 'line', 'circle', 'rect', 'polyline', 'polygon'].includes(tag)) return null;
    const o = { tag, cls: e.getAttribute('class') || undefined, rect: this.rect(e) };
    for (const a of ['aria-label', 'role', 'tabindex', 'style', 'src', 'alt', 'width', 'height', 'controls', 'loop', 'preload', 'controlslist', 'data-path', 'href', 'contenteditable']) { const v = e.getAttribute(a); if (v !== null) o[a] = v; }
    if (!e.children.length) { const t = (e.textContent || '').trim(); if (t) o.text = t.slice(0, 120); }
    if (d < max && e.children.length) o.children = Array.from(e.children).slice(0, kids).map((c) => this.skeleton(c, d + 1, max, kids)).filter(Boolean);
    if (e.children.length > kids) o.more = e.children.length - kids;
    return o;
  },
  styles(e, keys) {
    if (!e) return null;
    const cs = getComputedStyle(e); const o = {};
    for (const k of keys || ['display', 'position', 'width', 'height', 'max-height', 'margin', 'padding', 'border', 'border-radius', 'background-color', 'overflow', 'cursor', 'box-shadow', 'outline', 'color', 'font-size']) o[k] = cs.getPropertyValue(k);
    return o;
  },
  leaf() {
    const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l));
    for (const l of leaves.slice(1)) l.detach();
    return leaves[0] || app.workspace.getLeaf(false);
  },
  async open(path, mode) {
    const leaf = this.leaf();
    if (!leaf.view.file || leaf.view.file.path !== path) await leaf.openFile(app.vault.getAbstractFileByPath(path));
    const vs = leaf.getViewState();
    if (vs.type === 'markdown') {
      if (mode === 'source') { vs.state.mode = 'source'; vs.state.source = true; }
      if (mode === 'live') { vs.state.mode = 'source'; vs.state.source = false; }
      if (mode === 'preview') { vs.state.mode = 'preview'; }
      await leaf.setViewState(vs);
    }
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(1200);
    return { file: leaf.view.file && leaf.view.file.path, type: leaf.view.getViewType(), state: leaf.getViewState().state };
  },
  root() { return this.leaf().view.containerEl; },
  embeds() { return Array.from(this.root().querySelectorAll('.internal-embed')).filter((e) => e.offsetParent); },
  canvasOf(el) {
    // The embedded canvas object: Obsidian keeps it on the embed's child component.
    const out = {};
    for (const k of Object.keys(el)) out[k] = typeof el[k];
    return out;
  },
};
return true;`);
const ev = (s) => c.evaluate(s);
const S = 'const S = window.__s;';
let shotN = 0;
async function shot(name) {
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
const open = (p, m) => ev(`${S} return await S.open(${JSON.stringify(p)}, ${JSON.stringify(m || null)});`);
const mouseEv = (type, x, y, extra = {}) => c.send('Input.dispatchMouseEvent', { type, x: Math.round(x), y: Math.round(y), ...extra });
const wheel = (x, y, dx, dy, mods = 0) => c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: dx, deltaY: dy, modifiers: mods });

// What one canvas embed looks like now: its rects, the canvas's own
// transform and the nodes' screen rects.
const canvasState = (i = 0) => ev(`${S}
  const e = S.embeds().filter((x) => x.classList.contains('canvas-embed') || /\\.canvas/.test(x.getAttribute('src') || ''))[${i}];
  if (!e) return null;
  const canvasEl = e.querySelector('.canvas');
  const wrapper = e.querySelector('.canvas-wrapper');
  const nodes = Array.from(e.querySelectorAll('.canvas-node')).map((n) => ({ cls: S.cls(n), rect: S.rect(n), text: (n.innerText || '').slice(0, 40) }));
  let model = null;
  try {
    // Walk the embed's component tree for an object with a canvas.
    const seen = new Set();
    const find = (o, d) => { if (!o || typeof o !== 'object' || seen.has(o) || d > 4) return null; seen.add(o);
      if (o.tx !== undefined && o.ty !== undefined && o.tZoom !== undefined && o.nodes) return o;
      for (const k of Object.keys(o)) { if (['app', 'containerEl', 'parent', 'workspace', 'vault'].includes(k)) continue; try { const r = find(o[k], d + 1); if (r) return r; } catch {} }
      return null; };
    const cv = find(e, 0);
    if (cv) model = { tx: cv.tx, ty: cv.ty, tZoom: cv.tZoom, zoom: cv.zoom, x: cv.x, y: cv.y, scale: cv.scale, readonly: cv.readonly, selection: Array.from(cv.selection || []).map((n) => n.id), keys: Object.keys(cv).slice(0, 80) };
  } catch (err) { model = 'ERR ' + err; }
  return { src: e.getAttribute('src'), cls: S.cls(e), rect: S.rect(e), style: e.getAttribute('style'), styles: S.styles(e),
    canvasTransform: canvasEl && canvasEl.style.transform, wrapper: wrapper && S.cls(wrapper), nodes, model,
    edges: e.querySelectorAll('.canvas-edges path.canvas-display-path').length,
    scroller: (() => { const s = S.root().querySelector('.markdown-preview-view, .cm-scroller'); return s ? s.scrollTop : null; })(),
    active: document.activeElement && S.cls(document.activeElement),
    leafType: S.leaf().view.getViewType(), leafFile: S.leaf().view.file && S.leaf().view.file.path };`);

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio, apiVersion: (() => { try { return require('obsidian').apiVersion; } catch { return null; } })() };`);
  await ev(`app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse(); return true;`);
  return { version: out.version };
});

await step('s01-canvas-reading', async () => {
  const o = await open('Note.md', 'preview');
  await sleep(1500);
  const markup = await ev(`${S} const e = S.embeds()[0]; return e ? S.skeleton(e, 0, 7, 6) : null;`);
  const st = await canvasState(0);
  const deep = await ev(`${S} const e = S.embeds()[0]; if (!e) return null;
    const q = (sel) => Array.from(e.querySelectorAll(sel)).slice(0, 3).map((x) => ({ cls: S.cls(x), rect: S.rect(x), styles: S.styles(x) }));
    return { header: q('.embed-title, .markdown-embed-title, .canvas-embed-title, .embed-header'), link: q('.markdown-embed-link, .canvas-embed-link, .embed-link'),
      controls: q('.canvas-controls'), minimap: q('.canvas-minimap'), background: q('.canvas-background'), wrapper: q('.canvas-wrapper'), card: q('.canvas-card-menu'),
      linkAria: Array.from(e.querySelectorAll('[aria-label]')).map((x) => [S.cls(x), x.getAttribute('aria-label')]) };`);
  return { open: o, markup, state: st, deep, shot: await shot('canvas-reading') };
});

await step('s01b-minimap', async () => {
  await open('Note.md', 'preview');
  await sleep(1500);
  return await ev(`${S} const e = S.embeds()[0]; const svg = e && e.querySelector('svg.canvas-minimap'); if (!svg) return null;
    const attrs = (x) => Object.fromEntries(Array.from(x.attributes).map((a) => [a.name, a.value]));
    const cs = (x) => { const c = getComputedStyle(x); return { fill: c.fill, stroke: c.stroke, strokeWidth: c.strokeWidth, opacity: c.opacity, fillOpacity: c.fillOpacity, rx: c.rx, ry: c.ry, strokeLinecap: c.strokeLinecap, filter: c.filter }; };
    const walk = (x) => ({ tag: x.tagName, attrs: attrs(x), cs: cs(x), rect: S.rect(x), children: Array.from(x.children).map(walk) });
    const title = e.querySelector('.embed-title');
    return { svg: walk(svg), svgStyles: S.styles(svg, ['width', 'height', 'max-height', 'padding', 'background-color', 'border-radius', 'display', 'margin', 'cursor']),
      title: { text: title.innerText, rect: S.rect(title), styles: S.styles(title, ['font-size', 'font-weight', 'color', 'gap', 'padding', 'line-height', 'align-items', 'cursor']), icon: S.styles(title.querySelector('.file-embed-icon'), ['width', 'height', 'color', 'margin-right']) },
      embedStyles: S.styles(e, ['cursor', 'margin', 'padding', 'display']) };`);
});

await step('s01e-minimap-edges', async () => {
  await open('Edges.md', 'preview');
  await sleep(4000);
  const r = await ev(`${S} return S.embeds().map((e) => { const svg = e.querySelector('svg.canvas-minimap'); if (!svg) return null;
    const attrs = (x) => Object.fromEntries(Array.from(x.attributes).map((a) => [a.name, a.value]));
    return { src: e.getAttribute('src'), rect: S.rect(e), svgRect: S.rect(svg), viewBox: svg.getAttribute('viewBox'), els: Array.from(svg.children).map((x) => { const c = getComputedStyle(x); return { tag: x.tagName, attrs: attrs(x), fill: c.fill, stroke: c.stroke, sw: c.strokeWidth, fo: c.fillOpacity }; }) }; });`);
  return { embeds: r, shot: await shot('minimap-edges') };
});

await step('s01c-minimap-light', async () => {
  await ev(`app.changeTheme && app.changeTheme('moonstone'); document.body.classList.remove('theme-dark'); document.body.classList.add('theme-light'); return true;`);
  await sleep(800);
  const r = await ev(`${S} const svg = S.embeds()[0].querySelector('svg.canvas-minimap'); return Array.from(svg.querySelectorAll('*')).slice(0, 12).map((x) => { const c = getComputedStyle(x); return [x.tagName, x.getAttribute('class'), c.fill, c.stroke, c.strokeWidth, c.opacity]; });`);
  const shotL = await shot('canvas-reading-light');
  await ev(`document.body.classList.remove('theme-light'); document.body.classList.add('theme-dark'); app.changeTheme && app.changeTheme('obsidian'); return true;`);
  await sleep(500);
  return { light: r, shot: shotL };
});

await step('s01d-minimap-click', async () => {
  await open('Note.md', 'preview');
  await sleep(1500);
  const st = await canvasState(0);
  const [x, y, w, h] = st.rect;
  const res = {};
  // Hover, then the cursor over the minimap and the title.
  await mouseEv('mouseMoved', x + w / 2, y + 200); await sleep(500);
  res.cursorOverMinimap = await ev(`${S} const el = document.elementFromPoint(${x + w / 2}, ${y + 200}); return [S.cls(el), getComputedStyle(el).cursor, el.tagName];`);
  res.hoverShot = await shot('minimap-hover');
  await c.clickAt(x + w / 2, y + 200); await sleep(1500);
  res.afterMinimapClick = await ev(`${S} const ls = []; app.workspace.iterateRootLeaves((l) => ls.push({ type: l.view.getViewType(), file: l.view.file && l.view.file.path, active: l === app.workspace.activeLeaf })); return ls;`);
  await open('Note.md', 'preview'); await sleep(1500);
  await c.clickAt(x + 30, y + 12); await sleep(1500);
  res.afterTitleClick = await ev(`${S} const ls = []; app.workspace.iterateRootLeaves((l) => ls.push({ type: l.view.getViewType(), file: l.view.file && l.view.file.path, active: l === app.workspace.activeLeaf })); return ls;`);
  await open('Note.md', 'preview'); await sleep(1500);
  await c.clickAt(x + w / 2, y + 200, { mods: { ctrl: true } }); await sleep(1500);
  res.afterCtrlClick = await ev(`${S} const ls = []; app.workspace.iterateRootLeaves((l) => ls.push({ type: l.view.getViewType(), file: l.view.file && l.view.file.path, active: l === app.workspace.activeLeaf })); return ls;`);
  await open('Note.md', 'preview'); await sleep(1500);
  await wheel(x + w / 2, y + 200, 0, 200); await sleep(700);
  res.wheelScrollsNote = await ev(`${S} return S.root().querySelector('.markdown-preview-view').scrollTop;`);
  return res;
});

await step('s02-canvas-sizes', async () => {
  await open('Sizes.md', 'preview');
  await sleep(1500);
  for (let i = 0; i < 6; i++) { await ev(`${S} const p = S.root().querySelector('.markdown-preview-view'); p.scrollTop += 600; return true;`); await sleep(600); }
  await ev(`${S} S.root().querySelector('.markdown-preview-view').scrollTop = 0; return true;`); await sleep(600);
  const r = await ev(`${S} return Array.from(S.root().querySelectorAll('.internal-embed')).map((e) => ({ src: e.getAttribute('src'), alt: e.getAttribute('alt'), width: e.getAttribute('width'), height: e.getAttribute('height'), cls: S.cls(e), rect: S.rect(e), style: e.getAttribute('style'), text: e.children.length ? null : e.textContent, inner: S.cls(e.firstElementChild), nodes: e.querySelectorAll('.canvas-node').length, transform: (e.querySelector('.canvas') || {}).style ? e.querySelector('.canvas').style.transform : null }));`);
  const missing = await ev(`${S} const e = Array.from(S.root().querySelectorAll('.internal-embed')).find((x) => /Missing/.test(x.getAttribute('src'))); return e ? S.skeleton(e) : null;`);
  const empty = await ev(`${S} const e = Array.from(S.root().querySelectorAll('.internal-embed')).find((x) => /Empty/.test(x.getAttribute('src'))); return e ? S.skeleton(e, 0, 5) : null;`);
  return { embeds: r, missing, empty, shot: await shot('canvas-sizes') };
});

await step('s03b-canvas-hover', async () => {
  await open('Note.md', 'preview');
  await sleep(1500);
  const st = await canvasState(0);
  const [x, y, w, h] = st.rect;
  await mouseEv('mouseMoved', x + w - 30, y + 30); await sleep(600);
  const hover = await ev(`${S} const e = S.embeds()[0]; return { hovered: Array.from(e.querySelectorAll('*')).filter((x) => getComputedStyle(x).opacity !== '0' && /link|title|button|controls/.test(S.cls(x) || '')).map((x) => [S.cls(x), S.rect(x), x.getAttribute('aria-label')]).slice(0, 20) };`);
  return { hover, shot: await shot('canvas-hover') };
});

await step('s04-canvas-live', async () => {
  const o4 = await open('Note.md', 'live');
  await sleep(500);
  const dbg = await ev(`${S} const v = S.leaf().view; return { type: v.getViewType(), mode: v.getMode && v.getMode(), keys: Object.keys(v).slice(0, 40), hasEditor: !!v.editor, ae: !!app.workspace.activeEditor };`);
  try { await ev(`${S} (S.leaf().view.editor || app.workspace.activeEditor.editor).setCursor({ line: 0, ch: 0 }); return true;`); } catch (e) { return { o4, dbg, err: String(e) }; }
  await sleep(1500);
  const markup = await ev(`${S} const e = S.root().querySelector('.cm-embed-block, .internal-embed'); return e ? S.skeleton(e.closest('.cm-embed-block') || e, 0, 6, 6) : null;`);
  const st = await canvasState(0);
  const res = { markup, state: st, shot: await shot('canvas-live') };
  if (st && st.rect) {
    const [x, y, w, h] = st.rect;
    await c.clickAt(x + 12, y + h - 12); await sleep(800);
    res.clickBg = { leaf: await ev(`${S} const v = S.leaf().view; return { type: v.getViewType(), file: v.file && v.file.path, cursor: v.editor ? v.editor.getCursor() : null };`), shot: await shot('canvas-live-click') };
    await open('Note.md', 'live'); await sleep(800);
    await c.clickAt(x + 30, y + 12); await sleep(1200);
    res.clickTitle = await ev(`${S} const v = S.leaf().view; return { type: v.getViewType(), file: v.file && v.file.path, cursor: v.editor ? v.editor.getCursor() : null };`);
    await open('Note.md', 'live'); await sleep(800);
    res.state2 = await canvasState(0);
        await wheel(x + w / 2, y + h / 2, 0, 120); await sleep(700);
    res.wheel = await canvasState(0);
    const btn = await ev(`${S} const b = S.root().querySelector('.edit-block-button'); return b ? { rect: S.rect(b), aria: b.getAttribute('aria-label') } : null;`);
    res.editButton = btn;
  }
  return res;
});

await step('s05-nested', async () => {
  await open('Nested.md', 'preview');
  await sleep(2500);
  const r = await ev(`${S} const top = S.root();
    const all = Array.from(top.querySelectorAll('.internal-embed')).map((e) => ({ src: e.getAttribute('src'), cls: S.cls(e), rect: S.rect(e), depth: (() => { let d = 0, p = e.parentElement; while (p) { if (p.classList && p.classList.contains('internal-embed')) d++; p = p.parentElement; } return d; })(), nodes: e.querySelectorAll(':scope .canvas-node').length, text: e.querySelector('.canvas') ? null : (e.innerText || '').slice(0, 80) }));
    return all;`);
  const shotA = await shot('nested');
  await open('Self.canvas');
  await sleep(2000);
  const self = await ev(`${S} const r = S.root(); return { nodes: Array.from(r.querySelectorAll('.canvas-node')).map((n) => ({ rect: S.rect(n), cls: S.cls(n), text: (n.innerText || '').slice(0, 80), innerEmbeds: n.querySelectorAll('.internal-embed').length, innerCanvas: n.querySelectorAll('.canvas').length })) };`);
  return { nested: r, shotA, self, shotB: await shot('self-canvas') };
});

await step('s06-audio-tab', async () => {
  const res = {};
  res.canPlay = await ev(`const a = document.createElement('audio'); return Object.fromEntries(['audio/mpeg', 'audio/wav', 'audio/mp4', 'audio/aac', 'audio/flac', 'audio/ogg', 'audio/ogg; codecs=opus', 'audio/webm', 'audio/3gpp', 'video/mp4', 'video/mp4; codecs="avc1.42E01E"', 'video/webm; codecs="vp9"', 'video/quicktime', 'video/x-matroska', 'video/ogg; codecs="theora"'].map((t) => [t, a.canPlayType(t)]));`);
  res.formats = {};
  for (const e of AUDIO) {
    const o = await open(`tone.${e}`);
    await sleep(800);
    res.formats[e] = await ev(`${S} const a = S.root().querySelector('audio');
      return { open: ${JSON.stringify(o)}, viewType: S.leaf().view.getViewType(), audio: a ? { attrs: Array.from(a.attributes).map((x) => [x.name, x.value.slice(0, 60)]), duration: a.duration, readyState: a.readyState, error: a.error && a.error.code, rect: S.rect(a), styles: S.styles(a) } : null,
        content: S.skeleton(S.root().querySelector('.view-content'), 0, 5) };`);
    if (e === 'mp3') res.shot = await shot('audio-tab-mp3');
  }
  res.leafState = await ev(`${S} return S.leaf().getViewState();`);
  res.headerTitle = await ev(`${S} const t = S.root().querySelector('.view-header-title'); return t ? t.textContent : null;`);
  res.actions = await ev(`${S} return Array.from(S.root().querySelectorAll('.view-actions .clickable-icon')).map((b) => b.getAttribute('aria-label'));`);
  return res;
});

await step('s07-audio-embed', async () => {
  const res = {};
  await open('Audio.md', 'preview');
  await sleep(2500);
  res.reading = await ev(`${S} return S.embeds().map((e) => ({ src: e.getAttribute('src'), cls: S.cls(e), rect: S.rect(e), width: e.getAttribute('width'), skel: S.skeleton(e, 0, 3), audio: (() => { const a = e.querySelector('audio'); return a ? { duration: a.duration, error: a.error && a.error.code, rect: S.rect(a), attrs: Array.from(a.attributes).map((x) => [x.name, x.value.slice(0, 60)]) } : null; })() }));`);
  res.readingShot = await shot('audio-reading');
  // Keys on a focused player.
  const first = res.reading.find((x) => x.audio);
  if (first) {
    const [x, y, w, h] = first.audio.rect;
    await c.clickAt(x + 20, y + h / 2); await sleep(600); // the play button
    res.afterPlayClick = await ev(`${S} const a = S.embeds().map((e) => e.querySelector('audio')).filter(Boolean)[0]; return { paused: a.paused, t: a.currentTime, active: document.activeElement && document.activeElement.tagName };`);
    await sleep(800);
    await c.send('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32, text: ' ' }); await c.send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 }); await sleep(400);
    res.afterSpace = await ev(`${S} const a = S.embeds().map((e) => e.querySelector('audio')).filter(Boolean)[0]; return { paused: a.paused, t: a.currentTime };`);
    for (const k of ['ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'k', 'm']) {
      const before = await ev(`${S} const a = S.embeds().map((e) => e.querySelector('audio')).filter(Boolean)[0]; return { t: a.currentTime, v: a.volume, muted: a.muted, paused: a.paused };`);
      await c.pressKey(k); await sleep(300);
      const after = await ev(`${S} const a = S.embeds().map((e) => e.querySelector('audio')).filter(Boolean)[0]; return { t: a.currentTime, v: a.volume, muted: a.muted, paused: a.paused, active: document.activeElement && document.activeElement.tagName };`);
      res['key_' + k] = { before, after };
    }
    await ev(`${S} S.embeds().map((e) => e.querySelector('audio')).filter(Boolean).forEach((a) => a.pause()); return true;`);
  }
  await open('Audio.md', 'live');
  await ev(`${S} (S.leaf().view.editor || app.workspace.activeEditor.editor).setCursor({ line: 0, ch: 0 }); return true;`);
  await sleep(2000);
  res.live = await ev(`${S} return Array.from(S.root().querySelectorAll('.internal-embed')).map((e) => ({ src: e.getAttribute('src'), cls: S.cls(e), rect: S.rect(e), parent: S.cls(e.parentElement), audio: !!e.querySelector('audio') }));`);
  res.liveShot = await shot('audio-live');
  const lf = res.live.find((x) => x.audio);
  if (lf) {
    const [x, y, w, h] = lf.rect;
    await c.clickAt(x + w - 10, y + h / 2); await sleep(600);
    res.liveClickEnd = { cursor: await ev(`${S} return (S.leaf().view.editor || app.workspace.activeEditor.editor).getCursor();`), paused: await ev(`${S} const a = S.root().querySelector('audio'); return a && a.paused;`) };
    await ev(`${S} S.root().querySelectorAll('audio').forEach((a) => a.pause()); return true;`);
  }
  return res;
});

await step('s08-video', async () => {
  const res = { tabs: {} };
  for (const e of VIDEO) {
    const o = await open(`clip.${e}`);
    await sleep(1500);
    res.tabs[e] = await ev(`${S} const v = S.root().querySelector('video');
      return { open: ${JSON.stringify(o)}, viewType: S.leaf().view.getViewType(), video: v ? { attrs: Array.from(v.attributes).map((x) => [x.name, x.value.slice(0, 60)]), duration: v.duration, w: v.videoWidth, h: v.videoHeight, readyState: v.readyState, error: v.error && v.error.code, rect: S.rect(v), styles: S.styles(v) } : null,
        content: S.skeleton(S.root().querySelector('.view-content'), 0, 4) };`);
    if (e === 'mp4' || e === 'webm') res['shot_' + e] = await shot('video-tab-' + e);
  }
  await open('Video.md', 'preview');
  await sleep(3000);
  res.reading = await ev(`${S} return S.embeds().map((e) => ({ src: e.getAttribute('src'), cls: S.cls(e), rect: S.rect(e), width: e.getAttribute('width'), height: e.getAttribute('height'), skel: S.skeleton(e, 0, 2), video: (() => { const v = e.querySelector('video'); return v ? { duration: v.duration, w: v.videoWidth, h: v.videoHeight, error: v.error && v.error.code, rect: S.rect(v), attrs: Array.from(v.attributes).map((x) => [x.name, x.value.slice(0, 60)]) } : null; })() }));`);
  res.readingShot = await shot('video-reading');
  await open('Video.md', 'live');
  await ev(`${S} (S.leaf().view.editor || app.workspace.activeEditor.editor).setCursor({ line: 0, ch: 0 }); return true;`);
  await sleep(2000);
  res.live = await ev(`${S} return Array.from(S.root().querySelectorAll('.internal-embed')).map((e) => ({ src: e.getAttribute('src'), cls: S.cls(e), rect: S.rect(e), video: !!e.querySelector('video') }));`);
  res.liveShot = await shot('video-live');
  return res;
});

await step('s09-leaf-json', async () => {
  await open('tone.mp3');
  await sleep(500);
  await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); await app.workspace.saveLayout && app.workspace.saveLayout(); return true;`);
  await sleep(1500);
  const a = bytes('.obsidian/workspace.json');
  await open('clip.mp4');
  await sleep(500);
  await ev(`await app.workspace.saveLayout && app.workspace.saveLayout(); return true;`);
  await sleep(1500);
  const v = bytes('.obsidian/workspace.json');
  const leafOf = (txt) => { try { const j = JSON.parse(txt); const leaves = []; const walk = (n) => { if (!n) return; if (n.type === 'leaf') leaves.push(n.state); (n.children || []).forEach(walk); }; walk(j.main); return leaves; } catch (e) { return 'ERR ' + e; } };
  return { audio: leafOf(a), video: leafOf(v) };
});

c.close();
process.exit(0);
