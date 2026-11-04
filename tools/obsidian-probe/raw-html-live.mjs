// Raw HTML in Live Preview, and a raw <table> in both views: what the editor
// draws for an inline tag with the cursor away from it, on its line, and
// inside it; what a raw <table> block becomes in Live Preview; and how the
// reading view lays a raw table out against a Markdown one (computed styles
// per cell). Screenshots of each state go to SHOTS.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT, RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || path.join(path.dirname(RESULTS), 'raw-html-live-shots');
const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.rawHtmlLive = all.rawHtmlLive || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.rawHtmlLive[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
async function step(name, fn) {
  out.order.push(name);
  const t = Date.now();
  try { const v = await fn(); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e) }; }
  save(); return out.steps[name];
}

const NOTE = [
  '# Live HTML probe',
  '',
  'Inline: a <b>bold</b> word, <i>it</i>, <u>under</u>, x<sup>sup</sup>, H<sub>2</sub>O, <kbd>Ctrl</kbd>, <mark>mk</mark>, <span style="color: red">red</span>, <a href="https://example.invalid/">link</a>, and a<br>break.',
  '',
  'Unknown <foo>foo tag</foo> and <script>x</script> end, <!-- a comment --> after.',
  '',
  '<table>',
  '<tr><th>H1</th><th>H2</th></tr>',
  '<tr><td>a</td><td><b>b</b> **md**</td></tr>',
  '</table>',
  '',
  '| md | table |',
  '| --- | --- |',
  '| c | d |',
  '',
  '<table border="1"><thead><tr><th align="left">L</th><th align="right">R</th></tr></thead><tbody><tr><td colspan="2" style="color: red">wide cell</td></tr><tr><td>one</td><td>two</td></tr></tbody></table>',
  '',
  '<table><tr><td>c</td></tr></table>',
  '',
  'Unclosed <b>bold never closed',
  '',
  'Tail paragraph.',
  '',
].join('\n');

// Block HTML in Live Preview, one construct per block, and the inline edge
// cases: nesting, Markdown between tags, the cursor at an element's edges.
const BLOCKS = [
  '# Blocks',
  '',
  '<div>a div block</div>',
  '',
  '<pre>a pre block</pre>',
  '',
  '<details><summary>sum</summary>body</details>',
  '',
  '<center>centered</center>',
  '',
  '<p>a p block</p>',
  '',
  '<!-- a comment block -->',
  '',
  '<img src="nowhere.png" alt="img block">',
  '',
  'Nested <b><i>both</i></b> and <b>**md** inside</b> and <span>[[Live]]</span> end.',
  '',
  'Edges <u>uu</u> end.',
  '',
  'Tail.',
  '',
].join('\n');

if (phase === 'fixtures') {
  fs.mkdirSync(VAULT, { recursive: true });
  fs.writeFileSync(path.join(VAULT, 'Live.md'), NOTE);
  fs.writeFileSync(path.join(VAULT, 'Blocks.md'), BLOCKS);
  out.wrote = ['Live.md', 'Blocks.md'];
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

out.version = await c.evaluate(`return { app: require('electron').ipcRenderer.sendSync('version'), apiVersion: (window.require && require('obsidian') && require('obsidian').apiVersion) || null };`).catch((e) => String(e));

const open = (mode) => c.evaluate(`
  const leaf = app.workspace.getLeaf(false);
  await leaf.openFile(app.vault.getAbstractFileByPath('Live.md'));
  const st = leaf.getViewState();
  st.state = Object.assign({}, st.state, { mode: ${JSON.stringify(mode)} === 'reading' ? 'preview' : 'source', source: ${JSON.stringify(mode)} === 'source' });
  await leaf.setViewState(st);
  app.workspace.setActiveLeaf(leaf, { focus: true });
  await new Promise(r => setTimeout(r, 600));
  return leaf.getViewState().state;`);

// Every editor line's markup, plus the html widgets and their computed look.
const readEditor = () => c.evaluate(`
  const view = app.workspace.activeLeaf.view;
  const root = view.contentEl.querySelector('.cm-content');
  const lines = Array.from(root.children).map((el) => ({
    tag: el.tagName, cls: el.className, text: el.textContent.slice(0, 200), html: el.outerHTML.slice(0, 3000),
  }));
  const embeds = Array.from(view.contentEl.querySelectorAll('.cm-html-embed, .cm-embed-block')).map((el) => {
    const r = el.getBoundingClientRect();
    return { cls: el.className, tag: el.tagName, html: el.outerHTML.slice(0, 3000), rect: [r.x, r.y, r.width, r.height] };
  });
  const tok = Array.from(root.querySelectorAll('[class*="cm-html"], [class*="cm-tag"], [class*="cm-hmd-html"]')).slice(0, 40).map((el) => {
    const cs = getComputedStyle(el);
    return { cls: el.className, text: el.textContent, color: cs.color, display: cs.display, fontSize: cs.fontSize, fontFamily: cs.fontFamily.slice(0, 60) };
  });
  const sel = view.editor.listSelections();
  return { lines, embeds, tokens: tok, selections: sel, hasFocus: view.editor.hasFocus() };`);

const cursor = (line, ch) => c.evaluate(`
  const ed = app.workspace.activeLeaf.view.editor;
  ed.focus(); ed.setCursor({ line: ${line}, ch: ${ch} });
  await new Promise(r => setTimeout(r, 400));
  return ed.getCursor();`);

const styleOf = `(el) => { if (!el) return null; const cs = getComputedStyle(el); const r = el.getBoundingClientRect();
  return { tag: el.tagName, cls: el.className, text: el.textContent.slice(0, 40), rect: [r.x, r.y, r.width, r.height],
    fontWeight: cs.fontWeight, fontSize: cs.fontSize, color: cs.color, background: cs.backgroundColor,
    border: [cs.borderTopWidth, cs.borderTopStyle, cs.borderTopColor, cs.borderLeftWidth, cs.borderLeftStyle].join(' '),
    padding: [cs.paddingTop, cs.paddingRight, cs.paddingBottom, cs.paddingLeft].join(' '), textAlign: cs.textAlign,
    verticalAlign: cs.verticalAlign, lineHeight: cs.lineHeight, borderCollapse: cs.borderCollapse, width: cs.width, display: cs.display,
    marginTop: cs.marginTop, marginBottom: cs.marginBottom }; }`;

await step('01-live-preview-cursor-at-end', async () => {
  const st = await open('live');
  const cur = await cursor(21, 0);
  await sleep(300);
  return { state: st, cursor: cur, editor: await readEditor(), shot: await shot('01-lp-cursor-at-end') };
});

await step('02-cursor-inside-bold', async () => {
  // Line 2 (zero-based), inside the word "bold" of <b>bold</b>.
  const line = NOTE.split('\n')[2];
  const ch = line.indexOf('bold</b>') + 2;
  const cur = await cursor(2, ch);
  return { cursor: cur, line2: (await readEditor()).lines[2], shot: await shot('02-lp-cursor-in-bold') };
});

await step('03-cursor-on-line-outside-tags', async () => {
  const cur = await cursor(2, 3);
  return { cursor: cur, line2: (await readEditor()).lines[2], shot: await shot('03-lp-cursor-line-start') };
});

await step('04-cursor-on-the-opening-tag', async () => {
  const line = NOTE.split('\n')[2];
  const ch = line.indexOf('<b>') + 1;
  const cur = await cursor(2, ch);
  return { cursor: cur, line2: (await readEditor()).lines[2], shot: await shot('04-lp-cursor-in-open-tag') };
});

await step('05-cursor-inside-sup', async () => {
  const line = NOTE.split('\n')[2];
  const ch = line.indexOf('sup</sup>') + 1;
  const cur = await cursor(2, ch);
  return { cursor: cur, line2: (await readEditor()).lines[2] };
});

await step('06-cursor-in-raw-table', async () => {
  const cur = await cursor(8, 3);
  return { cursor: cur, editor: await readEditor(), shot: await shot('06-lp-cursor-in-table') };
});

await step('07-cursor-on-unknown-line', async () => {
  const cur = await cursor(4, 0);
  return { cursor: cur, editor: await readEditor() };
});

await step('08-lp-widget-styles', async () => {
  await cursor(21, 0);
  return c.evaluate(`
    const st = ${styleOf};
    const view = app.workspace.activeLeaf.view;
    const tables = Array.from(view.contentEl.querySelectorAll('.cm-content table'));
    return { tables: tables.map((t) => ({ html: t.outerHTML.slice(0, 2000), table: st(t), parent: st(t.parentElement), grand: st(t.parentElement && t.parentElement.parentElement),
      cells: Array.from(t.querySelectorAll('th, td')).map(st) })),
      inline: ['b', 'i', 'u', 'sup', 'sub', 'kbd', 'mark', 'span', 'a'].map((tg) => {
        const el = view.contentEl.querySelector('.cm-content .cm-line ' + tg) || view.contentEl.querySelector('.cm-content ' + tg);
        return { tg, ...(st(el) || {}), html: el ? el.outerHTML.slice(0, 300) : null, parentHtml: el && el.parentElement ? el.parentElement.outerHTML.slice(0, 400) : null };
      }) };`);
});

await step('09-reading-view', async () => {
  await open('reading');
  await sleep(500);
  const r = await c.evaluate(`
    const st = ${styleOf};
    const view = app.workspace.activeLeaf.view;
    const el = view.previewMode.containerEl;
    const tables = Array.from(el.querySelectorAll('table'));
    return {
      sections: Array.from(el.querySelectorAll('.markdown-preview-section > div')).map((d) => ({ cls: d.className, html: d.outerHTML.slice(0, 2500) })),
      tables: tables.map((t) => ({ html: t.outerHTML.slice(0, 2500), table: st(t), parent: st(t.parentElement), cells: Array.from(t.querySelectorAll('th, td')).map(st), rows: Array.from(t.querySelectorAll('tr')).map(st) })),
      inline: ['b', 'sup', 'sub', 'kbd'].map((tg) => st(el.querySelector('p ' + tg))),
      body: st(el.querySelector('p')),
    };`);
  return { ...r, shot: await shot('09-reading') };
});

const openBlocks = () => c.evaluate(`
  const leaf = app.workspace.getLeaf(false);
  await leaf.openFile(app.vault.getAbstractFileByPath('Blocks.md'));
  const st = leaf.getViewState();
  st.state = Object.assign({}, st.state, { mode: 'source', source: false });
  await leaf.setViewState(st);
  app.workspace.setActiveLeaf(leaf, { focus: true });
  await new Promise(r => setTimeout(r, 600));
  return leaf.getViewState().state;`);

await step('10-blocks-live-preview', async () => {
  const st = await openBlocks();
  await cursor(22, 0);
  await sleep(300);
  const ed = await readEditor();
  return { state: st, lines: ed.lines.map((l) => ({ cls: l.cls, html: l.html.slice(0, 1200) })), embeds: ed.embeds, shot: await shot('10-lp-blocks') };
});

const lineAt = async (ln, ch) => { const cur = await cursor(ln, ch); const ed = await readEditor(); return { cursor: cur, line: ed.lines.find((l) => l.cls.includes('cm-active')) || null }; };
const BL = BLOCKS.split('\n');
await step('11-cursor-in-div-block', async () => ({ ...(await lineAt(2, 6)), shot: await shot('11-lp-cursor-in-div') }));
await step('12-edge-before-open', async () => lineAt(18, BL[18].indexOf('<u>')));
await step('13-edge-after-close', async () => lineAt(18, BL[18].indexOf('</u>') + 4));
await step('14-edge-inside-open', async () => lineAt(18, BL[18].indexOf('<u>') + 1));
await step('15-nested-cursor-in-inner', async () => lineAt(16, BL[16].indexOf("both") + 1));
await step('16-selection-across', async () => c.evaluate(`
  const ed = app.workspace.activeLeaf.view.editor;
  ed.focus(); ed.setSelection({ line: 18, ch: 0 }, { line: 18, ch: 3 });
  await new Promise(r => setTimeout(r, 300));
  const a = app.workspace.activeLeaf.view.contentEl.querySelector('.cm-active').outerHTML;
  ed.setSelection({ line: 18, ch: 2 }, { line: 18, ch: ${BL[18].indexOf('uu') + 1} });
  await new Promise(r => setTimeout(r, 300));
  const b = app.workspace.activeLeaf.view.contentEl.querySelector('.cm-active').outerHTML;
  return { selectionBefore: a, selectionIntoElement: b };`));

await step('17-blocks-reading', async () => {
  await c.evaluate(`const leaf = app.workspace.activeLeaf; const st = leaf.getViewState(); st.state = Object.assign({}, st.state, { mode: 'preview' }); await leaf.setViewState(st); await new Promise(r => setTimeout(r, 600)); return true;`);
  const r = await c.evaluate(`const el = app.workspace.activeLeaf.view.previewMode.containerEl; return Array.from(el.querySelectorAll('.markdown-preview-section > div')).map((d) => d.outerHTML.slice(0, 600));`);
  return { sections: r, shot: await shot('17-reading-blocks') };
});

out.endedAt = new Date().toISOString();
save();
c.close();
process.exit(0);
