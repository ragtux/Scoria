// slides:start: the Slides core plugin.
//
//   s00  context: version, the command's name, icon, hotkey; when the palette
//        offers it (a note in each mode, an empty tab, a canvas)
//   s01  start on `Deck.md`: what appears (a leaf? a modal? fullscreen?), the
//        DOM added to <body> and to the leaf, workspace.json, focus
//   s02  every slide: its markup and text, so the cut can be read off
//   s03  the keys: ArrowRight, ArrowLeft, Space, PageDown, PageUp, Home, End,
//        Enter, Backspace, and what each does to the slide index
//   s04  the controls: every button in the presentation, a click on each
//   s05  Escape: what closes, and where focus and the leaf end up
//   s06  the cut of tricky notes, one at a time: `---` inside a code block,
//        `***` and `___`, a setext heading underline, frontmatter only, no
//        separator at all, an empty note, a leading `---` after frontmatter
//   s07  started from reading view and from source mode
//
// Screenshots go to <results>-shots/. ONLY_STEPS=<regex> narrows the run.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
// FROM_STEP=s08 runs that step and every later one.
const FROM = process.env.FROM_STEP || null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && (ONLY || FROM) && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return null;
  if (FROM && name < FROM) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[slides] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

const DECK = [
  '---',
  'title: The deck',
  'tags: [talk]',
  '---',
  '# First slide',
  '',
  'Opening paragraph with **bold** and a [[Other]] link.',
  '',
  '---',
  '',
  '## Second slide',
  '',
  '- one',
  '- two',
  '',
  '---',
  '',
  'Third slide has no heading.',
  '',
  '```js',
  'const x = 1;',
  '```',
  '',
  '---',
  '# Fourth, no blank line around the rule',
  'Text right under.',
  '---',
  '',
  '| a | b |',
  '| - | - |',
  '| 1 | 2 |',
  '',
].join('\n');

const TRICKY = {
  'T-code.md': 'Before\n\n```\n---\n```\n\nAfter\n',
  'T-stars.md': 'One\n\n***\n\nTwo\n\n___\n\nThree\n\n- - -\n\nFour\n',
  'T-setext.md': 'A heading\n---\n\nBody\n\n---\n\nNext\n',
  'T-fm-only.md': '---\nkey: value\n---\n',
  'T-none.md': '# Only one\n\nNo separator here.\n',
  'T-empty.md': '',
  'T-fm-rule.md': '---\nkey: value\n---\n---\n\nAfter a leading rule\n\n---\n\nLast\n',
  'T-trailing.md': 'One\n\n---\n\nTwo\n\n---\n',
  'T-hr-dashes.md': 'One\n\n-----\n\nTwo\n\n  ---\n\nThree\n',
  'T-quote.md': '> quoted\n> ---\n> more\n\nAfter\n',
};

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Deck.md'), DECK);
  fs.writeFileSync(vp('Other.md'), '# Other\n\nThe other note.\n');
  for (const [k, v] of Object.entries(TRICKY)) fs.writeFileSync(vp(k), v);
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify({ nodes: [], edges: [] }));
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    outline: true, canvas: true, 'command-palette': true, 'file-recovery': true, 'page-preview': true,
    properties: true, slides: true,
  }, null, 2));
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

await c.evaluate(`
window.__t = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x * 10) / 10, Math.round(r.y * 10) / 10, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10]; },
  bodyKids() { return Array.from(document.body.children).map((e) => e.tagName.toLowerCase() + '.' + this.cls(e)); },
  skeleton(e, d = 0, max = 7) {
    if (!e) return null;
    const tag = e.tagName.toLowerCase();
    if (['path', 'line', 'circle', 'rect', 'polyline', 'polygon'].includes(tag)) return null;
    const o = { tag, cls: e.getAttribute('class') || undefined };
    for (const a of ['aria-label', 'data-index', 'role', 'tabindex', 'style', 'data-callout']) { const v = e.getAttribute(a); if (v !== null) o[a] = v; }
    if (!e.children.length) { const t = (e.textContent || '').trim(); if (t) o.text = t.slice(0, 120); }
    if (d < max && e.children.length) o.children = Array.from(e.children).map((c) => this.skeleton(c, d + 1, max)).filter(Boolean);
    return o;
  },
  async open(path, mode) {
    const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l));
    for (const l of leaves.slice(1)) l.detach();
    const leaf = leaves[0] || app.workspace.getLeaf(false);
    await leaf.openFile(app.vault.getAbstractFileByPath(path));
    if (mode) { const vs = leaf.getViewState(); vs.state.mode = mode === 'source' ? 'source' : mode; if (mode === 'source') vs.state.source = true; if (mode === 'live') { vs.state.mode = 'source'; vs.state.source = false; } await leaf.setViewState(vs); }
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(700);
    return { file: leaf.view.file && leaf.view.file.path, state: leaf.getViewState().state };
  },
  offered(id) { return app.commands.listCommands().some((x) => x.id === id); },
  ui() {
    return {
      bodyCls: document.body.className,
      bodyKids: this.bodyKids(),
      fullscreen: document.fullscreenElement ? document.fullscreenElement.tagName.toLowerCase() + '.' + this.cls(document.fullscreenElement) : null,
      focus: document.activeElement ? document.activeElement.tagName.toLowerCase() + '.' + this.cls(document.activeElement) : null,
      modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => this.cls(m)),
      notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent.trim()),
      leaves: (() => { const o = []; app.workspace.iterateAllLeaves((l) => o.push({ type: l.view.getViewType(), file: l.view.file ? l.view.file.path : null, active: app.workspace.activeLeaf === l })); return o; })(),
      slideEls: Array.from(document.querySelectorAll('[class*=slide]')).map((e) => ({ cls: this.cls(e), rect: this.rect(e) })).slice(0, 40),
      win: [innerWidth, innerHeight],
    };
  },
  // The visible/current slide: any element whose class names a slide and is shown.
  slideState() {
    const els = Array.from(document.querySelectorAll('[class*=slide]'));
    return els.map((e) => ({ cls: this.cls(e), rect: this.rect(e), display: getComputedStyle(e).display, visibility: getComputedStyle(e).visibility, opacity: getComputedStyle(e).opacity, text: (e.innerText || '').trim().slice(0, 200) })).slice(0, 60);
  },
};
return true;`);
const ev = (s) => c.evaluate(s);
const T = 'const T = window.__t;';
let shotN = 0;
async function shot(name) {
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
const start = async () => { const r = await ev(`return app.commands.executeCommandById('slides:start');`); await sleep(1500); return r; };
const stop = async () => { await c.pressKey('Escape'); await sleep(800); };

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio };`);
  const cmd = await ev(`const x = app.commands.commands['slides:start']; return x ? { name: x.name, icon: x.icon || null, keys: (app.hotkeyManager.defaultKeys || {})['slides:start'] || null, hasCheck: !!x.checkCallback, hasCallback: !!x.callback, hasEditor: !!(x.editorCallback || x.editorCheckCallback) } : null;`);
  const plugin = await ev(`const p = app.internalPlugins.getPluginById('slides'); return p ? { enabled: p.enabled, name: p.instance && p.instance.name, desc: p.instance && p.instance.description, options: p.instance && p.instance.options ? JSON.parse(JSON.stringify(p.instance.options)) : null, ribbon: Array.from(document.querySelectorAll('.side-dock-ribbon-action')).map((e) => e.getAttribute('aria-label')) } : null;`);
  const offered = {};
  for (const mode of ['live', 'source', 'preview']) offered['Deck.md:' + mode] = await ev(`${T} await T.open('Deck.md', '${mode}'); return T.offered('slides:start');`);
  offered.canvas = await ev(`${T} await T.open('Board.canvas'); return T.offered('slides:start');`);
  offered.empty = await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l)); await ls[0].setViewState({ type: 'empty', state: {} }); await new Promise((r) => setTimeout(r, 500)); return app.commands.listCommands().some((x) => x.id === 'slides:start');`);
  return { version: out.version, cmd, plugin, offered };
});

await step('s01-start', async () => {
  const opened = await ev(`${T} return await T.open('Deck.md', 'live');`);
  const wsBefore = bytes('.obsidian/workspace.json');
  const before = await ev(`${T} return T.ui();`);
  const ran = await start();
  const after = await ev(`${T} return T.ui();`);
  const shot1 = await shot('started');
  const dom = await ev(`${T}
    const added = Array.from(document.body.children).filter((e) => !${JSON.stringify(before.bodyKids)}.includes(e.tagName.toLowerCase() + '.' + T.cls(e)));
    const host = document.querySelector('.slides-container, .slides, [class*=slides]');
    return { added: added.map((e) => T.skeleton(e, 0, 9)), host: host ? { cls: T.cls(host), rect: T.rect(host), parent: T.cls(host.parentElement), grand: T.cls(host.parentElement && host.parentElement.parentElement) } : null,
      hostSkeleton: host ? T.skeleton(host, 0, 9) : null };`);
  await sleep(2500);
  const wsAfter = bytes('.obsidian/workspace.json');
  return { opened, ran, before, after, dom, shot: shot1, wsChanged: wsBefore !== wsAfter, wsAfter };
});

await step('s02-slides', async () => {
  // Assumes the presentation from s01 is still up; walk forward with ArrowRight.
  const seen = [];
  for (let i = 0; i < 8; i++) {
    seen.push({ i, state: await ev(`${T} return T.slideState();`), shot: await shot('slide-' + i) });
    await c.pressKey('ArrowRight'); await sleep(700);
  }
  return { seen };
});

await step('s03-keys', async () => {
  const res = {};
  const idx = () => ev(`${T} const s = T.slideState(); return s.filter((x) => x.display !== 'none' && x.visibility !== 'hidden' && x.opacity !== '0' && x.rect && x.rect[2] > 0).map((x) => [x.cls, x.text.slice(0, 40)]);`);
  await stop();
  await ev(`${T} await T.open('Deck.md', 'live'); return true;`);
  await start();
  res.start = await idx();
  for (const k of ['ArrowRight', 'ArrowRight', 'ArrowLeft', 'Space', 'PageDown', 'PageUp', 'End', 'Home', 'ArrowDown', 'ArrowUp', 'Enter', 'Backspace']) {
    if (k === 'Space') { await c.send('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', text: ' ', windowsVirtualKeyCode: 32 }); await c.send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 }); }
    else await c.pressKey(k);
    await sleep(600);
    res[k + '#' + Object.keys(res).length] = await idx();
  }
  // A click on the slide body.
  const r = await ev(`const e = document.querySelector('[class*=slides]'); if (!e) return null; const r = e.getBoundingClientRect(); return { cx: r.x + r.width * 0.75, cy: r.y + r.height / 2 };`);
  if (r) { await c.clickAt(r.cx, r.cy); await sleep(600); res.afterClickRight = await idx(); }
  return res;
});

await step('s04-controls', async () => {
  const buttons = await ev(`${T} const host = document.querySelector('[class*=slides]'); if (!host) return null;
    const root = host.closest('.modal-container, .workspace-leaf, body') || document.body;
    return Array.from(root.querySelectorAll('button, .clickable-icon, [aria-label]')).filter((b) => b.offsetParent || getComputedStyle(b).position === 'fixed').map((b) => ({ tag: b.tagName.toLowerCase(), cls: T.cls(b), aria: b.getAttribute('aria-label'), text: (b.textContent || '').trim().slice(0, 40), rect: T.rect(b), svg: b.querySelector('svg') ? b.querySelector('svg').getAttribute('class') : null, opacity: getComputedStyle(b).opacity }));`);
  // Hover the bottom of the slide (controls may show on hover).
  const host = await ev(`const e = document.querySelector('[class*=slides]'); if (!e) return null; const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height];`);
  let hovered = null;
  if (host) {
    await c.mouseMove(host[0] + host[2] / 2, host[1] + host[3] - 20); await sleep(700);
    hovered = await ev(`${T} return Array.from(document.querySelectorAll('[class*=slides] *')).filter((b) => /button|icon|control|nav|close|exit/.test(T.cls(b) || '')).map((b) => ({ cls: T.cls(b), aria: b.getAttribute('aria-label'), rect: T.rect(b), opacity: getComputedStyle(b).opacity, display: getComputedStyle(b).display }));`);
    await shot('hover-bottom');
    await c.mouseMove(host[0] + host[2] - 20, host[1] + 20); await sleep(700);
    await shot('hover-top-right');
  }
  return { buttons, hovered };
});

await step('s05-escape', async () => {
  const before = await ev(`${T} return T.ui();`);
  await c.pressKey('Escape'); await sleep(900);
  const after = await ev(`${T} return T.ui();`);
  const shotAfter = await shot('after-escape');
  // Re-start and close with each visible close-ish control instead.
  await ev(`${T} await T.open('Deck.md', 'live'); return true;`);
  await start();
  const closers = await ev(`${T} return Array.from(document.querySelectorAll('[class*=slides] [aria-label], .modal-close-button, [class*=slides] .clickable-icon')).map((b) => ({ cls: T.cls(b), aria: b.getAttribute('aria-label'), rect: T.rect(b) }));`);
  let clicked = null;
  const close = closers.find((x) => /close|exit/i.test((x.aria || '') + ' ' + (x.cls || '')) && x.rect && x.rect[2] > 0);
  if (close) {
    await c.mouseMove(close.rect[0] + close.rect[2] / 2, close.rect[1] + close.rect[3] / 2); await sleep(400);
    await c.clickAt(close.rect[0] + close.rect[2] / 2, close.rect[1] + close.rect[3] / 2); await sleep(900);
    clicked = { close, after: await ev(`${T} return T.ui();`) };
  } else { await stop(); }
  return { before, after, shotAfter, closers, clicked };
});

await step('s06-tricky', async () => {
  const res = {};
  for (const f of Object.keys(TRICKY)) {
    await ev(`${T} await T.open(${JSON.stringify(f)}, 'live'); return true;`);
    const offered = await ev(`${T} return T.offered('slides:start');`);
    await start();
    const slides = await ev(`${T} const host = document.querySelector('[class*=slides]'); if (!host) return null;
      return { skeleton: T.skeleton(host, 0, 8), all: Array.from(host.querySelectorAll('[class*=slide]')).map((e) => ({ cls: T.cls(e), html: e.innerHTML.replace(/<svg[\\s\\S]*?<\\/svg>/g, '<svg/>').slice(0, 600) })) };`);
    const ui = await ev(`${T} return { notices: T.ui().notices, modals: T.ui().modals };`);
    res[f] = { offered, slides, ui, shot: await shot('tricky-' + f.replace(/\W/g, '')) };
    await stop();
  }
  return res;
});

await step('s07-modes', async () => {
  const res = {};
  for (const mode of ['preview', 'source']) {
    await ev(`${T} await T.open('Deck.md', '${mode}'); return true;`);
    await start();
    res[mode] = { ui: await ev(`${T} const u = T.ui(); return { fullscreen: u.fullscreen, bodyKids: u.bodyKids, focus: u.focus };`), shot: await shot('from-' + mode) };
    await stop();
    res[mode].afterEscape = await ev(`${T} const l = app.workspace.activeLeaf; return { state: l.getViewState().state, ui: { fullscreen: T.ui().fullscreen, focus: T.ui().focus } };`);
  }
  return res;
});

// Second pass: s02/s03 pressed keys while focus stayed in the editor, and
// nothing moved. Here the slide is clicked first, and the index is read off
// `.slides > section.present`.
const IDX = `const ss = Array.from(document.querySelectorAll('.slides > section')); return { index: ss.findIndex((s) => s.classList.contains('present')), count: ss.length,
  classes: ss.map((s) => s.className), number: (document.querySelector('.slide-number') || {}).textContent || null,
  focus: document.activeElement ? document.activeElement.tagName.toLowerCase() + '.' + (document.activeElement.className || '') : null,
  open: !!document.querySelector('.slides-container') };`;

await step('s08-keys-focused', async () => {
  const res = { seq: [] };
  await ev(`${T} await T.open('Deck.md', 'live'); return true;`);
  await start();
  res.start = await ev(IDX);
  const host = await ev(`const e = document.querySelector('.reveal'); const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height];`);
  await c.clickAt(host[0] + host[2] / 2, host[1] + host[3] / 2); await sleep(600);
  res.afterCenterClick = await ev(IDX);
  const keys = ['ArrowRight', 'ArrowRight', 'ArrowLeft', 'Space', 'PageDown', 'PageUp', 'End', 'Home', 'ArrowDown', 'ArrowUp', 'Enter', 'Backspace', 'n', 'p', 'l', 'h', 'ArrowRight', 'ArrowRight', 'ArrowRight', 'ArrowRight', 'ArrowRight'];
  for (const k of keys) {
    if (k === 'Space') { await c.send('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', text: ' ', windowsVirtualKeyCode: 32 }); await c.send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 }); }
    else await c.pressKey(k);
    await sleep(700);
    res.seq.push([k, await ev(IDX)]);
  }
  // Shift+Space, and F / O / B / Escape-less keys reveal.js binds.
  await c.pressKey('Home'); await sleep(500);
  for (const k of ['f', 'o', 'b', '.', '?']) {
    await c.pressKey(k); await sleep(900);
    res.seq.push(['key ' + k, { ...(await ev(IDX)), fullscreen: await ev(`return !!document.fullscreenElement;`), overlays: await ev(`return Array.from(document.querySelectorAll('.reveal .overview, .reveal.overview, .pause-overlay, .reveal.paused, .reveal-viewport.paused, .overlay, .reveal.overview')).map((e) => e.className);`), reveal: await ev(`return document.querySelector('.reveal') && document.querySelector('.reveal').className;`) }]);
    await shot('key-' + k.replace(/\W/g, '_'));
    await c.pressKey(k); await sleep(600);
  }
  res.end = await ev(IDX);
  return res;
});

await step('s09-controls', async () => {
  const res = {};
  const st = () => ev(`return Array.from(document.querySelectorAll('.reveal .controls button, .reveal .controls > *, .slides-close-btn, .reveal .progress, .slide-number')).map((b) => { const r = b.getBoundingClientRect(); const cs = getComputedStyle(b); return { tag: b.tagName.toLowerCase(), cls: b.className && b.className.baseVal !== undefined ? b.getAttribute('class') : b.className, aria: b.getAttribute('aria-label'), rect: [r.x, r.y, r.width, r.height], display: cs.display, opacity: cs.opacity, visibility: cs.visibility, color: cs.color, cursor: cs.cursor }; });`);
  await c.pressKey('Home'); await sleep(600);
  res.atFirst = { idx: await ev(IDX), controls: await st() };
  const right = await c.rect('.reveal .controls .navigate-right');
  if (right) { await c.clickAt(right.cx, right.cy); await sleep(800); res.afterRightClick = { idx: await ev(IDX), controls: await st() }; }
  const left = await c.rect('.reveal .controls .navigate-left');
  if (left) { await c.clickAt(left.cx, left.cy); await sleep(800); res.afterLeftClick = { idx: await ev(IDX) }; }
  await c.pressKey('End'); await sleep(700);
  res.atLast = { idx: await ev(IDX), controls: await st(), shot: await shot('last-slide') };
  // The close button: hover, tooltip, click.
  const cb = await c.rect('.slides-close-btn');
  res.closeRect = cb;
  if (cb) {
    await c.mouseMove(cb.cx, cb.cy); await sleep(900);
    res.closeHover = await ev(`const b = document.querySelector('.slides-close-btn'); const cs = getComputedStyle(b); const s = b.querySelector('svg'); const ss = s ? getComputedStyle(s) : null; return { color: cs.color, opacity: cs.opacity, bg: cs.backgroundColor, padding: cs.padding, position: cs.position, top: cs.top, right: cs.right, svg: s ? { w: s.getAttribute('width'), h: s.getAttribute('height'), cls: s.getAttribute('class'), color: ss.color, stroke: ss.stroke, rect: (() => { const r = s.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })() } : null, tooltips: Array.from(document.querySelectorAll('.tooltip')).map((t) => t.textContent) };`);
    res.closeHoverShot = await shot('close-hover');
    await c.clickAt(cb.cx, cb.cy); await sleep(900);
    res.afterClose = await ev(IDX);
  }
  return res;
});

await step('s10-styles', async () => {
  await ev(`${T} await T.open('Deck.md', 'live'); return true;`);
  await start();
  const P = ['font-family', 'font-size', 'font-weight', 'line-height', 'color', 'text-transform', 'letter-spacing', 'text-align', 'margin', 'padding', 'background-color', 'text-shadow', 'word-wrap', 'border', 'border-radius', 'box-shadow', 'width', 'max-width'];
  const read = async (sel) => ev(`const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; const cs = getComputedStyle(e); const o = { rect: (() => { const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })() }; for (const p of ${JSON.stringify(P)}) o[p] = cs.getPropertyValue(p); return o;`);
  const res = {};
  for (const sel of ['.slides-container', '.reveal', '.reveal .slides', '.reveal .slide-background.present', '.reveal .slides > section.present', '.reveal .slides > section.present h1', '.reveal .slides > section.present p', '.reveal .slides > section.present a', '.reveal .slides > section.present strong']) res[sel] = await read(sel);
  for (let i = 0; i < 4; i++) {
    await c.pressKey('ArrowRight'); await sleep(700);
  }
  await c.pressKey('Home'); await sleep(600);
  // Every slide's children styles, walking with the API-free keys.
  const per = [];
  for (let i = 0; i < 4; i++) {
    per.push(await ev(`const s = document.querySelector('.reveal .slides > section.present'); if (!s) return null; const r0 = s.getBoundingClientRect();
      return { cls: s.className, style: s.getAttribute('style'), rect: [r0.x, r0.y, r0.width, r0.height], kids: Array.from(s.children).map((e) => { const cs = getComputedStyle(e); const r = e.getBoundingClientRect(); const o = { tag: e.tagName.toLowerCase(), cls: e.className, rect: [r.x, r.y, r.width, r.height] }; for (const p of ${JSON.stringify(P)}) o[p] = cs.getPropertyValue(p); return o; }) };`));
    await c.pressKey('ArrowRight'); await sleep(700);
  }
  res.per = per;
  res.vars = await ev(`const cs = getComputedStyle(document.querySelector('.reveal')); const o = {}; for (let i = 0; i < cs.length; i++) { const p = cs[i]; if (p.startsWith('--r-') || p.startsWith('--slide')) o[p] = cs.getPropertyValue(p).trim(); } return o;`);
  await stop();
  return res;
});

await step('s11-empty-and-light', async () => {
  const res = {};
  await ev(`${T} await T.open('T-empty.md', 'live'); return true;`);
  await start();
  res.empty = { idx: await ev(IDX), ui: await ev(`${T} const u = T.ui(); return { bodyKids: u.bodyKids, notices: u.notices };`), shot: await shot('empty-note') };
  await stop();
  // The light theme.
  await ev(`app.changeTheme ? app.changeTheme('moonstone') : app.setTheme('moonstone'); return true;`); await sleep(800);
  await ev(`${T} await T.open('Deck.md', 'live'); return true;`);
  await start();
  res.light = { bg: await ev(`return getComputedStyle(document.querySelector('.reveal')).backgroundColor + ' / ' + getComputedStyle(document.querySelector('.slides-container')).backgroundColor + ' / ' + getComputedStyle(document.querySelector('.slide-background.present')).backgroundColor;`), h1: await ev(`const e = document.querySelector('.slides > section.present h1'); return e ? getComputedStyle(e).color : null;`), shot: await shot('light') };
  await stop();
  await ev(`app.changeTheme ? app.changeTheme('obsidian') : app.setTheme('obsidian'); return true;`); await sleep(500);
  // A window resize: the scale.
  res.scale = await ev(`return document.querySelector('.reveal') ? null : 'closed';`);
  return res;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
