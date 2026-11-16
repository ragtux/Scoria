// Obsidian's PDF view, second pass: what the first one (pdf.mjs) left open.
// Text selection and what it is numbered by, the selection's menu and its
// copies, following a `#page=N&selection=a,b,c,d` link, the two-page spreads,
// "Adapt to theme" and a password-protected file.
//
//   s00  context: version, theme, window
//   s01  pdf.js's text content per page (getTextContent items, marked content
//        included) beside the text layer's DOM, for the typst file and for a
//        hand-written file whose content stream exercises every way two runs
//        can meet: one Tj after another, a small and a large TJ kern, a font
//        change, a line move inside one BT, a superscript, a far gap, an
//        explicit space run, a wrapped paragraph
//   s02  the node numbering: every text-layer span of text.pdf's page 1 and of
//        doc.pdf's page 1 selected in turn (a Range over its text) and copied
//        by "Copy link to selection" through a real right-click; then a
//        partial span, a selection over several spans, one across two pages
//   s03  the mouse and the keyboard: a drag over a word, a double click, a
//        triple click, a drag over two lines, a drag from page 1 to page 2,
//        Ctrl+A on the focused viewer, Ctrl+C, Shift+click; screenshots of the
//        selection's look
//   s04  the menu on a selection, in the leaf and in an embed; every row's copy
//   s05  following a selection link from a note: where the view lands, what
//        marks the text, for how long, and the variants (`&color=`, a link
//        into a page the view is not on, the same link twice)
//   s06  spreads: Two-page (odd) and (even) from the Display options menu, the
//        page rects, fit width and height, Right/Left, the current page,
//        what a re-open keeps; scroll modes if anything offers them
//   s07  Adapt to theme: pixels of the page canvas off and on, in the dark and
//        the light theme, over white, black text, grey text, red, green, blue
//        and grey fills and an RGB image; where the toggle persists
//   s08  a password-protected file (AES-256, user password `secret`): what
//        opens, a wrong password, the right one; an owner-password-only file
//
// Phases: fixtures | run. ONLY_STEPS=s02,s05 runs a subset (prefix match).
// SHOTS names the screenshot folder (default <results>-shots). Needs typst
// and qpdf on PATH for the fixtures.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = process.env.SHOTS || RESULTS.replace(/\.json$/, '-shots');

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const ONLY = process.env.ONLY_STEPS && phase !== 'fixtures' ? process.env.ONLY_STEPS.split(',').filter(Boolean) : null;
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
if (phase === 'fixtures') all.fixtures = out; else all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const STEP_MS = Number(process.env.STEP_MS || 240000);
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout after ' + STEP_MS + ' ms')), STEP_MS); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  const s = out.steps[name];
  console.error(`[pdf-sel] ${name}: ${s.ok ? 'ok' : 'FAILED ' + s.error.split('\n')[0]} (${s.ms} ms)`);
  return s;
}

// ---- fixtures --------------------------------------------------------------
// The same typst file pdf.mjs used, so `selection=8,0,8,17` can be read again.
const DOC_TYP = `#set document(title: "Probe document", author: "probe")
#set page(paper: "a4")
#set heading(numbering: "1.1")
#set text(size: 14pt)

= One
== One point one
#text(size: 40pt)[This is page one.]
#lorem(60)

#pagebreak()
= Two
== Two point one
#text(size: 40pt)[This is page two.]
#lorem(60)

#pagebreak()
#set page(paper: "a4", flipped: true)
= Three
== Three point one
#text(size: 40pt)[This is page three.]
#lorem(60)

#pagebreak()
#set page(paper: "a4", flipped: false)
= Four
== Four point one
#text(size: 40pt)[This is page four.]
#lorem(60)

#pagebreak()
= Five
== Five point one
#text(size: 40pt)[This is page five.]
#lorem(60)
`;

// A hand-written PDF: standard 14 fonts, no marked content, every way two
// text runs can meet, and colour swatches for Adapt to theme.
function textPdf() {
  const page1 = [
    'BT /F1 12 Tf 72 720 Td (Hello) Tj ( world) Tj ET',
    'BT /F1 12 Tf 72 700 Td [(Kern)-120(ed)] TJ ET',
    'BT /F1 12 Tf 72 680 Td [(Wide)-2000(gap)] TJ ET',
    'BT /F1 12 Tf 72 660 Td (Plain ) Tj /F2 12 Tf (bold) Tj /F1 12 Tf ( again) Tj ET',
    'BT /F1 12 Tf 72 640 Td (Line one) Tj 0 -14 Td (Line two) Tj ET',
    'BT /F1 12 Tf 72 600 Td (E = mc) Tj 0 5 Td /F1 8 Tf (2) Tj ET',
    'BT /F1 12 Tf 72 580 Td (Left column) Tj 300 0 Td (Right column) Tj ET',
    'BT /F1 12 Tf 72 560 Td (Tab) Tj 60 0 Td (stop) Tj ET',
    'BT /F1 12 Tf 72 540 Td (Spaced) Tj ( ) Tj (words) Tj ET',
    'BT /F1 10 Tf 12 TL 72 510 Td (First wrapped line of a paragraph that goes on) Tj T* (and its second line.) Tj ET',
    '1 0 0 rg 72 400 100 50 re f',
    '0 0 1 rg 200 400 100 50 re f',
    '0 0.5 0 rg 328 400 100 50 re f',
    '0.5 g 456 400 100 50 re f',
    '0.4 g BT /F1 24 Tf 72 350 Td (Grey text) Tj ET',
    '0 g BT /F1 24 Tf 300 350 Td (Black text) Tj ET',
    'q 100 0 0 50 72 270 cm BI /W 2 /H 1 /CS /RGB /BPC 8 ID \xff\x80\x00\x00\x80\xff EI Q',
  ].join('\n');
  const page2 = [
    'BT /F1 12 Tf 72 720 Td (Second page first line) Tj ET',
    'BT /F1 12 Tf 72 700 Td (Second page second line) Tj ET',
  ].join('\n');
  const objs = [];
  objs[1] = '<< /Type /Catalog /Pages 2 0 R >>';
  objs[2] = '<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>';
  const res = '<< /Font << /F1 5 0 R /F2 6 0 R >> >>';
  objs[3] = `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources ${res} /Contents 7 0 R >>`;
  objs[4] = `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources ${res} /Contents 8 0 R >>`;
  objs[5] = '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>';
  objs[6] = '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>';
  objs[7] = `<< /Length ${Buffer.byteLength(page1, 'latin1')} >>\nstream\n${page1}\nendstream`;
  objs[8] = `<< /Length ${Buffer.byteLength(page2, 'latin1')} >>\nstream\n${page2}\nendstream`;
  let body = '%PDF-1.4\n%\xe2\xe3\xcf\xd3\n';
  const offsets = [];
  for (let i = 1; i < objs.length; i++) { offsets[i] = Buffer.byteLength(body, 'latin1'); body += `${i} 0 obj\n${objs[i]}\nendobj\n`; }
  const xref = Buffer.byteLength(body, 'latin1');
  body += `xref\n0 ${objs.length}\n0000000000 65535 f \n`;
  for (let i = 1; i < objs.length; i++) body += String(offsets[i]).padStart(10, '0') + ' 00000 n \n';
  body += `trailer\n<< /Size ${objs.length} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(body, 'latin1');
}

if (phase === 'fixtures') {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'pdf-sel-typst-'));
  fs.writeFileSync(path.join(tmp, 'doc.typ'), DOC_TYP);
  execSync('typst compile doc.typ doc.pdf', { cwd: tmp, stdio: 'inherit' });
  fs.copyFileSync(path.join(tmp, 'doc.pdf'), path.join(VAULT, 'doc.pdf'));
  fs.writeFileSync(path.join(VAULT, 'text.pdf'), textPdf());
  execSync(`qpdf --encrypt secret owner 256 -- "${path.join(tmp, 'doc.pdf')}" "${path.join(VAULT, 'locked.pdf')}"`);
  execSync(`qpdf --encrypt "" owner 256 -- "${path.join(tmp, 'doc.pdf')}" "${path.join(VAULT, 'owner-only.pdf')}"`);
  execSync(`qpdf --allow-weak-crypto --encrypt secret owner 128 --use-aes=n -- "${path.join(tmp, 'doc.pdf')}" "${path.join(VAULT, 'locked-rc4.pdf')}"`);
  fs.mkdirSync(path.join(VAULT, '.obsidian'), { recursive: true });
  fs.writeFileSync(path.join(VAULT, '.obsidian', 'appearance.json'), JSON.stringify({ theme: 'obsidian' }, null, 2));
  fs.writeFileSync(path.join(VAULT, 'Links.md'), [
    '# Links', '',
    'One span: [[doc.pdf#page=1&selection=8,0,8,17|doc, page 1]]', '',
    'Partial: [[doc.pdf#page=1&selection=8,5,8,12|partial]]', '',
    'Spans: [[text.pdf#page=1&selection=0,2,4,3|spans]]', '',
    'Colour: [[doc.pdf#page=1&selection=8,0,8,17&color=red|coloured]]', '',
    'Page two: [[doc.pdf#page=2&selection=8,0,8,17|page two]]', '',
    'Bad: [[doc.pdf#page=1&selection=999,0,999,4|bad]]', '',
    'Embed:', '', '![[doc.pdf]]', '',
  ].join('\n'));
  out.fixtures = { files: fs.readdirSync(VAULT), typst: execSync('typst --version').toString().trim(), qpdf: execSync('qpdf --version').toString().split('\n')[0] };
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

// ---- run -------------------------------------------------------------------
const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }
const ev = (b) => c.evaluate(b);
let shotN = 0;
async function shot(name, clip) {
  fs.mkdirSync(SHOTS, { recursive: true });
  const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
  const r = await c.send('Page.captureScreenshot', clip ? { format: 'png', clip: { ...clip, scale: 1 } } : { format: 'png' });
  fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
  return f;
}

await ev(`
window.__s = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls(el) { if (!el) return ''; return typeof el.className === 'string' ? el.className : (el.getAttribute && el.getAttribute('class')) || ''; },
  rect(el) { if (!el) return null; const b = el.getBoundingClientRect(); const r = (n) => Math.round(n * 100) / 100; return { x: r(b.x), y: r(b.y), w: r(b.width), h: r(b.height) }; },
  async file(p) { for (let i = 0; i < 40; i++) { const f = app.vault.getAbstractFileByPath(p); if (f) return f; await this.sleep(500); } throw new Error('missing ' + p); },
  pdfLeaf() { return app.workspace.getLeavesOfType('pdf')[0] || null; },
  async openPdf(p) {
    const old = app.workspace.getLeavesOfType('pdf').concat(app.workspace.getLeavesOfType('markdown'));
    const f = await this.file(p);
    const leaf = app.workspace.getLeaf('tab');
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    for (const l of old) if (l !== leaf) l.detach();
    return leaf;
  },
  async openMd(p, mode) {
    const old = app.workspace.getLeavesOfType('markdown').concat(app.workspace.getLeavesOfType('pdf'));
    const f = await this.file(p);
    const leaf = app.workspace.getLeaf('tab');
    await leaf.setViewState({ type: 'markdown', state: { file: f.path, mode: mode || 'preview', source: false } });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    for (const l of old) if (l !== leaf) l.detach();
    return leaf;
  },
  owner(leaf) { leaf = leaf || this.pdfLeaf(); const v = leaf && leaf.view; return v && v.viewer && v.viewer.child && v.viewer.child.pdfViewer; },
  viewer(leaf) { const o = this.owner(leaf); return o && o.pdfViewer; },
  content(leaf) { leaf = leaf || this.pdfLeaf(); return leaf.view.containerEl.querySelector('.view-content'); },
  page(n, root) { return (root || this.content()).querySelector('.page[data-page-number="' + n + '"]'); },
  spans(n, root) { const p = this.page(n, root); return p ? Array.from(p.querySelectorAll('.textLayer span.textLayerNode, .textLayer > span:not(.markedContent)')) : []; },
  vstate(v) { v = v || this.viewer(); if (!v) return null; const cont = v.container; return { page: v.currentPageNumber, scale: v.currentScale, scaleValue: v.currentScaleValue, spreadMode: v.spreadMode, scrollMode: v.scrollMode, pagesCount: v.pagesCount, scrollTop: cont.scrollTop, scrollLeft: cont.scrollLeft, clientWidth: cont.clientWidth, clientHeight: cont.clientHeight, scrollHeight: cont.scrollHeight, scrollWidth: cont.scrollWidth }; },
  layerTree(n, root) {
    const tl = this.page(n, root) && this.page(n, root).querySelector('.textLayer'); if (!tl) return null;
    const walk = (el) => Array.from(el.childNodes).map((ch) => {
      if (ch.nodeType === 3) return { text: ch.textContent };
      const o = { tag: ch.tagName.toLowerCase(), cls: this.cls(ch) || undefined };
      if (ch.tagName === 'BR') return o;
      if (ch.classList.contains('markedContent')) { o.id = ch.id || undefined; o.children = walk(ch); return o; }
      o.text = ch.textContent; o.style = ch.getAttribute('style'); o.rect = this.rect(ch); return o;
    });
    return { cls: this.cls(tl), style: tl.getAttribute('style'), childNodes: tl.childNodes.length, tree: walk(tl) };
  },
  ui() { return { menus: Array.from(document.querySelectorAll('.menu')).map((m) => this.cls(m)), modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => this.cls(m)), notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent.trim()), focus: document.activeElement ? document.activeElement.tagName.toLowerCase() + '.' + this.cls(document.activeElement) : null, activeLeafType: app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType() }; },
  menus() {
    return Array.from(document.querySelectorAll('.menu')).map((m) => ({ cls: this.cls(m), rect: this.rect(m),
      items: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((e) => ({
        separator: e.classList.contains('menu-separator'), cls: this.cls(e), section: e.getAttribute('data-section'),
        title: e.querySelector('.menu-item-title') ? e.querySelector('.menu-item-title').textContent.trim() : (e.textContent || '').trim(),
        icon: e.querySelector('.menu-item-icon svg') ? e.querySelector('.menu-item-icon svg').getAttribute('class') : null,
        checked: e.classList.contains('mod-checked') || !!e.querySelector('.mod-checked'), submenu: e.classList.contains('mod-submenu'),
        html: e.outerHTML.length < 1500 ? e.outerHTML : e.outerHTML.slice(0, 1500) })) }));
  },
  dismiss() { for (const m of document.querySelectorAll('.menu')) m.remove(); return true; },
  selInfo() {
    const s = window.getSelection(); if (!s.rangeCount) return { text: '', rangeCount: 0 };
    const r = s.getRangeAt(0);
    const desc = (n) => { const el = n.nodeType === 3 ? n.parentElement : n; const page = el && el.closest('.page'); const spans = page ? Array.from(page.querySelectorAll('.textLayer span.textLayerNode, .textLayer > span:not(.markedContent)')) : []; return { tag: el && el.tagName.toLowerCase(), cls: this.cls(el), page: page && page.getAttribute('data-page-number'), spanIndex: spans.indexOf(el), text: el && el.textContent.slice(0, 60), nodeType: n.nodeType }; };
    return { text: s.toString(), rangeCount: s.rangeCount, collapsed: s.isCollapsed, start: desc(r.startContainer), startOffset: r.startOffset, end: desc(r.endContainer), endOffset: r.endOffset, rects: Array.from(r.getClientRects()).slice(0, 12).map((b) => ({ x: Math.round(b.x * 10) / 10, y: Math.round(b.y * 10) / 10, w: Math.round(b.width * 10) / 10, h: Math.round(b.height * 10) / 10 })) };
  },
  // Select spans [a..b] of page n (inclusive), from character so to eo of the last; offsets default to whole spans.
  selectSpans(n, a, so, b, eo, pageB) {
    const sa = this.spans(n); const sb = this.spans(pageB || n);
    const A = sa[a], B = sb[b]; if (!A || !B) return { error: 'no span', count: sa.length };
    const ta = A.firstChild || A, tb = B.firstChild || B;
    const r = document.createRange(); r.setStart(ta, so == null ? 0 : so); r.setEnd(tb, eo == null ? (tb.textContent || '').length : eo);
    const s = window.getSelection(); s.removeAllRanges(); s.addRange(r);
    A.scrollIntoView({ block: 'center' });
    return { ok: true };
  },
  clearHistory() { localStorage.removeItem('pdfjs.history'); return true; },
};
return true;`);
const S = 'const S = window.__s;';
const menuRow = async (title, wait = 900) => {
  const r = await ev(`${S} const items = Array.from(document.querySelectorAll('.menu .menu-item')); const it = items.find((e) => ((e.querySelector('.menu-item-title') || e).textContent || '').trim() === ${JSON.stringify(title)}); return it ? S.rect(it) : null;`);
  if (!r) throw new Error('no menu row ' + title + ' in ' + JSON.stringify(await c.openMenuItems()) + ' ui ' + JSON.stringify(await ev(`${S} return S.ui();`)));
  await c.clickAt(r.x + r.w / 2, r.y + r.h / 2); await sleep(wait); return r;
};
const openPdf = async (p, wait = 2500) => { await ev(`${S} S.clearHistory(); app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse(); await S.openPdf(${JSON.stringify(p)}); return true;`); await sleep(wait); };
const selRect = () => ev(`${S} const rs = window.getSelection().rangeCount ? Array.from(window.getSelection().getRangeAt(0).getClientRects()).filter((b) => b.width > 1) : []; if (!rs.length) return null; const b = rs[0]; return { x: b.x + Math.min(b.width / 2, 20), y: b.y + b.height / 2 };`);
const sentinel = () => c.writeClipboard('PROBE-SENTINEL-' + Date.now());
const clip = async () => { const t = await c.readClipboard(); return /^PROBE-SENTINEL-/.test(t) ? '(no clipboard write)' : t; };
async function copyRow(title) {
  const at = await selRect(); if (!at) return { error: 'no selection rect' };
  await sentinel();
  await c.clickAt(at.x, at.y, { button: 'right' }); await sleep(600);
  const items = await c.openMenuItems();
  try { await menuRow(title, 600); } catch (e) { await ev(`${S} S.dismiss(); return true;`); return { error: e.message, items }; }
  return { clipboard: await clip(), items };
}

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio, theme: document.body.classList.contains('theme-dark') ? 'dark' : 'light', apiVersion: (() => { try { return require('obsidian').apiVersion; } catch { return null; } })() };`);
  return out.version;
});

await step('s01-text-content', async () => {
  const r = {};
  for (const f of ['text.pdf', 'doc.pdf']) {
    await openPdf(f);
    r[f] = await ev(`${S} const v = S.viewer(); const res = { pages: v.pagesCount, page: [] };
      for (const n of [1, 2]) {
        const pv = v.getPageView(n - 1); const page = pv.pdfPage;
        const tc = await page.getTextContent({ includeMarkedContent: true });
        const tc2 = await page.getTextContent();
        page.view;
        S.page(n).scrollIntoView(); await S.sleep(1200);
        const layer = pv.textLayer;
        res.page.push({ n, view: page.view, rotate: page.rotate,
          items: tc.items.map((it) => it.type ? { type: it.type, id: it.id, tag: it.tag } : { str: it.str, hasEOL: it.hasEOL, dir: it.dir, transform: it.transform.map((x) => Math.round(x * 1000) / 1000), width: Math.round(it.width * 1000) / 1000, height: Math.round(it.height * 1000) / 1000, fontName: it.fontName }),
          plainItems: tc2.items.length,
          styles: tc.styles,
          layerKeys: layer ? Object.keys(layer) : null,
          textDivs: layer && layer.textDivs ? layer.textDivs.length : null,
          textContentItemsStr: layer && layer.textContentItemsStr ? layer.textContentItemsStr.slice(0, 80) : null,
          tree: S.layerTree(n) });
      }
      return res;`);
  }
  return r;
});

await step('s02-numbering', async (partial) => {
  const r = { perSpan: {} };
  for (const f of ['text.pdf', 'doc.pdf']) {
    await openPdf(f);
    const count = await ev(`${S} S.page(1).scrollIntoView(); await S.sleep(1200); return S.spans(1).length;`);
    const rows = [];
    for (let i = 0; i < Math.min(count, f === 'doc.pdf' ? 14 : 40); i++) {
      const sel = await ev(`${S} const r = S.selectSpans(1, ${i}, null, ${i}, null); await S.sleep(250); return { r, info: S.selInfo() };`);
      if (!sel.info.text) { rows.push({ i, empty: true, sel }); continue; }
      const res = await copyRow('Copy link to selection');
      rows.push({ i, text: sel.info.text, link: res.clipboard, error: res.error });
      partial[f] = rows;
    }
    r.perSpan[f] = rows;
  }
  // Partial, several spans, across pages, in text.pdf.
  await openPdf('text.pdf');
  await ev(`${S} S.page(1).scrollIntoView(); await S.sleep(1200); return true;`);
  const cases = [['partial', 1, 2, 1, 5], ['spans 0..4', 0, 2, 4, 3], ['spans 2..7 whole', 2, null, 7, null]];
  r.cases = [];
  for (const [name, a, so, b, eo] of cases) {
    const info = await ev(`${S} S.selectSpans(1, ${a}, ${so}, ${b}, ${eo}); await S.sleep(250); return S.selInfo();`);
    const link = await copyRow('Copy link to selection');
    const quote = await (async () => { await ev(`${S} S.selectSpans(1, ${a}, ${so}, ${b}, ${eo}); return true;`); await sleep(250); return copyRow('Copy as quote'); })();
    const copy = await (async () => { await ev(`${S} S.selectSpans(1, ${a}, ${so}, ${b}, ${eo}); return true;`); await sleep(250); return copyRow('Copy'); })();
    r.cases.push({ name, info, link: link.clipboard, quote: quote.clipboard, copy: copy.clipboard, items: link.items });
  }
  // Across pages: the last span of page 1 to the first of page 2.
  const last = await ev(`${S} const v = S.viewer(); S.page(2).scrollIntoView(); await S.sleep(1200); S.page(1).scrollIntoView({ block: 'end' }); await S.sleep(800); return S.spans(1).length;`);
  const cross = await ev(`${S} const a = S.spans(1).length - 1; const r = S.selectSpans(1, a, 0, 0, null, 2); await S.sleep(300); return { r, info: S.selInfo() };`);
  r.cross = { info: cross, link: (await copyRow('Copy link to selection')).clipboard };
  await ev(`${S} S.selectSpans(1, S.spans(1).length - 1, 0, 0, null, 2); return true;`); await sleep(250);
  r.cross.quote = (await copyRow('Copy as quote')).clipboard;
  r.crossLastIndex = last;
  return r;
});

await step('s03-mouse-keys', async () => {
  const r = {};
  await openPdf('text.pdf');
  const spanRect = (i) => ev(`${S} S.page(1).scrollIntoView(); const s = S.spans(1)[${i}]; return s ? S.rect(s) : null;`);
  await ev(`${S} S.page(1).scrollIntoView(); await S.sleep(1200); return true;`);
  const a = await spanRect(0); // "Hello world" or "Hello"
  r.span0 = a;
  // Drag over part of the first span.
  await ev(`window.getSelection().removeAllRanges(); return true;`);
  await c.drag(a.x + 1, a.y + a.h / 2, a.x + a.w * 0.4, a.y + a.h / 2, { steps: 8, pauseMs: 30 }); await sleep(300);
  r.dragPart = await ev(`${S} return S.selInfo();`);
  r.shotDrag = await shot('drag-part');
  // Double click a word.
  await ev(`window.getSelection().removeAllRanges(); return true;`);
  await c.clickAt(a.x + a.w * 0.75, a.y + a.h / 2, { clickCount: 1 });
  await c.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: Math.round(a.x + a.w * 0.75), y: Math.round(a.y + a.h / 2), button: 'left', clickCount: 2 });
  await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: Math.round(a.x + a.w * 0.75), y: Math.round(a.y + a.h / 2), button: 'left', clickCount: 2 });
  await sleep(300);
  r.doubleClick = await ev(`${S} return S.selInfo();`);
  r.shotDouble = await shot('double-click');
  // Triple click on the wrapped paragraph's first line.
  const para = await ev(`${S} const sp = S.spans(1); const i = sp.findIndex((s) => /First wrapped/.test(s.textContent)); return i < 0 ? null : { i, rect: S.rect(sp[i]) };`);
  if (para) {
    const x = Math.round(para.rect.x + 20), y = Math.round(para.rect.y + para.rect.h / 2);
    await ev(`window.getSelection().removeAllRanges(); return true;`);
    for (const n of [1, 2, 3]) {
      await c.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: n });
      await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: n });
      await sleep(60);
    }
    await sleep(300);
    r.tripleClick = await ev(`${S} return S.selInfo();`);
    r.shotTriple = await shot('triple-click');
    await c.clickAt(x, y + 200); await sleep(200);
  }
  // Drag over two lines (Line one -> Line two).
  const l1 = await ev(`${S} const sp = S.spans(1); const i = sp.findIndex((s) => /Line one/.test(s.textContent)); const j = sp.findIndex((s) => /Line two/.test(s.textContent)); return i < 0 || j < 0 ? null : { a: S.rect(sp[i]), b: S.rect(sp[j]) };`);
  if (l1) {
    await ev(`window.getSelection().removeAllRanges(); return true;`);
    await c.drag(l1.a.x + 1, l1.a.y + l1.a.h / 2, l1.b.x + l1.b.w - 2, l1.b.y + l1.b.h / 2, { steps: 10, pauseMs: 30 }); await sleep(300);
    r.dragTwoLines = await ev(`${S} return S.selInfo();`);
    r.shotTwoLines = await shot('drag-two-lines');
    await sentinel(); await c.pressKey('c', { ctrl: true }); await sleep(400);
    r.dragTwoLinesCtrlC = await clip();
  }
  // Drag from the middle of the page into blank space below the text.
  const blank = await ev(`${S} const p = S.rect(S.page(1)); return p;`);
  await ev(`window.getSelection().removeAllRanges(); return true;`);
  await c.drag(a.x + 1, a.y + a.h / 2, blank.x + blank.w / 2, blank.y + blank.h * 0.9, { steps: 12, pauseMs: 30 }); await sleep(300);
  r.dragIntoBlank = await ev(`${S} return S.selInfo();`);
  r.shotIntoBlank = await shot('drag-into-blank');
  // Shift+click extends?
  await ev(`window.getSelection().removeAllRanges(); return true;`);
  await c.clickAt(a.x + 2, a.y + a.h / 2); await sleep(150);
  const b2 = await spanRect(3);
  await c.clickAt(b2.x + b2.w - 2, b2.y + b2.h / 2, { mods: { shift: true } }); await sleep(300);
  r.shiftClick = await ev(`${S} return S.selInfo();`);
  // Ctrl+A with the viewer focused (a click on a page first).
  await c.clickAt(blank.x + blank.w / 2, blank.y + blank.h * 0.95); await sleep(200);
  r.focusAfterClick = await ev(`${S} return S.ui().focus;`);
  await c.pressKey('a', { ctrl: true }); await sleep(300);
  r.ctrlA = await ev(`${S} const i = S.selInfo(); return { length: i.text.length, head: i.text.slice(0, 80), start: i.start, end: i.end };`);
  r.shotCtrlA = await shot('ctrl-a');
  // Click on a span, then Ctrl+A.
  await c.clickAt(a.x + 3, a.y + a.h / 2); await sleep(200);
  r.focusAfterSpanClick = await ev(`${S} return S.ui().focus;`);
  await c.pressKey('a', { ctrl: true }); await sleep(300);
  r.ctrlAOnSpan = await ev(`${S} const i = S.selInfo(); return { length: i.text.length, head: i.text.slice(0, 120), tail: i.text.slice(-60), start: i.start, end: i.end };`);
  // Across pages by dragging.
  await ev(`${S} const v = S.viewer(); v.container.scrollTop = S.page(2).offsetTop - 300; await S.sleep(1000); return true;`);
  const cp = await ev(`${S} const a = S.spans(1); const b = S.spans(2); return { a: a.length ? S.rect(a[a.length - 1]) : null, b: b.length ? S.rect(b[0]) : null };`);
  if (cp.a && cp.b) {
    await ev(`window.getSelection().removeAllRanges(); return true;`);
    await c.drag(cp.a.x + 1, cp.a.y + cp.a.h / 2, cp.b.x + cp.b.w - 2, cp.b.y + cp.b.h / 2, { steps: 14, pauseMs: 30 }); await sleep(300);
    r.dragAcrossPages = await ev(`${S} return S.selInfo();`);
    r.shotAcross = await shot('drag-across-pages');
  } else r.dragAcrossPages = { skipped: cp };
  // The selection's colours.
  r.look = await ev(`${S} const tl = S.page(1).querySelector('.textLayer'); const sp = S.spans(1)[0]; const sel = getComputedStyle(sp, '::selection'); const tsel = getComputedStyle(tl, '::selection');
    return { textLayer: { opacity: getComputedStyle(tl).opacity, mixBlend: getComputedStyle(tl).mixBlendMode, color: getComputedStyle(tl).color, lineHeight: getComputedStyle(tl).lineHeight }, span: { color: getComputedStyle(sp).color, transform: getComputedStyle(sp).transform, cursor: getComputedStyle(sp).cursor }, spanSelection: { bg: sel.backgroundColor, color: sel.color }, layerSelection: { bg: tsel.backgroundColor, color: tsel.color }, accent: getComputedStyle(document.body).getPropertyValue('--text-selection'), endOfContent: !!tl.querySelector('.endOfContent'), endStyle: tl.querySelector('.endOfContent') ? tl.querySelector('.endOfContent').getAttribute('style') : null };`);
  await ev(`window.getSelection().removeAllRanges(); return true;`);
  return r;
});

await step('s04-menu', async () => {
  const r = {};
  await openPdf('doc.pdf');
  await ev(`${S} S.page(1).scrollIntoView(); await S.sleep(1200); S.selectSpans(1, 8, null, 8, null); return true;`); await sleep(300);
  const at = await selRect();
  await c.clickAt(at.x, at.y, { button: 'right' }); await sleep(700);
  r.leafMenu = await ev(`${S} return S.menus();`);
  r.shotMenu = await shot('selection-menu');
  // Hover each row with a submenu.
  r.submenus = [];
  for (const it of (r.leafMenu[0] && r.leafMenu[0].items) || []) {
    if (!it.submenu) continue;
    const rr = await ev(`${S} const items = Array.from(document.querySelectorAll('.menu .menu-item')); const e = items.find((x) => (x.querySelector('.menu-item-title') || x).textContent.trim() === ${JSON.stringify(it.title)}); return e ? S.rect(e) : null;`);
    if (rr) { await c.mouseMove(rr.x + rr.w / 2, rr.y + rr.h / 2); await sleep(700); r.submenus.push({ title: it.title, menus: await ev(`${S} return S.menus();`) }); }
  }
  await ev(`${S} S.dismiss(); return true;`);
  // In an embed, in the reading view.
  await ev(`${S} await S.openMd('Links.md', 'preview'); return true;`); await sleep(3000);
  const emb = await ev(`${S} const e = document.querySelector('.workspace-leaf.mod-active .pdf-embed'); if (!e) return null; e.scrollIntoView(); await S.sleep(2000); const sp = Array.from(e.querySelectorAll('.page[data-page-number="1"] .textLayer span.textLayerNode')); const s = sp.find((x) => /This is page one/.test(x.textContent)); if (!s) return { spans: sp.length }; const range = document.createRange(); range.selectNodeContents(s); const sel = window.getSelection(); sel.removeAllRanges(); sel.addRange(range); const b = s.getBoundingClientRect(); return { x: b.x + 10, y: b.y + b.height / 2 };`);
  if (emb && emb.x) {
    await sentinel();
    await c.clickAt(emb.x, emb.y, { button: 'right' }); await sleep(700);
    r.embedMenu = await ev(`${S} return S.menus();`);
    if (r.embedMenu.length) { try { await menuRow('Copy link to selection'); r.embedLink = await clip(); } catch (e) { r.embedLinkError = e.message; } }
    await ev(`${S} S.dismiss(); return true;`);
  } else r.embed = emb;
  return r;
});

await step('s05-follow', async () => {
  const r = {};
  const follow = async (label, settle = [300, 1500, 4000, 8000]) => {
    await ev(`${S} S.clearHistory(); await S.openMd('Links.md', 'preview'); return true;`); await sleep(1500);
    const link = await ev(`${S} const a = Array.from(document.querySelectorAll('.workspace-leaf.mod-active a.internal-link')).find((x) => x.textContent.trim() === ${JSON.stringify(label)}); if (!a) return null; a.scrollIntoView({ block: 'center' }); await S.sleep(200); return { href: a.getAttribute('data-href'), rect: S.rect(a) };`);
    if (!link) return { error: 'no link ' + label };
    await c.mouseMove(link.rect.x + link.rect.w / 2, link.rect.y + link.rect.h / 2); await sleep(400);
    await c.clickAt(link.rect.x + link.rect.w / 2, link.rect.y + link.rect.h / 2);
    await sleep(800);
    let clicks = 1;
    if (!(await ev(`${S} return !!S.pdfLeaf();`))) { await c.clickAt(link.rect.x + link.rect.w / 2, link.rect.y + link.rect.h / 2); clicks++; }
    let fallback = false;
    if (!(await ev(`${S} return !!S.pdfLeaf();`))) { fallback = true; await ev(`app.workspace.openLinkText(${JSON.stringify(link.href)}, 'Links.md', false); return true;`); await sleep(300); }
    const snaps = [{ clicks, fallback }];
    let t = 0;
    for (const at of settle) {
      await sleep(at - t); t = at;
      snaps.push(await ev(`${S} const leaf = S.pdfLeaf(); if (!leaf) return { at: ${at}, noPdf: true, ui: S.ui() };
        const v = S.viewer(leaf); const content = S.content(leaf); const st = S.vstate(v);
        const marks = Array.from(content.querySelectorAll('.page *')).filter((e) => { const c = S.cls(e); return /highlight|select|mod-|focus|flash/i.test(c) && !/textLayerNode|markedContent|canvasWrapper|textLayer$/.test(c); }).slice(0, 20).map((e) => ({ tag: e.tagName.toLowerCase(), cls: S.cls(e), text: (e.textContent || '').slice(0, 60), rect: S.rect(e), bg: getComputedStyle(e).backgroundColor, style: e.getAttribute('style'), parentCls: S.cls(e.parentElement), page: e.closest('.page') && e.closest('.page').getAttribute('data-page-number') }));
        const pageTop = (n) => { const p = S.page(n, content); return p ? p.offsetTop : null; };
        return { at: ${at}, state: leaf.view.getState(), viewState: leaf.getViewState().state, subpath: S.owner(leaf) && S.owner(leaf).subpath, vstate: st, pageTops: [1, 2].map(pageTop), sel: S.selInfo(), marks, focus: S.ui().focus };`));
      if (at === 1500) snaps[snaps.length - 1].shot = await shot('follow-' + label.replace(/\W+/g, '-'));
    }
    return { link, snaps };
  };
  const labels = (process.env.FOLLOW || 'doc, page 1|partial|spans|coloured|page two|bad').split('|');
  for (const label of labels) r[label] = await follow(label);
  // The same link twice into an open leaf, and a markdown highlight rule dump.
  r.css = await ev(`const out = []; for (const sh of document.styleSheets) { let rules; try { rules = sh.cssRules; } catch { continue; } for (const ru of rules) { const t = ru.cssText || ''; if (/textLayer[^{]*(highlight|selected|mod-)|pdf-.*highlight|\\.highlight\\b/.test(t) && t.length < 600) out.push(t); } } return out.slice(0, 80);`);
  return r;
});

await step('s06-spreads', async () => {
  const r = {};
  await openPdf('doc.pdf');
  const dump = () => ev(`${S} const content = S.content(); const v = S.viewer(); return { vstate: S.vstate(v), spreads: Array.from(content.querySelectorAll('.pdfViewer > .spread')).map((sp) => ({ rect: S.rect(sp), pages: Array.from(sp.querySelectorAll('.page')).map((p) => p.getAttribute('data-page-number')), style: sp.getAttribute('style') })), pages: Array.from(content.querySelectorAll('.page')).map((p) => ({ n: p.getAttribute('data-page-number'), rect: S.rect(p), offsetTop: p.offsetTop, offsetLeft: p.offsetLeft, style: p.getAttribute('style') })), viewerCls: S.cls(content.querySelector('.pdfViewer')), viewerStyle: content.querySelector('.pdfViewer').getAttribute('style'), container: S.rect(content.querySelector('.pdf-viewer-container')) };`);
  r.single = await dump();
  const display = async () => { await c.clickElement('.workspace-leaf.mod-active .pdf-toolbar [aria-label="Display options"]'); await sleep(600); return ev(`${S} return S.menus();`); };
  r.menu = await display();
  for (const [key, row] of [['odd', 'Two-page (odd)'], ['even', 'Two-page (even)']]) {
    await menuRow(row, 1500);
    r[key] = await dump();
    r[key].shot = await shot('spread-' + key);
    r[key].menuAfter = await display();
    await menuRow('Fit height', 1200);
    r[key].fitHeight = await dump();
    await display(); await menuRow('Fit width', 1200);
    // Right and Left.
    await c.clickAt(r[key].fitHeight.container.x + 10, r[key].fitHeight.container.y + 100); await sleep(200);
    const keys = [];
    for (const k of ['ArrowRight', 'ArrowRight', 'ArrowLeft', 'End', 'Home']) { await c.pressKey(k); await sleep(500); keys.push({ k, st: await ev(`${S} return S.vstate();`) }); }
    r[key].keys = keys;
    await display();
  }
  // Back to single page; what a re-open keeps.
  await menuRow('Two-page (odd)', 1200);
  r.history = await ev(`return localStorage.getItem('pdfjs.history');`);
  await ev(`${S} const f = await S.file('doc.pdf'); const leaf = S.pdfLeaf(); await leaf.openFile(await S.file('text.pdf')); await S.sleep(1500); await leaf.openFile(f); return true;`); await sleep(2500);
  r.reopen = await dump();
  await display(); await menuRow('Single page', 1000);
  r.backToSingle = await dump();
  // Does anything offer scroll modes?
  r.scrollModeUi = await ev(`${S} const c = S.content(); return Array.from(c.querySelectorAll('[aria-label]')).map((e) => e.getAttribute('aria-label'));`);
  return r;
});

await step('s07-adapt', async () => {
  const r = {};
  // Canvas pixels at PDF points of text.pdf page 1 (612 x 792).
  const POINTS = { white: [560, 760], black: [302, 357], grey: [74, 357], red: [122, 425], blue: [250, 425], green: [378, 425], greyFill: [506, 425], imgLeft: [97, 295], imgRight: [147, 295], hello: [74, 723] };
  const sample = () => ev(`${S} const p = S.page(1); const cv = p.querySelector('canvas'); if (!cv) return null; const ctx = cv.getContext('2d', { willReadFrequently: true }); const sx = cv.width / 612, sy = cv.height / 792; const pts = ${JSON.stringify(POINTS)}; const res = {};
    for (const [k, [x, y]] of Object.entries(pts)) { const d = ctx.getImageData(Math.round(x * sx), Math.round((792 - y) * sy), 1, 1).data; res[k] = [d[0], d[1], d[2], d[3]]; }
    const near = {}; for (const k of ['black', 'grey', 'hello']) { const [x, y] = pts[k]; const d = ctx.getImageData(Math.round(x * sx), Math.round((792 - y) * sy) - 12, 40, 24).data; let min = [255, 255, 255], max = [0, 0, 0]; for (let i = 0; i < d.length; i += 4) { const lum = d[i] + d[i + 1] + d[i + 2]; if (lum < min[0] + min[1] + min[2]) min = [d[i], d[i + 1], d[i + 2]]; if (lum > max[0] + max[1] + max[2]) max = [d[i], d[i + 1], d[i + 2]]; } near[k] = { darkest: min, lightest: max }; }
    return { canvas: [cv.width, cv.height], style: cv.getAttribute('style'), filter: getComputedStyle(cv).filter, wrapperFilter: getComputedStyle(cv.parentElement).filter, pageFilter: getComputedStyle(p).filter, mix: getComputedStyle(cv).mixBlendMode, px: res, near };`);
  const stateOf = () => ev(`${S} const o = S.owner(); const c = S.content(); return { pageInvert: o && o.pageInvert, pageBackground: o && o.pageBackground, containerCls: S.cls(c.querySelector('.pdf-container')), viewerBg: getComputedStyle(c.querySelector('.pdf-viewer-container')).backgroundColor, pageBg: getComputedStyle(S.page(1)).backgroundColor, pdfBackground: getComputedStyle(c).getPropertyValue('--pdf-background'), opts: o && o.pdfViewer && o.pdfViewer.pageColors };`);
  const displayBtn = async () => { const r = await ev(`${S} const leaf = app.workspace.activeLeaf; const b = Array.from(leaf.view.containerEl.querySelectorAll('[aria-label="Display options"]')).find((e) => e.getBoundingClientRect().width > 0); return b ? S.rect(b) : { none: true, leafType: leaf.view.getViewType(), n: document.querySelectorAll('[aria-label="Display options"]').length };`); if (r.none) throw new Error('no display button ' + JSON.stringify(r)); await c.clickAt(r.x + r.w / 2, r.y + r.h / 2); };
  const toggle = async () => { await displayBtn(); await sleep(600); if (!(await c.openMenuItems()).length) { await c.mouseMove(300, 600); await sleep(300); await displayBtn(); await sleep(800); } if (!(await c.openMenuItems()).length) { const o = await ev(`${S} const o = S.owner(); return o ? Object.keys(o).concat(Object.getOwnPropertyNames(Object.getPrototypeOf(o))) : null;`); throw new Error('menu will not open; owner keys ' + JSON.stringify(o)); } const m = await ev(`${S} return S.menus();`); await menuRow('Adapt to theme', 2500); return m; };
  const files = () => { const o = {}; for (const f of fs.readdirSync(path.join(VAULT, '.obsidian'))) { try { o[f] = fs.readFileSync(path.join(VAULT, '.obsidian', f), 'utf8').slice(0, 2000); } catch {} } return o; };
  for (const theme of ['dark', 'light']) {
    await ev(`app.changeTheme && app.changeTheme(${JSON.stringify(theme === 'dark' ? 'obsidian' : 'moonstone')}); app.vault.setConfig && app.vault.setConfig('theme', ${JSON.stringify(theme === 'dark' ? 'obsidian' : 'moonstone')}); return document.body.className;`);
    await sleep(800);
    await openPdf('text.pdf', 3000);
    const t = {};
    t.body = await ev(`return document.body.classList.contains('theme-dark') ? 'dark' : 'light';`);
    t.off = { state: await stateOf(), pixels: await sample() };
    t.shotOff = await shot('adapt-off-' + theme);
    const lsBefore = await ev(`return Object.fromEntries(Object.keys(localStorage).map((k) => [k, localStorage.getItem(k).slice(0, 400)]));`);
    const filesBefore = files();
    t.menuBefore = await toggle();
    t.on = { state: await stateOf(), pixels: await sample() };
    t.shotOn = await shot('adapt-on-' + theme);
    const lsAfter = await ev(`return Object.fromEntries(Object.keys(localStorage).map((k) => [k, localStorage.getItem(k).slice(0, 400)]));`);
    const filesAfter = files();
    t.localStorageChanged = Object.keys(lsAfter).filter((k) => lsAfter[k] !== lsBefore[k]).map((k) => ({ k, before: lsBefore[k] || null, after: lsAfter[k] }));
    t.filesChanged = Object.keys(filesAfter).filter((k) => filesAfter[k] !== filesBefore[k]).map((k) => ({ k, after: filesAfter[k] }));
    // Reopen: does it stay on? Another file?
    await ev(`${S} const leaf = S.pdfLeaf(); await leaf.openFile(await S.file('doc.pdf')); await S.sleep(2000); return true;`);
    t.otherFile = await stateOf();
    await ev(`${S} const leaf = S.pdfLeaf(); await leaf.openFile(await S.file('text.pdf')); await S.sleep(2500); return true;`);
    t.reopened = await stateOf();
    t.newLeaf = await (async () => { await ev(`${S} const leaf = app.workspace.getLeaf('tab'); await leaf.openFile(await S.file('text.pdf')); app.workspace.setActiveLeaf(leaf, { focus: true }); await S.sleep(2500); return true;`); const s = await ev(`${S} const leaves = app.workspace.getLeavesOfType('pdf'); const leaf = leaves[leaves.length - 1]; const o = S.owner(leaf); return { pageInvert: o && o.pageInvert, cls: S.cls(S.content(leaf).querySelector('.pdf-container')) };`); await ev(`${S} const leaves = app.workspace.getLeavesOfType('pdf'); leaves[leaves.length - 1].detach(); return true;`); return s; })();
    t.menuAfter = await (async () => { await displayBtn(); await sleep(600); const m = await ev(`${S} return S.menus();`); await ev(`${S} S.dismiss(); return true;`); return m; })();
    // Turn it off again.
    await toggle();
    t.offAgain = await stateOf();
    r[theme] = t;
  }
  await ev(`app.changeTheme && app.changeTheme('obsidian'); app.vault.setConfig && app.vault.setConfig('theme', 'obsidian'); return true;`);
  return r;
});

await step('s08-password', async () => {
  const r = {};
  await ev(`${S} for (const l of app.workspace.getLeavesOfType('pdf')) l.detach(); return true;`);
  for (const f of ['locked.pdf', 'locked-rc4.pdf']) {
    const t = {};
    await ev(`${S} S.clearHistory(); const f = await S.file(${JSON.stringify(f)}); const leaf = app.workspace.getLeaf('tab'); leaf.openFile(f); app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`);
    await sleep(3000);
    const look = () => ev(`${S} const m = document.querySelector('.modal-container'); const all = app.workspace.getLeavesOfType('pdf'); const leaf = all.find((l) => l.view.file && l.view.file.path === ${JSON.stringify(f)}) || null; const dlg = leaf && leaf.view.containerEl.querySelector('.pdf-password-dialog'); const dialog = dlg ? { html: dlg.outerHTML.slice(0, 5000), rect: S.rect(dlg), hidden: getComputedStyle(dlg).display === 'none', parts: Array.from(dlg.querySelectorAll('*')).map((e) => ({ tag: e.tagName.toLowerCase(), cls: S.cls(e), text: e.children.length ? '' : e.textContent.trim().slice(0, 100), rect: S.rect(e), color: getComputedStyle(e).color, bg: getComputedStyle(e).backgroundColor, fs: getComputedStyle(e).fontSize, fw: getComputedStyle(e).fontWeight, border: getComputedStyle(e).border, radius: getComputedStyle(e).borderRadius, padding: getComputedStyle(e).padding, placeholder: e.placeholder, value: e.value, type: e.type })), style: { bg: getComputedStyle(dlg).backgroundColor, display: getComputedStyle(dlg).display, padding: getComputedStyle(dlg).padding, border: getComputedStyle(dlg).border, radius: getComputedStyle(dlg).borderRadius, position: getComputedStyle(dlg).position } } : null; const res0 = { dialog, active: document.activeElement && (document.activeElement.tagName + '.' + S.cls(document.activeElement) + ' type=' + document.activeElement.type) }; const content = leaf && S.content(leaf); return { ...res0, ui: S.ui(), modal: m ? { cls: S.cls(m), text: m.textContent.trim().slice(0, 400), html: m.outerHTML.slice(0, 6000), inputs: Array.from(m.querySelectorAll('input')).map((i) => ({ type: i.type, placeholder: i.placeholder, value: i.value, cls: S.cls(i) })), buttons: Array.from(m.querySelectorAll('button')).map((b) => ({ text: b.textContent.trim(), cls: S.cls(b) })), title: m.querySelector('.modal-title') && m.querySelector('.modal-title').textContent } : null, leaf: leaf ? { state: leaf.view.getState(), pages: S.viewer(leaf) && S.viewer(leaf).pagesCount, toolbar: content && content.querySelector('.pdf-toolbar') && content.querySelector('.pdf-toolbar').textContent.trim(), input: content && content.querySelector('.pdf-page-input') && content.querySelector('.pdf-page-input').value, other: content && Array.from(content.querySelectorAll('*')).filter((e) => /password|error|empty|message/i.test(S.cls(e))).map((e) => ({ cls: S.cls(e), text: e.textContent.trim().slice(0, 200) })) } : null };`);
    t.opened = await look();
    t.shotOpened = await shot('password-' + f);
    if (t.opened.dialog || (t.opened.modal && t.opened.modal.inputs.length)) {
      await c.typeText('wrong', 40); await c.pressKey('Enter'); await sleep(1500);
      t.wrong = await look();
      t.shotWrong = await shot('password-wrong-' + f);
      await ev(`const i = document.querySelector('.modal-container input, .pdf-password-dialog input'); if (i) { i.focus(); i.select(); } return true;`);
      await c.typeText('secret', 40); await c.pressKey('Enter'); await sleep(2500);
      t.right = await look();
      t.shotRight = await shot('password-right-' + f);
    }
    // Escape / cancel path on a second open.
    await ev(`${S} for (const l of app.workspace.getLeavesOfType('pdf')) l.detach(); return true;`); await sleep(500);
    if (t.opened.modal || t.opened.dialog) {
      await ev(`${S} const f = await S.file(${JSON.stringify(f)}); const leaf = app.workspace.getLeaf('tab'); leaf.openFile(f); app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`); await sleep(2500);
      await c.pressKey('Escape'); await sleep(1200);
      t.escaped = await look();
      t.shotEscaped = await shot('password-escape-' + f);
      await ev(`${S} for (const l of app.workspace.getLeavesOfType('pdf')) l.detach(); return true;`); await sleep(500);
    }
    r[f] = t;
  }
  await openPdf('owner-only.pdf');
  r.ownerOnly = await ev(`${S} return { pages: S.viewer() && S.viewer().pagesCount, ui: S.ui() };`);
  return r;
});

await step('s09-cross-page', async () => {
  const r = {};
  await openPdf('text.pdf');
  await ev(`${S} S.page(2).scrollIntoView(); await S.sleep(1200); const v = S.viewer(); v.container.scrollTop = S.page(2).offsetTop - 500; await S.sleep(1200); return true;`);
  for (const title of ['Copy link to selection', 'Copy as quote', 'Copy']) {
    r[title] = { sel: await ev(`${S} const a = S.spans(1).length - 1; S.selectSpans(1, a, 0, 0, null, 2); await S.sleep(300); const v = S.viewer(); v.container.scrollTop = S.page(2).offsetTop - 500; await S.sleep(300); return S.selInfo();`) };
    const at = await ev(`${S} const s = S.spans(2)[0]; const b = s.getBoundingClientRect(); return { x: b.x + 10, y: b.y + b.height / 2 };`);
    await sentinel();
    await c.clickAt(at.x, at.y, { button: 'right' }); await sleep(700);
    r[title].items = await c.openMenuItems();
    try { await menuRow(title, 600); r[title].clipboard = await clip(); } catch (e) { r[title].error = e.message; await ev(`${S} S.dismiss(); return true;`); }
  }
  return r;
});

out.finishedAt = new Date().toISOString();
save();
c.close();
process.exit(0);
