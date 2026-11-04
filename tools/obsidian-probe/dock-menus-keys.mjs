// The right dock's Tags, All properties and Outline panes, second pass: the
// row context menus under a real right click (Input.dispatchMouseEvent with
// button "right", the pointer moved there first), with a capture listener on
// the document recording whether the contextmenu event reached the page and
// whether anything called preventDefault on it; the keyboard of each pane
// after focusing it three ways (its tab header, a click in its empty space,
// a click on a row), recording focus, row classes, the tree's focused item and
// every side effect per key; and the computed style of the outline filter's
// `span.suggestion-highlight` in both themes, with the stylesheet rules that
// match it. The All properties row, whose menu is known, is the control.
// Phases: fixtures | run   (node dock-menus-keys.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run'].includes(phase)) { console.error('phase must be fixtures|run'); process.exit(2); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.dockMenusKeys = all.dockMenusKeys || {};
const out = all.dockMenusKeys[phase + (process.env.ONLY ? ':' + process.env.ONLY : '')] = { startedAt: new Date().toISOString(), steps: {}, order: [] };
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
async function step(name, fn) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now();
  try { const v = await fn(); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e) }; }
  save();
  return out.steps[name];
}
const vp = (rel) => path.join(VAULT, rel);

if (phase === 'fixtures') {
  const filler = (label, n) => Array.from({ length: n }, (_, i) => `${label} body line ${i + 1} with some words.`).join('\n');
  fs.writeFileSync(vp('Alpha.md'), ['---', 'tags:', '  - project/alpha', '  - common', 'status: draft', '---', '# Alpha', '', 'Inline #common and #project/beta/deep.', ''].join('\n'));
  fs.writeFileSync(vp('Beta.md'), ['---', 'tags: [project/beta, common]', 'count: 7', '---', '# Beta', '', '#topic and #Lambda.', ''].join('\n'));
  fs.writeFileSync(vp('Outline.md'), [
    '# Top heading', filler('top', 10),
    '## Second level', filler('second', 10),
    '### Third level under second', filler('third', 10),
    '## Another second', filler('another', 10),
    '# Second top', filler('second top', 10),
    '## Last', filler('last', 10),
  ].join('\n') + '\n');
  out.files = fs.readdirSync(VAULT);
  save();
  process.exit(0);
}

const c = await connect();
out.version = await c.evaluate(`return { app: (window.require && window.require('electron').ipcRenderer.sendSync('version')) || null, apiVersion: window.require ? (() => { try { return window.require('obsidian').apiVersion; } catch { return null; } })() : null, ua: navigator.userAgent, nativeMenus: app.vault.getConfig('nativeMenus'), platform: process.platform };`);
save();

const H = `
const cs = (el, props) => { const s = getComputedStyle(el); const o = {}; for (const p of props) o[p] = s.getPropertyValue(p); return o; };
const rect = (el) => { const r = el.getBoundingClientRect(); return [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; };
const leafOf = (t) => app.workspace.getLeavesOfType(t)[0];
const paneOf = (t) => leafOf(t).view.containerEl;
const menuDom = () => Array.from(document.querySelectorAll('.menu')).map(m => ({ cls: m.className, rect: rect(m), items: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map(i => ({ cls: i.className, text: (i.querySelector('.menu-item-title') || i).textContent.trim(), icon: (i.querySelector('.menu-item-icon svg') || i.querySelector('svg') || { getAttribute: () => null }).getAttribute('class') })) }));
const ae = () => { const a = document.activeElement; if (!a) return null; const leaf = a.closest('.workspace-leaf-content'); return { tag: a.tagName, cls: typeof a.className === 'string' ? a.className : '', text: (a.textContent || '').trim().slice(0, 40), leafType: leaf && leaf.getAttribute('data-type'), tabindex: a.getAttribute('tabindex') }; };
const rowsState = (t) => Array.from(paneOf(t).querySelectorAll('.tree-item-self')).filter(s => s.getBoundingClientRect().width > 0).map(s => ({ text: s.textContent.trim().slice(0, 30), cls: s.className, itemCls: s.parentElement.className })).filter(r => /focus|select|active|is-being|has-/.test(r.cls + ' ' + r.itemCls));
const mdLeaf = () => app.workspace.getLeavesOfType('markdown')[0];
const side = () => { const l = mdLeaf(); const v = l && l.view; const s = app.workspace.getLeavesOfType('search')[0]; return { activeLeaf: app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType(), file: v && v.file && v.file.path, mode: v && v.getMode && v.getMode(), cursor: v && v.editor && v.editor.getCursor(), searchQuery: s && s.getViewState().state.query, rightCollapsed: app.workspace.rightSplit.collapsed, menus: menuDom().map(m => m.items.map(i => i.text)), prompts: Array.from(document.querySelectorAll('.modal, .prompt')).map(m => m.className + ': ' + m.textContent.trim().slice(0, 60)) }; };
const treeInfo = (t) => { const v = leafOf(t).view; const tr = v.tree; return { viewKeys: Object.keys(v).filter(k => !/^(app|leaf|containerEl)$/.test(k)), tree: tr ? { keys: Object.keys(tr), focusedItem: tr.focusedItem ? ((tr.focusedItem.selfEl || tr.focusedItem.el || {}).textContent || '').trim().slice(0, 30) : null, activeDom: tr.activeDom ? ((tr.activeDom.selfEl || {}).textContent || '').trim().slice(0,30) : null, selected: tr.selectedDoms ? Array.from(tr.selectedDoms).map(d => ((d.selfEl || {}).textContent || '').trim().slice(0, 30)) : null } : null }; };
`;

const shot = async (name) => {
  if (!SHOTS) return null;
  fs.mkdirSync(SHOTS, { recursive: true });
  const r = await c.send('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  return name + '.png';
};

// A capture-phase recorder on the document for contextmenu and keydown.
await c.evaluate(`
  window.__probe = { ctx: [], keys: [] };
  document.addEventListener('contextmenu', (e) => {
    const rec = { target: (e.target.className || '') + '', text: (e.target.textContent || '').trim().slice(0, 30), button: e.button, x: e.clientX, y: e.clientY };
    window.__probe.ctx.push(rec);
    setTimeout(() => { rec.defaultPrevented = e.defaultPrevented; }, 0);
  }, true);
  document.addEventListener('keydown', (e) => {
    const rec = { key: e.key, target: (e.target.className || '') + '', tag: e.target.tagName };
    window.__probe.keys.push(rec);
    setTimeout(() => { rec.defaultPrevented = e.defaultPrevented; }, 0);
  }, true);
  return true;`);

const listeners = async (selector) => {
  const r = await c.send('Runtime.evaluate', { includeCommandLineAPI: true, returnByValue: true, expression: `(() => {
    const out = {};
    const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return null;
    let n = el, depth = 0;
    while (n && depth < 12) {
      const ls = getEventListeners(n);
      const pick = {}; for (const k of ['contextmenu','keydown','click','mousedown','auxclick','focus','focusin']) if (ls[k]) pick[k] = ls[k].length;
      if (Object.keys(pick).length) out[depth + ':' + n.tagName + '.' + ((typeof n.className === 'string' ? n.className : '').split(' ').slice(0, 3).join('.'))] = pick;
      n = n.parentElement; depth++;
    }
    return out; })()` });
  return r.result.value;
};

async function openPanes() {
  await c.evaluate(`
    await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Outline.md'));
    app.workspace.rightSplit.expand();
    await app.commands.executeCommandById('tag-pane:open');
    return true;`);
  await sleep(1200);
}

async function rightClickRow(type, text) {
  const r = await c.evaluate(H + `
    const s = Array.from(paneOf(${JSON.stringify(type)}).querySelectorAll('.tree-item-self')).find(s => s.getBoundingClientRect().width > 0 && s.textContent.trim().startsWith(${JSON.stringify(text)}));
    if (!s) return null; const b = s.querySelector('.tree-item-inner').getBoundingClientRect(); return { x: b.x + Math.min(20, b.width / 2), y: b.y + b.height / 2, row: s.textContent.trim() };`);
  if (!r) throw new Error('no row ' + text + ' in ' + type);
  await c.evaluate(`window.__probe.ctx = []; return true;`);
  await c.clickAt(r.x, r.y, { button: 'right' });
  await sleep(700);
  const after = await c.evaluate(H + `return { ctx: window.__probe.ctx, menus: menuDom(), side: side(), active: ae() };`);
  return { at: r, ...after };
}

await step('s01-setup', async () => {
  await openPanes();
  return c.evaluate(H + `return { tag: !!leafOf('tag'), props: !!leafOf('all-properties'), outline: !!leafOf('outline'), nativeMenus: app.vault.getConfig('nativeMenus') };`);
});

// ---------------------------------------------------------------------------
// Context menus
// ---------------------------------------------------------------------------
for (const [type, cmd, rows] of [
  ['all-properties', 'properties:open', ['status', 'tags', 'aliases', 'cssclasses', 'count']],
  ['tag', 'tag-pane:open', ['common', 'project', 'project/alpha']],
  ['outline', 'outline:open', ['Top heading', 'Second level']],
]) {
  await step(`ctx-${type}`, async () => {
    await c.evaluate(`await app.commands.executeCommandById(${JSON.stringify(cmd)}); return true;`);
    await sleep(900);
    const o = { listeners: await listeners(`.workspace-leaf-content[data-type="${type}"] .tree-item-self .tree-item-inner`) };
    for (const row of rows) {
      o[row] = await rightClickRow(type, row);
      o[row].shot = await shot(`ctx-${type}-${row.replace(/\W+/g, '_')}`);
      await c.pressKey('Escape'); await sleep(300);
    }
    // The ctrl-click and middle-click a row also might answer.
    o.view = await c.evaluate(H + `return treeInfo(${JSON.stringify(type)});`);
    return o;
  });
}

// ---------------------------------------------------------------------------
// Keyboard
// ---------------------------------------------------------------------------
const KEYSEQ = ['ArrowDown', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'End', 'Home', 'Space', 'Enter', 'Escape'];
async function keyRun(type) {
  const log = [];
  log.push({ key: '(start)', ...(await c.evaluate(H + `return { active: ae(), rows: rowsState(${JSON.stringify(type)}), tree: treeInfo(${JSON.stringify(type)}).tree, side: side() };`)) });
  for (const k of KEYSEQ) {
    await c.evaluate(`window.__probe.keys = []; return true;`);
    await c.pressKey(k); await sleep(350);
    log.push({ key: k, ...(await c.evaluate(H + `return { seen: window.__probe.keys, active: ae(), rows: rowsState(${JSON.stringify(type)}), tree: treeInfo(${JSON.stringify(type)}).tree, side: side() };`)) });
  }
  return log;
}
async function reset(type, cmd) {
  await c.pressKey('Escape'); await sleep(200);
  await c.evaluate(`
    const l = app.workspace.getLeavesOfType('markdown')[0];
    await l.openFile(app.vault.getAbstractFileByPath('Outline.md'));
    app.workspace.setActiveLeaf(l, { focus: true });
    l.view.editor && l.view.editor.setCursor({ line: 0, ch: 0 });
    await app.commands.executeCommandById(${JSON.stringify(cmd)});
    return true;`);
  await sleep(800);
}

for (const [type, cmd, rowText] of [
  ['tag', 'tag-pane:open', 'common'],
  ['all-properties', 'properties:open', 'status'],
  ['outline', 'outline:open', 'Second level'],
]) {
  await step(`keys-${type}-tabheader`, async () => {
    await reset(type, cmd);
    const r = await c.evaluate(H + `const h = leafOf(${JSON.stringify(type)}).tabHeaderEl; const b = h.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    await c.clickAt(r.x, r.y); await sleep(500);
    return { log: await keyRun(type) };
  });
  await step(`keys-${type}-emptyspace`, async () => {
    await reset(type, cmd);
    const r = await c.evaluate(H + `const p = paneOf(${JSON.stringify(type)}).querySelector('.view-content') || paneOf(${JSON.stringify(type)}); const b = p.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height - 20 };`);
    await c.clickAt(r.x, r.y); await sleep(500);
    return { at: r, log: await keyRun(type) };
  });
  await step(`keys-${type}-rowclick`, async () => {
    await reset(type, cmd);
    const r = await c.evaluate(H + `const s = Array.from(paneOf(${JSON.stringify(type)}).querySelectorAll('.tree-item-self')).find(s => s.getBoundingClientRect().width > 0 && s.textContent.trim().startsWith(${JSON.stringify(rowText)})); const b = s.getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height / 2 };`);
    await c.clickAt(r.x, r.y); await sleep(700);
    return { at: r, log: await keyRun(type) };
  });
  // The pane focused programmatically, as a command palette "focus" would.
  await step(`keys-${type}-setActiveLeaf`, async () => {
    await reset(type, cmd);
    await c.evaluate(H + `app.workspace.setActiveLeaf(leafOf(${JSON.stringify(type)}), { focus: true }); return true;`);
    await sleep(400);
    return { log: await keyRun(type) };
  });
}

// ---------------------------------------------------------------------------
// The outline filter's highlight, both themes
// ---------------------------------------------------------------------------
const HL = `
  const rules = [];
  for (const sh of Array.from(document.styleSheets)) {
    let rs; try { rs = sh.cssRules; } catch { continue; }
    const walk = (list) => { for (const r of Array.from(list)) { if (r.cssRules && !r.selectorText) walk(r.cssRules); else if (r.selectorText && /suggestion-highlight/.test(r.selectorText)) rules.push({ sheet: sh.href || (sh.ownerNode && sh.ownerNode.id) || 'inline', selector: r.selectorText, decl: r.style.cssText }); } };
    walk(rs);
  }
  const spans = Array.from(document.querySelectorAll('.workspace-leaf-content[data-type="outline"] .suggestion-highlight'));
  const s = spans[0];
  const b = document.body;
  const vars = {}; for (const n of ['--text-accent','--text-highlight-bg','--text-normal','--text-muted','--font-semibold','--font-bold','--font-medium','--bold-weight','--color-accent','--interactive-accent','--text-selection','--nav-item-color','--nav-item-color-hover','--search-result-background']) vars[n] = getComputedStyle(b).getPropertyValue(n).trim();
  return { count: spans.length, texts: spans.map(x => x.textContent), parent: s && s.parentElement.className, style: s && cs(s, ['color','font-weight','background-color','text-decoration-line','text-decoration-color','font-style','padding','border-radius','box-shadow']), parentStyle: s && cs(s.parentElement, ['color','font-weight']), rowCls: s && s.closest('.tree-item-self').className, rules, vars, theme: document.body.className.match(/theme-\\w+/g) };`;

await step('hl-outline', async () => {
  await reset('outline', 'outline:open');
  const o = {};
  const shown = await c.evaluate(H + `return leafOf('outline').getViewState().state.showSearch;`);
  if (!shown) { await c.clickElement('.workspace-leaf-content[data-type="outline"] .nav-action-button[aria-label="Show search filter"]'); await sleep(500); }
  await c.clickElement('.workspace-leaf-content[data-type="outline"] .search-input-container input'); await sleep(200);
  await c.typeText('second'); await sleep(700);
  await c.mouseMove(5, 5); await sleep(300);
  o.dark = await c.evaluate(H + HL);
  o.darkShot = await shot('outline-highlight-dark');
  // Hover the row holding a match: does the highlight change?
  const r = await c.evaluate(`const s = document.querySelector('.workspace-leaf-content[data-type="outline"] .suggestion-highlight'); const b = s.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.mouseMove(r.x, r.y); await sleep(400);
  o.darkHover = await c.evaluate(H + `const s = document.querySelector('.workspace-leaf-content[data-type="outline"] .suggestion-highlight'); return { style: cs(s, ['color','font-weight','background-color']), parent: cs(s.parentElement, ['color']) };`);
  await c.mouseMove(5, 5);
  await c.evaluate(`app.changeTheme ? app.changeTheme('moonstone') : app.setTheme('moonstone'); return true;`);
  await sleep(800);
  o.light = await c.evaluate(H + HL);
  o.lightShot = await shot('outline-highlight-light');
  await c.evaluate(`app.changeTheme ? app.changeTheme('obsidian') : app.setTheme('obsidian'); return true;`);
  await sleep(500);
  return o;
});

// The tags filter: is there a highlight there too?
await step('hl-tags', async () => {
  await c.evaluate(`await app.commands.executeCommandById('tag-pane:open'); return true;`); await sleep(600);
  const shown = await c.evaluate(H + `return leafOf('tag').getViewState().state.showSearch;`);
  if (!shown) { await c.clickElement('.workspace-leaf-content[data-type="tag"] .nav-action-button[aria-label="Show search filter"]'); await sleep(500); }
  await c.clickElement('.workspace-leaf-content[data-type="tag"] .search-input-container input'); await sleep(200);
  await c.typeText('proj'); await sleep(700);
  return c.evaluate(H + `return { spans: Array.from(paneOf('tag').querySelectorAll('.suggestion-highlight, mark, .search-result-file-matched-text')).map(s => ({ cls: s.className, text: s.textContent })) };`);
});

// ---------------------------------------------------------------------------
// Keyboard, second pass: the edges, Enter, Escape, F2, Delete, the focus
// ring's style, a mouse click's effect on the focused row, a filtered list.
// ---------------------------------------------------------------------------
const focusedText = (type) => c.evaluate(H + `const t = treeInfo(${JSON.stringify(type)}).tree; return { focused: t && t.focusedItem, ring: rowsState(${JSON.stringify(type)}).filter(r => /has-focus/.test(r.cls)).map(r => r.text), collapsed: Array.from(paneOf(${JSON.stringify(type)}).querySelectorAll('.tree-item.is-collapsed > .tree-item-self')).map(s => s.textContent.trim().slice(0, 30)), side: side(), active: ae() };`);
const activate = async (type, cmd) => {
  await c.evaluate(`await app.commands.executeCommandById(${JSON.stringify(cmd)}); app.workspace.setActiveLeaf(app.workspace.getLeavesOfType(${JSON.stringify(type)})[0], { focus: true }); return true;`);
  await sleep(400);
};
const press = async (type, k, mods) => { await c.pressKey(k, mods); await sleep(350); return { key: (mods ? JSON.stringify(mods) : '') + k, ...(await focusedText(type)) }; };

for (const [type, cmd] of [['tag', 'tag-pane:open'], ['all-properties', 'properties:open'], ['outline', 'outline:open']]) {
  await step(`keys2-${type}-edges`, async () => {
    // A clean leaf: detach and reopen so no item is focused.
    await c.evaluate(`app.workspace.getLeavesOfType(${JSON.stringify(type)}).forEach(l => l.detach()); await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Outline.md')); return true;`);
    await sleep(400);
    await activate(type, cmd);
    const log = [{ key: '(start)', ...(await focusedText(type)) }];
    const n = await c.evaluate(H + `return paneOf(${JSON.stringify(type)}).querySelectorAll('.tree-item-self').length;`);
    log.push(await press(type, 'ArrowUp'));
    log.push(await press(type, 'ArrowUp'));
    for (let i = 0; i < n + 1; i++) log.push(await press(type, 'ArrowDown'));
    log.push(await press(type, 'ArrowDown', { shift: true }));
    log.push(await press(type, 'PageUp'));
    log.push(await press(type, 'Home'));
    log.push(await press(type, 'Tab'));
    return { n, log };
  });
  await step(`keys2-${type}-enter-escape`, async () => {
    await activate(type, cmd);
    const log = [];
    log.push(await press(type, 'ArrowDown'));
    log.push(await press(type, 'Escape'));
    await activate(type, cmd);
    log.push(await press(type, 'ArrowDown'));
    log.push(await press(type, 'Enter'));
    await activate(type, cmd);
    log.push({ key: '(reactivated)', ...(await focusedText(type)) });
    log.push(await press(type, 'Enter', { ctrl: true }));
    await activate(type, cmd);
    log.push(await press(type, 'F2'));
    log.push({ key: '(after F2 dom)', dom: await c.evaluate(H + `const i = paneOf(${JSON.stringify(type)}).querySelector('[contenteditable], input:not([type=search])'); return i && { tag: i.tagName, cls: i.className, text: i.textContent || i.value };`) });
    log.push(await press(type, 'Escape'));
    await activate(type, cmd);
    log.push(await press(type, 'Delete'));
    log.push({ key: '(after Delete dom)', modal: await c.evaluate(`return Array.from(document.querySelectorAll('.modal-container')).map(m => m.textContent.trim().slice(0, 200));`) });
    log.push(await press(type, 'Escape'));
    return { log };
  });
  await step(`keys2-${type}-ring-style`, async () => {
    await activate(type, cmd);
    await c.pressKey('ArrowDown'); await sleep(300);
    await c.mouseMove(5, 5); await sleep(300);
    return c.evaluate(H + `
      const s = paneOf(${JSON.stringify(type)}).querySelector('.tree-item-self.has-focus'); if (!s) return { none: true };
      const rules = [];
      for (const sh of Array.from(document.styleSheets)) { let rs; try { rs = sh.cssRules; } catch { continue; }
        const walk = (list) => { for (const r of Array.from(list)) { if (r.cssRules && !r.selectorText) walk(r.cssRules); else if (r.selectorText && /has-focus/.test(r.selectorText) && /tree-item|nav-|is-focus/.test(r.selectorText)) rules.push({ selector: r.selectorText, decl: r.style.cssText }); } };
        walk(rs); }
      const b = document.body; const vars = {}; for (const n of ['--background-modifier-border-focus','--nav-item-background-active','--nav-item-background-hover','--background-modifier-hover','--interactive-accent','--radius-s','--radius-m']) vars[n] = getComputedStyle(b).getPropertyValue(n).trim();
      return { text: s.textContent.trim(), cls: s.className, style: cs(s, ['box-shadow','outline','outline-offset','background-color','color','border-radius','border']), inner: cs(s.querySelector('.tree-item-inner'), ['color','font-weight']), rules, vars };`);
  });
}

await step('keys2-outline-click-then-keys', async () => {
  await activate('outline', 'outline:open');
  const r = await c.evaluate(H + `const s = Array.from(paneOf('outline').querySelectorAll('.tree-item-self')).filter(s => s.getBoundingClientRect().width > 0)[2]; const b = s.getBoundingClientRect(); return { x: b.x + 40, y: b.y + b.height / 2, text: s.textContent.trim() };`);
  await c.clickAt(r.x, r.y); await sleep(600);
  const log = [{ key: '(clicked ' + r.text + ')', ...(await focusedText('outline')) }];
  const h = await c.evaluate(H + `const b = leafOf('outline').tabHeaderEl.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  await c.clickAt(h.x, h.y); await sleep(400);
  log.push({ key: '(tab header)', ...(await focusedText('outline')) });
  log.push(await press('outline', 'ArrowDown'));
  return { log };
});

await step('keys2-outline-filtered', async () => {
  await activate('outline', 'outline:open');
  const shown = await c.evaluate(H + `return leafOf('outline').getViewState().state.showSearch;`);
  if (!shown) { await c.clickElement('.workspace-leaf-content[data-type="outline"] .nav-action-button[aria-label="Show search filter"]'); await sleep(500); }
  await c.clickElement('.workspace-leaf-content[data-type="outline"] .search-input-container input'); await sleep(200);
  await c.pressKey('a', { ctrl: true }); await c.pressKey('Backspace');
  await c.typeText('second'); await sleep(600);
  const log = [];
  log.push(await press('outline', 'ArrowDown'));
  log.push({ key: '(in input) visible', rows: await c.evaluate(H + `return Array.from(paneOf('outline').querySelectorAll('.tree-item-self')).filter(s => s.getBoundingClientRect().width > 0).map(s => s.textContent.trim());`) });
  await activate('outline', 'outline:open');
  for (let i = 0; i < 5; i++) log.push(await press('outline', 'ArrowDown'));
  return { log };
});

out.done = true;
save();
c.close();
process.exit(0);
