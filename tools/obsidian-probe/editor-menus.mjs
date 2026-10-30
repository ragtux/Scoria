// Three observations the backlog's open entries wait on: the editor's
// right-click menu (source mode, Live Preview, reading view; on plain text,
// on a selection and on a wikilink — and whether the click moves the caret),
// the view header's Bookmark and "More options" actions, and every core
// plugin's options tab: each row's name, description and control, with each
// toggle flipped and flipped back so the key it writes and the file it lives
// in are read off disk, and each dropdown walked through its options and
// restored. Text fields are read, not typed into.
//
// Settings rows are reached through `app.setting.activeTab.containerEl`
// (frame-setting-key.mjs, plugin-options.mjs). Menu rows come from the
// `.menu` DOM the way ui-rules.mjs s14 reads them: title, lucide class,
// section, disabled, submenu arrow.
//
// Phases: fixtures | run   (node editor-menus.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT, RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const vp = (rel) => path.join(VAULT, rel);

// Every core plugin id Scoria lists (scoria-formats CORE_PLUGINS), all on,
// so each one's tab exists. Sync and Publish have tabs too; their rows are
// worth a look even though Scoria will never run them.
const CORE = ['audio-recorder', 'backlink', 'bases', 'bookmarks', 'canvas', 'command-palette',
  'daily-notes', 'file-recovery', 'file-explorer', 'footnotes', 'graph', 'note-composer',
  'outgoing-link', 'outline', 'page-preview', 'properties', 'publish', 'switcher', 'random-note',
  'global-search', 'slash-command', 'slides', 'sync', 'tag-pane', 'templates', 'zk-prefixer',
  'webviewer', 'word-count', 'workspaces'];

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Note.md'), 'First paragraph with a few plain words in it.\n\nSecond paragraph links [[Other]] and has a #tag and a [link](https://example.com).\n\n- [ ] a task\n- bullet\n\n```js\ncode();\n```\n');
  fs.writeFileSync(vp('Other.md'), 'The other note.\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  const plugins = {}; for (const id of CORE) plugins[id] = true;
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify(plugins, null, 2));
  fs.writeFileSync(RESULTS, '{}');
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const T0 = Date.now();
const c = await connect();
const out = { vault: await c.vaultPath(), startedAt: new Date().toISOString(), steps: {}, order: [] };
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(out, null, 1)); };
save();
const ONLY = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return null;
  out.order.push(name);
  const t = Date.now(); const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save(); return out.steps[name];
}
const ev = (js) => c.evaluate(js);
const cfg = (name) => { try { return fs.readFileSync(vp('.obsidian/' + name), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
const configFiles = () => Object.fromEntries(fs.readdirSync(vp('.obsidian')).filter((f) => f.endsWith('.json')).map((f) => [f, cfg(f)]));

out.version = await ev(`return { ua: navigator.userAgent, title: document.title };`);

// Menu rows: title, lucide icon class, section, disabled, submenu.
const MENU_ROWS = `
  return Array.from(document.querySelectorAll('.menu')).map((m) => ({
    cls: m.className,
    items: Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((it) => {
      if (it.classList.contains('menu-separator')) return { separator: true };
      const svg = it.querySelector('.menu-item-icon svg');
      return {
        title: ((it.querySelector('.menu-item-title') || {}).textContent || '').trim(),
        icon: svg ? svg.getAttribute('class') : null,
        section: it.getAttribute('data-section'),
        disabled: it.classList.contains('is-disabled'),
        submenu: !!it.querySelector('.menu-item-icon.mod-submenu, .lucide-chevron-right'),
        cls: it.className,
      };
    }),
  }));`;

const openNote = async (file, mode) => ev(`
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const f = app.vault.getAbstractFileByPath(${JSON.stringify(file)});
  const leaf = app.workspace.getLeaf(false);
  await leaf.openFile(f);
  await sleep(500);
  const st = leaf.getViewState();
  st.state.mode = ${JSON.stringify(mode === 'reading' ? 'preview' : 'source')};
  st.state.source = ${mode === 'source'};
  await leaf.setViewState(st);
  await sleep(900);
  const v = leaf.view;
  return { mode: v.getMode && v.getMode(), source: v.getState && v.getState().source };`);

const editorCursor = () => ev(`
  const v = app.workspace.activeEditor; if (!v || !v.editor) return null;
  const ed = v.editor; return { cursor: ed.getCursor(), selection: ed.getSelection(), hasFocus: ed.hasFocus() };`);

// Viewport rect of a word in the editor, found by walking the rendered lines.
const wordRect = (needle, root) => ev(`
  const root = document.querySelector(${JSON.stringify(root)}); if (!root) return { noRoot: true };
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let n; while ((n = walker.nextNode())) {
    const i = n.textContent.indexOf(${JSON.stringify(needle)});
    if (i >= 0) {
      const range = document.createRange(); range.setStart(n, i); range.setEnd(n, i + ${JSON.stringify(needle)}.length);
      const r = range.getBoundingClientRect();
      return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2, text: n.textContent };
    }
  }
  return { notFound: true };`);

const rightClickMenu = async (rect) => {
  await c.clickAt(rect.cx, rect.cy, { button: 'right' });
  await sleep(700);
  const menus = await ev(MENU_ROWS);
  const flat = await c.openMenuItems();
  await c.pressKey('Escape'); await sleep(300);
  return { menus, flat };
};

// ---- 1. the editor's right-click menu ------------------------------------
for (const mode of ['live', 'source']) {
  await step(`s01-editor-menu-${mode}`, async (r) => {
    r.opened = await openNote('Note.md', mode);
    const root = '.workspace-leaf.mod-active .cm-editor';
    // Caret at the start, then a right click on "plain": does it move?
    await ev(`app.workspace.activeEditor.editor.setCursor({ line: 0, ch: 0 }); app.workspace.activeEditor.editor.focus(); return true;`);
    await sleep(200);
    r.before = await editorCursor();
    const plain = await wordRect('plain', root);
    r.plainRect = plain;
    if (plain.cx) {
      r.plain = await rightClickMenu(plain);
      r.afterPlain = await editorCursor();
    }
    // A selection, then a right click inside it: does the selection survive?
    await ev(`const ed = app.workspace.activeEditor.editor; ed.setSelection({ line: 0, ch: 6 }, { line: 0, ch: 20 }); ed.focus(); return ed.getSelection();`);
    await sleep(200);
    r.selectionBefore = await editorCursor();
    const inside = await wordRect('paragraph', root);
    if (inside.cx) {
      r.insideSelection = await rightClickMenu(inside);
      r.afterInside = await editorCursor();
    }
    // A right click outside the selection.
    await ev(`const ed = app.workspace.activeEditor.editor; ed.setSelection({ line: 0, ch: 6 }, { line: 0, ch: 20 }); ed.focus(); return true;`);
    const outside = await wordRect('words', root);
    if (outside.cx) {
      r.outsideSelection = await rightClickMenu(outside);
      r.afterOutside = await editorCursor();
    }
    // On the wikilink, the tag, the external link and the task checkbox.
    await ev(`app.workspace.activeEditor.editor.setCursor({ line: 0, ch: 0 }); return true;`);
    const link = await wordRect('Other', root);
    if (link.cx) r.onWikilink = await rightClickMenu(link);
    const tag = await wordRect('tag', root);
    if (tag.cx) r.onTag = await rightClickMenu(tag);
    const ext = await wordRect(mode === 'live' ? 'link' : 'example.com', root);
    if (ext.cx) r.onExternalLink = await rightClickMenu(ext);
    const task = await c.rect('.workspace-leaf.mod-active .cm-editor .task-list-item-checkbox', 0);
    if (task) r.onCheckbox = await rightClickMenu(task);
    const code = await wordRect('code()', root);
    if (code.cx) r.onCodeBlock = await rightClickMenu(code);
    return r;
  });
}

await step('s02-reading-menu', async (r) => {
  r.opened = await openNote('Note.md', 'reading');
  const root = '.workspace-leaf.mod-active .markdown-reading-view';
  const plain = await wordRect('plain', root);
  if (plain.cx) r.plain = await rightClickMenu(plain);
  const link = await wordRect('Other', root);
  if (link.cx) r.onWikilink = await rightClickMenu(link);
  // A selection made by dragging, then the menu over it.
  const from = await wordRect('First', root);
  const to = await wordRect('words', root);
  if (from.cx && to.cx) {
    await c.drag(from.x + 1, from.cy, to.x + to.w - 1, to.cy);
    await sleep(300);
    r.selected = await ev(`return window.getSelection().toString();`);
    r.onSelection = await rightClickMenu(to);
  }
  return r;
});

// ---- 2. the view header's actions ------------------------------------------
await step('s03-view-header-actions', async (r) => {
  r.opened = await openNote('Note.md', 'live');
  r.actions = await ev(`
    const h = document.querySelector('.workspace-leaf.mod-active .view-header');
    return Array.from(h.querySelectorAll('.view-action, .view-header-title-container, .view-header-nav-buttons .clickable-icon')).map((a) => ({
      aria: a.getAttribute('aria-label'), cls: a.className,
      icon: (() => { const s = a.querySelector('svg'); return s && s.getAttribute('class'); })(),
      rect: (() => { const b = a.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; })(),
    }));`);
  const more = await c.rect('.workspace-leaf.mod-active .view-header .view-action[aria-label="More options"]', 0);
  if (more) {
    await c.clickAt(more.cx, more.cy);
    await sleep(700);
    r.moreOptions = await ev(MENU_ROWS);
    r.moreOptionsFlat = await c.openMenuItems();
    await c.pressKey('Escape'); await sleep(300);
  }
  // The same menu in reading view, and for a note with no selection.
  await openNote('Note.md', 'reading');
  const more2 = await c.rect('.workspace-leaf.mod-active .view-header .view-action[aria-label="More options"]', 0);
  if (more2) {
    await c.clickAt(more2.cx, more2.cy);
    await sleep(700);
    r.moreOptionsReading = await c.openMenuItems();
    await c.pressKey('Escape'); await sleep(300);
  }
  const bm = await c.rect('.workspace-leaf.mod-active .view-header .view-action.mod-bookmark, .workspace-leaf.mod-active .view-header .view-action[aria-label="Bookmark"]', 0);
  r.bookmarkRect = bm;
  if (bm) {
    await c.clickAt(bm.cx, bm.cy);
    await sleep(800);
    r.bookmarkOpens = await ev(`
      const m = document.querySelector('.modal'); const menu = document.querySelector('.menu');
      return { modal: m ? { cls: m.className, title: ((m.querySelector('.modal-title') || {}).textContent || '').trim(), text: m.textContent.trim().slice(0, 400),
        inputs: Array.from(m.querySelectorAll('input')).map((i) => ({ value: i.value, placeholder: i.placeholder })),
        buttons: Array.from(m.querySelectorAll('button')).map((b) => b.textContent.trim()) } : null,
        menu: menu ? Array.from(menu.querySelectorAll('.menu-item-title')).map((t) => t.textContent) : null,
        bookmarksFile: null };`);
    r.bookmarksJson = cfg('bookmarks.json');
    await c.pressKey('Escape'); await sleep(300);
  }
  return r;
});

// ---- 3. every core plugin's options tab -----------------------------------
const ROWS = `
  const el = app.setting.activeTab.containerEl;
  return Array.from(el.querySelectorAll('.setting-item')).map((it) => ({
    name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(),
    description: ((it.querySelector('.setting-item-description') || {}).innerHTML || '').trim(),
    descriptionText: ((it.querySelector('.setting-item-description') || {}).textContent || '').trim(),
    heading: it.classList.contains('setting-item-heading'),
    control: (() => {
      const sel = it.querySelector('select');
      if (sel) return { kind: 'dropdown', value: sel.value, options: Array.from(sel.options).map((o) => ({ value: o.value, label: o.textContent })) };
      const tog = it.querySelector('.checkbox-container');
      if (tog) return { kind: 'toggle', on: tog.classList.contains('is-enabled') };
      const slider = it.querySelector('input[type="range"]');
      if (slider) return { kind: 'slider', value: slider.value, min: slider.min, max: slider.max, step: slider.step };
      const txt = it.querySelector('input[type="text"], input[type="search"], input[type="number"], input:not([type])');
      if (txt) return { kind: 'text', value: txt.value, placeholder: txt.getAttribute('placeholder'), type: txt.type, classes: txt.className };
      const ta = it.querySelector('textarea');
      if (ta) return { kind: 'textarea', value: ta.value, placeholder: ta.getAttribute('placeholder') };
      const btns = Array.from(it.querySelectorAll('button'));
      if (btns.length) return { kind: 'button', text: btns.map((b) => b.textContent.trim()), classes: btns.map((b) => b.className) };
      const extra = it.querySelector('.setting-item-control');
      return { kind: 'none', controlHtml: extra ? extra.innerHTML.slice(0, 300) : null };
    })(),
    hidden: it.offsetParent === null,
    classes: it.className,
  }));`;

await step('s04-plugin-tabs', async (r) => {
  r.tabs = {};
  r.navItems = await ev(`
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    app.setting.open(); await sleep(1200);
    return { tabs: app.setting.pluginTabs.map((t) => ({ id: t.id, name: t.name })),
      settingTabs: app.setting.settingTabs.map((t) => ({ id: t.id, name: t.name })),
      nav: Array.from(app.setting.containerEl.querySelectorAll('.vertical-tab-nav-item')).map((x) => x.textContent.trim()) };`);
  save();
  for (const tab of r.navItems.tabs) {
    const t = { id: tab.id, name: tab.name };
    try {
      t.open = await ev(`
        const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
        app.setting.openTabById(${JSON.stringify(tab.id)});
        await sleep(900);
        const a = app.setting.activeTab;
        return { tabId: a.id, tabName: a.name, html: a.containerEl.innerHTML.length };`);
      t.rows = await ev(ROWS);
      t.filesBefore = configFiles();
      // Every toggle on and off again: the key and file it writes.
      t.toggled = {};
      for (const row of t.rows.filter((x) => x.control.kind === 'toggle' && !x.hidden)) {
        const flips = [];
        for (let k = 0; k < 2; k++) {
          const before = configFiles();
          const res = await ev(`
            const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
            const el = app.setting.activeTab.containerEl;
            const row = Array.from(el.querySelectorAll('.setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === ${JSON.stringify(row.name)});
            if (!row) return { noRow: true };
            const tog = row.querySelector('.checkbox-container'); if (!tog) return { noToggle: true };
            tog.click(); await sleep(1200);
            return { on: tog.classList.contains('is-enabled'),
              visible: Array.from(el.querySelectorAll('.setting-item')).map((it) => ({ name: (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim(), shown: it.offsetParent !== null })) };`);
          const after = configFiles();
          const changed = {};
          for (const f of new Set([...Object.keys(before), ...Object.keys(after)])) if (before[f] !== after[f]) changed[f] = { before: before[f], after: after[f] };
          flips.push({ ...res, changed });
        }
        t.toggled[row.name] = flips;
        save();
      }
      // Every dropdown through its options and back to the first value seen.
      t.dropdowns = {};
      for (const row of t.rows.filter((x) => x.control.kind === 'dropdown' && !x.hidden)) {
        const picks = {};
        const original = row.control.value;
        const order = [...row.control.options.map((o) => o.value).filter((v) => v !== original), original];
        for (const value of order) {
          const before = configFiles();
          const res = await ev(`
            const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
            const el = app.setting.activeTab.containerEl;
            const row = Array.from(el.querySelectorAll('.setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === ${JSON.stringify(row.name)});
            if (!row) return { noRow: true };
            const sel = row.querySelector('select'); sel.value = ${JSON.stringify(value)};
            sel.dispatchEvent(new Event('change', { bubbles: true })); await sleep(1200);
            return { value: sel.value, visible: Array.from(el.querySelectorAll('.setting-item')).map((it) => ({ name: (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim(), shown: it.offsetParent !== null })) };`);
          const after = configFiles();
          const changed = {};
          for (const f of new Set([...Object.keys(before), ...Object.keys(after)])) if (before[f] !== after[f]) changed[f] = { before: before[f], after: after[f] };
          picks[value] = { ...res, changed };
        }
        t.dropdowns[row.name] = picks;
        save();
      }
      t.rowsAfter = await ev(ROWS);
      t.filesAfter = configFiles();
    } catch (e) {
      t.error = String(e && e.stack ? e.stack : e);
    }
    r.tabs[tab.id] = t;
    save();
  }
  await ev(`app.setting.close(); return true;`);
  await sleep(500);
  return r;
});

out.finishedAt = new Date().toISOString();
save();
c.close();
process.exit(0);
