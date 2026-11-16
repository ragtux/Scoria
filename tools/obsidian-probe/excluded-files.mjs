// Excluded files (`userIgnoreFilters` in app.json): the filter syntax, which
// surfaces hide or demote an excluded file, and the Settings row's Manage
// modal (its markup, strings, and the app.json bytes each edit writes).
//
//   s00  context: version, the key's default, the API that answers "is this ignored"
//   s01  the matcher: ~20 filter shapes against ~25 paths
//   s02  global search: what a query lists with and without the filter
//   s03  quick switcher: the rows (and their classes) for several queries
//   s04  link autocompletion: the suggester's items for several queries,
//        and the rows a typed `[[` draws
//   s05  graph view and local graph: the nodes
//   s06  backlinks pane: linked and unlinked mentions of Target
//   s07  tags pane, file explorer, Bases table, resolvedLinks
//   s08  the Settings row (a navigable row in 1.15: it opens a sub-page of
//        the Settings window, which is a popout window of its own), the
//        sub-page, the Add exclusion modal, adds by Enter, duplicates and a
//        blank, deletes down to none, the app.json bytes after each
//   s09  the modal's folder suggestions, Done with a padded value, Cancel,
//        Done with nothing typed, a picked folder, the row's "N rules"
//   s10  the switcher's recent files, a downranked row's colour, the
//        outgoing pane's unlinked mentions, an excluded note's own panes
//
// The Settings window lives in another document: everything in s08-s09 goes
// through `row.ownerDocument` (kept as `window.__xfdoc`), and its inputs are
// driven by setting the value and dispatching events in that window, since
// CDP input goes to the main page.
//
// ONLY_STEPS=<regex> narrows the run. Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 180000) {
  if (ONLY && !ONLY.test(name)) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[excluded] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const write = (rel, s) => { fs.mkdirSync(path.dirname(vp(rel)), { recursive: true }); fs.writeFileSync(vp(rel), s); };
const APP_JSON = '{\n  "userIgnoreFilters": [\n    "Archive/"\n  ],\n  "probeUnknownKey": {\n    "kept": true\n  }\n}';

if (phase === 'fixtures') {
  write('.obsidian/app.json', APP_JSON);
  write('Keep.md', 'zebra keep. Mentions Target here.\n[[Target]]\n#keeptag\n');
  write('Target.md', '# Target\nzebra target body\n');
  write('Archive/Old.md', 'zebra old. Mentions Target here.\n[[Target]]\n#oldtag\n');
  write('Archive/Older.md', 'zebra older, also Target unlinked.\n');
  write('Archive/Deep/Deepest.md', 'zebra deepest Target\n');
  write('Archived note.md', 'zebra archived Target\n');
  write('archive lower/lower.md', 'zebra lower Target\n');
  write('Other/Archive/Nested.md', 'zebra nested Target\n');
  write('Old twin.md', 'zebra twin of Old\n');
  write('Data/Table.base', 'views:\n  - type: table\n    name: Table\n');
  write('Archive/pic.png', 'not really a png');
  write('Mentioner.md', 'This mentions Older and Target and Deepest here.\n');
  process.exit(0);
}

const c = await connect();
const ev = (s) => c.evaluate(s);
const appJson = () => fs.readFileSync(vp('.obsidian/app.json'), 'utf8');

await step('s00', async () => ev(`
  const proto = (o) => { const s = new Set(); let p = o; while (p && p !== Object.prototype) { for (const k of Object.getOwnPropertyNames(p)) s.add(k); p = Object.getPrototypeOf(p); } return [...s]; };
  const pick = (o) => proto(o).filter((k) => /ignor|exclud|filter/i.test(k));
  return {
    title: document.title, ua: navigator.userAgent,
    defaultFilters: app.vault.getConfig('userIgnoreFilters'),
    metadataCache: pick(app.metadataCache), vault: pick(app.vault), workspace: pick(app.workspace),
    hasIsUserIgnored: typeof app.metadataCache.isUserIgnored,
    fields: Object.keys(app.metadataCache).filter((k) => /ignor|filter/i.test(k)),
    ignoreState: Object.fromEntries(Object.keys(app.metadataCache).filter((k) => /ignor|filter/i.test(k)).map((k) => [k, String(app.metadataCache[k]).slice(0, 300)])),
  };`));

const PATHS = ['Archive/Old.md', 'Archive/Deep/Deepest.md', 'Archive', 'Archive.md', 'Archived note.md',
  'archive lower/lower.md', 'archive/x.md', 'Other/Archive/Nested.md', 'Keep.md', 'Old twin.md', 'Archive/pic.png',
  'Data/Table.base', 'a.png', 'Notes/Sub/x.md', 'Notes/Subway.md', 'Notes/sub/x.md', 'x.md', 'Archive Old.md',
  'Drafts/2024/a.md', 'Folder.with.dots/a.md', 'Daily/2024-01-01.md', 'Daily/notes.md', '.hidden/a.md'];
const FILTERS = [[], ['Archive/'], ['Archive'], ['/Archive'], ['archive/'], ['Archive/Old.md'], ['Archive/Old'],
  ['/arch/'], ['/Arch/'], ['/arch/i'], ['/^Archive/'], ['/Old\\.md$/'], ['/[/'], ['*.png'], ['/\\.png$/'],
  ['Notes/Sub'], ['Notes/Sub/'], [' Archive/'], [''], ['/'], ['/Daily\\/\\d{4}/'], ['Archive/', 'Keep.md'],
  ['Folder.with.dots'], ['Archive/ '], ['ARCHIVE/'], ['/^archive\\/old/'], ['\tArchive/'], ['/ Old/'], ['./Archive'], ['Archive\\'], ['//'], ['/.*/']];

await step('s01', async (partial) => {
  const rows = [];
  partial.rows = rows;
  for (const f of FILTERS) {
    const r = await ev(`
      app.vault.setConfig('userIgnoreFilters', ${JSON.stringify(f)});
      await new Promise((r) => setTimeout(r, 150));
      const paths = ${JSON.stringify(PATHS)};
      const res = {};
      for (const p of paths) { try { res[p] = app.metadataCache.isUserIgnored(p); } catch (e) { res[p] = 'throws ' + e.message; } }
      return { compiled: String(app.metadataCache.userIgnoreFilters), filters: ${JSON.stringify(f)}, ignored: paths.filter((p) => res[p] === true), odd: Object.fromEntries(Object.entries(res).filter(([, v]) => typeof v !== 'boolean')) };`);
    rows.push(r);
  }
  await ev(`app.vault.setConfig('userIgnoreFilters', ['Archive/']); return true;`);
  await sleep(800);
  return { rows, appJsonAfter: appJson() };
});

const searchOnce = (q) => ev(`
  const gs = app.internalPlugins.getPluginById('global-search').instance;
  gs.openGlobalSearch(${JSON.stringify(q)});
  await new Promise((r) => setTimeout(r, 2500));
  const leaf = app.workspace.getLeavesOfType('search')[0];
  const v = leaf.view;
  const files = [...v.containerEl.querySelectorAll('.search-result-file-title .tree-item-inner')].map((e) => e.textContent);
  const lookup = v.dom && v.dom.resultDomLookup ? [...v.dom.resultDomLookup.keys()].map((f) => f.path) : null;
  const summary = (v.containerEl.querySelector('.search-results-info') || {}).textContent || null;
  return { q: ${JSON.stringify(q)}, files, lookup, summary, text: v.containerEl.innerText.slice(0, 600) };`);

await step('s02', async () => {
  const withFilter = [];
  for (const q of ['zebra', 'path:Archive zebra', 'file:Old', 'Target', '/zeb.a/']) withFilter.push(await searchOnce(q));
  await ev(`app.vault.setConfig('userIgnoreFilters', []); return true;`);
  await sleep(800);
  const without = [await searchOnce('zebra'), await searchOnce('path:Archive zebra')];
  await ev(`app.vault.setConfig('userIgnoreFilters', ['Archive/']); return true;`);
  await sleep(800);
  const again = [await searchOnce('zebra')];
  return { withFilter, without, again };
});

const switcherRows = async (q) => {
  await ev(`app.commands.executeCommandById('switcher:open'); return true;`);
  await sleep(600);
  await c.typeText(q, 40);
  await sleep(700);
  const r = await ev(`
    const items = [...document.querySelectorAll('.prompt .suggestion-item')].map((e) => ({ text: e.innerText.replace(/\\n/g, ' | '), cls: e.className, html: e.outerHTML.slice(0, 500) }));
    return { items };`);
  await c.pressKey('Escape');
  await sleep(300);
  return { q, ...r };
};

await step('s03', async () => {
  const rows = [];
  for (const q of ['', 'old', 'o', 'arch', 'deep', 'pic', 'zzz']) rows.push(await switcherRows(q));
  const internal = await ev(`
    const m = app.internalPlugins.getPluginById('switcher');
    return { keys: m ? Object.keys(m.instance || {}) : null };`);
  return { rows, internal };
});

await step('s04', async () => {
  const api = await ev(`
    const s = app.workspace.editorSuggest.suggests[0];
    const leaf = app.workspace.getLeaf(false);
    await leaf.openFile(app.vault.getAbstractFileByPath('Keep.md'));
    await new Promise((r) => setTimeout(r, 600));
    const editor = leaf.view.editor; const file = leaf.view.file;
    const res = {};
    for (const q of ['Old', 'O', '', 'Deep', 'Archive', 'pic']) {
      const ctx = { query: q, start: { line: 0, ch: 0 }, end: { line: 0, ch: q.length + 2 }, editor, file };
      s.context = ctx;
      let items = await s.getSuggestions(ctx);
      res[q] = (items || []).slice(0, 25).map((it) => { const o = {}; for (const k of Object.keys(it)) { const v = it[k]; o[k] = v && typeof v === 'object' ? (v.path || (Array.isArray(v) ? JSON.stringify(v).slice(0, 80) : Object.keys(v).join(','))) : v; } return o; });
    }
    return res;`);
  // Real typing: the rows a `[[O` draws.
  const typed = await (async () => {
    await ev(`
      const leaf = app.workspace.getMostRecentLeaf();
      const e = leaf.view.editor; e.setCursor({ line: 3, ch: 0 }); e.focus(); return true;`);
    await sleep(300);
    const shots = {};
    for (const q of ['O', 'Deep']) {
      await c.typeText('[[' + q, 60);
      await sleep(900);
      shots[q] = await ev(`return [...document.querySelectorAll('.suggestion-container .suggestion-item')].map((e) => ({ text: e.innerText.replace(/\\n/g, ' | '), cls: e.className, html: e.outerHTML.slice(0, 600) }));`);
      await c.pressKey('Escape');
      for (let i = 0; i < q.length + 4; i++) await c.pressKey('Backspace');
      await sleep(200);
    }
    return shots;
  })();
  const after = await ev(`return app.vault.adapter.read('Keep.md');`);
  return { api, typed, keepAfter: after };
});

await step('s05', async () => ev(`
  app.workspace.getLeavesOfType('graph').forEach((l) => l.detach());
  app.commands.executeCommandById('graph:open');
  await new Promise((r) => setTimeout(r, 3500));
  const g = app.workspace.getLeavesOfType('graph')[0];
  const nodes = g && g.view.renderer ? g.view.renderer.nodes.map((n) => n.id) : null;
  const leaf = app.workspace.getLeaf('tab');
  await leaf.openFile(app.vault.getAbstractFileByPath('Target.md'));
  await new Promise((r) => setTimeout(r, 500));
  app.commands.executeCommandById('graph:open-local');
  await new Promise((r) => setTimeout(r, 3500));
  const lg = app.workspace.getLeavesOfType('localgraph')[0];
  const lnodes = lg && lg.view.renderer ? lg.view.renderer.nodes.map((n) => n.id) : null;
  return { nodes, localNodes: lnodes };`));

await step('s06', async () => ev(`
  const leaf = app.workspace.getLeaf(false);
  await leaf.openFile(app.vault.getAbstractFileByPath('Target.md'));
  app.workspace.setActiveLeaf(leaf, { focus: true });
  app.commands.executeCommandById('backlink:open');
  await new Promise((r) => setTimeout(r, 1500));
  const bl = app.workspace.getLeavesOfType('backlink')[0].view;
  const b = bl.backlink;
  const keys = Object.keys(b);
  // Expand unlinked mentions.
  if (b.unlinkedCollapsed !== undefined && b.unlinkedCollapsed) { b.setUnlinkedCollapsed ? b.setUnlinkedCollapsed(false) : null; }
  const head = [...bl.containerEl.querySelectorAll('.tree-item-self')].find((e) => /Unlinked mentions/.test(e.textContent));
  if (head && head.parentElement.classList.contains('is-collapsed')) head.click();
  await new Promise((r) => setTimeout(r, 2500));
  const files = (dom) => dom && dom.resultDomLookup ? [...dom.resultDomLookup.keys()].map((f) => f.path) : null;
  return { keys, linked: files(b.backlinkDom), unlinked: files(b.unlinkedDom), text: bl.containerEl.innerText.slice(0, 900) };`));

await step('s07', async () => ev(`
  app.commands.executeCommandById('tag-pane:open');
  await new Promise((r) => setTimeout(r, 1200));
  const tp = app.workspace.getLeavesOfType('tag')[0];
  const tags = tp ? tp.view.containerEl.innerText : null;
  const getTags = app.metadataCache.getTags ? app.metadataCache.getTags() : null;
  app.commands.executeCommandById('file-explorer:open');
  await new Promise((r) => setTimeout(r, 600));
  const fe = app.workspace.getLeavesOfType('file-explorer')[0].view;
  const explorer = Object.keys(fe.fileItems);
  const explorerCls = Object.fromEntries(Object.entries(fe.fileItems).map(([k, v]) => [k, v.el.className + ' / ' + (v.selfEl ? v.selfEl.className : '')]).filter(([k]) => /Archive/.test(k)));
  const leaf = app.workspace.getLeaf('tab');
  await leaf.openFile(app.vault.getAbstractFileByPath('Data/Table.base'));
  await new Promise((r) => setTimeout(r, 2500));
  const baseRows = [...leaf.view.containerEl.querySelectorAll('.bases-tr, .bases-table-row, [class*="bases"] .bases-td:first-child')].map((e) => e.innerText.replace(/\\n/g, ' | ')).slice(0, 40);
  const baseText = leaf.view.containerEl.innerText.slice(0, 800);
  return { tags, getTags, explorer, explorerCls, baseRows, baseText,
    resolvedKeep: app.metadataCache.resolvedLinks['Keep.md'], resolvedOld: app.metadataCache.resolvedLinks['Archive/Old.md'],
    dest: (app.metadataCache.getFirstLinkpathDest('Old', 'Keep.md') || {}).path || null,
    random: (() => { const r = app.internalPlugins.getPluginById('random-note'); return r ? r.enabled : null; })() };`));

const findRow = `const items = [...app.setting.activeTab.containerEl.querySelectorAll('.setting-item')];
    const row = items.find((e) => (e.querySelector('.setting-item-name') || {}).textContent === 'Excluded files');`;
const pane = `const root = app.setting.containerEl;
    const ctl = [...root.querySelectorAll('input, button, select, textarea, [contenteditable], .clickable-icon, .setting-item.mod-navigable')].filter((e) => e.offsetParent !== null).map((e) => ({ tag: e.tagName, type: e.type, cls: e.className && e.className.baseVal === undefined ? e.className : '', placeholder: e.placeholder, value: e.value, text: (e.innerText || '').slice(0, 80), aria: e.getAttribute('aria-label') }));`;

await step('s08', async (partial) => {
  const res = partial;
  res.open = await ev(`
    const st = [];
    const probe = (t) => st.push({ t, open: !!(app.setting.containerEl && app.setting.containerEl.isConnected), mods: document.querySelectorAll('.modal').length, tab: app.setting.activeTab && app.setting.activeTab.id });
    app.setting.open(); probe('afterOpen'); app.setting.openTabById('file'); probe('afterTab');
    await new Promise((r) => setTimeout(r, 800)); probe('800');
    window.__xfst = st;
    ${findRow}
    row.scrollIntoView();
    await new Promise((r) => setTimeout(r, 300));
    const r = row.getBoundingClientRect();
    probe('end'); const chain = []; for (let e = row; e; e = e.parentElement) chain.push(e.tagName + '.' + String(e.className).replace(/ /g, '.')); window.__xfdoc = row.ownerDocument; return { chain, st, x: r.x + r.width / 2, y: r.y + r.height / 2 };`);
  res.before = await ev(`return { settingOpen: !!(app.setting.containerEl && app.setting.containerEl.isConnected), mods: document.querySelectorAll('.modal').length };`);
  res.poll = await ev(`${findRow}
    const log = [];
    row.click();
    for (let i = 0; i < 15; i++) {
      await new Promise((r) => setTimeout(r, 100));
      log.push({ t: i * 100, settingOpen: !!(app.setting.containerEl && app.setting.containerEl.isConnected), mods: [...document.querySelectorAll('.modal')].map((m) => m.className + ' :: ' + m.innerText.slice(0, 300)) });
    }
    return log;`);
  await sleep(1000);
  res.after = await ev(`
    const D = window.__xfdoc;
    const mods = [...D.querySelectorAll('.setting-page, .modal-container, .menu, .suggestion-container')].map((e) => ({ cls: e.className, text: e.innerText.slice(0, 2500), html: e.outerHTML.slice(0, 14000) }));
    const ctl = [...D.querySelectorAll('.setting-page input, .setting-page button, .setting-page .clickable-icon, .setting-page .setting-item')].filter((e) => e.offsetParent !== null).map((e) => ({ tag: e.tagName, type: e.type, cls: String(e.className), placeholder: e.placeholder, value: e.value, text: (e.innerText || '').slice(0, 80), aria: e.getAttribute('aria-label') }));
    return { mods, ctl, active: D.activeElement && D.activeElement.outerHTML.slice(0, 400), title: D.title };`);
  const dump = `const D2 = window.__xfdoc;
    const grab = (doc) => [...doc.querySelectorAll('.setting-page, .modal-container, .menu, .suggestion-container, .prompt')].map((e) => ({ cls: e.className, text: e.innerText.slice(0, 2000), html: e.outerHTML.slice(0, 9000) }));
    const res = { popout: grab(D2), main: grab(document), active: D2.activeElement && D2.activeElement.outerHTML.slice(0, 500), mainActive: document.activeElement && document.activeElement.outerHTML.slice(0, 300) };`;
  res.addClick = await ev(`
    const D = window.__xfdoc;
    const b = D.querySelector('.setting-page [aria-label="Add exclusion..."]');
    b.click();
    await new Promise((r) => setTimeout(r, 900));
    ${dump}
    return res;`);
  // Type into whatever input took focus (or the first visible one), in the window it lives in.
  const typeInto = async (text, key) => ev(`
    const D = window.__xfdoc;
    const cands = [D.activeElement, ...D.querySelectorAll('input, textarea'), document.activeElement, ...document.querySelectorAll('.modal-container input, .prompt input, .menu input')];
    const el = cands.find((e) => e && (e.tagName === 'INPUT' || e.tagName === 'TEXTAREA') && e.offsetParent !== null);
    if (!el) return { input: null };
    el.focus();
    const W = el.ownerDocument.defaultView;
    const setter = Object.getOwnPropertyDescriptor(W.HTMLInputElement.prototype, 'value').set;
    setter.call(el, ${JSON.stringify(text)});
    el.dispatchEvent(new W.Event('input', { bubbles: true }));
    await new Promise((r) => setTimeout(r, 500));
    const before = { value: el.value, sugg: [...el.ownerDocument.querySelectorAll('.suggestion-item')].map((x) => x.innerText + ' :: ' + x.className) };
    ${key ? "el.dispatchEvent(new W.KeyboardEvent('keydown', { key: " + JSON.stringify(key) + ", code: " + JSON.stringify(key) + ", bubbles: true, cancelable: true }));" : ''}
    await new Promise((r) => setTimeout(r, 900));
    return { input: el.outerHTML.slice(0, 400), inPopout: el.ownerDocument === D, before, config: app.vault.getConfig('userIgnoreFilters') };`);
  res.type1 = await typeInto('Drafts/', null);
  res.type1Dump = await ev(`${dump} return res;`);
  res.enter1 = await typeInto('Drafts/', 'Enter');
  res.enter1Dump = await ev(`${dump} return res;`);
  await sleep(800);
  res.appJsonAfterAdd = appJson();
  // A second add: a regex.
  res.add2 = await ev(`const D = window.__xfdoc; const b = D.querySelector('.setting-page [aria-label="Add exclusion..."]'); if (b) b.click(); await new Promise((r) => setTimeout(r, 900)); ${dump} return res;`);
  res.enter2 = await typeInto('/\\.png$/', 'Enter');
  await sleep(800);
  res.appJsonAfterAdd2 = appJson();
  // Add the same entry again, and an empty one.
  res.add3 = await ev(`const D = window.__xfdoc; const b = D.querySelector('.setting-page [aria-label="Add exclusion..."]'); if (b) b.click(); await new Promise((r) => setTimeout(r, 900)); return true;`);
  res.enter3 = await typeInto('Drafts/', 'Enter');
  res.add4 = await ev(`const D = window.__xfdoc; const b = D.querySelector('.setting-page [aria-label="Add exclusion..."]'); if (b) b.click(); await new Promise((r) => setTimeout(r, 900)); return true;`);
  res.enter4 = await typeInto('   ', 'Enter');
  res.afterDupDump = await ev(`${dump} return res;`);
  await sleep(800);
  res.appJsonAfterDup = appJson();
  res.searchAfterAdd = await searchOnce('zebra').catch((e) => String(e));
  // Delete the first entry.
  res.del = await ev(`
    const D = window.__xfdoc;
    const x = D.querySelector('.setting-page .setting-items [aria-label="Delete"]');
    if (!x) return { del: null };
    x.click();
    await new Promise((r) => setTimeout(r, 900));
    ${dump}
    res.config = app.vault.getConfig('userIgnoreFilters');
    return res;`);
  await sleep(800);
  res.appJsonAfterDelete = appJson();
  // Delete everything.
  res.delAll = await ev(`
    const D = window.__xfdoc;
    for (let i = 0; i < 10; i++) { const x = D.querySelector('.setting-page .setting-items [aria-label="Delete"]'); if (!x) break; x.click(); await new Promise((r) => setTimeout(r, 500)); }
    ${dump}
    res.config = app.vault.getConfig('userIgnoreFilters');
    return res;`);
  await sleep(800);
  res.appJsonAfterDeleteAll = appJson();
  // Back to the Files tab: the row's value with nothing excluded.
  res.back = await ev(`
    const D = window.__xfdoc;
    const b = D.querySelector('.setting-page-back-button'); if (b) b.click();
    await new Promise((r) => setTimeout(r, 900));
    const rows = [...D.querySelectorAll('.setting-item')].filter((e) => (e.querySelector('.setting-item-name') || {}).textContent === 'Excluded files');
    return { rows: rows.map((r) => r.outerHTML) };`);
  return res;
});

await step('s09', async (partial) => {
  const res = partial;
  await ev(`app.vault.setConfig('userIgnoreFilters', ['Archive/', 'Data', 'Keep.md']); return true;`);
  await sleep(800);
  res.open = await ev(`
    app.setting.open(); app.setting.openTabById('file');
    await new Promise((r) => setTimeout(r, 900));
    ${findRow}
    window.__xfdoc = row.ownerDocument;
    const html = row.outerHTML;
    row.click();
    await new Promise((r) => setTimeout(r, 900));
    return { rowHtml: html, list: [...window.__xfdoc.querySelectorAll('.setting-page .setting-items .setting-item-name')].map((e) => e.textContent) };`);
  const openAdd = `const D = window.__xfdoc; const b = D.querySelector('.setting-page [aria-label="Add exclusion..."]'); b.click(); await new Promise((r) => setTimeout(r, 900));`;
  const suggest = async (text) => ev(`
    const D = window.__xfdoc;
    const el = D.querySelector('.modal-container input');
    const W = D.defaultView;
    Object.getOwnPropertyDescriptor(W.HTMLInputElement.prototype, 'value').set.call(el, ${JSON.stringify(text)});
    el.dispatchEvent(new W.Event('input', { bubbles: true }));
    await new Promise((r) => setTimeout(r, 600));
    return [...D.querySelectorAll('.suggestion-container .suggestion-item')].map((x) => ({ text: x.innerText, html: x.outerHTML.slice(0, 400) }));`);
  await ev(`${openAdd} return true;`);
  res.sugg = {};
  for (const q of ['', 'A', 'Arch', 'o', 'Archive/', 'Other/Ar', 'Keep', '/x/']) res.sugg[q] = await suggest(q);
  // Done with a padded value, the suggestion list still open.
  await suggest('  Padded/  ');
  res.done = await ev(`
    const D = window.__xfdoc;
    [...D.querySelectorAll('.modal-container button')].find((b) => b.textContent === 'Done').click();
    await new Promise((r) => setTimeout(r, 900));
    return { config: app.vault.getConfig('userIgnoreFilters'), modals: D.querySelectorAll('.modal-container').length, list: [...D.querySelectorAll('.setting-page .setting-items .setting-item-name')].map((e) => e.textContent) };`);
  // Cancel.
  await ev(`${openAdd} return true;`);
  await suggest('Cancelled/');
  res.cancel = await ev(`
    const D = window.__xfdoc;
    [...D.querySelectorAll('.modal-container button')].find((b) => b.textContent === 'Cancel').click();
    await new Promise((r) => setTimeout(r, 900));
    return { config: app.vault.getConfig('userIgnoreFilters'), modals: D.querySelectorAll('.modal-container').length };`);
  // Done with nothing typed.
  await ev(`${openAdd} return true;`);
  res.emptyDone = await ev(`
    const D = window.__xfdoc;
    [...D.querySelectorAll('.modal-container button')].find((b) => b.textContent === 'Done').click();
    await new Promise((r) => setTimeout(r, 900));
    return { config: app.vault.getConfig('userIgnoreFilters'), modals: D.querySelectorAll('.modal-container').length, notices: [...document.querySelectorAll('.notice'), ...D.querySelectorAll('.notice')].map((n) => n.innerText), html: (D.querySelector('.modal-container') || {}).outerHTML || null };`);
  await ev(`const D = window.__xfdoc; const m = D.querySelector('.modal-container'); if (m) [...m.querySelectorAll('button')].find((b) => b.textContent === 'Cancel').click(); return true;`);
  // Pick a suggestion row by clicking it.
  await ev(`${openAdd} return true;`);
  await suggest('Oth');
  res.pick = await ev(`
    const D = window.__xfdoc;
    const items = [...D.querySelectorAll('.suggestion-container .suggestion-item')];
    const it = items.find((x) => /Other/.test(x.innerText) && !x.innerText.startsWith("+")) || items[0];
    const text = it ? it.innerText : null;
    if (it) { it.dispatchEvent(new D.defaultView.MouseEvent('mousedown', { bubbles: true })); it.click(); }
    await new Promise((r) => setTimeout(r, 900));
    const inp = D.querySelector('.modal-container input');
    return { picked: text, value: inp ? inp.value : null, modals: D.querySelectorAll('.modal-container').length, config: app.vault.getConfig('userIgnoreFilters') };`);
  await ev(`const D = window.__xfdoc; const m = D.querySelector('.modal-container'); if (m) [...m.querySelectorAll('button')].find((b) => b.textContent === 'Done').click(); await new Promise((r) => setTimeout(r, 700)); return true;`);
  res.afterPick = await ev(`return app.vault.getConfig('userIgnoreFilters');`);
  res.back = await ev(`
    const D = window.__xfdoc;
    D.querySelector('.setting-page-back-button').click();
    await new Promise((r) => setTimeout(r, 900));
    const rows = [...D.querySelectorAll('.setting-item')].filter((e) => (e.querySelector('.setting-item-name') || {}).textContent === 'Excluded files');
    return { rows: rows.map((r) => r.outerHTML) };`);
  await sleep(800);
  res.appJson = appJson();
  return res;
});

await step('s10', async () => {
  const res = {};
  // Recent files in the switcher's empty query, an excluded one among them.
  res.recent = await ev(`
    const leaf = app.workspace.getLeaf(false);
    for (const p of ['Keep.md', 'Archive/Old.md', 'Target.md']) { await leaf.openFile(app.vault.getAbstractFileByPath(p)); await new Promise((r) => setTimeout(r, 400)); }
    return app.workspace.getLastOpenFiles();`);
  const sw = await switcherRows('');
  res.switcherEmpty = sw;
  // Styles of a downranked row against a plain one.
  await ev(`app.commands.executeCommandById('switcher:open'); return true;`);
  await sleep(600);
  await c.typeText('o', 40);
  await sleep(700);
  res.styles = await ev(`
    const pick = (e) => { if (!e) return null; const cs = getComputedStyle(e); const t = e.querySelector('.suggestion-title') || e; const ts = getComputedStyle(t); return { cls: e.className, opacity: cs.opacity, color: cs.color, titleColor: ts.color, titleOpacity: ts.opacity, filter: cs.filter, fontStyle: ts.fontStyle }; };
    const items = [...document.querySelectorAll('.prompt .suggestion-item')];
    const plain = items.find((e) => !e.classList.contains('mod-downranked') && !e.classList.contains('is-selected'));
    const down = items.find((e) => e.classList.contains('mod-downranked') && !e.classList.contains('is-selected'));
    const vars = {}; for (const v of ['--text-normal', '--text-muted', '--text-faint']) vars[v] = getComputedStyle(document.body).getPropertyValue(v);
    return { plain: pick(plain), down: pick(down), vars };`);
  await c.pressKey('Escape');
  await sleep(300);
  // Outgoing links pane: unlinked mentions of an excluded note's name.
  res.outgoing = await ev(`
    const leaf = app.workspace.getLeaf(false);
    await leaf.openFile(app.vault.getAbstractFileByPath('Mentioner.md'));
    app.workspace.setActiveLeaf(leaf, { focus: true });
    app.commands.executeCommandById('outgoing-links:open');
    await new Promise((r) => setTimeout(r, 2000));
    const l = app.workspace.getLeavesOfType('outgoing-link')[0];
    return l ? l.view.containerEl.innerText.slice(0, 800) : null;`);
  // An excluded note open: its own backlinks pane and outgoing pane.
  res.excludedOpen = await ev(`
    const leaf = app.workspace.getLeaf(false);
    await leaf.openFile(app.vault.getAbstractFileByPath('Archive/Older.md'));
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await new Promise((r) => setTimeout(r, 2000));
    const b = app.workspace.getLeavesOfType('backlink')[0];
    return { backlink: b ? b.view.containerEl.innerText.slice(0, 600) : null };`);
  return res;
});

save();
c.close();
process.exit(0);
