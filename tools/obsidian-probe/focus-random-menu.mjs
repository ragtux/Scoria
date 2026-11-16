// The three commands the 2026-11-09 table left "done, to confirm"
// ([[Development#The unrunnable commands, 2026-11-09]]), on what
// app-window-commands.mjs and note-composer-unique.mjs did not watch:
//
//   s00  version, the three commands' availability
//   s01  editor:focus over a base and a PDF with a note behind them in the
//        same group; over a canvas, a base, a PDF and an empty tab in a second
//        group with the note on screen in the first; a reading-view note
//        beside an editing one; the most recent note hidden behind a canvas
//        with another note on screen; the search field holding DOM focus
//   s02  editor:context-menu where the menu cannot fit below or to the right
//        of the caret (a caret near the window's bottom, one near its right
//        edge), and Source mode with a selection
//   s03  random-note: which kinds come up (notes beside a canvas, a base and a
//        PDF), from an empty tab, a canvas tab, a pinned tab, the search
//        field, and a second group; the history it leaves
//   s04  the editor menu's three submenus, Format, Paragraph and Insert: their
//        rows with and without a selection, in Live Preview and Source mode,
//        and what each row does to `alpha beta gamma` (bytes, selection, any
//        modal), every row on a freshly seeded note
//   s06  the command each submenu row is named after, run by id on the same
//        fresh note, selection and caret, to compare with the row's bytes
//   s07  which submenu rows carry a check, by the line the caret is on
//   s05  Format > Highlight's own submenu: its rows, and what each writes
//        over a selection and at a caret
//
// Phases: fixtures | run. ONLY=<regex> picks steps of `run`.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }

const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
const out = { startedAt: new Date().toISOString(), env: { ONLY: process.env.ONLY || null, vault: VAULT }, steps: {}, order: [] };
all[phase + (process.env.RUN_LABEL ? ':' + process.env.RUN_LABEL : '')] = out;
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
save();
const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return;
  out.order.push(name);
  const r = {};
  out.steps[name] = { ok: null, partial: r }; save();
  try { await withTimeout(fn(r), ms, name); out.steps[name] = { ok: true, ...r }; }
  catch (e) { out.steps[name] = { ok: false, error: String(e && e.stack ? e.stack : e), ...r }; }
  save();
}

const vp = (rel) => path.join(VAULT, rel);

// A one-page PDF, offsets computed so the xref is exact.
function tinyPdf() {
  const objs = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
    null,
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
  ];
  const stream = 'BT /F1 18 Tf 20 100 Td (Probe) Tj ET';
  objs[3] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  let s = '%PDF-1.4\n';
  const offs = [];
  objs.forEach((o, i) => { offs.push(s.length); s += `${i + 1} 0 obj\n${o}\nendobj\n`; });
  const x = s.length;
  s += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n` + offs.map((o) => String(o).padStart(10, '0') + ' 00000 n \n').join('');
  s += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${x}\n%%EOF\n`;
  return s;
}

const FMT = '# Fmt\n\nalpha beta gamma\n\nsecond line\n';

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Note A.md'), 'Alpha body\n');
  fs.writeFileSync(vp('Note B.md'), 'Bravo body\n');
  fs.writeFileSync(vp('Note C.md'), 'Charlie body\n');
  fs.writeFileSync(vp('Board.canvas'), '{"nodes":[],"edges":[]}');
  fs.writeFileSync(vp('Table.base'), 'views:\n  - type: table\n    name: Table\n');
  fs.writeFileSync(vp('Doc.pdf'), tinyPdf());
  const long = ['# Long', ''];
  for (let i = 1; i <= 120; i++) long.push(`line ${i} with a few words on it`);
  long.push('');
  fs.writeFileSync(vp('Long.md'), long.join('\n'));
  const wide = 'word '.repeat(400).trim();
  fs.writeFileSync(vp('Wide.md'), `# Wide\n\n${wide}\n\nshort selected words here\n`);
  fs.writeFileSync(vp('Fmt.md'), FMT);
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  save();
  process.exit(0);
}

const c = await connect();
out.vaultPath = await c.vaultPath();
save();
await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  avail(id) { return app.commands.listCommands().some((k) => k.id === id); },
  run(id) { let ran = null, err = null; try { ran = app.commands.executeCommandById(id); } catch (x) { err = String(x); } return { ran, err }; },
  notices() { return Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent); },
  leafDesc(l) { if (!l) return null; const v = l.view; return { type: v.getViewType(), file: v.file ? v.file.path : null, mode: v.getMode ? v.getMode() : null, pinned: !!l.pinned, group: l.parent ? app.workspace.rootSplit.children.indexOf(l.parent) : null }; },
  active() {
    const l = app.workspace.activeLeaf; const ae = document.activeElement;
    return { leaf: this.leafDesc(l),
      activeElement: ae ? { tag: ae.tagName, cls: String(ae.className).slice(0, 100), inCm: !!(ae.closest && ae.closest('.cm-editor')),
        leafType: ae.closest && ae.closest('.workspace-leaf-content') ? ae.closest('.workspace-leaf-content').getAttribute('data-type') : null,
        leafFile: (() => { const lc = ae.closest && ae.closest('.workspace-leaf'); if (!lc) return null; let f = null; app.workspace.iterateAllLeaves((x) => { if (x.containerEl === lc && x.view.file) f = x.view.file.path; }); return f; })() } : null };
  },
  md() { return app.workspace.getLeavesOfType('markdown').map((l) => ({ file: l.view.file && l.view.file.path, mode: l.view.getMode(), hasFocus: l.view.editor ? l.view.editor.hasFocus() : null, holdsDomFocus: l.view.containerEl.contains(document.activeElement), isActive: l === app.workspace.activeLeaf, visible: !!l.view.containerEl.offsetParent, group: app.workspace.rootSplit.children.indexOf(l.parent) })); },
  tabs() { return app.workspace.rootSplit.children.map((g) => (g.children || []).map((l) => this.leafDesc(l).type + ':' + (this.leafDesc(l).file || '') + (l === app.workspace.activeLeaf ? '*' : '') + (g.currentTab !== undefined && g.children[g.currentTab] === l ? '(shown)' : ''))); },
  rootLeaves() { const out = []; app.workspace.iterateAllLeaves((l) => { if (l.getRoot() === app.workspace.rootSplit) out.push(l); }); return out; },
  reset() {
    // One group with one empty tab.
    for (const l of this.rootLeaves()) { l.setPinned(false); l.detach(); }
    return app.workspace.getLeaf(false);
  },
  menu() {
    const m = document.querySelector('.menu'); if (!m) return null;
    const r = m.getBoundingClientRect();
    return { rect: { x: r.x, y: r.y, w: r.width, h: r.height }, cls: m.className,
      rows: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => x.classList.contains('menu-separator') ? '---' :
        ((x.querySelector('.menu-item-title') || x).textContent || '').trim() + (x.classList.contains('is-disabled') ? ' (disabled)' : '')) };
  },
  closeMenus() { document.querySelectorAll('.menu').forEach((m) => m.remove()); },
};
return true;`);
const ev = (body) => c.evaluate(body);
const avail = (id) => ev(`return window.__p.avail(${JSON.stringify(id)});`);
const run = (id) => ev(`return window.__p.run(${JSON.stringify(id)});`);
const active = () => ev(`return window.__p.active();`);
const openIn = (file, how) => ev(`const l = app.workspace.getLeaf(${JSON.stringify(how)}); ${file ? `await l.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(file)}));` : ''} app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(600); return window.__p.leafDesc(l);`);
const reset = () => ev(`window.__p.reset(); await window.__p.sleep(400); return true;`);

await step('s00-meta', async (r) => {
  r.enable = await ev(`const p = app.internalPlugins.plugins['random-note']; if (p && !p.enabled) await p.enable(true); await window.__p.sleep(500); return { randomNote: !!(p && p.enabled) };`);
  r.version = await ev(`return { app: require('electron').ipcRenderer.sendSync('version'), api: app.apiVersion || null, ua: navigator.userAgent, title: document.title, inner: [innerWidth, innerHeight] };`);
  r.commands = await ev(`return ['editor:focus', 'editor:context-menu', 'random-note'].map((id) => { const k = app.commands.commands[id]; return k ? { id, name: k.name, icon: k.icon || null, kinds: ['callback', 'checkCallback', 'editorCallback', 'editorCheckCallback'].filter((x) => k[x]) } : { id, missing: true }; });`);
});

await step('s01-editor-focus', async (r) => {
  const fcase = async (label) => {
    const o = { tabs: await ev(`return window.__p.tabs();`), before: await active(), available: await avail('editor:focus') };
    o.run = await run('editor:focus');
    await sleep(400);
    o.after = await active();
    o.tabsAfter = await ev(`return window.__p.tabs();`);
    o.md = await ev(`return window.__p.md();`);
    await sleep(1200);
    o.later = { active: await active(), activeEditorFile: await ev(`const e = app.workspace.activeEditor; return e && e.file ? e.file.path : null;`) };
    r[label] = o; save();
  };
  // One group: Note A behind a base, then behind a PDF.
  for (const [label, file] of [['sameGroupBase', 'Table.base'], ['sameGroupPdf', 'Doc.pdf']]) {
    await reset();
    await openIn('Note A.md', false);
    await openIn(file, 'tab');
    await fcase(label);
  }
  // Two groups: Note A on screen in the first, the second's active tab is X.
  for (const [label, file] of [['otherGroupCanvas', 'Board.canvas'], ['otherGroupBase', 'Table.base'], ['otherGroupPdf', 'Doc.pdf'], ['otherGroupEmpty', null]]) {
    await reset();
    await openIn('Note A.md', false);
    await ev(`const l = app.workspace.getLeaf('split', 'vertical'); ${file ? `await l.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(file)}));` : ''} app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(700); return true;`);
    await fcase(label);
  }
  // Where a key goes after the command, with the canvas still the active leaf.
  await reset();
  await openIn('Note A.md', false);
  await ev(`const l = app.workspace.getLeaf('split', 'vertical'); await l.openFile(app.vault.getAbstractFileByPath('Board.canvas')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(700); return true;`);
  await fcase('otherGroupCanvasThenType');
  await c.typeText('Z');
  await sleep(600);
  r.otherGroupCanvasThenType.typed = { active: await active(), noteA: await ev(`return app.workspace.getLeavesOfType('markdown').map((l) => l.view.editor.getValue());`),
    headerActive: await ev(`return Array.from(document.querySelectorAll('.mod-root .workspace-tabs')).map((t) => t.classList.contains('mod-active'));`) };
  await ev(`const l = app.workspace.getLeavesOfType('markdown')[0]; l.view.editor.setValue('Alpha body\\n'); await l.view.save(); return true;`);
  // Two groups, two notes: Note A editing on the left, Note B reading on the right, B active.
  await reset();
  await openIn('Note A.md', false);
  await ev(`const l = app.workspace.getLeaf('split', 'vertical'); await l.openFile(app.vault.getAbstractFileByPath('Note B.md')); const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'preview' }); await l.setViewState(s); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(600); return true;`);
  await fcase('readingBesideEditing');
  // The most recent note hidden: left [Note A], right [Note B, canvas];
  // activate A, then B, then the canvas over B.
  await reset();
  await openIn('Note A.md', false);
  await ev(`const l = app.workspace.getLeaf('split', 'vertical'); await l.openFile(app.vault.getAbstractFileByPath('Note B.md')); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(500); return true;`);
  await openIn('Board.canvas', 'tab');
  await fcase('recentNoteHiddenOtherOnScreen');
  // Left [Note A] visible, right [Note B] with the canvas over it; A was the last note active.
  await reset();
  await ev(`const a = app.workspace.getLeaf(false); await a.openFile(app.vault.getAbstractFileByPath('Note B.md')); await window.__p.sleep(300);
    const r = app.workspace.getLeaf('split', 'vertical'); await r.openFile(app.vault.getAbstractFileByPath('Board.canvas')); await window.__p.sleep(300);
    const left = app.workspace.getLeaf('split', 'vertical'); await left.openFile(app.vault.getAbstractFileByPath('Note C.md')); app.workspace.setActiveLeaf(left, { focus: true }); await window.__p.sleep(400);
    app.workspace.setActiveLeaf(r, { focus: true }); await window.__p.sleep(500); return true;`);
  await fcase('threeGroupsLastNoteC');
  // The search field holding DOM focus, Note A in the main area.
  await reset();
  await openIn('Note A.md', false);
  await run('global-search:open');
  await sleep(500);
  await c.typeText('alpha');
  await sleep(500);
  r.searchFocused = await active();
  await fcase('fromSearchField');
  // The search field, with Note A behind a canvas.
  await openIn('Board.canvas', 'tab');
  await run('global-search:open');
  await sleep(500);
  await fcase('fromSearchFieldCanvasShown');
});

await step('s02-context-menu-edges', async (r) => {
  const mcase = async (label, file, source, setup) => {
    await reset();
    await openIn(file, false);
    await ev(`const l = app.workspace.activeLeaf; const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'source', source: ${source} }); await l.setViewState(s); await window.__p.sleep(500); return true;`);
    const o = {};
    o.setup = await ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ${setup}; await window.__p.sleep(400);
      const cm = ed.cm; const head = cm.state.selection.main.head; const co = cm.coordsAtPos(head);
      return { cursor: ed.getCursor(), sel: ed.getSelection(), coords: co ? { left: co.left, right: co.right, top: co.top, bottom: co.bottom } : null, inner: [innerWidth, innerHeight], doc: [document.documentElement.clientWidth, document.documentElement.clientHeight] };`);
    o.run = await run('editor:context-menu');
    await sleep(700);
    o.menu = await ev(`return window.__p.menu();`);
    await ev(`window.__p.closeMenus(); return true;`);
    r[label] = o; save();
  };
  // A line near the window's bottom: find the visible line with the largest bottom.
  const nearBottom = `let best = null; for (let n = 1; n <= ed.cm.state.doc.lines; n++) { const ln = ed.cm.state.doc.line(n); if (ln.length < 3) continue; const pos = ln.from + 2; const co = ed.cm.coordsAtPos(pos); if (co && co.bottom < innerHeight - 4) best = n; } ed.setCursor({ line: best - 1, ch: 2 })`;
  await mcase('lpNearBottom', 'Long.md', false, nearBottom);
  await mcase('sourceNearBottom', 'Long.md', true, nearBottom);
  // A real right click at the same places, to see whether the menu of a
  // click flips the same way.
  const rclick = async (label, file, setup) => {
    await mcase(label + 'Setup', file, false, setup);
    const co = r[label + 'Setup'].setup.coords;
    const x = Math.round(co.left) + 2, y = Math.round((co.top + co.bottom) / 2);
    await c.clickAt(x, y, { button: 'right' });
    await sleep(700);
    r[label] = { at: [x, y], menu: await ev(`return window.__p.menu();`), inner: await ev(`return [innerWidth, innerHeight];`) };
    await ev(`window.__p.closeMenus(); return true;`);
    save();
  };
  await rclick('rightClickNearBottom', 'Long.md', nearBottom);
  // Mid-screen after a scroll, for the anchor in a scrolled editor.
  await mcase('lpScrolledMiddle', 'Long.md', false, `ed.setCursor({ line: 80, ch: 4 }); ed.scrollIntoView({ from: { line: 80, ch: 0 }, to: { line: 80, ch: 0 } }, true)`);
  // The right edge: readable line length off, the caret at the end of the first visual row.
  await ev(`app.vault.setConfig('readableLineLength', false); await window.__p.sleep(300); return true;`);
  await mcase('lpNearRight', 'Wide.md', false, `const line = ed.cm.state.doc.line(3); let best = line.from; const top0 = ed.cm.coordsAtPos(line.from).top; for (let p = line.from; p < line.to; p += 1) { const co = ed.cm.coordsAtPos(p); if (co.top !== top0) break; best = p; } ed.setCursor(ed.offsetToPos(best))`);
  await rclick('rightClickNearRight', 'Wide.md', `const line = ed.cm.state.doc.line(3); let best = line.from; const top0 = ed.cm.coordsAtPos(line.from).top; for (let p = line.from; p < line.to; p += 1) { const co = ed.cm.coordsAtPos(p); if (co.top !== top0) break; best = p; } ed.setCursor(ed.offsetToPos(best - 6))`);
  await ev(`app.vault.setConfig('readableLineLength', true); await window.__p.sleep(300); return true;`);
  await mcase('sourceSelection', 'Wide.md', true, `ed.setSelection({ line: 4, ch: 6 }, { line: 4, ch: 20 })`);
  await mcase('lpSelectionBackward', 'Wide.md', false, `ed.setSelection({ line: 4, ch: 20 }, { line: 4, ch: 6 })`);
});

await step('s03-random-note', async (r) => {
  const rcase = async (label, n = 1) => {
    const o = { before: await active(), tabs: await ev(`return window.__p.tabs();`), available: await avail('random-note'), picks: [] };
    for (let i = 0; i < n; i++) { await run('random-note'); await sleep(350); o.picks.push((await active()).leaf); }
    o.after = await active();
    o.tabsAfter = await ev(`return window.__p.tabs();`);
    o.history = await ev(`const l = app.workspace.activeLeaf; return { back: (l.history.backHistory || []).map((h) => h.state && h.state.state && h.state.state.file || h.title), forward: (l.history.forwardHistory || []).length };`);
    o.notices = await ev(`return window.__p.notices();`);
    r[label] = o; save();
  };
  await reset();
  await openIn('Note A.md', false);
  await rcase('kinds', 40);
  r.kindsCount = {};
  for (const p of r.kinds.picks) r.kindsCount[p.file] = (r.kindsCount[p.file] || 0) + 1;
  await reset();
  await rcase('fromEmptyTab');
  await reset();
  await openIn('Board.canvas', false);
  await rcase('fromCanvas');
  await reset();
  await openIn('Note A.md', false);
  await ev(`app.workspace.activeLeaf.setPinned(true); await window.__p.sleep(300); return true;`);
  await rcase('fromPinned', 3);
  await reset();
  await openIn('Note A.md', false);
  await run('global-search:open');
  await sleep(400);
  await rcase('fromSearchField');
  await reset();
  await openIn('Note A.md', false);
  await ev(`const l = app.workspace.getLeaf('split', 'vertical'); await l.openFile(app.vault.getAbstractFileByPath('Board.canvas')); await window.__p.sleep(300); app.workspace.setActiveLeaf(app.workspace.getLeavesOfType('markdown')[0], { focus: true }); await window.__p.sleep(400); return true;`);
  await rcase('splitFromLeftNote', 3);
  // The ribbon's button, again, for its place in this vault's plugins.
  r.ribbon = await ev(`return Array.from(document.querySelectorAll('.side-dock-actions .side-dock-ribbon-action')).map((b) => b.getAttribute('aria-label') + ' [' + (b.querySelector('svg') ? b.querySelector('svg').getAttribute('class') : '') + ']');`);
});

await step('s04-submenus', async (r) => {
  const PARENTS = (process.env.PARENTS || 'Format,Paragraph,Insert').split(',');
  // A fresh note per case: rewriting one note under its open editor merges
  // the previous case's bytes back in ("modified externally, merging").
  let seedN = 0;
  const seed = async (source) => {
    seedN += 1;
    return ev(`const f = await app.vault.create('Fmt ' + ${seedN} + '.md', ${JSON.stringify(FMT)}); const olds = window.__p.rootLeaves(); const l = app.workspace.getLeaf('tab'); await l.openFile(f); for (const o of olds) if (o !== l) o.detach(); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(200);
      const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'source', source: ${source} }); await l.setViewState(s); await window.__p.sleep(400);
      return l.view.editor.getValue() === ${JSON.stringify(FMT)};`);
  };
  const place = (sel) => ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ${sel ? 'ed.setSelection({ line: 2, ch: 6 }, { line: 2, ch: 10 })' : 'ed.setCursor({ line: 2, ch: 8 })'}; await window.__p.sleep(200); return { sel: ed.getSelection(), cur: ed.getCursor() };`);
  const openSub = async (parent) => {
    await ev(`window.__p.closeMenus(); return true;`);
    await run('editor:context-menu');
    await sleep(500);
    const rr = await ev(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim() === ${JSON.stringify(parent)});
      if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!rr) return null;
    await c.mouseMove(rr.cx - 10, rr.cy);
    await sleep(150);
    await c.mouseMove(rr.cx, rr.cy);
    await sleep(700);
    return ev(`const ms = Array.from(document.querySelectorAll('.menu')); if (ms.length < 2) return { menus: ms.length };
      const m = ms[ms.length - 1]; const b = m.getBoundingClientRect();
      return { menus: ms.length, rect: { x: b.x, y: b.y, w: b.width, h: b.height }, rows: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => x.classList.contains('menu-separator') ? '---' :
        { title: ((x.querySelector('.menu-item-title') || x).textContent || '').trim(), icon: x.querySelector('.menu-item-icon svg') ? x.querySelector('.menu-item-icon svg').getAttribute('class') : null,
          disabled: x.classList.contains('is-disabled'), checked: !!x.querySelector('.mod-checked, .menu-item-icon.mod-checked') || x.classList.contains('mod-checked'), sub: x.classList.contains('has-submenu') || !!x.querySelector('.mod-submenu') }) };`);
  };
  await reset();
  await openIn('Fmt.md', false);
  // The rows, in four contexts.
  for (const [label, source, sel] of [['lp', false, false], ['lpSel', false, true], ['src', true, false], ['srcSel', true, true]]) {
    await seed(source);
    await place(sel);
    r[label] = {};
    for (const parent of PARENTS) { r[label][parent] = await openSub(parent); save(); }
    await ev(`window.__p.closeMenus(); return true;`);
  }
  // Each row's effect, Live Preview, with and without a selection.
  for (const [label, sel] of [['effectSel', true], ['effectCaret', false]]) {
    r[label] = {};
    for (const parent of PARENTS) {
      const rows = ((r[sel ? 'lpSel' : 'lp'][parent] || r[sel ? 'srcSel' : 'src'][parent] || {}).rows || []).filter((x) => x !== '---');
      r[label][parent] = [];
      for (let j = rows.length - 1; j >= 0; j--) {
        if (rows[j].sub) { r[label][parent].push({ title: rows[j].title, skipped: 'has a submenu' }); continue; }
        const seeded = await seed(false);
        const before = { seeded, ...(await place(sel)) };
        await openSub(parent);
        const hit = await ev(`const ms = Array.from(document.querySelectorAll('.menu')); const m = ms[ms.length - 1]; if (!m || ms.length < 2) return null;
          const it = Array.from(m.querySelectorAll('.menu-item'))[${j}]; if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2, title: ((it.querySelector('.menu-item-title') || it).textContent || '').trim() };`);
        if (!hit) { r[label][parent].push({ title: rows[j].title, error: 'row not found' }); continue; }
        await c.mouseMove(hit.cx, hit.cy); await sleep(150);
        await c.clickAt(hit.cx, hit.cy);
        await sleep(700);
        const after = await ev(`const ed = app.workspace.activeLeaf.view.editor; const m = document.querySelector('.modal-container .modal');
          return { value: ed.getValue(), sel: ed.getSelection(), from: ed.getCursor('from'), to: ed.getCursor('to'), focusInCm: !!(document.activeElement && document.activeElement.closest('.cm-editor')),
            modal: m ? { title: (m.querySelector('.modal-title') || {}).textContent || null, text: m.textContent.slice(0, 200), cls: m.className } : null,
            prompt: document.querySelector('.prompt') ? document.querySelector('.prompt').textContent.slice(0, 200) : null,
            suggest: document.querySelector('.suggestion-container') ? document.querySelector('.suggestion-container').textContent.slice(0, 200) : null,
            notices: window.__p.notices() };`);
        r[label][parent].push({ title: hit.title, before, after });
        save();
        for (let k = 0; k < 2; k++) { await c.pressKey('Escape'); await sleep(200); }
        await ev(`document.querySelectorAll('.modal-container').forEach((m) => m.remove()); document.querySelectorAll('.notice').forEach((n) => n.remove()); window.__p.closeMenus(); return true;`);
      }
    }
  }
}, 400000);

const ROW_COMMANDS = {
  Bold: 'editor:toggle-bold', Italic: 'editor:toggle-italics', Strikethrough: 'editor:toggle-strikethrough', Code: 'editor:toggle-code',
  Math: 'editor:toggle-inline-math', Comment: 'editor:toggle-comments', 'Clear formatting': 'editor:clear-formatting',
  'Bullet list': 'editor:toggle-bullet-list', 'Numbered list': 'editor:toggle-numbered-list', 'Task list': 'editor:toggle-checklist-status',
  'Heading 1': 'editor:set-heading-1', 'Heading 2': 'editor:set-heading-2', 'Heading 3': 'editor:set-heading-3', 'Heading 4': 'editor:set-heading-4',
  'Heading 5': 'editor:set-heading-5', 'Heading 6': 'editor:set-heading-6', Body: 'editor:set-heading-0', Quote: 'editor:toggle-blockquote',
  Footnote: 'editor:insert-footnote', Table: 'editor:insert-table', Callout: 'editor:insert-callout', 'Horizontal rule': 'editor:insert-horizontal-rule',
  'Code block': 'editor:insert-codeblock', 'Math block': 'editor:insert-mathblock', 'New base': 'bases:insert',
};
await step('s06-row-commands', async (r) => {
  let n = 0;
  const seed = () => { n += 1; return ev(`const f = await app.vault.create('Cmd ' + ${n} + '.md', ${JSON.stringify(FMT)}); const olds = window.__p.rootLeaves(); const l = app.workspace.getLeaf('tab'); await l.openFile(f); for (const o of olds) if (o !== l) o.detach(); app.workspace.setActiveLeaf(l, { focus: true }); await window.__p.sleep(200);
    const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'source', source: false }); await l.setViewState(s); await window.__p.sleep(400); return l.view.editor.getValue() === ${JSON.stringify(FMT)};`); };
  const place = (sel) => ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ${sel ? 'ed.setSelection({ line: 2, ch: 6 }, { line: 2, ch: 10 })' : 'ed.setCursor({ line: 2, ch: 8 })'}; await window.__p.sleep(200); return { sel: ed.getSelection() };`);
  await reset();
  r.commandNames = await ev(`return Object.fromEntries(${JSON.stringify(Object.values(ROW_COMMANDS))}.map((id) => [id, app.commands.commands[id] ? app.commands.commands[id].name : null]));`);
  for (const [label, sel] of [['sel', true], ['caret', false]]) {
    r[label] = {};
    for (const [title, id] of Object.entries(ROW_COMMANDS).reverse()) {
      const seeded = await seed();
      await place(sel);
      const ran = await run(id);
      await sleep(600);
      r[label][title] = { id, seeded, ran, after: await ev(`const ed = app.workspace.activeLeaf.view.editor; return { value: ed.getValue(), sel: ed.getSelection(), from: ed.getCursor('from'), to: ed.getCursor('to') };`) };
      save();
      for (let k = 0; k < 2; k++) { await c.pressKey('Escape'); await sleep(150); }
      await ev(`document.querySelectorAll('.modal-container').forEach((m) => m.remove()); return true;`);
    }
  }
}, 300000);

const KINDS = '# One\n\n### Three\n\n- bullet\n\n1. numbered\n\n- [ ] task\n\n> quote\n\nplain **bold** and *it* and ==hl== and `code` and %%c%% and $m$\n';
await step('s07-checks', async (r) => {
  await reset();
  await ev(`const f = await app.vault.create('Kinds.md', ${JSON.stringify(KINDS)}); const l = app.workspace.activeLeaf; await l.openFile(f);
    const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'source', source: true }); await l.setViewState(s); await window.__p.sleep(500); return true;`);
  const cases = [['h1', 0, 3], ['h3', 2, 6], ['bullet', 4, 4], ['numbered', 6, 5], ['task', 8, 8], ['quote', 10, 4], ['plain', 12, 2],
    ['inBold', 12, 10], ['inItalic', 12, 21], ['inHighlight', 12, 32], ['inCode', 12, 43], ['inComment', 12, 52], ['inMath', 12, 61]];
  for (const [label, line, ch] of cases) {
    await ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor({ line: ${line}, ch: ${ch} }); await window.__p.sleep(200); return true;`);
    r[label] = { line: await ev(`const ed = app.workspace.activeLeaf.view.editor; return ed.getLine(${line});`) };
    for (const parent of ['Format', 'Paragraph']) {
      await ev(`window.__p.closeMenus(); return true;`);
      await run('editor:context-menu'); await sleep(500);
      const rr = await ev(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim() === ${JSON.stringify(parent)});
        if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
      if (!rr) { r[label][parent] = null; continue; }
      await c.mouseMove(rr.cx - 10, rr.cy); await sleep(150); await c.mouseMove(rr.cx, rr.cy); await sleep(700);
      r[label][parent] = await ev(`const ms = document.querySelectorAll('.menu'); if (ms.length < 2) return null; return Array.from(ms[1].querySelectorAll('.menu-item')).filter((x) => x.classList.contains('mod-checked') || x.querySelector('.mod-checked')).map((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim());`);
      save();
    }
    await ev(`window.__p.closeMenus(); return true;`);
  }
}, 200000);

await step('s05-highlight', async (r) => {
  let n = 0;
  const seed = () => { n += 1; return ev(`const f = await app.vault.create('Hl ' + ${n} + '.md', ${JSON.stringify(FMT)}); const l = app.workspace.activeLeaf; await l.openFile(f);
    const s = l.getViewState(); s.state = Object.assign({}, s.state, { mode: 'source', source: false }); await l.setViewState(s); await window.__p.sleep(400); return l.view.editor.getValue() === ${JSON.stringify(FMT)};`); };
  const place = (sel) => ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ${sel ? 'ed.setSelection({ line: 2, ch: 6 }, { line: 2, ch: 10 })' : 'ed.setCursor({ line: 2, ch: 8 })'}; await window.__p.sleep(200); return { sel: ed.getSelection() };`);
  const hoverRow = async (title, depth) => {
    const rr = await ev(`const ms = Array.from(document.querySelectorAll('.menu')); const m = ms[${depth}]; if (!m) return null;
      const it = Array.from(m.querySelectorAll('.menu-item')).find((x) => ((x.querySelector('.menu-item-title') || x).textContent || '').trim() === ${JSON.stringify(title)});
      if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
    if (!rr) return false;
    await c.mouseMove(rr.cx - 10, rr.cy); await sleep(150); await c.mouseMove(rr.cx, rr.cy); await sleep(700);
    return true;
  };
  const openHl = async () => {
    await ev(`window.__p.closeMenus(); return true;`);
    await run('editor:context-menu'); await sleep(500);
    if (!(await hoverRow('Format', 0))) return { error: 'no Format' };
    if (!(await hoverRow('Highlight', 1))) return { error: 'no Highlight' };
    return ev(`const ms = Array.from(document.querySelectorAll('.menu')); if (ms.length < 3) return { menus: ms.length };
      const m = ms[2]; return { menus: ms.length, rows: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((x) => x.classList.contains('menu-separator') ? '---' :
        { title: ((x.querySelector('.menu-item-title') || x).textContent || '').trim(), icon: x.querySelector('.menu-item-icon svg') ? x.querySelector('.menu-item-icon svg').getAttribute('class') : null,
          iconHtml: (x.querySelector('.menu-item-icon') || {}).innerHTML ? x.querySelector('.menu-item-icon').innerHTML.slice(0, 300) : null, disabled: x.classList.contains('is-disabled') }) };`);
  };
  await reset();
  r.seed0 = await seed();
  await place(true);
  r.rows = await openHl();
  save();
  const rows = ((r.rows || {}).rows || []).filter((x) => x !== '---');
  for (const [label, sel] of [['sel', true], ['caret', false]]) {
    r[label] = [];
    for (let j = 0; j < rows.length; j++) {
      const seeded = await seed();
      const before = await place(sel);
      await openHl();
      const hit = await ev(`const m = document.querySelectorAll('.menu')[2]; if (!m) return null; const it = Array.from(m.querySelectorAll('.menu-item'))[${j}]; if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
      if (!hit) { r[label].push({ title: rows[j].title, error: 'row not found' }); continue; }
      await c.mouseMove(hit.cx, hit.cy); await sleep(150);
      await c.clickAt(hit.cx, hit.cy); await sleep(700);
      const after = await ev(`const ed = app.workspace.activeLeaf.view.editor; return { value: ed.getValue(), sel: ed.getSelection(), from: ed.getCursor('from'), to: ed.getCursor('to') };`);
      r[label].push({ title: rows[j].title, seeded, before, after });
      save();
      await c.pressKey('Escape'); await sleep(200);
      await ev(`window.__p.closeMenus(); return true;`);
    }
  }
}, 200000);

c.close();
process.exit(0);
