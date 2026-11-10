// workspace:toggle-stacked-tabs: a tab group's stacked mode.
//
//   s00  context: version, the command's name, hotkey and when it is offered
//   s01  three tabs, the middle active: workspace.json before, after one run,
//        after a second run (the bytes of the group node), the DOM classes
//   s02  six tabs stacked: the strip's geometry with each tab active in turn —
//        header rects, the leaves' rects, the container's scroll, the
//        header's inner markup and computed styles, every custom property
//        whose name holds "stack" on the stacked .workspace-tabs
//   s03  a click on an inactive header; the wheel over the stack
//   s04  the tab commands while stacked: next/previous tab, new tab, close,
//        go to tab N, split right (does the new group stack?)
//   s05  the tab-list menu's row and the tab header's right-click menu
//   s06  a sidebar group (the right dock): offered? what changes?
//   s07  a popout: move-to-new-window, then stack it; the floating node bytes
//   s08  the window narrowed: the geometry at 900 px
//   after-restart: does the stacked group come back, and in what DOM
//
// Screenshots go to <results>-shots/. ONLY_STEPS=<regex> narrows the run.
// Phases: fixtures | run | after-restart
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 120000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[stacked] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  const names = ['Alpha', 'Beta', 'Gamma', 'Delta', 'Epsilon', 'Zeta'];
  for (const n of names) {
    const body = Array.from({ length: 12 }, (_, i) => `${n} paragraph ${i + 1}.`).join('\n\n');
    fs.writeFileSync(vp(n + '.md'), `# ${n}\n\n${body}\n`);
  }
  fs.writeFileSync(vp('A rather long note title for the stacked spine.md'), '# Long\n\nText.\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    outline: true, canvas: true, bookmarks: true, 'command-palette': true, 'file-recovery': true,
    'page-preview': true, 'tag-pane': true, properties: true,
  }, null, 2));
  process.exit(0);
}

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

const HELPERS = `
window.__t = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x * 100) / 100, Math.round(r.y * 100) / 100, Math.round(r.width * 100) / 100, Math.round(r.height * 100) / 100]; },
  leafInfo(l) {
    const v = l.view; const vs = l.getViewState();
    return { type: v.getViewType(), file: v.file ? v.file.path : null, title: l.getDisplayText(), active: app.workspace.activeLeaf === l };
  },
  rootLeaves() { const o = []; app.workspace.iterateRootLeaves((l) => { o.push(this.leafInfo(l)); }); return o; },
  offered(id) { return app.commands.listCommands().some((x) => x.id === id); },
  async setup(files, activeIndex) {
    app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse();
    const leaves = []; app.workspace.iterateRootLeaves((l) => { leaves.push(l); });
    // Unstack any stacked group first.
    const seen = new Set(); for (const l of leaves) { try { if (l.parent && !seen.has(l.parent) && seen.add(l.parent) && l.parent.containerEl && l.parent.containerEl.classList.contains('mod-stacked')) { app.workspace.setActiveLeaf(l); app.commands.executeCommandById('workspace:toggle-stacked-tabs'); } } catch (e) {} }
    for (const l of leaves.slice(1)) l.detach();
    await this.sleep(200);
    const keep = []; app.workspace.iterateRootLeaves((l) => { keep.push(l); });
    let leaf = keep[0] || app.workspace.getLeaf(false);
    for (const l of keep.slice(1)) l.detach();
    const made = [];
    for (let i = 0; i < files.length; i++) {
      const target = i === 0 ? leaf : app.workspace.createLeafInParent(leaf.parent, leaf.parent.children.length);
      if (files[i] === null) await target.setViewState({ type: 'empty', state: {} });
      else await target.openFile(app.vault.getAbstractFileByPath(files[i]));
      made.push(target);
    }
    const act = made[activeIndex === undefined ? made.length - 1 : activeIndex];
    app.workspace.setActiveLeaf(act, { focus: true });
    await this.sleep(600);
    return this.rootLeaves();
  },
  stackVars(el) {
    const cs = getComputedStyle(el); const o = {};
    for (let i = 0; i < cs.length; i++) { const p = cs[i]; if (p.startsWith('--') && /stack|tab/.test(p)) o[p] = cs.getPropertyValue(p).trim(); }
    return o;
  },
  geometry(doc) {
    doc = doc || document;
    const tabs = doc.querySelector('.mod-root .workspace-tabs');
    if (!tabs) return null;
    const hc = tabs.querySelector('.workspace-tab-header-container');
    const hci = tabs.querySelector('.workspace-tab-header-container-inner');
    const tc = tabs.querySelector('.workspace-tab-container');
    const st = (e, props) => { if (!e) return null; const cs = getComputedStyle(e); const o = {}; for (const p of props) o[p] = cs.getPropertyValue(p); return o; };
    const HP = ['position', 'left', 'right', 'top', 'width', 'height', 'min-width', 'max-width', 'flex', 'z-index', 'writing-mode', 'transform', 'background-color', 'border-left', 'border-right', 'box-shadow', 'padding', 'margin', 'display', 'opacity', 'cursor'];
    const LP = ['position', 'left', 'right', 'width', 'min-width', 'max-width', 'flex', 'z-index', 'box-shadow', 'border-left', 'border-right', 'background-color', 'display', 'margin', 'padding'];
    const headers = Array.from(tabs.querySelectorAll('.workspace-tab-header'));
    const leaves = Array.from(tabs.querySelectorAll(':scope > .workspace-tab-container > .workspace-leaf, .workspace-tab-container > .workspace-leaf'));
    return {
      tabsCls: this.cls(tabs), tabsRect: this.rect(tabs),
      headerContainer: hc ? { cls: this.cls(hc), rect: this.rect(hc), style: st(hc, ['display', 'height', 'width', 'position']) } : null,
      headerContainerInner: hci ? { cls: this.cls(hci), rect: this.rect(hci) } : null,
      tabContainer: tc ? { cls: this.cls(tc), rect: this.rect(tc), scrollLeft: tc.scrollLeft, scrollWidth: tc.scrollWidth, clientWidth: tc.clientWidth, style: st(tc, ['display', 'flex-direction', 'overflow-x', 'overflow-y', 'position', 'scroll-snap-type']) } : null,
      headers: headers.map((h) => {
        const inner = h.querySelector('.workspace-tab-header-inner');
        const title = h.querySelector('.workspace-tab-header-inner-title');
        const icon = h.querySelector('.workspace-tab-header-inner-icon');
        const close = h.querySelector('.workspace-tab-header-inner-close-button');
        return {
          label: h.getAttribute('aria-label'), cls: this.cls(h), parent: this.cls(h.parentElement), rect: this.rect(h), style: st(h, HP),
          inner: inner ? { rect: this.rect(inner), style: st(inner, ['writing-mode', 'transform', 'flex-direction', 'padding', 'display', 'height', 'width']) } : null,
          title: title ? { text: title.textContent, rect: this.rect(title), style: st(title, ['writing-mode', 'transform', 'text-orientation', 'font-size', 'font-weight', 'color', 'padding', 'white-space', 'overflow', 'text-overflow']) } : null,
          icon: icon ? { rect: this.rect(icon), style: st(icon, ['display', 'transform', 'padding']) } : null,
          close: close ? { rect: this.rect(close), style: st(close, ['display', 'transform', 'padding']) } : null,
        };
      }),
      leaves: leaves.map((l) => ({ cls: this.cls(l), rect: this.rect(l), style: st(l, LP),
        childHeader: (() => { const h = l.querySelector(':scope > .workspace-tab-header, .workspace-tab-header'); return h ? { label: h.getAttribute('aria-label'), cls: this.cls(h), rect: this.rect(h) } : null; })(),
        viewHeader: this.rect(l.querySelector('.view-header')), viewContent: this.rect(l.querySelector('.view-content')),
        children: Array.from(l.children).map((ch) => this.cls(ch)) })),
      vars: this.stackVars(tabs),
    };
  },
  headerSkeleton(h) {
    const desc = (e, d = 0) => ({ tag: e.tagName.toLowerCase(), cls: e.getAttribute('class'), aria: e.getAttribute('aria-label'), text: e.children.length ? undefined : (e.textContent || '').trim().slice(0, 60),
      children: d < 5 ? Array.from(e.children).filter((c) => !['path', 'line', 'circle', 'rect', 'polyline'].includes(c.tagName.toLowerCase())).map((c) => desc(c, d + 1)) : [] });
    return desc(h);
  },
  menus(doc) {
    doc = doc || document;
    return Array.from(doc.querySelectorAll('.menu')).map((m) => ({ cls: this.cls(m), rows: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((e) => {
      if (e.classList.contains('menu-separator')) return { sep: true };
      const t = e.querySelector('.menu-item-title'); const svg = e.querySelector('.menu-item-icon svg');
      return { title: t ? t.textContent.trim() : e.textContent.trim(), section: e.getAttribute('data-section'), icon: svg ? svg.getAttribute('class') : null, checked: e.classList.contains('mod-checked') };
    }) }));
  },
  row(title) {
    const ms = Array.from(document.querySelectorAll('.menu')); const m = ms[ms.length - 1]; if (!m) return null;
    const it = Array.from(m.querySelectorAll('.menu-item')).find((e) => ((e.querySelector('.menu-item-title') || e).textContent || '').trim() === title);
    if (!it) return null; const r = it.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2 };
  },
};
return true;`;
await c.evaluate(HELPERS);
const ev = (s) => c.evaluate(s);
const T = 'const T = window.__t;';
let shotN = 0;
async function shot(name, cl = c) {
  try {
    const r = await cl.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
async function wsRaw() {
  const before = bytes('.obsidian/workspace.json');
  await ev(`app.workspace.requestSaveLayout(); return true;`);
  const t = Date.now();
  while (Date.now() - t < 6000) { const b = bytes('.obsidian/workspace.json'); if (b !== before) { await sleep(400); break; } await sleep(100); }
  await sleep(300);
  return bytes('.obsidian/workspace.json');
}
function mainRaw(raw) { if (!raw) return null; const a = raw.indexOf('"main"'); const b = raw.indexOf('"left"'); return raw.slice(a, b); }
const toggle = async () => { const r = await ev(`return app.commands.executeCommandById('workspace:toggle-stacked-tabs');`); await sleep(900); return r; };
if (phase === 'after-restart') {
  await sleep(1500);
  await step('r00-restored', async () => ({
    geometry: await ev(`${T} return T.geometry();`),
    leaves: await ev(`${T} return T.rootLeaves();`),
    isStacked: await ev(`const l = app.workspace.activeLeaf; return !!(l && l.parent && l.parent.isStacked);`),
    shot: await shot('after-restart'),
    ws: mainRaw(bytes('.obsidian/workspace.json')),
  }));
  save(); c.close(); process.exit(0);
}
const dismiss = async () => { await c.pressKey('Escape'); await sleep(250); await ev(`for (const m of document.querySelectorAll('.menu')) m.remove(); return true;`); await sleep(150); };

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, apiVersion: (typeof apiVersion !== 'undefined' ? apiVersion : null), ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio };`);
  const cmd = await ev(`const x = app.commands.commands['workspace:toggle-stacked-tabs']; return x ? { name: x.name, icon: x.icon || null, keys: (app.hotkeyManager.defaultKeys || {})['workspace:toggle-stacked-tabs'] || null, hasCheck: !!x.checkCallback, hasCallback: !!x.callback } : null;`);
  const offeredMd = await ev(`${T} await T.setup(['Alpha.md'], 0); return T.offered('workspace:toggle-stacked-tabs');`);
  const offeredEmpty = await ev(`${T} await T.setup([null], 0); return T.offered('workspace:toggle-stacked-tabs');`);
  return { version: out.version, cmd, offeredMd, offeredEmpty };
});

await step('s01-toggle-bytes', async () => {
  const leaves = await ev(`${T} return await T.setup(['Alpha.md', 'Beta.md', 'Gamma.md'], 1);`);
  const before = mainRaw(await wsRaw());
  const domBefore = await ev(`${T} return T.geometry();`);
  const shotBefore = await shot('three-unstacked');
  const ran = await toggle();
  const after = mainRaw(await wsRaw());
  const domAfter = await ev(`${T} return T.geometry();`);
  const apiAfter = await ev(`const l = app.workspace.activeLeaf; return { isStacked: l.parent && l.parent.isStacked, parentType: l.parent && l.parent.type, active: l.getDisplayText() };`);
  const shotAfter = await shot('three-stacked');
  const skeleton = await ev(`${T} return Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')).map((h) => T.headerSkeleton(h));`);
  const leafSkeleton = await ev(`${T} const l = document.querySelector('.mod-root .workspace-tabs .workspace-leaf'); return l ? Array.from(l.children).map((ch) => T.headerSkeleton(ch)).map((x) => ({ ...x, children: x.cls && x.cls.includes('workspace-tab-header') ? x.children : '(omitted)' })) : null;`);
  const ran2 = await toggle();
  const after2 = mainRaw(await wsRaw());
  const domAfter2 = await ev(`${T} return T.geometry();`);
  const activeAfter2 = await ev(`return app.workspace.activeLeaf.getDisplayText();`);
  return { leaves, before, ran, after, ran2, after2, domBefore, domAfter, apiAfter, skeleton, leafSkeleton, domAfter2: { tabsCls: domAfter2.tabsCls, headerParents: domAfter2.headers.map((h) => h.parent) }, activeAfter2, shotBefore, shotAfter };
});

await step('s02-six-geometry', async () => {
  await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Gamma.md', 'Delta.md', 'Epsilon.md', 'Zeta.md'], 0); return true;`);
  await toggle();
  const per = [];
  for (let i = 0; i < 6; i++) {
    await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); app.workspace.setActiveLeaf(ls[${i}], { focus: true }); return true;`);
    await sleep(900);
    per.push({ active: i, geometry: await ev(`${T} return T.geometry();`), shot: await shot('six-active-' + i) });
  }
  return { per };
});

await step('s03-click-wheel', async () => {
  await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Gamma.md', 'Delta.md', 'Epsilon.md', 'Zeta.md'], 0); return true;`);
  await toggle();
  const res = {};
  const g0 = await ev(`${T} return T.geometry();`);
  res.start = { scrollLeft: g0.tabContainer.scrollLeft, headers: g0.headers.map((h) => [h.label, h.rect]) };
  // Click the header of tab 4 (Epsilon).
  const hr = g0.headers[4].rect;
  await c.clickAt(hr[0] + hr[2] / 2, hr[1] + Math.min(hr[3] / 2, 60));
  const samples = [];
  for (let k = 0; k < 8; k++) { await sleep(60); samples.push(await ev(`const tc = document.querySelector('.mod-root .workspace-tab-container'); return [Date.now(), tc.scrollLeft];`)); }
  await sleep(600);
  const g1 = await ev(`${T} return T.geometry();`);
  res.afterHeaderClick = { active: await ev(`return app.workspace.activeLeaf.getDisplayText();`), scrollLeft: g1.tabContainer.scrollLeft, samples, headers: g1.headers.map((h) => [h.label, h.rect, h.cls]), shot: await shot('after-click-epsilon') };
  // Click a header to the left (Alpha).
  const ha = g1.headers[0].rect;
  await c.clickAt(ha[0] + ha[2] / 2, ha[1] + Math.min(ha[3] / 2, 60)); await sleep(900);
  const g2 = await ev(`${T} return T.geometry();`);
  res.afterHeaderClickLeft = { active: await ev(`return app.workspace.activeLeaf.getDisplayText();`), scrollLeft: g2.tabContainer.scrollLeft, headers: g2.headers.map((h) => [h.label, h.rect]) };
  // The wheel over the content of the stack: vertical, then horizontal deltas.
  const tc = g2.tabContainer.rect;
  const mx = tc[0] + tc[2] / 2, my = tc[1] + tc[3] / 2;
  await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: mx, y: my, deltaX: 0, deltaY: 300 }); await sleep(700);
  res.afterWheelY = await ev(`const tc = document.querySelector('.mod-root .workspace-tab-container'); return { scrollLeft: tc.scrollLeft, active: app.workspace.activeLeaf.getDisplayText() };`);
  await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: mx, y: my, deltaX: 300, deltaY: 0 }); await sleep(700);
  res.afterWheelX = await ev(`const tc = document.querySelector('.mod-root .workspace-tab-container'); return { scrollLeft: tc.scrollLeft, active: app.workspace.activeLeaf.getDisplayText() };`);
  await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: mx, y: my, deltaX: 0, deltaY: 300, modifiers: 8 }); await sleep(700);
  res.afterWheelShiftY = await ev(`const tc = document.querySelector('.mod-root .workspace-tab-container'); return { scrollLeft: tc.scrollLeft, active: app.workspace.activeLeaf.getDisplayText() };`);
  res.shotEnd = await shot('after-wheel');
  // Close button on a stacked header: present? click it.
  const cb = await ev(`const hs = Array.from(document.querySelectorAll('.mod-root .workspace-tab-header')); const h = hs[2]; const b = h.querySelector('.workspace-tab-header-inner-close-button'); if (!b) return null; const r = b.getBoundingClientRect(); return { label: h.getAttribute('aria-label'), cx: r.x + r.width / 2, cy: r.y + r.height / 2, display: getComputedStyle(b).display, rect: [r.x, r.y, r.width, r.height] };`);
  res.closeButton = cb;
  if (cb && cb.rect[2] > 0) {
    await c.mouseMove(cb.cx, cb.cy); await sleep(300);
    await c.clickAt(cb.cx, cb.cy); await sleep(900);
    res.afterCloseButton = await ev(`${T} return T.rootLeaves();`);
  }
  return res;
});

await step('s04-tab-commands', async () => {
  const res = {};
  const state = async () => ev(`${T} const g = T.geometry(); return { leaves: T.rootLeaves().map((l) => (l.active ? '*' : '') + l.title), tabsCls: g && g.tabsCls, groups: document.querySelectorAll('.mod-root .workspace-tabs').length, stackedGroups: document.querySelectorAll('.mod-root .workspace-tabs.mod-stacked').length, scrollLeft: g && g.tabContainer.scrollLeft };`);
  await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Gamma.md'], 1); return true;`);
  await toggle();
  res.start = await state();
  await ev(`return app.commands.executeCommandById('workspace:next-tab');`); await sleep(700); res.nextTab = await state();
  await ev(`return app.commands.executeCommandById('workspace:previous-tab');`); await sleep(700); res.previousTab = await state();
  await ev(`return app.commands.executeCommandById('workspace:goto-tab-1');`); await sleep(700); res.gotoTab1 = await state();
  await ev(`return app.commands.executeCommandById('workspace:goto-last-tab');`); await sleep(700); res.gotoLast = await state();
  await ev(`return app.commands.executeCommandById('workspace:goto-tab-1');`); await sleep(500);
  await ev(`return app.commands.executeCommandById('workspace:new-tab');`); await sleep(900); res.newTab = await state();
  res.newTabShot = await shot('new-tab-while-stacked');
  await ev(`return app.commands.executeCommandById('workspace:close');`); await sleep(900); res.closeNewTab = await state();
  // Open a file from the quick switcher into the active stacked tab.
  await ev(`const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); app.workspace.setActiveLeaf(ls[0], { focus: true }); return true;`); await sleep(400);
  await ev(`return app.commands.executeCommandById('switcher:open');`); await sleep(700);
  await c.typeText('Delta'); await sleep(500); await c.pressKey('Enter'); await sleep(1200);
  res.afterSwitcher = await state();
  await ev(`return app.commands.executeCommandById('workspace:split-vertical');`); await sleep(1200); res.splitRight = await state();
  res.splitShot = await shot('split-right-while-stacked');
  res.splitWs = mainRaw(await wsRaw());
  // Close the tab group.
  await ev(`return app.commands.executeCommandById('workspace:close-tab-group');`); await sleep(900); res.closeGroup = await state();
  return res;
});

await step('s05-menus', async () => {
  const res = {};
  await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Gamma.md'], 1); return true;`);
  // Tab list menu, unstacked.
  const tl = await c.rect('.mod-root .workspace-tab-header-tab-list');
  res.tabListRect = tl;
  if (tl) { await c.clickAt(tl.cx, tl.cy); await sleep(700); res.tabListUnstacked = await ev(`${T} return T.menus();`); await dismiss(); }
  // Click "Stack tabs" in it.
  if (tl) {
    await c.clickAt(tl.cx, tl.cy); await sleep(700);
    const r = await ev(`${T} return T.row('Stack tabs');`);
    if (r) { await c.clickAt(r.cx, r.cy); await sleep(900); }
    res.afterStackRow = await ev(`${T} return T.geometry().tabsCls;`);
    res.afterStackRowWs = mainRaw(await wsRaw());
  }
  const tl2 = await c.rect('.mod-root .workspace-tab-header-tab-list');
  res.tabListRectStacked = tl2;
  if (tl2) { await c.clickAt(tl2.cx, tl2.cy); await sleep(700); res.tabListStacked = await ev(`${T} return T.menus();`); res.tabListStackedShot = await shot('tab-list-stacked'); await dismiss(); }
  // Right-click a stacked header.
  const g = await ev(`${T} return T.geometry();`);
  const h = g.headers[1].rect;
  await c.mouseMove(h[0] + h[2] / 2, h[1] + 40); await sleep(150);
  await c.clickAt(h[0] + h[2] / 2, h[1] + 40, { button: 'right' }); await sleep(700);
  res.headerMenuStacked = await ev(`${T} return T.menus();`);
  await dismiss();
  // The header strip itself: where does the new-tab button go?
  res.stripButtons = await ev(`${T} return Array.from(document.querySelectorAll('.mod-root .workspace-tabs .workspace-tab-header-container > *, .mod-root .workspace-tabs .workspace-tab-header-container-inner ~ *')).map((e) => ({ cls: T.cls(e), rect: T.rect(e), display: getComputedStyle(e).display }));`);
  // Unstack via the menu row.
  const tl3 = await c.rect('.mod-root .workspace-tab-header-tab-list');
  if (tl3) {
    await c.clickAt(tl3.cx, tl3.cy); await sleep(700);
    const r = await ev(`${T} return T.row('Unstack tabs');`);
    if (r) { await c.clickAt(r.cx, r.cy); await sleep(900); }
    res.afterUnstackRow = await ev(`${T} return T.geometry().tabsCls;`);
  }
  return res;
});

await step('s06-sidebar', async () => {
  const res = {};
  await ev(`${T} await T.setup(['Alpha.md'], 0); return true;`);
  res.setup = await ev(`
    const rs = app.workspace.rightSplit; rs.expand();
    for (const t of ['backlink', 'outline']) { if (!app.workspace.getLeavesOfType(t).length) { const l = app.workspace.getRightLeaf(false); await l.setViewState({ type: t, active: true }); } }
    await new Promise((r) => setTimeout(r, 800));
    const l = app.workspace.getLeavesOfType('outline')[0]; app.workspace.setActiveLeaf(l, { focus: true });
    await new Promise((r) => setTimeout(r, 400));
    return { active: app.workspace.activeLeaf.getViewState().type, offered: app.commands.listCommands().some((x) => x.id === 'workspace:toggle-stacked-tabs') };`);
  res.ran = await toggle();
  res.after = await ev(`return { sideCls: Array.from(document.querySelectorAll('.mod-right-split .workspace-tabs')).map((e) => e.className), rootCls: Array.from(document.querySelectorAll('.mod-root .workspace-tabs')).map((e) => e.className), active: app.workspace.activeLeaf.getViewState().type };`);
  res.shot = await shot('sidebar-after-toggle');
  res.ws = await wsRaw();
  if (/mod-stacked/.test(JSON.stringify(res.after.sideCls))) { await toggle(); }
  res.afterSecond = await ev(`return { sideCls: Array.from(document.querySelectorAll('.mod-right-split .workspace-tabs')).map((e) => e.className), rootCls: Array.from(document.querySelectorAll('.mod-root .workspace-tabs')).map((e) => e.className) };`);
  await ev(`for (const l of document.querySelectorAll('.mod-root .workspace-tabs.mod-stacked')) {} const ls = []; app.workspace.iterateRootLeaves((l) => { ls.push(l); }); app.workspace.rightSplit.collapse(); return true;`);
  return res;
});

await step('s07-popout', async () => {
  const res = {};
  await ev(`${T} await T.setup(['Alpha.md', 'Beta.md'], 0); return true;`);
  const tb = await pageTargets();
  await ev(`return app.commands.executeCommandById('workspace:move-to-new-window');`); await sleep(2500);
  const ta = await pageTargets();
  const pop = ta.find((u) => !tb.includes(u));
  res.popoutTarget = !!pop;
  await ev(`const w = app.workspace.floatingSplit.children[0]; const l = []; app.workspace.iterateAllLeaves((x) => { if (x.getContainer() === w) l.push(x); }); return true;`);
  // Add a second tab in the popout, make it active, stack.
  res.setup = await ev(`const w = app.workspace.floatingSplit.children[0]; let leaf = null; app.workspace.iterateAllLeaves((x) => { if (x.getContainer() === w) leaf = x; });
    const n = app.workspace.createLeafInParent(leaf.parent, leaf.parent.children.length); await n.openFile(app.vault.getAbstractFileByPath('Gamma.md'));
    app.workspace.setActiveLeaf(n, { focus: true }); await new Promise((r) => setTimeout(r, 600));
    return { offered: app.commands.listCommands().some((x) => x.id === 'workspace:toggle-stacked-tabs'), active: app.workspace.activeLeaf.getDisplayText() };`);
  res.ran = await toggle();
  res.popCls = await ev(`const w = app.workspace.floatingSplit.children[0]; return Array.from(w.doc.querySelectorAll('.workspace-tabs')).map((e) => e.className);`);
  res.mainCls = await ev(`return Array.from(document.querySelectorAll('.mod-root .workspace-tabs')).map((e) => e.className);`);
  const raw = await wsRaw();
  res.floatingRaw = raw ? raw.slice(raw.indexOf('"floating"'), raw.indexOf('"active"')) : null;
  if (pop) { const p = await connect(pop); res.shot = await shot('popout-stacked', p); p.close(); }
  return res;
});

await step('s08-narrow', async () => {
  // Close popouts, six tabs stacked, then shrink the window.
  await ev(`for (const ch of [...app.workspace.floatingSplit.children]) { try { ch.win.close(); } catch (e) {} } await new Promise((r) => setTimeout(r, 1200)); return true;`);
  await ev(`${T} await T.setup(['Alpha.md', 'Beta.md', 'Gamma.md', 'Delta.md', 'Epsilon.md', 'Zeta.md', 'A rather long note title for the stacked spine.md'], 3); return true;`);
  await toggle();
  const res = {};
  res.wide = await ev(`${T} return T.geometry();`);
  res.wideShot = await shot('seven-wide-active-3');
  // Leave it stacked for the after-restart phase.
  res.wsFinal = mainRaw(await wsRaw());
  return res;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
// Quit so after-restart can see what comes back.
if (!ONLY) { try { await c.quit(); } catch { /* ignore */ } }
try { c.close(); } catch { /* ignore */ }
process.exit(0);
