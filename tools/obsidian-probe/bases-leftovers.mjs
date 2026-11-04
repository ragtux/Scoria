// Bases leftovers: five behaviours the backlog lists as never observed, read
// off a real Obsidian after bases-ui.mjs (2026-11-01):
//
//   s01  a Cards card whose note has no cover (`image: note.cover`)
//   s02  the list layout's "Indent properties" toggle: its row, the key it
//        writes, and what it changes in the layout
//   s03  "Add summary" on a table column's summary menu
//   s04  shift-click cell selection in the table, Ctrl+C, Ctrl+V, Ctrl+Z
//   s05  where a card dropped mid-column lands in a Kanban view, and what the
//        card offers as drag data
//
// Phases: fixtures | run   (node bases-leftovers.mjs <phase>)
// ONLY_STEPS=s03,s04 re-runs a step or two. SHOTS names the folder the PNGs go to.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || path.join(path.dirname(RESULTS || '.'), '2026-11-04-bases-leftovers-shots');
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run'); process.exit(2); }
if (phase === 'after-restart') process.exit(0);

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.basesLeftovers = all.basesLeftovers || {};
const ONLY = process.env.ONLY_STEPS && phase === 'run' ? process.env.ONLY_STEPS.split(',') : null;
const prev = ONLY && all.basesLeftovers[phase] && all.basesLeftovers[phase].steps ? all.basesLeftovers[phase] : null;
const out = prev ? { ...prev, reruns: [...(prev.reruns || []), { at: new Date().toISOString(), only: ONLY }] } : { startedAt: new Date().toISOString(), shots: SHOTS, steps: {}, order: [] };
all.basesLeftovers[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  console.error(`[bases-leftovers] ${name}`);
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; console.error(`[bases-leftovers] ${name} failed: ${e && e.message}`); }
  save();
  return out.steps[name];
}

// ---- fixtures -------------------------------------------------------------
const vp = (rel) => path.join(VAULT, rel);

/** A tiny valid PNG (w x h, one flat colour). */
function png(w, h, [r, g, b]) {
  const crc = (buf) => { let c = ~0; for (const x of buf) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); } return (~c) >>> 0; };
  const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const cc = Buffer.alloc(4); cc.writeUInt32BE(crc(td)); return Buffer.concat([len, td, cc]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const rows = [];
  for (let y = 0; y < h; y++) { const row = Buffer.alloc(1 + w * 3); for (let x = 0; x < w; x++) { row[1 + x * 3] = r; row[2 + x * 3] = g; row[3 + x * 3] = b; } rows.push(row); }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(Buffer.concat(rows))), chunk('IEND', Buffer.alloc(0))]);
}

const note = (name, fm, body) => [name, `---\n${fm}\n---\n\n${body}\n`];
// Eight rock notes, sorted by name: Andesite, Basalt, Ignimbrite, Obsidian
// rock, Pumice, Rhyolite, Scoria, Tuff. Tuff has no `cover` key at all,
// Ignimbrite has an empty `cover:`.
const NOTES = Object.fromEntries([
  note('Scoria.md', 'title: Scoria rock\nrating: 4\ntags:\n  - rock\nstatus: todo\ndensity: 1.21\ncover: "[[red.png]]"', '# Scoria\n\nFrothed basalt.'),
  note('Pumice.md', 'title: Pumice\nrating: 5\ntags:\n  - rock\nstatus: done\ndensity: 0.64\ncover: "[[blue.png]]"', '# Pumice\n\nFloats.'),
  note('Basalt.md', 'title: Basalt\nrating: 3\ntags:\n  - rock\nstatus: doing\ndensity: 2.9\ncover: "[[red.png]]"', '# Basalt\n\nDense.'),
  note('Obsidian rock.md', 'title: Obsidian\nrating: 5\ntags:\n  - rock\nstatus: done\ndensity: 2.4\ncover: "[[blue.png]]"', '# Obsidian\n\nVolcanic glass.'),
  note('Tuff.md', 'title: Tuff\nrating: 2\ntags:\n  - rock\nstatus: todo\ndensity: 1.6', '# Tuff\n\nWelded ash.'),
  note('Andesite.md', 'title: Andesite\nrating: 3\ntags:\n  - rock\nstatus: doing\ndensity: 2.7\ncover: "[[red.png]]"', '# Andesite\n\nIntermediate.'),
  note('Rhyolite.md', 'title: Rhyolite\nrating: 4\ntags:\n  - rock\nstatus: done\ndensity: 2.5\ncover: "[[blue.png]]"', '# Rhyolite\n\nSilica rich.'),
  note('Ignimbrite.md', 'title: Ignimbrite\nrating: 2\ntags:\n  - rock\nstatus: todo\ndensity: 2.2\ncover:', '# Ignimbrite\n\nPyroclastic flow.'),
]);
const BASE = `filters:
  and:
    - 'file.hasTag("rock")'
views:
  - type: cards
    name: Cards
    order:
      - file.name
      - status
    sort:
      - property: file.name
        direction: ASC
    image: note.cover
    cardSize: 160
  - type: list
    name: List
    order:
      - file.name
      - status
      - rating
    sort:
      - property: file.name
        direction: ASC
  - type: table
    name: Table
    order:
      - file.name
      - status
      - rating
      - density
    sort:
      - property: file.name
        direction: ASC
    summaries:
      rating: Sum
  - type: kanban
    name: Board
    order:
      - file.name
      - rating
    groupBy:
      property: status
      direction: ASC
`;
const BASE_NAME = 'Leftovers.base';

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  for (const [name, text] of Object.entries(NOTES)) fs.writeFileSync(vp(name), text);
  fs.writeFileSync(vp('red.png'), png(64, 48, [200, 60, 40]));
  fs.writeFileSync(vp('blue.png'), png(48, 64, [40, 80, 200]));
  fs.writeFileSync(vp(BASE_NAME), BASE);
  fs.mkdirSync(SHOTS, { recursive: true });
  out.fixtures = fs.readdirSync(VAULT).sort();
  save();
  process.exit(0);
}

// ---- run ------------------------------------------------------------------
fs.mkdirSync(SHOTS, { recursive: true });
const c = await connect();
const vault = await c.vaultPath();
if (vault.replace(/[\\/]+$/, '').toLowerCase() !== VAULT.replace(/[\\/]+$/, '').toLowerCase()) { console.error('wrong vault open: ' + vault); c.close(); process.exit(3); }
out.vault = vault;
out.version = await c.evaluate(`return { title: document.title, ua: navigator.userAgent, theme: document.body.className, apiVersion: (() => { try { return window.require('obsidian').apiVersion; } catch (e) { return null; } })() };`);
out.label = (out.version.ua.match(/obsidian\/[\d.]+/i) || out.version.title.match(/Obsidian [\d.]+/) || ['Obsidian ?'])[0];
const ev = (js) => c.evaluate(js);
// A late Input.dragIntercepted timeout from cdp.mjs must not kill the run.
process.on('unhandledRejection', (e) => { (out.unhandled = out.unhandled || []).push(String(e && e.message || e)); console.error('[bases-leftovers] unhandled: ' + (e && e.message)); });

// ---- screenshots ----------------------------------------------------------
let shotN = prev ? 200 + ((prev.reruns || []).length - 1) * 100 : 0;
const shot = async (name, clipSel) => {
  const n = String(++shotN).padStart(3, '0');
  const rec = {};
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = path.join(SHOTS, `${n}-${name}.png`);
    fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
    rec.full = path.basename(f);
  } catch (e) { rec.error = e.message; }
  if (clipSel) {
    try {
      const b = await ev(`
        const els = Array.from(document.querySelectorAll(${JSON.stringify(clipSel)})).filter(e => e.offsetWidth > 0);
        if (!els.length) return null;
        let x0 = 1e9, y0 = 1e9, x1 = 0, y1 = 0;
        for (const e of els) { const r = e.getBoundingClientRect(); x0 = Math.min(x0, r.x); y0 = Math.min(y0, r.y); x1 = Math.max(x1, r.right); y1 = Math.max(y1, r.bottom); }
        return { x: Math.max(0, x0 - 10), y: Math.max(0, y0 - 10), w: Math.min(window.innerWidth, x1 + 10) - Math.max(0, x0 - 10), h: Math.min(window.innerHeight, y1 + 10) - Math.max(0, y0 - 10) };`);
      if (b && b.w > 0 && b.h > 0) {
        const r = await c.send('Page.captureScreenshot', { format: 'png', clip: { x: b.x, y: b.y, width: b.w, height: b.h, scale: 1 } });
        const f = path.join(SHOTS, `${n}-${name}-clip.png`);
        fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
        rec.clip = path.basename(f); rec.clipRect = b;
      }
    } catch (e) { rec.clipError = e.message; }
  }
  return rec;
};

// ---- DOM readers (from bases-ui.mjs) --------------------------------------
const ROW_SEL = '.menu-item, .menu-separator, .bases-toolbar-menu-item, .suggestion-item, .suggestion-empty, .input-row, .filter-row, .filter-group, .filter-group-header, .bases-toolbar-section-header, .bases-toolbar-menu-container-header, .back-button, .combobox-button, button, input, select:not(.is-measuring), textarea, [contenteditable="true"], .multi-select-pill, .clickable-icon, .setting-item, .modal-title, .modal-content > *, .prompt-input, .suggestion-group, .bases-menu-group, .formula-editor, .filter-row-error, .bases-toolbar-section, .search-input-container, .checkbox-container';
const READ_LIB = `
  const txt = (e) => (e && e.textContent || '').replace(/\\s+/g, ' ').trim();
  const lucide = (e) => { const s = e && e.querySelector && e.querySelector('svg'); const cl = s ? String(s.getAttribute('class') || '') : ''; const m = cl.match(/lucide-[\\w-]+/); return m ? m[0] : (s ? cl : null); };
  const lucides = (e) => Array.from(e.querySelectorAll('svg')).map(s => { const m = String(s.getAttribute('class') || '').match(/lucide-[\\w-]+/); return m ? m[0] : null; }).filter(Boolean);
  const collapse = (html) => html.replace(/<svg[^>]*class="([^"]*)"[^>]*>[\\s\\S]*?<\\/svg>/g, (m, cl) => '<svg class="' + cl + '"/>').replace(/<svg[\\s\\S]*?<\\/svg>/g, '<svg/>');
  const rectOf = (e) => { const r = e.getBoundingClientRect(); return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) }; };
  const depthFrom = (root, e) => { let d = 0; for (let p = e; p && p !== root; p = p.parentElement) d++; return d; };
  const readRow = (root, e) => {
    const r = { tag: e.tagName.toLowerCase(), cls: String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className || ''), depth: depthFrom(root, e), text: txt(e).slice(0, 200) };
    if (e.classList.contains('menu-separator')) r.separator = true;
    const t = e.querySelector('.menu-item-title, .bases-toolbar-menu-item-name, .combobox-button-label, .input-row-label, .suggestion-content, .setting-item-name, .back-label, .suggestion-title');
    if (t) r.title = txt(t).slice(0, 200);
    const d = e.querySelector('.setting-item-description, .suggestion-note, .suggestion-aux, .input-row-description');
    if (d) r.desc = txt(d).slice(0, 200);
    const ic = lucide(e); if (ic) r.icon = ic;
    const ics = lucides(e); if (ics.length > 1) r.icons = ics.slice(0, 12);
    const aria = e.getAttribute('aria-label'); if (aria) r.aria = aria;
    const ph = e.getAttribute('placeholder') || e.getAttribute('data-placeholder') || (e.querySelector('[placeholder]') ? e.querySelector('[placeholder]').getAttribute('placeholder') : null); if (ph) r.placeholder = ph;
    if (e.tagName === 'INPUT' || e.tagName === 'TEXTAREA') { r.type = e.getAttribute('type') || 'text'; r.value = e.value; if (e.type === 'checkbox') r.checked = e.checked; }
    if (e.tagName === 'SELECT') { r.value = e.value; r.options = Array.from(e.options).map(o => ({ value: o.value, label: txt(o), selected: o.selected || undefined })); }
    if (e.isContentEditable) r.value = txt(e).slice(0, 200);
    const cb = e.querySelector('input[type="checkbox"]'); if (cb) r.checkbox = cb.checked;
    const states = ['mod-checked', 'is-active', 'is-selected', 'is-enabled', 'mod-active', 'is-collapsed', 'is-disabled', 'mod-warning', 'mod-cta'].filter(k => e.classList.contains(k));
    if (states.length) r.states = states;
    if (e.hasAttribute('disabled') || e.classList.contains('is-disabled')) r.disabled = true;
    r.rect = rectOf(e); if (e.offsetHeight === 0 || getComputedStyle(e).display === 'none') r.hidden = true;
    return r;
  };
  const readSurface = (root) => ({
    cls: String(root.className || ''), rect: rectOf(root), text: txt(root).slice(0, 400),
    rows: Array.from(root.querySelectorAll(${JSON.stringify(ROW_SEL)})).map(e => readRow(root, e)),
    html: collapse(root.outerHTML).slice(0, 40000),
  });
  const SURF = '.menu, .popover, .suggestion-container, .modal-container, .notice, .prompt, .hover-popover';
`;
const surfaces = () => ev(`${READ_LIB} return Array.from(document.querySelectorAll(SURF)).map(readSurface);`);
const surfaceClasses = () => ev(`${READ_LIB} return Array.from(document.querySelectorAll(SURF)).map(e => String(e.className));`);
const dismiss = async () => { try { await ev(`document.querySelectorAll('.menu, .suggestion-container').forEach(m => m.remove()); return true;`); } catch { /* ignore */ } await sleep(150); };
const escapeAll = async (n = 2) => { for (let i = 0; i < n; i++) { await c.pressKey('Escape'); await sleep(200); } };
const closePopovers = async () => { await escapeAll(1); await ev(`document.querySelectorAll('.popover.bases-new-item-popover, .hover-popover').forEach(e => e.remove()); return true;`); await sleep(300); };
const activeElement = () => ev(`${READ_LIB}
  const a = document.activeElement; if (!a) return null;
  return { tag: a.tagName.toLowerCase(), cls: String(a.className || '').slice(0, 200), editable: a.isContentEditable, text: txt(a).slice(0, 120), value: a.value !== undefined ? String(a.value).slice(0, 120) : undefined, rect: rectOf(a) };`);

const openBase = async (p) => {
  await dismiss();
  await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`);
  await sleep(1000);
  await ev(`
    let f = null;
    for (let i = 0; i < 40 && !f; i++) { f = app.vault.getAbstractFileByPath(${JSON.stringify(p)}); if (!f) await new Promise((r) => setTimeout(r, 500)); }
    if (!f) throw new Error('missing ' + ${JSON.stringify(p)});
    const leaf = app.workspace.getLeaf('tab'); await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`);
  await sleep(2500);
  return true;
};
const openPanel = async (cls) => {
  for (let attempt = 0; attempt < 3; attempt++) {
    await c.clickElement('.workspace-leaf.mod-active .' + cls);
    for (let i = 0; i < 8; i++) { await sleep(150); if (await ev(`return !!document.querySelector('.menu.bases-toolbar-menu');`)) return { opened: true, attempt }; }
    await dismiss(); await escapeAll(1);
  }
  return { opened: false };
};
const switchView = async (name) => {
  await dismiss();
  await openPanel('bases-toolbar-views-menu');
  const r = await ev(`
    const rows = Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-item'));
    const row = rows.find((e) => ((e.querySelector('.bases-toolbar-menu-item-name') || {}).textContent || '').trim() === ${JSON.stringify(name)});
    if (!row) return { found: false, rows: rows.map(e => (e.textContent || '').trim()) };
    const info = row.querySelector('.bases-toolbar-menu-item-info') || row; const b = info.getBoundingClientRect();
    return { found: true, x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (r.found) { await c.clickAt(r.x, r.y); await sleep(2000); }
  await dismiss();
  const now = await ev(`try { const v = app.workspace.activeLeaf.view; return v.controller ? v.controller.viewName : null; } catch (e) { return 'ERR:' + e.message; }`);
  return { ...r, viewNameNow: now };
};
const clickText = async (sel, text, { exact = true, index = 0 } = {}) => {
  const r = await ev(`
    const els = Array.from(document.querySelectorAll(${JSON.stringify(sel)})).filter(e => { const t = (e.textContent || '').replace(/\\s+/g, ' ').trim(); return ${exact} ? t === ${JSON.stringify(text)} : t.includes(${JSON.stringify(text)}); });
    const el = els[${index}]; if (!el) return null; const b = el.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(el.className) };`);
  if (!r) return { clicked: false, reason: 'no ' + sel + ' with text ' + text };
  await c.clickAt(r.x, r.y); await sleep(700);
  return { clicked: true, at: r };
};
const openViewConfig = async (viewName) => {
  for (let attempt = 0; attempt < 3; attempt++) {
    await dismiss(); await escapeAll(1);
    await openPanel('bases-toolbar-views-menu');
    const r = await ev(`
      const rows = Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-item'));
      const row = rows.find((e) => ((e.querySelector('.bases-toolbar-menu-item-name') || {}).textContent || '').trim() === ${JSON.stringify(viewName)});
      if (!row) return { found: false, rows: rows.map(e => ((e.querySelector('.bases-toolbar-menu-item-name') || {}).textContent || '').trim()) };
      const icon = row.querySelector('.bases-toolbar-menu-item-icon');
      const b = (icon || row).getBoundingClientRect();
      return { found: true, viaIcon: !!icon, x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (!r.found) return r;
    await c.clickAt(r.x, r.y);
    for (let i = 0; i < 20; i++) {
      if (await ev(`return !!document.querySelector('.view-config-menu');`)) return { ...r, ready: true, attempt };
      await sleep(250);
    }
  }
  return { found: true, ready: false };
};
const readBase = () => fs.readFileSync(vp(BASE_NAME), 'utf8');
const noteNames = Object.keys(NOTES);
const readNotes = () => Object.fromEntries(noteNames.map((n) => [n, fs.readFileSync(vp(n), 'utf8')]));
const diffNotes = (a, b) => Object.fromEntries(noteNames.filter((n) => a[n] !== b[n]).map((n) => [n, { before: a[n], after: b[n] }]));
const restoreFixtures = async () => {
  let changed = [];
  for (const [n, t] of Object.entries(NOTES)) if (fs.readFileSync(vp(n), 'utf8') !== t) { fs.writeFileSync(vp(n), t); changed.push(n); }
  if (readBase() !== BASE) { fs.writeFileSync(vp(BASE_NAME), BASE); changed.push(BASE_NAME); }
  if (changed.length) await sleep(2000);
  return changed;
};

// ---- s01: a card with no cover -------------------------------------------
const CARDS = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-cards-item'))`;
const readCards = () => ev(`${READ_LIB}
  return ${CARDS}.map(e => {
    const cover = e.querySelector('.bases-cards-cover');
    const ccs = cover ? getComputedStyle(cover) : null;
    const cs = getComputedStyle(e);
    return {
      title: txt(e.querySelector('.bases-cards-property.mod-title .bases-cards-line')), cls: String(e.className), style: e.getAttribute('style'), rect: rectOf(e),
      bg: cs.backgroundColor, boxShadow: cs.boxShadow, borderRadius: cs.borderRadius,
      cover: cover ? { cls: String(cover.className), style: cover.getAttribute('style'), rect: rectOf(cover), bgColor: ccs.backgroundColor, bgImage: ccs.backgroundImage.slice(0, 160), display: ccs.display, height: ccs.height, children: Array.from(cover.children).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className && x.className.baseVal !== undefined ? x.className.baseVal : x.className), rect: rectOf(x) })), html: collapse(cover.outerHTML).slice(0, 800) } : null,
      props: Array.from(e.querySelectorAll('.bases-cards-property')).map(pp => ({ prop: pp.getAttribute('data-property'), style: pp.getAttribute('style'), rect: rectOf(pp) })),
      html: collapse(e.outerHTML).slice(0, 3000),
    };
  });`);

await step('s01-card-no-cover', async (p) => {
  await openBase(BASE_NAME);
  p.switched = await switchView('Cards');
  await sleep(800);
  p.container = await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active .bases-cards-container') || document.querySelector('.workspace-leaf.mod-active .bases-view'); return e ? { cls: String(e.className), rect: rectOf(e), style: e.getAttribute('style'), scrollH: e.scrollHeight } : null;`);
  p.cards = await readCards();
  // make sure Tuff (no cover key) and Ignimbrite (empty cover) are rendered
  for (const name of ['Tuff', 'Ignimbrite']) {
    if (!p.cards.some((x) => x.title === name)) {
      await ev(`const s = document.querySelector('.workspace-leaf.mod-active .view-content .bases-view') || document.querySelector('.workspace-leaf.mod-active .view-content'); s.scrollTop = s.scrollHeight; s.querySelectorAll('*').forEach(x => { if (x.scrollHeight > x.clientHeight + 10) x.scrollTop = x.scrollHeight; }); return true;`);
      await sleep(1000);
      p.cards = await readCards();
      p.scrolledFor = name;
    }
  }
  p.shot = await shot('cards-no-cover', '.workspace-leaf.mod-active .bases-cards-item');
  const by = Object.fromEntries(p.cards.map((x) => [x.title, x]));
  const withCover = by['Tuff'] && p.cards.find((x) => x.cover && /url/.test(x.cover.bgImage) && x.rect.y === by['Tuff'].rect.y) || p.cards.find((x) => x.cover && /url/.test(x.cover.bgImage));
  p.compare = {
    tuff: by['Tuff'] ? { rect: by['Tuff'].rect, cover: by['Tuff'].cover, style: by['Tuff'].style } : 'not rendered',
    ignimbrite: by['Ignimbrite'] ? { rect: by['Ignimbrite'].rect, cover: by['Ignimbrite'].cover, style: by['Ignimbrite'].style } : 'not rendered',
    withCover: withCover ? { title: withCover.title, rect: withCover.rect, cover: withCover.cover, style: withCover.style } : null,
  };
  // a clip of Tuff next to a covered card
  if (by['Tuff']) {
    const r = by['Tuff'].rect;
    const o = withCover ? withCover.rect : r;
    const x0 = Math.min(r.x, o.x) - 10, y0 = Math.min(r.y, o.y) - 10, x1 = Math.max(r.x + r.w, o.x + o.w) + 10, y1 = Math.max(r.y + r.h, o.y + o.h) + 10;
    try { const s = await c.send('Page.captureScreenshot', { format: 'png', clip: { x: Math.max(0, x0), y: Math.max(0, y0), width: x1 - Math.max(0, x0), height: y1 - Math.max(0, y0), scale: 1 } }); const f = path.join(SHOTS, `${String(++shotN).padStart(3, '0')}-cards-tuff-vs-covered.png`); fs.writeFileSync(f, Buffer.from(s.data, 'base64')); p.pairShot = path.basename(f); } catch (e) { p.pairShotError = e.message; }
  }
  return p;
});

// ---- s02: the list layout's "Indent properties" ---------------------------
const LIST_LAYOUT = () => ev(`${READ_LIB}
  const vc = document.querySelector('.workspace-leaf.mod-active .view-content');
  const list = vc.querySelector('.bases-list-container, [class*="bases-list"]');
  const items = Array.from(vc.querySelectorAll('.bases-list-item')).slice(0, 2);
  const st = (e) => { const cs = getComputedStyle(e); return { cls: String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className), tag: e.tagName.toLowerCase(), rect: rectOf(e), text: txt(e).slice(0, 60), display: cs.display, flexDirection: cs.flexDirection, flexWrap: cs.flexWrap, gap: cs.gap, padding: cs.padding, margin: cs.margin, paddingInlineStart: cs.paddingInlineStart, marginInlineStart: cs.marginInlineStart, textIndent: cs.textIndent, listStyle: cs.listStyleType, before: getComputedStyle(e, '::before').content, beforeWidth: getComputedStyle(e, '::before').width, after: getComputedStyle(e, '::after').content }; };
  return {
    listCls: list ? String(list.className) : null, listStyle: list ? list.getAttribute('style') : null,
    ancestors: (() => { const a = []; for (let x = items[0]; x && x !== vc; x = x.parentElement) a.push(String(x.className)); return a; })(),
    items: items.map(it => ({ outer: collapse(it.outerHTML).replace(/>\\s+</g, '><').slice(0, 3000), nodes: [it, ...Array.from(it.querySelectorAll('*'))].filter(x => x.tagName !== 'path').slice(0, 30).map(st) })),
  };`);

await step('s02-list-indent', async (p) => {
  await restoreFixtures();
  await openBase(BASE_NAME);
  p.switched = await switchView('List');
  await sleep(600);
  p.layoutBefore = await LIST_LAYOUT();
  p.shotBefore = await shot('list-indent-default', '.workspace-leaf.mod-active .view-content');
  p.bytes0 = readBase();
  p.config = await openViewConfig('List');
  await sleep(500);
  const findRow = `const rows = Array.from(document.querySelectorAll('.menu .input-row')); const row = rows.find(r => /indent/i.test(r.textContent || ''));`;
  p.row = await ev(`${READ_LIB} ${findRow} if (!row) return { found: false, rows: rows.map(r => txt(r)) };
    const cb = row.querySelector('input[type="checkbox"]'); const cont = row.querySelector('.checkbox-container');
    return { found: true, row: readRow(row.parentElement, row), label: txt(row.querySelector('.input-row-label')), desc: txt(row.querySelector('.input-row-description, .setting-item-description')) || null, ariaLabel: row.getAttribute('aria-label'), tooltip: cont && cont.getAttribute('aria-label'), checked: cb ? cb.checked : null, containerCls: cont ? String(cont.className) : null, html: collapse(row.outerHTML), neighbours: rows.map(r => txt(r.querySelector('.input-row-label'))) };`);
  p.configSurfaces = await surfaces();
  p.configShot = await shot('list-config-page', '.menu');
  const toggle = async () => {
    const r = await ev(`${findRow} if (!row) return null; const e = row.querySelector('.checkbox-container') || row.querySelector('input'); const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (!r) return { clicked: false };
    const b0 = readBase();
    await c.clickAt(r.x, r.y);
    // when does the write land: poll the file for 6 s
    const t0 = Date.now(); const writes = [];
    let last = b0;
    while (Date.now() - t0 < 6000) { await sleep(200); const now = readBase(); if (now !== last) { writes.push({ atMs: Date.now() - t0, bytes: now }); last = now; } }
    return { clicked: true, at: r, writes, nowChecked: await ev(`${findRow} return row ? { checked: row.querySelector('input[type="checkbox"]').checked, cls: String(row.querySelector('.checkbox-container').className) } : null;`) };
  };
  p.toggleOn = await toggle();
  p.toggleOnShotConfig = await shot('list-config-indent-on', '.menu');
  p.bytes1 = readBase();
  await dismiss(); await escapeAll(1); await sleep(600);
  p.layoutOn = await LIST_LAYOUT();
  p.shotOn = await shot('list-indent-on', '.workspace-leaf.mod-active .view-content');
  p.config2 = await openViewConfig('List');
  await sleep(400);
  p.toggleOff = await toggle();
  p.toggleOffShotConfig = await shot('list-config-indent-off', '.menu');
  p.bytes2 = readBase();
  await dismiss(); await escapeAll(1); await sleep(3000);
  p.bytes2AfterClose = readBase();
  p.layoutOff = await LIST_LAYOUT();
  // the config page once more: what state does the toggle show now?
  p.config3 = await openViewConfig('List'); await sleep(400);
  p.rowAgain = await ev(`${findRow} return row ? { checked: row.querySelector('input[type="checkbox"]').checked, cls: String(row.querySelector('.checkbox-container').className) } : null;`);
  await dismiss(); await escapeAll(1);
  // in memory: the view's config for the key
  p.inMemory = await ev(`try { const v = app.workspace.activeLeaf.view; const ctl = v.controller; const cfg = ctl && (ctl.viewConfig || (ctl.query && ctl.query.views && ctl.query.views.find(x => x.name === 'List'))); return cfg ? JSON.parse(JSON.stringify(cfg, (k, val) => (typeof val === 'function' ? undefined : val))) : Object.keys(ctl || {}); } catch (e) { return 'ERR ' + e.message; }`);
  p.shotOff = await shot('list-indent-off-again', '.workspace-leaf.mod-active .view-content');
  p.changed1 = p.bytes1 !== p.bytes0; p.changed2 = p.bytes2 !== p.bytes1; p.backToOriginal = p.bytes2 === p.bytes0;
  return p;
});

// ---- s03: "Add summary" -----------------------------------------------------
const colIndex = (name) => `(() => { const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); return hs.findIndex(h => (h.textContent || '').replace(/\\s+/g, ' ').trim() === ${JSON.stringify(name)}); })()`;
await step('s03-add-summary', async (p) => {
  await restoreFixtures();
  await openBase(BASE_NAME);
  p.switched = await switchView('Table');
  await sleep(600);
  p.headers = await ev(`return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')).map(e => e.textContent.trim());`);
  const footCell = async (name) => ev(`${READ_LIB} const i = ${colIndex(name)}; const row = document.querySelector('.workspace-leaf.mod-active .bases-table-footer'); const cell = row && row.querySelectorAll('.bases-td')[i]; if (!cell) return null; const b = cell.getBoundingClientRect(); return { i, x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(cell), html: collapse(cell.outerHTML).slice(0, 1000) };`);
  const cell = await footCell('rating');
  p.footerCell = cell || await footCell('Rating');
  const fc = p.footerCell;
  if (!fc) { p.footerHtml = await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active .bases-table-container'); return e ? collapse(e.outerHTML).slice(0, 8000) : null;`); return p; }
  p.bytes0 = readBase();
  await c.clickAt(fc.x, fc.y); await sleep(900);
  p.menu = await surfaces();
  p.menuShot = await shot('summary-menu', '.menu');
  p.clickAdd = await clickText('.menu .suggestion-item', 'Add summary');
  await sleep(900);
  p.afterAdd = await surfaces();
  p.afterAddActive = await activeElement();
  p.afterAddShot = await shot('summary-add-page', '.menu, .modal-container, .suggestion-container');
  p.bytesAfterAddClick = readBase();
  // the fields on that page: inputs, contenteditables, buttons
  const fields = () => ev(`${READ_LIB}
    const root = document.querySelector('.menu') || document.querySelector('.modal-container'); if (!root) return null;
    return { inputs: Array.from(root.querySelectorAll('input, textarea, [contenteditable="true"]')).map(e => { const b = e.getBoundingClientRect(); return { tag: e.tagName.toLowerCase(), cls: String(e.className), type: e.getAttribute('type'), placeholder: e.getAttribute('placeholder') || e.getAttribute('data-placeholder') || (e.querySelector('.cm-placeholder') ? txt(e.querySelector('.cm-placeholder')) : null), value: e.value !== undefined ? e.value : txt(e), x: b.x + b.width / 2, y: b.y + b.height / 2, w: b.width, h: b.height, label: (() => { const r = e.closest('.input-row, .setting-item, .bases-toolbar-section'); return r ? txt(r.querySelector('.input-row-label, .setting-item-name, .bases-toolbar-section-header')) : null; })() }; }).filter(x => x.w > 0),
      buttons: Array.from(root.querySelectorAll('button, .clickable-icon, .mod-cta, .back-button, .bases-toolbar-menu-container-header > *')).map(e => { const b = e.getBoundingClientRect(); return { tag: e.tagName.toLowerCase(), cls: String(e.className), text: txt(e), aria: e.getAttribute('aria-label'), icon: lucide(e), x: b.x + b.width / 2, y: b.y + b.height / 2, w: b.width }; }).filter(x => x.w > 0),
      cmPlaceholders: Array.from(root.querySelectorAll('.cm-placeholder')).map(e => txt(e)) };`);
  p.fields = await fields();
  // fill: a text input gets the name, a contenteditable (CodeMirror) the formula
  const f = p.fields;
  p.fill = [];
  if (f) {
    const textIn = f.inputs.find((x) => x.tag === 'input' && (!x.type || x.type === 'text'));
    const editor = f.inputs.find((x) => x.tag !== 'input');
    if (textIn) {
      await c.clickAt(textIn.x, textIn.y); await sleep(300);
      await c.pressKey('a', { ctrl: true }); await c.typeText('Doubled', 40); await sleep(600);
      p.fill.push({ into: 'text input', label: textIn.label, placeholder: textIn.placeholder, surfaces: await surfaceClasses(), active: await activeElement() });
    }
    if (editor) {
      await c.clickAt(editor.x, editor.y); await sleep(300);
      await c.typeText('values.sum() * 2', 40); await sleep(900);
      p.fill.push({ into: 'editor', label: editor.label, placeholder: editor.placeholder, surfaces: await surfaces(), active: await activeElement() });
      p.fillShot = await shot('summary-add-filled', '.menu, .suggestion-container, .modal-container');
    }
    p.fieldsAfterFill = await fields();
    p.bytesAfterFill = readBase();
    // a save/done button, else Enter
    const btn = (p.fieldsAfterFill && p.fieldsAfterFill.buttons || []).find((b) => /^(save|done|add|create|ok)$/i.test(b.text));
    if (btn) { await c.clickAt(btn.x, btn.y); p.commit = { via: 'button', button: btn.text }; }
    else { await c.pressKey('Escape'); await sleep(300); p.commit = { via: 'Escape (closes the autocomplete)' }; await sleep(300); }
    await sleep(1500);
    p.afterCommit = await surfaces();
    p.afterCommitShot = await shot('summary-add-after-commit', '.menu, .modal-container');
    p.bytesAfterCommit = readBase();
  }
  await dismiss(); await escapeAll(1); await sleep(1200);
  p.bytesAfterClose = readBase();
  p.footerAfter = await ev(`${READ_LIB} const row = document.querySelector('.workspace-leaf.mod-active .bases-table-footer'); return row ? Array.from(row.querySelectorAll('.bases-td')).map(c => txt(c)) : null;`);
  p.tableShot = await shot('summary-add-table-after', '.workspace-leaf.mod-active .bases-table-container');
  // the summary menu again: is the new one listed?
  const fc2 = await footCell('rating') || await footCell('Rating');
  if (fc2) { await c.clickAt(fc2.x, fc2.y); await sleep(900); p.menuAgain = await surfaces(); p.menuAgainShot = await shot('summary-menu-again', '.menu'); await dismiss(); await escapeAll(1); }
  p.restored = await restoreFixtures();
  return p;
});

// ---- s04: shift-click cell selection, copy, paste, undo --------------------
const DATA_ROW = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tbody .bases-tr, .workspace-leaf.mod-active .bases-tr')).filter(r => r.querySelector('.bases-table-cell') && !r.classList.contains('bases-table-group-summary-row') && !r.classList.contains('bases-table-footer') && !r.closest('.bases-thead'))`;
/** Every element under the table container: class and non-style attributes, by DOM index. */
const domSnap = () => ev(`
  const root = document.querySelector('.workspace-leaf.mod-active .bases-view') || document.querySelector('.workspace-leaf.mod-active .view-content');
  const els = [root, ...root.querySelectorAll('*')].filter(e => !(e instanceof SVGElement) || e.tagName === 'svg');
  return els.map(e => { const a = {}; for (const at of e.attributes) if (at.name !== 'style' && at.name !== 'd') a[at.name] = at.value; const row = e.closest && e.closest('.bases-tr'); return { tag: e.tagName.toLowerCase(), attrs: a, where: row ? ((row.querySelector('.bases-td') || {}).textContent || '').trim().slice(0, 30) + ' / ' + Array.from(row.children).indexOf(e.closest('.bases-td') || e) : null }; });`);
const domDiff = (a, b) => {
  const outd = { countBefore: a.length, countAfter: b.length, changed: [], added: [] };
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i].tag !== b[i].tag) { outd.misaligned = i; break; }
    const ka = JSON.stringify(a[i].attrs), kb = JSON.stringify(b[i].attrs);
    if (ka !== kb) outd.changed.push({ i, tag: b[i].tag, where: b[i].where, before: a[i].attrs, after: b[i].attrs });
  }
  if (b.length > a.length) outd.added = b.slice(a.length).slice(0, 40);
  outd.changed = outd.changed.slice(0, 80);
  return outd;
};
const cellGrid = () => ev(`${READ_LIB}
  const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')).map(h => txt(h));
  return { headers: hs, rows: (${DATA_ROW}).map(r => ({ rowCls: String(r.className), rowAttrs: Array.from(r.attributes).filter(a => a.name !== 'style').map(a => a.name + '=' + a.value), cells: Array.from(r.querySelectorAll(':scope > .bases-td')).map((td, j) => { const cs = getComputedStyle(td); return { col: hs[j], text: txt(td).slice(0, 40), cls: String(td.className), attrs: Array.from(td.attributes).filter(a => a.name !== 'style' && a.name !== 'class').map(a => a.name + '=' + a.value), bg: cs.backgroundColor, shadow: cs.boxShadow, outline: cs.outlineStyle + ' ' + cs.outlineColor }; }) })),
    overlays: Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="select"], .workspace-leaf.mod-active [class*="range"]')).filter(e => !e.closest('.bases-toolbar')).map(e => ({ cls: String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className), rect: rectOf(e), style: e.getAttribute('style') })).slice(0, 30) };`);
const cellAt = (row, col) => ev(`const r = (${DATA_ROW})[${row}]; if (!r) return null; const e = r.querySelectorAll(':scope > .bases-td')[${col}]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: e.textContent.trim(), row: (r.querySelector('.bases-td') || {}).textContent.trim() };`);
const clip = () => ev(`const cb = window.require('electron').clipboard; return { formats: cb.availableFormats(), text: cb.readText(), html: cb.readHTML() };`);
const markClipboard = () => c.writeClipboard('SENTINEL-before-copy');
const focusSummary = () => ev(`${READ_LIB} const a = document.activeElement; return a ? { tag: a.tagName.toLowerCase(), cls: String(a.className).slice(0, 80), editable: a.isContentEditable, inTable: !!a.closest('.bases-table-container') } : null;`);

await step('s04-table-selection', async (p) => {
  await restoreFixtures();
  await openBase(BASE_NAME);
  p.switched = await switchView('Table');
  await sleep(800); await closePopovers();
  p.gridRest = await cellGrid();
  const snap0 = await domSnap();
  // A: click (0, status), shift-click (2, density)
  const a = await cellAt(0, 1), b = await cellAt(2, 3);
  p.anchor = a; p.focus = b;
  await c.clickAt(a.x, a.y); await sleep(700);
  p.afterClick = { active: await focusSummary(), surfaces: await surfaceClasses(), diff: domDiff(snap0, await domSnap()) };
  const snap1 = await domSnap();
  await c.clickAt(b.x, b.y, { mods: { shift: true } }); await sleep(800);
  const snap2 = await domSnap();
  p.afterShiftClick = { active: await focusSummary(), surfaces: await surfaceClasses(), diffFromClick: domDiff(snap1, snap2), diffFromRest: domDiff(snap0, snap2), grid: await cellGrid() };
  p.shiftShot = await shot('table-shift-click', '.workspace-leaf.mod-active .bases-table-container');
  // B: click, Escape (leave any editor), shift-click
  await escapeAll(2); await sleep(300);
  await c.clickAt(a.x, a.y); await sleep(600); await c.pressKey('Escape'); await sleep(400);
  const snap3 = await domSnap();
  p.variantEscape = { activeAfterEscape: await focusSummary() };
  await c.clickAt(b.x, b.y, { mods: { shift: true } }); await sleep(800);
  p.variantEscape.active = await focusSummary();
  p.variantEscape.diff = domDiff(snap3, await domSnap());
  p.variantEscape.grid = await cellGrid();
  p.variantEscapeShot = await shot('table-escape-shift-click', '.workspace-leaf.mod-active .bases-table-container');
  // A click opens the cell's editor, so its Ctrl+C/V belong to the editor.
  // Two Escapes leave the editor (bases-ui s25); the selection is then driven
  // from the table container.
  const leaveEditor = async () => { await c.pressKey('Escape'); await sleep(250); await c.pressKey('Escape'); await sleep(350); };
  // C: click, Escape x2, then Shift+Arrow
  await escapeAll(2); await c.clickAt(a.x, a.y); await sleep(500); await leaveEditor();
  p.variantShiftArrows = { activeBefore: await focusSummary(), gridBefore: (await cellGrid()).overlays };
  const snap4 = await domSnap();
  await c.pressKey('ArrowDown', { shift: true }); await sleep(300); await c.pressKey('ArrowRight', { shift: true }); await sleep(500);
  Object.assign(p.variantShiftArrows, { active: await focusSummary(), diff: domDiff(snap4, await domSnap()), grid: await cellGrid(), shot: await shot('table-shift-arrows', '.workspace-leaf.mod-active .bases-table-container') });
  // C2: click, Escape x2, shift-click
  await escapeAll(2); await c.clickAt(a.x, a.y); await sleep(500); await leaveEditor();
  await c.clickAt(b.x, b.y, { mods: { shift: true } }); await sleep(700);
  p.variantEscape2ShiftClick = { active: await focusSummary(), grid: await cellGrid(), surfaces: await surfaceClasses(), shot: await shot('table-escape2-shift-click', '.workspace-leaf.mod-active .bases-table-container') };
  // D: a mouse drag from a to b
  await escapeAll(2); await sleep(300);
  const snap5 = await domSnap();
  await c.drag(a.x, a.y, b.x, b.y, { steps: 10, pauseMs: 50 }); await sleep(700);
  p.variantDrag = { active: await focusSummary(), surfaces: await surfaceClasses(), diff: domDiff(snap5, await domSnap()), grid: await cellGrid(), shot: await shot('table-mouse-drag', '.workspace-leaf.mod-active .bases-table-container') };
  // Copy from the drag selection (focus on the container, no editor)
  await markClipboard();
  await c.pressKey('c', { ctrl: true }); await sleep(800);
  p.copyDragSelection = { active: await focusSummary(), clipboard: await clip(), custom: await ev(`const cb = window.require('electron').clipboard; const o = {}; for (const f of cb.availableFormats()) { try { o[f] = { buffer: cb.readBuffer(f).toString('utf8').slice(0, 2000), read: String(cb.read(f)).slice(0, 2000) }; } catch (e) { o[f] = 'ERR ' + e.message; } } return o;`) };
  // Copy from the shift-click selection with the editor open (what the first pass did)
  await escapeAll(2); await c.clickAt(a.x, a.y); await sleep(600);
  await c.clickAt(b.x, b.y, { mods: { shift: true } }); await sleep(700);
  await markClipboard();
  await c.pressKey('c', { ctrl: true }); await sleep(800);
  p.copyShiftClickEditorOpen = { active: await focusSummary(), clipboard: await clip() };
  // Paste: copy the block again by drag (so every clipboard format is Obsidian's own),
  // drag-select rows 4..6 (Pumice, Rhyolite, Scoria) x status..density, Ctrl+V
  await escapeAll(2); await sleep(300);
  await c.drag(a.x, a.y, b.x, b.y, { steps: 10, pauseMs: 50 }); await sleep(600);
  await c.pressKey('c', { ctrl: true }); await sleep(800);
  p.pasteClipboard = await clip();
  const notes0 = readNotes(); const base0 = readBase();
  const pa = await cellAt(4, 1), pb = await cellAt(6, 3);
  p.pasteTarget = { from: pa, to: pb };
  await c.drag(pa.x, pa.y, pb.x, pb.y, { steps: 10, pauseMs: 50 }); await sleep(700);
  p.pasteSelection = { active: await focusSummary(), overlays: (await cellGrid()).overlays };
  await c.pressKey('v', { ctrl: true }); await sleep(2500);
  const notes1 = readNotes();
  p.paste = { active: await focusSummary(), surfaces: await surfaceClasses(), changed: diffNotes(notes0, notes1), baseChanged: readBase() !== base0, grid: await cellGrid(), shot: await shot('table-after-paste', '.workspace-leaf.mod-active .bases-table-container') };
  // Undo
  await c.pressKey('z', { ctrl: true }); await sleep(2500);
  const notes2 = readNotes();
  p.undo = { active: await focusSummary(), stillDifferentFromBefore: diffNotes(notes0, notes2), changedByUndo: diffNotes(notes1, notes2), grid: await cellGrid(), shot: await shot('table-after-undo', '.workspace-leaf.mod-active .bases-table-container') };
  // Redo, for the record
  await c.pressKey('z', { ctrl: true, shift: true }); await sleep(2500);
  p.redo = { changedFromUndone: diffNotes(notes2, readNotes()) };
  await c.pressKey('z', { ctrl: true }); await sleep(2000);
  await restoreFixtures(); await sleep(1000);
  // Paste of plain text only (no obsidian/* formats), distinct numbers per cell,
  // into a drag-selected 3x3 block (rows 4..6 x status..density)
  await escapeAll(2); await sleep(300);
  await c.drag(pa.x, pa.y, pb.x, pb.y, { steps: 10, pauseMs: 50 }); await sleep(700);
  p.plainSelection = { active: await focusSummary(), overlays: (await cellGrid()).overlays };
  const PLAIN = ['11', '12', '13'].join('\t') + '\n' + ['21', '22', '23'].join('\t') + '\n' + ['31', '32', '33'].join('\t');
  await c.writeClipboard(PLAIN);
  p.plainClipboard = await clip();
  const n0 = readNotes();
  await c.pressKey('v', { ctrl: true }); await sleep(2500);
  const n1 = readNotes();
  p.plainPaste = { clipboard: PLAIN, active: await focusSummary(), changed: diffNotes(n0, n1), grid: await cellGrid(), shot: await shot('table-after-plain-paste', '.workspace-leaf.mod-active .bases-table-container') };
  await c.pressKey('z', { ctrl: true }); await sleep(2500);
  p.plainUndo = { stillDifferentFromBefore: diffNotes(n0, readNotes()) };
  await escapeAll(2);
  p.restored = await restoreFixtures();
  return p;
});

// ---- s05: kanban drop position and drag data ------------------------------
const REAL_COLS = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-kanban-column')).filter(e => getComputedStyle(e).opacity !== '0' && !/^Fj/.test(e.textContent))`;
const columns = () => ev(`${READ_LIB} return (${REAL_COLS}).map(e => ({ header: txt(e.querySelector('.bases-group-value')) || txt(e.querySelector('.bases-kanban-column-header')), count: txt(e.querySelector('.bases-kanban-column-count')), cards: Array.from(e.querySelectorAll('.bases-kanban-card')).map(k => ({ title: txt(k.querySelector('.mod-title .bases-kanban-card-line')) || txt(k).slice(0, 30), rect: rectOf(k), cls: String(k.className), draggable: k.getAttribute('draggable') })) }));`);
const DRAG_SPY = `
  if (!window.__dragSpy) {
    window.__dragSpy = [];
    const rec = (e, phase) => { const dt = e.dataTransfer; const o = { type: e.type, phase, target: String(e.target && e.target.className || '').slice(0, 80), types: dt ? Array.from(dt.types) : null, effectAllowed: dt ? dt.effectAllowed : null, dropEffect: dt ? dt.dropEffect : null, data: {} };
      if (dt) for (const t of dt.types) { try { o.data[t] = dt.getData(t); } catch (err) { o.data[t] = 'ERR ' + err.message; } }
      if (e.type === 'dragover' && window.__dragSpy.filter(x => x.type === 'dragover').length > 6) return;
      window.__dragSpy.push(o); };
    window.addEventListener('dragstart', (e) => {
      const dt = e.dataTransfer; if (dt && !dt.__wrapped) { const sd = dt.setData.bind(dt); dt.setData = (k, v) => { window.__dragSpy.push({ type: 'setData', key: k, value: String(v).slice(0, 400) }); return sd(k, v); }; const di = dt.setDragImage.bind(dt); dt.setDragImage = (el, x, y) => { window.__dragSpy.push({ type: 'setDragImage', el: String(el && el.className || el && el.tagName || '').slice(0, 80), x, y }); return di(el, x, y); }; dt.__wrapped = true; }
      rec(e, 'capture'); }, true);
    window.addEventListener('dragstart', (e) => rec(e, 'bubble'), false);
    for (const t of ['dragenter', 'dragover', 'drop', 'dragend']) window.addEventListener(t, (e) => rec(e, 'capture'), true);
    for (const t of ['drop']) window.addEventListener(t, (e) => rec(e, 'bubble-defaultPrevented=' + e.defaultPrevented), false);
  }
  window.__dragSpy.length = 0; return true;`;

await step('s05-kanban-drop', async (p) => {
  await restoreFixtures();
  await openBase(BASE_NAME);
  p.switched = await switchView('Board');
  await sleep(800); await closePopovers();
  p.columnsBefore = await columns();
  p.shotBefore = await shot('kanban-before', '.workspace-leaf.mod-active .view-content');
  const cols = p.columnsBefore;
  const done = cols.findIndex((x) => x.header === 'done');
  const todo = cols.findIndex((x) => x.header === 'todo');
  const doing = cols.findIndex((x) => x.header === 'doing');
  p.colIdx = { done, todo, doing };
  const trial = async (label, srcTitle, srcCol, dstCol, where) => {
    await ev(DRAG_SPY);
    const cs = await columns();
    const src = cs[srcCol].cards.find((k) => k.title === srcTitle);
    const dc = cs[dstCol].cards;
    let tx, ty;
    const colRect = await ev(`const e = (${REAL_COLS})[${dstCol}]; const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height };`);
    tx = colRect.x + colRect.w / 2;
    if (where === 'between-0-1') ty = (dc[0].rect.y + dc[0].rect.h + dc[1].rect.y) / 2;
    else if (where === 'upper-half-of-1') ty = dc[1].rect.y + dc[1].rect.h * 0.25;
    else if (where === 'top') ty = dc[0].rect.y + 4;
    else ty = dc[dc.length - 1].rect.y + dc[dc.length - 1].rect.h + 20;
    const r = { label, src: srcTitle, from: cs[srcCol].header, to: cs[dstCol].header, where, target: { x: tx, y: ty }, targetCardsBefore: dc.map((k) => k.title) };
    const notes0 = readNotes(); const base0 = readBase();
    // hover the card first: the first drag of a session has failed to start without it
    await c.mouseMove(src.rect.x + src.rect.w / 2, src.rect.y + 12); await sleep(600);
    let d = await c.dragOverOnly(src.rect.x + src.rect.w / 2, src.rect.y + 12, tx, ty);
    if (!d.started) { r.firstAttempt = d.error; await c.mouseMove(src.rect.x + src.rect.w / 2, src.rect.y + src.rect.h / 2); await sleep(800); await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: Math.round(src.rect.x + 20), y: Math.round(src.rect.y + 12), button: 'left', clickCount: 1 }); await sleep(800); d = await c.dragOverOnly(src.rect.x + 20, src.rect.y + src.rect.h - 12, tx, ty); }
    r.method = 'Input.setInterceptDrags + Input.dispatchDragEvent';
    r.started = d.started;
    r.intercepted = d.data ? { dragOperationsMask: d.data.dragOperationsMask, items: (d.data.items || []).map((i) => ({ mimeType: i.mimeType, data: String(i.data).slice(0, 400), title: i.title, baseURL: i.baseURL })), files: d.data.files, keys: Object.keys(d.data) } : d.error;
    if (d.started) {
      await sleep(400);
      // a second dragOver at the same point (some handlers need two)
      await c.send('Input.dispatchDragEvent', { type: 'dragOver', x: Math.round(tx), y: Math.round(ty), data: d.data }); await sleep(400);
      r.midDrag = await ev(`${READ_LIB} return { marks: Array.from(document.querySelectorAll('.workspace-leaf.mod-active *')).filter(e => /drag|drop|placeholder|is-moving|ghost|insert/i.test(String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className))).map(e => ({ cls: String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className), rect: rectOf(e), style: e.getAttribute('style') })).slice(0, 20), bodyCls: document.body.className, dragGhosts: Array.from(document.querySelectorAll('body > .drag-ghost, body > [class*="drag"]')).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 60) })) };`);
      r.midDragShot = await shot('kanban-' + label + '-mid', '.workspace-leaf.mod-active .view-content');
      await d.finish(tx, ty);
    }
    await sleep(2000);
    r.spy = await ev(`return window.__dragSpy;`);
    r.columnsAfter = (await columns()).map((x) => ({ header: x.header, count: x.count, cards: x.cards.map((k) => k.title) }));
    r.notesChanged = diffNotes(notes0, readNotes());
    r.baseChanged = readBase() !== base0; if (r.baseChanged) r.baseAfter = readBase();
    r.shotAfter = await shot('kanban-' + label + '-after', '.workspace-leaf.mod-active .view-content');
    return r;
  };
  p.trials = [];
  const fresh = async () => { await restoreFixtures(); await openBase(BASE_NAME); await switchView('Board'); await sleep(800); await closePopovers(); await c.mouseMove(5, 5); await sleep(300); };
  // Tuff (todo, last by name) into the middle of done (Obsidian rock | Pumice | Rhyolite)
  p.trials.push(await trial('tuff-mid-done', 'Tuff', todo, done, 'between-0-1'));
  save();
  // Tuff onto the top of done
  await fresh();
  p.trials.push(await trial('tuff-top-done', 'Tuff', todo, done, 'top'));
  save();
  // Andesite (doing, first by name) below the last card of done
  await fresh();
  p.trials.push(await trial('andesite-bottom-done', 'Andesite', doing, done, 'bottom'));
  save();
  // Within a column: Rhyolite (done, last) to the top of done
  await fresh();
  p.trials.push(await trial('rhyolite-top-of-done', 'Rhyolite', done, done, 'top'));
  save();
  // Real mouse press/move/release (no interception), for the record
  await c.send('Input.setInterceptDrags', { enabled: false });
  await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 5, y: 5, button: 'left', clickCount: 1 });
  await fresh(); await sleep(1500);
  {
    await ev(DRAG_SPY);
    const cs2 = await columns();
    p.realMouseColumnsBefore = cs2.map((x) => x.header + ':' + x.cards.map((k) => k.title).join(','));
    const src = cs2.find((x) => x.header === 'todo').cards.find((k) => k.title === 'Tuff');
    const dc = cs2.find((x) => x.header === 'done').cards;
    const notes0 = readNotes();
    const tx = dc[0].rect.x + dc[0].rect.w / 2, ty = (dc[0].rect.y + dc[0].rect.h + dc[1].rect.y) / 2;
    await c.drag(src.rect.x + src.rect.w / 2, src.rect.y + src.rect.h / 2, tx, ty, { steps: 16, pauseMs: 50 });
    await sleep(1800);
    p.realMouse = { method: 'Input.dispatchMouseEvent press/move/release, no interception', spy: await ev(`return window.__dragSpy;`), columnsAfter: (await columns()).map((x) => ({ header: x.header, cards: x.cards.map((k) => k.title) })), notesChanged: diffNotes(notes0, readNotes()) };
  }
  p.restored = await restoreFixtures();
  return p;
});

out.vaultFilesAtEnd = fs.readdirSync(VAULT).sort();
out.baseAtEnd = readBase();
out.shotFiles = fs.readdirSync(SHOTS).sort();
save();
c.close();
process.exit(0);
