// What Obsidian does with rich text pasted into the editor, and with files
// pasted from a file manager or dropped from one — the parts the 2026-10-18
// and 2026-10-25 attachment-paste runs left open.
//
//   s00  context: version, `autoConvertHtml` as `getConfig` reads it with no
//        key in app.json, the commands whose id or name says paste
//   s01  the HTML fixtures: each put on the Electron clipboard with
//        `clipboard.write({ html, text })`, a real Ctrl+V into an empty Live
//        Preview note, the editor's text read back
//   s02  context: the same HTML pasted mid-line, into a list item, inside a
//        fenced code block, in Source mode
//   s03  Ctrl+Shift+V over HTML; and `autoConvertHtml: false` (bytes of
//        app.json after the toggle)
//   s04  HTML carrying a data: image, and HTML with an image beside it on the
//        clipboard (a browser's "Copy image")
//   s05  files on the clipboard as a file manager puts them (`text/uri-list`),
//        one and two files, through Electron and through wl-copy
//   s06  drops: two files at once, at a pointer position away from the caret;
//        a file already inside the vault; a .md file from outside; dragged
//        HTML and dragged plain text; a drop over a selection
//
// The caller runs it in a private headless sway with USER_DATA_DIR and its
// own CDP_PORT. ONLY_STEPS=<prefix,...> narrows the run.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawn } from 'node:child_process';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT, RESULTS = process.env.RESULTS;
const vp = (rel) => path.join(VAULT, rel);
const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
const ONLY = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  out.order.push(name);
  const t = Date.now(); const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save(); return out.steps[name];
}
function tree() {
  const walk = (dir, prefix) => {
    const o = [];
    for (const e of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (e.name === '.obsidian' || e.name === '.trash') continue;
      const rel = prefix ? `${prefix}/${e.name}` : e.name;
      if (e.isDirectory()) { o.push(rel + '/'); o.push(...walk(path.join(dir, e.name), rel)); }
      else o.push(rel + ' (' + fs.statSync(path.join(dir, e.name)).size + 'b)');
    }
    return o;
  };
  return walk(VAULT, '');
}
const readIf = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch { return null; } };
const SRC = path.join(path.dirname(VAULT), 'paste-html-fixtures-' + path.basename(VAULT));
const src = (f) => path.join(SRC, f);
// A 1x1 PNG.
const PNG = Buffer.from('89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360f8cfc0f01f0005000201e2213ebc0000000049454e44ae426082', 'hex');

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('Sub'), { recursive: true });
  fs.mkdirSync(SRC, { recursive: true });
  fs.writeFileSync(src('photo.png'), PNG);
  fs.writeFileSync(src('second shot.png'), PNG);
  fs.writeFileSync(src('notes.txt'), 'some notes\n');
  fs.writeFileSync(src('Outside Note.md'), '# Outside\n');
  fs.writeFileSync(vp('Inside.png'), PNG);
  fs.writeFileSync(vp('Target.md'), '\n');
  fs.writeFileSync(vp('Drop.md'), 'line zero\nline one\nline two\nline three\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'command-palette': true }, null, 2));
  save(); process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

await c.evaluate(`
window.__ph = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  view() {
    const a = app.workspace.activeLeaf;
    if (a && a.view && a.view.getViewType() === 'markdown') return a.view;
    return app.workspace.getLeavesOfType('markdown')[0].view;
  },
  async open(file, mode) {
    const ls = app.workspace.getLeavesOfType('markdown');
    const leaf = ls.length ? ls[0] : app.workspace.getLeaf(false);
    if (!leaf.view.file || leaf.view.file.path !== file) await leaf.openFile(app.vault.getAbstractFileByPath(file));
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(300);
    const st = leaf.getViewState();
    const want = { mode: 'source', source: mode === 'source' };
    if (st.state.mode !== want.mode || !!st.state.source !== want.source) {
      st.state = Object.assign({}, st.state, want);
      await leaf.setViewState(st);
      await this.sleep(300);
    }
    return { file: leaf.view.file.path, state: leaf.getViewState().state };
  },
  set(text, line, ch) {
    const ed = this.view().editor;
    ed.setValue(text);
    ed.focus();
    ed.setCursor({ line, ch });
    return ed.getCursor();
  },
  select(a, b) { const ed = this.view().editor; ed.focus(); ed.setSelection(a, b); return ed.listSelections(); },
  value() { return this.view().editor.getValue(); },
  cursor() { const ed = this.view().editor; return { cursor: ed.getCursor(), selections: ed.listSelections() }; },
  clip(o) {
    const { clipboard, nativeImage } = window.require('electron');
    clipboard.clear();
    const data = {};
    if (o.text !== undefined) data.text = o.text;
    if (o.html !== undefined) data.html = o.html;
    if (o.image) data.image = nativeImage.createFromPath(o.image);
    clipboard.write(data);
    return clipboard.availableFormats();
  },
  clipBuffer(format, s, text) {
    const { clipboard } = window.require('electron');
    clipboard.clear();
    clipboard.writeBuffer(format, Buffer.from(s, 'utf8'));
    return clipboard.availableFormats();
  },
  formats() { return window.require('electron').clipboard.availableFormats(); },
  notices() { return Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent); },
  clearNotices() { document.querySelectorAll('.notice').forEach((n) => n.remove()); },
  config(k, v) { if (v === undefined) return app.vault.getConfig(k); app.vault.setConfig(k, v); return app.vault.getConfig(k); },
  // The viewport point of a document position.
  coords(line, ch) {
    const ed = this.view().editor;
    const cm = ed.cm;
    const pos = ed.posToOffset({ line, ch });
    const r = cm.coordsAtPos(pos);
    return { x: r.left, y: (r.top + r.bottom) / 2 };
  },
};
return true;`);

const ev = (b) => c.evaluate(b);
const J = JSON.stringify;
// A click into the editor's text: after a mode switch the keyboard is
// elsewhere and a Ctrl+V reaches nothing (the first run lost s02-s04 to it).
async function focusEditor() {
  const r = await ev(`const el = window.__ph.view().containerEl.querySelector('.cm-content'); const b = el.getBoundingClientRect(); return { x: b.x + 4, y: b.y + 6 };`);
  await c.clickAt(Math.round(r.x), Math.round(r.y));
  await sleep(150);
}

async function pasteHtml(fx, { mode = 'live', start = ['', 0, 0], shift = false, file = 'Target.md' } = {}) {
  await ev(`return await window.__ph.open(${J(file)}, ${J(mode)});`);
  await focusEditor();
  await ev(`return window.__ph.set(${J(start[0])}, ${start[1]}, ${start[2]});`);
  const formats = await ev(`return window.__ph.clip(${J({ html: fx.html, text: fx.text, image: fx.image })});`);
  const before = tree();
  await ev(`window.__ph.clearNotices(); return true;`);
  await c.pressKey('v', { ctrl: true, shift });
  await sleep(700);
  return {
    formats,
    value: await ev(`return window.__ph.value();`),
    cursor: await ev(`return window.__ph.cursor();`),
    treeDelta: tree().filter((x) => !before.includes(x)),
    notices: await ev(`return window.__ph.notices();`),
  };
}

// Each fixture: the HTML a browser would put on the clipboard (Chromium
// wraps a copy in `<meta charset='utf-8'>` and the fragment markers), and the
// text/plain beside it.
const WRAP = (body) => `<meta charset='utf-8'><html><head></head><body><!--StartFragment-->${body}<!--EndFragment--></body></html>`;
const FIXTURES = [
  { id: 'headings', html: '<h1>One</h1><h2>Two</h2><h3>Three</h3><h4>Four</h4><h5>Five</h5><h6>Six</h6>', text: 'One\nTwo\nThree\nFour\nFive\nSix' },
  { id: 'inline-marks', html: '<p><b>b</b> <strong>strong</strong> <i>i</i> <em>em</em> <del>del</del> <s>s</s> <strike>strike</strike> <code>code</code> <u>u</u> <mark>mark</mark> <sup>sup</sup> <sub>sub</sub> <kbd>kbd</kbd></p>', text: 'b strong i em del s strike code u mark sup sub kbd' },
  { id: 'nested-marks', html: '<p><b>bold <i>both</i></b> and <em><strong>both2</strong></em> and <b> padded </b>x</p>', text: 'bold both and both2 and padded x' },
  { id: 'links', html: '<p><a href="https://example.com/a b?q=1&amp;r=2">Example</a> <a href="https://example.com" title="T">titled</a> <a>no href</a> <a href="/rel/path">relative</a> <a href="https://example.com/x">https://example.com/x</a> <a href="mailto:a@b.c">mail</a> <a href="#frag">frag</a></p>', text: 'Example titled no href relative https://example.com/x mail frag' },
  { id: 'images', html: '<p><img src="https://example.com/a.png" alt="Alt text"> <img src="https://example.com/b.png"> <img src="https://example.com/c.png" alt="t" title="Title" width="100"></p>', text: '' },
  { id: 'image-in-link', html: '<a href="https://example.com/page"><img src="https://example.com/i.png" alt="I"></a>', text: '' },
  { id: 'ul-nested', html: '<ul><li>one</li><li>two<ul><li>two a</li><li>two b<ul><li>deep</li></ul></li></ul></li><li>three</li></ul>', text: 'one\ntwo\ntwo a\ntwo b\ndeep\nthree' },
  { id: 'ol', html: '<ol><li>first</li><li>second<ol><li>inner</li></ol></li></ol><ol start="5"><li>five</li><li>six</li></ol>', text: '' },
  { id: 'li-paragraphs', html: '<ul><li><p>para one</p><p>para two</p></li><li>plain</li></ul>', text: '' },
  { id: 'tasks', html: '<ul><li><input type="checkbox" checked> done</li><li><input type="checkbox"> todo</li></ul>', text: '' },
  { id: 'table-thead', html: '<table><thead><tr><th>A</th><th>B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr><tr><td>x | y</td><td><b>bold</b></td></tr></tbody></table>', text: '' },
  { id: 'table-no-thead', html: '<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>', text: '' },
  { id: 'table-align-uneven', html: '<table><tr><th align="left">L</th><th align="center">C</th><th align="right">R</th></tr><tr><td>1</td><td>2</td></tr></table>', text: '' },
  { id: 'pre', html: '<pre><code>fn main() {\n    println!("hi");\n}</code></pre>', text: '' },
  { id: 'pre-lang', html: '<pre><code class="language-rust">let x = 1;\n</code></pre><pre class="lang-js">var y;</pre>', text: '' },
  { id: 'blockquote', html: '<blockquote><p>quoted</p><blockquote><p>nested</p></blockquote></blockquote><p>after</p>', text: '' },
  { id: 'br-and-paras', html: '<p>line one<br>line two</p><p>second para</p><hr><p>after rule</p>', text: '' },
  { id: 'entities', html: '<p>&amp; &lt;tag&gt; &quot;q&quot; &nbsp;nbsp&nbsp; &copy; &#8212; &eacute;</p>', text: '' },
  { id: 'script-style-comment', html: '<style>p{color:red}</style><p>visible</p><script>alert(1)</script><!-- comment --><noscript>ns</noscript>', text: 'visible' },
  { id: 'chromium-wrapper', html: WRAP('<span style="color: rgb(0, 0, 0);">Hello <b>world</b></span>'), text: 'Hello world' },
  { id: 'plain-span-only', html: WRAP('<span>just text</span>'), text: 'just text' },
  { id: 'markdown-chars', html: '<p>*stars* _under_ # hash [brackets] `tick` 1. num &lt;angle&gt; a\\b ~tilde~ ==eq== [[wiki]] #tag</p>', text: '' },
  { id: 'whitespace', html: '<p>a\n   b\t\tc</p>\n\n<div>div one</div><div>div two</div>', text: '' },
  { id: 'gdocs-spans', html: WRAP('<b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:700;">Bold by style</span><span style="font-style:italic;"> italic by style</span></p></b>'), text: '' },
  { id: 'headings-with-inline', html: '<h2><a href="https://e.com">Linked</a> <code>code</code> heading</h2>', text: '' },
  { id: 'empty-elements', html: '<p></p><p><b></b></p><p>x</p><div></div>', text: 'x' },
  { id: 'dl-sup', html: '<dl><dt>Term</dt><dd>Definition</dd></dl>', text: '' },
  { id: 'html-no-text', html: '<b>only html</b>', text: undefined },
  { id: 'text-differs', html: '<p>from html</p>', text: 'from text' },
  { id: 'iframe-video', html: '<p>before</p><iframe src="https://www.youtube.com/embed/x"></iframe><video src="v.mp4"></video><p>after</p>', text: '' },
  { id: 'inline-code-backtick', html: '<p><code>a`b</code> and <code>``x</code></p>', text: '' },
  { id: 'pre-with-backticks', html: '<pre><code>```\ninner\n```</code></pre>', text: '' },
  { id: 'nested-list-in-ol-with-p', html: '<ol><li>one<ul><li>a</li></ul></li><li>two</li></ol>', text: '' },
  { id: 'center-font', html: '<center>centered</center><font color="red">font</font>', text: '' },
  { id: 'heading-in-li', html: '<ul><li><h3>Heading item</h3></li></ul>', text: '' },
  { id: 'br-only', html: 'a<br><br><br>b', text: '' },
  // Added on the second pass, to settle what the first one left open.
  { id: 'ol-start-alone', html: '<ol start="5"><li>five</li><li>six</li></ol>', text: '' },
  { id: 'ol-after-ul', html: '<ul><li>u</li></ul><ol><li>o1</li><li>o2</li></ol>', text: '' },
  { id: 'link-escapes', html: '<p><a href="https://e.com/a_(b)">parens</a> <a href="https://e.com/café?x=%20y">unicode</a> <a href="https://e.com/q?a=&quot;b&quot;" title=\'say "hi"\'>quote</a> <a href="https://e.com/x"></a> <a href="https://e.com/<b>">angle</a> <a href=" https://e.com/trim ">trim</a></p>', text: '' },
  { id: 'img-odd', html: '<p><img alt="no src"> <img src="https://e.com/a b.png" alt="sp"> <img src="https://e.com/(p).png" alt="multi\nline alt"></p>', text: '' },
  { id: 'table-cell-blocks', html: '<table><tr><th>h<br>two</th><th>h2</th></tr><tr><td><p>p1</p><p>p2</p></td><td colspan="2">wide</td></tr></table>', text: '' },
  { id: 'table-in-text', html: '<p>before</p><table><tr><th>a</th></tr><tr><td>1</td></tr></table><p>after</p>', text: '' },
  { id: 'trailing-space-in-block', html: '<p>a </p>b<p> c</p>', text: '' },
  { id: 'code-with-markup', html: '<pre><code><span class="k">let</span> x = <b>1</b>;\n\n\nend</code></pre><p><code>a <b>b</b></code></p>', text: '' },
  { id: 'quote-with-list', html: '<blockquote><ul><li>a</li><li>b</li></ul></blockquote>', text: '' },
  { id: 'emphasis-edge', html: '<p>a<em>b</em>c <strong> </strong> <em>x </em>y <b>multi<br>line</b></p>', text: '' },
  { id: 'heading-br', html: '<h3>one<br>two</h3>', text: '' },
  { id: 'unknown-and-section', html: '<section><article><custom-el>custom</custom-el></article></section><figure><img src="https://e.com/f.png" alt="f"><figcaption>cap</figcaption></figure>', text: '' },
  { id: 'list-loose-text', html: '<ul><li>one\n  <b>bold</b>\n</li><li>  two  </li></ul>', text: '' },
  { id: 'ol-in-ul-depth', html: '<ul><li>a<ol><li>b<ul><li>c</li></ul></li></ol></li></ul>', text: '' },
  { id: 'crlf-text', html: '<p>line\r\nnext</p><pre><code>x\r\ny</code></pre>', text: '' },
];

// ---- s00 -----------------------------------------------------------------
await step('s00-context', async (r) => {
  r.version = await ev(`return { title: document.title, ua: navigator.userAgent, apiVersion: (typeof apiVersion !== 'undefined' ? apiVersion : null) };`);
  r.autoConvertHtml = await ev(`return app.vault.getConfig('autoConvertHtml');`);
  r.appJson = readIf('.obsidian/app.json');
  r.commands = await ev(`return Object.values(app.commands.commands).filter((c) => /paste|drop/i.test(c.id + ' ' + c.name)).map((c) => ({ id: c.id, name: c.name, hotkeys: c.hotkeys || null, editor: !!c.editorCallback || !!c.editorCheckCallback }));`);
  r.defaultHotkeys = await ev(`const h = app.hotkeyManager; return Object.fromEntries(Object.entries(h.defaultKeys || {}).filter(([k]) => /paste/.test(k)));`);
  return r;
});

// ---- s01 -----------------------------------------------------------------
await step('s01-fixtures-live-preview', async (r) => {
  r.cases = {};
  for (const fx of FIXTURES) r.cases[fx.id] = { html: fx.html, text: fx.text ?? null, ...(await pasteHtml(fx)) };
  return r;
});

// ---- s02 -----------------------------------------------------------------
await step('s02-context', async (r) => {
  const h2 = FIXTURES.find((f) => f.id === 'headings');
  const paras = FIXTURES.find((f) => f.id === 'br-and-paras');
  const marks = FIXTURES.find((f) => f.id === 'nested-marks');
  const ul = FIXTURES.find((f) => f.id === 'ul-nested');
  r.midLineBlock = await pasteHtml(h2, { start: ['BEFORE AFTER', 0, 7] });
  r.midLineParas = await pasteHtml(paras, { start: ['BEFORE AFTER', 0, 7] });
  r.midLineInline = await pasteHtml(marks, { start: ['BEFORE AFTER', 0, 7] });
  r.inListItem = await pasteHtml(ul, { start: ['- item ', 0, 7] });
  r.inIndentedListItem = await pasteHtml(ul, { start: ['- item\n\t- sub ', 1, 7] });
  r.inQuote = await pasteHtml(paras, { start: ['> quote ', 0, 8] });
  r.inCodeBlock = await pasteHtml(marks, { start: ['```\n\n```', 1, 0] });
  r.inInlineCode = await pasteHtml(marks, { start: ['`x`', 0, 2] });
  r.sourceMode = await pasteHtml(marks, { mode: 'source' });
  r.sourceModeTable = await pasteHtml(FIXTURES.find((f) => f.id === 'table-thead'), { mode: 'source' });
  r.overSelection = await (async () => {
    await ev(`return await window.__ph.open('Target.md', 'live');`);
    await focusEditor();
    await ev(`window.__ph.set('keep SELECTED keep', 0, 0); return window.__ph.select({line:0,ch:5},{line:0,ch:13});`);
    await ev(`return window.__ph.clip(${J({ html: '<b>bold</b>', text: 'bold' })});`);
    await c.pressKey('v', { ctrl: true });
    await sleep(600);
    return { value: await ev(`return window.__ph.value();`), cursor: await ev(`return window.__ph.cursor();`) };
  })();
  r.undo = await (async () => {
    await ev(`return await window.__ph.open('Target.md', 'live');`);
    await focusEditor();
    await ev(`return window.__ph.set('start ', 0, 6);`);
    await ev(`return window.__ph.clip(${J(paras)});`);
    await c.pressKey('v', { ctrl: true });
    await sleep(600);
    const pasted = await ev(`return window.__ph.value();`);
    await c.pressKey('z', { ctrl: true });
    await sleep(500);
    return { pasted, afterOneUndo: await ev(`return window.__ph.value();`) };
  })();
  return r;
});

// ---- s03 -----------------------------------------------------------------
await step('s03-plain-paste-and-the-setting', async (r) => {
  const some = ['inline-marks', 'links', 'table-thead', 'text-differs', 'html-no-text'].map((id) => FIXTURES.find((f) => f.id === id));
  r.ctrlShiftV = {};
  for (const fx of some) r.ctrlShiftV[fx.id] = await pasteHtml(fx, { shift: true });
  r.setFalse = await ev(`return window.__ph.config('autoConvertHtml', false);`);
  await sleep(800);
  r.appJsonFalse = readIf('.obsidian/app.json');
  r.settingOff = {};
  for (const fx of some) r.settingOff[fx.id] = await pasteHtml(fx);
  r.settingOffShift = {};
  for (const fx of some.slice(0, 2)) r.settingOffShift[fx.id] = await pasteHtml(fx, { shift: true });
  r.setTrue = await ev(`return window.__ph.config('autoConvertHtml', true);`);
  await sleep(800);
  r.appJsonTrue = readIf('.obsidian/app.json');
  return r;
});

// ---- s04 -----------------------------------------------------------------
await step('s04-images-in-html', async (r) => {
  const dataUri = 'data:image/png;base64,' + PNG.toString('base64');
  r.dataUri = await pasteHtml({ html: `<p>x <img src="${dataUri}" alt="inline"> y</p>`, text: 'x  y' });
  r.htmlPlusImage = await pasteHtml({ html: '<img src="https://example.com/copied.png" alt="copied">', image: src('photo.png') });
  r.htmlPlusImageAndText = await pasteHtml({ html: '<img src="https://example.com/copied.png">', text: 'https://example.com/copied.png', image: src('photo.png') });
  r.htmlPlusImageOff = await (async () => {
    await ev(`return window.__ph.config('autoConvertHtml', false);`);
    const got = await pasteHtml({ html: '<img src="https://example.com/copied.png" alt="copied">', image: src('photo.png') });
    await ev(`return window.__ph.config('autoConvertHtml', true);`);
    return got;
  })();
  return r;
});

// ---- s05 -----------------------------------------------------------------
async function pasteFiles(load, { start = ['BEFORE AFTER', 0, 7] } = {}) {
  await ev(`return await window.__ph.open('Target.md', 'live');`);
  await focusEditor();
  await ev(`return window.__ph.set(${J(start[0])}, ${start[1]}, ${start[2]});`);
  const formats = await load();
  const before = tree();
  await ev(`window.__ph.clearNotices(); return true;`);
  await c.pressKey('v', { ctrl: true });
  await sleep(1500);
  return { formats, value: await ev(`return window.__ph.value();`), treeDelta: tree().filter((x) => !before.includes(x)), notices: await ev(`return window.__ph.notices();`) };
}
const uri = (p) => 'file://' + p.split('/').map(encodeURIComponent).join('/');
await step('s05-files-on-the-clipboard', async (r) => {
  await ev(`return window.__ph.config('attachmentFolderPath', 'Attachments');`);
  r.electronUriListOne = await pasteFiles(() => ev(`return window.__ph.clipBuffer('text/uri-list', ${J(uri(src('photo.png')) + '\r\n')});`));
  r.electronUriListTwo = await pasteFiles(() => ev(`return window.__ph.clipBuffer('text/uri-list', ${J(uri(src('photo.png')) + '\r\n' + uri(src('notes.txt')) + '\r\n')});`));
  // The way a file manager offers a copy, from outside the app.
  const wl = (type, data) => async () => {
    const p = spawn('wl-copy', ['-t', type], { stdio: ['pipe', 'ignore', 'pipe'] });
    p.stdin.end(data);
    await sleep(800);
    return ev(`return window.__ph.formats();`);
  };
  let kb = null;
  try {
    execFileSync('wl-copy', ['--version']);
    kb = spawn('wtype', ['-s', '120000', '-k', 'F24'], { stdio: 'ignore' });
    await sleep(800);
    r.wlUriListOne = await pasteFiles(wl('text/uri-list', uri(src('photo.png')) + '\r\n'));
    r.wlUriListTwo = await pasteFiles(wl('text/uri-list', uri(src('second shot.png')) + '\r\n' + uri(src('notes.txt')) + '\r\n'));
    r.wlGnomeCopied = await pasteFiles(wl('x-special/gnome-copied-files', 'copy\n' + uri(src('photo.png'))));
    r.wlHtml = await pasteFiles(wl('text/html', '<b>from wl-copy</b>'), { start: ['', 0, 0] });
    r.wlPlain = await pasteFiles(wl('text/plain;charset=utf-8', 'from wl-copy plain'), { start: ['', 0, 0] });
    r.wlPng = await pasteFiles(wl('image/png', PNG), { start: ['', 0, 0] });
  } catch (e) { r.wlCopy = 'unavailable: ' + e.message; }
  if (kb) kb.kill();
  return r;
});

// ---- s06 -----------------------------------------------------------------
async function drop(at, data, { start = null, select = null, mode = 'live' } = {}) {
  await ev(`return await window.__ph.open('Drop.md', ${J(mode)});`);
  await ev(`return window.__ph.set('line zero\\nline one\\nline two\\nline three\\n', ${start ? start[0] : 0}, ${start ? start[1] : 0});`);
  if (select) await ev(`return window.__ph.select(${J(select[0])}, ${J(select[1])});`);
  await sleep(200);
  const pt = await ev(`return window.__ph.coords(${at[0]}, ${at[1]});`);
  const x = Math.round(pt.x), y = Math.round(pt.y);
  const before = tree();
  await ev(`window.__ph.clearNotices(); return true;`);
  const payload = { items: [], dragOperationsMask: 1, ...data };
  let dispatched;
  try {
    for (const type of ['dragEnter', 'dragOver', 'drop']) await c.send('Input.dispatchDragEvent', { type, x, y, data: payload });
    dispatched = 'dispatched';
  } catch (e) { dispatched = 'error: ' + e.message; }
  await sleep(1500);
  return { at, point: { x, y }, dispatched, value: await ev(`return window.__ph.value();`), cursor: await ev(`return window.__ph.cursor();`), treeDelta: tree().filter((x) => !before.includes(x)), notices: await ev(`return window.__ph.notices();`) };
}
await step('s06-drops', async (r) => {
  await ev(`return window.__ph.config('attachmentFolderPath', 'Attachments');`);
  r.twoFilesAtPointer = await drop([2, 4], { files: [src('photo.png'), src('notes.txt')] }, { start: [0, 0] });
  r.oneFileAtLineEnd = await drop([1, 8], { files: [src('second shot.png')] }, { start: [3, 2] });
  r.fileInsideVault = await drop([2, 4], { files: [vp('Inside.png')] });
  r.fileInsideVaultCaretElsewhere = await drop([1, 4], { files: [vp('Inside.png')] }, { start: [3, 2] });
  r.twoFilesCaretElsewhere = await drop([1, 4], { files: [src('photo.png'), src('notes.txt')] }, { start: [3, 2] });
  r.threeFilesInsideAndOut = await drop([1, 4], { files: [vp('Inside.png'), src('photo.png'), src('Outside Note.md')] }, { start: [3, 2] });
  r.mdFromOutside = await drop([2, 4], { files: [src('Outside Note.md')] });
  r.html = await drop([2, 4], { items: [{ mimeType: 'text/html', data: '<b>dragged</b> <a href="https://e.com">link</a>' }, { mimeType: 'text/plain', data: 'dragged link' }] });
  r.plain = await drop([2, 4], { items: [{ mimeType: 'text/plain', data: 'plain dragged' }] });
  r.uriListItem = await drop([2, 4], { items: [{ mimeType: 'text/uri-list', data: 'https://example.com/page' }, { mimeType: 'text/plain', data: 'https://example.com/page' }] });
  r.overSelection = await drop([0, 6], { files: [src('photo.png')] }, { select: [{ line: 0, ch: 5 }, { line: 0, ch: 9 }] });
  r.outsideSelection = await drop([2, 4], { files: [src('photo.png')] }, { select: [{ line: 0, ch: 5 }, { line: 0, ch: 9 }] });
  r.htmlSettingOff = await (async () => {
    await ev(`return window.__ph.config('autoConvertHtml', false);`);
    const got = await drop([2, 4], { items: [{ mimeType: 'text/html', data: '<b>dragged</b>' }, { mimeType: 'text/plain', data: 'dragged' }] });
    await ev(`return window.__ph.config('autoConvertHtml', true);`);
    return got;
  })();
  r.undoTwoFiles = await (async () => {
    const got = await drop([2, 4], { files: [src('photo.png'), src('notes.txt')] });
    await c.pressKey('z', { ctrl: true });
    await sleep(500);
    return { dropped: got.value, afterOneUndo: await ev(`return window.__ph.value();`) };
  })();
  r.sourceMode = await drop([2, 4], { files: [src('photo.png')] }, { mode: 'source' });
  return r;
});

out.treeAtEnd = tree();
out.appJsonAtEnd = readIf('.obsidian/app.json');
out.finishedAt = new Date().toISOString();
save();
c.close();
process.exit(0);
