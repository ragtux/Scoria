// Copy path's submenu, which bases-cells-search-menu.mjs read on one match
// row only: its rows and what each copies for every kind of file a Search
// result can stand for (a note, a note whose name needs escaping, a canvas),
// whether a row raises a notice, and the same submenu in the Files view on a
// note, a canvas, a PDF and a folder, to tell whether the two are one.
//
// Fixtures: `search/` holds `One.md`, `Two words & #2.md`, `Board.canvas`
// (a text card) and `Paper.pdf`, the first three with `needle` in them, and
// `search/Sub/` an empty-ish folder with one note; `Linker.md` at the root
// links to `[[One]]`.
//
//   s00  context: version, window
//   s01  the Search pane: the result rows, and for each file row the Copy
//        path submenu dumped and each of its rows clicked: clipboard, notice
//   s02  the same from a match row of One.md
//   s03  the Files view: the menu's Copy path row on each file and the
//        folder, its submenu if any, and each row's clipboard and notice
//   s04  the tab header's menu and the view header's More options on a
//        note, a canvas and a PDF, each open in the one root leaf
//   s05  a wikilink's menu in Live Preview and in Source mode (a right
//        click on `[[One]]` in `Linker.md`), and in the reading view
//   s06  a Backlinks pane row's menu (One.md's backlink from Linker.md)
//
// ONLY_STEPS=<regex> narrows the run; HOLD=1 holds it open for hand probing
// until `<results>.stop` exists. Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run'].includes(phase)) { console.error('phase must be fixtures|run'); process.exit(2); }
const vp = (rel) => path.join(VAULT, rel);

// A one-page PDF with no text, written by hand: enough for the Files view
// to list it and open its menu.
const PDF = [
  '%PDF-1.4',
  '1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj',
  '2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj',
  '3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj',
  'trailer<</Root 1 0 R>>',
  '%%EOF',
  '',
].join('\n');
const FILES = {
  'search/One.md': 'The first needle.\n\nA second needle line.\n',
  'search/Two words & #2.md': 'Another needle here.\n',
  'search/Board.canvas': JSON.stringify({ nodes: [{ id: 'a1', type: 'text', text: 'needle on a card', x: 0, y: 0, width: 250, height: 60 }], edges: [] }, null, '\t'),
  'search/Paper.pdf': PDF,
  'search/Sub/Inner.md': 'Nothing to find.\n',
  'Linker.md': 'See [[One]] for the first.\n',
};

if (phase === 'fixtures') {
  for (const [rel, text] of Object.entries(FILES)) {
    fs.mkdirSync(path.dirname(vp(rel)), { recursive: true });
    fs.writeFileSync(vp(rel), text);
  }
  process.exit(0);
}

const T0 = Date.now();
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 180000) {
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
  console.error(`[copy-path] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const c = await connect();
const want = VAULT.replace(/\/+$/, '');
const got = (await c.vaultPath()).replace(/\/+$/, '');
if (got !== want) { console.error(`wrong vault: ${got}`); process.exit(3); }

// HOLD=1 keeps the run open for hand probing until `<results>.stop` exists.
if (process.env.HOLD) {
  console.error('[copy-path] holding; touch ' + RESULTS + '.stop to finish');
  while (!fs.existsSync(RESULTS + '.stop')) await sleep(1000);
  c.close(); process.exit(0);
}

// ---- helpers ---------------------------------------------------------------
const menuDump = () => c.evaluate(`
  return [...document.querySelectorAll('.menu')].map(m => ({
    items: [...m.querySelectorAll('.menu-item, .menu-separator')].map(e => e.classList.contains('menu-separator') ? { separator: true } : {
      title: (e.querySelector('.menu-item-title') || e).textContent.trim(),
      section: e.dataset.section || null,
      cls: e.className,
      icon: e.querySelector('.menu-item-icon svg') ? [...e.querySelector('.menu-item-icon svg').classList].join(' ') : null,
    }),
  }));`);
const scrub = (s) => (s == null ? s : s.split(VAULT).join('<vault>').replace(/vault=[^&]+/, 'vault=<name>'));
const clearNotices = () => c.evaluate(`document.querySelectorAll('.notice').forEach(n => n.remove()); return true;`);
const notices = () => c.evaluate(`return [...document.querySelectorAll('.notice')].map(n => n.textContent.trim());`);
const openSearch = async (q) => c.evaluate(`
  app.workspace.detachLeavesOfType('markdown');
  app.internalPlugins.getPluginById('global-search').instance.openGlobalSearch(${JSON.stringify(q)});
  await new Promise(r => setTimeout(r, 2500)); return true;`);
const closeAll = async () => { await c.pressKey('Escape'); await sleep(250); await c.pressKey('Escape'); await sleep(250); };
const rowCentre = (title, last) => c.evaluate(`
  const ms = document.querySelectorAll('.menu'); const m = ${last ? 'ms[ms.length - 1]' : 'ms[0]'};
  if (!m) return null;
  const it = [...m.querySelectorAll('.menu-item')].find(e => e.textContent.trim() === ${JSON.stringify(title)});
  if (!it) return null; const b = it.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);

// Opens the menu with `open`, hovers Copy path, and reads its submenu; then
// for each of its rows reopens and clicks it, reading the clipboard and any
// notice. A Copy path without a submenu is clicked as it is.
async function copyPathOf(open) {
  const r = {};
  await closeAll(); await open();
  r.menu = await menuDump();
  const cp = await rowCentre('Copy path', false);
  if (!cp) { r.copyPath = null; return r; }
  await c.mouseMove(cp.cx, cp.cy); await sleep(800);
  const menus = await menuDump();
  r.submenu = menus.length > 1 ? menus[menus.length - 1] : null;
  r.copied = {};
  const titles = r.submenu ? r.submenu.items.filter((i) => i.title).map((i) => i.title) : [null];
  for (const t of titles) {
    await closeAll(); await open();
    await c.writeClipboard('SENTINEL'); await clearNotices();
    let at = await rowCentre('Copy path', false);
    if (!at) { await closeAll(); await open(); await c.writeClipboard('SENTINEL'); await clearNotices(); at = await rowCentre('Copy path', false); }
    if (!at) { r.copied[t || 'Copy path'] = { menuMissing: true }; continue; }
    await c.mouseMove(at.cx, at.cy); await sleep(800);
    if (t) {
      const sub = await rowCentre(t, true);
      if (!sub) { r.copied[t] = { missing: true }; continue; }
      await c.mouseMove(sub.cx - 20, sub.cy); await sleep(200);
      await c.clickAt(sub.cx, sub.cy);
    } else {
      await c.clickAt(at.cx, at.cy);
    }
    await sleep(600);
    r.copied[t || 'Copy path'] = {
      clipboard: scrub(await c.readClipboard()),
      notices: (await notices()).map(scrub),
      menusLeft: (await menuDump()).length,
    };
  }
  await closeAll();
  return r;
}

// ---- steps -----------------------------------------------------------------
await step('s00-context', async () => ({
  version: await c.evaluate(`return require('electron').ipcRenderer.sendSync('version');`).catch(() => null),
  apiVersion: await c.evaluate(`return window.require('obsidian').apiVersion;`).catch(() => null),
  title: await c.evaluate(`return document.title;`),
}));

await step('s01-search-file-rows', async (p) => {
  await openSearch('needle');
  p.rows = await c.evaluate(`return [...document.querySelectorAll('.search-result-file-title, .search-result-file-match')].map(e => ({ cls: e.className, text: e.textContent.trim().slice(0, 60) }));`);
  const n = await c.evaluate(`return document.querySelectorAll('.search-result-file-title').length;`);
  p.files = [];
  for (let i = 0; i < n; i++) {
    const name = await c.evaluate(`return document.querySelectorAll('.search-result-file-title')[${i}].textContent.trim();`);
    p.files.push({
      name,
      ...(await copyPathOf(async () => {
        await openSearch('needle');
        const r = await c.rect('.search-result-file-title', i);
        await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600);
      })),
    });
  }
  return p;
});

await step('s02-search-match-row', async () => copyPathOf(async () => {
  await openSearch('needle');
  const r = await c.rect('.search-result-file-match', 0);
  await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600);
}));

await step('s03-files-view', async (p) => {
  await c.evaluate(`app.workspace.leftSplit.expand(); const l = app.workspace.getLeavesOfType('file-explorer')[0]; app.workspace.revealLeaf(l);
    l.view.revealInFolder(app.vault.getAbstractFileByPath('search/One.md')); await new Promise(r => setTimeout(r, 800)); return true;`);
  p.targets = {};
  for (const rel of ['search/One.md', 'search/Two words & #2.md', 'search/Board.canvas', 'search/Paper.pdf', 'search/Sub']) {
    p.targets[rel] = await copyPathOf(async () => {
      const r = await c.evaluate(`const e = document.querySelector('.nav-file-title[data-path=${JSON.stringify(rel).replace(/"/g, '\\"')}], .nav-folder-title[data-path=${JSON.stringify(rel).replace(/"/g, '\\"')}]');
        if (!e) return null; e.scrollIntoView({ block: 'center' }); const b = e.getBoundingClientRect(); return { cx: b.x + b.width / 2, cy: b.y + b.height / 2 };`);
      if (!r) throw new Error('no row for ' + rel);
      await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600);
    });
  }
  return p;
});

// Opens `rel` in the root split's most recent leaf, in `mode` for a note.
// Detaching every leaf first left no active root leaf in time for the
// selectors below (the first run's s04-s06 all missed), so the one leaf is
// reused instead.
const openIn = async (rel, mode) => c.evaluate(`
  const leaf = app.workspace.getMostRecentLeaf(app.workspace.rootSplit) || app.workspace.getLeaf(false);
  await leaf.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(rel)}), ${JSON.stringify(mode ? { state: { mode: mode === 'preview' ? 'preview' : 'source', source: mode === 'source' } } : {})});
  app.workspace.setActiveLeaf(leaf, { focus: true });
  await new Promise(r => setTimeout(r, 1500)); return true;`);
const at = (sel) => c.evaluate(`const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; const b = e.getBoundingClientRect(); return { cx: b.x + Math.min(b.width / 2, 12), cy: b.y + b.height / 2 };`);
const waitAt = async (sel) => { for (let i = 0; i < 20; i++) { const r = await at(sel); if (r) return r; await sleep(250); } return null; };
const rightAt = async (sel) => { const r = await waitAt(sel); if (!r) throw new Error('nothing at ' + sel); await c.clickAt(r.cx, r.cy, { button: 'right' }); await sleep(600); };
const leftAt = async (sel) => { const r = await waitAt(sel); if (!r) throw new Error('nothing at ' + sel); await c.clickAt(r.cx, r.cy); await sleep(600); };

await step('s04-tab-and-view-header', async (p) => {
  for (const rel of ['search/One.md', 'search/Board.canvas', 'search/Paper.pdf']) {
    p[rel] = {
      tab: await copyPathOf(async () => { await openIn(rel); await rightAt('.mod-root .workspace-tab-header.is-active'); }),
      viewHeader: await copyPathOf(async () => { await openIn(rel); await leftAt('.mod-root .workspace-leaf.mod-active .view-header .view-action[aria-label="More options"]'); }),
    };
  }
  return p;
});

await step('s05-wikilink', async (p) => {
  p.livePreview = await copyPathOf(async () => { await openIn('Linker.md', 'live'); await rightAt('.mod-root .workspace-leaf.mod-active .cm-hmd-internal-link'); });
  p.source = await copyPathOf(async () => { await openIn('Linker.md', 'source'); await rightAt('.mod-root .workspace-leaf.mod-active .cm-hmd-internal-link'); });
  p.reading = await copyPathOf(async () => { await openIn('Linker.md', 'preview'); await rightAt('.mod-root .workspace-leaf.mod-active .markdown-reading-view a.internal-link'); });
  return p;
});

await step('s06-backlinks-row', async () => copyPathOf(async () => {
  await openIn('search/One.md', 'live');
  await c.evaluate(`app.workspace.rightSplit.expand(); let l = app.workspace.getLeavesOfType('backlink')[0];
    if (!l) { l = app.workspace.getRightLeaf(false); await l.setViewState({ type: 'backlink', state: { file: 'search/One.md' } }); }
    app.workspace.revealLeaf(l); await new Promise(r => setTimeout(r, 1500)); return true;`);
  await rightAt('.workspace-leaf-content[data-type="backlink"] .search-result-file-title');
}));

c.close();
process.exit(0);
