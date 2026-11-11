// Slash commands: the core plugin that lists commands in an editor popup when
// `/` is typed. Real key events over CDP (Input.dispatchKeyEvent).
//
//   s00  context: version, the plugin's id, name, description, options, its
//        `core-plugins.json` key, whether it is on in a fresh vault, the
//        Core plugins row and whether it has a settings tab
//   s01  enabling it from Settings → Core plugins (the row's toggle): the bytes
//        of core-plugins.json before and after
//   s02  what triggers the popup: `/` at a line start, after a space, mid-word,
//        after punctuation, in a code block, in inline code, in frontmatter,
//        in a table cell, in a list item, a quote, a heading, a callout, in
//        Source mode and Live Preview; over a selection
//   s03  the popup's look: markup, rects, computed styles of the container,
//        a row, its title, its hotkey hints; a screenshot
//   s04  the rows: `/` alone (every row, scrolled), the instance's own list
//        against the palette's `listCommands()`, `/head`, `/tgl hd`, `/xyz`,
//        upper case, a query with a space
//   s05  ranking: pinned commands (command-palette's `pinned`) and commands
//        run recently through the palette; whether the slash popup records
//        recents of its own
//   s06  keys: ArrowDown/Up (wrap), Tab, Escape (and the text left), Home/End,
//        Backspace through the slash, a space right after `/`, typing past a
//        space, a second `/`
//   s07  accepting a row: the bytes before and after, caret, for several
//        commands (a heading toggle, bold, insert table, a callout, a list,
//        a modal-opening command, a non-editor command); Enter and a click;
//        what Ctrl+Z undoes afterwards
//   s08  reading view: does `/` do anything; and which rows are editor-only
//
// Screenshots go to <results>-shots/. ONLY_STEPS=<regex> narrows the run.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

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
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 240000) {
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
  console.error(`[slash] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Note.md'), 'Seed\n');
  fs.writeFileSync(vp('Other.md'), '# Other\n\nThe other note.\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  // The plugin's key is left out, so s00 sees its default.
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    outline: true, 'command-palette': true, 'file-recovery': true, 'page-preview': true, properties: true,
    templates: true,
  }, null, 2));
  fs.mkdirSync(vp('Templates'), { recursive: true });
  fs.writeFileSync(vp('Templates/T1.md'), 'Template body\n');
  fs.writeFileSync(vp('.obsidian/templates.json'), JSON.stringify({ folder: 'Templates' }, null, 2));
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

await c.evaluate(`
window.__s = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x * 10) / 10, Math.round(r.y * 10) / 10, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10]; },
  skeleton(e, d = 0, max = 7) {
    if (!e) return null;
    const tag = e.tagName.toLowerCase();
    if (['path', 'line', 'circle', 'rect', 'polyline', 'polygon'].includes(tag)) return null;
    const o = { tag, cls: e.getAttribute('class') || undefined };
    for (const a of ['aria-label', 'data-index', 'role', 'tabindex', 'style']) { const v = e.getAttribute(a); if (v !== null) o[a] = v; }
    if (!e.children.length) { const t = (e.textContent || '').trim(); if (t) o.text = t.slice(0, 120); }
    if (d < max && e.children.length) o.children = Array.from(e.children).slice(0, 6).map((c) => this.skeleton(c, d + 1, max)).filter(Boolean);
    return o;
  },
  leaf() {
    const leaves = []; app.workspace.iterateRootLeaves((l) => leaves.push(l));
    for (const l of leaves.slice(1)) l.detach();
    return leaves[0] || app.workspace.getLeaf(false);
  },
  async open(path, mode) {
    const leaf = this.leaf();
    if (!leaf.view.file || leaf.view.file.path !== path) await leaf.openFile(app.vault.getAbstractFileByPath(path));
    const vs = leaf.getViewState();
    if (mode === 'source') { vs.state.mode = 'source'; vs.state.source = true; }
    if (mode === 'live') { vs.state.mode = 'source'; vs.state.source = false; }
    if (mode === 'preview') { vs.state.mode = 'preview'; }
    await leaf.setViewState(vs);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(500);
    return { file: leaf.view.file && leaf.view.file.path, state: leaf.getViewState().state };
  },
  view() { return this.leaf().view; },
  ed() { return this.view().editor; },
  async seed(content, cursor, sel) {
    const view = this.view();
    const ed = this.ed();
    await app.vault.modify(view.file, content);
    await this.sleep(250);
    let seedOk = ed.getValue() === content;
    if (!seedOk) { ed.setValue(content); await this.sleep(100); seedOk = ed.getValue() === content; }
    ed.focus();
    if (sel) ed.setSelection(sel[0], sel[1]); else ed.setCursor(cursor || { line: 0, ch: 0 });
    await this.sleep(80);
    return { seedOk };
  },
  read() {
    const ed = this.ed();
    return { value: ed.getValue(), cursor: ed.getCursor(), from: ed.getCursor('from'), to: ed.getCursor('to') };
  },
  popup() {
    const cs = Array.from(document.querySelectorAll('.suggestion-container')).filter((e) => e.offsetParent || getComputedStyle(e).display !== 'none');
    if (!cs.length) return null;
    const box = cs[cs.length - 1];
    const items = Array.from(box.querySelectorAll('.suggestion-item'));
    return {
      cls: this.cls(box), rect: this.rect(box), count: items.length,
      selected: items.findIndex((i) => i.classList.contains('is-selected')),
      rows: items.map((i) => ({ text: (() => { const t = i.querySelector('.suggestion-title') || i; const pre = t.querySelector('.suggestion-prefix'); const spans = Array.from(t.children).filter((e) => e !== pre); return (pre ? pre.textContent + ': ' : '') + spans.map((e) => e.textContent).join(''); })(), aux: (i.querySelector('.suggestion-aux') || {}).innerText || null, cls: this.cls(i) })),
      empty: (box.querySelector('.suggestion-empty') || {}).innerText || null,
      instructions: Array.from(box.querySelectorAll('.prompt-instruction')).map((e) => e.innerText.trim()),
    };
  },
  // The slash suggest instance among the editor suggests.
  suggest() {
    const s = app.workspace.editorSuggest && app.workspace.editorSuggest.suggests;
    if (!s) return null;
    return s.find((x) => /slash/i.test(x.constructor && x.constructor.name || '') || (x.plugin && /slash/i.test(x.plugin.id || ''))) || null;
  },
};
return true;`);
const ev = (s) => c.evaluate(s);
const S = 'const S = window.__s;';
let shotN = 0;
async function shot(name) {
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
const popup = () => ev(`${S} return S.popup();`);
const read = () => ev(`${S} return S.read();`);
const esc = async () => { await c.pressKey('Escape'); await sleep(250); };
const type = async (s) => { await c.typeText(s, 60); await sleep(350); };
const seed = (content, cursor, sel) => ev(`${S} return await S.seed(${JSON.stringify(content)}, ${JSON.stringify(cursor || null)}, ${JSON.stringify(sel || null)});`);
const mode = (m) => ev(`${S} return await S.open('Note.md', '${m}');`);

let PID = null;

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio, apiVersion: (window.require && (() => { try { return require('obsidian').apiVersion; } catch { return null; } })()) || null };`);
  const plugins = await ev(`return Object.entries(app.internalPlugins.plugins).map(([id, p]) => ({ id, name: p.instance && p.instance.name, enabled: p.enabled }));`);
  const slash = plugins.filter((p) => /slash/i.test(p.id) || /slash/i.test(p.name || ''));
  PID = slash.length ? slash[0].id : null;
  const detail = PID ? await ev(`const p = app.internalPlugins.getPluginById(${JSON.stringify(PID)}); const i = p.instance;
    return { enabled: p.enabled, name: i.name, description: i.description, defaultOn: i.defaultOn, options: i.options ? JSON.parse(JSON.stringify(i.options)) : null,
      instanceKeys: Object.keys(i), protoKeys: Object.getOwnPropertyNames(Object.getPrototypeOf(i)), hasSettingTab: !!(p.instance && (i.settingTab || i.addSettingTab)),
      commands: Object.keys(app.commands.commands).filter((k) => k.startsWith(${JSON.stringify(PID)} + ':')) };`) : null;
  const corePluginsJson = bytes('.obsidian/core-plugins.json');
  return { version: out.version, slash, pid: PID, detail, corePluginsJson, allIds: plugins.map((p) => p.id) };
});

await step('s01-enable', async () => {
  if (!PID) PID = await ev(`return Object.keys(app.internalPlugins.plugins).find((k) => /slash/i.test(k));`);
  const before = bytes('.obsidian/core-plugins.json');
  await ev(`app.setting.open(); app.setting.openTabById('plugins'); return true;`); await sleep(1200);
  const row = await ev(`${S} const el = app.setting.activeTab.containerEl;
    const rows = Array.from(el.querySelectorAll('.setting-item'));
    const r = rows.find((s) => /slash/i.test((s.querySelector('.setting-item-name') || {}).textContent || ''));
    if (!r) return { rows: rows.map((s) => (s.querySelector('.setting-item-name') || {}).textContent) };
    const prev = r.previousElementSibling, next = r.nextElementSibling;
    return { name: r.querySelector('.setting-item-name').textContent, desc: (r.querySelector('.setting-item-description') || {}).textContent, html: r.outerHTML.slice(0, 2000),
      neighbours: [(prev && prev.querySelector('.setting-item-name') || {}).textContent, (next && next.querySelector('.setting-item-name') || {}).textContent],
      toggleOn: !!r.querySelector('.checkbox-container.is-enabled'), buttons: Array.from(r.querySelectorAll('.clickable-icon, button')).map((b) => b.getAttribute('aria-label')) };`);
  const s1 = await shot('core-plugins-row-off');
  // Click the row's toggle the way a person does.
  const clicked = await ev(`const el = app.setting.activeTab.containerEl;
    const r = Array.from(el.querySelectorAll('.setting-item')).find((s) => /slash/i.test((s.querySelector('.setting-item-name') || {}).textContent || ''));
    const t = r && r.querySelector('.checkbox-container'); if (!t) return false; t.click(); return true;`);
  await sleep(1500);
  const rowAfter = await ev(`const el = app.setting.activeTab.containerEl;
    const r = Array.from(el.querySelectorAll('.setting-item')).find((s) => /slash/i.test((s.querySelector('.setting-item-name') || {}).textContent || ''));
    return r ? { toggleOn: !!r.querySelector('.checkbox-container.is-enabled'), buttons: Array.from(r.querySelectorAll('.clickable-icon, button')).map((b) => b.getAttribute('aria-label')), html: r.outerHTML.slice(0, 2000) } : null;`);
  const s2 = await shot('core-plugins-row-on');
  // Settings side tabs: does a "Slash commands" tab appear under Core plugins?
  const tabs = await ev(`return Array.from(document.querySelectorAll('.vertical-tab-nav-item')).map((e) => e.textContent.trim());`);
  const tabIds = await ev(`return (app.setting.pluginTabs || []).map((t) => ({ id: t.id, name: t.name }));`);
  await ev(`app.setting.close(); return true;`); await sleep(1500);
  const after = bytes('.obsidian/core-plugins.json');
  const enabled = await ev(`return app.internalPlugins.getPluginById(${JSON.stringify(PID)}).enabled;`);
  const options = await ev(`const i = app.internalPlugins.getPluginById(${JSON.stringify(PID)}).instance; return i.options ? JSON.parse(JSON.stringify(i.options)) : null;`);
  const pluginJson = bytes(`.obsidian/${PID}.json`);
  const files = fs.readdirSync(vp('.obsidian'));
  return { before, row, clicked, rowAfter, after, enabled, options, pluginJson, files, tabs, tabIds, shots: [s1, s2] };
});

// The trigger matrix: content, caret, the text typed, and whether a popup shows.
const TRIGGERS = [
  ['line-start', '', { line: 0, ch: 0 }],
  ['line-start-text-after', 'abc', { line: 0, ch: 0 }],
  ['after-space', 'hello ', { line: 0, ch: 6 }],
  ['mid-word', 'hello', { line: 0, ch: 5 }],
  ['inside-word', 'hello', { line: 0, ch: 2 }],
  ['after-paren', 'call(', { line: 0, ch: 5 }],
  ['after-tab', '\t', { line: 0, ch: 1 }],
  ['after-indent', '    ', { line: 0, ch: 4 }],
  ['after-slash', 'a /', { line: 0, ch: 3 }],
  ['list-item', '- ', { line: 0, ch: 2 }],
  ['task-item', '- [ ] ', { line: 0, ch: 6 }],
  ['ordered', '1. ', { line: 0, ch: 3 }],
  ['quote', '> ', { line: 0, ch: 2 }],
  ['heading', '# ', { line: 0, ch: 2 }],
  ['callout', '> [!note]\n> ', { line: 1, ch: 2 }],
  ['fenced-code', '```\n\n```\n', { line: 1, ch: 0 }],
  ['fenced-code-after-space', '```js\nx \n```\n', { line: 1, ch: 2 }],
  ['inline-code', 'a `b ` c', { line: 0, ch: 5 }],
  ['frontmatter', '---\nkey: \n---\nBody\n', { line: 1, ch: 5 }],
  ['frontmatter-line-start', '---\n\n---\nBody\n', { line: 1, ch: 0 }],
  ['math-block', '$$\n\n$$\n', { line: 1, ch: 0 }],
  ['table-cell', '| a | b |\n| - | - |\n| 1 |  |\n', { line: 2, ch: 7 }],
  ['url', 'see http:', { line: 0, ch: 9 }],
  ['after-wikilink', '[[Other]] ', { line: 0, ch: 10 }],
  ['inside-wikilink', '[[Oth]]', { line: 0, ch: 5 }],
  ['html-comment', '<!-- x  -->', { line: 0, ch: 6 }],
  ['obsidian-comment', '%% x  %%', { line: 0, ch: 5 }],
];

await step('s02-triggers', async () => {
  const res = {};
  for (const m of ['live', 'source']) {
    await mode(m);
    res[m] = {};
    for (const [name, content, cursor] of TRIGGERS) {
      const sd = await seed(content, cursor);
      await type('/');
      const p = await popup();
      const r = await read();
      res[m][name] = { content, cursor, seedOk: sd.seedOk, popup: !!p, count: p ? p.count : 0, first: p && p.rows[0] ? p.rows[0].text : null, value: r.value };
      if (name === 'table-cell' && m === 'live') res[m][name].shot = await shot('table-cell-live');
      await esc();
    }
    // A selection, then `/`: the selection is replaced, and does the popup open?
    await seed('one two three', null, [{ line: 0, ch: 4 }, { line: 0, ch: 7 }]);
    await type('/');
    res[m].overSelection = { popup: !!(await popup()), read: await read() };
    await esc();
    // `/` typed right after a `/`-popup was dismissed with Escape, then more text.
    await seed('', { line: 0, ch: 0 });
    await type('/'); await esc(); await type('he');
    res[m].afterEscapeTyping = { popup: !!(await popup()), read: await read() };
    await esc();
  }
  return res;
});

await step('s03-look', async () => {
  await mode('live');
  await seed('First line\n\n', { line: 2, ch: 0 });
  await type('/');
  const look = await ev(`${S}
    const box = Array.from(document.querySelectorAll('.suggestion-container')).pop(); if (!box) return null;
    const st = (e, keys) => { if (!e) return null; const cs = getComputedStyle(e); const o = {}; for (const k of keys) o[k] = cs[k]; return o; };
    const item = box.querySelector('.suggestion-item');
    const sel = box.querySelector('.suggestion-item.is-selected');
    const withAux = Array.from(box.querySelectorAll('.suggestion-item')).find((i) => i.querySelector('.suggestion-aux kbd, .suggestion-hotkey, kbd'));
    const caret = (() => { const s = window.getSelection(); if (!s.rangeCount) return null; const r = s.getRangeAt(0).getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })();
    const cm = S.view().editor.cm; const pos = cm.state.selection.main.head; const cc = cm.coordsAtPos(pos);
    return {
      html: box.outerHTML.slice(0, 4000),
      skeleton: S.skeleton(box, 0, 6),
      rect: S.rect(box), caretCoords: cc ? [cc.left, cc.top, cc.right, cc.bottom] : null, caret,
      boxStyle: st(box, ['width', 'maxWidth', 'minWidth', 'height', 'maxHeight', 'padding', 'borderRadius', 'border', 'boxShadow', 'backgroundColor', 'fontSize', 'overflow', 'position', 'zIndex']),
      suggestionStyle: st(box.querySelector('.suggestion'), ['padding', 'maxHeight', 'overflowY']),
      itemStyle: st(item, ['height', 'padding', 'margin', 'borderRadius', 'fontSize', 'lineHeight', 'color', 'backgroundColor', 'display', 'justifyContent', 'alignItems']),
      itemRect: S.rect(item),
      selectedStyle: st(sel, ['backgroundColor', 'color']),
      auxHtml: withAux ? withAux.outerHTML.slice(0, 1500) : null,
      kbdStyle: withAux ? st(withAux.querySelector('kbd') || withAux.querySelector('.suggestion-hotkey'), ['fontSize', 'padding', 'borderRadius', 'backgroundColor', 'color', 'margin', 'fontFamily', 'border']) : null,
      rows: Array.from(box.querySelectorAll('.suggestion-item')).length,
      scroll: (() => { const s = box.querySelector('.suggestion'); return s ? { scrollHeight: s.scrollHeight, clientHeight: s.clientHeight } : null; })(),
      win: [innerWidth, innerHeight],
    };`);
  const s1 = await shot('popup-slash');
  await type('ta');
  const s2 = await shot('popup-ta');
  const p = await popup();
  await esc();
  // Light theme too.
  await ev(`app.changeTheme && app.changeTheme('moonstone'); return true;`); await sleep(600);
  await seed('First line\n\n', { line: 2, ch: 0 });
  await type('/');
  const s3 = await shot('popup-slash-light');
  await esc();
  await ev(`app.changeTheme && app.changeTheme('obsidian'); return true;`); await sleep(600);
  // A caret near the bottom of the pane: does the popup open above?
  const many = Array.from({ length: 60 }, (_, i) => 'line ' + i).join('\n') + '\n';
  await seed(many, { line: 60, ch: 0 });
  await ev(`${S} S.ed().scrollIntoView({ from: { line: 60, ch: 0 }, to: { line: 60, ch: 0 } }, true); return true;`); await sleep(300);
  await type('/');
  const bottom = await ev(`${S} const box = Array.from(document.querySelectorAll('.suggestion-container')).pop(); const cm = S.view().editor.cm; const cc = cm.coordsAtPos(cm.state.selection.main.head); return { rect: box ? S.rect(box) : null, caret: cc ? [cc.left, cc.top, cc.bottom] : null };`);
  const s4 = await shot('popup-bottom');
  await esc();
  return { look, ta: p, shots: [s1, s2, s3, s4], bottom };
});

await step('s04-rows', async () => {
  await mode('live');
  const res = {};
  await seed('', { line: 0, ch: 0 });
  await type('/');
  res.slash = await popup();
  // Every row: what the suggest itself would list for the empty query, against the palette's list.
  res.palette = await ev(`return app.commands.listCommands().map((c) => ({ id: c.id, name: c.name }));`);
  res.allCommandsCount = await ev(`return Object.keys(app.commands.commands).length;`);
  res.editorCommands = await ev(`return Object.values(app.commands.commands).filter((c) => c.editorCallback || c.editorCheckCallback).map((c) => c.id);`);
  res.suggest = await ev(`${S} const s = S.suggest(); if (!s) return { found: false, names: (app.workspace.editorSuggest.suggests || []).map((x) => x.constructor.name) };
    return { found: true, ctor: s.constructor.name, keys: Object.keys(s), limit: s.limit, ctx: s.context ? { query: s.context.query, start: s.context.start, end: s.context.end } : null };`);
  // The DOM rows of the empty query, all of them (scroll the list).
  res.domAll = await ev(`${S} const box = Array.from(document.querySelectorAll('.suggestion-container')).pop(); if (!box) return null;
    return Array.from(box.querySelectorAll('.suggestion-item')).map((i) => ({ title: (i.querySelector('.suggestion-title') || i).innerText.trim(), aux: (i.querySelector('.suggestion-aux') || {}).innerText || null }));`);
  await esc();
  for (const q of ['head', 'tgl hd', 'xyz', 'HEAD', 'insert table', 'bold', 'tab', 'cal', 'sethead', 'h1', 'toggle', 'e']) {
    await seed('', { line: 0, ch: 0 });
    await type('/' + q);
    res['q:' + q] = { popup: await popup(), read: await read() };
    if (q === 'head') res['q:' + q].shot = await shot('popup-head');
    await esc();
  }
  return res;
});

await step('s05-ranking', async () => {
  await mode('live');
  const res = {};
  res.paletteOptions = await ev(`const p = app.internalPlugins.getPluginById('command-palette'); return p && p.instance.options ? JSON.parse(JSON.stringify(p.instance.options)) : null;`);
  // Pin two commands in the palette.
  await ev(`const p = app.internalPlugins.getPluginById('command-palette'); p.instance.options.pinned = ['editor:insert-table', 'editor:toggle-bold']; p.instance.saveSettings && p.instance.saveSettings(p); return true;`);
  await sleep(500);
  await seed('', { line: 0, ch: 0 });
  await type('/');
  res.pinned = await popup();
  res.pinnedShot = await shot('popup-pinned');
  await esc();
  await seed('', { line: 0, ch: 0 });
  await type('/bold');
  res.pinnedQuery = await popup();
  await esc();
  // Recent: run commands through the palette (typing the name, Enter), then look again.
  for (const name of ['Toggle highlight', 'Toggle italics']) {
    await seed('x', { line: 0, ch: 1 });
    await ev(`app.commands.executeCommandById('command-palette:open'); return true;`); await sleep(600);
    await c.typeText(name, 40); await sleep(500);
    await c.pressKey('Enter'); await sleep(600);
  }
  res.paletteOptionsAfter = await ev(`const p = app.internalPlugins.getPluginById('command-palette'); return p && p.instance.recentCommands ? p.instance.recentCommands.slice() : (p && p.instance.options ? JSON.parse(JSON.stringify(p.instance.options)) : null);`);
  res.paletteInstanceKeys = await ev(`const p = app.internalPlugins.getPluginById('command-palette'); return Object.keys(p.instance);`);
  await seed('', { line: 0, ch: 0 });
  await type('/');
  res.afterRecent = await popup();
  await esc();
  // Unpin, and run a command through the slash popup: does it become recent for the palette or the popup?
  await ev(`const p = app.internalPlugins.getPluginById('command-palette'); p.instance.options.pinned = []; p.instance.saveSettings && p.instance.saveSettings(p); return true;`);
  await seed('', { line: 0, ch: 0 });
  await type('/');
  res.unpinned = await popup();
  await esc();
  await seed('word', { line: 0, ch: 4 });
  await type(' /strikethrough');
  res.beforeAccept = await popup();
  await c.pressKey('Enter'); await sleep(600);
  res.acceptedRead = await read();
  await seed('', { line: 0, ch: 0 });
  await type('/');
  res.afterSlashAccept = await popup();
  await esc();
  await ev(`app.commands.executeCommandById('command-palette:open'); return true;`); await sleep(700);
  res.paletteRowsAfterSlash = await ev(`return Array.from(document.querySelectorAll('.prompt .suggestion-item')).slice(0, 8).map((i) => i.innerText.trim());`);
  await esc();
  res.paletteRecentAfterSlash = await ev(`const p = app.internalPlugins.getPluginById('command-palette'); return p.instance.recentCommands ? p.instance.recentCommands.slice() : null;`);
  res.corePluginsJson = bytes('.obsidian/command-palette.json');
  return res;
});

await step('s06-keys', async () => {
  await mode('live');
  const res = {};
  const sel = async () => { const p = await popup(); return p ? { selected: p.selected, title: p.rows[p.selected] && p.rows[p.selected].text, count: p.count } : null; };
  await seed('', { line: 0, ch: 0 });
  await type('/');
  res.initial = await sel();
  await c.pressKey('ArrowDown'); await sleep(150); res.down1 = await sel();
  await c.pressKey('ArrowDown'); await sleep(150); res.down2 = await sel();
  await c.pressKey('ArrowUp'); await sleep(150); res.up1 = await sel();
  await c.pressKey('ArrowUp'); await sleep(150); res.up2 = await sel();
  await c.pressKey('ArrowUp'); await sleep(150); res.upWrap = await sel();
  await c.pressKey('ArrowDown'); await sleep(150); res.downWrap = await sel();
  await c.pressKey('PageDown'); await sleep(150); res.pageDown = await sel();
  await c.pressKey('Home'); await sleep(150); res.home = { sel: await sel(), read: await read() };
  await esc();
  // Escape: popup gone, the text stays.
  await seed('', { line: 0, ch: 0 });
  await type('/tab');
  await esc();
  res.escape = { popup: !!(await popup()), read: await read() };
  // After Escape, typing more: reopens?
  await type('l');
  res.escapeThenType = { popup: await popup(), read: await read() };
  await esc();
  // Tab
  await seed('', { line: 0, ch: 0 });
  await type('/bold');
  await c.pressKey('Tab'); await sleep(400);
  res.tab = { popup: !!(await popup()), read: await read() };
  await esc();
  // Backspace through the slash
  await seed('', { line: 0, ch: 0 });
  await type('/b');
  await c.pressKey('Backspace'); await sleep(250); res.bs1 = { popup: !!(await popup()), read: await read() };
  await c.pressKey('Backspace'); await sleep(250); res.bs2 = { popup: !!(await popup()), read: await read() };
  await esc();
  // Space right after the slash
  await seed('', { line: 0, ch: 0 });
  await type('/ ');
  res.slashSpace = { popup: await popup(), read: await read() };
  await esc();
  // Typing past a space: `/insert table`, `/insert   x`, `/a b c d`
  for (const q of ['insert table', 'toggle bold ', 'a b c d', 'xyz abc']) {
    await seed('', { line: 0, ch: 0 });
    await type('/' + q);
    const p = await popup();
    res['past:' + q] = { popup: !!p, count: p ? p.count : 0, first: p && p.rows[0] ? p.rows[0].text : null, empty: p ? p.empty : null, read: await read() };
    await esc();
  }
  // A second slash
  await seed('', { line: 0, ch: 0 });
  await type('/a/');
  res.secondSlash = { popup: await popup(), read: await read() };
  await esc();
  // ArrowLeft/Right move the caret out of the query?
  await seed('', { line: 0, ch: 0 });
  await type('/bol');
  await c.pressKey('ArrowLeft'); await sleep(250); res.left1 = { popup: !!(await popup()), p: await popup(), read: await read() };
  await c.pressKey('ArrowLeft'); await sleep(250); await c.pressKey('ArrowLeft'); await sleep(250); await c.pressKey('ArrowLeft'); await sleep(250);
  res.leftPastSlash = { popup: !!(await popup()), read: await read() };
  await esc();
  // Mod+Enter, Shift+Enter
  await seed('', { line: 0, ch: 0 });
  await type('/bold');
  await c.pressKey('Enter', { shift: true }); await sleep(400);
  res.shiftEnter = { popup: !!(await popup()), read: await read() };
  await esc();
  // Click outside closes?
  await seed('abc\n\n\n', { line: 1, ch: 0 });
  await type('/');
  const r = await ev(`${S} const cm = S.view().editor.cm; const cc = cm.coordsAtPos(0); return cc ? [cc.left + 5, cc.top + 5] : null;`);
  if (r) { await c.clickAt(r[0], r[1]); await sleep(400); }
  res.clickAway = { popup: !!(await popup()), read: await read() };
  return res;
});

// Accept a row whose title is exactly `name` after typing `/` + query, by arrow keys and Enter.
async function accept(content, cursor, query, name, opts = {}) {
  const sd = await seed(content, cursor, opts.sel);
  await type('/' + query);
  if (opts.pause) await sleep(opts.pause);
  const p = await popup();
  const idx = p ? p.rows.findIndex((r) => r.text === name) : -1;
  const o = { in: content, caret: cursor, query, name, seedOk: sd.seedOk, rows: p ? p.rows.slice(0, 8).map((r) => r.text) : null, idx };
  if (idx < 0) { await esc(); o.error = 'row not found'; return o; }
  o.selectedBefore = p.selected;
  if (opts.click) {
    const r = await ev(`const it = Array.from(document.querySelectorAll('.suggestion-container')).pop().querySelectorAll('.suggestion-item')[${idx}]; const b = it.getBoundingClientRect(); return [b.x + b.width / 2, b.y + b.height / 2];`);
    await c.clickAt(r[0], r[1]);
  } else {
    for (let i = 0; i < idx - (p.selected < 0 ? -1 : p.selected); i++) { await c.pressKey('ArrowDown'); await sleep(80); }
    const q = await popup();
    o.selectedTitle = q && q.rows[q.selected] ? q.rows[q.selected].text : null;
    await c.pressKey('Enter');
  }
  await sleep(opts.wait || 700);
  const r = await read();
  o.out = r.value; o.cursor = r.cursor; o.from = r.from; o.to = r.to;
  o.popupAfter = !!(await popup());
  o.modal = await ev(`${S} const m = document.querySelector('.modal-container, .modal.mod-settings'); return m ? { cls: S.cls(m), title: (m.querySelector('.modal-title') || {}).textContent || null, prompt: !!m.querySelector('.prompt'), input: (m.querySelector('input') || {}).value ?? null, placeholder: (m.querySelector('input') || {}).placeholder || null } : null;`);
  if (o.modal && opts.shot) o.shot = await shot(opts.shot);
  if (o.modal) {
    await esc(); await sleep(300);
    const r2 = await read(); o.afterModalClose = r2.value; o.cursorAfterModalClose = r2.cursor;
  }
  if (opts.undo) {
    await ev(`${S} S.ed().focus(); return true;`);
    await c.pressKey('z', { ctrl: true }); await sleep(300);
    o.undo1 = (await read()).value;
    await c.pressKey('z', { ctrl: true }); await sleep(300);
    o.undo2 = (await read()).value;
    await c.pressKey('z', { ctrl: true }); await sleep(300);
    o.undo3 = (await read()).value;
  }
  o.leaves = await ev(`const o = []; app.workspace.iterateAllLeaves((l) => o.push(l.view.getViewType())); return o;`);
  o.tabHeaders = await ev(`return document.querySelectorAll('.workspace-tab-header').length;`);
  o.settingsUp = await ev(`return !!document.querySelector('.modal.mod-settings') || !!(app.setting && app.setting.containerEl && app.setting.containerEl.isConnected);`);
  await ev(`await app.commands.executeCommandById('editor:save-file'); return true;`); await sleep(300);
  o.fileBytes = bytes('Note.md');
  return o;
}

await step('s07-accept', async () => {
  const res = {};
  for (const m of ['live', 'source']) {
    await mode(m);
    res[m] = [];
    const cases = [
      ['Hello', { line: 0, ch: 5 }, 'heading', null, { pick: /heading/i }],
      ['Hello', { line: 0, ch: 0 }, 'bold', 'Toggle bold'],
      ['Hello ', { line: 0, ch: 6 }, 'bold', 'Toggle bold', { undo: true }],
      ['Hello world', { line: 0, ch: 6 }, 'bold', 'Toggle bold'],
      ['', { line: 0, ch: 0 }, 'insert table', 'Insert table', { undo: true }],
      ['Intro\n', { line: 1, ch: 0 }, 'insert table', 'Insert table'],
      ['', { line: 0, ch: 0 }, 'callout', 'Insert callout'],
      ['Item', { line: 0, ch: 4 }, 'bullet', 'Toggle bullet list'],
      ['Some ', { line: 0, ch: 5 }, 'code block', 'Insert code block'],
      ['', { line: 0, ch: 0 }, 'horizontal', 'Insert horizontal rule'],
      ['Text ', { line: 0, ch: 5 }, 'link', 'Insert Markdown link'],
      ['Text ', { line: 0, ch: 5 }, 'command palette', 'Command palette: Open command palette', { shot: m === 'live' ? 'modal-palette' : null }],
      ['Text ', { line: 0, ch: 5 }, 'template', 'Templates: Insert template', { shot: m === 'live' ? 'modal-template' : null }],
      ['Text ', { line: 0, ch: 5 }, 'settings', 'Open settings'],
      ['Text ', { line: 0, ch: 5 }, 'new tab', 'Open new tab'],
      ['A\nB', { line: 0, ch: 1 }, 'swap down', 'Move line down'],
      ['Text ', { line: 0, ch: 5 }, 'date', 'Templates: Insert current date'],
      ['Tail text', { line: 0, ch: 4 }, 'bold', 'Toggle bold'],
    ];
    for (const [content, cursor, query, name, opts = {}] of cases) {
      let nm = name;
      if (!nm && opts.pick) {
        await seed(content, cursor); await type('/' + query); const p = await popup(); await esc();
        nm = p && p.rows.find((r) => opts.pick.test(r.text)) ? p.rows.find((r) => opts.pick.test(r.text)).text : null;
        res[m].push({ headingRows: p ? p.rows.map((r) => r.text) : null });
        if (!nm) continue;
      }
      res[m].push(await accept(content, cursor, query, nm, opts));
      // Put the single markdown leaf back if a command opened another.
      await ev(`app.setting && app.setting.close && app.setting.close(); return true;`);
      await mode(m);
    }
    // A click instead of Enter.
    res[m].push({ click: await accept('Hello ', { line: 0, ch: 6 }, 'italic', 'Toggle italics', { click: true }) });
  }
  return res;
});

await step('s08-preview-and-editor-only', async () => {
  const res = {};
  await mode('preview');
  await ev(`${S} S.view().containerEl.focus(); return true;`);
  await type('/');
  res.preview = { popup: !!(await popup()), modal: await ev(`return !!document.querySelector('.modal-container');`), file: bytes('Note.md') };
  await esc();
  // Rows offered in the editor vs. commands that are not editor commands.
  await mode('live');
  await seed('', { line: 0, ch: 0 });
  await type('/');
  const rows = (await popup() || { rows: [] }).rows.map((r) => r.text);
  const cmds = await ev(`return Object.values(app.commands.commands).map((c) => ({ id: c.id, name: c.name, editor: !!(c.editorCallback || c.editorCheckCallback), check: !!c.checkCallback }));`);
  const listed = await ev(`return app.commands.listCommands().map((c) => c.name);`);
  res.rowCount = rows.length;
  res.listedCount = listed.length;
  res.rowsNotListed = rows.filter((r) => !listed.includes(r));
  res.listedNotRows = listed.filter((r) => !rows.includes(r)).slice(0, 80);
  res.nonEditorRows = rows.filter((r) => { const c = cmds.find((x) => x.name === r); return c && !c.editor; }).slice(0, 80);
  res.editorRows = rows.filter((r) => { const c = cmds.find((x) => x.name === r); return c && c.editor; }).length;
  res.rows = rows;
  await esc();
  return res;
});

// The accepts s07 missed (a space in the query closes the popup, so each
// query here is one word), the undo steps with a pause before the accept so
// CodeMirror cannot join the typing to it, and the modal-opening commands.
await step('s09-accept-more', async () => {
  const res = {};
  for (const m of ['live', 'source']) {
    await mode(m);
    res[m] = [];
    const cases = [
      ['', { line: 0, ch: 0 }, 'table', 'Insert table', { undo: true, pause: 1200 }],
      ['Intro\n', { line: 1, ch: 0 }, 'table', 'Insert table'],
      ['Intro ', { line: 0, ch: 6 }, 'table', 'Insert table'],
      ['Item', { line: 0, ch: 0 }, 'bullet', 'Toggle bullet list'],
      ['Some ', { line: 0, ch: 5 }, 'codeblock', 'Insert code block'],
      ['Hello', { line: 0, ch: 0 }, 'heading', 'Toggle heading'],
      ['Hello', { line: 0, ch: 0 }, 'h2', 'Set as heading 2'],
      ['## Hello', { line: 0, ch: 3 }, 'h1', 'Set as heading 1'],
      ['A\nB', { line: 0, ch: 0 }, 'linedown', 'Move line down'],
      ['one\ntwo', { line: 1, ch: 0 }, 'quote', 'Toggle blockquote'],
      ['- item ', { line: 0, ch: 7 }, 'checkbox', 'Toggle checkbox status'],
      ['Hello ', { line: 0, ch: 6 }, 'bold', 'Toggle bold', { undo: true, pause: 1200 }],
      ['Text ', { line: 0, ch: 5 }, 'palette', 'Command palette: Open command palette', { shot: m === 'live' ? 'modal-palette' : null }],
      ['Text ', { line: 0, ch: 5 }, 'template', 'Templates: Insert template', { shot: m === 'live' ? 'modal-template' : null }],
      ['Text ', { line: 0, ch: 5 }, 'date', 'Templates: Insert current date'],
      ['Text ', { line: 0, ch: 5 }, 'settings', 'Open settings', { wait: 1500 }],
      ['Text ', { line: 0, ch: 5 }, 'newtab', 'New tab'],
      ['Text ', { line: 0, ch: 5 }, 'switcher', 'Quick switcher: Open quick switcher'],
      ['x', { line: 0, ch: 0 }, 'math', 'Insert math block'],
      ['x', { line: 0, ch: 1 }, 'footnote', 'Insert footnote'],
    ];
    for (const [content, cursor, query, name, opts = {}] of cases) {
      res[m].push(await accept(content, cursor, query, name, opts));
      await ev(`app.setting && app.setting.close && app.setting.close(); document.querySelectorAll('.modal-container').forEach((e) => e.remove()); return true;`);
      await mode(m);
    }
    res[m].push({ click: await accept('Hello ', { line: 0, ch: 6 }, 'italic', 'Toggle italic', { click: true }) });
  }
  return res;
});

// What s07/s09 left ambiguous: which row Enter took (read back before the
// key), the commands that open no modal (New tab, Open settings), a caret
// inside the query, the page keys and End, and New tab from the palette.
await step('s10-leftovers', async () => {
  await mode('live');
  const res = {};
  res.accepts = [];
  for (const [content, cursor, query, name, opts = {}] of [
    ['Hello', { line: 0, ch: 0 }, 'heading', 'Toggle heading'],
    ['# Hello', { line: 0, ch: 0 }, 'heading', 'Toggle heading'],
    ['Item', { line: 0, ch: 0 }, 'bullet', 'Toggle bullet list'],
    ['Item', { line: 0, ch: 0 }, 'cycle', 'Cycle bullet/checkbox'],
    ['x ', { line: 0, ch: 2 }, 'footnote', 'Insert footnote'],
    ['Text ', { line: 0, ch: 5 }, 'newtab', 'New tab', { wait: 1500 }],
    ['Text ', { line: 0, ch: 5 }, 'settings', 'Open settings', { wait: 2000 }],
  ]) {
    const before = await ev(`return document.querySelectorAll('.workspace-tab-header').length;`);
    const o = await accept(content, cursor, query, name, opts);
    o.tabHeadersBefore = before;
    if (o.settingsUp) o.shot = await shot('settings-from-slash');
    res.accepts.push(o);
    await ev(`app.setting && app.setting.close && app.setting.close(); return true;`); await sleep(300);
    await mode('live');
  }
  // The caret moved back inside the query, then Enter: what is removed.
  await seed('Hello ', { line: 0, ch: 6 });
  await type('/bold');
  await c.pressKey('ArrowLeft'); await sleep(200); await c.pressKey('ArrowLeft'); await sleep(200);
  res.insideQuery = { popup: await popup() };
  await c.pressKey('Enter'); await sleep(600);
  res.insideQuery.after = await read();
  // Page keys and End.
  await seed('', { line: 0, ch: 0 });
  await type('/');
  const sel = async () => { const p = await popup(); return p ? [p.selected, p.rows[p.selected] && p.rows[p.selected].text] : null; };
  res.page = { start: await sel() };
  await c.pressKey('PageDown'); await sleep(150); res.page.pd1 = await sel();
  await c.pressKey('PageDown'); await sleep(150); res.page.pd2 = await sel();
  await c.pressKey('PageUp'); await sleep(150); res.page.pu1 = await sel();
  await c.pressKey('End'); await sleep(150); res.page.end = { sel: await sel(), read: await read() };
  await c.pressKey('Home'); await sleep(150); res.page.home = { sel: await sel(), read: await read() };
  await esc();
  // New tab through the palette, for comparison.
  await seed('Text ', { line: 0, ch: 5 });
  const tb = await ev(`return document.querySelectorAll('.workspace-tab-header').length;`);
  await ev(`app.commands.executeCommandById('command-palette:open'); return true;`); await sleep(600);
  await c.typeText('New tab', 40); await sleep(500);
  res.paletteNewTabRow = await ev(`const s = document.querySelector('.prompt .suggestion-item.is-selected'); return s ? s.innerText : null;`);
  await c.pressKey('Enter'); await sleep(1200);
  res.paletteNewTab = { before: tb, after: await ev(`return document.querySelectorAll('.workspace-tab-header').length;`), leaves: await ev(`const o = []; app.workspace.iterateAllLeaves((l) => o.push(l.view.getViewType())); return o;`) };
  return res;
});

c.close();
process.exit(0);
