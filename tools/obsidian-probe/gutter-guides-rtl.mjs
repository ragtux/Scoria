// Line numbers, indentation guides and right-to-left: the three Editor tab
// toggles (`showLineNumber`, `showIndentGuide`, `rightToLeft`). Geometry and
// computed styles are read from the DOM; nothing of Obsidian's CSS is kept
// beyond the computed values of the properties named here.
//
//   s00  context: version, window, the three keys' defaults (getConfig)
//   s01  line numbers in Source mode: the gutter's box, its elements (text,
//        rect, classes) beside each line block, computed styles of the
//        gutter, a number and the active line's number, the CSS variables
//        whose names mention gutter, line number or fold; the content's
//        left edge with and without the gutter, readable line length on/off
//   s02  the gutter's width as the line count grows (9 .. 10000 lines)
//   s03  line numbers in Live Preview: properties, a table, a code block,
//        math, an embed, a folded heading, a folded list, a wrapped line
//   s04  fold markers beside the gutter: the hovered heading's and list's
//        indicator rects with the gutter on and off; the fold gutter if any
//   s05  `editor:toggle-line-numbers` takes effect live, and reading view
//   s06  indentation guides: lists (tabs, spaces, ordered, tasks) and code,
//        in Source mode and Live Preview, on and off; the guide elements,
//        their ::before styles, the active guide, a wrapped item
//   s07  indentation guides in reading view
//   s08  right-to-left: classes and dir attributes, line alignment, list
//        markers, the gutter's side, mixed lines, in all three modes; the
//        caret under ArrowRight/ArrowLeft/Home/End on Latin, Hebrew and
//        mixed lines, with the setting on and off
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
  console.error(`[gutter] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);

const LONG = 'This paragraph is long enough to wrap across several rows of the editor at the default width, so that the probe can see how a wrapped line is numbered in the gutter and how its rows sit against the number that stands for the whole of it, which matters for the layout.';
const NOTE = [
  '---',
  'title: Probe',
  'tags: [a, b]',
  '---',
  '# Heading one',
  '',
  LONG,
  '',
  '- item one',
  '\t- child',
  '\t\t- grandchild',
  '\t- child two',
  '- item two',
  '',
  '1. first',
  '2. second',
  '',
  '```js',
  'function f() {',
  '    if (x) {',
  '        return 1;',
  '    }',
  '}',
  '```',
  '',
  '| a | b |',
  '| - | - |',
  '| 1 | 2 |',
  '',
  '![[Other]]',
  '',
  '$$',
  'x^2',
  '$$',
  '',
  '## Heading two',
  '',
  'Last line.',
  '',
].join('\n');

const LISTS = [
  '- tab one',
  '\t- tab child',
  '\t\t- tab grandchild',
  '\t\t\t- tab great',
  '\t- tab child two',
  '- tab two',
  '',
  '- space one',
  '    - space child',
  '        - space grandchild',
  '',
  '1. ord one',
  '\t1. ord child',
  '\t\t1. ord grandchild',
  '',
  '- [ ] task one',
  '\t- [ ] task child',
  '',
  '- wrap parent',
  '\t- ' + LONG,
  '\t\t- after wrap',
  '',
  '- parent',
  '\tcontinuation paragraph inside the item',
  '',
  '```',
  'code',
  '    indented four',
  '\ttabbed',
  '        indented eight',
  '```',
  '',
  'plain',
  '    indented paragraph line',
  '',
].join('\n');

const RTL = [
  '# כותרת ראשית',
  '',
  'English only line.',
  '',
  'שורה בעברית בלבד.',
  '',
  'Mixed שלום world עולם end.',
  '',
  'שלום English בתוך עברית.',
  '',
  '- פריט אחד',
  '\t- פריט ילד',
  '- English item',
  '',
  '1. ראשון',
  '2. second',
  '',
  '> ציטוט',
  '',
  '```',
  'code line',
  '```',
  '',
].join('\n');

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Note.md'), NOTE);
  fs.writeFileSync(vp('Lists.md'), LISTS);
  fs.writeFileSync(vp('Rtl.md'), RTL);
  fs.writeFileSync(vp('Other.md'), '# Other\n\nThe other note.\n');
  fs.writeFileSync(vp('Long.md'), 'x\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

await c.evaluate(`
window.__g = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x * 10) / 10, Math.round(r.y * 10) / 10, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10]; },
  style(e, props, pseudo) {
    if (!e) return null;
    const cs = getComputedStyle(e, pseudo || null); const o = {};
    for (const p of props) o[p] = cs.getPropertyValue(p);
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
    if (mode === 'source') { vs.state.mode = 'source'; vs.state.source = true; }
    if (mode === 'live') { vs.state.mode = 'source'; vs.state.source = false; }
    if (mode === 'preview') { vs.state.mode = 'preview'; }
    await leaf.setViewState(vs);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(700);
    return { file: leaf.view.file && leaf.view.file.path, state: leaf.getViewState().state };
  },
  view() { return this.leaf().view; },
  ed() { return this.view().editor; },
  cm() { return this.ed().cm; },
  root() { return this.view().containerEl; },
  async setConfig(k, v) { app.vault.setConfig(k, v); if (app.workspace.updateOptions) app.workspace.updateOptions(); await this.sleep(500); return app.vault.getConfig(k); },
  // Every line block in the viewport with its line number and type, and the
  // gutter elements, in document order.
  blocks() {
    const cm = this.cm(); const top0 = cm.contentDOM.getBoundingClientRect().top;
    return cm.viewportLineBlocks.map((b) => ({ line: cm.state.doc.lineAt(b.from).number, toLine: cm.state.doc.lineAt(b.to).number, top: Math.round(b.top * 10) / 10, height: Math.round(b.height * 10) / 10, type: Array.isArray(b.type) ? b.type.map(String) : String(b.type) }));
  },
  gutters() {
    const root = this.root();
    const gs = Array.from(root.querySelectorAll('.cm-gutters'));
    return gs.map((g) => ({
      cls: this.cls(g), rect: this.rect(g),
      style: this.style(g, ['position', 'left', 'right', 'inset-inline-start', 'margin-left', 'margin-right', 'margin-inline-end', 'padding-left', 'padding-right', 'background-color', 'border-right', 'border-left', 'color', 'font-family', 'font-size', 'z-index', 'min-height', 'display', 'direction', 'flex-direction']),
      gutters: Array.from(g.querySelectorAll('.cm-gutter')).map((gg) => ({
        cls: this.cls(gg), rect: this.rect(gg),
        style: this.style(gg, ['min-width', 'width', 'padding-left', 'padding-right', 'margin-left', 'margin-right', 'flex-grow', 'display']),
        els: Array.from(gg.querySelectorAll('.cm-gutterElement')).slice(0, 80).map((el) => ({ text: el.textContent, cls: this.cls(el), rect: this.rect(el), vis: getComputedStyle(el).visibility, html: el.innerHTML.slice(0, 200) })),
      })),
    }));
  },
  numberStyles() {
    const root = this.root();
    const props = ['color', 'opacity', 'font-family', 'font-size', 'font-weight', 'font-variant-numeric', 'font-feature-settings', 'line-height', 'text-align', 'padding-left', 'padding-right', 'padding-top', 'margin-left', 'margin-right', 'min-width', 'background-color', 'vertical-align', 'display', 'justify-content', 'align-items'];
    const els = Array.from(root.querySelectorAll('.cm-lineNumbers .cm-gutterElement')).filter((e) => getComputedStyle(e).visibility !== 'hidden');
    const active = els.find((e) => e.classList.contains('cm-active'));
    const plain = els.find((e) => !e.classList.contains('cm-active'));
    const line = root.querySelector('.cm-line');
    return { plain: this.style(plain, props), active: this.style(active, props), activeText: active && active.textContent, lineText: this.style(line, ['font-family', 'font-size', 'line-height', 'color', 'padding-left', 'padding-right']) };
  },
  vars(re) {
    // The names of custom properties that some rule declares, matched by re,
    // and their computed values on body and on the gutter.
    const names = new Set();
    for (const sh of Array.from(document.styleSheets)) {
      let rules; try { rules = sh.cssRules; } catch { continue; }
      const walk = (rs) => { for (const r of Array.from(rs)) { if (r.style) for (const p of Array.from(r.style)) if (p.startsWith('--') && re.test(p)) names.add(p); if (r.cssRules) walk(r.cssRules); } };
      walk(rules);
    }
    const g = this.root().querySelector('.cm-gutters') || document.body;
    const o = {};
    for (const n of Array.from(names).sort()) o[n] = { body: getComputedStyle(document.body).getPropertyValue(n).trim(), gutter: getComputedStyle(g).getPropertyValue(n).trim() };
    return o;
  },
  layout() {
    const root = this.root();
    const q = (s) => root.querySelector(s);
    return {
      sourceView: this.cls(q('.markdown-source-view')), sizer: this.rect(q('.cm-sizer')), content: this.rect(q('.cm-content')), contentContainer: this.rect(q('.cm-contentContainer')),
      scroller: this.rect(q('.cm-scroller')), firstLine: this.rect(q('.cm-line')), inlineTitle: this.rect(q('.inline-title')),
      sizerStyle: this.style(q('.cm-sizer'), ['margin-left', 'margin-right', 'max-width', 'padding-left', 'padding-right']),
      contentStyle: this.style(q('.cm-content'), ['padding-left', 'padding-right', 'max-width', 'direction', 'text-align']),
      ccStyle: this.style(q('.cm-contentContainer'), ['margin-left', 'margin-right', 'padding-left', 'flex-basis', 'max-width']),
    };
  },
  async seed(content, cursor) {
    const view = this.view();
    const ed = this.ed();
    await app.vault.modify(view.file, content);
    await this.sleep(400);
    let seedOk = ed.getValue() === content;
    if (!seedOk) { ed.setValue(content); await this.sleep(150); seedOk = ed.getValue() === content; }
    ed.focus();
    ed.setCursor(cursor || { line: 0, ch: 0 });
    await this.sleep(150);
    return { seedOk };
  },
  // Lines with their class, dir, direction, text-align, the rect of their
  // text (first and last character), and the elements inside them whose
  // class mentions indent or list.
  lines(max = 60) {
    const root = this.root();
    const ls = Array.from(root.querySelectorAll('.cm-content > .cm-line, .cm-content > .cm-embed-block, .cm-content > *')).slice(0, max);
    return ls.map((l) => {
      const cs = getComputedStyle(l);
      let first = null, last = null;
      try {
        const tw = document.createTreeWalker(l, NodeFilter.SHOW_TEXT); const texts = []; let n; while ((n = tw.nextNode())) if (n.textContent.length) texts.push(n);
        if (texts.length) {
          const r1 = document.createRange(); r1.setStart(texts[0], 0); r1.setEnd(texts[0], 1); first = this.rect(r1);
          const t = texts[texts.length - 1]; const r2 = document.createRange(); r2.setStart(t, t.length - 1); r2.setEnd(t, t.length); last = this.rect(r2);
        }
      } catch {}
      return {
        cls: this.cls(l), dir: l.getAttribute('dir'), text: l.textContent.slice(0, 80), rect: this.rect(l), first, last,
        style: { direction: cs.direction, textAlign: cs.textAlign, unicodeBidi: cs.unicodeBidi, paddingLeft: cs.paddingLeft, paddingRight: cs.paddingRight, textIndent: cs.textIndent, paddingInlineStart: cs.paddingInlineStart },
        parts: Array.from(l.querySelectorAll('[class*="indent"], [class*="list"], [class*="bullet"], [class*="formatting-list"], .task-list-item-checkbox')).slice(0, 12).map((e) => ({ tag: e.tagName.toLowerCase(), cls: this.cls(e), text: e.textContent.slice(0, 20), rect: this.rect(e) })),
      };
    });
  },
  guides() {
    const root = this.root();
    const els = Array.from(root.querySelectorAll('.cm-indent, [class*="indent-guide"], .cm-hmd-list-indent, [class*="list-indent"]'));
    const props = ['content', 'position', 'display', 'left', 'right', 'top', 'bottom', 'width', 'height', 'border-left', 'border-right', 'border-inline-end', 'border-inline-start', 'background-color', 'opacity', 'inset-inline-end', 'margin-inline-start', 'transform', 'pointer-events', 'z-index', 'color'];
    return els.slice(0, 40).map((e) => {
      const line = e.closest('.cm-line');
      return { cls: this.cls(e), text: JSON.stringify(e.textContent), rect: this.rect(e), line: line && line.textContent.slice(0, 40), lineCls: line && this.cls(line),
        self: this.style(e, ['display', 'position', 'width', 'padding-inline-end', 'margin-inline-end', 'border-inline-end', 'border-right', 'background-image', 'color']),
        before: this.style(e, props, '::before'), after: this.style(e, props, '::after') };
    });
  },
  readingLists() {
    const root = this.root();
    const pv = root.querySelector('.markdown-preview-view');
    const props = ['content', 'position', 'display', 'left', 'right', 'top', 'bottom', 'width', 'height', 'border-left', 'border-right', 'border-inline-end', 'border-inline-start', 'background-color', 'inset-inline-start', 'margin-inline-start', 'transform'];
    const uls = Array.from(root.querySelectorAll('.markdown-preview-view ul, .markdown-preview-view ol')).slice(0, 12);
    return { pvCls: this.cls(pv), rendered: this.cls(root.querySelector('.markdown-rendered')),
      lists: uls.map((u) => ({ tag: u.tagName, cls: this.cls(u), rect: this.rect(u), parentLi: !!u.parentElement.closest('li'), self: this.style(u, ['padding-inline-start', 'margin-inline-start', 'position', 'border-inline-start', 'direction']), before: this.style(u, props, '::before'), firstLi: (() => { const li = u.querySelector(':scope > li'); return li ? { rect: this.rect(li), cls: this.cls(li), before: this.style(li, props, '::before'), bullet: this.rect(li.querySelector('.list-bullet')), collapse: this.rect(li.querySelector('.list-collapse-indicator')) } : null; })() })) };
  },
};
return true;`);
const ev = (s) => c.evaluate(s);
const G = 'const G = window.__g;';
let shotN = 0;
async function shot(name) {
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
const open = (p, m) => ev(`${G} return await G.open(${JSON.stringify(p)}, '${m}');`);
const setConfig = (k, v) => ev(`${G} return await G.setConfig(${JSON.stringify(k)}, ${JSON.stringify(v)});`);
const seed = (content, cursor) => ev(`${G} return await G.seed(${JSON.stringify(content)}, ${JSON.stringify(cursor || null)});`);
const snap = async (extra = {}) => ev(`${G} return { gutters: G.gutters(), blocks: G.blocks(), numbers: G.numberStyles(), layout: G.layout(), cursor: G.ed().getCursor() };`);
const cursor = () => ev(`${G} return G.ed().getCursor();`);
const setCursor = (line, ch) => ev(`${G} G.ed().focus(); G.ed().setCursor({ line: ${line}, ch: ${ch} }); await G.sleep(150); return G.ed().getCursor();`);

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio, apiVersion: (() => { try { return require('obsidian').apiVersion; } catch { return null; } })() };`);
  const defaults = await ev(`return { showLineNumber: app.vault.getConfig('showLineNumber'), showIndentGuide: app.vault.getConfig('showIndentGuide'), rightToLeft: app.vault.getConfig('rightToLeft'), readableLineLength: app.vault.getConfig('readableLineLength'), foldHeading: app.vault.getConfig('foldHeading'), foldIndent: app.vault.getConfig('foldIndent') };`);
  const bodyCls = await ev(`return document.body.className;`);
  return { version: out.version, defaults, bodyCls };
});

await step('s01-source-numbers', async () => {
  const r = {};
  await setConfig('showLineNumber', false);
  await open('Note.md', 'source');
  await setCursor(6, 0);
  r.off = await ev(`${G} return { layout: G.layout(), gutters: G.gutters(), bodyCls: document.body.className };`);
  r.offShot = await shot('source-off');
  r.set = await setConfig('showLineNumber', true);
  await sleep(500);
  await setCursor(6, 0);
  r.on = await snap();
  r.onShot = await shot('source-on');
  r.vars = await ev(`${G} return G.vars(/gutter|line-?num|fold|indent|indentation/i);`);
  r.bodyClsOn = await ev(`return document.body.className;`);
  // Readable line length off: where the gutter goes then.
  await setConfig('readableLineLength', false);
  await sleep(400);
  r.wide = await ev(`${G} return { layout: G.layout(), gutters: G.gutters().map((g) => ({ rect: g.rect, style: g.style })) };`);
  r.wideShot = await shot('source-on-wide');
  await setConfig('readableLineLength', true);
  // Active line moves: the number of the caret's line and a selection over several lines.
  await setCursor(10, 2);
  r.activeChild = await ev(`${G} return Array.from(G.root().querySelectorAll('.cm-lineNumbers .cm-gutterElement.cm-active')).map((e) => e.textContent);`);
  await ev(`${G} G.ed().setSelection({ line: 8, ch: 0 }, { line: 12, ch: 3 }); await G.sleep(150); return true;`);
  r.activeSelection = await ev(`${G} return Array.from(G.root().querySelectorAll('.cm-lineNumbers .cm-gutterElement.cm-active')).map((e) => e.textContent);`);
  await ev(`${G} G.ed().setSelections([{ anchor: { line: 4, ch: 0 } }, { anchor: { line: 14, ch: 0 } }]); await G.sleep(150); return true;`);
  r.activeMulti = await ev(`${G} return Array.from(G.root().querySelectorAll('.cm-lineNumbers .cm-gutterElement.cm-active')).map((e) => e.textContent);`);
  // Unfocused editor: is the active number still marked?
  await ev(`document.activeElement && document.activeElement.blur(); await new Promise((r) => setTimeout(r, 200)); return true;`);
  r.activeBlurred = await ev(`${G} return Array.from(G.root().querySelectorAll('.cm-lineNumbers .cm-gutterElement.cm-active')).map((e) => e.textContent);`);
  // Dark theme colours.
  await ev(`app.changeTheme && app.changeTheme('obsidian'); await new Promise((r) => setTimeout(r, 400)); return true;`);
  await setCursor(6, 0);
  r.dark = await ev(`${G} return { numbers: G.numberStyles(), vars: G.vars(/gutter|line-?num/i) };`);
  r.darkShot = await shot('source-on-dark');
  await ev(`app.changeTheme && app.changeTheme('moonstone'); await new Promise((r) => setTimeout(r, 400)); return true;`);
  r.light = await ev(`${G} return { numbers: G.numberStyles() };`);
  return r;
});

await step('s02-width-growth', async () => {
  const r = { rows: [] };
  await open('Long.md', 'source');
  for (const n of [1, 9, 10, 99, 100, 999, 1000, 9999, 10000]) {
    const text = Array.from({ length: n }, (_, i) => 'line ' + (i + 1)).join('\n');
    await seed(text, { line: n - 1, ch: 0 });
    await sleep(300);
    const m = await ev(`${G} const g = G.root().querySelector('.cm-lineNumbers'); const gs = G.root().querySelector('.cm-gutters'); const els = Array.from(g.querySelectorAll('.cm-gutterElement')); const vis = els.filter((e) => getComputedStyle(e).visibility !== 'hidden');
      return { lines: G.cm().state.doc.lines, gutters: G.rect(gs), numbers: G.rect(g), spacer: els[0] && { text: els[0].textContent, vis: getComputedStyle(els[0]).visibility, rect: G.rect(els[0]) }, lastVisible: vis.length && { text: vis[vis.length - 1].textContent, rect: G.rect(vis[vis.length - 1]) }, content: G.rect(G.root().querySelector('.cm-content')), firstLine: G.rect(G.root().querySelector('.cm-line')) };`);
    r.rows.push({ n, ...m });
  }
  r.shot = await shot('width-10000');
  return r;
});

await step('s03-live-numbers', async () => {
  const r = {};
  await open('Note.md', 'live');
  await seed(NOTE, { line: 37, ch: 0 });
  await sleep(800);
  r.live = await snap();
  r.liveShot = await shot('live-on');
  // Caret inside the table and the code block: are those lines numbered then?
  await setCursor(26, 2);
  await sleep(300);
  r.inTable = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  r.inTableShot = await shot('live-caret-in-table');
  await setCursor(19, 2);
  await sleep(300);
  r.inCode = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  await setCursor(1, 2);
  await sleep(300);
  r.inProps = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  // Fold the first heading and the first list item.
  await setCursor(4, 0);
  await ev(`app.commands.executeCommandById('editor:toggle-fold'); await new Promise((r) => setTimeout(r, 400)); return true;`);
  r.headingFolded = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  r.headingFoldedShot = await shot('live-heading-folded');
  await ev(`app.commands.executeCommandById('editor:unfold-all'); await new Promise((r) => setTimeout(r, 300)); return true;`);
  await setCursor(8, 3);
  await ev(`app.commands.executeCommandById('editor:toggle-fold'); await new Promise((r) => setTimeout(r, 400)); return true;`);
  r.listFolded = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  await ev(`app.commands.executeCommandById('editor:unfold-all'); await new Promise((r) => setTimeout(r, 300)); return true;`);
  // The same folds in Source mode.
  await open('Note.md', 'source');
  await setCursor(4, 0);
  await ev(`app.commands.executeCommandById('editor:toggle-fold'); await new Promise((r) => setTimeout(r, 400)); return true;`);
  r.sourceHeadingFolded = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  r.sourceHeadingFoldedShot = await shot('source-heading-folded');
  await ev(`app.commands.executeCommandById('editor:unfold-all'); await new Promise((r) => setTimeout(r, 300)); return true;`);
  // Properties hidden / source display.
  await open('Note.md', 'live');
  r.propsSetting = await ev(`return app.vault.getConfig('propertiesInDocument');`);
  await setConfig('propertiesInDocument', 'hidden');
  await open('Note.md', 'live');
  await setCursor(37, 0);
  r.propsHidden = await ev(`${G} return { gutters: G.gutters(), blocks: G.blocks() };`);
  await setConfig('propertiesInDocument', 'visible');
  return r;
});

await step('s04-fold-markers', async () => {
  const r = {};
  await open('Note.md', 'source');
  await setCursor(37, 0);
  const hoverAt = async (line) => {
    const pt = await ev(`${G} const cm = G.cm(); const pos = cm.state.doc.line(${line + 1}).from; const co = cm.coordsAtPos(pos); return co && { x: co.left + 20, y: (co.top + co.bottom) / 2 };`);
    if (!pt) return null;
    await c.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(pt.x), y: Math.round(pt.y) });
    await sleep(300);
    return ev(`${G} const ind = Array.from(G.root().querySelectorAll('.cm-fold-indicator, .collapse-indicator, [class*="fold"]')).filter((e) => e.getBoundingClientRect().width > 0);
      return { pt: ${JSON.stringify(pt)}, indicators: ind.slice(0, 10).map((e) => ({ cls: G.cls(e), rect: G.rect(e), line: (e.closest('.cm-line') || {}).textContent, parent: G.cls(e.parentElement), style: G.style(e, ['position', 'left', 'right', 'inset-inline-start', 'opacity', 'visibility', 'padding-inline-end', 'margin-inline-start']) })), gutters: G.gutters().map((g) => ({ rect: g.rect, gutters: g.gutters.map((x) => ({ cls: x.cls, rect: x.rect })) })) };`);
  };
  for (const v of [true, false]) {
    await setConfig('showLineNumber', v);
    await sleep(300);
    r[v ? 'on' : 'off'] = { heading: await hoverAt(4), list: await hoverAt(8), child: await hoverAt(9) };
    r[(v ? 'on' : 'off') + 'Shot'] = await shot('fold-hover-' + (v ? 'on' : 'off'));
  }
  await setConfig('showLineNumber', true);
  await open('Note.md', 'live');
  r.live = { heading: await hoverAt(4), list: await hoverAt(8) };
  r.liveShot = await shot('fold-hover-live');
  return r;
});

await step('s05-toggle-and-reading', async () => {
  const r = {};
  await open('Note.md', 'source');
  r.before = await ev(`return app.vault.getConfig('showLineNumber');`);
  r.ran = await ev(`return app.commands.executeCommandById('editor:toggle-line-numbers');`);
  await sleep(400);
  r.after = await ev(`${G} return { cfg: app.vault.getConfig('showLineNumber'), gutters: G.gutters().map((g) => ({ rect: g.rect, n: g.gutters.map((x) => x.cls) })) };`);
  r.appJson = fs.readFileSync(vp('.obsidian/app.json'), 'utf8');
  await ev(`return app.commands.executeCommandById('editor:toggle-line-numbers');`);
  await sleep(400);
  r.again = await ev(`${G} return { cfg: app.vault.getConfig('showLineNumber'), gutters: G.gutters().map((g) => ({ rect: g.rect })) };`);
  r.appJson2 = fs.readFileSync(vp('.obsidian/app.json'), 'utf8');
  r.listed = await ev(`return app.commands.listCommands().filter((c) => /line-numbers|indent-guide|rtl|right-to-left|direction/i.test(c.id)).map((c) => ({ id: c.id, name: c.name }));`);
  await setConfig('showLineNumber', true);
  await open('Note.md', 'preview');
  r.reading = await ev(`${G} return { gutters: G.root().querySelectorAll('.markdown-preview-view .cm-gutters, .markdown-preview-view [class*="line-number"]').length, pv: G.cls(G.root().querySelector('.markdown-preview-view')) };`);
  r.readingShot = await shot('reading-with-numbers-on');
  return r;
});

await step('s06-indent-guides-editor', async () => {
  const r = {};
  await setConfig('showLineNumber', false);
  for (const mode of ['live', 'source']) {
    for (const on of [true, false]) {
      await setConfig('showIndentGuide', on);
      await open('Lists.md', mode);
      await seed(LISTS, { line: 2, ch: 4 });
      await sleep(500);
      const key = `${mode}-${on ? 'on' : 'off'}`;
      r[key] = await ev(`${G} return { sourceView: G.cls(G.root().querySelector('.markdown-source-view')), bodyCls: document.body.className, guides: G.guides(), lines: G.lines(40) };`);
      r[key + 'Shot'] = await shot('guides-' + key);
    }
  }
  await setConfig('showIndentGuide', true);
  await open('Lists.md', 'live');
  // Active guide: the caret on a grandchild, then on a top-level item.
  for (const [name, line, ch] of [['grandchild', 2, 6], ['child', 1, 4], ['top', 0, 3], ['great', 3, 6], ['wrapped', 19, 20], ['continuation', 23, 4]]) {
    await setCursor(line, ch);
    await sleep(250);
    r['active-' + name] = await ev(`${G} return G.guides().filter((g) => /active/.test(g.cls) || /active/.test(g.lineCls || '')).map((g) => ({ cls: g.cls, line: g.line, rect: g.rect, before: g.before }));`);
  }
  r.activeShot = await shot('guides-live-active');
  r.vars = await ev(`${G} return G.vars(/indent|guide|list/i);`);
  return r;
});

await step('s07-indent-guides-reading', async () => {
  const r = {};
  for (const on of [true, false]) {
    await setConfig('showIndentGuide', on);
    await open('Lists.md', 'preview');
    await sleep(500);
    r[on ? 'on' : 'off'] = await ev(`${G} return G.readingLists();`);
    r[(on ? 'on' : 'off') + 'Shot'] = await shot('guides-reading-' + (on ? 'on' : 'off'));
  }
  await setConfig('showIndentGuide', true);
  return r;
});

await step('s08-rtl', async () => {
  const r = {};
  await setConfig('showLineNumber', true);
  for (const on of [false, true]) {
    r[`set-${on}`] = await setConfig('rightToLeft', on);
    for (const mode of ['source', 'live', 'preview']) {
      await open('Rtl.md', mode);
      await sleep(500);
      const key = `${on ? 'rtl' : 'ltr'}-${mode}`;
      if (mode === 'preview') {
        r[key] = await ev(`${G} const pv = G.root().querySelector('.markdown-preview-view'); const sizer = G.root().querySelector('.markdown-preview-sizer');
          return { pv: G.cls(pv), dir: pv && pv.getAttribute('dir'), sizerDir: sizer && sizer.getAttribute('dir'), viewDir: G.root().getAttribute('dir'), style: G.style(pv, ['direction', 'text-align']),
            blocks: Array.from(G.root().querySelectorAll('.markdown-preview-sizer > div')).slice(0, 20).map((d) => { const e = d.firstElementChild; const li = d.querySelector('li'); return { cls: G.cls(d), tag: e && e.tagName, dir: e && e.getAttribute('dir'), text: d.textContent.slice(0, 50), rect: G.rect(e), style: G.style(e, ['direction', 'text-align', 'padding-inline-start', 'padding-left', 'padding-right', 'unicode-bidi']), li: li && { rect: G.rect(li), dir: li.getAttribute('dir'), bullet: G.rect(li.querySelector('.list-bullet')), style: G.style(li, ['direction', 'text-align']) } }; }) };`);
      } else {
        r[key] = await ev(`${G} const content = G.root().querySelector('.cm-content'); return { sourceView: G.cls(G.root().querySelector('.markdown-source-view')), viewDir: G.root().getAttribute('dir'), contentDir: content.getAttribute('dir'), editorDir: G.cm().dom.getAttribute('dir'), textDirection: String(G.cm().textDirection), contentStyle: G.style(content, ['direction', 'text-align', 'unicode-bidi']), gutters: G.gutters().map((g) => ({ rect: g.rect, style: g.style })), layout: G.layout(), lines: G.lines(30) };`);
      }
      r[key + 'Shot'] = await shot(key);
    }
    // Caret moves, Source mode: start of the line, ArrowRight x3; end, ArrowLeft x3; Home; End.
    await open('Rtl.md', 'source');
    const moves = {};
    for (const [name, line] of [['latin', 2], ['hebrew', 4], ['mixed', 6], ['hebrewFirst', 8], ['heading', 0], ['item', 10]]) {
      const m = { start: await setCursor(line, 0) };
      const seq = [];
      for (let i = 0; i < 3; i++) { await c.pressKey('ArrowRight'); await sleep(80); seq.push(await cursor()); }
      m.right = seq;
      const len = await ev(`${G} return G.ed().getLine(${line}).length;`);
      await setCursor(line, len);
      const seqL = [];
      for (let i = 0; i < 3; i++) { await c.pressKey('ArrowLeft'); await sleep(80); seqL.push(await cursor()); }
      m.leftFromEnd = seqL;
      // Across the mixed run: walk right from 0 through the line.
      if (name === 'mixed' || name === 'hebrewFirst') {
        await setCursor(line, 0);
        const walk = [];
        for (let i = 0; i < len + 2; i++) { await c.pressKey('ArrowRight'); await sleep(50); walk.push((await cursor()).ch); }
        m.walkRight = walk;
      }
      await setCursor(line, 3);
      await c.pressKey('Home'); await sleep(80); m.home = await cursor();
      await c.pressKey('End'); await sleep(80); m.end = await cursor();
      // Where the caret is drawn at ch 0 and at the end.
      m.coords = await ev(`${G} const cm = G.cm(); const l = cm.state.doc.line(${line + 1}); const a = cm.coordsAtPos(l.from), b = cm.coordsAtPos(l.to); return { from: a && [Math.round(a.left), Math.round(a.top)], to: b && [Math.round(b.left), Math.round(b.top)] };`);
      moves[name] = m;
    }
    r[`moves-${on ? 'rtl' : 'ltr'}`] = moves;
  }
  await setConfig('rightToLeft', false);
  r.appJson = fs.readFileSync(vp('.obsidian/app.json'), 'utf8');
  return r;
});

c.close();
process.exit(0);
