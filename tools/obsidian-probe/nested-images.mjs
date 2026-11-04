// Image embeds nested in list items, blockquotes and callouts, in Live
// Preview and the reading view: is the image on the bullet's line, at the
// item's indent, and how tall is the line it sits on. Also the remote image
// in the same containers, served by this process (`HTTP_PORT`).
// Phases: fixtures | run. `SHOTS` names a folder for screenshots.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import zlib from 'node:zlib';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT, RESULTS = process.env.RESULTS, SHOTS = process.env.SHOTS;
const PORT = Number(process.env.HTTP_PORT || 8741);
const BASE = `http://127.0.0.1:${PORT}`;
const T0 = Date.now();
const out = { phase, startedAt: new Date().toISOString(), steps: {}, httpLog: [] };
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(out, null, 1)); };
async function step(name, fn) {
  const t = Date.now();
  try { const v = await fn(); out.steps[name] = { ok: true, ms: Date.now() - t, value: v }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e) }; }
  save(); return out.steps[name];
}

function png(w, h, rgb) {
  let raw = Buffer.alloc(0);
  const row = Buffer.alloc(w * 3);
  for (let x = 0; x < w; x++) { row[x * 3] = rgb[0]; row[x * 3 + 1] = rgb[1]; row[x * 3 + 2] = rgb[2]; }
  for (let y = 0; y < h; y++) raw = Buffer.concat([raw, Buffer.from([0]), row]);
  const table = [];
  for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; table[n] = c >>> 0; }
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type), data]);
    let crc = 0xffffffff;
    for (const b of td) crc = table[(crc ^ b) & 0xff] ^ (crc >>> 8);
    const c = Buffer.alloc(4); c.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
    return Buffer.concat([len, td, c]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
const PIC = png(120, 60, [0x30, 0x80, 0xc0]);
const REMOTE = png(120, 60, [0xc0, 0x50, 0x30]);

const NOTE = [
  '# Nested images',
  '',
  'Case A: a bullet that is only an embed.',
  '',
  '- ![[pic.png]]',
  '- after A',
  '',
  'Case B: an embed on the line after the bullet text (lazy continuation).',
  '',
  '- item B',
  '  ![[pic.png]]',
  '',
  'Case C: an embed as its own paragraph inside a loose item.',
  '',
  '- item C',
  '',
  '  ![[pic.png]]',
  '',
  'Case D: an ordered item that is only an embed, then a nested bullet.',
  '',
  '1. ![[pic.png|60]]',
  '   - ![[pic.png]]',
  '',
  'Case E: a blockquote that is only an embed.',
  '',
  '> ![[pic.png]]',
  '',
  'Case F: a callout whose body is only an embed.',
  '',
  '> [!note]',
  '> ![[pic.png]]',
  '',
  'Case G: a task item that is only an embed.',
  '',
  '- [ ] ![[pic.png]]',
  '',
  'Case H: remote images in a bullet and a quote.',
  '',
  `- ![remote](${BASE}/remote.png)`,
  `> ![sized|60x30](${BASE}/remote.png)`,
  '',
  'Case I: a remote 404 in a bullet.',
  '',
  `- ![missing alt](${BASE}/missing.png)`,
  '',
  'Case J: top-level controls.',
  '',
  '![[pic.png]]',
  '',
  `![remote](${BASE}/remote.png)`,
  '',
  `![missing alt](${BASE}/missing.png)`,
  '',
  'End.',
  '',
].join('\n');

if (phase === 'fixtures') {
  fs.writeFileSync(path.join(VAULT, 'pic.png'), PIC);
  fs.writeFileSync(path.join(VAULT, 'Nested.md'), NOTE);
  console.log('fixtures written');
  process.exit(0);
}

const server = http.createServer((req, res) => {
  out.httpLog.push({ t: Date.now() - T0, url: req.url });
  if (req.url === '/remote.png') { res.writeHead(200, { 'Content-Type': 'image/png' }); res.end(REMOTE); return; }
  res.writeHead(404, { 'Content-Type': 'text/plain' }); res.end('nope');
});
await new Promise((r) => server.listen(PORT, '127.0.0.1', r));

const c = await connect();
const shot = async (name) => {
  if (!SHOTS) return null;
  fs.mkdirSync(SHOTS, { recursive: true });
  const r = await c.send('Page.captureScreenshot', { format: 'png' });
  const f = path.join(SHOTS, name + '.png');
  fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
  return f;
};

await step('version', () => c.evaluate(`return { version: require('electron').ipcRenderer.sendSync('version'), apiVersion: (window.apiVersion||null), ua: navigator.userAgent };`));

const OPEN = (mode) => `
  const ls = app.workspace.getLeavesOfType('markdown');
  const leaf = ls.length ? ls[0] : app.workspace.getLeaf(false);
  await leaf.openFile(app.vault.getAbstractFileByPath('Nested.md'));
  app.workspace.setActiveLeaf(leaf, { focus: true });
  await new Promise(r => setTimeout(r, 400));
  const st = leaf.getViewState();
  st.state = Object.assign({}, st.state, { mode: ${JSON.stringify(mode)}, source: false });
  await leaf.setViewState(st);
  await new Promise(r => setTimeout(r, 1200));
  return leaf.getViewState().state;`;

// Every editor line: its text, classes, rect, and the images and widgets in it.
const LP_DUMP = `
  const view = app.workspace.activeLeaf.view;
  const R = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x: Math.round(b.x*100)/100, y: Math.round(b.y*100)/100, w: Math.round(b.width*100)/100, h: Math.round(b.height*100)/100 }; };
  const content = view.containerEl.querySelector('.cm-content');
  const lines = [];
  for (const el of content.children) {
    const imgs = Array.from(el.querySelectorAll('img')).map(i => ({ src: i.getAttribute('src'), alt: i.getAttribute('alt'), width: i.getAttribute('width'), height: i.getAttribute('height'), rect: R(i), complete: i.complete, naturalWidth: i.naturalWidth, parentClass: i.parentElement.className, display: getComputedStyle(i).display }));
    const embeds = Array.from(el.querySelectorAll('.internal-embed, .image-embed, .image-wrapper')).map(e => ({ cls: e.className, tag: e.tagName, rect: R(e), display: getComputedStyle(e).display }));
    const bullets = Array.from(el.querySelectorAll('.list-bullet, .cm-formatting-list, .task-list-item-checkbox, .cm-formatting-quote, .callout-title')).map(e => ({ cls: e.className, text: e.textContent, rect: R(e) }));
    lines.push({ tag: el.tagName, cls: el.className, text: el.textContent.slice(0, 80), rect: R(el), paddingLeft: getComputedStyle(el).paddingLeft, textIndent: getComputedStyle(el).textIndent, imgs, embeds, bullets, html: el.outerHTML.length < 1200 ? el.outerHTML : el.outerHTML.slice(0, 1200) + '…' });
  }
  return { contentRect: R(content), lines };`;

const RV_DUMP = `
  const view = app.workspace.activeLeaf.view;
  const R = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x: Math.round(b.x*100)/100, y: Math.round(b.y*100)/100, w: Math.round(b.width*100)/100, h: Math.round(b.height*100)/100 }; };
  const pv = view.containerEl.querySelector('.markdown-preview-view');
  const imgs = Array.from(pv.querySelectorAll('img')).map(i => {
    const li = i.closest('li'); const bq = i.closest('blockquote'); const co = i.closest('.callout');
    const p = i.closest('p');
    return { src: i.getAttribute('src'), alt: i.getAttribute('alt'), rect: R(i), complete: i.complete, naturalWidth: i.naturalWidth,
      li: li ? { rect: R(li), html: li.outerHTML.slice(0, 600) } : null,
      liBullet: li ? R(li.querySelector(':scope > .list-bullet, :scope > .task-list-item-checkbox')) : null,
      bq: bq ? R(bq) : null, callout: co ? R(co) : null, p: p ? R(p) : null };
  });
  return { imgs };`;

await step('lp-open', () => c.evaluate(OPEN('source')));
// Park the cursor on the last line so no case shows its source.
await step('lp-cursor-end', () => c.evaluate(`const ed = app.workspace.activeLeaf.view.editor; const n = ed.lineCount(); ed.focus(); ed.setCursor({ line: n - 1, ch: 0 }); await new Promise(r => setTimeout(r, 1500)); return ed.getCursor();`));
await step('lp-dump', () => c.evaluate(LP_DUMP));
await step('lp-shot-top', () => shot('lp-top'));
await step('lp-scroll-mid', () => c.evaluate(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.cm-scroller'); s.scrollTop = s.scrollHeight / 2 - 200; await new Promise(r => setTimeout(r, 1200)); return s.scrollTop;`));
await step('lp-shot-mid', () => shot('lp-mid'));
await step('lp-scroll-end', () => c.evaluate(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.cm-scroller'); s.scrollTop = s.scrollHeight; await new Promise(r => setTimeout(r, 1200)); return s.scrollTop;`));
await step('lp-shot-end', () => shot('lp-end'));
// The cursor on case A's line: does the line go back to source?
await step('lp-cursor-on-A', () => c.evaluate(`const ed = app.workspace.activeLeaf.view.editor; const s = app.workspace.activeLeaf.view.containerEl.querySelector('.cm-scroller'); s.scrollTop = 0; ed.setCursor({ line: 4, ch: 3 }); await new Promise(r => setTimeout(r, 1200)); return ed.getLine(4);`));
await step('lp-dump-cursor-A', () => c.evaluate(LP_DUMP));
await step('lp-shot-cursor-A', () => shot('lp-cursor-A'));
await step('rv-open', () => c.evaluate(OPEN('preview')));
await step('rv-scroll', () => c.evaluate(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-preview-view'); for (let y = 0; y < s.scrollHeight + 600; y += 300) { s.scrollTop = y; await new Promise(r => setTimeout(r, 150)); } s.scrollTop = 0; await new Promise(r => setTimeout(r, 600)); return s.scrollHeight;`));
await step('rv-dump', () => c.evaluate(RV_DUMP));
await step('rv-shot', () => shot('rv-top'));
c.close();
server.close();
save();
console.log('done');
process.exit(0);
