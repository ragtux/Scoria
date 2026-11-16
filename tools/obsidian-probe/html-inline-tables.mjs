// "Some HTML in notes": how Obsidian draws <sup>, <sub>, <small>, <kbd> and
// <mark> (size, baseline shift, line box), an inline `style` attribute's
// common properties, and a raw table's colspan, rowspan and <caption>, in the
// reading view and Live Preview, in both themes. Baselines are read from
// zero-size inline-block markers put into the rendered DOM (an empty
// inline-block sits with its bottom edge on the baseline); the note itself is
// never edited. Screenshots go to SHOTS.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT, RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || path.join(path.dirname(RESULTS), 'html-inline-tables-shots');
const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.htmlInlineTables = all.htmlInlineTables || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.htmlInlineTables[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
async function step(name, fn) {
  out.order.push(name);
  const t = Date.now();
  try {
    const v = await Promise.race([fn(), sleep(30000).then(() => { throw new Error('step timeout'); })]);
    out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) };
  } catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e) }; }
  save(); return out.steps[name];
}

const INLINE = [
  '# Inline',
  '',
  'Plain line for the paragraph height.',
  '',
  'Base x<sup>sup</sup> end.',
  '',
  'Base H<sub>2</sub>O end.',
  '',
  'Base <small>small</small> end.',
  '',
  'Base <kbd>Ctrl</kbd> end.',
  '',
  'Base <mark>mk</mark> end.',
  '',
  'Base <big>big</big> <ins>ins</ins> <del>del</del> <s>s</s> <u>u</u> <samp>samp</samp> <var>var</var> <tt>tt</tt> <code>code</code> end.',
  '',
  'Nested x<sup><sup>2</sup></sup> and <sup><b>bold</b></sup> and <small><sup>s</sup></small> and <sub>a<sup>b</sup></sub> and <small><small>ss</small></small> end.',
  '',
  'Footnote ref[^1] end.',
  '',
  '## Heading x<sup>2</sup> and H<sub>2</sub>',
  '',
  '- list x<sup>2</sup> and H<sub>2</sub>O',
  '',
  'Style <span style="color: red">red</span> <span style="color:#00ff00">hex</span> <span style="background-color: yellow">bg</span> <span style="font-weight: bold">fw</span> <span style="font-style: italic">fs</span> <span style="text-decoration: underline">tdu</span> <span style="text-decoration: line-through">tds</span> <span style="color: rgb(0, 0, 255)">rgb</span> <span style="color: hsl(120, 100%, 25%)">hsl</span> <span style="color: var(--text-accent)">var</span> <span style="font-size: 20px">big</span> <b style="color: red">bred</b> <span style="color: red; font-weight: 700">both</span> <span style="color: rebeccapurple">rp</span> <span style="background: orange">bgs</span> <font color="red">font</font> end.',
  '',
  'Wrapped paragraph with a sup x<sup>2</sup> in its first line and enough words after it that the reading view has to wrap it onto a second and maybe a third line at the default readable line length, so the rows can be compared with and without the raised text in them.',
  '',
  'Wrapped paragraph with no raised text at all in its first line and enough words after it that the reading view has to wrap it onto a second and maybe a third line at the default readable line length, so the rows can be compared with and without the raised text in them.',
  '',
  '[^1]: the note.',
  '',
].join('\n');

const TABLES = [
  '# Tables',
  '',
  '<table>',
  '<caption>The caption</caption>',
  '<tr><th>A</th><th>B</th><th>C</th></tr>',
  '<tr><td colspan="2">ab</td><td>c</td></tr>',
  '<tr><td rowspan="2">d</td><td>e</td><td>f</td></tr>',
  '<tr><td>g</td><td>h</td></tr>',
  '</table>',
  '',
  '<table>',
  '<thead><tr><th colspan="3">wide head</th></tr></thead>',
  '<tbody><tr><td>1</td><td>2</td><td>3</td></tr></tbody>',
  '</table>',
  '',
  '<table><tr><td rowspan="3">tall cell with more words</td><td>r1</td></tr><tr><td>r2</td></tr><tr><td>r3</td></tr></table>',
  '',
  '<table><caption style="caption-side: bottom">bottom caption</caption><tr><td>x</td><td>y</td></tr></table>',
  '',
  '<table><tr><td colspan="5">over-wide</td></tr><tr><td>a</td><td>b</td></tr></table>',
  '',
  '<table><tr><td rowspan="0">zero</td><td>a</td></tr><tr><td>b</td></tr></table>',
  '',
  '<table><caption>A much longer caption text that might be wider than the table itself is</caption><tr><td>x</td></tr></table>',
  '',
  '<table><caption><b>bold</b> caption</caption><tr><th>h</th></tr><tr><td colspan="0">c0</td></tr></table>',
  '',
  '<table><tr><td rowspan="5">past the end</td><td>a</td></tr><tr><td>b</td></tr></table>',
  '',
  'Tail.',
  '',
].join('\n');

if (phase === 'fixtures') {
  fs.mkdirSync(VAULT, { recursive: true });
  fs.writeFileSync(path.join(VAULT, 'Inline.md'), INLINE);
  fs.writeFileSync(path.join(VAULT, 'Tables.md'), TABLES);
  out.wrote = ['Inline.md', 'Tables.md'];
  save();
  process.exit(0);
}

const c = await connect();
fs.mkdirSync(SHOTS, { recursive: true });
const shot = async (name) => {
  const r = await c.send('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  return name + '.png';
};

out.version = await c.evaluate(`return { title: document.title, ua: navigator.userAgent };`).catch((e) => String(e));

const open = (file, mode) => c.evaluate(`
  const leaf = app.workspace.getLeaf(false);
  await leaf.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(file)}));
  const st = leaf.getViewState();
  st.state = Object.assign({}, st.state, { mode: ${JSON.stringify(mode)} === 'reading' ? 'preview' : 'source', source: false });
  await leaf.setViewState(st);
  app.workspace.setActiveLeaf(leaf, { focus: true });
  await new Promise(r => setTimeout(r, 900));
  return leaf.getViewState().state;`);

const theme = (dark) => c.evaluate(`
  app.changeTheme(${dark ? "'obsidian'" : "'moonstone'"});
  await new Promise(r => setTimeout(r, 600));
  return { dark: document.body.classList.contains('theme-dark') };`);

// Shared helpers injected into each evaluate.
const HELPERS = `
  const st = (el) => { if (!el) return null; const cs = getComputedStyle(el); const r = el.getBoundingClientRect();
    return { tag: el.tagName, cls: el.className, text: el.textContent.slice(0, 60), rect: [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 1000) / 1000),
      fontSize: cs.fontSize, fontWeight: cs.fontWeight, fontStyle: cs.fontStyle, fontFamily: cs.fontFamily.slice(0, 80), color: cs.color, background: cs.backgroundColor,
      textDecoration: cs.textDecorationLine + ' ' + cs.textDecorationColor, verticalAlign: cs.verticalAlign, lineHeight: cs.lineHeight, display: cs.display,
      padding: [cs.paddingTop, cs.paddingRight, cs.paddingBottom, cs.paddingLeft].join(' '),
      margin: [cs.marginTop, cs.marginRight, cs.marginBottom, cs.marginLeft].join(' '),
      border: [cs.borderTopWidth, cs.borderTopStyle, cs.borderTopColor].join(' '), borderRadius: cs.borderRadius,
      textAlign: cs.textAlign, captionSide: cs.captionSide, boxShadow: cs.boxShadow, width: cs.width, height: cs.height }; };
  const marker = () => { const m = document.createElement('span'); m.className = 'probe-bl';
    m.style.cssText = 'display:inline-block;width:0;height:0;padding:0;margin:0;border:0;vertical-align:baseline'; return m; };
  const baseline = (el, where) => { const m = marker(); if (where === 'before') el.parentNode.insertBefore(m, el); else el.appendChild(m);
    const b = m.getBoundingClientRect().bottom; m.remove(); return Math.round(b * 1000) / 1000; };
  const glyphBox = (el) => { const rg = document.createRange(); rg.selectNodeContents(el); const r = rg.getBoundingClientRect();
    return [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 1000) / 1000); };
  // One inline element against its parent: size, shift of its baseline
  // above the parent's (positive = raised), and the line box it sits in.
  const inlineCase = (el) => {
    if (!el) return null;
    const parentBaseline = baseline(el, 'before');
    const ownBaseline = baseline(el, 'inside');
    const block = el.closest('p, li, h1, h2, h3, .cm-line, td, th, caption') || el.parentElement;
    return { el: st(el), html: el.outerHTML.slice(0, 300), parent: st(el.parentElement), glyph: glyphBox(el),
      parentBaseline, ownBaseline, raised: Math.round((parentBaseline - ownBaseline) * 1000) / 1000,
      block: st(block), blockHeight: block.getBoundingClientRect().height };
  };`;

const readInline = (root) => c.evaluate(`
  ${HELPERS}
  const view = app.workspace.activeLeaf.view;
  const root = ${root};
  const q = (sel) => root.querySelector(sel);
  const paras = Array.from(root.querySelectorAll('p, .cm-line')).map((p) => ({ text: p.textContent.slice(0, 50), h: p.getBoundingClientRect().height,
    rects: Array.from(p.getClientRects()).length, lines: (() => { const rg = document.createRange(); rg.selectNodeContents(p); return Array.from(rg.getClientRects()).map((r) => [Math.round(r.y * 100) / 100, Math.round(r.height * 100) / 100]).slice(0, 40); })() }));
  const pick = (tag, nth = 0) => Array.from(root.querySelectorAll(tag))[nth] || null;
  const cases = {};
  for (const tg of ['sup', 'sub', 'small', 'kbd', 'mark', 'big', 'ins', 'del', 's', 'u', 'samp', 'var', 'tt', 'code', 'font']) cases[tg] = inlineCase(pick(tg));
  // Nesting: every sup/sub/small in the "Nested" paragraph.
  const nestedP = Array.from(root.querySelectorAll('p, .cm-line')).find((p) => p.textContent.startsWith('Nested'));
  const nested = nestedP ? Array.from(nestedP.querySelectorAll('sup, sub, small')).map(inlineCase) : null;
  const heading = Array.from(root.querySelectorAll('h2, .HyperMD-header-2')).map((h) => Array.from(h.querySelectorAll('sup, sub')).map(inlineCase));
  const list = Array.from(root.querySelectorAll('li, .HyperMD-list-line')).map((h) => Array.from(h.querySelectorAll('sup, sub')).map(inlineCase));
  const footnote = inlineCase(root.querySelector('sup.footnote-ref, .cm-footref'));
  const styleP = Array.from(root.querySelectorAll('p, .cm-line')).find((p) => p.textContent.startsWith('Style'));
  const styles = styleP ? Array.from(styleP.querySelectorAll('span, b, font')).map((el) => ({ html: el.outerHTML.slice(0, 200), st: st(el) })) : null;
  const body = st(Array.from(root.querySelectorAll('p, .cm-line')).find((p) => p.textContent.startsWith('Plain')));
  return { paras, cases, nested, heading, list, footnote, styles, body };`);

const readTables = (root) => c.evaluate(`
  ${HELPERS}
  const root = ${root};
  return Array.from(root.querySelectorAll('table')).map((t) => ({
    html: t.outerHTML.slice(0, 3000), table: st(t), wrapper: st(t.parentElement), caption: st(t.querySelector('caption')),
    captionGlyph: t.querySelector('caption') ? glyphBox(t.querySelector('caption')) : null,
    rows: Array.from(t.rows).map((r) => ({ rect: st(r).rect, cells: Array.from(r.cells).map((cell) => ({ text: cell.textContent, colSpan: cell.colSpan, rowSpan: cell.rowSpan,
      attrColspan: cell.getAttribute('colspan'), attrRowspan: cell.getAttribute('rowspan'), cellIndex: cell.cellIndex, st: st(cell) })) })),
  }));`);

const READING = `app.workspace.activeLeaf.view.previewMode.containerEl`;
const SOURCE = `app.workspace.activeLeaf.view.contentEl.querySelector('.cm-content')`;

const parkCursor = (line) => c.evaluate(`
  const ed = app.workspace.activeLeaf.view.editor;
  ed.focus(); ed.setCursor({ line: ${line}, ch: 0 });
  await new Promise(r => setTimeout(r, 500));
  return ed.getCursor();`);

for (const dark of [true, false]) {
  const tag = dark ? 'dark' : 'light';
  await step(`${tag}-01-theme`, () => theme(dark));
  await step(`${tag}-02-inline-reading`, async () => {
    await open('Inline.md', 'reading');
    await sleep(600);
    return { ...(await readInline(READING)), shot: await shot(`${tag}-inline-reading`) };
  });
  await step(`${tag}-03-inline-live`, async () => {
    await open('Inline.md', 'live');
    await parkCursor(0);
    await sleep(400);
    return { ...(await readInline(SOURCE)), shot: await shot(`${tag}-inline-live`) };
  });
  await step(`${tag}-04-tables-reading`, async () => {
    await open('Tables.md', 'reading');
    await sleep(600);
    const tables = await readTables(READING);
    const s1 = await shot(`${tag}-tables-reading-top`);
    await c.evaluate(`const el = app.workspace.activeLeaf.view.previewMode.containerEl; el.scrollTop = el.scrollHeight; await new Promise(r => setTimeout(r, 500)); return true;`);
    const more = await readTables(READING);
    const s2 = await shot(`${tag}-tables-reading-bottom`);
    return { tables, more, shots: [s1, s2] };
  });
}

out.endedAt = new Date().toISOString();
save();
c.close();
process.exit(0);
