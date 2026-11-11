// Page preview: the hover popover, under real pointer and key events
// (Input.dispatchMouseEvent / dispatchKeyEvent), timed by a MutationObserver
// in the renderer rather than by polling over the socket.
//
//   s00  context: version, platform, the plugin's options, hover sources
//   s01  reading view, [[Beta]] with no modifier (Reading view is the one
//        surface that does not need Mod by default): the delay, markup,
//        geometry, computed styles, what the popover holds; a screenshot
//   s02  leaving: pointer to empty space (the hide delay), pointer from the
//        link into the popover and out of it, a second link while one is up
//   s03  subpaths and targets: #Second, #^blk, an alias, a Markdown link,
//        [[Missing]], an image, a canvas, a base, an external link, a tag
//   s04  placement: a link near the bottom and near the right edge
//   s05  Live Preview: no modifier, Mod held before the move, Mod pressed
//        while hovering, Mod released while the popover is up
//   s06  source mode: the same
//   s07  the settings: each key flipped at runtime and the hover retried
//   s08  inside the popover: wheel, click, typing, a link clicked, a nested
//        hover, Escape
//   s09  the other surfaces with and without Mod: Files, tab header,
//        Backlinks, Outgoing links, Search, Outline, Bookmarks, Properties,
//        Graph is skipped (canvas pixels)
//   s10  workspace.json and the vault: what a hover writes
//   s11  the sources s09 left: a link-valued property in File properties,
//        a file cell in a base's table, a graph node (found through the
//        renderer); a second link above the first while its popover is up;
//        a click on a missing note's popover
//
// `SHOTS` names a folder for PNGs. `ONLY_STEPS=s03` re-runs a step.
// Phases: fixtures | run   (node page-preview.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.pagePreview = all.pagePreview || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && process.env.ONLY_STEPS && all.pagePreview.run) {
  const prev = all.pagePreview.run;
  Object.assign(out, { steps: prev.steps || {}, order: prev.order || [] });
}
all.pagePreview[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const ONLY = process.env.ONLY_STEPS && phase === 'run' ? process.env.ONLY_STEPS.split(',') : null;
const STEP_MS = Number(process.env.STEP_MS || 120000);
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), STEP_MS); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[preview] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
// A 40 x 30 solid PNG, so the image target has a size to report.
import zlib from 'node:zlib';
function png(w, h) {
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type), data]);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(zlib.crc32(td));
    return Buffer.concat([len, td, crc]);
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  const row = Buffer.concat([Buffer.from([0]), Buffer.alloc(w * 3, 0x80)]);
  const raw = Buffer.concat(Array.from({ length: h }, () => row));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('Folder'), { recursive: true });
  const filler = (n, w) => Array.from({ length: n }, (_, i) => `${w} line ${i + 1}, some words to fill the width of the pane a little.`).join('\n\n');
  fs.writeFileSync(vp('Alpha.md'), [
    '# Alpha', '',
    'Plain [[Beta]] link.', '',
    'Heading [[Beta#Second]] link.', '',
    'Block [[Beta#^blk]] link.', '',
    'Alias [[Beta|the beta]] link.', '',
    'Markdown [md beta](Beta.md) link.', '',
    'Missing [[Missing]] link.', '',
    'Image [[Pic.png]] link.', '',
    'Canvas [[Board.canvas]] link.', '',
    'Base [[Table.base]] link.', '',
    'Long [[Long]] link.', '',
    'Short [[Folder/Gamma|Gamma]] link.', '',
    'External [site](https://example.com) link.', '',
    'Tag #topic here.', '',
    '## Lower', '',
    filler(40, 'Alpha'), '',
    'Bottom [[Beta]] link at the end.', '',
  ].join('\n'));
  fs.writeFileSync(vp('Beta.md'), [
    '---', 'tags: [one]', 'status: draft', '---', '# Beta', '',
    'Beta first paragraph with a link to [[Folder/Gamma|Gamma]].', '',
    filler(12, 'Beta'), '',
    '## Second', '',
    'Text under the second heading.', '',
    filler(12, 'Second'), '',
    'A block paragraph. ^blk', '',
    filler(12, 'After'), '',
  ].join('\n'));
  fs.writeFileSync(vp('Props.md'), '---\nrelated: "[[Beta]]"\n---\n# Props\n\nA note with a link-valued property.\n');
  fs.writeFileSync(vp('Long.md'), '# Long\n\n' + filler(200, 'Long') + '\n');
  fs.writeFileSync(vp('Folder/Gamma.md'), '# Gamma\n\nShort. See [[Alpha]].\n');
  fs.writeFileSync(vp('Pic.png'), png(40, 30));
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify({ nodes: [{ id: 'a1', type: 'text', text: 'Hello', x: 0, y: 0, width: 250, height: 60 }], edges: [] }, null, '\t'));
  fs.writeFileSync(vp('Table.base'), 'views:\n  - type: table\n    name: Table\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/bookmarks.json'), JSON.stringify({ items: [{ type: 'file', ctime: 1792373000000, path: 'Beta.md' }] }, null, 2));
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    outline: true, canvas: true, bases: true, bookmarks: true, 'command-palette': true, 'file-recovery': true,
    'page-preview': true, 'tag-pane': true, properties: true, graph: true,
  }, null, 2));
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

const HELPERS = `
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  r: (e) => { if (!e) return null; const b = e.getBoundingClientRect(); return { x: Math.round(b.x * 10) / 10, y: Math.round(b.y * 10) / 10, w: Math.round(b.width * 10) / 10, h: Math.round(b.height * 10) / 10 }; },
  log: [],
  t0: 0,
  // Every popover added or removed, and every mouseover/mouseout/key on the
  // document, stamped with performance.now(): the delays are read off this.
  install() {
    if (this.obs) return;
    this.obs = new MutationObserver((ms) => {
      const now = performance.now();
      for (const m of ms) {
        for (const n of m.addedNodes) if (n.nodeType === 1 && n.classList.contains('popover')) this.log.push({ t: now, ev: 'add', cls: this.cls(n) });
        for (const n of m.removedNodes) if (n.nodeType === 1 && n.classList.contains('popover')) this.log.push({ t: now, ev: 'remove', cls: this.cls(n) });
      }
    });
    this.obs.observe(document.body, { childList: true, subtree: true });
    for (const type of ['mouseover', 'mouseout', 'keydown', 'keyup']) {
      document.addEventListener(type, (e) => {
        const tg = e.target;
        this.log.push({ t: performance.now(), ev: type, key: e.key, mod: { ctrl: e.ctrlKey, meta: e.metaKey }, tgt: tg && tg.nodeType === 1 ? (tg.tagName.toLowerCase() + '.' + (this.cls(tg) || '')).slice(0, 80) : null });
      }, true);
    }
  },
  mark() { this.log = []; this.t0 = performance.now(); return true; },
  events() { return this.log.map((x) => ({ ...x, t: Math.round(x.t - this.t0) })); },
  popovers() {
    return Array.from(document.querySelectorAll('.popover')).map((p) => {
      const cs = getComputedStyle(p);
      const content = p.querySelector('.markdown-embed-content, .popover-content, .view-content') || p.firstElementChild;
      const scroller = p.querySelector('.markdown-preview-view, .cm-scroller, .view-content');
      return {
        cls: this.cls(p), rect: this.r(p),
        style: { width: cs.width, height: cs.height, maxHeight: cs.maxHeight, maxWidth: cs.maxWidth, minHeight: cs.minHeight, padding: cs.padding,
          border: cs.border, borderRadius: cs.borderRadius, boxShadow: cs.boxShadow, background: cs.backgroundColor, zIndex: cs.zIndex,
          position: cs.position, overflow: cs.overflow, fontSize: cs.fontSize },
        tree: this.tree(p, 5),
        text: (p.innerText || '').slice(0, 400),
        scroller: scroller ? { cls: this.cls(scroller), rect: this.r(scroller), scrollTop: scroller.scrollTop, scrollHeight: scroller.scrollHeight, clientHeight: scroller.clientHeight,
          padding: getComputedStyle(scroller).padding, fontSize: getComputedStyle(scroller).fontSize } : null,
        contentCls: content ? this.cls(content) : null,
        editable: !!p.querySelector('[contenteditable=true]'),
        inlineTitle: !!p.querySelector('.inline-title'),
        properties: !!p.querySelector('.metadata-container'),
        embedTitle: (p.querySelector('.markdown-embed-title') || {}).textContent || null,
        link: !!p.querySelector('.markdown-embed-link'),
        firstVisible: this.firstVisible(p),
      };
    });
  },
  // The first block whose top is inside the popover's scroller: which part of
  // the note the popover opens on.
  firstVisible(p) {
    const sc = p.querySelector('.markdown-preview-view, .cm-scroller'); if (!sc) return null;
    const top = sc.getBoundingClientRect().top;
    const blocks = Array.from(sc.querySelectorAll('.markdown-preview-sizer > div, .cm-line'));
    const b = blocks.find((e) => e.getBoundingClientRect().bottom > top + 2);
    return b ? { text: (b.textContent || '').trim().slice(0, 80), cls: this.cls(b), dy: Math.round(b.getBoundingClientRect().top - top),
      highlighted: !!sc.querySelector('.is-flashing, .mod-flashing, .is-highlighted') } : null;
  },
  tree(e, depth) {
    if (!e || depth < 0) return null;
    const kids = Array.from(e.children).slice(0, 6).map((k) => this.tree(k, depth - 1)).filter(Boolean);
    const o = { tag: e.tagName.toLowerCase(), cls: this.cls(e) };
    if (kids.length) o.kids = kids;
    if (e.children.length > 6) o.more = e.children.length - 6;
    return o;
  },
  // The rect of the first element under root whose own text is exactly t.
  findText(root, sel, t, nth) {
    root = typeof root === 'string' ? document.querySelector(root) : root; if (!root) return null;
    const els = Array.from(root.querySelectorAll(sel)).filter((e) => (e.textContent || '').trim() === t);
    const e = els[nth || 0]; if (!e) return null;
    e.scrollIntoView({ block: 'nearest' });
    const b = e.getBoundingClientRect();
    return { x: b.x, y: b.y, w: b.width, h: b.height, cx: b.x + Math.min(b.width / 2, 20), cy: b.y + b.height / 2, cls: this.cls(e), tag: e.tagName.toLowerCase(), href: e.getAttribute('data-href') || e.getAttribute('href') };
  },
  async open(file, mode, source) {
    const leaf = app.workspace.getLeaf(false);
    await leaf.openFile(app.vault.getAbstractFileByPath(file), { state: { mode, source: !!source } });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(600);
    const v = leaf.view;
    if (mode === 'source' && v.editor) { v.editor.setCursor({ line: v.editor.lineCount() - 1, ch: 0 }); }
    await this.sleep(300);
    return { type: v.getViewType(), mode: v.getMode ? v.getMode() : null, file: v.file && v.file.path };
  },
  closePopovers() {
    for (const p of Array.from(document.querySelectorAll('.popover'))) { try { p.remove(); } catch {} }
    const hp = app.internalPlugins.getPluginById('page-preview');
    return true;
  },
  ui() {
    return { leaves: (() => { const o = []; app.workspace.iterateRootLeaves((l) => o.push({ type: l.view.getViewType(), file: l.view.file ? l.view.file.path : null, active: app.workspace.activeLeaf === l })); return o; })(),
      focus: document.activeElement ? document.activeElement.tagName.toLowerCase() + '.' + this.cls(document.activeElement) : null,
      win: { w: window.innerWidth, h: window.innerHeight } };
  },
};
window.__p.install();
return true;`;
await c.evaluate(HELPERS);
const ev = (s) => c.evaluate(s);

const isMac = await ev(`return document.body.classList.contains('mod-macos');`);
// The platform's Mod: Cmd on macOS, Ctrl elsewhere.
const MODS = isMac ? { meta: true } : { ctrl: true };
const MODKEY = isMac ? { key: 'Meta', code: 'MetaLeft', vk: 91, bit: 4 } : { key: 'Control', code: 'ControlLeft', vk: 17, bit: 2 };
const modDown = () => c.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: MODKEY.key, code: MODKEY.code, windowsVirtualKeyCode: MODKEY.vk, modifiers: MODKEY.bit });
const modUp = () => c.send('Input.dispatchKeyEvent', { type: 'keyUp', key: MODKEY.key, code: MODKEY.code, windowsVirtualKeyCode: MODKEY.vk, modifiers: 0 });
const move = (x, y, mods) => c.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(x), y: Math.round(y), modifiers: mods ? (mods.meta ? 4 : 0) | (mods.ctrl ? 2 : 0) : 0 });
const park = async () => { await move(5, 300); await sleep(100); };

async function shot(name) {
  if (!SHOTS) return null;
  fs.mkdirSync(SHOTS, { recursive: true });
  const { data } = await c.send('Page.captureScreenshot', { format: 'png' });
  const f = path.join(SHOTS, `page-preview-${name}.png`);
  fs.writeFileSync(f, Buffer.from(data, 'base64'));
  return f;
}

// Wait for every popover to go, so one case does not leak into the next.
async function clear() {
  await park();
  for (let i = 0; i < 30; i++) { if (!(await ev(`return document.querySelectorAll('.popover').length;`))) return true; await sleep(100); }
  await ev(`return window.__p.closePopovers();`);
  await sleep(200);
  return false;
}

// Hover a point (optionally with Mod in the move's modifiers), wait `waitMs`,
// return the event log and the popovers then.
async function hoverAt(r, { mods = null, waitMs = 1500 } = {}) {
  await clear();
  await ev(`return window.__p.mark();`);
  // Two moves: an enter from just outside, then onto the link, as a pointer arrives.
  await move(r.cx - 1, r.cy, mods);
  await move(r.cx, r.cy, mods);
  await sleep(waitMs);
  return ev(`return { events: window.__p.events(), popovers: window.__p.popovers() };`);
}
const addAt = (res) => { const a = res.events.find((e) => e.ev === 'add'); return a ? a.t : null; };
const removeAt = (res) => { const a = res.events.find((e) => e.ev === 'remove'); return a ? a.t : null; };
const READ = '.workspace-leaf.mod-active .markdown-reading-view';
const LP = '.workspace-leaf.mod-active .markdown-source-view';
const linkIn = (root, text, nth = 0) => ev(`return window.__p.findText(${JSON.stringify(root)}, 'a.internal-link, a.external-link, a.tag, .cm-hmd-internal-link, .cm-link, .cm-url, .cm-hashtag, .cm-underline', ${JSON.stringify(text)}, ${nth});`);

await step('s00-context', async () => ev(`
  const pp = app.internalPlugins.getPluginById('page-preview');
  return {
    version: (window.require && window.require('electron').ipcRenderer.sendSync('version')) || null,
    platform: document.body.className.match(/mod-(windows|macos|linux)/)[0],
    enabled: !!(pp && pp.enabled),
    options: pp && pp.instance ? JSON.parse(JSON.stringify(pp.instance.overrides || pp.instance.options || null)) : null,
    instanceKeys: pp && pp.instance ? Object.keys(pp.instance) : null,
    instanceProto: pp && pp.instance ? Object.getOwnPropertyNames(Object.getPrototypeOf(pp.instance)) : null,
    instanceValues: pp && pp.instance ? Object.fromEntries(Object.entries(pp.instance).filter(([k, v]) => v === null || ['string', 'number', 'boolean'].includes(typeof v) || (typeof v === 'object' && !(v instanceof Node) && Object.keys(v).length < 20)).map(([k, v]) => { try { return [k, JSON.parse(JSON.stringify(v))]; } catch { return [k, String(v)]; } })) : null,
    sources: app.workspace.hoverLinkSources ? JSON.parse(JSON.stringify(app.workspace.hoverLinkSources)) : null,
    win: { w: window.innerWidth, h: window.innerHeight, dpr: window.devicePixelRatio },
  };`));

await step('s01-reading-basic', async () => {
  const opened = await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const r = await linkIn(READ, 'Beta');
  const res = await hoverAt(r, { waitMs: 2000 });
  const s = await shot('s01-reading');
  const vars = await ev(`
    const cs = getComputedStyle(document.body);
    const names = ['--popover-width', '--popover-height', '--popover-max-height', '--popover-font-size', '--popover-pdf-width', '--popover-pdf-height',
      '--background-primary', '--background-modifier-border', '--shadow-s', '--shadow-l', '--radius-m', '--radius-l', '--layer-popover'];
    return Object.fromEntries(names.map((n) => [n, cs.getPropertyValue(n).trim()]));`);
  return { opened, link: r, delayMs: addAt(res), ...res, vars, shot: s };
});

await step('s02-leaving', async (partial) => {
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const r = await linkIn(READ, 'Beta');
  // (a) away to empty space after the popover is up
  const a = await hoverAt(r, { waitMs: 1500 });
  await ev(`return window.__p.mark();`);
  await move(r.cx, r.cy + 400); await sleep(50); await park();
  await sleep(1500);
  const awayEv = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().length };`);
  partial.away = awayEv;
  // (b) from the link into the popover, sit there, then out
  const b = await hoverAt(r, { waitMs: 1500 });
  const pr = b.popovers[0] && b.popovers[0].rect;
  let into = null, out_ = null;
  if (pr) {
    await ev(`return window.__p.mark();`);
    const steps = 8;
    const tx = pr.x + pr.w / 2, ty = pr.y + Math.min(40, pr.h / 2);
    for (let i = 1; i <= steps; i++) { await move(r.cx + ((tx - r.cx) * i) / steps, r.cy + ((ty - r.cy) * i) / steps); await sleep(30); }
    await sleep(1500);
    into = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().length };`);
    await ev(`return window.__p.mark();`);
    await move(pr.x + pr.w + 200 < 1400 ? pr.x + pr.w + 150 : pr.x - 150, ty); await sleep(1500);
    out_ = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().length };`);
  }
  // (c) a quick pass over the link that leaves before the delay
  await clear();
  await ev(`return window.__p.mark();`);
  await move(r.cx, r.cy); await sleep(150); await park(); await sleep(1200);
  const quick = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().length };`);
  // (d) a second link while the first popover is up
  const g = await linkIn(READ, 'Gamma');
  const d1 = await hoverAt(r, { waitMs: 1500 });
  await ev(`return window.__p.mark();`);
  await move(g.cx, g.cy); await sleep(2000);
  const second = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().map((p) => ({ rect: p.rect, text: p.text.slice(0, 60) })) };`);
  return { first: { delayMs: addAt(a) }, away: awayEv, hideDelayMs: removeAt(awayEv), into, out: out_, outHideMs: out_ ? removeAt(out_) : null, quick, second, firstOf2: d1.popovers.length };
});

await step('s03-targets', async (partial) => {
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const cases = [
    ['heading', 'Beta > Second'], ['block', 'Beta > ^blk'], ['alias', 'the beta'], ['markdown', 'md beta'],
    ['missing', 'Missing'], ['image', 'Pic.png'], ['canvas', 'Board.canvas'], ['base', 'Table.base'],
    ['long', 'Long'], ['short', 'Gamma'], ['external', 'site'], ['tag', '#topic'],
  ];
  const o = {};
  for (const [k, t] of cases) {
    const r = await linkIn(READ, t);
    if (!r) { o[k] = { missingLink: true }; continue; }
    const res = await hoverAt(r, { waitMs: 2000 });
    o[k] = { link: r, delayMs: addAt(res), popovers: res.popovers };
    if (res.popovers.length) await shot('s03-' + k);
    partial[k] = o[k];
  }
  return o;
});

await step('s04-placement', async () => {
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const o = {};
  // Near the bottom: scroll so the last Beta link sits near the window's bottom.
  o.bottom = await (async () => {
    await ev(`
      const els = Array.from(document.querySelectorAll('${READ} a.internal-link')).filter((e) => e.textContent.trim() === 'Beta');
      const e = els[els.length - 1]; e.scrollIntoView({ block: 'end' }); return true;`);
    await sleep(400);
    const r = await ev(`
      const els = Array.from(document.querySelectorAll('${READ} a.internal-link')).filter((e) => e.textContent.trim() === 'Beta');
      const e = els[els.length - 1]; const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height, cx: b.x + 10, cy: b.y + b.height / 2 };`);
    const res = await hoverAt(r, { waitMs: 2000 });
    await shot('s04-bottom');
    return { link: r, popovers: res.popovers.map((p) => p.rect), win: await ev(`return window.__p.ui().win;`) };
  })();
  // At the top of the pane.
  o.top = await (async () => {
    await ev(`document.querySelector('${READ} .markdown-preview-view').scrollTop = 0; return true;`);
    await sleep(300);
    const r = await linkIn(READ, 'Beta');
    const res = await hoverAt(r, { waitMs: 2000 });
    return { link: r, popovers: res.popovers.map((p) => p.rect) };
  })();
  // Near the right edge: shrink the right sidebar away and make the pane's
  // readable line width off so the link can sit far right.
  o.right = await (async () => {
    await ev(`app.workspace.rightSplit.collapse(); app.workspace.leftSplit.collapse(); return true;`);
    await sleep(400);
    const r0 = await linkIn(READ, 'Beta');
    // Fake a right-edge anchor: a temporary absolutely placed internal link in the pane.
    const r = await ev(`
      const host = document.querySelector('${READ} .markdown-preview-sizer');
      const a = document.createElement('a'); a.className = 'internal-link'; a.setAttribute('data-href', 'Beta'); a.setAttribute('href', 'Beta'); a.textContent = 'EdgeBeta';
      a.id = '__edge'; a.style.position = 'fixed'; a.style.right = '8px'; a.style.top = '200px';
      host.appendChild(a);
      const b = a.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height, cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    const res = await hoverAt(r, { waitMs: 2000 });
    await shot('s04-right');
    await ev(`const a = document.getElementById('__edge'); if (a) a.remove(); return true;`);
    return { normalLink: r0, link: r, popovers: res.popovers.map((p) => p.rect), note: 'a fixed-position a.internal-link injected into the sizer; whether the reading view handles hover on injected anchors is part of the answer' };
  })();
  return o;
});

async function editorCases(source) {
  const opened = await ev(`return window.__p.open('Alpha.md', 'source', ${source});`);
  const o = { opened };
  const r = await linkIn(LP, 'Beta');
  o.link = r;
  if (!r) { o.linkTree = await ev(`return window.__p.tree(document.querySelector('${LP} .cm-line:nth-child(3)'), 4);`); return o; }
  o.plain = await hoverAt(r, { waitMs: 2000 });
  o.plain.delayMs = addAt(o.plain);
  // Mod held in the move's own modifiers (and the key down first).
  await clear(); await modDown();
  o.modBefore = await hoverAt(r, { mods: MODS, waitMs: 2000 });
  o.modBefore.delayMs = addAt(o.modBefore);
  await shot((source ? 's06-source' : 's05-lp') + '-mod');
  // Releasing Mod with the popover up.
  await ev(`return window.__p.mark();`);
  await modUp(); await sleep(1500);
  o.modReleased = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().length };`);
  // Hover first, Mod pressed after.
  await clear();
  await ev(`return window.__p.mark();`);
  await move(r.cx - 1, r.cy); await move(r.cx, r.cy); await sleep(800);
  await modDown(); await sleep(2000);
  o.modAfter = await ev(`return { events: window.__p.events(), popovers: window.__p.popovers().map((p) => ({ rect: p.rect, cls: p.cls })) };`);
  o.modAfter.delayFromKeyMs = (() => { const k = o.modAfter.events.find((e) => e.ev === 'keydown'); const a = o.modAfter.events.find((e) => e.ev === 'add'); return k && a ? a.t - k.t : null; })();
  await modUp();
  // The other modifier on macOS (Ctrl) or Meta elsewhere.
  const other = isMac ? { ctrl: true } : { meta: true };
  o.otherMod = await hoverAt(r, { mods: other, waitMs: 2000 });
  o.otherMod = { mods: other, popovers: o.otherMod.popovers.length };
  // Cursor on the link's line (Live Preview shows the source there).
  if (!source) {
    await ev(`const v = app.workspace.activeLeaf.view; v.editor.setCursor({ line: 2, ch: 3 }); v.editor.focus(); return true;`);
    await sleep(400);
    const r2 = await linkIn(LP, 'Beta');
    await modDown();
    o.onCursorLine = await hoverAt(r2 || r, { mods: MODS, waitMs: 2000 });
    o.onCursorLine = { link: r2, popovers: o.onCursorLine.popovers.length };
    await modUp();
  }
  // A heading link from the editor, with Mod.
  const rh = await linkIn(LP, 'Beta#Second');
  if (rh) { await modDown(); const h = await hoverAt(rh, { mods: MODS, waitMs: 2000 }); await modUp(); o.heading = { link: rh, popovers: h.popovers }; }
  return o;
}
await step('s05-live-preview', () => editorCases(false));
await step('s06-source', () => editorCases(true));

await step('s07-settings', async () => {
  const pp = `app.internalPlugins.getPluginById('page-preview').instance`;
  const o = {};
  o.before = await ev(`const i = ${pp}; return JSON.parse(JSON.stringify(i.options || null));`);
  // Reading view requiring Mod.
  await ev(`const i = ${pp}; i.options = Object.assign({}, i.options, { preview: true }); return true;`);
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  let r = await linkIn(READ, 'Beta');
  o.readingRequireMod_plain = (await hoverAt(r, { waitMs: 2000 })).popovers.length;
  await modDown(); o.readingRequireMod_mod = (await hoverAt(r, { mods: MODS, waitMs: 2000 })).popovers.length; await modUp();
  // Editing view not requiring Mod.
  await ev(`const i = ${pp}; i.options = Object.assign({}, i.options, { preview: false, editor: false }); return true;`);
  await ev(`return window.__p.open('Alpha.md', 'source', false);`);
  r = await linkIn(LP, 'Beta');
  o.editorNoMod_plain = r ? (await hoverAt(r, { waitMs: 2000 })).popovers.length : 'no link';
  // Plugin disabled: does a Mod hover do anything?
  await ev(`const i = ${pp}; i.options = Object.assign({}, i.options, { editor: true }); return true;`);
  await ev(`await app.internalPlugins.getPluginById('page-preview').disable(); return true;`);
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  r = await linkIn(READ, 'Beta');
  o.disabled_plain = (await hoverAt(r, { waitMs: 2000 })).popovers.length;
  await modDown(); o.disabled_mod = (await hoverAt(r, { mods: MODS, waitMs: 2000 })).popovers.length; await modUp();
  await ev(`await app.internalPlugins.getPluginById('page-preview').enable(); return true;`);
  await sleep(500);
  o.after = await ev(`const i = app.internalPlugins.getPluginById('page-preview').instance; i.options = {}; return JSON.parse(JSON.stringify(i.options));`);
  o.configOnDisk = (() => { try { return fs.readFileSync(vp('.obsidian/page-preview.json'), 'utf8'); } catch { return null; } })();
  return o;
});

await step('s08-inside', async () => {
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const r = await linkIn(READ, 'Beta');
  const before = fs.readFileSync(vp('Beta.md'), 'utf8');
  const up = await hoverAt(r, { waitMs: 1500 });
  const p = up.popovers[0];
  if (!p) return { noPopover: true };
  const o = { popover: p.rect };
  // A nested hover on the reading popover's own Gamma link (no Mod: it is a reading view).
  const g = await ev(`const p = document.querySelector('.popover'); return p ? window.__p.findText(p, 'a.internal-link', 'Gamma') : null;`);
  o.gammaLink = g;
  if (g) {
    for (let i = 1; i <= 6; i++) { await move(r.cx + ((g.cx - r.cx) * i) / 6, r.cy + ((g.cy - r.cy) * i) / 6); await sleep(30); }
    await ev(`return window.__p.mark();`);
    await move(g.cx - 1, g.cy); await move(g.cx, g.cy); await sleep(2000);
    o.nested = await ev(`return { events: window.__p.events().filter((e) => e.ev === 'add' || e.ev === 'remove'), popovers: window.__p.popovers().map((p) => ({ rect: p.rect, cls: p.cls, text: p.text.slice(0, 40) })) };`);
    await shot('s08-nested');
    // Off both: which goes first.
    await ev(`return window.__p.mark();`);
    await park(); await sleep(1500);
    o.nestedLeave = await ev(`return { events: window.__p.events().filter((e) => e.ev === 'add' || e.ev === 'remove'), popovers: window.__p.popovers().length };`);
    // Back up, then into the child and out the far side of the child only.
    await hoverAt(r, { waitMs: 1200 });
    const g2 = await ev(`const p = document.querySelector('.popover'); return p ? window.__p.findText(p, 'a.internal-link', 'Gamma') : null;`);
    for (let i = 1; i <= 6; i++) { await move(r.cx + ((g2.cx - r.cx) * i) / 6, r.cy + ((g2.cy - r.cy) * i) / 6); await sleep(30); }
    await sleep(1500);
    // A plain click on the nested link inside the popover.
    await c.clickAt(g2.cx, g2.cy); await sleep(1200);
    o.linkClicked = await ev(`return { popovers: window.__p.popovers().length, ui: window.__p.ui() };`);
    await clear();
    await ev(`return window.__p.open('Alpha.md', 'preview');`);
  }
  // Up again; into it; wheel.
  const up2 = await hoverAt(r, { waitMs: 1500 });
  const q = up2.popovers[0];
  const cx = q.rect.x + q.rect.w / 2, cy = q.rect.y + q.rect.h / 2;
  for (let i = 1; i <= 6; i++) { await move(r.cx + ((cx - r.cx) * i) / 6, r.cy + ((cy - r.cy) * i) / 6); await sleep(30); }
  await sleep(300);
  await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: Math.round(cx), y: Math.round(cy), deltaX: 0, deltaY: 300 });
  await sleep(500);
  o.afterWheel = await ev(`const p = window.__p.popovers()[0]; return p ? { scroller: p.scroller, mainScroll: document.querySelector('${READ} .markdown-preview-view').scrollTop } : null;`);
  // A click on text in the popover: it turns into an editor.
  await ev(`return window.__p.mark();`);
  await c.clickAt(cx, cy); await sleep(600);
  o.afterClick = await ev(`return { events: window.__p.events().filter((e) => e.ev === 'add' || e.ev === 'remove'), popovers: window.__p.popovers().map((p) => ({ cls: p.cls, rect: p.rect, editable: p.editable, tree: p.tree, scroller: p.scroller })), ui: window.__p.ui(),
    sel: (() => { const e = document.querySelector('.popover .markdown-source-view'); const l = e && app.workspace.activeEditor; return l && l.editor ? { cursor: l.editor.getCursor(), file: l.file && l.file.path } : null; })() };`);
  await shot('s08-clicked');
  // Typing, then waiting past the save delay.
  await c.typeText('Z', 40); await sleep(3500);
  const after = fs.readFileSync(vp('Beta.md'), 'utf8');
  o.afterType = { popovers: await ev(`return window.__p.popovers().length;`), betaChanged: after !== before, zLine: after.split('\n').find((l) => l.includes('Z')) || null };
  // Pointer leaves an editing popover: does it stay?
  await ev(`return window.__p.mark();`);
  await park(); await sleep(2000);
  o.editingLeave = await ev(`return { events: window.__p.events().filter((e) => e.ev === 'add' || e.ev === 'remove'), popovers: window.__p.popovers().length };`);
  // Escape.
  if (o.editingLeave.popovers) {
    await ev(`return window.__p.mark();`);
    await c.pressKey('Escape'); await sleep(800);
    o.escape = await ev(`return { popovers: window.__p.popovers().length, events: window.__p.events().filter((e) => e.ev === 'remove' || e.ev === 'keydown') };`);
  }
  // A click outside an editing popover.
  const up3 = await hoverAt(r, { waitMs: 1500 });
  if (up3.popovers[0]) {
    const p3 = up3.popovers[0].rect;
    for (let i = 1; i <= 6; i++) { await move(r.cx + ((p3.x + 100 - r.cx) * i) / 6, r.cy + ((p3.y + 100 - r.cy) * i) / 6); await sleep(30); }
    await c.clickAt(p3.x + 100, p3.y + 100); await sleep(500);
    await ev(`return window.__p.mark();`);
    await c.clickAt(20, 700); await sleep(800);
    o.clickOutside = await ev(`return { popovers: window.__p.popovers().length, events: window.__p.events().filter((e) => e.ev === 'remove') };`);
  }
  fs.writeFileSync(vp('Beta.md'), before);
  await clear();
  return o;
});

await step('s09-surfaces', async (partial) => {
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  await ev(`app.workspace.leftSplit.expand(); app.workspace.rightSplit.expand(); return true;`);
  await sleep(500);
  const o = {};
  const both = async (name, rectExpr) => {
    const r = await ev(rectExpr);
    if (!r) { o[name] = { noTarget: true }; partial[name] = o[name]; return; }
    const plain = await hoverAt(r, { waitMs: 1800 });
    await modDown();
    const mod = await hoverAt(r, { mods: MODS, waitMs: 1800 });
    await modUp();
    o[name] = { target: r, plain: plain.popovers.length, mod: mod.popovers.length, modDelayMs: addAt(mod), modPopover: mod.popovers[0] ? { rect: mod.popovers[0].rect, text: mod.popovers[0].text.slice(0, 80), firstVisible: mod.popovers[0].firstVisible } : null };
    if (mod.popovers.length) await shot('s09-' + name);
    partial[name] = o[name];
  };
  const rectOf = (sel, pred) => `
    const els = Array.from(document.querySelectorAll(${JSON.stringify(sel)}))${pred ? `.filter((e) => ${pred})` : ''};
    const e = els[0]; if (!e) return null; e.scrollIntoView({ block: 'nearest' });
    const b = e.getBoundingClientRect(); if (!b.width) return null;
    return { x: b.x, y: b.y, w: b.width, h: b.height, cx: b.x + Math.min(b.width / 2, 30), cy: b.y + b.height / 2, cls: window.__p.cls(e) };`;
  // Files
  await ev(`app.commands.executeCommandById('file-explorer:open'); return true;`); await sleep(500);
  await both('files', rectOf('.nav-file-title', `e.getAttribute('data-path') === 'Beta.md'`));
  // Tab header: open Beta in a second tab so a header names it.
  await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Beta.md')); const a = app.workspace.getLeavesOfType('markdown').find((x) => x.view.file && x.view.file.path === 'Alpha.md'); app.workspace.setActiveLeaf(a, { focus: true }); return true;`);
  await sleep(500);
  await both('tabHeader', rectOf('.mod-root .workspace-tab-header', `e.getAttribute('aria-label') === 'Beta'`));
  // Bookmarks
  await ev(`app.commands.executeCommandById('bookmarks:open'); return true;`); await sleep(500);
  await both('bookmarks', rectOf('.workspace-leaf-content[data-type=bookmarks] .tree-item-self', `e.textContent.trim() === 'Beta'`));
  // Backlinks of Alpha: Gamma links to it.
  await ev(`app.commands.executeCommandById('backlink:open'); return true;`); await sleep(1200);
  await both('backlinks', rectOf('.workspace-leaf-content[data-type=backlink] .tree-item-self, .workspace-leaf-content[data-type=backlink] .search-result-file-title', `/Gamma/.test(e.textContent)`));
  // Outgoing links of Alpha.
  await ev(`app.commands.executeCommandById('outgoing-links:open'); return true;`); await sleep(1200);
  await both('outgoing', rectOf('.workspace-leaf-content[data-type=outgoing-link] .tree-item-self', `e.textContent.trim().startsWith('Beta')`));
  // Outline of Alpha: the Lower heading.
  await ev(`app.commands.executeCommandById('outline:open'); return true;`); await sleep(1000);
  await both('outline', rectOf('.workspace-leaf-content[data-type=outline] .tree-item-self', `e.textContent.trim() === 'Lower'`));
  // Search for "Second".
  await ev(`app.commands.executeCommandById('global-search:open'); await window.__p.sleep(300); const l = app.workspace.getLeavesOfType('search')[0]; l.view.setQuery('Second'); return true;`);
  await sleep(1500);
  await both('search', rectOf('.workspace-leaf-content[data-type=search] .tree-item-self.search-result-file-title, .workspace-leaf-content[data-type=search] .search-result-file-title', `/Beta/.test(e.textContent)`));
  await both('searchMatch', rectOf('.workspace-leaf-content[data-type=search] .search-result-file-match', `/Second/.test(e.textContent)`));
  // Properties of Beta: a link-valued property is not in the fixture; the
  // pane's rows are recorded instead.
  o.surfacesShown = await ev(`return Array.from(document.querySelectorAll('.workspace-leaf-content')).map((e) => e.getAttribute('data-type'));`);
  return o;
});

await step('s10-writes', async () => {
  const ws = vp('.obsidian/workspace.json');
  const before = fs.existsSync(ws) ? fs.readFileSync(ws, 'utf8') : null;
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const r = await linkIn(READ, 'Beta');
  await hoverAt(r, { waitMs: 1500 });
  await ev(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout(); return true;`);
  await sleep(2500);
  const during = fs.existsSync(ws) ? fs.readFileSync(ws, 'utf8') : null;
  await clear();
  const leaves = await ev(`const o = []; app.workspace.iterateAllLeaves((l) => o.push({ type: l.view.getViewType(), inPopover: !!(l.containerEl && l.containerEl.closest('.popover')) })); return o;`);
  return { workspaceChangedByHover: before !== during, popoverLeavesInLayout: during ? /hover|popover/.test(during) : null, leaves,
    obsidianFiles: fs.readdirSync(vp('.obsidian')) };
});

await step('s11-more-surfaces', async (partial) => {
  const o = {};
  const both = async (name, r) => {
    if (!r) { o[name] = { noTarget: true }; partial[name] = o[name]; return; }
    const plain = await hoverAt(r, { waitMs: 1800 });
    await modDown();
    const mod = await hoverAt(r, { mods: MODS, waitMs: 1800 });
    await modUp();
    o[name] = { target: r, plain: plain.popovers.length, mod: mod.popovers.length, plainDelayMs: addAt(plain), modDelayMs: addAt(mod),
      popover: (mod.popovers[0] || plain.popovers[0]) ? (({ rect, text, firstVisible, cls }) => ({ rect, cls, text: text.slice(0, 120), firstVisible }))(mod.popovers[0] || plain.popovers[0]) : null };
    if (mod.popovers.length || plain.popovers.length) await shot('s11-' + name);
    partial[name] = o[name];
  };
  // The deepest element under root whose own text is exactly t.
  const leaf = (root, t) => ev(`
    const root = document.querySelector(${JSON.stringify(root)}); if (!root) return null;
    const els = Array.from(root.querySelectorAll('*')).filter((e) => (e.textContent || '').trim() === ${JSON.stringify(t)} && !Array.from(e.children).some((k) => (k.textContent || '').trim() === ${JSON.stringify(t)}));
    const e = els[0]; if (!e) return null; e.scrollIntoView({ block: 'nearest' });
    const b = e.getBoundingClientRect(); if (!b.width) return null;
    return { x: b.x, y: b.y, w: b.width, h: b.height, cx: b.x + Math.min(b.width / 2, 20), cy: b.y + b.height / 2, cls: window.__p.cls(e), tag: e.tagName.toLowerCase(),
      parents: [e.parentElement, e.parentElement && e.parentElement.parentElement].map((x) => x ? x.tagName.toLowerCase() + '.' + window.__p.cls(x) : null) };`);

  // File properties of Props: its `related` value links Beta.
  await ev(`return window.__p.open('Props.md', 'preview');`);
  await ev(`app.workspace.rightSplit.expand(); app.commands.executeCommandById('properties:open-local'); return true;`);
  await sleep(1200);
  o.propertiesView = await ev(`const l = app.workspace.getLeavesOfType('file-properties')[0]; return l ? { type: l.view.getViewType(), html: l.view.containerEl.innerHTML.slice(0, 1500) } : null;`);
  await both('properties', await leaf('.workspace-leaf-content[data-type=file-properties]', 'Beta'));
  // The same link in the note's own properties block, for comparison: the
  // reading view's source, or the properties one?
  await both('propertiesInNote', await leaf('.workspace-leaf.mod-active .markdown-reading-view .metadata-container', 'Beta'));

  // A base's table: every file is a row, its name a link.
  await ev(`return window.__p.open('Table.base');`);
  await sleep(1500);
  o.baseView = await ev(`const v = app.workspace.activeLeaf.view; return { type: v.getViewType(), sample: v.containerEl.innerText.slice(0, 300) };`);
  await both('bases', await leaf('.workspace-leaf.mod-active', 'Beta'));

  // The graph: Beta's node, put on screen through the renderer's own pan and scale.
  await ev(`app.commands.executeCommandById('graph:open'); return true;`);
  await sleep(4000);
  const g = await ev(`
    const l = app.workspace.getLeavesOfType('graph')[0]; if (!l) return null;
    app.workspace.setActiveLeaf(l, { focus: true });
    const r = l.view.renderer; const n = r.nodes.find((x) => x.id === 'Beta.md'); if (!n) return { keys: Object.keys(r) };
    const el = r.interactiveEl || r.canvas || l.view.containerEl.querySelector('canvas');
    const b = el.getBoundingClientRect();
    const dpr = window.devicePixelRatio;
    return { keys: Object.keys(r).filter((k) => !/^_/.test(k)).slice(0, 80), scale: r.scale, panX: r.panX, panY: r.panY, nx: n.x, ny: n.y, dpr, rect: { x: b.x, y: b.y, w: b.width, h: b.height },
      cands: [
        { cx: b.x + (n.x * r.scale + r.panX) / dpr, cy: b.y + (n.y * r.scale + r.panY) / dpr },
        { cx: b.x + n.x * r.scale + r.panX, cy: b.y + n.y * r.scale + r.panY },
      ] };`);
  o.graph = g;
  if (g && g.cands) {
    for (let i = 0; i < g.cands.length; i++) {
      const r = { ...g.cands[i], w: 1, h: 1 };
      await move(r.cx - 1, r.cy); await move(r.cx, r.cy); await sleep(300);
      const hovered = await ev(`const r = app.workspace.getLeavesOfType('graph')[0].view.renderer;
        return Object.entries(r).filter(([k, v]) => v && typeof v === 'object' && v.id === 'Beta.md').map(([k]) => k);`);
      o['graphCand' + i] = hovered;
      if (hovered.length) {
        await both('graph', r);
        // Off the centre, still on the node: does the popover follow the
        // pointer or stay on the node?
        await both('graphOff', { ...r, cx: r.cx + 3, cy: r.cy + 5 });
        o.graphNode = await ev(`const r = app.workspace.getLeavesOfType('graph')[0].view.renderer; const n = r.nodes.find((x) => x.id === 'Beta.md');
          return { weight: n.weight, size: n.getSize ? n.getSize() : null, keys: Object.keys(n).filter((k) => !/^_/.test(k)).slice(0, 40), nodeScale: r.nodeScale, fNodeSizeMult: r.fNodeSizeMult };`);
        break;
      }
    }
  }
  // The graph and anything it left up go before the next case.
  await ev(`for (const l of app.workspace.getLeavesOfType('graph')) l.detach(); return window.__p.closePopovers();`);
  await clear();

  // A second link above the first while the first's popover is up.
  await ev(`return window.__p.open('Alpha.md', 'preview');`);
  const lower = await linkIn(READ, 'Beta > Second');
  const upper = await linkIn(READ, 'Beta');
  const first = await hoverAt(lower, { waitMs: 1500 });
  await ev(`return window.__p.mark();`);
  for (let i = 1; i <= 5; i++) { await move(lower.cx + ((upper.cx - lower.cx) * i) / 5, lower.cy + ((upper.cy - lower.cy) * i) / 5); await sleep(30); }
  await sleep(2000);
  o.secondLink = { first: first.popovers.map((p) => ({ rect: p.rect, text: p.text.slice(0, 40) })),
    after: await ev(`return { events: window.__p.events().filter((e) => e.ev === 'add' || e.ev === 'remove'), popovers: window.__p.popovers().map((p) => ({ rect: p.rect, text: p.text.slice(0, 40) })) };`) };
  partial.secondLink = o.secondLink;

  // A click on a missing note's popover: "Click to create".
  const miss = await linkIn(READ, 'Missing');
  const mp = await hoverAt(miss, { waitMs: 1500 });
  if (mp.popovers[0]) {
    const pr = mp.popovers[0].rect;
    for (let i = 1; i <= 5; i++) { await move(miss.cx + ((pr.x + pr.w / 2 - miss.cx) * i) / 5, miss.cy + ((pr.y + pr.h / 2 - miss.cy) * i) / 5); await sleep(30); }
    await c.clickAt(pr.x + pr.w / 2, pr.y + pr.h / 2); await sleep(1500);
    o.missingClick = { created: fs.existsSync(vp('Missing.md')), createdBytes: fs.existsSync(vp('Missing.md')) ? fs.readFileSync(vp('Missing.md'), 'utf8').length : null,
      ui: await ev(`return window.__p.ui();`), popovers: await ev(`return window.__p.popovers().map((p) => ({ cls: p.cls, rect: p.rect, editable: p.editable, text: p.text.slice(0, 60) }));`) };
    await shot('s11-missing-click');
  }
  await clear();
  return o;
});

save();
c.close();
process.exit(0);
