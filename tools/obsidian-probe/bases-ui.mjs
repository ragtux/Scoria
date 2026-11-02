// Bases UI and menus: every control, menu row, icon, separator, submenu,
// tooltip and panel the Bases core plugin shows for a `.base` open in the main
// area, in all four layouts, plus the embedded base, the commands and the "New
// base" entry points, keyboard behavior in the panels and the table, and the
// computed styles of the toolbar, header, rows and panels (dark theme).
//
// The format, write-back, expression language and property ids were settled
// by bases.mjs / bases-expr.mjs / bases-ids.mjs; this run is about the UI only.
//
// The Bases toolbar does not open `.menu-item` lists (bases.mjs s05): each
// control opens a `.menu.bases-toolbar-menu` holding a panel of comboboxes,
// native selects, sliders and `.suggestion-item` rows; a combobox's options
// open in a `.suggestion-container` that is not inside the `.menu`; "Edit
// property..." and the formula editor may be pages inside that same menu. So
// every capture here reads *whatever* floats: `.menu`, `.popover`,
// `.suggestion-container`, `.modal-container`, `.notice` — their classes, every
// row-like element with its text, lucide icon, checked/disabled/submenu state,
// inputs and placeholders, plus a collapsed outerHTML and a PNG.
//
// Phases: fixtures | run   (node bases-ui.mjs <phase>)
// ONLY_STEPS=s04,s05 re-runs a step or two (never filters the fixtures phase).
// SHOTS names the folder the PNGs go to.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || path.join(path.dirname(RESULTS || '.'), '2026-11-01-bases-ui-shots');
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run'); process.exit(2); }
if (phase === 'after-restart') process.exit(0);

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.basesUi = all.basesUi || {};
// A re-run with ONLY_STEPS merges into the run record it finds, so the first
// pass's steps survive; its screenshots are numbered from 200 so they never
// overwrite the first pass's files.
const ONLY_RAW = process.env.ONLY_STEPS && phase === 'run' ? process.env.ONLY_STEPS.split(',') : null;
const prev = ONLY_RAW && all.basesUi[phase] && all.basesUi[phase].steps ? all.basesUi[phase] : null;
const out = prev ? { ...prev, reruns: [...(prev.reruns || []), { at: new Date().toISOString(), only: ONLY_RAW }] } : { startedAt: new Date().toISOString(), shots: SHOTS, steps: {}, order: [] };
all.basesUi[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const ONLY = ONLY_RAW;
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  console.error(`[bases-ui] ${name}`);
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; console.error(`[bases-ui] ${name} failed: ${e && e.message}`); }
  save();
  return out.steps[name];
}

// ---- fixtures -------------------------------------------------------------
const vp = (rel) => path.join(VAULT, rel);

/** A tiny valid PNG (w x h, one flat colour), so the cards view has real images. */
function png(w, h, [r, g, b]) {
  const crc = (buf) => { let c = ~0; for (const x of buf) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); } return (~c) >>> 0; };
  const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const cc = Buffer.alloc(4); cc.writeUInt32BE(crc(td)); return Buffer.concat([len, td, cc]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const rows = [];
  for (let y = 0; y < h; y++) { const row = Buffer.alloc(1 + w * 3); for (let x = 0; x < w; x++) { row[1 + x * 3] = r; row[2 + x * 3] = g; row[3 + x * 3] = b; } rows.push(row); }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(Buffer.concat(rows))), chunk('IEND', Buffer.alloc(0))]);
}

const note = (name, fm, body) => [name, `---\n${fm}\n---\n\n${body}\n`];
const NOTES = Object.fromEntries([
  note('Scoria.md', 'title: Scoria rock\nrating: 4\ndue: 2026-03-14\ndone: false\nkinds:\n  - basalt\n  - frothy\nrelated: "[[Pumice]]"\ntags:\n  - rock\n  - volcanic\nstatus: todo\ndensity: 1.21\ncover: "[[red.png]]"\nwhen: 2026-03-14T10:30:00', '# Scoria\n\nFrothed basalt.'),
  note('Pumice.md', 'title: Pumice\nrating: 5\ndue: 2026-04-01\ndone: true\nkinds:\n  - felsic\nrelated: "[[Scoria]]"\ntags:\n  - rock\nstatus: done\ndensity: 0.64\ncover: "[[blue.png]]"', '# Pumice\n\nFloats.'),
  note('Basalt.md', 'title: Basalt\nrating: 3\ndue: 2026-05-20\ndone: false\nkinds:\n  - mafic\nrelated: "[[Obsidian rock]]"\ntags:\n  - rock\nstatus: doing\ndensity: 2.9\ncover: "[[red.png]]"', '# Basalt\n\nDense.'),
  note('Obsidian rock.md', 'title: Obsidian\nrating: 5\ndue: 2026-01-02\ndone: true\nkinds:\n  - glass\n  - felsic\nrelated: "[[Basalt]]"\ntags:\n  - rock\n  - glass\nstatus: done\ndensity: 2.4\ncover: "[[blue.png]]"', '# Obsidian\n\nVolcanic glass.'),
  note('Tuff.md', 'title: Tuff\nrating: 2\ndue: 2026-07-07\ndone: false\nkinds:\n  - ash\nrelated: "[[Scoria]]"\ntags:\n  - rock\nstatus: todo\ndensity: 1.6', '# Tuff\n\nWelded ash.'),
  note('Andesite.md', 'title: Andesite\nrating: 3\ndue: 2026-02-28\ndone: false\nkinds:\n  - intermediate\nrelated: "[[Basalt]]"\ntags:\n  - rock\nstatus: doing\ndensity: 2.7\ncover: "[[red.png]]"', '# Andesite\n\nIntermediate.'),
  note('Rhyolite.md', 'title: Rhyolite\nrating: 4\ndue: 2026-09-09\ndone: true\nkinds:\n  - felsic\nrelated: "[[Pumice]]"\ntags:\n  - rock\nstatus: done\ndensity: 2.5\ncover: "[[blue.png]]"', '# Rhyolite\n\nSilica rich.'),
  note('Field notes.md', 'title: A day out\nrating: 1\ndone: false\ntags:\n  - trip\nstatus: todo', '# Field notes\n\nNo density, no cover, no due.'),
]);

// The main base: four views. The table has order, sort, groupBy, a formula
// column and a summary; the cards view an image property; the kanban a group.
// Base-level filters hold one statement per property type so the operator
// list per type can be read off each row's combobox.
const ROCKS = `filters:
  and:
    - 'file.hasTag("rock")'
    - 'rating >= 1'
    - 'status != "archived"'
formulas:
  score: 'rating * 2'
  label: 'title + " (" + status + ")"'
properties:
  status:
    displayName: Status
  formula.score:
    displayName: Score
  note.rating:
    displayName: Rating
summaries:
  customMean: 'values.mean().round(2)'
views:
  - type: table
    name: Table
    order:
      - file.name
      - status
      - rating
      - due
      - done
      - kinds
      - related
      - tags
      - formula.score
      - density
    sort:
      - property: rating
        direction: DESC
      - property: file.name
        direction: ASC
    groupBy:
      property: status
      direction: ASC
    summaries:
      rating: Average
      density: Sum
      formula.score: Max
    filters:
      and:
        - 'done != "nope"'
  - type: cards
    name: Cards
    order:
      - file.name
      - status
      - rating
      - kinds
    image: note.cover
    cardSize: 220
  - type: list
    name: List
    order:
      - file.name
      - status
      - rating
  - type: kanban
    name: Board
    order:
      - file.name
      - rating
      - due
    groupBy:
      property: status
      direction: ASC
`;
// The formula, tags, link and datetime columns first, so they are among the
// rendered (virtualized) columns and their header, summary and cell editors
// can be reached without a horizontal scroll.
const FORMULA_BASE = `formulas:
  score: 'rating * 2'
properties:
  formula.score:
    displayName: Score
views:
  - type: table
    name: Table
    order:
      - file.name
      - formula.score
      - tags
      - related
      - when
      - density
    summaries:
      formula.score: Max
`;
const MINIMAL = `views:
  - type: table
    name: Plain
`;
const EMBEDS = `# Embeds

A whole base:

![[Rocks.base]]

A named view:

![[Rocks.base#Cards]]

The minimal one:

![[Minimal.base]]
`;

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  for (const [name, text] of Object.entries(NOTES)) fs.writeFileSync(vp(name), text);
  fs.writeFileSync(vp('red.png'), png(64, 48, [200, 60, 40]));
  fs.writeFileSync(vp('blue.png'), png(48, 64, [40, 80, 200]));
  fs.writeFileSync(vp('Rocks.base'), ROCKS);
  fs.writeFileSync(vp('Minimal.base'), MINIMAL);
  fs.writeFileSync(vp('Formula.base'), FORMULA_BASE);
  fs.writeFileSync(vp('Embeds.md'), EMBEDS);
  fs.mkdirSync(vp('Folder'), { recursive: true });
  fs.writeFileSync(vp('Folder/Inside.md'), 'inside\n');
  fs.mkdirSync(SHOTS, { recursive: true });
  out.fixtures = fs.readdirSync(VAULT).sort();
  save();
  process.exit(0);
}

// ---- run ------------------------------------------------------------------
fs.mkdirSync(SHOTS, { recursive: true });
const c = await connect();
const vault = await c.vaultPath();
if (vault.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault open: ' + vault); c.close(); process.exit(3); }
out.vault = vault;
out.version = await c.evaluate(`return { title: document.title, ua: navigator.userAgent, theme: document.body.className };`);
out.label = (out.version.title.match(/Obsidian [\d.]+/) || ['Obsidian ?'])[0];
const ev = (js) => c.evaluate(js);

// ---- screenshots ----------------------------------------------------------
let shotN = prev ? 200 + ((prev.reruns || []).length - 1) * 100 : 0;
/** Full-window PNG, and optionally one clipped to the union of `clipSel`'s rects (plus 10 px). */
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

// ---- DOM readers ----------------------------------------------------------
// One reader for every floating surface. Rows are anything row-like; each is
// recorded once with its depth so nesting (a submenu row, a filter group
// inside a group) can be reconstructed from the order.
const ROW_SEL = '.menu-item, .menu-separator, .bases-toolbar-menu-item, .suggestion-item, .suggestion-empty, .input-row, .filter-row, .filter-group, .filter-group-header, .filter-group-header-actions > *, .bases-toolbar-section-header, .bases-toolbar-menu-container-header, .back-button, .combobox-button, button, input, select:not(.is-measuring), textarea, [contenteditable="true"], .multi-select-pill, .clickable-icon, .setting-item, .modal-title, .modal-content > *, .prompt-input, .suggestion-group, .bases-menu-group, .formula-editor, .filter-row-error, .bases-toolbar-section, .search-input-container';
const READ_LIB = `
  const txt = (e) => (e && e.textContent || '').replace(/\\s+/g, ' ').trim();
  const lucide = (e) => { const s = e && e.querySelector && e.querySelector('svg'); const cl = s ? String(s.getAttribute('class') || '') : ''; const m = cl.match(/lucide-[\\w-]+/); return m ? m[0] : (s ? cl : null); };
  const lucides = (e) => Array.from(e.querySelectorAll('svg')).map(s => { const m = String(s.getAttribute('class') || '').match(/lucide-[\\w-]+/); return m ? m[0] : null; }).filter(Boolean);
  const collapse = (html) => html.replace(/<svg[^>]*class="([^"]*)"[^>]*>[\\s\\S]*?<\\/svg>/g, (m, cl) => '<svg class="' + cl + '"/>').replace(/<svg[\\s\\S]*?<\\/svg>/g, '<svg/>');
  const rectOf = (e) => { const r = e.getBoundingClientRect(); return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) }; };
  const depthFrom = (root, e) => { let d = 0; for (let p = e; p && p !== root; p = p.parentElement) d++; return d; };
  const readRow = (root, e) => {
    const r = {
      tag: e.tagName.toLowerCase(), cls: String(e.className && e.className.baseVal !== undefined ? e.className.baseVal : e.className || ''),
      depth: depthFrom(root, e), text: txt(e).slice(0, 200),
    };
    if (e.classList.contains('menu-separator')) r.separator = true;
    const t = e.querySelector('.menu-item-title, .bases-toolbar-menu-item-name, .combobox-button-label, .input-row-label, .suggestion-content, .setting-item-name, .back-label, .suggestion-title');
    if (t) r.title = txt(t).slice(0, 200);
    const d = e.querySelector('.setting-item-description, .suggestion-note, .suggestion-aux');
    if (d) r.desc = txt(d).slice(0, 200);
    const ic = lucide(e); if (ic) r.icon = ic;
    const ics = lucides(e); if (ics.length > 1) r.icons = ics.slice(0, 12);
    const aria = e.getAttribute('aria-label'); if (aria) r.aria = aria;
    const ph = e.getAttribute('placeholder') || (e.querySelector('[placeholder]') ? e.querySelector('[placeholder]').getAttribute('placeholder') : null); if (ph) r.placeholder = ph;
    if (e.tagName === 'INPUT' || e.tagName === 'TEXTAREA') { r.type = e.getAttribute('type') || 'text'; r.value = e.value; if (e.type === 'checkbox') r.checked = e.checked; if (e.min) r.min = e.min; if (e.max) r.max = e.max; if (e.step) r.step = e.step; }
    if (e.tagName === 'SELECT') { r.value = e.value; r.options = Array.from(e.options).map(o => ({ value: o.value, label: txt(o), selected: o.selected || undefined })); }
    if (e.isContentEditable) r.value = txt(e).slice(0, 200);
    const cb = e.querySelector(':scope > input[type="checkbox"]'); if (cb) r.checkbox = cb.checked;
    const hk = e.querySelector('.menu-item-hotkey, .suggestion-hotkey, .setting-hotkey'); if (hk) r.hotkey = txt(hk);
    const states = ['mod-checked', 'is-active', 'is-selected', 'is-enabled', 'mod-active', 'is-collapsed', 'mod-hidden', 'mod-implicit', 'mod-formula', 'is-disabled', 'mod-warning', 'is-warning', 'mod-submenu', 'has-submenu', 'mod-cta', 'is-pinned'].filter(k => e.classList.contains(k));
    if (states.length) r.states = states;
    if (e.hasAttribute('disabled') || e.classList.contains('is-disabled')) r.disabled = true;
    if (e.querySelector('.menu-item-icon.mod-submenu, .lucide-chevron-right, .menu-item-submenu')) r.submenu = true;
    const sec = e.getAttribute('data-section') || e.getAttribute('data-group'); if (sec) r.section = sec;
    const rr = rectOf(e); r.rect = rr; if (e.offsetHeight === 0 || getComputedStyle(e).display === 'none') r.hidden = true;
    const ti = e.getAttribute('tabindex'); if (ti !== null) r.tabindex = ti;
    return r;
  };
  const readSurface = (root) => ({
    cls: String(root.className || ''), rect: rectOf(root), text: txt(root).slice(0, 400),
    style: root.getAttribute('style'),
    rows: Array.from(root.querySelectorAll(${JSON.stringify(ROW_SEL)})).map(e => readRow(root, e)),
    html: collapse(root.outerHTML).slice(0, 60000),
  });
  const SURF = '.menu, .popover, .suggestion-container, .modal-container, .notice, .prompt, .bases-view-config-modal, .hover-popover';
  const surfaces = () => Array.from(document.querySelectorAll(SURF)).filter(e => !e.closest('.menu') || e.classList.contains('menu') || !e.closest('.menu').contains(e.parentElement) || true).map(readSurface);
`;
const surfaces = () => ev(`${READ_LIB}
  // a .menu inside another .menu is not a thing, but a .suggestion-container can sit in a .modal; report both
  return Array.from(document.querySelectorAll(SURF)).map(readSurface);`);
const dismiss = async () => {
  try { await ev(`document.querySelectorAll('.menu, .suggestion-container').forEach(m => m.remove()); return true;`); } catch { /* ignore */ }
  await sleep(150);
};
const escapeAll = async (n = 2) => { for (let i = 0; i < n; i++) { await c.pressKey('Escape'); await sleep(200); } };
const closeModal = async () => {
  const had = await ev(`const m = document.querySelector('.modal-container'); if (!m) return false; const b = m.querySelector('.modal-close-button'); if (b) b.click(); else m.remove(); return true;`);
  if (had) await sleep(400);
  return had;
};
const activeElement = () => ev(`${READ_LIB}
  const a = document.activeElement; if (!a) return null;
  return { tag: a.tagName.toLowerCase(), cls: String(a.className || '').slice(0, 200), editable: a.isContentEditable, text: txt(a).slice(0, 120), value: a.value !== undefined ? String(a.value).slice(0, 120) : undefined, rect: rectOf(a),
    closest: ['.bases-td', '.bases-tr', '.bases-table-header', '.bases-cards-item', '.bases-kanban-card', '.menu', '.modal-container', '.view-content', '.bases-list-item'].map(s => a.closest(s) ? s : null).filter(Boolean) };`);

/** The active leaf's toolbar and view header, left to right. */
const toolbarDump = () => ev(`${READ_LIB}
  const leaf = document.querySelector('.workspace-leaf.mod-active') || document.querySelector('.workspace-leaf');
  const tb = leaf.querySelector('.bases-toolbar');
  const header = leaf.querySelector('.view-header');
  const tab = document.querySelector('.workspace-split.mod-root .workspace-tab-header.is-active');
  const item = (e) => ({ cls: String(e.className || ''), text: txt(e).slice(0, 120), aria: e.getAttribute('aria-label'), icon: lucide(e), icons: lucides(e), rect: rectOf(e),
    inner: Array.from(e.querySelectorAll('*')).filter(x => x.children.length === 0 || x.tagName === 'SPAN' || x.tagName === 'DIV').slice(0, 12).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className && x.className.baseVal !== undefined ? x.className.baseVal : x.className || ''), text: txt(x).slice(0, 60), icon: x.tagName === 'svg' ? (String(x.getAttribute('class')).match(/lucide-[\\w-]+/) || [null])[0] : undefined })),
    html: collapse(e.outerHTML).slice(0, 2500) });
  return {
    leafCls: String(leaf.className || ''), contentCls: String((leaf.querySelector('.view-content') || {}).className || ''), viewType: (leaf.querySelector('.workspace-leaf-content') || {}).getAttribute ? leaf.querySelector('.workspace-leaf-content').getAttribute('data-type') : null,
    toolbar: tb ? { cls: String(tb.className), rect: rectOf(tb), parentCls: String(tb.parentElement.className), items: Array.from(tb.children).map(item), itemsDeep: Array.from(tb.querySelectorAll('.bases-toolbar-item')).map(item), html: collapse(tb.outerHTML).slice(0, 12000) } : null,
    viewHeader: header ? { cls: String(header.className), rect: rectOf(header), children: Array.from(header.children).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 120), icon: lucide(e), rect: rectOf(e) })),
      icon: lucide(header.querySelector('.view-header-icon')), title: txt(header.querySelector('.view-header-title')), titleCls: String((header.querySelector('.view-header-title') || {}).className || ''),
      actions: Array.from(header.querySelectorAll('.view-actions > *, .view-header-left > *, .view-header-nav-buttons > *')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), text: txt(e).slice(0, 60), rect: rectOf(e), hidden: e.offsetWidth === 0 })),
      html: collapse(header.outerHTML).slice(0, 8000) } : null,
    tab: tab ? { cls: String(tab.className), title: txt(tab.querySelector('.workspace-tab-header-inner-title')), icon: lucide(tab.querySelector('.workspace-tab-header-inner-icon')), aria: tab.getAttribute('aria-label') } : null,
    viewName: (() => { try { const v = app.workspace.activeLeaf.view; return v.controller ? v.controller.viewName : null; } catch (e) { return 'ERR:' + e.message; } })(),
    layout: (() => { try { const v = app.workspace.activeLeaf.view; return v.controller && v.controller.view && v.controller.view.type || (v.controller && v.controller.view && v.controller.view.constructor.name) || null; } catch (e) { return 'ERR:' + e.message; } })(),
  };`);

/** Open a `.base` alone (every other bases/markdown leaf detached first). */
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
/** Open a toolbar panel and wait for its `.menu.bases-toolbar-menu`. A click
 * right after a DOM-removed menu can toggle Obsidian's own "open" state off
 * instead of opening, so retry until the panel is really there. */
const openPanel = async (cls) => {
  for (let attempt = 0; attempt < 3; attempt++) {
    await c.clickElement('.workspace-leaf.mod-active .' + cls);
    for (let i = 0; i < 8; i++) { await sleep(150); if (await ev(`return !!document.querySelector('.menu.bases-toolbar-menu');`)) return { opened: true, attempt }; }
    await dismiss(); await escapeAll(1);
  }
  return { opened: false };
};
/** Switch the active base's view by its name, through the views panel. */
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
/** The first element whose text (own, trimmed) matches, under `root`. */
const clickText = async (sel, text, { exact = true, index = 0 } = {}) => {
  const r = await ev(`
    const els = Array.from(document.querySelectorAll(${JSON.stringify(sel)})).filter(e => { const t = (e.textContent || '').replace(/\\s+/g, ' ').trim(); return ${exact} ? t === ${JSON.stringify(text)} : t.includes(${JSON.stringify(text)}); });
    const el = els[${index}]; if (!el) return null; const b = el.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(el.className) };`);
  if (!r) return { clicked: false, reason: 'no ' + sel + ' with text ' + text };
  await c.clickAt(r.x, r.y); await sleep(700);
  return { clicked: true, at: r };
};
const clickMenuRow = (title) => clickText('.menu .menu-item', title);
const hoverMenuRow = async (title) => {
  const r = await ev(`
    const rows = Array.from(document.querySelectorAll('.menu .menu-item'));
    const el = rows.find((e) => ((e.querySelector('.menu-item-title') || {}).textContent || '').trim() === ${JSON.stringify(title)});
    if (!el) return null; const b = el.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (!r) return { hovered: false };
  await c.mouseMove(r.x, r.y); await sleep(700);
  return { hovered: true, at: r };
};
/** Open the Configure view page for `viewName` through the row's gear icon. */
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
      return { found: true, viaIcon: !!icon, iconLucide: icon ? String((icon.querySelector('svg') || {}).getAttribute ? icon.querySelector('svg').getAttribute('class') : '') : null, x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (!r.found) return r;
    await c.clickAt(r.x, r.y);
    for (let i = 0; i < 20; i++) {
      const ready = await ev(`return !!document.querySelector('.view-config-menu');`);
      if (ready) return { ...r, ready: true, attempt };
      await sleep(250);
    }
  }
  return { found: true, ready: false };
};
const comboOptions = async (sel, index = 0) => {
  const r = await c.rect(sel, index);
  if (!r) return { found: false, sel };
  await c.clickAt(r.cx, r.cy); await sleep(800);
  const s = await surfaces();
  const sugg = s.filter((x) => /suggestion-container/.test(x.cls));
  const sh = await shot('combo-' + sel.replace(/[^a-z0-9]+/gi, '-').slice(0, 40), '.suggestion-container, .menu');
  return { found: true, rect: r, suggestions: sugg, allSurfaces: s.map((x) => x.cls), shot: sh };
};
const STYLE_PROPS = ['fontSize', 'fontWeight', 'fontFamily', 'lineHeight', 'height', 'width', 'minHeight', 'padding', 'margin', 'gap', 'color', 'backgroundColor', 'borderColor', 'borderWidth', 'borderRadius', 'borderBottom', 'boxShadow', 'opacity', 'display', 'alignItems', 'justifyContent', 'position', 'top', 'left', 'maxHeight', 'maxWidth', 'overflow', 'textOverflow', 'whiteSpace', 'cursor'];
const styles = (sels) => ev(`${READ_LIB}
  const props = ${JSON.stringify(STYLE_PROPS)};
  const out = {};
  for (const sel of ${JSON.stringify(sels)}) {
    const el = document.querySelector(sel); if (!el) { out[sel] = null; continue; }
    const cs = getComputedStyle(el); const o = { rect: rectOf(el), cls: String(el.className || '').slice(0, 200) };
    for (const p of props) o[p] = cs[p];
    out[sel] = o;
  }
  return out;`);
const vaultFiles = () => fs.readdirSync(VAULT).filter((n) => !n.startsWith('.')).sort();

// ---- 01: version, commands, hotkeys, ribbon, Files-pane menus --------------
await step('s01-commands-and-entry-points', async (p) => {
  p.commands = await ev(`
    const hm = app.hotkeyManager;
    return Object.keys(app.commands.commands).filter(id => id.startsWith('bases')).map(id => { const cmd = app.commands.commands[id]; return { id, name: cmd.name, icon: cmd.icon || null,
      hasCheckCallback: !!cmd.checkCallback, hasCallback: !!cmd.callback,
      defaultHotkeys: (() => { try { return JSON.parse(JSON.stringify(hm.getDefaultHotkeys ? hm.getDefaultHotkeys(id) : (hm.defaultKeys && hm.defaultKeys[id]) || null)); } catch (e) { return 'ERR:' + e.message; } })(),
      customHotkeys: (() => { try { return JSON.parse(JSON.stringify(hm.getHotkeys ? hm.getHotkeys(id) : null)); } catch (e) { return 'ERR:' + e.message; } })() }; });`);
  p.ribbon = await ev(`return Array.from(document.querySelectorAll('.side-dock-ribbon-action')).map(e => ({ aria: e.getAttribute('aria-label'), icon: (String((e.querySelector('svg') || {}).getAttribute ? e.querySelector('svg').getAttribute('class') : '').match(/lucide-[\\w-]+/) || [null])[0], hidden: e.offsetWidth === 0 }));`);
  p.ribbonConfig = await ev(`try { return JSON.parse(JSON.stringify(app.workspace.leftRibbon.items.map(i => ({ id: i.id, title: i.title, icon: i.icon, hidden: i.hidden })))); } catch (e) { return 'ERR:' + e.message; }`);
  // the palette, filtered to "bases"
  await ev(`app.commands.executeCommandById('command-palette:open'); return true;`); await sleep(800);
  await c.typeText('Bases', 30); await sleep(700);
  p.palette = await surfaces();
  p.paletteShot = await shot('palette-bases', '.prompt, .modal-container');
  await escapeAll(1); await closeModal();
  // the Files pane: the header buttons, the background's and a folder's context menus
  await ev(`app.workspace.revealLeaf(app.workspace.getLeavesOfType('file-explorer')[0]); return true;`); await sleep(600);
  p.filesHeader = await ev(`${READ_LIB} const pane = document.querySelector('.workspace-leaf-content[data-type="file-explorer"]'); if (!pane) return null;
    return Array.from(pane.querySelectorAll('.nav-buttons-container > *')).map(e => ({ aria: e.getAttribute('aria-label'), icon: lucide(e), cls: String(e.className) }));`);
  const bg = await c.rect('.workspace-leaf-content[data-type="file-explorer"] .nav-files-container');
  if (bg) {
    await c.clickAt(bg.cx, bg.y + bg.h - 20, { button: 'right' }); await sleep(800);
    p.filesBackgroundMenu = await surfaces();
    p.filesBackgroundShot = await shot('files-background-menu', '.menu');
    await dismiss();
  }
  const folder = await c.rect('.workspace-leaf-content[data-type="file-explorer"] .nav-folder-title[data-path="Folder"]');
  if (folder) {
    await c.clickAt(folder.cx, folder.cy, { button: 'right' }); await sleep(800);
    p.folderMenu = await surfaces();
    p.folderMenuShot = await shot('files-folder-menu', '.menu');
    await dismiss();
  }
  const baseRow = await c.rect('.workspace-leaf-content[data-type="file-explorer"] .nav-file-title[data-path="Rocks.base"]');
  p.baseRowInExplorer = await ev(`${READ_LIB} const e = document.querySelector('.nav-file-title[data-path="Rocks.base"]'); if (!e) return null; return { cls: String(e.className), text: txt(e), tag: txt(e.querySelector('.nav-file-tag')), html: collapse(e.outerHTML).slice(0, 1500) };`);
  if (baseRow) {
    await c.clickAt(baseRow.cx, baseRow.cy, { button: 'right' }); await sleep(800);
    p.baseFileMenu = await surfaces();
    p.baseFileMenuShot = await shot('files-base-row-menu', '.menu');
    await dismiss();
  }
  // "New base" through the command: what it names the file, and what the first view looks like
  const before = vaultFiles();
  await ev(`app.commands.executeCommandById('bases:new-file'); return true;`); await sleep(3000);
  p.newFileCreated = vaultFiles().filter((n) => !before.includes(n));
  p.newFileBytes = p.newFileCreated.map((n) => ({ name: n, text: fs.readFileSync(vp(n), 'utf8') }));
  p.newFileSurfaces = await surfaces();
  p.newFileToolbar = await toolbarDump();
  p.newFileBody = await ev(`${READ_LIB} const leaf = document.querySelector('.workspace-leaf.mod-active .view-content'); return leaf ? { cls: String(leaf.className), text: txt(leaf).slice(0, 600), html: collapse(leaf.innerHTML).slice(0, 8000) } : null;`);
  p.newFileShot = await shot('new-base-first-view');
  p.newFileActive = await activeElement();
  await escapeAll(1); await dismiss();
  // the "Create new base" entry in the Files folder menu, if there is one: name it, do not click
  return p;
});

// ---- 02: the toolbar and view header per layout ----------------------------
await step('s02-toolbar-per-layout', async (p) => {
  await openBase('Rocks.base');
  p.layouts = {};
  for (const [name, key] of [['Table', 'table'], ['Cards', 'cards'], ['List', 'list'], ['Board', 'kanban']]) {
    const sw = await switchView(name);
    await sleep(500);
    const dump = await toolbarDump();
    const sh = await shot('toolbar-' + key);
    const body = await ev(`${READ_LIB}
      const vc = document.querySelector('.workspace-leaf.mod-active .view-content'); if (!vc) return null;
      const classes = new Set(); vc.querySelectorAll('[class*="bases-"]').forEach(e => String(e.className).split(/\\s+/).forEach(k => k.startsWith('bases-') && classes.add(k)));
      return { contentCls: String(vc.className), classes: Array.from(classes).sort(), structure: collapse(vc.innerHTML).replace(/>\\s+</g, '><').slice(0, 15000) };`);
    p.layouts[key] = { switched: sw, ...dump, body, shot: sh };
    save();
  }
  await switchView('Table');
  // the view-header's More options menu, Bookmark, and the tab header's own context menu
  await dismiss();
  const more = await c.clickElement('.workspace-leaf.mod-active .view-header .view-actions .view-action[aria-label="More options"]');
  await sleep(900);
  p.moreOptions = { rect: more, menus: await surfaces(), shot: await shot('view-header-more-options', '.menu') };
  // submenus of More options
  p.moreOptionsSubmenus = {};
  const subRows = (p.moreOptions.menus[0] ? p.moreOptions.menus[0].rows : []).filter((r) => r.submenu && r.title);
  for (const row of subRows) {
    await hoverMenuRow(row.title); await sleep(600);
    p.moreOptionsSubmenus[row.title] = { menus: await surfaces(), shot: await shot('view-header-more-' + row.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu') };
  }
  await dismiss();
  const tab = await c.rect('.workspace-split.mod-root .workspace-tab-header.is-active');
  if (tab) { await c.clickAt(tab.cx, tab.cy, { button: 'right' }); await sleep(800); p.tabMenu = { menus: await surfaces(), shot: await shot('tab-header-menu', '.menu') }; await dismiss(); }
  // tooltips: hover each toolbar item and read any .tooltip
  p.tooltips = [];
  const n = await ev(`return document.querySelectorAll('.workspace-leaf.mod-active .bases-toolbar-item').length;`);
  for (let i = 0; i < n; i++) {
    const r = await c.rect('.workspace-leaf.mod-active .bases-toolbar-item', i);
    if (!r || !r.w) continue;
    await c.mouseMove(r.cx, r.cy); await sleep(1200);
    p.tooltips.push(await ev(`const t = document.querySelector('.tooltip'); const it = document.querySelectorAll('.workspace-leaf.mod-active .bases-toolbar-item')[${i}]; return { item: it ? it.className : null, aria: it ? it.getAttribute('aria-label') : null, tooltip: t ? { text: t.textContent, cls: t.className } : null };`));
  }
  for (const aria of ['Bookmark', 'More options']) {
    const r = await c.rect(`.workspace-leaf.mod-active .view-header .view-action[aria-label="${aria}"]`);
    if (!r) continue; await c.mouseMove(r.cx, r.cy); await sleep(1200);
    p.tooltips.push(await ev(`const t = document.querySelector('.tooltip'); return { item: ${JSON.stringify(aria)}, tooltip: t ? { text: t.textContent, cls: t.className } : null };`));
  }
  await c.mouseMove(5, 5);
  return p;
});

// ---- 03: the view switcher ------------------------------------------------
await step('s03-view-switcher', async (p) => {
  await switchView('Table');
  await dismiss();
  await openPanel('bases-toolbar-views-menu');
  p.panel = await surfaces();
  p.panelShot = await shot('views-panel', '.menu');
  // hover a non-active row: what appears
  const row = await c.rect('.menu .bases-toolbar-menu-item', 1);
  if (row) { await c.mouseMove(row.cx, row.cy); await sleep(600); p.hoverRow = await surfaces(); p.hoverShot = await shot('views-panel-hover-row', '.menu'); }
  // right-click a row
  if (row) { await c.clickAt(row.cx, row.cy, { button: 'right' }); await sleep(800); p.rightClickRow = await surfaces(); p.rightClickShot = await shot('views-panel-rightclick-row', '.menu'); }
  await dismiss();
  // type into the search box
  await openPanel('bases-toolbar-views-menu');
  await c.clickElement('.menu .search-input-container input'); await sleep(200);
  await c.typeText('Ca', 40); await sleep(500);
  p.searchTyped = await surfaces();
  p.searchShot = await shot('views-panel-search', '.menu');
  await dismiss();
  // the Configure view page and its header's own menu (more-vertical)
  p.config = await openViewConfig('Table');
  p.configPage = await surfaces();
  p.configShot = await shot('view-config-table', '.menu');
  const mv = await ev(`const h = document.querySelector('.menu .bases-toolbar-menu-container-header'); if (!h) return null; const b = Array.from(h.querySelectorAll('.clickable-icon')).filter(e => !e.classList.contains('close-icon'))[0]; if (!b) return null; const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, icon: String((b.querySelector('svg') || {}).getAttribute ? b.querySelector('svg').getAttribute('class') : ''), aria: b.getAttribute('aria-label') };`);
  p.configHeaderButton = mv;
  if (mv) {
    await c.clickAt(mv.x, mv.y); await sleep(800);
    p.configHeaderMenu = await surfaces();
    p.configHeaderMenuShot = await shot('view-config-header-menu', '.menu');
    // submenus / hover each row for a tooltip
    const rows = (p.configHeaderMenu.find((m) => m.rows.some((r) => r.cls.includes('menu-item'))) || { rows: [] }).rows.filter((r) => r.submenu);
    p.configHeaderSubmenus = {};
    for (const r of rows) { await hoverMenuRow(r.title); p.configHeaderSubmenus[r.title] = await surfaces(); }
    await ev(`document.querySelectorAll('.menu').forEach(m => { if (!m.classList.contains('bases-toolbar-menu')) m.remove(); }); return true;`);
  }
  // the back button
  const back = await c.rect('.menu .back-button');
  if (back) { await c.clickAt(back.cx, back.cy); await sleep(600); p.afterBack = await surfaces(); }
  await dismiss();
  // Add view: the page, the Layout combobox's options; then Escape without creating
  await openPanel('bases-toolbar-views-menu');
  p.addViewClick = await clickText('.menu .bases-toolbar-menu-item', 'Add view');
  await sleep(800);
  p.addViewPage = await surfaces();
  p.addViewShot = await shot('add-view-page', '.menu');
  p.addViewLayoutOptions = await comboOptions('.menu .view-config-menu .combobox-button');
  await escapeAll(1);
  p.afterEscapeInLayoutCombo = (await surfaces()).map((s) => s.cls);
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await dismiss(); await escapeAll(1); await sleep(1500);
  p.addViewWroteFile = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
  p.fileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  // per-view keyboard: Escape in the views panel
  await openPanel('bases-toolbar-views-menu');
  p.escapeBefore = (await surfaces()).map((s) => s.cls);
  await c.pressKey('Escape'); await sleep(400);
  p.escapeAfter = (await surfaces()).map((s) => s.cls);
  await dismiss();
  return p;
});

// ---- 04: the Properties panel ---------------------------------------------
await step('s04-properties-panel', async (p) => {
  await switchView('Table'); await dismiss();
  await openPanel('bases-toolbar-properties-menu');
  p.panel = await surfaces();
  p.panelShot = await shot('properties-panel', '.menu');
  // hover a row
  const row = await c.rect('.menu .bases-toolbar-menu-item', 1);
  if (row) { await c.mouseMove(row.cx, row.cy); await sleep(600); p.hoverShot = await shot('properties-panel-hover', '.menu'); p.hoverRow = await ev(`${READ_LIB} const e = document.querySelectorAll('.menu .bases-toolbar-menu-item')[1]; return e ? readRow(e.closest('.menu'), e) : null;`); }
  // the row's own icon on a plain property: the property config page
  const gear = await ev(`const rows = Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-item')); const r = rows.find(e => !e.classList.contains('mod-formula') && !e.classList.contains('mod-implicit')) || rows[1]; const ic = r && r.querySelector('.bases-toolbar-menu-item-icon'); if (!ic) return null; const b = ic.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, name: (r.querySelector('.bases-toolbar-menu-item-name') || {}).textContent, icon: String((ic.querySelector('svg') || {}).getAttribute ? ic.querySelector('svg').getAttribute('class') : ''), aria: ic.getAttribute('aria-label') };`);
  p.propertyGear = gear;
  if (gear) {
    await c.clickAt(gear.x, gear.y); await sleep(900);
    p.propertyPage = await surfaces();
    p.propertyPageShot = await shot('properties-property-page', '.menu, .modal-container, .suggestion-container');
    await dismiss(); await closeModal();
  }
  // a formula property's icon: the formula editor
  await openPanel('bases-toolbar-properties-menu');
  const fgear = await ev(`const r = document.querySelector('.menu .bases-toolbar-menu-item.mod-formula'); const ic = r && r.querySelector('.bases-toolbar-menu-item-icon'); if (!ic) return null; const b = ic.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, name: (r.querySelector('.bases-toolbar-menu-item-name') || {}).textContent, icon: String((ic.querySelector('svg') || {}).getAttribute ? ic.querySelector('svg').getAttribute('class') : ''), aria: ic.getAttribute('aria-label') };`);
  p.formulaGear = fgear;
  if (fgear) {
    await c.clickAt(fgear.x, fgear.y); await sleep(1000);
    p.formulaPage = await surfaces();
    p.formulaPageShot = await shot('properties-formula-page', '.menu, .modal-container, .suggestion-container');
    p.formulaActive = await activeElement();
    // the error state: type an invalid formula into whichever editor is focused
    const ed = await ev(`const e = document.querySelector('.menu .formula-editor .cm-content, .modal-container .cm-content, .menu textarea, .modal-container textarea, .menu input[type="text"], .modal-container input[type="text"]'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className) };`);
    p.formulaEditor = ed;
    if (ed) {
      await c.clickAt(ed.x, ed.y); await sleep(300);
      await c.pressKey('End'); await c.typeText(' + (', 40); await sleep(900);
      p.formulaErrorState = await surfaces();
      p.formulaErrorShot = await shot('properties-formula-error', '.menu, .modal-container, .suggestion-container');
      // the function suggestions: type a letter
      await c.typeText('d', 40); await sleep(900);
      p.formulaSuggestions = await surfaces();
      p.formulaSuggestShot = await shot('properties-formula-suggest', '.menu, .modal-container, .suggestion-container');
      // undo the typing so the file is not left broken
      for (let i = 0; i < 5; i++) await c.pressKey('Backspace');
      await sleep(600);
      p.formulaAfterUndo = await surfaces();
    }
    await escapeAll(2); await dismiss(); await closeModal();
  }
  // the search box: a name that exists, a name that does not (create offer)
  await openPanel('bases-toolbar-properties-menu');
  await c.clickElement('.menu .search-input-container input'); await sleep(200);
  await c.typeText('zzznew', 40); await sleep(700);
  p.searchNew = await surfaces();
  p.searchNewShot = await shot('properties-panel-create', '.menu, .suggestion-container');
  await escapeAll(1); await dismiss();
  // toggle a property off and on through its checkbox, and read the file
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await openPanel('bases-toolbar-properties-menu');
  const cb = await ev(`const rows = Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-item')); const r = rows.find(e => (e.querySelector('.bases-toolbar-menu-item-name') || {}).textContent === 'density'); const i = r && r.querySelector('input[type="checkbox"]'); if (!i) return null; const b = (i.offsetWidth ? i : r).getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, checked: i.checked };`);
  p.toggle = cb;
  if (cb) {
    await c.clickAt(cb.x, cb.y); await sleep(1200);
    p.toggleAfter = await ev(`const rows = Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-item')); const r = rows.find(e => (e.querySelector('.bases-toolbar-menu-item-name') || {}).textContent === 'density'); return r ? { cls: r.className, checked: (r.querySelector('input[type="checkbox"]') || {}).checked } : null;`);
    p.toggleShot = await shot('properties-panel-toggled', '.menu');
    await c.clickAt(cb.x, cb.y); await sleep(1200);
  }
  await dismiss(); await sleep(1500);
  p.fileChanged = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
  p.fileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  return p;
});

// ---- 05: Filters ----------------------------------------------------------
await step('s05-filters', async (p) => {
  await switchView('Table'); await dismiss();
  await openPanel('bases-toolbar-filter-menu');
  p.panelCollapsed = await surfaces();
  p.collapsedShot = await shot('filters-panel-collapsed', '.menu');
  // expand every collapsed section header
  for (let i = 0; i < 4; i++) {
    const h = await ev(`const e = Array.from(document.querySelectorAll('.menu .bases-toolbar-section-header.is-collapsed'))[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 12, y: b.y + b.height / 2, text: e.textContent };`);
    if (!h) break; await c.clickAt(h.x, h.y); await sleep(500);
  }
  p.panel = await surfaces();
  p.panelShot = await shot('filters-panel-expanded', '.menu');
  p.conjunctionOptions = await ev(`return Array.from(document.querySelectorAll('.menu select.conjunction')).map(s => ({ value: s.value, options: Array.from(s.options).map(o => ({ value: o.value, label: o.textContent })) }));`);
  // the operator list per property type: the filter rows are in the fixture's order
  p.operators = {};
  const nrows = await ev(`return document.querySelectorAll('.menu .filter-row').length;`);
  for (let i = 0; i < nrows; i++) {
    const prop = await ev(`const r = document.querySelectorAll('.menu .filter-row')[${i}]; if (!r) return null; return { property: (r.querySelector('.filter-property-select .combobox-button-label') || {}).textContent, operator: (r.querySelector('.filter-operator .combobox-button-label') || {}).textContent, propIcon: (String((r.querySelector('.filter-property-select svg') || {}).getAttribute ? r.querySelector('.filter-property-select svg').getAttribute('class') : '').match(/lucide-[\\w-]+/) || [null])[0] };`);
    if (!prop) continue;
    const op = await c.rect('.menu .filter-row .filter-operator', i);
    if (!op) continue;
    await c.clickAt(op.cx, op.cy); await sleep(800);
    const s = await surfaces();
    p.operators[`${i}:${prop.property}`] = { ...prop, options: s.filter((x) => /suggestion-container/.test(x.cls)).map((x) => x.rows.filter((r) => r.cls.includes('suggestion-item')).map((r) => ({ text: r.text, icon: r.icon, selected: r.states }))), shot: i < 3 ? await shot('filters-operators-' + i, '.menu, .suggestion-container') : undefined };
    await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
    save();
  }
  // the property combobox: what it lists (with per-type icons)
  const pc = await c.rect('.menu .filter-row .filter-property-select', 0);
  if (pc) { await c.clickAt(pc.cx, pc.cy); await sleep(800); p.propertyCombo = await surfaces(); p.propertyComboShot = await shot('filters-property-combo', '.menu, .suggestion-container'); await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`); }
  // the advanced toggle on the first row
  const adv = await c.rect('.menu .filter-row .filer-expression-mode-toggle, .menu .filter-row [aria-label="Advanced filter"]', 0);
  if (adv) {
    await c.clickAt(adv.cx, adv.cy); await sleep(800);
    p.advanced = await surfaces(); p.advancedShot = await shot('filters-advanced', '.menu, .suggestion-container');
    p.advancedActive = await activeElement();
    await c.clickAt(adv.cx, adv.cy); await sleep(600);
  }
  // hover a row: the row actions' visibility and tooltips
  const r0 = await c.rect('.menu .filter-row', 0);
  if (r0) { await c.mouseMove(r0.cx, r0.cy); await sleep(700); p.hoverRow = await ev(`${READ_LIB} const r = document.querySelector('.menu .filter-row'); return { actions: Array.from(r.querySelectorAll('.filter-row-actions > *')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), opacity: getComputedStyle(e).opacity, display: getComputedStyle(e).display })), grip: !!r.querySelector('.grip-handle, [class*="drag"]') };`); }
  // group header actions: hover the header
  p.groupHeaderActions = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.menu .filter-group-header')).map(h => ({ cls: String(h.className), select: (h.querySelector('select') || {}).value, actions: Array.from(h.querySelectorAll('.filter-group-header-actions > *')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), text: txt(e) })) }));`);
  // "Add filter" / "Add filter group" / any action rows: list, then click Add filter group once and read the new DOM
  p.actionRows = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.menu .bases-toolbar-menu-action, .menu .filter-group-actions > *, .menu button, .menu .clickable-icon[aria-label]')).map(e => ({ cls: String(e.className), text: txt(e), aria: e.getAttribute('aria-label'), icon: lucide(e), hidden: e.offsetHeight === 0 }));`);
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  const addGroup = await clickText('.menu .text-icon-button, .menu .bases-toolbar-menu-action', 'Add filter group');
  p.addGroupClick = addGroup;
  await sleep(800);
  p.afterAddGroup = await surfaces(); p.afterAddGroupShot = await shot('filters-after-add-group', '.menu');
  const addFilter = await clickText('.menu .text-icon-button, .menu .bases-toolbar-menu-action', 'Add filter');
  p.addFilterClick = addFilter; await sleep(800);
  p.afterAddFilter = await surfaces(); p.afterAddFilterShot = await shot('filters-after-add-filter', '.menu, .suggestion-container');
  p.afterAddFilterActive = await activeElement();
  // the property combobox of the new empty row, if one opened: dump it
  await escapeAll(1);
  // remove the group again: the Remove buttons
  p.removeButtons = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.menu [aria-label*="Remove"], .menu [aria-label*="Delete"]')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e) }));`);
  await dismiss(); await sleep(1500);
  p.fileChanged = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
  p.fileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  // put the fixture back if the empty group was written, so later steps see the same base
  if (p.fileChanged) { fs.writeFileSync(vp('Rocks.base'), before); await sleep(2000); p.restored = true; }
  // Enter / Escape inside a value field
  await openPanel('bases-toolbar-filter-menu');
  for (let i = 0; i < 4; i++) { const h = await ev(`const e = Array.from(document.querySelectorAll('.menu .bases-toolbar-section-header.is-collapsed'))[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 12, y: b.y + b.height / 2 };`); if (!h) break; await c.clickAt(h.x, h.y); await sleep(400); }
  const val = await c.rect('.menu .filter-row .filter-rhs-container [contenteditable="true"], .menu .filter-row .filter-rhs-container input', 0);
  if (val) {
    await c.clickAt(val.cx, val.cy); await sleep(300);
    p.valueFocused = await activeElement();
    await c.pressKey('Enter'); await sleep(500);
    p.afterEnter = { active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls) };
    await c.pressKey('Escape'); await sleep(500);
    p.afterEscape = { active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls) };
  }
  await dismiss();
  return p;
});

// ---- 06: Sort and Group panels ---------------------------------------------
await step('s06-sort-and-group', async (p) => {
  await switchView('Table'); await dismiss();
  await openPanel('bases-toolbar-sort-menu');
  p.sortPanel = await surfaces(); p.sortShot = await shot('sort-panel', '.menu');
  const r0 = await c.rect('.menu .base-toolbar-sort-item', 0);
  if (r0) { await c.mouseMove(r0.cx, r0.cy); await sleep(600); p.sortHover = await ev(`${READ_LIB} const r = document.querySelector('.menu .base-toolbar-sort-item'); return { grip: r.querySelector('.grip-handle') ? { icon: lucide(r.querySelector('.grip-handle')), opacity: getComputedStyle(r.querySelector('.grip-handle')).opacity, cursor: getComputedStyle(r.querySelector('.grip-handle')).cursor } : null, remove: Array.from(r.querySelectorAll('.clickable-icon')).map(e => ({ aria: e.getAttribute('aria-label'), icon: lucide(e), opacity: getComputedStyle(e).opacity })) };`); p.sortHoverShot = await shot('sort-panel-hover', '.menu'); }
  p.sortPropertyOptions = await comboOptions('.menu .base-toolbar-sort-item .bases-sort-property', 0);
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  p.sortDirectionOptions = await comboOptions('.menu .base-toolbar-sort-item .bases-sort-direction', 0);
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  // Add sort: the new row
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  p.addSort = await clickText('.menu .bases-toolbar-menu-action', 'Add sort');
  await sleep(800);
  p.afterAddSort = await surfaces(); p.afterAddSortShot = await shot('sort-panel-after-add', '.menu, .suggestion-container');
  await escapeAll(1);
  // Escape in the panel
  p.escapeSurfaces = (await surfaces()).map((s) => s.cls);
  await dismiss(); await sleep(1500);
  p.sortFileChanged = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
  if (p.sortFileChanged) { p.sortFileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8'); fs.writeFileSync(vp('Rocks.base'), before); await sleep(2000); }
  // the Group panel
  await openPanel('bases-toolbar-group-menu');
  p.groupPanel = await surfaces(); p.groupShot = await shot('group-panel', '.menu');
  p.groupPropertyOptions = await comboOptions('.menu .bases-groupby-container .bases-sort-property', 0);
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  p.groupDirectionOptions = await comboOptions('.menu .bases-groupby-container .bases-sort-direction', 0);
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  p.groupListSection = await ev(`${READ_LIB} const s = document.querySelector('.menu .bases-group-list-container'); return s ? { display: getComputedStyle(s).display, html: collapse(s.outerHTML).slice(0, 6000), rows: Array.from(s.querySelectorAll('.suggestion-item, .bases-toolbar-menu-item')).map(e => readRow(s, e)) } : null;`);
  await dismiss();
  // the Results panel, for completeness, with the fixture's state
  await openPanel('bases-toolbar-results-menu');
  p.resultsPanel = await surfaces(); p.resultsShot = await shot('results-panel', '.menu');
  await dismiss();
  // the Search control: what clicking it does
  const s = await c.clickElement('.workspace-leaf.mod-active .bases-toolbar-search'); await sleep(800);
  p.searchClick = { rect: s, toolbar: await toolbarDump(), active: await activeElement(), surfaces: (await surfaces()).map((x) => x.cls), shot: await shot('toolbar-search-open') };
  await c.typeText('Pum', 40); await sleep(900);
  p.searchTyped = { toolbar: (await toolbarDump()).toolbar, rows: await ev(`return document.querySelectorAll('.workspace-leaf.mod-active .bases-tr:not(.bases-thead .bases-tr)').length;`), shot: await shot('toolbar-search-typed') };
  await c.pressKey('Escape'); await sleep(600);
  p.searchAfterEscape = { toolbar: (await toolbarDump()).toolbar, active: await activeElement() };
  await ev(`const i = document.querySelector('.workspace-leaf.mod-active .bases-toolbar input'); if (i) { i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true })); } return true;`);
  await escapeAll(1);
  return p;
});

// ---- 07: layout settings per layout (Configure view) -----------------------
await step('s07-view-config-per-layout', async (p) => {
  p.layouts = {};
  for (const [name, key] of [['Table', 'table'], ['Cards', 'cards'], ['List', 'list'], ['Board', 'kanban']]) {
    await switchView(name);
    const opened = await openViewConfig(name);
    const page = await surfaces();
    const sh = await shot('view-config-' + key, '.menu');
    const rec = { opened, page, shot: sh };
    // the selects' options are in `page`; open the comboboxes other than Layout
    const combos = await ev(`return Array.from(document.querySelectorAll('.menu .view-config-menu .input-row')).map((r, i) => ({ i, label: (r.querySelector('.input-row-label') || {}).textContent || null, combo: !!r.querySelector('.combobox-button') })).filter(x => x.combo && x.label !== 'Layout');`);
    rec.combos = {};
    for (const cb of combos) {
      const r = await c.rect('.menu .view-config-menu .input-row', cb.i);
      const b = await ev(`const r = document.querySelectorAll('.menu .view-config-menu .input-row')[${cb.i}]; const e = r && r.querySelector('.combobox-button'); if (!e) return null; const bb = e.getBoundingClientRect(); return { x: bb.x + bb.width / 2, y: bb.y + bb.height / 2 };`);
      if (!b) continue;
      await c.clickAt(b.x, b.y); await sleep(800);
      rec.combos[cb.label] = { suggestions: (await surfaces()).filter((x) => /suggestion-container/.test(x.cls)), shot: await shot(`view-config-${key}-${String(cb.label).replace(/[^a-z0-9]+/gi, '-').toLowerCase()}`, '.menu, .suggestion-container') };
      await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
      if (!(await ev(`return !!document.querySelector('.view-config-menu');`))) await openViewConfig(name);
    }
    // slider tooltips / values
    rec.sliders = await ev(`return Array.from(document.querySelectorAll('.menu .view-config-menu input[type="range"]')).map(e => ({ label: ((e.closest('.input-row') || {}).querySelector ? (e.closest('.input-row').querySelector('.input-row-label') || {}).textContent : null), min: e.min, max: e.max, step: e.step, value: e.value, sliderValueText: ((e.closest('.input-row') || {}).querySelector ? ((e.closest('.input-row').querySelector('.slider-value') || {}).textContent || null) : null) }));`);
    await dismiss();
    p.layouts[key] = rec; save();
  }
  await switchView('Table');
  return p;
});

// ---- 08: the table's column header ----------------------------------------
await step('s08-column-header', async (p) => {
  await switchView('Table'); await dismiss();
  p.headerDom = await ev(`${READ_LIB}
    const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header, .workspace-leaf.mod-active .bases-th'));
    const head = document.querySelector('.workspace-leaf.mod-active .bases-thead, .workspace-leaf.mod-active .bases-table-header-row, .workspace-leaf.mod-active [class*="bases-table-head"]');
    return { count: hs.length, headers: hs.map(e => ({ cls: String(e.className), text: txt(e), icon: lucide(e), icons: lucides(e), rect: rectOf(e), html: collapse(e.outerHTML).slice(0, 1500) })), headRow: head ? { cls: String(head.className), html: collapse(head.outerHTML).replace(/>\\s+</g, '><').slice(0, 12000) } : null,
      addControl: Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-container [class*="add"], .workspace-leaf.mod-active .bases-table-container .clickable-icon')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), rect: rectOf(e), text: txt(e).slice(0, 40) })) };`);
  // hover the first header: resize handle, drag affordance, cursor
  const h0 = await c.rect('.workspace-leaf.mod-active .bases-table-header', 1);
  if (h0) {
    await c.mouseMove(h0.cx, h0.cy); await sleep(700);
    p.hoverHeader = await ev(`${READ_LIB} const e = document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header')[1]; const rz = e.querySelector('.bases-table-header-resizer'); const cs = getComputedStyle(e);
      return { cls: String(e.className), cursor: cs.cursor, bg: cs.backgroundColor, draggable: e.getAttribute('draggable'), resizer: rz ? { cls: String(rz.className), rect: rectOf(rz), cursor: getComputedStyle(rz).cursor, opacity: getComputedStyle(rz).opacity, bg: getComputedStyle(rz).backgroundColor, width: getComputedStyle(rz).width } : null, tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`);
    p.hoverHeaderShot = await shot('table-header-hover', '.workspace-leaf.mod-active .bases-table-container');
    const rz = await c.rect('.workspace-leaf.mod-active .bases-table-header .bases-table-header-resizer', 1);
    if (rz) { await c.mouseMove(rz.cx, rz.cy); await sleep(600); p.hoverResizer = await ev(`const rz = document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header-resizer')[1]; const cs = getComputedStyle(rz); return { cursor: cs.cursor, opacity: cs.opacity, bg: cs.backgroundColor, cls: rz.className };`); p.hoverResizerShot = await shot('table-header-resizer-hover', '.workspace-leaf.mod-active .bases-table-container'); }
    // left click
    await c.clickAt(h0.cx, h0.cy); await sleep(800);
    p.leftClick = { surfaces: await surfaces(), active: await activeElement(), shot: await shot('table-header-click', '.menu, .workspace-leaf.mod-active .bases-table-container') };
    await dismiss();
    // right click
    await c.clickAt(h0.cx, h0.cy, { button: 'right' }); await sleep(800);
    p.rightClick = { surfaces: await surfaces(), shot: await shot('table-header-context', '.menu') };
    // the submenus (Summarize..., and anything with an arrow)
    p.submenus = {};
    const menu = p.rightClick.surfaces.find((m) => /^menu/.test(m.cls));
    for (const r of (menu ? menu.rows : []).filter((r) => r.cls.includes('menu-item') && (r.submenu || /\.\.\.$/.test(r.title || '')))) {
      await hoverMenuRow(r.title); await sleep(500);
      let s = await surfaces();
      if (s.length < 2) { await clickMenuRow(r.title); await sleep(800); s = await surfaces(); }
      p.submenus[r.title] = { surfaces: s, shot: await shot('table-header-' + r.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .modal-container, .suggestion-container') };
      await closeModal();
      // re-open the context menu for the next one
      await dismiss(); await c.clickAt(h0.cx, h0.cy, { button: 'right' }); await sleep(800);
    }
    await dismiss();
    // the formula column's menu (Edit formula?)
    const fh = await ev(`const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header')); const e = hs.find(h => /Score/.test(h.textContent)); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (fh) { await c.clickAt(fh.x, fh.y, { button: 'right' }); await sleep(800); p.formulaHeaderMenu = { surfaces: await surfaces(), shot: await shot('table-header-formula-context', '.menu') }; await dismiss(); }
    const nh = await ev(`const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header')); const e = hs.find(h => /^\\s*Name\\s*$|file name/i.test(h.textContent)); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: e.textContent };`);
    if (nh) { await c.clickAt(nh.x, nh.y, { button: 'right' }); await sleep(800); p.nameHeaderMenu = { header: nh, surfaces: await surfaces(), shot: await shot('table-header-name-context', '.menu') }; await dismiss(); }
  }
  // the "+" at the right end of the header row
  const add = await ev(`const cands = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-container *')).filter(e => e.querySelector(':scope > svg.lucide-plus, :scope > svg[class*="plus"]') || /bases-table-(header-)?add|mod-add|new-column/.test(String(e.className))); const e = cands[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { cls: String(e.className), aria: e.getAttribute('aria-label'), x: b.x + b.width / 2, y: b.y + b.height / 2, w: b.width, h: b.height, parentCls: String(e.parentElement.className) };`);
  p.addColumn = add;
  if (add && add.w) {
    await c.mouseMove(add.x, add.y); await sleep(900);
    p.addColumnTooltip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
    await c.clickAt(add.x, add.y); await sleep(900);
    p.addColumnMenu = { surfaces: await surfaces(), shot: await shot('table-add-column', '.menu, .suggestion-container') };
    await escapeAll(1); await dismiss();
  }
  // the summary row and its per-column menu
  p.summaryRow = await ev(`${READ_LIB} const s = document.querySelector('.workspace-leaf.mod-active [class*="summar"]'); if (!s) return { none: true, classes: Array.from(new Set(Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-container [class]')).flatMap(e => String(e.className).split(/\\s+/)))).filter(k => k.startsWith('bases-')).sort() };
    const row = s.closest('.bases-tr') || s; return { cls: String(s.className), rowCls: String(row.className), rect: rectOf(row), cells: Array.from(row.querySelectorAll('.bases-td, [class*="summary-cell"], [class*="summary"]')).map(e => ({ cls: String(e.className), text: txt(e), icon: lucide(e), rect: rectOf(e) })), html: collapse(row.outerHTML).slice(0, 6000) };`);
  const sc = await ev(`const cells = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="summary"]')).filter(e => e.offsetWidth > 20); const e = cells.find(x => /\\d/.test(x.textContent)) || cells[1] || cells[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: e.textContent, cls: String(e.className) };`);
  if (sc) {
    await c.mouseMove(sc.x, sc.y); await sleep(600);
    p.summaryHover = { cell: sc, shot: await shot('table-summary-hover', '.workspace-leaf.mod-active .bases-table-container'), tooltip: await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`) };
    await c.clickAt(sc.x, sc.y); await sleep(800);
    p.summaryClickMenu = { surfaces: await surfaces(), shot: await shot('table-summary-menu', '.menu, .suggestion-container') };
    const sub = (p.summaryClickMenu.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && r.submenu);
    p.summarySubmenus = {};
    for (const r of sub) { await hoverMenuRow(r.title); await sleep(500); p.summarySubmenus[r.title] = await surfaces(); }
    await dismiss();
    const empty = await ev(`const cells = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="summary"]')).filter(e => e.offsetWidth > 20 && !/\\S/.test(e.textContent)); const e = cells[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className) };`);
    if (empty) { await c.clickAt(empty.x, empty.y); await sleep(800); p.summaryEmptyClick = { cell: empty, surfaces: await surfaces(), shot: await shot('table-summary-empty-menu', '.menu, .suggestion-container') }; await dismiss(); }
  }
  // header drag: start a drag and read the affordance mid-way, then cancel
  const a = await c.rect('.workspace-leaf.mod-active .bases-table-header', 2);
  const b = await c.rect('.workspace-leaf.mod-active .bases-table-header', 4);
  if (a && b) {
    const d = await c.dragOverOnly(a.cx, a.cy, b.cx, b.cy);
    p.headerDrag = { started: d.started, data: d.data ? { mimeTypes: d.data.items && d.data.items.map((i) => i.mimeType), items: d.data.items && d.data.items.map((i) => ({ mimeType: i.mimeType, data: String(i.data).slice(0, 200) })) } : d.error };
    if (d.started) {
      p.headerDragMid = await ev(`${READ_LIB} return { classesWithDrag: Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], body > .drag-ghost, .drag-reorder-ghost')).map(e => ({ cls: String(e.className), rect: rectOf(e), text: txt(e).slice(0, 60) })), bodyCls: document.body.className };`);
      p.headerDragShot = await shot('table-header-drag', '.workspace-leaf.mod-active .bases-table-container');
      await d.finish();
    }
  }
  return p;
});

// ---- 09: rows and cells ---------------------------------------------------
await step('s09-rows-and-cells', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  p.tableDom = await ev(`${READ_LIB}
    const t = document.querySelector('.workspace-leaf.mod-active .bases-table-container'); if (!t) return null;
    const rows = Array.from(t.querySelectorAll('.bases-tr')).slice(0, 6);
    return { cls: String(t.className), rows: rows.map(r => ({ cls: String(r.className), rect: rectOf(r), cells: Array.from(r.children).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 60), icon: lucide(e), rect: rectOf(e), inner: collapse(e.innerHTML).slice(0, 600) })) })),
      groups: Array.from(t.querySelectorAll('[class*="group"]')).slice(0, 6).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 80), icon: lucide(e), icons: lucides(e), rect: rectOf(e), html: collapse(e.outerHTML).slice(0, 1200) })) };`);
  // group header: hover and click
  const g = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-table-container [class*="group-header"], .workspace-leaf.mod-active .bases-table-container [class*="bases-group"]'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 30, y: b.y + b.height / 2, cls: String(e.className) };`);
  p.groupHeader = g;
  if (g) {
    await c.mouseMove(g.x, g.y); await sleep(700);
    p.groupHover = { controls: await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active .bases-table-container [class*="group-header"], .workspace-leaf.mod-active .bases-table-container [class*="bases-group"]'); return Array.from(e.querySelectorAll('*')).filter(x => x.matches('.clickable-icon, button, [aria-label], svg')).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className && x.className.baseVal !== undefined ? x.className.baseVal : x.className), aria: x.getAttribute('aria-label'), opacity: getComputedStyle(x).opacity }));`), shot: await shot('table-group-header-hover', '.workspace-leaf.mod-active .bases-table-container') };
    await c.clickAt(g.x, g.y); await sleep(800);
    p.groupClick = { surfaces: await surfaces(), dom: await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-table-container [class*="group-header"], .workspace-leaf.mod-active .bases-table-container [class*="bases-group"]'); return { cls: String(e.className), parentCls: String(e.parentElement.className) };`), shot: await shot('table-group-header-click', '.workspace-leaf.mod-active .bases-table-container') };
    await dismiss();
    await c.clickAt(g.x, g.y, { button: 'right' }); await sleep(800);
    p.groupRightClick = { surfaces: await surfaces(), shot: await shot('table-group-header-context', '.menu') };
    await dismiss();
    // click again to restore if it folded
    const folded = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-table-container [class*="group-header"], .workspace-leaf.mod-active .bases-table-container [class*="bases-group"]'); return /collapsed|folded/.test(String(e.className) + String(e.parentElement.className));`);
    if (folded) { await c.clickAt(g.x, g.y); await sleep(600); }
  }
  // the first data row: hover, right-click the name cell, right-click another cell
  const nameCell = await ev(`const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); if (!r) return null; const e = r.querySelector('.bases-td'); const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: e.textContent, rowCls: String(r.className) };`);
  p.nameCell = nameCell;
  if (nameCell) {
    await c.mouseMove(nameCell.x, nameCell.y); await sleep(700);
    p.rowHover = { row: await ev(`${READ_LIB} const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); const cs = getComputedStyle(r); return { cls: String(r.className), bg: cs.backgroundColor, cellBg: getComputedStyle(r.querySelector('.bases-td')).backgroundColor, controls: Array.from(r.querySelectorAll('.clickable-icon, button, [aria-label]')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), opacity: getComputedStyle(x).opacity })), tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`), shot: await shot('table-row-hover', '.workspace-leaf.mod-active .bases-table-container') };
    await c.clickAt(nameCell.x, nameCell.y, { button: 'right' }); await sleep(800);
    p.nameCellMenu = { surfaces: await surfaces(), shot: await shot('table-row-context', '.menu') };
    const sub = (p.nameCellMenu.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && r.submenu);
    p.nameCellSubmenus = {};
    for (const r of sub) { await hoverMenuRow(r.title); await sleep(500); p.nameCellSubmenus[r.title] = { surfaces: await surfaces(), shot: await shot('table-row-context-' + r.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu') }; }
    await dismiss();
    // single click on the name cell: edit? open?
    await c.clickAt(nameCell.x, nameCell.y); await sleep(900);
    p.nameCellClick = { active: await activeElement(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), surfaces: (await surfaces()).map((s) => s.cls), cellNow: await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active .bases-td'); return e ? { cls: String(e.className), html: collapse(e.innerHTML).slice(0, 800) } : null;`), shot: await shot('table-name-cell-click', '.workspace-leaf.mod-active .bases-table-container') };
    await escapeAll(1);
    if (p.nameCellClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); }
    // the link inside the name cell
    const link = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-td a, .workspace-leaf.mod-active .bases-td .internal-link, .workspace-leaf.mod-active .bases-td [class*="link"]'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + Math.min(10, b.width / 2), y: b.y + b.height / 2, cls: String(e.className), tag: e.tagName, text: e.textContent };`);
    p.nameLink = link;
    if (link) { await c.mouseMove(link.x, link.y); await sleep(900); p.nameLinkHover = { cursor: await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-td a, .workspace-leaf.mod-active .bases-td .internal-link, .workspace-leaf.mod-active .bases-td [class*="link"]'); return getComputedStyle(e).cursor;`), popover: (await surfaces()).map((s) => s.cls) }; }
    // double click on a data cell
    const cell2 = await ev(`const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); const e = r.querySelectorAll('.bases-td')[1]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className) };`);
    await c.clickAt(cell2.x, cell2.y, { clickCount: 2 }); await sleep(800);
    p.dblClick = { cell: cell2, active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('table-cell-dblclick', '.workspace-leaf.mod-active .bases-table-container, .suggestion-container') };
    await escapeAll(1);
    await c.clickAt(cell2.x, cell2.y, { button: 'right' }); await sleep(800);
    p.cellRightClick = { surfaces: await surfaces(), shot: await shot('table-cell-context', '.menu') };
    await dismiss();
  }
  // in-place editing of every cell type on the first row: click, read the editor, Escape
  p.cellEdit = {};
  const ncells = await ev(`const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); return r ? r.querySelectorAll('.bases-td').length : 0;`);
  const headers = await ev(`return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header')).map(e => e.textContent.trim());`);
  const before = fs.readFileSync(vp('Scoria.md'), 'utf8');
  for (let i = 0; i < ncells; i++) {
    const cell = await ev(`${READ_LIB} const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); const e = r.querySelectorAll('.bases-td')[${i}]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: txt(e).slice(0, 60), htmlBefore: collapse(e.innerHTML).slice(0, 800), rect: rectOf(e) };`);
    if (!cell) continue;
    await c.clickAt(cell.x, cell.y); await sleep(700);
    const rec = { header: headers[i], cell, afterClick: { active: await activeElement(), cell: await ev(`${READ_LIB} const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); const e = r.querySelectorAll('.bases-td')[${i}]; return { cls: String(e.className), html: collapse(e.innerHTML).slice(0, 1500), editing: Array.from(e.querySelectorAll('input, [contenteditable="true"], select, .multi-select-container, .metadata-input-longtext, .cm-editor')).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className), type: x.getAttribute('type'), value: x.value !== undefined ? x.value : (x.textContent || '').slice(0, 60), placeholder: x.getAttribute('placeholder') })) };`), surfaces: (await surfaces()).map((s) => ({ cls: s.cls, rows: s.rows.filter((r) => r.cls.includes('suggestion-item')).map((r) => r.text).slice(0, 12) })) } };
    if (i < 10) rec.shot = await shot('cell-edit-' + i + '-' + String(headers[i] || '').replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.workspace-leaf.mod-active .bases-table-container, .suggestion-container');
    // Enter then Escape: does Enter commit / open / move down?
    await c.pressKey('Enter'); await sleep(500);
    rec.afterEnter = { active: await activeElement(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), surfaces: (await surfaces()).map((s) => s.cls) };
    if (rec.afterEnter.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); rec.reopened = true; }
    else { await escapeAll(2); await dismiss(); }
    p.cellEdit[i] = rec; save();
  }
  await sleep(1500);
  p.scoriaChanged = fs.readFileSync(vp('Scoria.md'), 'utf8') !== before;
  p.scoriaAfter = p.scoriaChanged ? fs.readFileSync(vp('Scoria.md'), 'utf8') : null;
  // the checkbox cell: a real click on the box
  const box = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-td input[type="checkbox"], .workspace-leaf.mod-active .bases-td .checkbox-container'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), checked: e.checked, tag: e.tagName };`);
  p.checkbox = box;
  if (box) { const b2 = fs.readFileSync(vp('Scoria.md'), 'utf8'); await c.clickAt(box.x, box.y); await sleep(1500); p.checkboxAfter = { checked: await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-td input[type="checkbox"]'); return e ? e.checked : null;`), fileChanged: fs.readFileSync(vp('Scoria.md'), 'utf8') !== b2, fileNow: fs.readFileSync(vp('Scoria.md'), 'utf8') }; await c.clickAt(box.x, box.y); await sleep(1200); }
  // the New button
  const files0 = vaultFiles();
  await dismiss();
  const nb = await c.clickElement('.workspace-leaf.mod-active .bases-toolbar-new-item-menu'); await sleep(2000);
  p.newButton = { rect: nb, surfaces: await surfaces(), created: vaultFiles().filter((n) => !files0.includes(n)), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), shot: await shot('new-button-click') };
  p.newButton.createdBytes = p.newButton.created.map((n) => ({ name: n, text: fs.readFileSync(vp(n), 'utf8') }));
  await escapeAll(1); await dismiss();
  if (p.newButton.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); }
  p.newRowDom = await ev(`${READ_LIB} const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).filter(r => r.querySelector('.bases-td')); const last = r[r.length - 1]; return last ? { cls: String(last.className), text: txt(last).slice(0, 120), editing: Array.from(last.querySelectorAll('input, [contenteditable="true"]')).map(x => String(x.className)) } : null;`);
  // right-click the New button, and the row count text
  p.resultCountText = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-toolbar-result-count'); return e ? e.textContent : null;`);
  return p;
});

// ---- 10: kanban ----------------------------------------------------------
await step('s10-kanban', async (p) => {
  await openBase('Rocks.base'); await switchView('Board'); await dismiss();
  p.dom = await ev(`${READ_LIB}
    const vc = document.querySelector('.workspace-leaf.mod-active .view-content');
    const cols = Array.from(vc.querySelectorAll('[class*="kanban-column"], [class*="bases-kanban-col"]')).filter(e => !e.closest('[class*="kanban-column"]') || e.matches('[class*="kanban-column"]'));
    return { classes: Array.from(new Set(Array.from(vc.querySelectorAll('[class*="bases-"]')).flatMap(e => String(e.className).split(/\\s+/)))).filter(k => k.startsWith('bases-')).sort(),
      columns: cols.slice(0, 8).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 100), rect: rectOf(e), icons: lucides(e), controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).slice(0, 8).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), text: txt(x).slice(0, 30) })) })),
      html: collapse(vc.innerHTML).replace(/>\\s+</g, '><').slice(0, 20000) };`);
  const col = await ev(`const e = document.querySelector('.workspace-leaf.mod-active [class*="kanban-column-header"], .workspace-leaf.mod-active [class*="kanban-header"], .workspace-leaf.mod-active [class*="kanban-column"] > :first-child'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: e.textContent.trim().slice(0, 60) };`);
  p.columnHeader = col;
  if (col) {
    await c.mouseMove(col.x, col.y); await sleep(700);
    p.columnHover = { controls: await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active [class*="kanban-column-header"], .workspace-leaf.mod-active [class*="kanban-header"], .workspace-leaf.mod-active [class*="kanban-column"] > :first-child'); return Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label], svg')).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className && x.className.baseVal !== undefined ? x.className.baseVal : x.className), aria: x.getAttribute('aria-label'), opacity: getComputedStyle(x).opacity, display: getComputedStyle(x).display }));`), shot: await shot('kanban-column-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(col.x, col.y, { button: 'right' }); await sleep(800);
    p.columnRightClick = { surfaces: await surfaces(), shot: await shot('kanban-column-context', '.menu') };
    await dismiss();
    // every clickable icon in the column header
    const icons = await ev(`const e = document.querySelector('.workspace-leaf.mod-active [class*="kanban-column-header"], .workspace-leaf.mod-active [class*="kanban-header"], .workspace-leaf.mod-active [class*="kanban-column"] > :first-child'); return Array.from(e.querySelectorAll('.clickable-icon, button')).map(x => { const b = x.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, aria: x.getAttribute('aria-label'), cls: String(x.className) }; });`);
    p.columnButtons = {};
    for (const ic of icons) {
      await c.mouseMove(ic.x, ic.y); await sleep(900);
      const tip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
      await c.clickAt(ic.x, ic.y); await sleep(900);
      p.columnButtons[ic.aria || ic.cls] = { button: ic, tooltip: tip, surfaces: await surfaces(), created: vaultFiles().filter((n) => /Untitled/.test(n)), active: await activeElement(), shot: await shot('kanban-column-button-' + String(ic.aria || 'x').replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .suggestion-container, .workspace-leaf.mod-active .view-content') };
      await escapeAll(1); await dismiss();
      if ((await ev(`return app.workspace.activeLeaf.view.getViewType();`)) !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
    }
  }
  const card = await ev(`const e = document.querySelector('.workspace-leaf.mod-active [class*="kanban-card"], .workspace-leaf.mod-active [class*="kanban"] [class*="card"]'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: e.textContent.trim().slice(0, 60) };`);
  p.card = card;
  if (card) {
    await c.mouseMove(card.x, card.y); await sleep(700);
    p.cardHover = { dom: await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active [class*="kanban-card"], .workspace-leaf.mod-active [class*="kanban"] [class*="card"]'); return { cls: String(e.className), bg: getComputedStyle(e).backgroundColor, cursor: getComputedStyle(e).cursor, html: collapse(e.outerHTML).slice(0, 3000), controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), opacity: getComputedStyle(x).opacity })) };`), shot: await shot('kanban-card-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(card.x, card.y, { button: 'right' }); await sleep(800);
    p.cardRightClick = { surfaces: await surfaces(), shot: await shot('kanban-card-context', '.menu') };
    await dismiss();
    await c.clickAt(card.x, card.y); await sleep(900);
    p.cardClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls) };
    if (p.cardClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
    // the card's property cells: click one to edit
    const pc = await ev(`const e = document.querySelector('.workspace-leaf.mod-active [class*="kanban-card"] [class*="property"], .workspace-leaf.mod-active [class*="kanban-card"] .bases-td, .workspace-leaf.mod-active [class*="kanban-card"] > :nth-child(2)'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: e.textContent.trim().slice(0, 40) };`);
    if (pc) { await c.clickAt(pc.x, pc.y); await sleep(700); p.cardPropertyClick = { cell: pc, active: await activeElement(), shot: await shot('kanban-card-property-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') }; await escapeAll(1); }
    // drag a card to another column, read the affordance, cancel
    const col2 = await ev(`const cols = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="kanban-column"]')).filter(e => e.offsetWidth > 100); const e = cols[1]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (col2) {
      const d = await c.dragOverOnly(card.x, card.y, col2.x, col2.y);
      p.cardDrag = { started: d.started, mimeTypes: d.data && d.data.items ? d.data.items.map((i) => i.mimeType) : d.error };
      if (d.started) { p.cardDragMid = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], .workspace-leaf.mod-active [class*="placeholder"], body > [class*="drag"]')).map(e => ({ cls: String(e.className), rect: rectOf(e) }));`); p.cardDragShot = await shot('kanban-card-drag', '.workspace-leaf.mod-active .view-content'); await d.finish(); }
    }
  }
  // the background: right-click
  const bg = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .view-content'); const b = e.getBoundingClientRect(); return { x: b.right - 60, y: b.bottom - 60 };`);
  await c.clickAt(bg.x, bg.y, { button: 'right' }); await sleep(700);
  p.backgroundRightClick = { surfaces: await surfaces() };
  await dismiss();
  p.shot = await shot('kanban-view');
  return p;
});

// ---- 11: cards and list ---------------------------------------------------
await step('s11-cards-and-list', async (p) => {
  await openBase('Rocks.base'); await switchView('Cards'); await dismiss();
  p.cardsDom = await ev(`${READ_LIB}
    const vc = document.querySelector('.workspace-leaf.mod-active .view-content');
    const cards = Array.from(vc.querySelectorAll('[class*="bases-cards-item"], [class*="bases-card"]')).filter(e => e.offsetWidth > 100);
    return { classes: Array.from(new Set(Array.from(vc.querySelectorAll('[class*="bases-"]')).flatMap(e => String(e.className).split(/\\s+/)))).filter(k => k.startsWith('bases-')).sort(),
      cards: cards.slice(0, 3).map(e => ({ cls: String(e.className), rect: rectOf(e), text: txt(e).slice(0, 100), html: collapse(e.outerHTML).slice(0, 3000), img: Array.from(e.querySelectorAll('img, [style*="background-image"]')).map(i => ({ tag: i.tagName, cls: String(i.className), src: (i.getAttribute('src') || '').slice(0, 80), style: (i.getAttribute('style') || '').slice(0, 120), rect: rectOf(i), objectFit: getComputedStyle(i).objectFit })) })),
      containerHtml: collapse(vc.innerHTML).replace(/>\\s+</g, '><').slice(0, 15000) };`);
  p.cardsShot = await shot('cards-view');
  const card = await ev(`const e = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="bases-cards-item"], .workspace-leaf.mod-active [class*="bases-card"]')).find(e => e.offsetWidth > 100); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + 20, cx: b.x + b.width / 2, cy: b.y + b.height / 2, bottom: b.bottom - 15, cls: String(e.className) };`);
  p.card = card;
  if (card) {
    await c.mouseMove(card.cx, card.cy); await sleep(700);
    p.cardHover = { dom: await ev(`${READ_LIB} const e = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="bases-cards-item"], .workspace-leaf.mod-active [class*="bases-card"]')).find(e => e.offsetWidth > 100); const cs = getComputedStyle(e); return { cls: String(e.className), bg: cs.backgroundColor, border: cs.borderColor, boxShadow: cs.boxShadow, cursor: cs.cursor, controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), opacity: getComputedStyle(x).opacity })), tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`), shot: await shot('cards-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(card.cx, card.cy, { button: 'right' }); await sleep(800);
    p.cardRightClick = { surfaces: await surfaces(), shot: await shot('cards-context', '.menu') };
    await dismiss();
    // click the image area, then a property row
    await c.clickAt(card.x, card.y); await sleep(900);
    p.cardImageClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls) };
    if (p.cardImageClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Cards'); }
    await escapeAll(1);
    await c.clickAt(card.cx, card.bottom); await sleep(900);
    p.cardPropertyClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('cards-property-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') };
    if (p.cardPropertyClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Cards'); }
    await escapeAll(1);
    // the card title (file name) click
    const title = await ev(`const e = document.querySelector('.workspace-leaf.mod-active [class*="bases-cards-item"] [class*="title"], .workspace-leaf.mod-active [class*="bases-card"] [class*="name"], .workspace-leaf.mod-active [class*="bases-cards"] a, .workspace-leaf.mod-active [class*="bases-cards"] .internal-link'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 8, y: b.y + b.height / 2, cls: String(e.className), text: e.textContent.trim().slice(0, 40) };`);
    p.cardTitle = title;
    if (title) { await c.clickAt(title.x, title.y); await sleep(900); p.cardTitleClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement() }; if (p.cardTitleClick.leafType !== 'bases') { await openBase('Rocks.base'); } }
  }
  await switchView('List'); await dismiss();
  p.listDom = await ev(`${READ_LIB}
    const vc = document.querySelector('.workspace-leaf.mod-active .view-content');
    const items = Array.from(vc.querySelectorAll('[class*="bases-list-item"], [class*="bases-list"] li, [class*="bases-list"] > *')).filter(e => e.offsetHeight > 5);
    return { classes: Array.from(new Set(Array.from(vc.querySelectorAll('[class*="bases-"]')).flatMap(e => String(e.className).split(/\\s+/)))).filter(k => k.startsWith('bases-')).sort(),
      items: items.slice(0, 4).map(e => ({ tag: e.tagName.toLowerCase(), cls: String(e.className), rect: rectOf(e), text: txt(e).slice(0, 120), html: collapse(e.outerHTML).slice(0, 2500) })),
      containerHtml: collapse(vc.innerHTML).replace(/>\\s+</g, '><').slice(0, 12000), containerStyle: (() => { const l = vc.querySelector('[class*="bases-list"]'); if (!l) return null; const cs = getComputedStyle(l); return { fontSize: cs.fontSize, lineHeight: cs.lineHeight, padding: cs.padding, listStyle: cs.listStyleType }; })() };`);
  p.listShot = await shot('list-view');
  const li = await ev(`const e = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="bases-list-item"], .workspace-leaf.mod-active [class*="bases-list"] li')).find(e => e.offsetHeight > 5); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height / 2, cx: b.x + b.width / 2, cls: String(e.className) };`);
  if (li) {
    await c.mouseMove(li.x, li.y); await sleep(600);
    p.listHover = { bg: await ev(`const e = Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="bases-list-item"], .workspace-leaf.mod-active [class*="bases-list"] li')).find(e => e.offsetHeight > 5); return { bg: getComputedStyle(e).backgroundColor, cursor: getComputedStyle(e).cursor };`), shot: await shot('list-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(li.x, li.y, { button: 'right' }); await sleep(800);
    p.listRightClick = { surfaces: await surfaces(), shot: await shot('list-context', '.menu') };
    await dismiss();
    await c.clickAt(li.cx, li.y); await sleep(900);
    p.listClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), shot: await shot('list-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') };
    await escapeAll(1);
    if (p.listClick.leafType !== 'bases') { await openBase('Rocks.base'); }
  }
  return p;
});

// ---- 12: the embedded base --------------------------------------------------
await step('s12-embed', async (p) => {
  await dismiss();
  await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await sleep(800);
  const openMode = async (mode) => {
    await ev(`
      const f = app.vault.getAbstractFileByPath('Embeds.md');
      const leaf = app.workspace.getLeaf('tab'); await leaf.openFile(f, { state: { mode: ${JSON.stringify(mode)}, source: false } });
      app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`);
    await sleep(3500);
    // scroll so all embeds render
    await ev(`const s = document.querySelector('.workspace-leaf.mod-active .markdown-preview-view, .workspace-leaf.mod-active .cm-scroller'); if (s) { s.scrollTop = 0; } return true;`);
    await sleep(800);
  };
  const dumpEmbeds = () => ev(`${READ_LIB}
    const vc = document.querySelector('.workspace-leaf.mod-active .view-content');
    const embeds = Array.from(vc.querySelectorAll('.internal-embed, [class*="bases-embed"]')).filter(e => !e.parentElement.closest('.internal-embed'));
    return { mode: (app.workspace.activeLeaf.view.getMode ? app.workspace.activeLeaf.view.getMode() : null), count: embeds.length,
      embeds: embeds.map(e => ({ cls: String(e.className), src: e.getAttribute('src'), alt: e.getAttribute('alt'), rect: rectOf(e),
        toolbar: e.querySelector('.bases-toolbar') ? { cls: String(e.querySelector('.bases-toolbar').className), items: Array.from(e.querySelectorAll('.bases-toolbar-item')).map(i => ({ cls: String(i.className), text: txt(i).slice(0, 40), icon: lucide(i), hidden: i.offsetWidth === 0 })) } : null,
        header: Array.from(e.querySelectorAll('[class*="embed-title"], [class*="embed-header"], [class*="bases-embed-"]')).slice(0, 10).map(h => ({ cls: String(h.className), text: txt(h).slice(0, 60), icon: lucide(h), rect: rectOf(h), opacity: getComputedStyle(h).opacity })),
        controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).filter(x => !x.closest('.bases-toolbar-item')).slice(0, 20).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), text: txt(x).slice(0, 30), rect: rectOf(x), opacity: getComputedStyle(x).opacity, display: getComputedStyle(x).display })),
        viewName: txt(e.querySelector('.bases-toolbar-views-menu')) || null,
        layoutClasses: Array.from(new Set(Array.from(e.querySelectorAll('[class*="bases-"]')).flatMap(x => String(x.className).split(/\\s+/)))).filter(k => k.startsWith('bases-')).sort().slice(0, 60),
        html: collapse(e.outerHTML).replace(/>\\s+</g, '><').slice(0, 8000) })) };`);
  p.reading = {};
  await openMode('preview');
  p.reading.dump = await dumpEmbeds();
  p.reading.shot = await shot('embed-reading');
  const e0 = await c.rect('.workspace-leaf.mod-active .internal-embed', 0);
  if (e0) {
    await c.mouseMove(e0.cx, e0.y + 20); await sleep(900);
    p.reading.hover = { dump: await dumpEmbeds(), tooltip: await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`), shot: await shot('embed-reading-hover', '.workspace-leaf.mod-active .internal-embed') };
    // each control in the embed's header: hover for tooltip, click, read
    const ctrls = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .internal-embed'); return Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).filter(x => !x.closest('.bases-toolbar-item') && x.offsetWidth > 0).slice(0, 8).map(x => { const b = x.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, aria: x.getAttribute('aria-label'), cls: String(x.className) }; });`);
    p.reading.controls = {};
    for (const ct of ctrls) {
      await c.mouseMove(ct.x, ct.y); await sleep(900);
      const tip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
      await c.clickAt(ct.x, ct.y); await sleep(1200);
      p.reading.controls[ct.aria || ct.cls] = { control: ct, tooltip: tip, surfaces: await surfaces(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), leaves: await ev(`return app.workspace.getLeavesOfType('bases').length;`), shot: await shot('embed-reading-control-' + String(ct.aria || 'x').replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .suggestion-container') };
      await dismiss(); await escapeAll(1);
      if (p.reading.controls[ct.aria || ct.cls].leafType !== 'markdown' || p.reading.controls[ct.aria || ct.cls].leaves) { await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await openMode('preview'); }
      save();
    }
    const e1 = await c.rect('.workspace-leaf.mod-active .internal-embed', 0);
    if (e1) { await c.clickAt(e1.cx, e1.cy, { button: 'right' }); await sleep(800); p.reading.rightClick = { surfaces: await surfaces(), shot: await shot('embed-reading-context', '.menu') }; await dismiss(); }
    // the embedded toolbar's views control, if shown
    const vm = await c.rect('.workspace-leaf.mod-active .internal-embed .bases-toolbar-views-menu', 0);
    if (vm && vm.w) { await c.clickAt(vm.cx, vm.cy); await sleep(900); p.reading.viewsPanel = { surfaces: await surfaces(), shot: await shot('embed-reading-views-panel', '.menu') }; await dismiss(); }
    // the row context menu inside an embed
    const td = await c.rect('.workspace-leaf.mod-active .internal-embed .bases-td', 0);
    if (td) { await c.clickAt(td.cx, td.cy, { button: 'right' }); await sleep(800); p.reading.rowMenu = { surfaces: await surfaces() }; await dismiss(); }
  }
  await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await sleep(800);
  p.livePreview = {};
  await openMode('source');
  p.livePreview.dump = await dumpEmbeds();
  p.livePreview.shot = await shot('embed-live-preview');
  const l0 = await c.rect('.workspace-leaf.mod-active .internal-embed', 0);
  if (l0) {
    await c.mouseMove(l0.cx, l0.y + 20); await sleep(900);
    p.livePreview.hover = { dump: await dumpEmbeds(), shot: await shot('embed-lp-hover', '.workspace-leaf.mod-active .internal-embed') };
    await c.clickAt(l0.cx, l0.y + 20); await sleep(900);
    p.livePreview.click = { active: await activeElement(), caretLine: await ev(`try { return app.workspace.activeLeaf.view.editor.getCursor(); } catch (e) { return null; }`), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('embed-lp-click', '.workspace-leaf.mod-active .internal-embed') };
    await c.clickAt(l0.cx, l0.cy, { button: 'right' }); await sleep(800);
    p.livePreview.rightClick = { surfaces: await surfaces(), shot: await shot('embed-lp-context', '.menu') };
    await dismiss();
    const ctrls = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .internal-embed'); return Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).filter(x => !x.closest('.bases-toolbar-item') && x.offsetWidth > 0).slice(0, 8).map(x => { const b = x.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, aria: x.getAttribute('aria-label'), cls: String(x.className) }; });`);
    p.livePreview.controls = {};
    for (const ct of ctrls) {
      await c.mouseMove(ct.x, ct.y); await sleep(900);
      const tip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
      await c.clickAt(ct.x, ct.y); await sleep(1200);
      p.livePreview.controls[ct.aria || ct.cls] = { control: ct, tooltip: tip, surfaces: await surfaces(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), leaves: await ev(`return app.workspace.getLeavesOfType('bases').length;`), shot: await shot('embed-lp-control-' + String(ct.aria || 'x').replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .suggestion-container') };
      await dismiss(); await escapeAll(1);
      if (p.livePreview.controls[ct.aria || ct.cls].leafType !== 'markdown' || p.livePreview.controls[ct.aria || ct.cls].leaves) { await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await openMode('source'); }
      save();
    }
  }
  p.embedsFileAfter = fs.readFileSync(vp('Embeds.md'), 'utf8');
  p.rocksFileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await ev(`app.workspace.detachLeavesOfType('markdown'); return true;`);
  return p;
});

// ---- 13: keyboard in the table and the panels --------------------------------
await step('s13-keyboard', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  const cell = await ev(`const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); const e = r.querySelectorAll('.bases-td')[1]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.clickAt(cell.x, cell.y); await sleep(500);
  const state = () => ev(`${READ_LIB} const a = document.activeElement; const sel = document.querySelector('.workspace-leaf.mod-active .bases-td.is-selected, .workspace-leaf.mod-active .bases-td.is-active, .workspace-leaf.mod-active .bases-td.mod-selected, .workspace-leaf.mod-active [class*="bases-td"][class*="select"], .workspace-leaf.mod-active [class*="bases-td"][class*="focus"]');
    const cells = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-td')); const idx = cells.findIndex(c => c.contains(a) || c === a);
    return { active: a ? { tag: a.tagName.toLowerCase(), cls: String(a.className).slice(0, 120), editable: a.isContentEditable, text: txt(a).slice(0, 40) } : null, selectedCell: sel ? { cls: String(sel.className), text: txt(sel).slice(0, 40) } : null, activeCellIndex: idx, surfaces: Array.from(document.querySelectorAll(SURF)).map(e => String(e.className)), leafType: app.workspace.activeLeaf.view.getViewType() };`);
  p.afterClick = await state();
  p.keys = [];
  for (const [k, mods] of [['Escape'], ['Tab'], ['Tab'], ['Tab', { shift: true }], ['ArrowRight'], ['ArrowDown'], ['ArrowLeft'], ['ArrowUp'], ['Enter'], ['Escape'], ['Escape']]) {
    await c.pressKey(k, mods || {}); await sleep(450);
    const s = await state();
    p.keys.push({ key: k, mods: mods || null, ...s });
    if (s.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); await c.clickAt(cell.x, cell.y); await sleep(500); p.keys[p.keys.length - 1].reopened = true; }
  }
  p.keysShot = await shot('table-keyboard', '.workspace-leaf.mod-active .bases-table-container');
  // Escape and Enter in each panel
  p.panels = {};
  for (const cls of ['bases-toolbar-views-menu', 'bases-toolbar-properties-menu', 'bases-toolbar-sort-menu', 'bases-toolbar-filter-menu', 'bases-toolbar-results-menu', 'bases-toolbar-group-menu']) {
    await dismiss();
    await openPanel(cls);
    const rec = { opened: (await surfaces()).map((s) => s.cls), active: await activeElement() };
    await c.pressKey('ArrowDown'); await sleep(300);
    rec.afterArrowDown = { active: await activeElement(), selected: await ev(`const e = document.querySelector('.menu .is-selected, .menu .mod-active'); return e ? { cls: e.className, text: e.textContent.trim().slice(0, 40) } : null;`) };
    await c.pressKey('Enter'); await sleep(700);
    rec.afterEnter = { surfaces: (await surfaces()).map((s) => s.cls), active: await activeElement(), viewName: await ev(`try { return app.workspace.activeLeaf.view.controller.viewName; } catch (e) { return null; }`) };
    if (!rec.afterEnter.surfaces.length) { await openPanel(cls); }
    await c.pressKey('Escape'); await sleep(400);
    rec.afterEscape = { surfaces: (await surfaces()).map((s) => s.cls), active: await activeElement() };
    await dismiss();
    p.panels[cls] = rec;
    save();
  }
  await switchView('Table');
  return p;
});

// ---- 14: computed styles --------------------------------------------------
await step('s14-styles', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  p.theme = await ev(`return { body: document.body.className, theme: app.vault.getConfig('theme'), cssTheme: app.vault.getConfig('cssTheme'), baseFontSize: app.vault.getConfig('baseFontSize'), zoom: window.devicePixelRatio, inner: { w: window.innerWidth, h: window.innerHeight } };`);
  const L = '.workspace-leaf.mod-active ';
  p.table = await styles([L + '.view-header', L + '.view-header-title-container', L + '.view-header-title', L + '.view-header-icon', L + '.view-actions', L + '.view-action', L + '.view-content',
    L + '.bases-toolbar', L + '.bases-toolbar-item', L + '.bases-toolbar-item .text-icon-button', L + '.bases-toolbar-views-menu .text-icon-button', L + '.bases-toolbar-views-menu .text-button-label', L + '.bases-toolbar-views-menu .text-button-icon', L + '.bases-toolbar-views-menu .text-button-icon svg', L + '.bases-toolbar-result-count', L + '.bases-toolbar-sort-menu .text-icon-button', L + '.bases-toolbar-filter-menu .text-icon-button', L + '.bases-toolbar-new-item-menu .text-icon-button', L + '.bases-toolbar-new-item-menu', L + '.bases-toolbar-search', L + '.bases-toolbar .text-icon-button .flair, ' + L + '.bases-toolbar .text-button-count, ' + L + '.bases-toolbar [class*="count"]',
    L + '.bases-view', L + '.bases-table-container', L + '.bases-table', L + '.bases-thead', L + '.bases-tr', L + '.bases-table-header', L + '.bases-table-header-name', L + '.bases-table-header .bases-table-header-icon, ' + L + '.bases-table-header svg', L + '.bases-table-header-resizer', L + '.bases-tbody .bases-tr, ' + L + '.bases-tr:not(.bases-thead .bases-tr)', L + '.bases-td', L + '.bases-td .bases-table-cell-text, ' + L + '.bases-td > *', L + '.bases-td a, ' + L + '.bases-td .internal-link', L + '[class*="group-header"]', L + '[class*="summary"]', L + '.bases-tr [class*="add"], ' + L + '.bases-table-container [class*="add"]']);
  // hovered states: the toolbar button and a row
  const vb = await c.rect(L + '.bases-toolbar-views-menu .text-icon-button');
  if (vb) { await c.mouseMove(vb.cx, vb.cy); await sleep(500); p.toolbarButtonHover = await styles([L + '.bases-toolbar-views-menu .text-icon-button']); }
  const row = await ev(`const r = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tr')).find(r => r.querySelector('.bases-td')); const b = r.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.mouseMove(row.x, row.y); await sleep(500);
  p.rowHover = await styles([L + '.bases-tr:hover', L + '.bases-tr:hover .bases-td']);
  await c.mouseMove(5, 5);
  // the open views panel: open and read
  await openPanel('bases-toolbar-views-menu');
  p.viewsPanel = await styles(['.menu.bases-toolbar-menu', '.menu.bases-toolbar-menu .menu-scroll', '.menu .bases-toolbar-menu-container', '.menu .search-input-container', '.menu .search-input-container input', '.menu .bases-toolbar-items', '.menu .suggestion-group', '.menu .bases-toolbar-menu-item', '.menu .bases-toolbar-menu-item.is-selected', '.menu .bases-toolbar-menu-item-info', '.menu .bases-toolbar-menu-item-info-icon', '.menu .bases-toolbar-menu-item-info-icon svg', '.menu .bases-toolbar-menu-item-name', '.menu .bases-toolbar-menu-item-icon', '.menu .bases-toolbar-menu-item-icon svg']);
  p.viewsPanelWhole = await ev(`const m = document.querySelector('.menu'); if (!m) return null; const r = m.getBoundingClientRect(); const cs = getComputedStyle(m); return { x: r.x, y: r.y, w: r.width, h: r.height, padding: cs.padding, border: cs.border, radius: cs.borderRadius, shadow: cs.boxShadow, bg: cs.backgroundColor, zIndex: cs.zIndex, anchor: (() => { const b = document.querySelector('.workspace-leaf.mod-active .bases-toolbar-views-menu').getBoundingClientRect(); return { bx: b.x, by: b.y, bw: b.width, bh: b.height, dx: r.x - b.x, dy: r.y - b.bottom }; })() };`);
  await dismiss();
  await openPanel('bases-toolbar-filter-menu');
  for (let i = 0; i < 4; i++) { const h = await ev(`const e = Array.from(document.querySelectorAll('.menu .bases-toolbar-section-header.is-collapsed'))[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 12, y: b.y + b.height / 2 };`); if (!h) break; await c.clickAt(h.x, h.y); await sleep(400); }
  p.filterPanel = await styles(['.menu.bases-toolbar-filter-menu', '.menu .bases-toolbar-section', '.menu .bases-toolbar-section-header', '.menu .bases-toolbar-section-header .flair', '.menu .bases-toolbar-section-content', '.menu .bases-query-container', '.menu .filter-group', '.menu .filter-group-header', '.menu .filter-group-header select', '.menu .filter-group-statements', '.menu .filter-row', '.menu .filter-row .conjunction', '.menu .filter-statement', '.menu .filter-expression', '.menu .combobox-button', '.menu .combobox-button-label', '.menu .combobox-button-icon svg', '.menu .combobox-button-chevron', '.menu .filter-rhs-container', '.menu .filter-rhs-container .metadata-input-longtext', '.menu .filter-row-actions', '.menu .filter-row-actions button', '.menu .bases-toolbar-menu-action', '.menu .bases-toolbar-menu-actions', '.menu .filter-group .filter-group']);
  await dismiss();
  await openPanel('bases-toolbar-sort-menu');
  p.sortPanel = await styles(['.menu.bases-toolbar-sort-menu', '.menu .bases-sort-container', '.menu .base-toolbar-sort-item', '.menu .grip-handle', '.menu .grip-handle svg', '.menu .bases-sort-property', '.menu .bases-sort-direction', '.menu .base-toolbar-sort-item .clickable-icon', '.menu .bases-toolbar-menu-actions .bases-toolbar-menu-action']);
  await dismiss();
  await openPanel('bases-toolbar-properties-menu');
  p.propertiesPanel = await styles(['.menu.bases-toolbar-properties-menu', '.menu .bases-properties-page', '.menu .bases-toolbar-menu-item', '.menu .bases-toolbar-menu-item input[type="checkbox"]', '.menu .bases-toolbar-menu-item.mod-hidden', '.menu .bases-toolbar-menu-item.mod-formula .bases-toolbar-menu-item-name', '.menu .bases-toolbar-menu-item-icon']);
  await dismiss();
  await openViewConfig('Table');
  p.configPage = await styles(['.menu .bases-toolbar-menu-container-header', '.menu .back-button', '.menu .back-label', '.menu .view-config-menu', '.menu .input-row', '.menu .input-row-label', '.menu .input-row-content', '.menu .input-row input[type="text"]', '.menu .input-row select', '.menu .input-row .combobox-button']);
  await dismiss();
  // a context menu's rows
  const h0 = await c.rect(L + '.bases-table-header', 1);
  if (h0) { await c.clickAt(h0.cx, h0.cy, { button: 'right' }); await sleep(700); p.contextMenu = await styles(['.menu:not(.bases-toolbar-menu)', '.menu:not(.bases-toolbar-menu) .menu-item', '.menu:not(.bases-toolbar-menu) .menu-item-icon', '.menu:not(.bases-toolbar-menu) .menu-item-icon svg', '.menu:not(.bases-toolbar-menu) .menu-item-title', '.menu:not(.bases-toolbar-menu) .menu-separator']); await dismiss(); }
  // the other layouts' key boxes
  for (const [name, key, sels] of [['Cards', 'cards', [L + '.bases-cards-container, ' + L + '[class*="bases-cards"]', L + '[class*="bases-cards-item"]', L + '[class*="bases-cards-item"] img, ' + L + '[class*="bases-card"] img', L + '[class*="bases-cards-item"] [class*="title"], ' + L + '[class*="bases-cards"] [class*="name"]', L + '[class*="bases-cards-item"] [class*="property"]']],
    ['List', 'list', [L + '[class*="bases-list"]', L + '[class*="bases-list-item"], ' + L + '[class*="bases-list"] li', L + '[class*="bases-list"] a']],
    ['Board', 'kanban', [L + '[class*="bases-kanban"]', L + '[class*="kanban-column"]', L + '[class*="kanban-column-header"], ' + L + '[class*="kanban-header"]', L + '[class*="kanban-card"]', L + '[class*="kanban-card"] [class*="title"], ' + L + '[class*="kanban-card"] [class*="name"]']]]) {
    await switchView(name); await sleep(400);
    p[key] = await styles(sels);
  }
  await switchView('Table');
  // the CSS custom properties the Bases styles reference
  p.cssVars = await ev(`const cs = getComputedStyle(document.body); const names = ['--bases-toolbar-height', '--bases-table-row-height', '--bases-table-header-height', '--bases-cards-size', '--bases-table-cell-padding', '--bases-cards-image-height', '--bases-kanban-column-width', '--table-header-size', '--background-primary', '--background-secondary', '--background-modifier-hover', '--background-modifier-border', '--text-normal', '--text-muted', '--text-faint', '--text-accent', '--interactive-accent', '--font-ui-small', '--font-ui-smaller', '--font-ui-medium', '--size-4-1', '--size-4-2', '--size-4-3', '--header-height', '--icon-s', '--icon-xs', '--icon-m', '--radius-s', '--radius-m', '--input-height', '--menu-item-height'];
    const o = {}; for (const n of names) { const v = cs.getPropertyValue(n).trim(); if (v) o[n] = v; }
    // every --bases-* variable the stylesheets define
    const found = new Set(); for (const ss of Array.from(document.styleSheets)) { let rules; try { rules = ss.cssRules; } catch { continue; } for (const r of Array.from(rules)) { if (r.style) for (const k of Array.from(r.style)) if (k.startsWith('--bases') || k.startsWith('--table')) found.add(k); } }
    for (const n of found) { const v = cs.getPropertyValue(n).trim(); o[n] = v || '(unset on body)'; }
    return o;`);
  return p;
});


// ---- fix-ups: the first pass aimed some clicks at the header row (it is a
// `.bases-tr` of `.bases-td.mod-implicit` cells), at the hidden measuring
// template ("Fj") of the cards and kanban layouts, and at a zero-rect copy
// of the reading-view embed; and a click right after a DOM-removed menu
// toggled Obsidian's menu state instead of opening. These steps redo those
// captures on the right targets.
const DATA_ROW = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-tbody .bases-tr, .workspace-leaf.mod-active .bases-tr')).filter(r => r.querySelector('.bases-table-cell') && !r.classList.contains('bases-table-group-summary-row') && !r.classList.contains('bases-table-footer'))`;
const closePopovers = async () => { await escapeAll(1); await ev(`document.querySelectorAll('.popover.bases-new-item-popover, .hover-popover').forEach(e => e.remove()); return true;`); await sleep(300); };

await step('s15-fix-views', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  // the base tab's own context menu (the root split's active tab, not the sidebar's)
  const tab = await c.rect('.workspace-split.mod-root .workspace-tab-header.is-active');
  if (tab) { await c.clickAt(tab.cx, tab.cy, { button: 'right' }); await sleep(800); p.tabMenu = { tab: await ev(`${READ_LIB} const t = document.querySelector('.workspace-split.mod-root .workspace-tab-header.is-active'); return { cls: String(t.className), title: txt(t.querySelector('.workspace-tab-header-inner-title')), icon: lucide(t.querySelector('.workspace-tab-header-inner-icon')), aria: t.getAttribute('aria-label') };`), surfaces: await surfaces(), shot: await shot('base-tab-header-menu', '.menu') }; await dismiss(); }
  // Add view: the page, the Layout combobox's four options, Escape
  p.opened = await openPanel('bases-toolbar-views-menu');
  p.addViewClick = await clickText('.menu .bases-toolbar-menu-item', 'Add view');
  await sleep(800);
  p.addViewPage = await surfaces();
  p.addViewShot = await shot('add-view-page', '.menu');
  p.addViewActive = await activeElement();
  p.addViewLayoutOptions = await comboOptions('.menu .view-config-menu .combobox-button');
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  p.afterEscapeInCombo = (await surfaces()).map((x) => x.cls);
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await escapeAll(1); await dismiss(); await sleep(1500);
  p.addViewWroteFile = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
  p.viewsAfter = await ev(`return app.workspace.activeLeaf.view.query.views.map(v => v.name);`);
  if (p.addViewWroteFile) { p.fileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8'); fs.writeFileSync(vp('Rocks.base'), before); await sleep(2000); }
  // right-click on a views-panel row: does it switch, open a menu, or nothing
  await openPanel('bases-toolbar-views-menu');
  const row = await c.rect('.menu .bases-toolbar-menu-item', 2);
  if (row) { await c.clickAt(row.cx, row.cy, { button: 'right' }); await sleep(800); p.rightClickRow = { surfaces: await surfaces(), viewName: await ev(`return app.workspace.activeLeaf.view.controller.viewName;`) }; }
  await dismiss();
  // the gear's hover tooltip
  await openPanel('bases-toolbar-views-menu');
  const gear = await c.rect('.menu .bases-toolbar-menu-item .bases-toolbar-menu-item-icon', 0);
  if (gear) { await c.mouseMove(gear.cx, gear.cy); await sleep(1100); p.gearTooltip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`); p.gearHoverShot = await shot('views-panel-gear-hover', '.menu'); }
  await dismiss();
  return p;
});

await step('s16-fix-formula', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  p.opened = await openPanel('bases-toolbar-properties-menu');
  const fgear = await ev(`const r = document.querySelector('.menu .bases-toolbar-menu-item.mod-formula'); const ic = r && r.querySelector('.bases-toolbar-menu-item-icon'); if (!ic) return null; const b = ic.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, name: (r.querySelector('.bases-toolbar-menu-item-name') || {}).textContent, icon: String((ic.querySelector('svg') || {}).getAttribute ? ic.querySelector('svg').getAttribute('class') : ''), aria: ic.getAttribute('aria-label') };`);
  p.formulaGear = fgear;
  if (fgear) {
    await c.clickAt(fgear.x, fgear.y); await sleep(1000);
    p.formulaPage = await surfaces();
    p.formulaPageShot = await shot('properties-formula-page', '.menu, .modal-container, .suggestion-container');
    p.formulaActive = await activeElement();
    const ed = await ev(`const e = document.querySelector('.menu .formula-editor .cm-content, .menu .cm-content, .modal-container .cm-content, .menu textarea, .modal-container textarea'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width - 8, y: b.y + b.height / 2, cls: String(e.className), text: e.textContent };`);
    p.formulaEditor = ed;
    if (ed) {
      await c.clickAt(ed.x, ed.y); await sleep(300); await c.pressKey('End');
      await c.typeText(' + (', 40); await sleep(900);
      p.formulaErrorState = await surfaces();
      p.formulaErrorShot = await shot('properties-formula-error', '.menu, .modal-container, .suggestion-container');
      await c.typeText('d', 40); await sleep(900);
      p.formulaSuggestions = await surfaces();
      p.formulaSuggestShot = await shot('properties-formula-suggest', '.menu, .modal-container, .suggestion-container');
      await escapeAll(1);
      for (let i = 0; i < 5; i++) await c.pressKey('Backspace');
      await sleep(600);
      p.formulaAfterUndo = await surfaces();
      // the page's own header menu (more-vertical), if any
      const mv = await ev(`const h = document.querySelector('.menu .bases-toolbar-menu-container-header'); if (!h) return null; const b = Array.from(h.querySelectorAll('.clickable-icon')).filter(e => !e.classList.contains('close-icon'))[0]; if (!b) return null; const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, icon: String((b.querySelector('svg') || {}).getAttribute ? b.querySelector('svg').getAttribute('class') : '') };`);
      if (mv) { await c.clickAt(mv.x, mv.y); await sleep(700); p.formulaHeaderMenu = { surfaces: await surfaces(), shot: await shot('properties-formula-header-menu', '.menu') }; await ev(`document.querySelectorAll('.menu').forEach(m => { if (!m.classList.contains('bases-toolbar-menu')) m.remove(); }); return true;`); }
    }
    await escapeAll(2); await dismiss(); await closeModal();
  }
  p.fileAfterFormula = fs.readFileSync(vp('Rocks.base'), 'utf8');
  // "Add formula" from the panel's action rows
  await openPanel('bases-toolbar-properties-menu');
  p.addFormulaClick = await clickText('.menu .bases-toolbar-menu-item', 'Add formula');
  await sleep(900);
  p.addFormulaPage = await surfaces();
  p.addFormulaShot = await shot('properties-add-formula', '.menu, .modal-container, .suggestion-container');
  p.addFormulaActive = await activeElement();
  await escapeAll(2); await dismiss(); await closeModal(); await sleep(1200);
  p.fileAfterAddFormula = fs.readFileSync(vp('Rocks.base'), 'utf8');
  // "Hide all"
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await openPanel('bases-toolbar-properties-menu');
  p.hideAllClick = await clickText('.menu .bases-toolbar-menu-item', 'Hide all');
  await sleep(900);
  p.afterHideAll = { panel: await surfaces(), toolbar: (await toolbarDump()).toolbar.itemsDeep.map((i) => i.text), shot: await shot('properties-after-hide-all', '.menu') };
  await dismiss(); await sleep(1500);
  p.hideAllFile = fs.readFileSync(vp('Rocks.base'), 'utf8');
  fs.writeFileSync(vp('Rocks.base'), before); await sleep(2500);
  // the "Add "zzz" property" row: what it writes
  await openPanel('bases-toolbar-properties-menu');
  await c.clickElement('.menu .search-input-container input'); await sleep(200);
  await c.typeText('zzznew', 40); await sleep(600);
  p.addPropClick = await clickText('.menu .bases-toolbar-menu-item', 'Add "zzznew" property');
  await sleep(1000);
  p.afterAddProp = { panel: await surfaces(), headers: await ev(`return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-table-header')).map(e => e.textContent.trim());`) };
  await dismiss(); await sleep(1500);
  p.addPropFile = fs.readFileSync(vp('Rocks.base'), 'utf8');
  fs.writeFileSync(vp('Rocks.base'), before); await sleep(2500);
  return p;
});

await step('s17-fix-filters', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  await openPanel('bases-toolbar-filter-menu');
  for (let i = 0; i < 4; i++) { const h = await ev(`const e = Array.from(document.querySelectorAll('.menu .bases-toolbar-section-header.is-collapsed'))[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 12, y: b.y + b.height / 2 };`); if (!h) break; await c.clickAt(h.x, h.y); await sleep(400); }
  p.actionButtons = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.menu .text-icon-button')).map(e => ({ cls: String(e.className), text: txt(e), icon: lucide(e), rect: rectOf(e), parentCls: String(e.parentElement.className), section: (e.closest('.bases-toolbar-section') || {}).querySelector ? txt(e.closest('.bases-toolbar-section').querySelector('.bases-toolbar-section-header')) : null }));`);
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  p.addGroupClick = await clickText('.menu .text-icon-button', 'Add filter group', { index: 1 });
  await sleep(900);
  p.afterAddGroup = await surfaces(); p.afterAddGroupShot = await shot('filters-after-add-group', '.menu, .suggestion-container');
  p.afterAddGroupActive = await activeElement();
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  p.addFilterClick = await clickText('.menu .text-icon-button', 'Add filter', { index: 1 });
  await sleep(900);
  p.afterAddFilter = await surfaces(); p.afterAddFilterShot = await shot('filters-after-add-filter', '.menu, .suggestion-container');
  p.afterAddFilterActive = await activeElement();
  await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  // the new empty row's operator list (no property chosen) and its Remove button
  p.emptyRow = await ev(`${READ_LIB} const rows = Array.from(document.querySelectorAll('.menu .filter-row')); const r = rows[rows.length - 1]; return r ? { html: collapse(r.outerHTML).slice(0, 3000), rows: Array.from(r.querySelectorAll('*')).filter(e => e.matches('.combobox-button, button, input, [contenteditable]')).map(e => readRow(r, e)) } : null;`);
  p.groupHeaderActions = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.menu .filter-group-header')).map(h => ({ cls: String(h.className), select: (h.querySelector('select') || {}).value, nested: !!h.parentElement.parentElement.closest('.filter-group'), actions: Array.from(h.querySelectorAll('.filter-group-header-actions *')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), text: txt(e) })) }));`);
  // hover the nested group header for its actions
  const gh = await c.rect('.menu .filter-group .filter-group .filter-group-header', 0);
  if (gh) { await c.mouseMove(gh.cx, gh.cy); await sleep(600); p.nestedGroupHover = await ev(`${READ_LIB} const h = document.querySelector('.menu .filter-group .filter-group .filter-group-header'); return { actions: Array.from(h.querySelectorAll('.filter-group-header-actions *')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), opacity: getComputedStyle(e).opacity })), html: collapse(h.outerHTML).slice(0, 2000) };`); p.nestedGroupShot = await shot('filters-nested-group-hover', '.menu'); }
  // Remove the rows again
  for (let i = 0; i < 3; i++) { const rm = await ev(`const rows = Array.from(document.querySelectorAll('.menu .filter-row')); const r = rows[rows.length - 1]; const b = r && r.querySelector('[aria-label="Remove filter"]'); if (!b || rows.length <= 4) return null; const x = b.getBoundingClientRect(); return { x: x.x + x.width / 2, y: x.y + x.height / 2 };`); if (!rm) break; await c.clickAt(rm.x, rm.y); await sleep(500); }
  p.groupRemove = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.menu [aria-label*="Remove"], .menu [aria-label*="Delete"]')).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e) }));`);
  await dismiss(); await sleep(1500);
  p.fileChanged = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
  p.fileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  if (p.fileChanged) { fs.writeFileSync(vp('Rocks.base'), before); await sleep(2000); }
  // the value editors per type: click the rhs of each row and read what opens
  await openPanel('bases-toolbar-filter-menu');
  for (let i = 0; i < 4; i++) { const h = await ev(`const e = Array.from(document.querySelectorAll('.menu .bases-toolbar-section-header.is-collapsed'))[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 12, y: b.y + b.height / 2 };`); if (!h) break; await c.clickAt(h.x, h.y); await sleep(400); }
  p.values = {};
  const n = await ev(`return document.querySelectorAll('.menu .filter-row').length;`);
  for (let i = 0; i < n; i++) {
    const r = await ev(`${READ_LIB} const row = document.querySelectorAll('.menu .filter-row')[${i}]; const v = row.querySelector('.filter-rhs-container'); if (!v) return null; const b = v.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, property: txt(row.querySelector('.filter-property-select .combobox-button-label')), html: collapse(v.outerHTML).slice(0, 2000) };`);
    if (!r) continue;
    await c.clickAt(r.x, r.y); await sleep(700);
    p.values[r.property] = { rhs: r.html, active: await activeElement(), surfaces: (await surfaces()).filter((x) => !/bases-toolbar-menu/.test(x.cls)), shot: await shot('filters-value-' + r.property.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .suggestion-container') };
    await escapeAll(1); await ev(`document.querySelectorAll('.suggestion-container').forEach(e => e.remove()); return true;`);
  }
  await dismiss(); await sleep(1000);
  if (fs.readFileSync(vp('Rocks.base'), 'utf8') !== before) { p.valuesWrote = fs.readFileSync(vp('Rocks.base'), 'utf8'); fs.writeFileSync(vp('Rocks.base'), before); await sleep(2000); }
  return p;
});

await step('s18-fix-header', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  p.thead = await ev(`${READ_LIB} const h = document.querySelector('.workspace-leaf.mod-active .bases-thead'); return h ? { cls: String(h.className), rect: rectOf(h), html: collapse(h.outerHTML).replace(/>\\s+</g, '><').slice(0, 12000), cells: Array.from(h.querySelectorAll('.bases-td')).map(e => ({ cls: String(e.className), text: txt(e), icons: lucides(e), rect: rectOf(e), style: e.getAttribute('style') })) } : null;`);
  p.tableEnd = await ev(`${READ_LIB} const t = document.querySelector('.workspace-leaf.mod-active .bases-table-container'); const last = t.querySelector('.bases-thead .bases-tr'); return { scrollWidth: t.scrollWidth, clientWidth: t.clientWidth, rowWidth: last ? last.style.width : null, afterHeader: Array.from(t.querySelectorAll('.bases-thead ~ *, .bases-table > :not(.bases-thead):not(.bases-tbody)')).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 40) })), footer: (() => { const f = t.querySelector('.bases-table-footer'); return f ? { cls: String(f.className), text: txt(f).slice(0, 80), html: collapse(f.outerHTML).slice(0, 2000) } : null; })() };`);
  // hover the header cell: the sort chevron and the resizer (siblings inside .bases-td)
  const h1 = await c.rect('.workspace-leaf.mod-active .bases-thead .bases-td', 2);
  if (h1) {
    await c.mouseMove(h1.cx, h1.cy); await sleep(700);
    p.hoverHeader = await ev(`${READ_LIB} const td = document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')[2]; const cs = getComputedStyle(td); const sort = td.querySelector('.bases-table-header-sort'); const rz = td.querySelector('.bases-table-header-resizer'); return { cls: String(td.className), bg: cs.backgroundColor, cursor: cs.cursor, sort: sort ? { icon: lucide(sort), rect: rectOf(sort), opacity: getComputedStyle(sort).opacity, display: getComputedStyle(sort).display } : null, resizer: rz ? { cls: String(rz.className), draggable: rz.getAttribute('draggable'), rect: rectOf(rz), cursor: getComputedStyle(rz).cursor, opacity: getComputedStyle(rz).opacity, bg: getComputedStyle(rz).backgroundColor, width: getComputedStyle(rz).width } : null, html: collapse(td.outerHTML).slice(0, 1500), tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`);
    p.hoverHeaderShot = await shot('table-header-hover', '.workspace-leaf.mod-active .bases-thead');
    const rz = await c.rect('.workspace-leaf.mod-active .bases-thead .bases-td .bases-table-header-resizer', 2);
    if (rz) { await c.mouseMove(rz.cx, rz.cy); await sleep(600); p.hoverResizer = await ev(`const rz = document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td .bases-table-header-resizer')[2]; const cs = getComputedStyle(rz); return { cursor: cs.cursor, opacity: cs.opacity, bg: cs.backgroundColor, width: cs.width, cls: rz.className };`); p.hoverResizerShot = await shot('table-header-resizer-hover', '.workspace-leaf.mod-active .bases-thead'); }
    // a left click on the header cell
    await c.clickAt(h1.cx, h1.cy); await sleep(800);
    p.leftClick = { surfaces: await surfaces(), active: await activeElement(), shot: await shot('table-header-click', '.menu, .workspace-leaf.mod-active .bases-thead') };
    await dismiss();
    // a click on the sort chevron of the sorted column
    const sc = await c.rect('.workspace-leaf.mod-active .bases-thead .bases-table-header-sort', 0);
    if (sc && sc.w) { await c.clickAt(sc.cx, sc.cy); await sleep(800); p.sortChevronClick = { surfaces: await surfaces(), shot: await shot('table-header-sort-chevron-click', '.menu, .workspace-leaf.mod-active .bases-thead') }; await dismiss(); }
  }
  // scroll right to the formula column's header, right-click it
  await ev(`const t = document.querySelector('.workspace-leaf.mod-active .bases-table-container'); t.scrollLeft = t.scrollWidth; return t.scrollLeft;`); await sleep(800);
  p.headersAfterScroll = await ev(`return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')).map(e => e.textContent.trim());`);
  const fh = await ev(`const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const e = hs.find(h => /Score/.test(h.textContent)); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: e.textContent.trim() };`);
  p.formulaHeader = fh;
  if (fh) {
    await c.clickAt(fh.x, fh.y, { button: 'right' }); await sleep(800);
    p.formulaHeaderMenu = { surfaces: await surfaces(), shot: await shot('table-header-formula-context', '.menu') };
    const rows = (p.formulaHeaderMenu.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && /formula/i.test(r.title || ''));
    p.formulaHeaderSub = {};
    for (const r of rows) { await clickMenuRow(r.title); await sleep(900); p.formulaHeaderSub[r.title] = { surfaces: await surfaces(), shot: await shot('table-header-' + r.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .modal-container') }; await escapeAll(1); await dismiss(); await closeModal(); await c.clickAt(fh.x, fh.y, { button: 'right' }); await sleep(800); }
    await dismiss();
    // the formula column's summary cell
    const fs2 = await ev(`const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => /Score/.test(h.textContent)); const row = document.querySelector('.workspace-leaf.mod-active .bases-table-group-summary-row'); const cell = row && row.querySelectorAll('.bases-td')[i]; if (!cell) return null; const b = cell.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: cell.textContent.trim() };`);
    if (fs2) { await c.clickAt(fs2.x, fs2.y); await sleep(800); p.formulaSummaryMenu = { cell: fs2, surfaces: await surfaces(), shot: await shot('table-summary-formula-menu', '.menu') }; await dismiss(); }
  }
  // the summary menus per column type, on the group summary row and the footer
  await ev(`const t = document.querySelector('.workspace-leaf.mod-active .bases-table-container'); t.scrollLeft = 0; return true;`); await sleep(500);
  p.summaryMenus = {};
  for (const name of ['file name', 'status', 'Rating', 'due', 'done', 'kinds']) {
    const cell = await ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => txt(h) === ${JSON.stringify(name)}); if (i < 0) return null; const row = document.querySelector('.workspace-leaf.mod-active .bases-table-group-summary-row'); const cell = row && row.querySelectorAll('.bases-td')[i]; if (!cell) return null; const b = cell.getBoundingClientRect(); return { i, x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(cell), html: collapse(cell.outerHTML).slice(0, 1200) };`);
    if (!cell) { p.summaryMenus[name] = { missing: true }; continue; }
    await c.mouseMove(cell.x, cell.y); await sleep(500);
    const hover = await ev(`${READ_LIB} const row = document.querySelector('.workspace-leaf.mod-active .bases-table-group-summary-row'); const cell = row.querySelectorAll('.bases-td')[${cell.i}]; const b = cell.querySelector('.summary-expand-button'); return { cellBg: getComputedStyle(cell).backgroundColor, expand: b ? { rect: rectOf(b), opacity: getComputedStyle(b).opacity, display: getComputedStyle(b).display, icon: lucide(b) } : null, placeholder: !!cell.querySelector('.placeholder'), placeholderColor: cell.querySelector('.placeholder') ? getComputedStyle(cell.querySelector('.placeholder')).color : null, placeholderOpacity: cell.querySelector('.placeholder') ? getComputedStyle(cell.querySelector('.placeholder')).opacity : null };`);
    await c.clickAt(cell.x, cell.y); await sleep(800);
    p.summaryMenus[name] = { cell, hover, surfaces: await surfaces(), shot: await shot('table-summary-' + name.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu') };
    await escapeAll(1); await dismiss();
    save();
  }
  // the footer summary row (whole table) for Rating
  const foot = await ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => txt(h) === 'Rating'); const row = document.querySelector('.workspace-leaf.mod-active .bases-table-footer'); const cell = row && row.querySelectorAll('.bases-td')[i]; if (!cell) return null; const b = cell.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(cell), rowCls: String(row.className), html: collapse(row.outerHTML).slice(0, 3000), rect: rectOf(row) };`);
  p.footer = foot;
  if (foot) { await c.clickAt(foot.x, foot.y); await sleep(800); p.footerMenu = { surfaces: await surfaces(), shot: await shot('table-footer-summary-menu', '.menu') }; await escapeAll(1); await dismiss(); }
  // pick "Sum" for Rating through the group summary cell and read the file
  const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
  const rc = await ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => txt(h) === 'Rating'); const row = document.querySelector('.workspace-leaf.mod-active .bases-table-group-summary-row'); const cell = row.querySelectorAll('.bases-td')[i]; const b = cell.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.clickAt(rc.x, rc.y); await sleep(800);
  p.pickSum = await clickText('.menu .bases-toolbar-menu-item', 'Sum');
  await sleep(1500); await dismiss();
  p.pickSumFile = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before ? fs.readFileSync(vp('Rocks.base'), 'utf8') : null;
  if (p.pickSumFile) { fs.writeFileSync(vp('Rocks.base'), before); await sleep(2000); }
  // "Group by this property" and "Hide column" applied, read the file, restore
  for (const [title, col] of [['Group by this property', 'due'], ['Hide column', 'kinds']]) {
    const b0 = fs.readFileSync(vp('Rocks.base'), 'utf8');
    const hc = await ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const e = hs.find(h => txt(h) === ${JSON.stringify(col)}); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (!hc) continue;
    await c.clickAt(hc.x, hc.y, { button: 'right' }); await sleep(800);
    const cl = await clickMenuRow(title); await sleep(1500); await dismiss();
    p['applied:' + title] = { clicked: cl, toolbar: (await toolbarDump()).toolbar.itemsDeep.map((i) => i.text), file: fs.readFileSync(vp('Rocks.base'), 'utf8') !== b0 ? fs.readFileSync(vp('Rocks.base'), 'utf8') : null, shot: await shot('table-after-' + title.replace(/[^a-z0-9]+/gi, '-').toLowerCase()) };
    fs.writeFileSync(vp('Rocks.base'), b0); await sleep(2500);
  }
  return p;
});

await step('s19-fix-rows', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  p.rows = await ev(`${READ_LIB} const rows = ${DATA_ROW}; return rows.slice(0, 2).map(r => ({ cls: String(r.className), rect: rectOf(r), cells: Array.from(r.children).map(e => ({ cls: String(e.className), text: txt(e).slice(0, 40), type: (e.querySelector('[data-property-type]') || {}).getAttribute ? e.querySelector('[data-property-type]').getAttribute('data-property-type') : null, inner: collapse(e.innerHTML).slice(0, 900) })) }));`);
  const cellAt = (i) => ev(`${READ_LIB} const rows = ${DATA_ROW}; const r = rows[0]; const e = r && r.querySelectorAll('.bases-td')[${i}]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: txt(e).slice(0, 40), rect: rectOf(e) };`);
  const headers = await ev(`return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')).map(e => e.textContent.trim());`);
  const c0 = await cellAt(0);
  if (c0) {
    await c.mouseMove(c0.x, c0.y); await sleep(700);
    p.rowHover = { row: await ev(`${READ_LIB} const r = (${DATA_ROW})[0]; const cs = getComputedStyle(r); return { cls: String(r.className), bg: cs.backgroundColor, cellBg: getComputedStyle(r.querySelector('.bases-td')).backgroundColor, controls: Array.from(r.querySelectorAll('.clickable-icon, button, [aria-label]')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), opacity: getComputedStyle(x).opacity })), tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`), shot: await shot('table-row-hover', '.workspace-leaf.mod-active .bases-table-container') };
    await c.clickAt(c0.x, c0.y, { button: 'right' }); await sleep(800);
    p.nameCellMenu = { surfaces: await surfaces(), shot: await shot('table-row-context', '.menu') };
    const sub = (p.nameCellMenu.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && r.submenu);
    p.nameCellSubmenus = {};
    for (const r of sub) { await hoverMenuRow(r.title); await sleep(500); p.nameCellSubmenus[r.title] = { surfaces: await surfaces(), shot: await shot('table-row-context-' + r.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu') }; }
    await dismiss();
    // single click on the name cell (not on the link text)
    await c.clickAt(c0.x + c0.rect.w / 2 - 8, c0.y); await sleep(900);
    p.nameCellClick = { active: await activeElement(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), surfaces: (await surfaces()).map((s) => s.cls), cellNow: await ev(`${READ_LIB} const e = (${DATA_ROW})[0].querySelector('.bases-td'); return { cls: String(e.className), html: collapse(e.innerHTML).slice(0, 800), shadow: getComputedStyle(e).boxShadow, bg: getComputedStyle(e).backgroundColor };`), shot: await shot('table-name-cell-click', '.workspace-leaf.mod-active .bases-table-container') };
    await escapeAll(1);
    if (p.nameCellClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); }
    // the link
    const link = await ev(`const e = (${DATA_ROW})[0].querySelector('.internal-link'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 6, y: b.y + b.height / 2, text: e.textContent, cls: String(e.className) };`);
    p.nameLink = link;
    if (link) {
      await c.mouseMove(link.x, link.y); await sleep(1200);
      p.nameLinkHover = { cursor: await ev(`return getComputedStyle((${DATA_ROW})[0].querySelector('.internal-link')).cursor;`), popover: (await surfaces()).map((s) => s.cls) };
      await c.clickAt(link.x, link.y); await sleep(1200);
      p.nameLinkClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), leaves: await ev(`return app.workspace.getLeavesOfType('markdown').length + ' md, ' + app.workspace.getLeavesOfType('bases').length + ' bases';`) };
      if (p.nameLinkClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); }
    }
    // double click on the status cell
    const c1 = await cellAt(1);
    await c.clickAt(c1.x, c1.y, { clickCount: 2 }); await sleep(800);
    p.dblClick = { cell: c1, active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('table-cell-dblclick', '.workspace-leaf.mod-active .bases-table-container, .suggestion-container') };
    await escapeAll(2);
    await c.clickAt(c1.x, c1.y, { button: 'right' }); await sleep(800);
    p.cellRightClick = { surfaces: await surfaces(), shot: await shot('table-cell-context', '.menu') };
    await dismiss();
  }
  // in-place editing of each cell on the first data row
  p.cellEdit = {};
  const before = fs.readFileSync(vp('Andesite.md'), 'utf8');
  const n = await ev(`const r = (${DATA_ROW})[0]; return r ? r.querySelectorAll('.bases-td').length : 0;`);
  for (let i = 0; i < n; i++) {
    const cell = await cellAt(i);
    if (!cell) continue;
    await c.clickAt(cell.x, cell.y); await sleep(700);
    const rec = { header: headers[i], cell, afterClick: { active: await activeElement(), cell: await ev(`${READ_LIB} const e = (${DATA_ROW})[0].querySelectorAll('.bases-td')[${i}]; return { cls: String(e.className), shadow: getComputedStyle(e).boxShadow, bg: getComputedStyle(e).backgroundColor, html: collapse(e.innerHTML).slice(0, 1500), editing: Array.from(e.querySelectorAll('input, [contenteditable="true"], select, .multi-select-container, .cm-editor')).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className), type: x.getAttribute('type'), value: x.value !== undefined ? x.value : (x.textContent || '').slice(0, 60), placeholder: x.getAttribute('placeholder'), focused: x === document.activeElement || x.contains(document.activeElement) })) };`), surfaces: (await surfaces()).map((s) => ({ cls: s.cls, rows: s.rows.filter((r) => r.cls.includes('suggestion-item')).map((r) => ({ text: r.text, icon: r.icon })).slice(0, 12) })) } };
    rec.shot = await shot('cell-edit-' + i + '-' + String(headers[i] || '').replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.workspace-leaf.mod-active .bases-table-container, .suggestion-container');
    await c.pressKey('Enter'); await sleep(600);
    rec.afterEnter = { active: await activeElement(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), surfaces: (await surfaces()).map((s) => s.cls), cell: await ev(`${READ_LIB} const e = (${DATA_ROW})[0].querySelectorAll('.bases-td')[${i}]; return e ? { cls: String(e.className), shadow: getComputedStyle(e).boxShadow, editingFocused: !!(e.contains(document.activeElement) && document.activeElement !== e) } : null;`) };
    if (rec.afterEnter.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); rec.reopened = true; }
    else {
      await c.pressKey('Escape'); await sleep(500);
      rec.afterEscape = { active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), cell: await ev(`${READ_LIB} const e = (${DATA_ROW})[0].querySelectorAll('.bases-td')[${i}]; return e ? { cls: String(e.className), shadow: getComputedStyle(e).boxShadow } : null;`) };
      await escapeAll(1); await dismiss();
    }
    p.cellEdit[i] = rec; save();
  }
  await sleep(1500);
  p.andesiteChanged = fs.readFileSync(vp('Andesite.md'), 'utf8') !== before;
  p.andesiteAfter = p.andesiteChanged ? fs.readFileSync(vp('Andesite.md'), 'utf8') : null;
  // a real text edit: type into the status cell, Enter, read the file
  const c1 = await cellAt(1);
  if (c1) {
    const b0 = fs.readFileSync(vp('Andesite.md'), 'utf8');
    await c.clickAt(c1.x, c1.y); await sleep(500);
    await c.pressKey('a', { ctrl: true }); await c.typeText('edited', 40); await c.pressKey('Enter'); await sleep(1500);
    p.textEdit = { active: await activeElement(), fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== b0, fileAfter: fs.readFileSync(vp('Andesite.md'), 'utf8'), cellText: await ev(`${READ_LIB} return txt((${DATA_ROW})[0].querySelectorAll('.bases-td')[1]);`), rowsOrder: await ev(`${READ_LIB} return (${DATA_ROW}).map(r => txt(r.querySelector('.bases-td')));`) };
    await escapeAll(1);
    fs.writeFileSync(vp('Andesite.md'), b0); await sleep(2000);
  }
  // the checkbox
  const box = await ev(`const e = (${DATA_ROW})[0].querySelector('input[type="checkbox"]'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, checked: e.checked, cls: String(e.className) };`);
  p.checkbox = box;
  if (box) { const b2 = fs.readFileSync(vp('Andesite.md'), 'utf8'); await c.clickAt(box.x, box.y); await sleep(1500); p.checkboxAfter = { checked: await ev(`const e = (${DATA_ROW})[0].querySelector('input[type="checkbox"]'); return e ? e.checked : null;`), fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== b2, fileNow: fs.readFileSync(vp('Andesite.md'), 'utf8') }; fs.writeFileSync(vp('Andesite.md'), b2); await sleep(2000); }
  // the New button: the note it makes, the popover it opens, its DOM
  const files0 = vaultFiles();
  await dismiss();
  const nb = await c.clickElement('.workspace-leaf.mod-active .bases-toolbar-new-item-menu'); await sleep(2000);
  p.newButton = { rect: nb, surfaces: await surfaces(), created: vaultFiles().filter((x) => !files0.includes(x)), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), popover: await ev(`${READ_LIB} const po = document.querySelector('.popover.bases-new-item-popover'); if (!po) return null; return { cls: String(po.className), rect: rectOf(po), style: po.getAttribute('style'), header: Array.from(po.querySelectorAll('.popover-header, .popover-title, .clickable-icon, [aria-label]')).slice(0, 12).map(e => ({ cls: String(e.className), aria: e.getAttribute('aria-label'), icon: lucide(e), text: txt(e).slice(0, 40), rect: rectOf(e) })), inlineTitle: txt(po.querySelector('.inline-title')), html: collapse(po.outerHTML).replace(/>\\s+</g, '><').slice(0, 6000) };`), shot: await shot('new-button-popover') };
  p.newButton.createdBytes = p.newButton.created.map((x) => ({ name: x, text: fs.readFileSync(vp(x), 'utf8') }));
  p.newButton.rowsNow = await ev(`${READ_LIB} return (${DATA_ROW}).map(r => txt(r.querySelector('.bases-td')));`);
  await closePopovers();
  p.newButton.afterEscape = { surfaces: (await surfaces()).map((s) => s.cls), active: await activeElement() };
  return p;
});

await step('s20-fix-kanban', async (p) => {
  await openBase('Rocks.base'); await switchView('Board'); await dismiss(); await closePopovers();
  const REAL_COLS = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-kanban-column')).filter(e => getComputedStyle(e).opacity !== '0' && !/^Fj/.test(e.textContent))`;
  p.columns = await ev(`${READ_LIB} return (${REAL_COLS}).map(e => ({ cls: String(e.className), rect: rectOf(e), header: txt(e.querySelector('.bases-kanban-column-header')), count: txt(e.querySelector('.bases-kanban-column-count')), cards: e.querySelectorAll('.bases-kanban-card').length, footer: txt(e.querySelector('.bases-kanban-column-footer')), html: collapse(e.outerHTML).replace(/>\\s+</g, '><').slice(0, 4000) }));`);
  p.shot = await shot('kanban-view');
  const col = await ev(`${READ_LIB} const e = (${REAL_COLS})[1] || (${REAL_COLS})[0]; if (!e) return null; const h = e.querySelector('.bases-kanban-column-header'); const b = h.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(h), cls: String(h.className) };`);
  p.columnHeader = col;
  if (col) {
    await c.mouseMove(col.x, col.y); await sleep(700);
    p.columnHover = { controls: await ev(`${READ_LIB} const e = (${REAL_COLS})[1] || (${REAL_COLS})[0]; const h = e.querySelector('.bases-kanban-column-header'); return { cursor: getComputedStyle(h).cursor, bg: getComputedStyle(h).backgroundColor, controls: Array.from(h.querySelectorAll('*')).map(x => ({ tag: x.tagName.toLowerCase(), cls: String(x.className && x.className.baseVal !== undefined ? x.className.baseVal : x.className), aria: x.getAttribute('aria-label'), opacity: getComputedStyle(x).opacity, display: getComputedStyle(x).display, text: txt(x).slice(0, 30) })) };`), shot: await shot('kanban-column-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(col.x, col.y); await sleep(800);
    p.columnClick = { surfaces: await surfaces(), active: await activeElement() };
    await dismiss();
    await c.clickAt(col.x, col.y, { button: 'right' }); await sleep(800);
    p.columnRightClick = { surfaces: await surfaces(), shot: await shot('kanban-column-context', '.menu') };
    // Edit groups...
    p.editGroupsClick = await clickMenuRow('Edit groups...');
    await sleep(900);
    p.editGroups = { surfaces: await surfaces(), shot: await shot('kanban-edit-groups', '.menu, .modal-container') };
    await escapeAll(1); await dismiss(); await closeModal();
    // the footer "New" button
    const nbtn = await ev(`${READ_LIB} const e = (${REAL_COLS})[1] || (${REAL_COLS})[0]; const b = e.querySelector('.bases-kanban-column-create, .bases-kanban-column-footer .clickable-icon, .bases-kanban-column-footer [class*="create"], .bases-kanban-column-footer'); if (!b) return null; const r = b.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, cls: String(b.className), text: txt(b), icon: lucide(b), html: collapse(b.outerHTML).slice(0, 800) };`);
    p.footerButton = nbtn;
    if (nbtn) {
      await c.mouseMove(nbtn.x, nbtn.y); await sleep(1000);
      p.footerTooltip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
      const files0 = vaultFiles();
      await c.clickAt(nbtn.x, nbtn.y); await sleep(2000);
      p.footerClick = { created: vaultFiles().filter((x) => !files0.includes(x)), surfaces: (await surfaces()).map((s) => s.cls), active: await activeElement(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), shot: await shot('kanban-column-new-item') };
      p.footerClick.createdBytes = p.footerClick.created.map((x) => ({ name: x, text: fs.readFileSync(vp(x), 'utf8') }));
      await closePopovers();
      if (p.footerClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
    }
  }
  const card = await ev(`${READ_LIB} const cols = ${REAL_COLS}; const e = cols.flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(e.className), text: txt(e).slice(0, 60), html: collapse(e.outerHTML).slice(0, 3000), draggable: e.getAttribute('draggable') };`);
  p.card = card;
  if (card) {
    await c.mouseMove(card.x, card.y); await sleep(700);
    p.cardHover = { dom: await ev(`${READ_LIB} const cols = ${REAL_COLS}; const e = cols.flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]; const cs = getComputedStyle(e); return { bg: cs.backgroundColor, shadow: cs.boxShadow, cursor: cs.cursor, controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), opacity: getComputedStyle(x).opacity })) };`), shot: await shot('kanban-card-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(card.x, card.y, { button: 'right' }); await sleep(800);
    p.cardRightClick = { surfaces: await surfaces(), shot: await shot('kanban-card-context', '.menu') };
    const sub = (p.cardRightClick.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && r.submenu);
    p.cardSubmenus = {};
    for (const r of sub) { await hoverMenuRow(r.title); await sleep(500); p.cardSubmenus[r.title] = (await surfaces()).map((m) => m.rows.filter((x) => x.cls.includes('menu-item')).map((x) => [x.title, x.icon])); }
    await dismiss();
    await c.clickAt(card.x, card.y); await sleep(900);
    p.cardClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), cardShadow: await ev(`const cols = ${REAL_COLS}; const e = cols.flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]; return e ? getComputedStyle(e).boxShadow : null;`), shot: await shot('kanban-card-click') };
    if (p.cardClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
    await closePopovers();
    // a property line on the card
    const pc = await ev(`${READ_LIB} const cols = ${REAL_COLS}; const e = cols.flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]; const pr = e.querySelectorAll('.bases-kanban-card-property')[1]; if (!pr) return null; const b = pr.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, cls: String(pr.className), text: txt(pr), html: collapse(pr.outerHTML).slice(0, 1000) };`);
    if (pc) { await c.clickAt(pc.x, pc.y); await sleep(700); p.cardPropertyClick = { cell: pc, active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('kanban-card-property-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') }; await escapeAll(2); }
    // a title click
    const tc = await ev(`${READ_LIB} const cols = ${REAL_COLS}; const e = cols.flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]; const t = e.querySelector('.bases-kanban-card-property.mod-title .internal-link, .bases-kanban-card-property.mod-title'); const b = t.getBoundingClientRect(); return { x: b.x + 8, y: b.y + b.height / 2, cls: String(t.className), text: txt(t) };`);
    if (tc) { await c.clickAt(tc.x, tc.y); await sleep(1000); p.cardTitleClick = { target: tc, leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`) }; if (p.cardTitleClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); } }
    // drag to another column
    const card2 = await ev(`const cols = ${REAL_COLS}; const e = cols.flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    const col2 = await ev(`const cols = ${REAL_COLS}; const e = cols[2] || cols[0]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (card2 && col2) {
      const d = await c.dragOverOnly(card2.x, card2.y, col2.x, col2.y);
      p.cardDrag = { started: d.started, mimeTypes: d.data && d.data.items ? d.data.items.map((i) => i.mimeType + ':' + String(i.data).slice(0, 60)) : d.error };
      if (d.started) { p.cardDragMid = await ev(`${READ_LIB} return { marks: Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], .workspace-leaf.mod-active [class*="placeholder"], .workspace-leaf.mod-active .is-dragging, body > [class*="drag"]')).map(e => ({ cls: String(e.className), rect: rectOf(e) })), bodyCls: document.body.className };`); p.cardDragShot = await shot('kanban-card-drag', '.workspace-leaf.mod-active .view-content'); await d.finish(); }
    }
    // column header drag
    const ch1 = await ev(`const cols = ${REAL_COLS}; const h = cols[1].querySelector('.bases-kanban-column-header'); const b = h.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    const ch2 = await ev(`const cols = ${REAL_COLS}; const h = (cols[2] || cols[0]).querySelector('.bases-kanban-column-header'); const b = h.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (ch1 && ch2) { const d = await c.dragOverOnly(ch1.x, ch1.y, ch2.x, ch2.y); p.columnDrag = { started: d.started, mimeTypes: d.data && d.data.items ? d.data.items.map((i) => i.mimeType) : d.error }; if (d.started) { p.columnDragMid = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], .workspace-leaf.mod-active [class*="placeholder"]')).map(e => ({ cls: String(e.className), rect: rectOf(e) }));`); p.columnDragShot = await shot('kanban-column-drag', '.workspace-leaf.mod-active .view-content'); await d.finish(); } }
  }
  const bg = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-kanban-container'); const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.bottom - 40 };`);
  await c.clickAt(bg.x, bg.y, { button: 'right' }); await sleep(700);
  p.backgroundRightClick = { surfaces: await surfaces() };
  await dismiss();
  p.fileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  return p;
});

await step('s21-fix-cards', async (p) => {
  await openBase('Rocks.base'); await switchView('Cards'); await dismiss(); await closePopovers();
  const REAL = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-cards-item[draggable]'))`;
  p.cards = await ev(`${READ_LIB} return (${REAL}).slice(0, 2).map(e => ({ cls: String(e.className), rect: rectOf(e), style: e.getAttribute('style'), html: collapse(e.outerHTML).slice(0, 3000), cover: (() => { const cv = e.querySelector('.bases-cards-cover'); const cs = getComputedStyle(cv); return { rect: rectOf(cv), bgImage: cs.backgroundImage.slice(0, 80), bgSize: cs.backgroundSize, bgPosition: cs.backgroundPosition, bg: cs.backgroundColor, radius: cs.borderRadius, aspect: cs.aspectRatio }; })(), properties: Array.from(e.querySelectorAll('.bases-cards-property')).map(pr => ({ cls: String(pr.className), label: txt(pr.querySelector('.bases-cards-label')), line: txt(pr.querySelector('.bases-cards-line')), type: (pr.querySelector('[data-property-type]') || {}).getAttribute ? pr.querySelector('[data-property-type]').getAttribute('data-property-type') : null })) }));`);
  p.groupHeading = await ev(`${READ_LIB} const g = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-group-heading')).find(e => getComputedStyle(e).opacity !== '0'); return g ? { cls: String(g.className), text: txt(g), rect: rectOf(g), html: collapse(g.outerHTML).slice(0, 800) } : null;`);
  const card = await ev(`const e = (${REAL})[0]; if (!e) return null; const b = e.getBoundingClientRect(); const cv = e.querySelector('.bases-cards-cover').getBoundingClientRect(); const t = e.querySelector('.bases-cards-property.mod-title').getBoundingClientRect(); const pr = e.querySelectorAll('.bases-cards-property')[2]; const pb = pr ? pr.getBoundingClientRect() : null; return { cx: b.x + b.width / 2, cy: b.y + b.height / 2, coverX: cv.x + cv.width / 2, coverY: cv.y + cv.height / 2, titleX: t.x + 10, titleY: t.y + t.height / 2, propX: pb ? pb.x + pb.width / 2 : null, propY: pb ? pb.y + pb.height / 2 : null, propText: pr ? pr.textContent : null };`);
  p.card = card;
  if (card) {
    await c.mouseMove(card.coverX, card.coverY); await sleep(700);
    p.cardHover = { dom: await ev(`${READ_LIB} const e = (${REAL})[0]; const cs = getComputedStyle(e); return { bg: cs.backgroundColor, shadow: cs.boxShadow, cursor: cs.cursor, coverCursor: getComputedStyle(e.querySelector('.bases-cards-cover')).cursor, controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label]')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), opacity: getComputedStyle(x).opacity })), tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`), shot: await shot('cards-hover', '.workspace-leaf.mod-active .view-content') };
    await c.clickAt(card.coverX, card.coverY, { button: 'right' }); await sleep(800);
    p.cardRightClick = { surfaces: await surfaces(), shot: await shot('cards-context', '.menu') };
    await dismiss();
    await c.clickAt(card.coverX, card.coverY); await sleep(1000);
    p.coverClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), cardShadow: await ev(`const e = (${REAL})[0]; return e ? getComputedStyle(e).boxShadow : null;`), shot: await shot('cards-cover-click') };
    if (p.coverClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Cards'); }
    await closePopovers();
    await c.clickAt(card.titleX, card.titleY); await sleep(1000);
    p.titleClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), surfaces: (await surfaces()).map((s) => s.cls) };
    if (p.titleClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Cards'); }
    await closePopovers();
    if (card.propX) { await c.clickAt(card.propX, card.propY); await sleep(800); p.propertyClick = { text: card.propText, active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('cards-property-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') }; await escapeAll(2); if ((await ev(`return app.workspace.activeLeaf.view.getViewType();`)) !== 'bases') { await openBase('Rocks.base'); await switchView('Cards'); } }
    // drag a card
    const c2 = await ev(`const e = (${REAL})[1]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + 20 };`);
    const d = await c.dragOverOnly(card.coverX, card.coverY, c2.x, c2.y);
    p.cardDrag = { started: d.started, mimeTypes: d.data && d.data.items ? d.data.items.map((i) => i.mimeType + ':' + String(i.data).slice(0, 80)) : d.error };
    if (d.started) { p.cardDragMid = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], body > [class*="drag"], .drag-ghost')).map(e => ({ cls: String(e.className), rect: rectOf(e) }));`); await d.finish(); }
  }
  // the list layout: the row's own menu via the link vs the text
  await switchView('List'); await dismiss();
  const li = await ev(`${READ_LIB} const e = document.querySelector('.workspace-leaf.mod-active .bases-list-item'); const b = e.getBoundingClientRect(); const l = e.querySelector('.internal-link').getBoundingClientRect(); const v = e.querySelectorAll('.bases-list-property')[1].getBoundingClientRect(); return { rowX: b.right - 20, rowY: b.y + b.height / 2, linkX: l.x + 6, linkY: l.y + l.height / 2, propX: v.x + v.width / 2, propY: v.y + v.height / 2 };`);
  await c.clickAt(li.propX, li.propY); await sleep(800);
  p.listPropertyClick = { active: await activeElement(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), surfaces: (await surfaces()).map((s) => s.cls), shot: await shot('list-property-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') };
  await escapeAll(1);
  if (p.listPropertyClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('List'); }
  await c.clickAt(li.rowX, li.rowY, { button: 'right' }); await sleep(800);
  p.listRowEndRightClick = { surfaces: (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => r.cls.includes('menu-item')).map((r) => [r.title, r.icon]) })) };
  await dismiss();
  await c.clickAt(li.linkX, li.linkY, { button: 'right' }); await sleep(800);
  p.listLinkRightClick = { surfaces: (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => r.cls.includes('menu-item')).map((r) => [r.title, r.icon]) })) };
  await dismiss();
  return p;
});

await step('s22-fix-embed', async (p) => {
  await dismiss(); await closePopovers();
  await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await sleep(800);
  const openMode = async (mode) => {
    await ev(`const f = app.vault.getAbstractFileByPath('Embeds.md'); const leaf = app.workspace.getLeaf('tab'); await leaf.openFile(f, { state: { mode: ${JSON.stringify(mode)}, source: false } }); app.workspace.setActiveLeaf(leaf, { focus: true }); return true;`);
    await sleep(3500);
  };
  const VIS = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .internal-embed.bases-embed')).filter(e => e.offsetWidth > 0)`;
  const embedInfo = () => ev(`${READ_LIB} return (${VIS}).map(e => { const cs = getComputedStyle(e); return { tag: e.tagName.toLowerCase(), cls: String(e.className), src: e.getAttribute('src'), rect: rectOf(e), height: cs.height, maxHeight: cs.maxHeight, border: cs.borderWidth + ' ' + cs.borderColor, radius: cs.borderRadius, padding: cs.padding, overflow: cs.overflow, bg: cs.backgroundColor,
    toolbar: Array.from(e.querySelectorAll('.bases-toolbar-item')).map(i => ({ cls: String(i.className), text: txt(i).slice(0, 40), icon: lucide(i), rect: rectOf(i) })),
    extraText: Array.from(e.querySelectorAll('.bases-header > :not(.bases-toolbar), .bases-view > :not(.bases-table-container):not(.bases-cards-container):not(.bases-list-container):not(.bases-kanban-container)')).map(x => ({ cls: String(x.className), text: txt(x).slice(0, 80) })),
    controls: Array.from(e.querySelectorAll('.clickable-icon, button, [aria-label], .embed-action')).filter(x => !x.closest('.bases-toolbar-item') && !x.closest('.bases-td')).map(x => ({ cls: String(x.className), aria: x.getAttribute('aria-label'), icon: lucide(x), rect: rectOf(x), opacity: getComputedStyle(x).opacity, display: getComputedStyle(x).display })),
    viewRect: rectOf(e.querySelector('.bases-view') || e), tableRect: e.querySelector('.bases-table-container') ? rectOf(e.querySelector('.bases-table-container')) : null, scrollable: (() => { const t = e.querySelector('.bases-table-container'); return t ? { scrollH: t.scrollHeight, clientH: t.clientHeight, overflowY: getComputedStyle(t).overflowY } : null; })() }; });`);
  for (const [mode, key] of [['preview', 'reading'], ['source', 'livePreview']]) {
    await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await sleep(600);
    await openMode(mode);
    const rec = { embeds: await embedInfo() };
    rec.shot = await shot('embed-' + key);
    const e0 = await ev(`const e = (${VIS})[0]; if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + 12, cx: b.x + b.width / 2, cy: b.y + b.height / 2, right: b.right - 16 };`);
    if (e0) {
      await c.mouseMove(e0.right, e0.y); await sleep(900);
      rec.hover = { embeds: await embedInfo(), tooltip: await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`), shot: await shot('embed-' + key + '-hover', '.workspace-leaf.mod-active .internal-embed.bases-embed') };
      const ctrls = (rec.hover.embeds[0] || { controls: [] }).controls.filter((x) => x.rect.w > 0);
      rec.controls = {};
      for (const ct of ctrls.slice(0, 4)) {
        await c.mouseMove(ct.rect.x + ct.rect.w / 2, ct.rect.y + ct.rect.h / 2); await sleep(1000);
        const tip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`);
        await c.clickAt(ct.rect.x + ct.rect.w / 2, ct.rect.y + ct.rect.h / 2); await sleep(1200);
        rec.controls[ct.aria || ct.cls] = { control: ct, tooltip: tip, surfaces: await surfaces(), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), caret: await ev(`try { return app.workspace.activeLeaf.view.editor.getCursor(); } catch (e) { return null; }`), mode: await ev(`try { return app.workspace.activeLeaf.view.getMode(); } catch (e) { return null; }`), active: await activeElement(), shot: await shot('embed-' + key + '-control-' + String(ct.aria || 'x').replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .suggestion-container, .workspace-leaf.mod-active .view-content') };
        await dismiss(); await escapeAll(1);
        await ev(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`); await sleep(500); await openMode(mode);
      }
      // right-click inside the embed's table (on a name cell) and on the embed's toolbar
      const td = await ev(`const e = (${VIS})[0]; const cell = e && e.querySelector('.bases-tbody .bases-td, .bases-table-cell'); if (!cell) return null; const b = cell.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
      if (td) { await c.clickAt(td.x, td.y, { button: 'right' }); await sleep(800); rec.cellRightClick = { surfaces: (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => r.cls.includes('menu-item')).map((r) => [r.title, r.icon]) })), shot: await shot('embed-' + key + '-cell-context', '.menu') }; await dismiss(); }
      const tb = await ev(`const e = (${VIS})[0]; const t = e && e.querySelector('.bases-toolbar'); if (!t) return null; const b = t.getBoundingClientRect(); return { x: b.right - 30, y: b.y + b.height / 2 };`);
      if (tb) { await c.clickAt(tb.x, tb.y, { button: 'right' }); await sleep(800); rec.toolbarRightClick = { surfaces: (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => r.cls.includes('menu-item')).map((r) => [r.title, r.icon]) })) }; await dismiss(); }
      // the embedded views control and the properties control
      const vm = await ev(`const e = (${VIS})[0]; const v = e.querySelector('.bases-toolbar-views-menu'); const b = v.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
      await c.clickAt(vm.x, vm.y); await sleep(900);
      rec.viewsPanel = { surfaces: await surfaces(), shot: await shot('embed-' + key + '-views-panel', '.menu') };
      await dismiss();
      const before = fs.readFileSync(vp('Rocks.base'), 'utf8');
      const sm = await ev(`const e = (${VIS})[0]; const v = e.querySelector('.bases-toolbar-sort-menu'); const b = v.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
      await c.clickAt(sm.x, sm.y); await sleep(900);
      rec.sortPanel = { surfaces: (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => /combobox-button|menu-action|section-header/.test(r.cls)).map((r) => [r.title || r.text, r.icon]) })) };
      await dismiss(); await sleep(1500);
      rec.sortPanelWrote = fs.readFileSync(vp('Rocks.base'), 'utf8') !== before;
      // the "New" button inside the embed
      const files0 = vaultFiles();
      const nm = await ev(`const e = (${VIS})[0]; const v = e.querySelector('.bases-toolbar-new-item-menu'); const b = v.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
      await c.clickAt(nm.x, nm.y); await sleep(2000);
      rec.newInEmbed = { created: vaultFiles().filter((x) => !files0.includes(x)), surfaces: (await surfaces()).map((s) => s.cls), active: await activeElement(), shot: await shot('embed-' + key + '-new') };
      await closePopovers();
      // Live Preview: click on the embed's margin to see whether the caret lands and the embed gets a selection frame
      if (mode === 'source') {
        await c.clickAt(e0.right - 40, e0.y); await sleep(600);
        rec.marginClick = { caret: await ev(`try { return app.workspace.activeLeaf.view.editor.getCursor(); } catch (e) { return null; }`), active: await activeElement(), embedCls: await ev(`const e = (${VIS})[0]; return e ? String(e.className) + ' | parent ' + String(e.parentElement.className) : null;`), shot: await shot('embed-lp-margin-click', '.workspace-leaf.mod-active .internal-embed.bases-embed') };
      }
    }
    p[key] = rec; save();
  }
  p.embedsFileAfter = fs.readFileSync(vp('Embeds.md'), 'utf8');
  p.rocksFileAfter = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await ev(`app.workspace.detachLeavesOfType('markdown'); return true;`);
  return p;
});

await step('s23-fix-keyboard', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss(); await closePopovers();
  const cell = await ev(`const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[1]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.clickAt(cell.x, cell.y); await sleep(500);
  const state = () => ev(`${READ_LIB} const a = document.activeElement; const cells = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-td')); const idx = cells.findIndex(c => c.contains(a) || c === a); const cur = idx >= 0 ? cells[idx] : null;
    const marked = cells.filter(c => /is-|mod-/.test(c.className) && !c.classList.contains('mod-implicit')).map(c => ({ cls: String(c.className), text: txt(c).slice(0, 30) }));
    return { active: a ? { tag: a.tagName.toLowerCase(), cls: String(a.className).slice(0, 100), editable: a.isContentEditable, text: txt(a).slice(0, 30), type: a.getAttribute('type') } : null, cellIndex: idx, cell: cur ? { cls: String(cur.className), header: (() => { const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const row = cur.closest('.bases-tr'); const i = Array.from(row.children).indexOf(cur); return hs[i] ? txt(hs[i]) : null; })(), rowText: txt(cur.closest('.bases-tr').querySelector('.bases-td')).slice(0, 20), shadow: getComputedStyle(cur).boxShadow, editing: !!(cur.querySelector('input, [contenteditable="true"]') && cur.contains(a) && a !== cur) } : null, marked: marked.slice(0, 5), surfaces: Array.from(document.querySelectorAll(SURF)).map(e => String(e.className)), leafType: app.workspace.activeLeaf.view.getViewType() };`);
  p.afterClick = await state();
  p.afterClickShot = await shot('table-cell-click-state', '.workspace-leaf.mod-active .bases-table-container');
  p.keys = [];
  for (const [k, mods] of [['Escape'], ['Tab'], ['Tab'], ['Tab', { shift: true }], ['ArrowRight'], ['ArrowRight'], ['ArrowDown'], ['ArrowLeft'], ['ArrowUp'], ['Enter'], ['Escape'], ['Enter'], ['Enter'], ['Escape'], ['Escape']]) {
    await c.pressKey(k, mods || {}); await sleep(450);
    const s = await state();
    p.keys.push({ key: k, mods: mods || null, ...s });
    if (s.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); await c.clickAt(cell.x, cell.y); await sleep(500); p.keys[p.keys.length - 1].reopened = true; }
    if (k === 'Tab' && p.keys.length === 3) p.tabShot = await shot('table-keyboard-tab', '.workspace-leaf.mod-active .bases-table-container');
  }
  await escapeAll(2);
  // Escape from an open cell editor, and from the view (does it blur?)
  p.fileAfter = fs.readFileSync(vp('Andesite.md'), 'utf8');
  return p;
});


// ---- third pass: the formula column, the link's own menu, cell editors for
// tags / link / datetime, the keyboard focus ring (a box-shadow, not a class),
// and the kanban interactions the second pass did not reach.
const FOCUSED_CELL = `(() => { const cells = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-td')); const f = cells.find(c => /rgb\\(85, 85, 85\\)|rgb\\(138, 92, 245\\)/.test(getComputedStyle(c).boxShadow)); if (!f) return null; const row = f.closest('.bases-tr'); const i = Array.from(row.children).indexOf(f); const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); return { header: hs[i] ? hs[i].textContent.trim() : null, row: (row.querySelector('.bases-td') || {}).textContent, cls: String(f.className), shadow: getComputedStyle(f).boxShadow, bg: getComputedStyle(f).backgroundColor, editing: !!(f.contains(document.activeElement) && document.activeElement !== f) }; })()`;

await step('s24-formula-column', async (p) => {
  await openBase('Formula.base'); await dismiss(); await closePopovers();
  p.headers = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')).map(e => ({ text: txt(e), icons: lucides(e), cls: String(e.className) }));`);
  p.shot = await shot('formula-base');
  const hdr = async (name) => ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const e = hs.find(h => txt(h) === ${JSON.stringify(name)}); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  const sh = await hdr('Score');
  if (sh) {
    await c.clickAt(sh.x, sh.y, { button: 'right' }); await sleep(800);
    p.formulaHeaderMenu = { surfaces: await surfaces(), shot: await shot('table-header-formula-context', '.menu') };
    const rows = (p.formulaHeaderMenu.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && (r.submenu || /\.\.\.$/.test(r.title || '')));
    p.formulaHeaderSub = {};
    for (const r of rows) {
      await hoverMenuRow(r.title); await sleep(500);
      let sf = await surfaces();
      if (sf.length < 2) { await clickMenuRow(r.title); await sleep(900); sf = await surfaces(); }
      p.formulaHeaderSub[r.title] = { surfaces: sf, shot: await shot('table-header-formula-' + r.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu, .modal-container') };
      await escapeAll(1); await dismiss(); await closeModal();
      await c.clickAt(sh.x, sh.y, { button: 'right' }); await sleep(800);
    }
    await dismiss();
    const sc = await ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => txt(h) === 'Score'); const row = document.querySelector('.workspace-leaf.mod-active .bases-table-footer'); const cell = row && row.querySelectorAll('.bases-td')[i]; if (!cell) return null; const b = cell.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(cell) };`);
    if (sc) { await c.clickAt(sc.x, sc.y); await sleep(800); p.formulaSummaryMenu = { cell: sc, surfaces: await surfaces(), shot: await shot('table-summary-formula-menu', '.menu') }; await escapeAll(1); await dismiss(); }
  }
  for (const name of ['tags', 'related', 'when']) {
    const h = await hdr(name);
    if (!h) continue;
    await c.clickAt(h.x, h.y, { button: 'right' }); await sleep(700);
    p['headerMenu:' + name] = (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => r.cls.includes('menu-item') || r.cls.includes('menu-separator')).map((r) => r.separator ? '---' : [r.title, r.icon, r.states, r.submenu]) }));
    await dismiss();
  }
  // the cells of the first data row: Score (read-only?), tags, related, when
  const cellAt = (name) => ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => txt(h) === ${JSON.stringify(name)}); const r = (${DATA_ROW})[0]; const e = r && r.querySelectorAll('.bases-td')[i]; if (!e) return null; const b = e.getBoundingClientRect(); return { i, x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(e).slice(0, 40), html: collapse(e.innerHTML).slice(0, 900) };`);
  p.cells = {};
  for (const name of ['Score', 'tags', 'related', 'when', 'density']) {
    const cell = await cellAt(name);
    if (!cell) continue;
    await c.clickAt(cell.x, cell.y); await sleep(700);
    const rec = { cell, afterClick: { active: await activeElement(), focused: await ev(`return ${FOCUSED_CELL};`), html: await ev(`${READ_LIB} const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[${cell.i}]; return collapse(e.innerHTML).slice(0, 1500);`), surfaces: (await surfaces()).map((x) => ({ cls: x.cls, rows: x.rows.filter((r) => r.cls.includes('suggestion-item')).map((r) => [r.text, r.icon]).slice(0, 10) })) }, shot: await shot('cell-edit-' + name.toLowerCase(), '.workspace-leaf.mod-active .bases-table-container, .suggestion-container') };
    if (name === 'tags') {
      await c.typeText('newtag', 40); await sleep(500);
      rec.typed = { surfaces: (await surfaces()).map((x) => ({ cls: x.cls, rows: x.rows.filter((r) => r.cls.includes('suggestion-item')).map((r) => [r.text, r.icon]).slice(0, 10) })), shot: await shot('cell-edit-tags-typed', '.workspace-leaf.mod-active .bases-table-container, .suggestion-container') };
      const b0 = fs.readFileSync(vp('Andesite.md'), 'utf8');
      await c.pressKey('Enter'); await sleep(1200);
      rec.afterEnter = { html: await ev(`${READ_LIB} const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[${cell.i}]; return collapse(e.innerHTML).slice(0, 1500);`), active: await activeElement(), fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== b0, file: fs.readFileSync(vp('Andesite.md'), 'utf8') };
      await escapeAll(2);
      fs.writeFileSync(vp('Andesite.md'), b0); await sleep(2000);
    } else {
      await c.pressKey('Enter'); await sleep(600);
      rec.afterEnter = { active: await activeElement(), focused: await ev(`return ${FOCUSED_CELL};`), leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), surfaces: (await surfaces()).map((x) => x.cls) };
      if (rec.afterEnter.leafType !== 'bases') { await openBase('Formula.base'); rec.reopened = true; }
      else await escapeAll(2);
    }
    p.cells[name] = rec; save();
  }
  // right-click on the link text in the name cell, with submenus
  const link = await ev(`const e = (${DATA_ROW})[0].querySelector('.internal-link'); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + 6, y: b.y + b.height / 2, text: e.textContent };`);
  if (link) {
    await c.clickAt(link.x, link.y, { button: 'right' }); await sleep(800);
    p.linkMenu = { surfaces: await surfaces(), shot: await shot('table-link-context', '.menu') };
    const sub = (p.linkMenu.surfaces.find((m) => /^menu/.test(m.cls)) || { rows: [] }).rows.filter((r) => r.cls.includes('menu-item') && r.submenu);
    p.linkSubmenus = {};
    for (const r of sub) { await hoverMenuRow(r.title); await sleep(500); p.linkSubmenus[r.title] = { surfaces: (await surfaces()).slice(1), shot: await shot('table-link-context-' + r.title.replace(/[^a-z0-9]+/gi, '-').toLowerCase(), '.menu') }; }
    await dismiss();
    // hover the link: page preview?
    await c.mouseMove(link.x, link.y); await sleep(1500);
    p.linkHover = { surfaces: (await surfaces()).map((x) => x.cls), tooltip: await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`) };
    await c.mouseMove(5, 5);
  }
  // a typed edit on density, then Escape: reverted?
  const dc = await cellAt('density');
  if (dc) {
    const b0 = fs.readFileSync(vp('Andesite.md'), 'utf8');
    await c.clickAt(dc.x, dc.y); await sleep(500);
    await c.pressKey('a', { ctrl: true }); await c.typeText('9.9', 40); await sleep(300);
    p.escapeEdit = { typedValue: await ev(`const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[${dc.i}].querySelector('input'); return e ? e.value : null;`) };
    await c.pressKey('Escape'); await sleep(1200);
    p.escapeEdit.afterEscape = { cellText: await ev(`${READ_LIB} const r = (${DATA_ROW})[0]; return txt(r.querySelectorAll('.bases-td')[${dc.i}]);`), inputValue: await ev(`const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[${dc.i}].querySelector('input'); return e ? e.value : null;`), fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== b0, active: await activeElement(), focused: await ev(`return ${FOCUSED_CELL};`) };
    await escapeAll(1);
    // and Enter commits
    await c.clickAt(dc.x, dc.y); await sleep(500);
    await c.pressKey('a', { ctrl: true }); await c.typeText('9.9', 40); await c.pressKey('Enter'); await sleep(1500);
    p.enterEdit = { cellText: await ev(`${READ_LIB} const r = (${DATA_ROW})[0]; return txt(r.querySelectorAll('.bases-td')[${dc.i}]);`), fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== b0, file: fs.readFileSync(vp('Andesite.md'), 'utf8'), focused: await ev(`return ${FOCUSED_CELL};`), active: await activeElement() };
    fs.writeFileSync(vp('Andesite.md'), b0); await sleep(2000);
  }
  // the date cell's "Open daily note" icon on hover (Rocks.base has the date column)
  await openBase('Rocks.base'); await switchView('Table'); await dismiss();
  const due = await ev(`${READ_LIB} const hs = Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-thead .bases-td')); const i = hs.findIndex(h => txt(h) === 'due'); const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[i]; const b = e.getBoundingClientRect(); return { i, x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (due) {
    await c.mouseMove(due.x, due.y); await sleep(700);
    p.dateHover = await ev(`${READ_LIB} const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[${due.i}]; const ic = e.querySelector('.clickable-icon'); return { html: collapse(e.innerHTML).slice(0, 800), icon: ic ? { aria: ic.getAttribute('aria-label'), icon: lucide(ic), opacity: getComputedStyle(ic).opacity, rect: rectOf(ic) } : null };`);
    p.dateHoverShot = await shot('table-date-cell-hover', '.workspace-leaf.mod-active .bases-table-container');
    if (p.dateHover.icon) { await c.mouseMove(p.dateHover.icon.rect.x + 11, p.dateHover.icon.rect.y + 11); await sleep(1100); p.dateIconTooltip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`); }
  }
  return p;
});

await step('s25-keyboard-focus', async (p) => {
  await openBase('Rocks.base'); await switchView('Table'); await dismiss(); await closePopovers();
  const cell = await ev(`const r = (${DATA_ROW})[0]; const e = r.querySelectorAll('.bases-td')[1]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.clickAt(cell.x, cell.y); await sleep(500);
  const state = () => ev(`${READ_LIB} const a = document.activeElement; return { active: a ? { tag: a.tagName.toLowerCase(), cls: String(a.className).slice(0, 60), editable: a.isContentEditable, type: a.getAttribute('type') } : null, focused: ${FOCUSED_CELL}, surfaces: Array.from(document.querySelectorAll(SURF)).map(e => String(e.className)), leafType: app.workspace.activeLeaf.view.getViewType() };`);
  p.afterClick = await state();
  p.keys = [];
  const seq = [['Escape'], ['Escape'], ['Tab'], ['Tab'], ['Tab', { shift: true }], ['ArrowRight'], ['ArrowDown'], ['ArrowLeft'], ['ArrowUp'], ['Enter'], ['Escape'], ['ArrowRight'], ['ArrowRight'], ['ArrowRight'], ['Space'], ['Escape'], ['Home'], ['End'], ['PageDown'], ['Escape']];
  for (const [k, mods] of seq) {
    await c.pressKey(k, mods || {}); await sleep(450);
    const s = await state();
    p.keys.push({ key: k, mods: mods || null, ...s });
    if (s.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Table'); await c.clickAt(cell.x, cell.y); await sleep(500); p.keys[p.keys.length - 1].reopened = true; }
  }
  p.focusShot = await shot('table-keyboard-focus', '.workspace-leaf.mod-active .bases-table-container');
  // typing on a focused (not editing) cell
  await c.clickAt(cell.x, cell.y); await sleep(400); await c.pressKey('Escape'); await sleep(300); await c.pressKey('ArrowRight'); await sleep(300);
  const before = fs.readFileSync(vp('Andesite.md'), 'utf8');
  await c.typeText('7', 40); await sleep(500);
  p.typeOnFocused = { ...(await state()), inputValue: await ev(`const a = document.activeElement; return a && a.value !== undefined ? a.value : null;`) };
  await c.pressKey('Escape'); await sleep(800); await escapeAll(1);
  p.typeOnFocusedFileChanged = fs.readFileSync(vp('Andesite.md'), 'utf8') !== before;
  fs.writeFileSync(vp('Andesite.md'), before); await sleep(1500);
  // Delete / Backspace on a focused cell
  await c.clickAt(cell.x, cell.y); await sleep(400); await c.pressKey('Escape'); await sleep(300); await c.pressKey('ArrowRight'); await sleep(300);
  await c.pressKey('Delete'); await sleep(1200);
  p.deleteOnFocused = { ...(await state()), fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== before, file: fs.readFileSync(vp('Andesite.md'), 'utf8') };
  fs.writeFileSync(vp('Andesite.md'), before); await sleep(1500);
  await escapeAll(2);
  return p;
});

await step('s26-kanban-rest', async (p) => {
  await openBase('Rocks.base'); await switchView('Board'); await dismiss(); await closePopovers();
  const REAL_COLS = `Array.from(document.querySelectorAll('.workspace-leaf.mod-active .bases-kanban-column')).filter(e => getComputedStyle(e).opacity !== '0' && !/^Fj/.test(e.textContent))`;
  const CARD0 = `(${REAL_COLS}).flatMap(cc => Array.from(cc.querySelectorAll('.bases-kanban-card')))[0]`;
  const tc = await ev(`${READ_LIB} const e = ${CARD0}; const t = e.querySelector('.bases-kanban-card-property.mod-title .bases-kanban-card-line'); const b = t.getBoundingClientRect(); return { x: b.x + 8, y: b.y + b.height / 2, cls: String(t.className), text: txt(t), html: collapse(e.querySelector('.bases-kanban-card-property.mod-title').outerHTML).slice(0, 600) };`);
  p.cardTitle = tc;
  await c.mouseMove(tc.x, tc.y); await sleep(500);
  p.cardTitleHover = await ev(`const e = ${CARD0}; const t = e.querySelector('.bases-kanban-card-property.mod-title .bases-kanban-card-line'); return { cursor: getComputedStyle(t).cursor, color: getComputedStyle(t).color, decoration: getComputedStyle(t).textDecorationLine };`);
  await c.clickAt(tc.x, tc.y); await sleep(1000);
  p.cardTitleClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), surfaces: (await surfaces()).map((s) => s.cls) };
  if (p.cardTitleClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
  await closePopovers();
  // a card's non-title property: does it edit in place?
  const pc = await ev(`${READ_LIB} const e = ${CARD0}; const pr = e.querySelectorAll('.bases-kanban-card-property')[1]; const b = pr.querySelector('.bases-kanban-card-line').getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, text: txt(pr) };`);
  await c.clickAt(pc.x, pc.y); await sleep(800);
  p.cardPropertyClick = { target: pc, leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), surfaces: (await surfaces()).map((s) => s.cls), cardHtml: await ev(`${READ_LIB} const e = ${CARD0}; return e ? collapse(e.outerHTML).slice(0, 1500) : null;`), shot: await shot('kanban-card-property-click', '.workspace-leaf.mod-active .view-content, .suggestion-container') };
  if (p.cardPropertyClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
  await escapeAll(2); await closePopovers();
  // a click on the card's blank area (cover / padding)
  const blank = await ev(`const e = ${CARD0}; const b = e.getBoundingClientRect(); return { x: b.right - 10, y: b.bottom - 6 };`);
  await c.clickAt(blank.x, blank.y); await sleep(900);
  p.cardBlankClick = { leafType: await ev(`return app.workspace.activeLeaf.view.getViewType();`), activeFile: await ev(`const f = app.workspace.getActiveFile(); return f ? f.path : null;`), active: await activeElement(), cardShadow: await ev(`const e = ${CARD0}; return e ? getComputedStyle(e).boxShadow : null;`) };
  if (p.cardBlankClick.leafType !== 'bases') { await openBase('Rocks.base'); await switchView('Board'); }
  await closePopovers();
  // card drag to the next column
  const card2 = await ev(`const e = ${CARD0}; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  const col2 = await ev(`const cols = ${REAL_COLS}; const e = cols[1]; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  const d = await c.dragOverOnly(card2.x, card2.y, col2.x, col2.y);
  p.cardDrag = { started: d.started, mimeTypes: d.data && d.data.items ? d.data.items.map((i) => i.mimeType + ':' + String(i.data).slice(0, 60)) : d.error };
  if (d.started) { p.cardDragMid = await ev(`${READ_LIB} return { marks: Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], .workspace-leaf.mod-active [class*="placeholder"], .workspace-leaf.mod-active [class*="is-"], body > [class*="drag"]')).filter(e => !/bases-toolbar|text-icon-button/.test(String(e.className))).map(e => ({ cls: String(e.className), rect: rectOf(e) })).slice(0, 12), bodyCls: document.body.className };`); p.cardDragShot = await shot('kanban-card-drag', '.workspace-leaf.mod-active .view-content'); await d.finish(); }
  // a real drop into the next column: what the note gets
  const b0 = fs.readFileSync(vp('Andesite.md'), 'utf8');
  const d2 = await c.dragAndDrop(card2.x, card2.y, col2.x, col2.y);
  await sleep(1500);
  p.cardDrop = { started: d2.started, fileChanged: fs.readFileSync(vp('Andesite.md'), 'utf8') !== b0, file: fs.readFileSync(vp('Andesite.md'), 'utf8'), columns: await ev(`${READ_LIB} return (${REAL_COLS}).map(e => ({ header: txt(e.querySelector('.bases-group-value')), count: txt(e.querySelector('.bases-kanban-column-count')), cards: Array.from(e.querySelectorAll('.bases-kanban-card .mod-title .bases-kanban-card-line')).map(x => txt(x)) }));`), shot: await shot('kanban-after-drop') };
  fs.writeFileSync(vp('Andesite.md'), b0); await sleep(2000);
  // column header drag
  const ch1 = await ev(`const cols = ${REAL_COLS}; const h = cols[0].querySelector('.bases-kanban-column-header'); const b = h.getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height / 2 };`);
  const ch2 = await ev(`const cols = ${REAL_COLS}; const h = cols[2].querySelector('.bases-kanban-column-header'); const b = h.getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height / 2 };`);
  const d3 = await c.dragOverOnly(ch1.x, ch1.y, ch2.x, ch2.y);
  p.columnDrag = { started: d3.started, mimeTypes: d3.data && d3.data.items ? d3.data.items.map((i) => i.mimeType + ':' + String(i.data).slice(0, 60)) : d3.error };
  if (d3.started) { p.columnDragMid = await ev(`${READ_LIB} return Array.from(document.querySelectorAll('.workspace-leaf.mod-active [class*="drag"], .workspace-leaf.mod-active [class*="drop"], .workspace-leaf.mod-active [class*="placeholder"], body > [class*="drag"]')).filter(e => !/bases-toolbar|text-icon-button/.test(String(e.className))).map(e => ({ cls: String(e.className), rect: rectOf(e) })).slice(0, 12);`); p.columnDragShot = await shot('kanban-column-drag', '.workspace-leaf.mod-active .view-content'); await d3.finish(); }
  const rb = fs.readFileSync(vp('Rocks.base'), 'utf8');
  const d4 = await c.dragAndDrop(ch1.x, ch1.y, ch2.x, ch2.y); await sleep(1500);
  p.columnDrop = { started: d4.started, fileChanged: fs.readFileSync(vp('Rocks.base'), 'utf8') !== rb, file: fs.readFileSync(vp('Rocks.base'), 'utf8') !== rb ? fs.readFileSync(vp('Rocks.base'), 'utf8') : null, columns: await ev(`${READ_LIB} return (${REAL_COLS}).map(e => txt(e.querySelector('.bases-group-value')));`) };
  if (p.columnDrop.fileChanged) { fs.writeFileSync(vp('Rocks.base'), rb); await sleep(2000); }
  // the board's background: right-click and plain click
  const bg = await ev(`const e = document.querySelector('.workspace-leaf.mod-active .bases-kanban-container'); const b = e.getBoundingClientRect(); return { x: b.right - 30, y: b.bottom - 30 };`);
  await c.clickAt(bg.x, bg.y, { button: 'right' }); await sleep(700);
  p.backgroundRightClick = { surfaces: (await surfaces()).map((m) => ({ cls: m.cls, rows: m.rows.filter((r) => r.cls.includes('menu-item')).map((r) => [r.title, r.icon]) })) };
  await dismiss();
  // the column's "Hide column" and the group direction "Manual" from the Group panel
  const col0 = await ev(`const cols = ${REAL_COLS}; const h = cols[0].querySelector('.bases-kanban-column-header'); const b = h.getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height / 2 };`);
  const rb2 = fs.readFileSync(vp('Rocks.base'), 'utf8');
  await c.clickAt(col0.x, col0.y, { button: 'right' }); await sleep(700);
  p.hideColumn = { clicked: await clickMenuRow('Hide column') };
  await sleep(1500); await dismiss();
  p.hideColumn.columns = await ev(`${READ_LIB} return (${REAL_COLS}).map(e => txt(e.querySelector('.bases-group-value')));`);
  p.hideColumn.file = fs.readFileSync(vp('Rocks.base'), 'utf8') !== rb2 ? fs.readFileSync(vp('Rocks.base'), 'utf8') : null;
  p.hideColumn.groupPanel = null;
  await openPanel('bases-toolbar-group-menu');
  p.hideColumn.groupPanel = await surfaces();
  p.hideColumn.groupPanelShot = await shot('kanban-group-panel-after-hide', '.menu');
  await dismiss();
  fs.writeFileSync(vp('Rocks.base'), rb2); await sleep(2000);
  // "Add item" from the column menu
  const files0 = vaultFiles();
  await c.clickAt(col0.x, col0.y, { button: 'right' }); await sleep(700);
  p.addItem = { clicked: await clickMenuRow('Add item') };
  await sleep(2000);
  p.addItem.created = vaultFiles().filter((x) => !files0.includes(x)).map((x) => ({ name: x, text: fs.readFileSync(vp(x), 'utf8') }));
  p.addItem.surfaces = (await surfaces()).map((s) => s.cls);
  await closePopovers();
  return p;
});

out.vaultFilesAtEnd = vaultFiles();
out.rocksAtEnd = fs.readFileSync(vp('Rocks.base'), 'utf8');
out.shotFiles = fs.readdirSync(SHOTS).sort();
save();
c.close();
process.exit(0);
