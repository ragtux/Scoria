// The Bookmarks pane's look and its interactions, driven over CDP, for
// look-alike parity (the 2026-10-20 splits-bookmarks pass recorded the bytes
// each command writes, not the pane): the pane's markup with computed styles
// and the icon each row and header button names, the leaf's tab header,
// hover on a row and on every header button with its tooltip, which row the
// open note marks, the right-click menu of every kind of item with its icons,
// "Collapse all" and a group's own fold, the in-pane search filter under real
// keystrokes, what a click with Ctrl or the middle button opens, the menu's
// edit and delete paths and the modals they open, "New group", drags that
// reorder, nest and add from the Files pane with the bytes each writes and the
// drop indicator in between, the tree keyboard, the light theme and the empty
// state, with a screenshot of every state (SHOTS names the folder).
// Phases: fixtures | run   (node bookmarks-pane.mjs <phase>)
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
const BUDGET_MS = 540000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.bookmarksPane = all.bookmarksPane || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.bookmarksPane[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();
async function step(name, fn, budgetMs = 30000) {
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  try {
    const v = await Promise.race([fn(partial), sleep(budgetMs).then(() => { throw new Error('step budget exceeded'); })]);
    out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) };
  } catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  return out.steps[name];
}
const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
async function waitForChange(rel, before, timeoutMs = 5000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) { if (bytes(rel) !== before) { await sleep(500); return true; } await sleep(150); }
  return false;
}
const BM = '.obsidian/bookmarks.json';

// ---------------------------------------------------------------------------
if (phase === 'fixtures') {
  const filler = (n) => Array.from({ length: n }, (_, i) => `Line ${i + 1} of filler so the note has some height.`).join('\n\n');
  fs.mkdirSync(vp('sub'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), `# Alpha\n\nLinks to [[Beta]].\n\nA block to bookmark. ^blk1\n\n## Second heading\n\n${filler(6)}\n`);
  fs.writeFileSync(vp('Beta.md'), '# Beta\n\nBeta body, alpha mentioned.\n');
  fs.writeFileSync(vp('Gamma.md'), '# Gamma\n\nGamma body.\n');
  fs.writeFileSync(vp('Unbookmarked.md'), '# Unbookmarked\n\nDragged in from the Files pane.\n');
  fs.writeFileSync(vp('sub/Delta.md'), '# Delta\n\nIn a folder.\n');
  fs.writeFileSync(vp('Image.png'), Buffer.from('89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082', 'hex'));
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'global-search': true, graph: true, backlink: true, bookmarks: true, 'command-palette': true, switcher: true, outline: true, 'tag-pane': true, 'page-preview': true }, null, 2));
  const ct = 1792373000000;
  const items = [
    { type: 'file', ctime: ct, path: 'Alpha.md' },
    { type: 'file', ctime: ct + 1, path: 'Alpha.md', subpath: '#Second heading' },
    { type: 'file', ctime: ct + 2, path: 'Alpha.md', subpath: '#^blk1' },
    { type: 'folder', ctime: ct + 3, path: 'sub' },
    { type: 'search', ctime: ct + 4, query: 'alpha' },
    { type: 'graph', ctime: ct + 5, title: 'Graph view', options: { 'collapse-filter': true, search: '', showTags: false, showAttachments: false, hideUnresolved: false, showOrphans: true } },
    { type: 'url', ctime: ct + 6, url: 'https://example.com/x', title: 'Example site' },
    { type: 'group', ctime: ct + 7, items: [
      { type: 'file', ctime: ct + 8, path: 'Beta.md' },
      { type: 'group', ctime: ct + 9, items: [{ type: 'file', ctime: ct + 10, path: 'sub/Delta.md' }], title: 'Nested group' },
    ], title: 'Group one' },
    { type: 'group', ctime: ct + 11, items: [], title: 'Empty group' },
    { type: 'file', ctime: ct + 12, path: 'Missing.md' },
    { type: 'file', ctime: ct + 13, path: 'Image.png' },
    { type: 'file', ctime: ct + 14, path: 'Gamma.md', title: 'A custom title on Gamma that is long enough to be cut at the sidebar width' },
  ];
  fs.writeFileSync(vp(BM), JSON.stringify({ items }, null, 2));
  out.files = fs.readdirSync(VAULT);
  save();
  process.exit(0);
}

// ---------------------------------------------------------------------------
const c = await connect();
const PROBE_JS = `
const cs = (el, props) => { const s = getComputedStyle(el); const o = {}; for (const p of props) o[p] = s.getPropertyValue(p); return o; };
const PROPS = ['display','font-size','font-weight','line-height','color','background-color','padding','margin','height','min-height','border-radius','border','opacity','cursor','white-space','overflow','text-overflow','gap','align-items','justify-content','flex','width','position','top','left','flex-direction','box-shadow','outline','border-left','padding-inline-start','margin-inline-start'];
const rect = (el) => { const r = el.getBoundingClientRect(); return [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; };
function dump(el, depth) {
  if (depth > 26) return { tag: el.tagName, truncated: true };
  const tag = el.tagName.toLowerCase();
  const cls = el.className && el.className.baseVal !== undefined ? el.className.baseVal : el.className;
  const o = { tag, cls, rect: rect(el) };
  for (const a of Array.from(el.attributes)) if (a.name !== 'class') o['@' + a.name] = a.value;
  const own = Array.from(el.childNodes).filter(n => n.nodeType === 3).map(n => n.textContent).join('').trim(); if (own) o.text = own;
  if (tag === 'svg') { o.style = cs(el, ['width','height','color','stroke','stroke-width','opacity']); return o; }
  o.style = cs(el, PROPS);
  if (tag === 'input') o.value = el.value;
  const kids = Array.from(el.children); if (kids.length) o.children = kids.map(k => dump(k, depth + 1));
  return o;
}
const leaf = () => app.workspace.getLeavesOfType('bookmarks')[0];
const pane = () => leaf().view.containerEl;
const vars = (el, names) => { const s = getComputedStyle(el); const o = {}; for (const n of names) o[n] = s.getPropertyValue(n).trim(); return o; };
const depthOf = (el) => { let d = 0, p = el.parentElement; while (p) { if (p.classList.contains('tree-item-children')) d++; p = p.parentElement; } return d; };
const rows = () => Array.from(pane().querySelectorAll('.tree-item')).map(it => {
  const self = it.querySelector(':scope > .tree-item-self');
  const inner = self && self.querySelector('.tree-item-inner');
  const ic = self && self.querySelector('.tree-item-icon');
  const flair = self && self.querySelector('.tree-item-flair');
  const icons = self ? Array.from(self.querySelectorAll('svg')).map(s => s.getAttribute('class')) : [];
  const o = { itemCls: it.className, selfCls: self && self.className, depth: depthOf(it), text: inner && inner.textContent.trim(), icons,
    rect: self && rect(self), innerRect: inner && rect(inner), iconRect: ic && rect(ic),
    selfStyle: self && cs(self, ['padding','padding-inline-start','margin-inline-start','height','color','background-color','font-size','font-weight','border-radius','margin','opacity','display','align-items','cursor','gap']),
    innerStyle: inner && cs(inner, ['color','font-size','font-weight','white-space','overflow','text-overflow','display','flex','line-height','padding','margin','text-decoration','font-style']),
    iconStyle: ic && cs(ic, ['color','opacity','width','height','margin','padding','display','position','left','top','transform']),
    flair: flair && { text: flair.textContent.trim(), style: cs(flair, ['color','font-size','background-color','padding','border-radius']) },
    attrs: {} };
  if (self) for (const a of Array.from(self.attributes)) if (a.name !== 'class') o.attrs[a.name] = a.value;
  for (const a of Array.from(it.attributes)) if (a.name !== 'class') o.attrs['item:' + a.name] = a.value;
  const kids = it.querySelector(':scope > .tree-item-children'); if (kids) { o.childrenRect = rect(kids); o.childrenStyle = cs(kids, ['margin','padding','border-left','display','margin-inline-start','padding-inline-start']); o.childCount = kids.querySelectorAll(':scope > .tree-item').length; }
  return o;
});
const headerButtons = () => Array.from(pane().querySelectorAll('.nav-header .clickable-icon, .nav-header .nav-action-button')).map(b => ({ cls: b.className, label: b.getAttribute('aria-label'), icon: (b.querySelector('svg') || { getAttribute: () => null }).getAttribute('class'), rect: rect(b), style: cs(b, ['color','background-color','padding','border-radius','opacity','width','height','margin']) }));
const tooltip = () => Array.from(document.querySelectorAll('.tooltip')).map(t => ({ cls: t.className, text: t.textContent.trim(), rect: rect(t) }));
const menuDom = () => Array.from(document.querySelectorAll('.menu')).map(m => ({ cls: m.className, rect: rect(m), style: cs(m, ['background-color','border','border-radius','padding','box-shadow','font-size']),
  items: Array.from(m.querySelectorAll('.menu-item, .menu-separator, .menu-group')).filter(i => !i.classList.contains('menu-group')).map(i => ({ cls: i.className, text: (i.querySelector('.menu-item-title') || i).textContent.trim(), icon: (i.querySelector('.menu-item-icon svg') || { getAttribute: () => null }).getAttribute('class'), submenu: !!i.querySelector('.menu-item-icon.mod-submenu, .mod-submenu'), rect: rect(i) })) }));
const modal = () => { const m = document.querySelector('.modal-container .modal'); if (!m) return null; return { dom: dump(m, 0), title: (m.querySelector('.modal-title') || {}).textContent, text: m.innerText.slice(0, 1200), inputs: Array.from(m.querySelectorAll('input, textarea')).map(i => ({ type: i.type, value: i.value, placeholder: i.placeholder })), buttons: Array.from(m.querySelectorAll('button')).map(b => ({ text: b.textContent.trim(), cls: b.className })) }; };
const marked = () => rows().filter(r => /is-active|is-selected|has-focus|is-being-dragged|mod-active/.test((r.selfCls || '') + ' ' + (r.itemCls || ''))).map(r => ({ text: r.text, selfCls: r.selfCls, itemCls: r.itemCls, bg: r.selfStyle && r.selfStyle['background-color'], color: r.innerStyle && r.innerStyle.color }));
const leafState = () => { const l = leaf(); return { viewState: l.getViewState(), eph: l.getEphemeralState && l.getEphemeralState(), viewKeys: Object.keys(l.view).filter(k => !/El$|^(app|leaf|containerEl|scope|navigation|plugin)$/.test(k)) }; };
const leafCounts = () => { const o = {}; app.workspace.iterateAllLeaves(l => { const t = l.view.getViewType(); o[t] = (o[t] || 0) + 1; }); return { o, active: app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType(), activeFile: app.workspace.getActiveFile() && app.workspace.getActiveFile().path, focused: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className) }; };
const tagRow = (text, index) => { const it = Array.from(pane().querySelectorAll('.tree-item-self')).filter(s => { const i = s.querySelector('.tree-item-inner'); return i && i.textContent.trim() === text; })[index || 0]; if (!it) return null; it.setAttribute('data-cdp', 'row'); document.querySelectorAll('[data-cdp="row"]').forEach(e => { if (e !== it) e.removeAttribute('data-cdp'); }); return true; };
`;
const ev = (body) => c.evaluate(PROBE_JS + body);
async function shot(name) {
  if (!SHOTS) return null;
  try {
    fs.mkdirSync(SHOTS, { recursive: true });
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
    return name + '.png';
  } catch (e) { return 'ERR:' + e.message; }
}
async function rowRect(text, index = 0) {
  const ok = await ev(`return tagRow(${JSON.stringify(text)}, ${index});`);
  if (!ok) throw new Error('no row ' + text);
  return c.rect('[data-cdp="row"]');
}
const btnSel = (label) => `.workspace-leaf-content[data-type="bookmarks"] .nav-header [aria-label="${label}"]`;
async function clickButton(label) {
  const r = await c.clickElement(btnSel(label));
  if (!r) throw new Error('no header button ' + label);
  await sleep(500);
  return r;
}
async function clickMenuItem(label) {
  const r = await c.evaluate(`const it = Array.from(document.querySelectorAll('.menu .menu-item')).find(i => ((i.querySelector('.menu-item-title') || i).textContent || '').trim() === ${JSON.stringify(label)}); if (!it) return null; const b = it.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
  if (!r) throw new Error('no menu item ' + label);
  await c.clickAt(r.x, r.y);
  await sleep(600);
  return r;
}
const menuOpen = () => c.evaluate(`return document.querySelectorAll('.menu').length;`);
async function closeAll() {
  for (let i = 0; i < 3; i++) { if (!(await c.evaluate(`return !!document.querySelector('.menu, .modal-container');`))) return; await c.pressKey('Escape'); await sleep(250); }
}
const bmFile = () => { const raw = bytes(BM); let parsed = null; try { parsed = JSON.parse(raw); } catch {} return { raw, parsed }; };
const VARS = ['--nav-item-size','--nav-item-color','--nav-item-color-hover','--nav-item-color-active','--nav-item-color-selected','--nav-item-weight','--nav-item-padding','--nav-item-parent-padding','--nav-item-children-padding-start','--nav-item-children-margin-start','--nav-item-background-hover','--nav-item-background-active','--nav-item-background-selected','--nav-item-white-space','--nav-indentation-guide-width','--nav-indentation-guide-color','--nav-collapse-icon-color','--nav-collapse-icon-color-collapsed','--nav-tree-item-icon','--text-muted','--text-faint','--text-normal','--text-accent','--font-ui-small','--font-ui-smaller','--background-secondary','--background-modifier-hover','--background-modifier-border-focus','--interactive-accent','--icon-size','--icon-xs','--icon-s','--icon-color','--icon-color-hover','--icon-color-active','--icon-color-focused','--icon-opacity','--icon-opacity-hover','--icon-opacity-active','--clickable-icon-radius','--drag-ghost-background','--drag-ghost-text-color','--background-modifier-cover','--text-on-accent'];

// ---- the pane at rest ------------------------------------------------------
await step('s01-open', async () => {
  await c.evaluate(`
    await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Alpha.md'));
    app.workspace.leftSplit.expand();
    await app.commands.executeCommandById('bookmarks:open');
    return true;`);
  await sleep(1500);
  return ev(`const l = leaf(); return { counts: leafCounts(), leftWidth: app.workspace.leftSplit.containerEl.getBoundingClientRect().width,
    tabHeader: l.tabHeaderEl && { cls: l.tabHeaderEl.className, label: l.tabHeaderEl.getAttribute('aria-label'), icon: l.tabHeaderEl.querySelector('svg') && l.tabHeaderEl.querySelector('svg').getAttribute('class'), title: (l.tabHeaderEl.querySelector('.workspace-tab-header-inner-title') || {}).textContent },
    display: l.getDisplayText(), icon: l.view.getIcon && l.view.getIcon(), state: leafState(),
    commands: Object.keys(app.commands.commands).filter(k => k.startsWith('bookmarks:')).map(id => ({ id, name: app.commands.commands[id].name })),
    items: app.internalPlugins.getPluginById('bookmarks').instance.items.length };`);
});

await step('s02-dom', async () => ({ shot: await shot('01-default'), ...(await ev(`const p = pane(); return { dom: dump(p, 0), vars: vars(p, ${JSON.stringify(VARS)}), rows: rows(), headers: headerButtons(), marked: marked() };`)) }));

await step('s03-hover-rows', async () => {
  const o = {};
  for (const [name, text] of [['file', 'Beta'], ['group', 'Group one'], ['missing', 'Missing'], ['url', 'Example site']]) {
    const r = await rowRect(text);
    await c.mouseMove(r.cx, r.cy); await sleep(600);
    o[name] = await ev(`const s = document.querySelector('[data-cdp="row"]'); const inner = s.querySelector('.tree-item-inner'); const ic = s.querySelector('.tree-item-icon');
      return { hovered: s.matches(':hover'), cls: s.className, bg: getComputedStyle(s).backgroundColor, color: inner && getComputedStyle(inner).color, icon: ic && { color: getComputedStyle(ic).color, opacity: getComputedStyle(ic).opacity }, tooltip: tooltip(), popover: !!document.querySelector('.popover.hover-popover') };`);
    o[name].shot = await shot('02-hover-' + name);
  }
  // Ctrl held over a file row: the page preview.
  const r = await rowRect('Beta');
  await c.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(r.cx), y: Math.round(r.cy) + 1, modifiers: 2 });
  await sleep(1400);
  o.ctrlHover = await ev(`const p = document.querySelector('.popover.hover-popover'); return { popover: !!p, rect: p && rect(p) };`);
  o.ctrlHover.shot = await shot('02-hover-ctrl-preview');
  await c.mouseMove(5, 400); await sleep(500);
  return o;
});

await step('s04-header-buttons', async () => {
  const labels = await ev(`return headerButtons().map(b => b.label);`);
  const o = { labels };
  for (const l of labels) {
    const r = await c.hover(btnSel(l)); await sleep(1100);
    o[l] = { pointer: r, ...(await ev(`const b = document.querySelector(${JSON.stringify(btnSel(l))}); return { hovered: b.matches(':hover'), cls: b.className, style: cs(b, ['color','background-color','border-radius','opacity']), svg: b.querySelector('svg') && cs(b.querySelector('svg'), ['color','opacity','width','height']), tooltip: tooltip() };`)) };
  }
  o.shot = await shot('03-hover-header-last');
  await c.mouseMove(5, 400); await sleep(400);
  return o;
});

// ---- which row the open note marks ------------------------------------------
await step('s05-active-marking', async () => {
  const o = {};
  o.alpha = await ev(`return marked();`);
  await c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('sub/Delta.md')); return true;`); await sleep(700);
  o.deltaInNested = await ev(`return marked();`);
  o.shotDelta = await shot('04-active-delta');
  await c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Unbookmarked.md')); return true;`); await sleep(700);
  o.unbookmarked = await ev(`return marked();`);
  await c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Alpha.md')); return true;`); await sleep(700);
  // A click on a row: does the click itself leave a mark?
  const r = await rowRect('Beta'); await c.clickAt(r.cx, r.cy); await sleep(800);
  o.afterClickBeta = { marked: await ev(`return marked();`), counts: await ev(`return leafCounts();`) };
  o.shotClicked = await shot('04-after-click-beta');
  await c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Alpha.md')); return true;`); await sleep(500);
  return o;
});

// ---- the context menu of every kind ------------------------------------------
await step('s06-menus', async () => {
  const o = {};
  const cases = [['file', 'Alpha'], ['heading', 'Alpha#Second heading'], ['block', 'Alpha#^blk1'], ['folder', 'sub'], ['search', 'alpha'], ['graph', 'Graph view'], ['url', 'Example site'], ['group', 'Group one'], ['emptyGroup', 'Empty group'], ['image', 'Image.png'], ['titled', 'A custom title on Gamma that is long enough to be cut at the sidebar width'], ['inGroup', 'Beta']];
  const texts = await ev(`return rows().map(r => r.text);`);
  o.texts = texts;
  for (const [name, text] of cases) {
    const t = texts.includes(text) ? text : texts.find(x => x && x.startsWith(text.slice(0, 8)));
    if (!t) { o[name] = { missingRow: text }; continue; }
    const r = await rowRect(t);
    await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600);
    o[name] = { text: t, menu: await ev(`return menuDom();`), rowCls: await ev(`return document.querySelector('[data-cdp="row"]').className;`) };
    if (name === 'file') o[name].shot = await shot('05-menu-file');
    if (name === 'group') o[name].shot = await shot('05-menu-group');
    await c.pressKey('Escape'); await sleep(300);
    o[name].openAfterEscape = await menuOpen();
    if (o[name].openAfterEscape) { await c.clickAt(5, 400); await sleep(300); }
  }
  // Right-click on the pane's empty space below the rows.
  const below = await ev(`const p = pane().querySelector('.view-content') || pane(); const b = p.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.bottom - 30 };`);
  await c.clickAt(below.x, below.y, { button: 'right' }); await sleep(600);
  o.emptySpace = { menu: await ev(`return menuDom();`) };
  await closeAll();
  return o;
}, 90000);

// ---- folding -----------------------------------------------------------------
await step('s07-collapse-all', async () => {
  const o = {};
  o.before = { labels: await ev(`return headerButtons().map(b => b.label + ' ' + b.icon);`) };
  const label = o.before.labels.map(l => l.split(' svg-icon')[0].trim()).find(l => /collapse|expand/i.test(l));
  if (!label) return { ...o, skipped: 'no collapse button' };
  await clickButton(label);
  o.after = { labels: await ev(`return headerButtons().map(b => b.label + ' ' + b.icon);`), rows: await ev(`return rows().map(r => ({ text: r.text, itemCls: r.itemCls, depth: r.depth, visible: !!(r.rect && r.rect[3]) }));`), state: await ev(`return leafState();`), shot: await shot('06-collapsed-all') };
  const label2 = o.after.labels.map(l => l.split(' svg-icon')[0].trim()).find(l => /collapse|expand/i.test(l));
  await clickButton(label2);
  o.again = { labels: await ev(`return headerButtons().map(b => b.label + ' ' + b.icon);`), rows: await ev(`return rows().map(r => ({ text: r.text, itemCls: r.itemCls, visible: !!(r.rect && r.rect[3]) }));`) };
  return o;
});

await step('s08-group-fold', async () => {
  const o = {};
  // A click on the group's title: fold, open, or nothing?
  let r = await rowRect('Group one');
  await c.clickAt(r.x + r.w - 20, r.cy); await sleep(600);
  o.clickTitle = await ev(`const it = document.querySelector('[data-cdp="row"]').parentElement; return { itemCls: it.className, counts: leafCounts() };`);
  if (/is-collapsed/.test(o.clickTitle.itemCls)) { r = await rowRect('Group one'); await c.clickAt(r.x + r.w - 20, r.cy); await sleep(500); }
  // A click on the collapse chevron itself.
  await rowRect('Group one');
  const ci = await c.rect('[data-cdp="row"] .collapse-icon, [data-cdp="row"] .tree-item-icon');
  if (ci) { await c.clickAt(ci.cx, ci.cy); await sleep(600); }
  o.clickChevron = await ev(`const s = document.querySelector('[data-cdp="row"]'); const it = s.parentElement; const ic = s.querySelector('.collapse-icon, .tree-item-icon'); return { itemCls: it.className, iconCls: ic && ic.className, svg: ic && ic.querySelector('svg') && ic.querySelector('svg').getAttribute('class'), transform: ic && getComputedStyle(ic).transform, svgTransform: ic && ic.querySelector('svg') && getComputedStyle(ic.querySelector('svg')).transform, state: leafState() };`);
  o.shotFolded = await shot('07-group-folded');
  o.workspaceAfterFold = await c.evaluate(`app.workspace.requestSaveLayout && app.workspace.requestSaveLayout.run && app.workspace.requestSaveLayout.run(); await new Promise(r => setTimeout(r, 1500)); return true;`);
  try { const ws = JSON.parse(bytes('.obsidian/workspace.json')); const find = (n) => { if (!n || typeof n !== 'object') return null; if (n.type === 'leaf' && n.state && n.state.type === 'bookmarks') return n.state; for (const k of ['children']) if (Array.isArray(n[k])) for (const ch of n[k]) { const f = find(ch); if (f) return f; } return null; }; o.leafRecord = find(ws.left) || find(ws.right) || find(ws.main); } catch (e) { o.leafRecord = 'ERR ' + e.message; }
  o.bmBytesChanged = bmFile().raw !== null && !bmFile().raw.includes('"type": "group"') ? 'reformatted?' : 'same-shape';
  // Open it again.
  await rowRect('Group one');
  const ci2 = await c.rect('[data-cdp="row"] .collapse-icon, [data-cdp="row"] .tree-item-icon');
  if (ci2) { await c.clickAt(ci2.cx, ci2.cy); await sleep(500); }
  o.reopened = await ev(`return document.querySelector('[data-cdp="row"]').parentElement.className;`);
  return o;
});

// ---- the search filter ----------------------------------------------------------
await step('s09-filter', async () => {
  const o = {};
  const label = (await ev(`return headerButtons().map(b => b.label);`)).find(l => /search|filter/i.test(l || ''));
  if (!label) return { skipped: 'no filter button' };
  o.label = label;
  await clickButton(label);
  o.shown = await ev(`const sc = pane().querySelector('.search-input-container'); return { dom: sc && dump(sc, 0), buttonCls: document.querySelector(${JSON.stringify(btnSel(label))}).className, focused: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className + ' ' + (document.activeElement.placeholder || '')), headerDom: dump(pane().querySelector('.nav-header'), 0) };`);
  o.shotShown = await shot('08-filter-shown');
  const focusInput = async () => { const r = await c.rect('.workspace-leaf-content[data-type="bookmarks"] .search-input-container input'); if (r) await c.clickAt(r.cx, r.cy); await sleep(200); };
  await focusInput();
  for (const q of ['a', 'al', 'beta', 'del', 'zzz']) {
    await c.evaluate(`const i = document.querySelector('.workspace-leaf-content[data-type="bookmarks"] .search-input-container input'); i.select(); return true;`);
    await c.typeText(q, 40); await sleep(700);
    o['q_' + q] = await ev(`return { value: pane().querySelector('.search-input-container input').value, rows: rows().filter(r => r.rect && r.rect[3]).map(r => ({ text: r.text, depth: r.depth, itemCls: r.itemCls, html: document.querySelectorAll('.tree-item-inner')[0] && null })), hl: Array.from(pane().querySelectorAll('.tree-item-inner .suggestion-highlight, .tree-item-inner mark, .search-result-file-matched-text')).map(m => ({ text: m.textContent, cls: m.className, style: cs(m, ['color','background-color','font-weight']) })), empty: (pane().querySelector('.search-empty-state, .pane-empty') || {}).textContent || null, state: leafState().viewState.state };`);
    if (q === 'al') o.shotAl = await shot('08-filter-al');
    if (q === 'zzz') o.shotNone = await shot('08-filter-none');
  }
  await c.pressKey('Escape'); await sleep(400);
  o.afterEscape = await ev(`const sc = pane().querySelector('.search-input-container'); return { value: sc && sc.querySelector('input').value, display: sc && getComputedStyle(sc).display, focused: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className), rows: rows().filter(r => r.rect && r.rect[3]).length };`);
  await clickButton(label);
  o.hidden = await ev(`const sc = pane().querySelector('.search-input-container'); return { present: !!sc, display: sc && getComputedStyle(sc).display, value: sc && sc.querySelector('input').value, rows: rows().filter(r => r.rect && r.rect[3]).length, state: leafState().viewState.state };`);
  return o;
});

// ---- clicks with modifiers --------------------------------------------------------
await step('s10-click-modifiers', async () => {
  const o = {};
  await c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Alpha.md')); return true;`); await sleep(400);
  o.before = await ev(`return leafCounts();`);
  let r = await rowRect('Gamma', 0).catch(() => null);
  if (!r) r = await rowRect(await ev(`return rows().map(x => x.text).find(t => t && t.startsWith('A custom'));`));
  await c.clickAt(r.cx, r.cy, { mods: { ctrl: true } }); await sleep(900);
  o.ctrlClick = await ev(`return leafCounts();`);
  r = await rowRect('Beta');
  await c.clickAt(r.cx, r.cy, { button: 'middle' }); await sleep(900);
  o.middleClick = await ev(`return leafCounts();`);
  await c.evaluate(`const keep = app.workspace.getLeavesOfType('markdown'); for (const l of keep.slice(1)) l.detach(); return true;`); await sleep(400);
  // The folder row and the missing file row, plainly.
  r = await rowRect('sub'); await c.clickAt(r.cx, r.cy); await sleep(900);
  o.folderClick = await ev(`const fe = app.workspace.getLeavesOfType('file-explorer')[0]; return { counts: leafCounts(), leftActive: app.workspace.leftSplit.children && null, explorerShown: fe && !!fe.view.containerEl.offsetParent, focusedRow: fe && (fe.view.containerEl.querySelector('.has-focus, .is-active') || {}).textContent };`);
  o.shotFolder = await shot('09-folder-click');
  await c.evaluate(`await app.commands.executeCommandById('bookmarks:open'); return true;`); await sleep(700);
  const before = await ev(`return app.vault.getFiles().map(f => f.path).sort();`);
  r = null; o.missingHidden = true; if (false) await sleep(1);
  if (r) o.missingClick = { counts: await ev(`return leafCounts();`), modal: await ev(`return modal();`), notice: await c.evaluate(`return Array.from(document.querySelectorAll('.notice')).map(n => n.textContent);`), created: (await ev(`return app.vault.getFiles().map(f => f.path).sort();`)).filter(p => !before.includes(p)) };
  o.shotMissing = await shot('09-missing-click');
  await closeAll();
  await c.evaluate(`await app.workspace.getLeaf(false).openFile(app.vault.getAbstractFileByPath('Alpha.md')); await app.commands.executeCommandById('bookmarks:open'); return true;`); await sleep(600);
  return o;
});

// ---- menu actions: edit, new group, delete ------------------------------------------
async function menuAction(rowText, item) {
  const r = await rowRect(rowText);
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(500);
  const items = await c.openMenuItems();
  await clickMenuItem(item);
  return items;
}
await step('s11-edit', async () => {
  const o = {};
  const items = await ev(`return menuDom();`);
  const r = await rowRect('Beta');
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(500);
  o.menu = await c.openMenuItems();
  const edit = (o.menu[0] || []).find(l => /^(Edit|Rename)/i.test(l));
  if (!edit) { await closeAll(); return { ...o, skipped: 'no edit item' }; }
  o.edit = edit;
  await clickMenuItem(edit); await sleep(500);
  o.modal = await ev(`return modal();`);
  o.inline = await ev(`const s = document.querySelector('[data-cdp="row"]'); const i = s && s.querySelector('[contenteditable], input'); return i ? { tag: i.tagName, text: i.textContent || i.value, dom: dump(s, 0) } : null;`);
  o.shot = await shot('10-edit');
  const before = bytes(BM);
  if (o.modal) {
    await c.evaluate(`const i = document.querySelector('.modal-container input'); i.focus(); i.select(); return true;`);
    await c.typeText('Beta renamed', 25); await c.pressKey('Enter'); await sleep(400);
    o.afterEnter = await ev(`return modal();`);
    if (o.afterEnter) { o.shotAfterEnter = await shot('10-edit-after-enter'); await closeAll(); }
  } else if (o.inline) {
    await c.typeText('Beta renamed', 25); await c.pressKey('Enter'); await sleep(400);
  }
  o.changed = await waitForChange(BM, before, 4000);
  o.file = bmFile().raw;
  o.rows = await ev(`return rows().map(r => r.text);`);
  return o;
});

await step('s11b-edit-modal', async () => {
  const o = {};
  for (const [name, text] of [['file', 'Alpha'], ['url', 'Example site'], ['search', 'alpha'], ['heading', 'Alpha#Second heading']]) {
    const r = await rowRect(text);
    await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(500);
    await clickMenuItem('Edit...'); await sleep(500);
    o[name] = await ev(`return modal();`);
    if (name === 'file') o.shotFile = await shot('10-edit-modal-file');
    if (name === 'url') o.shotUrl = await shot('10-edit-modal-url');
    await closeAll(); await sleep(200);
  }
  // Rename then Escape: does Escape revert?
  const before = bytes(BM);
  const r = await rowRect('Example site');
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(500);
  await clickMenuItem('Rename'); await sleep(300);
  await c.typeText('Nope', 25); await c.pressKey('Escape'); await sleep(500);
  o.escapeRename = { changed: bytes(BM) !== before, rows: await ev(`return rows().map(r => r.text);`) };
  // Rename to empty: what does an empty title write?
  const r2 = await rowRect('Example site').catch(() => null);
  if (r2) {
    const before2 = bytes(BM);
    await c.clickAt(r2.cx, r2.cy, { button: 'right' }); await sleep(500);
    await clickMenuItem('Rename'); await sleep(300);
    await c.pressKey('Backspace'); await c.pressKey('Enter'); await sleep(600);
    o.emptyRename = { changed: await waitForChange(BM, before2, 2500), rows: await ev(`return rows().map(r => r.text);`), item: (bmFile().parsed.items.find(i => i.type === 'url') || null) };
  }
  // Blur commits?
  const r3 = await rowRect('Graph view');
  const before3 = bytes(BM);
  await c.clickAt(r3.cx, r3.cy, { button: 'right' }); await sleep(500);
  await clickMenuItem('Rename'); await sleep(300);
  await c.typeText('Graph blurred', 25); await c.clickAt(700, 500); await sleep(600);
  o.blurRename = { changed: await waitForChange(BM, before3, 2500), rows: await ev(`return rows().map(r => r.text);`) };
  return o;
}, 60000);

await step('s12-new-group', async () => {
  const o = {};
  const label = (await ev(`return headerButtons().map(b => b.label);`)).find(l => /group/i.test(l || ''));
  if (!label) return { skipped: 'no new group button' };
  const before = bytes(BM);
  await clickButton(label); await sleep(400);
  o.modal = await ev(`return modal();`);
  o.shot = await shot('11-new-group');
  o.renaming = await ev(`const s = pane().querySelector('.is-being-renamed'); const sel = window.getSelection(); return s ? { cls: s.className, text: s.textContent, focused: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className + ' ce=' + document.activeElement.getAttribute('contenteditable')), selection: sel && sel.toString(), dom: dump(s, 0) } : null;`);
  await c.typeText('Made group', 25); await sleep(200);
  o.typed = await ev(`const s = pane().querySelector('.is-being-renamed'); return s && s.textContent;`);
  await c.pressKey('Enter'); await sleep(500);
  if (o.modal) { o.afterEnter = await ev(`return modal();`); if (o.afterEnter) await closeAll(); }
  o.changed = await waitForChange(BM, before, 4000);
  o.file = bmFile().raw;
  o.rows = await ev(`return rows().map(r => ({ text: r.text, depth: r.depth, itemCls: r.itemCls }));`);
  // The group row's own menu has "New group"? and the header button on a selected group?
  return o;
});

await step('s13-group-menu-actions', async () => {
  const o = {};
  const r = await rowRect('Made group').catch(() => rowRect('Untitled group')).catch(() => null);
  if (!r) return { skipped: 'no made group' };
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(500);
  o.menu = await ev(`return menuDom();`);
  await closeAll();
  return o;
});

// ---- drags -----------------------------------------------------------------------
await step('s14-drag-indicator', async () => {
  const o = {};
  const a = await rowRect('Gamma').catch(async () => rowRect(await ev(`return rows().map(x => x.text).find(t => t && t.startsWith('A custom'));`)));
  const tgt = await rowRect('Alpha');
  const above = await c.dragOverOnly(a.cx, a.cy, tgt.cx, tgt.y + 3);
  o.above = { started: above.started, data: above.data && { items: above.data.items, dragOperationsMask: above.data.dragOperationsMask }, dom: await ev(`return { indicators: Array.from(document.querySelectorAll('.drag-ghost, .drop-indicator, .is-being-dragged-over, .is-being-dragged, .mod-drop-above, .mod-drop-below, .drag-reorder-ghost, .workspace-drop-overlay')).map(e => ({ cls: e.className, rect: rect(e), style: cs(e, ['background-color','border','border-top','border-bottom','height','box-shadow','outline','opacity']) })), ghost: (document.querySelector('.drag-ghost') || {}).textContent || null };`), shot: await shot('12-drag-above-alpha') };
  if (above.finish) await above.finish();
  await sleep(400);
  const g = await rowRect('Group one');
  const into = await c.dragOverOnly(a.cx, a.cy, g.cx, g.cy);
  o.intoGroup = { started: into.started, dom: await ev(`return { indicators: Array.from(document.querySelectorAll('.drop-indicator, .is-being-dragged-over, .is-being-dragged, .mod-drop-above, .mod-drop-below, .drag-ghost')).map(e => ({ cls: e.className, rect: rect(e), style: cs(e, ['background-color','border','height','box-shadow','outline']) })) };`), shot: await shot('12-drag-into-group') };
  if (into.finish) await into.finish();
  await sleep(400);
  return o;
});

await step('s15-drag-reorder', async () => {
  const o = {};
  let before = bytes(BM);
  const a = await rowRect(await ev(`return rows().map(x => x.text).find(t => t && t.startsWith('A custom'));`));
  const tgt = await rowRect('Alpha');
  o.drop1 = await c.dragAndDrop(a.cx, a.cy, tgt.cx, tgt.y + 3);
  o.reorder = { changed: await waitForChange(BM, before, 4000), rows: await ev(`return rows().map(r => ({ text: r.text, depth: r.depth }));`), file: bmFile().parsed && bmFile().parsed.items.map(i => i.title || i.path || i.query || i.url || i.type) };
  before = bytes(BM);
  const b = await rowRect('Image.png');
  const g = await rowRect('Group one');
  o.drop2 = await c.dragAndDrop(b.cx, b.cy, g.cx, g.cy);
  o.intoGroup = { changed: await waitForChange(BM, before, 4000), rows: await ev(`return rows().map(r => ({ text: r.text, depth: r.depth }));`), raw: bytes(BM) };
  o.shot = await shot('13-after-drags');
  return o;
}, 45000);

await step('s16-drag-from-files', async () => {
  const o = {};
  await c.evaluate(`app.workspace.leftSplit.expand(); return true;`);
  // Files and Bookmarks share the left dock: split it so both are visible.
  o.layout = await c.evaluate(`const bl = app.workspace.getLeavesOfType('bookmarks')[0]; const fe = app.workspace.getLeavesOfType('file-explorer')[0]; if (bl.parent === fe.parent) { const nl = app.workspace.getLeftLeaf(true); await nl.setViewState({ type: 'bookmarks', active: true }); bl.detach(); } app.workspace.revealLeaf(app.workspace.getLeavesOfType('file-explorer')[0]); app.workspace.revealLeaf(app.workspace.getLeavesOfType('bookmarks')[0]); return { same: app.workspace.getLeavesOfType('bookmarks')[0].parent === app.workspace.getLeavesOfType('file-explorer')[0].parent };`);
  await sleep(900);
  const src = await c.rect('.nav-file-title[data-path="Unbookmarked.md"]');
  const g = await rowRect('Alpha');
  if (!src || !g) return { ...o, skipped: 'no source or target', src, g };
  const before = bytes(BM);
  const over = await c.dragOverOnly(src.cx, src.cy, g.cx, g.y + 3);
  o.over = { started: over.started, mime: over.data && over.data.items && over.data.items.map(i => i.mimeType), dom: await ev(`return Array.from(document.querySelectorAll('.drop-indicator, .is-being-dragged-over, .mod-drop-above, .mod-drop-below')).map(e => ({ cls: e.className, rect: rect(e) }));`), shot: await shot('14-drag-from-files') };
  if (over.finish) await over.finish(g.cx, g.y + 3);
  o.changed = await waitForChange(BM, before, 4000);
  o.rows = await ev(`return rows().map(r => ({ text: r.text, depth: r.depth }));`);
  o.file = bmFile().parsed && bmFile().parsed.items.map(i => ({ type: i.type, path: i.path, title: i.title }));
  o.raw = bytes(BM);
  return o;
}, 45000);

// ---- the keyboard ---------------------------------------------------------------
await step('s17-keyboard', async () => {
  const o = {};
  await c.evaluate(`const l = app.workspace.getLeavesOfType('bookmarks')[0]; app.workspace.revealLeaf(l); app.workspace.setActiveLeaf(l, { focus: true }); return true;`); await sleep(500);
  const focusInfo = () => ev(`return { focused: document.activeElement && (document.activeElement.tagName + '.' + document.activeElement.className), marked: marked(), counts: leafCounts() };`);
  o.start = await focusInfo();
  for (const k of ['ArrowDown', 'ArrowDown', 'ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft', 'End', 'Home', 'Enter']) {
    await c.pressKey(k); await sleep(350);
    o[k + '_' + Object.keys(o).length] = await focusInfo();
  }
  o.shot = await shot('15-keyboard');
  await c.evaluate(`await app.commands.executeCommandById('bookmarks:open'); return true;`); await sleep(400);
  return o;
});

// ---- the light theme ------------------------------------------------------------
await step('s18-light', async () => {
  await c.evaluate(`app.changeTheme ? app.changeTheme('moonstone') : app.setTheme('moonstone'); app.workspace.leftSplit.expand(); await app.commands.executeCommandById('bookmarks:open'); return true;`); await sleep(1200);
  const o = { shot: await shot('16-light'), vars: await ev(`return vars(pane(), ${JSON.stringify(VARS)});`), rows: await ev(`return rows().slice(0, 4).map(r => ({ text: r.text, selfStyle: r.selfStyle, innerStyle: r.innerStyle, iconStyle: r.iconStyle }));`), marked: await ev(`return marked();`) };
  const r = await rowRect('Beta renamed').catch(() => rowRect('Beta'));
  await c.mouseMove(r.cx, r.cy); await sleep(500);
  o.hover = await ev(`const s = document.querySelector('[data-cdp="row"]'); return { bg: getComputedStyle(s).backgroundColor, color: getComputedStyle(s.querySelector('.tree-item-inner')).color };`);
  o.shotHover = await shot('16-light-hover');
  await c.mouseMove(5, 400);
  await c.evaluate(`app.changeTheme ? app.changeTheme('obsidian') : app.setTheme('obsidian'); return true;`); await sleep(800);
  return o;
});

// ---- delete and the empty state ------------------------------------------------------
await step('s19-delete', async () => {
  const o = {};
  const before = bytes(BM);
  const r = await rowRect('Image.png');
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(500);
  o.menu = await c.openMenuItems();
  const del = (o.menu[0] || []).find(l => /^(Delete|Remove)/i.test(l));
  if (!del) { await closeAll(); return { ...o, skipped: 'no delete item' }; }
  o.item = del;
  await clickMenuItem(del); await sleep(400);
  o.modal = await ev(`return modal();`);
  if (o.modal) { o.shot = await shot('17-delete-confirm'); await c.pressKey('Enter'); await sleep(400); await closeAll(); }
  o.changed = await waitForChange(BM, before, 4000);
  o.rows = await ev(`return rows().map(r => r.text);`);
  // Delete a non-empty group: confirm? its members?
  const before2 = bytes(BM);
  const g = await rowRect('Group one');
  await c.clickAt(g.cx, g.cy, { button: 'right' }); await sleep(500);
  const gm = await c.openMenuItems();
  const gdel = (gm[0] || []).find(l => /^(Delete|Remove)/i.test(l));
  if (gdel) {
    await clickMenuItem(gdel); await sleep(400);
    o.groupModal = await ev(`return modal();`);
    if (o.groupModal) { o.shotGroup = await shot('17-delete-group-confirm'); await c.pressKey('Enter'); await sleep(400); await closeAll(); }
    o.groupChanged = await waitForChange(BM, before2, 4000);
    o.groupRows = await ev(`return rows().map(r => ({ text: r.text, depth: r.depth }));`);
  } else await closeAll();
  return o;
});

await step('s20-empty', async () => {
  await c.evaluate(`const inst = app.internalPlugins.getPluginById('bookmarks').instance; inst.removeItems ? inst.removeItems(inst.items.slice()) : inst.items.splice(0); inst.onItemsChanged && inst.onItemsChanged(true); inst.saveData && inst.saveData(); return inst.items.length;`);
  await sleep(1200);
  return { shot: await shot('18-empty'), dom: await ev(`return dump(pane(), 0);`), headers: await ev(`return headerButtons();`), file: bytes(BM) };
});

await c.close();
save();
process.exit(0);
