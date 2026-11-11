// The observations five clusters of "unrunnable" commands wait on
// ([[Development#The unrunnable commands, 2026-11-09]], "Waiting on a probe"):
//
//   bl   backlink:toggle-backlinks-in-document: the markdown leaf's state in
//        workspace.json off, on and off again; the in-note block's DOM in Live
//        Preview, Source mode and reading view; the tab header menu's row.
//        SCEN=blsetting starts with backlinkInDocument on in backlink.json and
//        reads what a new tab carries.
//   fn   footnotes:open: the leaf (type, state, icon, title, dock), the pane's
//        markup, the empty states, a row click in each mode, editing, menus.
//   ws   the Workspaces plugin: each command's modal, workspaces.json after
//        saves, a load, an overwrite, a rename and a delete, and whether a load
//        rewrites workspace.json at once.
//   rh   editor:rename-heading: availability, the modal, the bytes in the note
//        and in every link form to the heading, odd names, alwaysUpdateLinks off.
//   al   markdown:add-alias over five frontmatter shapes: bytes before and after
//        a value, focus and caret, availability by mode.
//
// One scenario per run (SCEN=bl|blsetting|fn|ws|rh|al), each on the fresh
// vault run-block.sh makes; every scenario writes under its own key of one
// results file. ONLY_STEPS=<regex> narrows the run phase's steps.
// Disk bytes are read by node straight from the vault after polling out
// Obsidian's save debounce; each poll records how long the change took.
//
// Phases: fixtures | run   (node panes-workspaces-headings.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SCEN = process.env.SCEN || 'bl';
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 520000;
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all[SCEN] = all[SCEN] || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all[SCEN][phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('timeout ' + ms + 'ms: ' + what)), ms))]);
async function step(name, fn, ms = 90000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  out.steps[name] = { running: true, partial };
  try { const v = await withTimeout(fn(partial), ms, name); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  try { await cleanup(); } catch { /* ignore */ }
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
/** Poll a file until its bytes differ from `prev`, then let it settle; record the delay. */
async function pollChange(rel, prev, timeout = 9000) {
  const t = Date.now();
  while (Date.now() - t < timeout) {
    const b = bytes(rel);
    if (b !== prev) { const ms = Date.now() - t; await sleep(400); return { changed: true, ms, bytes: bytes(rel) }; }
    await sleep(100);
  }
  return { changed: false, waitedMs: timeout, bytes: bytes(rel) };
}
/** Every leaf in a workspace.json text, with the path of keys to it. */
function wsLeaves(text) {
  let ws; try { ws = JSON.parse(text); } catch { return null; }
  const leaves = [];
  const walk = (n, where) => {
    if (!n || typeof n !== 'object') return;
    if (n.type === 'leaf') leaves.push({ where, node: n });
    if (Array.isArray(n.children)) n.children.forEach((ch, i) => walk(ch, where + '.' + i));
  };
  for (const k of ['main', 'left', 'right', 'floating']) walk(ws[k], k);
  return { keys: Object.keys(ws), active: ws.active, lastOpenFiles: ws.lastOpenFiles, leaves };
}

// ---------------------------------------------------------------------------
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  const core = {
    'file-explorer': true, 'global-search': true, switcher: true, graph: false, backlink: true,
    'outgoing-link': true, 'tag-pane': true, properties: true, 'page-preview': true,
    'command-palette': true, 'editor-status': true, outline: true, 'file-recovery': true,
    workspaces: true, footnotes: true,
  };
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify(core, null, 2));
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  // backlinks
  fs.writeFileSync(vp('Target.md'), '# Target\n\nBody of the target note.\n');
  fs.writeFileSync(vp('LinkerA.md'), 'See [[Target]] here.\n');
  fs.writeFileSync(vp('LinkerB.md'), 'Another [[Target|alias]] link.\n\nAnd a second paragraph naming [[Target]] again.\n');
  fs.writeFileSync(vp('Mention.md'), 'This one mentions Target without a link.\n');
  fs.writeFileSync(vp('Other.md'), '# Other\n\nNothing links here.\n');
  if (SCEN === 'blsetting') fs.writeFileSync(vp('.obsidian/backlink.json'), '{\n  "backlinkInDocument": true\n}');
  // footnotes
  const filler = Array.from({ length: 40 }, (_, i) => `Filler line ${i + 1} so the note scrolls.`).join('\n\n');
  fs.writeFileSync(vp('Footnotes.md'), [
    '# Footnotes', '',
    'First claim.[^1] Second claim.[^named] Inline one.^[An inline note.] Uses one again.[^1] Missing one.[^missing]', '',
    filler, '',
    '[^named]: The named definition,', '    continued on a second line.', '',
    '[^1]: The first definition.', '',
    '[^orphan]: A definition nothing references.', '',
  ].join('\n'));
  fs.writeFileSync(vp('NoFootnotes.md'), '# Plain\n\nNo notes here.\n');
  // workspaces
  fs.writeFileSync(vp('A.md'), '# A\n\nNote A.\n');
  fs.writeFileSync(vp('B.md'), '# B\n\nNote B.\n');
  fs.writeFileSync(vp('C.md'), '# C\n\nNote C.\n');
  // rename heading / alias: seeded per case in the run phase
  fs.writeFileSync(vp('Note.md'), 'seed\n');
  fs.writeFileSync(vp('Linker.md'), 'seed\n');
  fs.writeFileSync(vp('Alias.md'), 'seed\n');
  out.files = fs.readdirSync(VAULT);
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

// ---------------------------------------------------------------------------
const c = await connect();
out.vaultPath = await c.vaultPath();
save();

await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  leaf() {
    const a = app.workspace.activeLeaf;
    if (a && a.view && a.view.getViewType() === 'markdown') return a;
    const ls = app.workspace.getLeavesOfType('markdown');
    return ls.length ? ls[0] : app.workspace.getLeaf(false);
  },
  view() { return this.leaf().view; },
  ed() { return this.view().editor; },
  async open(file, mode) {
    let f = null;
    for (let i = 0; i < 40 && !f; i++) { f = app.vault.getAbstractFileByPath(file); if (!f) await this.sleep(150); }
    const ls = app.workspace.getLeavesOfType('markdown');
    const leaf = ls.length ? ls[0] : app.workspace.getLeaf(false);
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(400);
    await this.mode(mode || 'live');
    return { leaves: app.workspace.getLeavesOfType('markdown').length, file: leaf.view.file && leaf.view.file.path, state: leaf.getViewState().state };
  },
  async mode(m, leaf) {
    leaf = leaf || this.leaf();
    const st = leaf.getViewState();
    const s = m === 'preview' ? { mode: 'preview' } : { mode: 'source', source: m === 'source' };
    st.state = Object.assign({}, st.state, s);
    await leaf.setViewState(st);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(350);
    return leaf.getViewState().state;
  },
  async seed(file, content, settle) {
    let f = app.vault.getAbstractFileByPath(file);
    await app.vault.modify(f, content);
    await this.sleep(settle || 900);
    const v = this.view();
    const ok = v.file && v.file.path === file ? (v.getMode() === 'preview' ? v.data === content : v.editor.getValue() === content) : null;
    return ok;
  },
  describe(el) {
    if (!el) return null;
    return { tag: el.tagName, cls: typeof el.className === 'string' ? el.className : (el.className && el.className.baseVal), text: (el.textContent || '').trim().slice(0, 160), aria: el.getAttribute && el.getAttribute('aria-label') };
  },
  CS: ['display','font-size','font-weight','line-height','color','background-color','padding','margin','border-radius','opacity','cursor','white-space','gap','flex-direction','text-overflow','overflow'],
  dump(el, depth, styles) {
    depth = depth || 0;
    if (!el) return null;
    if (depth > 14) return { tag: el.tagName, truncated: true };
    const tag = el.tagName.toLowerCase();
    const cls = typeof el.className === 'string' ? el.className : (el.className && el.className.baseVal);
    const r = el.getBoundingClientRect();
    const o = { tag, cls, rect: [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)] };
    for (const a of Array.from(el.attributes)) if (a.name !== 'class' && a.name !== 'd' && a.name !== 'points') o['@' + a.name] = a.value.slice(0, 200);
    const own = Array.from(el.childNodes).filter(n => n.nodeType === 3).map(n => n.textContent).join('').trim();
    if (own) o.text = own.slice(0, 300);
    if (styles && tag !== 'svg' && tag !== 'path') { const s = getComputedStyle(el); o.style = {}; for (const p of this.CS) o.style[p] = s.getPropertyValue(p); }
    if (tag === 'svg') return o;
    const kids = Array.from(el.children);
    if (kids.length) o.children = kids.slice(0, 60).map(k => this.dump(k, depth + 1, styles));
    if (kids.length > 60) o.moreChildren = kids.length - 60;
    return o;
  },
  modal() {
    const ms = Array.from(document.querySelectorAll('.modal-container'));
    if (!ms.length) return null;
    return ms.map((m) => {
      const root = m.querySelector('.modal, .prompt') || m;
      const q = (s) => Array.from(root.querySelectorAll(s));
      return {
        containerCls: m.className,
        cls: root.className,
        title: this.describe(root.querySelector('.modal-title')),
        text: (root.textContent || '').trim().slice(0, 600),
        inputs: q('input, textarea').map((i) => ({ cls: i.className, placeholder: i.placeholder, value: i.value, type: i.type, focused: document.activeElement === i, selStart: i.selectionStart, selEnd: i.selectionEnd })),
        buttons: q('button, .clickable-icon').map((b) => ({ ...this.describe(b), cta: b.classList.contains('mod-cta'), warning: b.classList.contains('mod-warning'), focused: document.activeElement === b })),
        items: q('.suggestion-item').map((x) => ({ cls: x.className, text: x.textContent.trim().slice(0, 160), html: x.innerHTML.slice(0, 600) })),
        instructions: q('.prompt-instruction').map((x) => x.textContent.trim()),
        html: root.outerHTML.slice(0, 6000),
      };
    });
  },
  active() { const a = document.activeElement; const d = this.describe(a); if (a && 'selectionStart' in a) { try { d.selStart = a.selectionStart; d.selEnd = a.selectionEnd; d.value = a.value; } catch {} } if (a && a.isContentEditable) { const s = window.getSelection(); d.contentEditable = true; d.selText = s && s.toString(); } return d; },
  cmd(id) {
    const c = app.commands.commands[id] || (app.commands.editorCommands && app.commands.editorCommands[id]);
    if (!c) return null;
    const o = { name: c.name, icon: c.icon };
    for (const k of ['callback','checkCallback','editorCallback','editorCheckCallback']) o[k] = typeof c[k] === 'function';
    o.hotkeys = c.hotkeys || null;
    return o;
  },
  allLeaves() {
    const o = [];
    app.workspace.iterateAllLeaves((l) => {
      let root = l.getRoot && l.getRoot();
      const where = root === app.workspace.leftSplit ? 'left' : root === app.workspace.rightSplit ? 'right' : root === app.workspace.rootSplit ? 'main' : 'other';
      const p = l.parent;
      o.push({ id: l.id, type: l.view.getViewType(), where, index: p && p.children ? p.children.indexOf(l) : null, siblings: p && p.children ? p.children.map(x => x.view.getViewType()) : null, display: l.getDisplayText(), icon: l.view.getIcon && l.view.getIcon(), state: l.getViewState() });
    });
    return o;
  },
  menus() {
    return Array.from(document.querySelectorAll('.menu')).map((m) => Array.from(m.querySelectorAll('.menu-item, .menu-separator')).map((it) => it.classList.contains('menu-separator') ? { sep: true, h: it.offsetHeight } : {
      title: ((it.querySelector('.menu-item-title') || it).textContent || '').trim(),
      icon: (() => { const s = it.querySelector('.menu-item-icon svg'); return s ? s.getAttribute('class') : null; })(),
      checked: !!it.querySelector('.menu-item-icon.mod-checked, .is-checked') || it.classList.contains('mod-checked'),
      section: it.getAttribute('data-section'), cls: it.className,
    }));
  },
};
return true;`);

const ev = (body) => c.evaluate(body);
const P = (body) => c.evaluate(`return await (async () => { const p = window.__p; ${body} })();`);
async function cleanup() {
  for (let i = 0; i < 3; i++) {
    const n = await ev(`return document.querySelectorAll('.modal-container, .menu, .suggestion-container').length;`);
    if (!n) return;
    await c.pressKey('Escape'); await sleep(200);
  }
  await ev(`document.querySelectorAll('.menu').forEach(m => m.remove()); return true;`);
}
const modal = () => P(`return p.modal();`);
const runCmd = (id) => ev(`return app.commands.executeCommandById(${JSON.stringify(id)});`);
/** Type into whatever has focus, after selecting all of it. */
async function replaceTyped(text) {
  await c.pressKey('a', { ctrl: true }); await sleep(80);
  if (text === '') { await c.pressKey('Backspace'); await sleep(80); }
  else await c.typeText(text, 25);
  await sleep(150);
}
/** Enter as a keyDown with text, which (unlike rawKeyDown) also raises keypress. */
async function enterWithKeypress() {
  const p = { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13, text: '\r', unmodifiedText: '\r' };
  await c.send('Input.dispatchKeyEvent', { type: 'keyDown', ...p });
  await c.send('Input.dispatchKeyEvent', { type: 'keyUp', ...p });
}
/** Is a command offered by the palette for this query? */
async function palette(query) {
  await runCmd('command-palette:open'); await sleep(400);
  await c.typeText(query, 20); await sleep(400);
  const items = await ev(`return Array.from(document.querySelectorAll('.prompt .suggestion-item')).map(x => x.textContent.trim());`);
  await c.pressKey('Escape'); await sleep(250);
  return items;
}

// ---------------------------------------------------------------------------
await step('s00-context', async (r) => {
  r.versions = await ev(`return { title: document.title, apiVersion: String(app.apiVersion), ua: navigator.userAgent };`);
  r.internalPlugins = await ev(`const o = {}; for (const [id, p] of Object.entries(app.internalPlugins.plugins)) o[id] = { enabled: p.enabled, name: p.instance && p.instance.name }; return o;`);
  r.coreJson = bytes('.obsidian/core-plugins.json');
  r.commands = await P(`
    const ids = Object.keys(app.commands.commands).filter(id => /^(footnotes|workspaces|backlink|markdown):/.test(id) || id === 'editor:rename-heading');
    for (const id of ['backlink:toggle-backlinks-in-document','footnotes:open','workspaces:save','workspaces:load','workspaces:save-and-load','workspaces:open-modal','editor:rename-heading','markdown:add-alias']) if (!ids.includes(id)) ids.push(id);
    const o = {}; for (const id of ids) o[id] = p.cmd(id); return o;`);
  r.ribbon = await ev(`return Array.from(document.querySelectorAll('.side-dock-ribbon .side-dock-ribbon-action')).map(b => ({ aria: b.getAttribute('aria-label'), icon: (b.querySelector('svg') || {}).getAttribute && b.querySelector('svg').getAttribute('class'), cls: b.className, display: getComputedStyle(b).display }));`);
  r.leaves = await P(`return p.allLeaves();`);
  r.workspaceJson = bytes('.obsidian/workspace.json');
  return r;
});

// ===========================================================================
if (SCEN === 'bl' || SCEN === 'blsetting') {
  const blockInfo = () => P(`
    const v = p.leaf().view;
    const els = Array.from(v.containerEl.querySelectorAll('[class*="backlink"]'));
    const top = els.filter(e => !els.some(o => o !== e && o.contains(e)));
    return { mode: v.getMode(), count: els.length, classes: [...new Set(els.map(e => e.className))].slice(0, 40),
      blocks: top.map(e => ({ parentCls: e.parentElement && e.parentElement.className, display: getComputedStyle(e).display, dump: p.dump(e, 0, true) })) };`);
  const mdLeafState = (text) => { const w = wsLeaves(text); return w && w.leaves.filter(l => l.node.state && l.node.state.type === 'markdown'); };
  const tabMenu = async () => {
    const hit = await c.rightClickElement('.mod-root .workspace-tab-header.is-active');
    await sleep(400);
    const m = await P(`return p.menus();`);
    await c.pressKey('Escape'); await sleep(250);
    await ev(`document.querySelectorAll('.menu').forEach(m => m.remove()); return true;`);
    return { hit: !!hit, menus: m };
  };
  const moreMenu = async () => {
    const hit = await c.clickElement('.mod-root .workspace-leaf.mod-active .view-header .view-action[aria-label="More options"]');
    await sleep(400);
    const m = await P(`return p.menus();`);
    await c.pressKey('Escape'); await sleep(250);
    await ev(`document.querySelectorAll('.menu').forEach(m => m.remove()); return true;`);
    return { hit: !!hit, menus: m };
  };

  await step('b01-open-target', async (r) => {
    r.backlinkJson = bytes('.obsidian/backlink.json');
    r.plugin = await ev(`const p = app.internalPlugins.plugins.backlink; const i = p && p.instance; return i ? { options: i.options || null, keys: Object.keys(i) } : null;`);
    r.opened = await P(`app.workspace.rightSplit.collapse(); return await p.open('Target.md', 'live');`);
    await sleep(2500);
    r.wsBefore = bytes('.obsidian/workspace.json');
    r.ws = await pollChange('.obsidian/workspace.json', r.wsBefore, 4000);
    r.mdLeaves = mdLeafState(r.ws.bytes);
    r.viewState = await P(`return p.leaf().getViewState();`);
    r.blockBefore = await blockInfo();
    r.tabMenu = await tabMenu();
    r.moreMenu = await moreMenu();
    return r;
  });

  await step('b02-toggle-on', async (r) => {
    const prev = bytes('.obsidian/workspace.json');
    r.ran = await runCmd('backlink:toggle-backlinks-in-document');
    await sleep(800);
    r.viewState = await P(`return p.leaf().getViewState();`);
    r.ws = await pollChange('.obsidian/workspace.json', prev, 6000);
    r.mdLeaves = mdLeafState(r.ws.bytes);
    r.backlinkJson = bytes('.obsidian/backlink.json');
    r.live = await blockInfo();
    r.tabMenu = await tabMenu();
    r.moreMenu = await moreMenu();
    return r;
  });

  await step('b03-block-in-modes', async (r) => {
    r.sourceState = await P(`return await p.mode('source');`); await sleep(600);
    r.source = await blockInfo();
    r.previewState = await P(`return await p.mode('preview');`); await sleep(900);
    r.preview = await blockInfo();
    // Unfold whatever is folded in the block, then read again.
    r.foldClicks = await P(`
      const v = p.leaf().view;
      const heads = Array.from(v.containerEl.querySelectorAll('[class*="backlink"] .tree-item-self.is-clickable, [class*="backlink"] .collapse-icon'));
      return heads.map(h => p.describe(h));`);
    r.livePState = await P(`return await p.mode('live');`); await sleep(600);
    r.liveAgain = await blockInfo();
    // Click the unlinked mentions header (by its text) and read again.
    const hit = await P(`
      const v = p.leaf().view;
      const all = Array.from(v.containerEl.querySelectorAll('[class*="backlink"] *')).filter(e => /^unlinked mentions/i.test((e.textContent||'').trim()) && e.children.length <= 3);
      const t = all[all.length - 1]; if (!t) return null; t.scrollIntoView({ block: 'center' }); await p.sleep(200);
      const b = t.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2, el: p.describe(t) };`);
    r.unlinkedHit = hit;
    if (hit) { await c.clickAt(hit.x, hit.y); await sleep(1200); r.afterUnlinkedClick = await blockInfo(); }
    const prev = bytes('.obsidian/workspace.json');
    r.ws = await pollChange('.obsidian/workspace.json', prev, 4000);
    r.mdLeaves = mdLeafState(r.ws.bytes);
    return r;
  });

  await step('b04-toggle-off', async (r) => {
    const prev = bytes('.obsidian/workspace.json');
    r.ran = await runCmd('backlink:toggle-backlinks-in-document');
    await sleep(800);
    r.viewState = await P(`return p.leaf().getViewState();`);
    r.ws = await pollChange('.obsidian/workspace.json', prev, 6000);
    r.mdLeaves = mdLeafState(r.ws.bytes);
    r.block = await blockInfo();
    r.tabMenu = await tabMenu();
    r.backlinkJson = bytes('.obsidian/backlink.json');
    return r;
  });

  await step('b05-new-tab', async (r) => {
    // A second tab on another note: what its state carries.
    r.opened = await P(`
      const leaf = app.workspace.getLeaf('tab');
      await leaf.openFile(app.vault.getAbstractFileByPath('Target.md'));
      app.workspace.setActiveLeaf(leaf, { focus: true }); await p.sleep(800);
      return leaf.getViewState();`);
    const prev = bytes('.obsidian/workspace.json');
    r.ws = await pollChange('.obsidian/workspace.json', prev, 6000);
    r.mdLeaves = mdLeafState(r.ws.bytes);
    r.block = await blockInfo();
    r.tabMenu = await tabMenu();
    return r;
  });
}

// ===========================================================================
if (SCEN === 'fn') {
  const fnLeaf = () => P(`return p.allLeaves().filter(l => !['markdown','empty','file-explorer','search','bookmarks','tag','backlink','outgoing-link','outline','all-properties','file-properties'].includes(l.type));`);
  const pane = (styles) => P(`
    let l = null; app.workspace.iterateAllLeaves(x => { if (/footnote/i.test(x.view.getViewType())) l = x; });
    if (!l) return null;
    return { type: l.view.getViewType(), state: l.getViewState(), display: l.getDisplayText(), icon: l.view.getIcon(),
      text: l.view.containerEl.innerText.slice(0, 2000), dump: p.dump(l.view.containerEl, 0, ${styles ? 'true' : 'false'}) };`);

  await step('f01-open-with-nothing-open', async (r) => {
    r.before = await P(`app.workspace.getLeavesOfType('markdown').forEach(l => l.detach()); await p.sleep(400); return p.allLeaves();`);
    const prev = bytes('.obsidian/workspace.json');
    r.ran = await runCmd('footnotes:open');
    await sleep(1200);
    r.after = await P(`return p.allLeaves();`);
    r.newLeaves = r.after.filter(a => !r.before.some(b => b.id === a.id));
    r.pane = await pane(true);
    r.activeLeaf = await ev(`const l = app.workspace.activeLeaf; return l && { type: l.view.getViewType(), id: l.id };`);
    r.rightCollapsed = await ev(`return app.workspace.rightSplit.collapsed;`);
    r.ws = await pollChange('.obsidian/workspace.json', prev, 6000);
    const w = wsLeaves(r.ws.bytes);
    r.wsLeaves = w && w.leaves.filter(l => l.node.state && /footnote/i.test(l.node.state.type));
    r.wsRight = (() => { try { return JSON.parse(r.ws.bytes).right; } catch { return null; } })();
    return r;
  });

  await step('f02-no-footnotes-note', async (r) => {
    r.opened = await P(`return await p.open('NoFootnotes.md', 'live');`);
    await sleep(1000);
    r.pane = await pane(true);
    return r;
  });

  await step('f03-footnotes-note', async (r) => {
    r.opened = await P(`return await p.open('Footnotes.md', 'live');`);
    await sleep(1200);
    r.pane = await pane(true);
    r.buttons = await P(`
      let l = null; app.workspace.iterateAllLeaves(x => { if (/footnote/i.test(x.view.getViewType())) l = x; });
      return Array.from(l.view.containerEl.querySelectorAll('.clickable-icon, .nav-action-button, button')).map(b => ({ ...p.describe(b), icon: b.querySelector('svg') && b.querySelector('svg').getAttribute('class'), active: b.classList.contains('is-active') }));`);
    r.tooltips = [];
    const n = r.buttons ? r.buttons.length : 0;
    for (let i = 0; i < n; i++) {
      const pt = await P(`let l = null; app.workspace.iterateAllLeaves(x => { if (/footnote/i.test(x.view.getViewType())) l = x; });
        const b = Array.from(l.view.containerEl.querySelectorAll('.clickable-icon, .nav-action-button, button'))[${i}]; const r = b.getBoundingClientRect(); return { x: r.x + r.width/2, y: r.y + r.height/2, w: r.width };`);
      if (!pt || !pt.w) { r.tooltips.push(null); continue; }
      await c.mouseMove(pt.x, pt.y); await sleep(1200);
      r.tooltips.push(await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`));
      await c.mouseMove(5, 300); await sleep(300);
    }
    r.rows = await P(`
      let l = null; app.workspace.iterateAllLeaves(x => { if (/footnote/i.test(x.view.getViewType())) l = x; });
      const c = l.view.containerEl.querySelector('.view-content') || l.view.containerEl;
      return Array.from(c.querySelectorAll('.tree-item-self, [class*="footnote"]')).map(e => ({ cls: e.className, text: e.innerText.slice(0, 200), editable: e.isContentEditable, ce: e.getAttribute('contenteditable') }));`);
    return r;
  });

  const rowRects = () => P(`
    let l = null; app.workspace.iterateAllLeaves(x => { if (/footnote/i.test(x.view.getViewType())) l = x; });
    const c = l.view.containerEl.querySelector('.view-content') || l.view.containerEl;
    let rows = Array.from(c.querySelectorAll('.tree-item-self'));
    if (!rows.length) rows = Array.from(c.querySelectorAll('[class*="footnote"]')).filter(e => !e.querySelector('[class*="footnote"]'));
    return rows.map(e => { const b = e.getBoundingClientRect(); return { x: b.x + Math.min(40, b.width / 2), y: b.y + b.height / 2, w: b.width, text: e.innerText.slice(0, 80), cls: e.className }; });`);
  const editorProbe = () => P(`
    const v = p.leaf().view; const m = v.getMode();
    const o = { mode: m, srcMode: v.getState().source, scroll: v.currentMode.getScroll ? v.currentMode.getScroll() : null, active: p.active(),
      flashing: Array.from(v.containerEl.querySelectorAll('.is-flashing, [class*="flash"]')).map(e => p.describe(e)) };
    if (m !== 'preview') { o.cursor = v.editor.getCursor(); o.sel = v.editor.getSelection(); o.from = v.editor.getCursor('from'); o.to = v.editor.getCursor('to'); o.line = v.editor.getLine(o.cursor.line); }
    return o;`);

  for (const mode of ['source', 'live', 'preview']) {
    await step('f04-click-' + mode, async (r) => {
      r.state = await P(`const s = await p.mode('${mode}'); const v = p.leaf().view; if (v.getMode() !== 'preview') { v.editor.setCursor({ line: 0, ch: 0 }); } v.currentMode.applyScroll && v.currentMode.applyScroll(0); await p.sleep(300); return s;`);
      r.rows = await rowRects();
      r.clicks = [];
      for (let i = 0; i < (r.rows || []).length && i < 5; i++) {
        await P(`const v = p.leaf().view; if (v.getMode() !== 'preview') v.editor.setCursor({ line: 0, ch: 0 }); v.currentMode.applyScroll && v.currentMode.applyScroll(0); await p.sleep(300); return true;`);
        const rows = await rowRects();
        const row = rows[i];
        await c.clickAt(row.x, row.y);
        await sleep(60);
        const quick = await editorProbe();
        await sleep(900);
        r.clicks.push({ row: row.text, quick, settled: await editorProbe(), panesAfter: await P(`return p.allLeaves().map(l => l.type + '@' + l.where);`) });
      }
      return r;
    });
  }

  await step('f05-edit-and-menu', async (r) => {
    await P(`return await p.mode('live');`);
    const rows = await rowRects();
    if (rows && rows[0]) {
      await c.clickAt(rows[0].x, rows[0].y, { clickCount: 2 }); await sleep(600);
      r.afterDbl = { active: await P(`return p.active();`), editableEls: await ev(`return Array.from(document.querySelectorAll('.workspace-split.mod-right-split [contenteditable="true"], .workspace-split.mod-right-split input')).map(e => e.className);`) };
      await c.pressKey('Escape'); await sleep(200);
      await c.clickAt(rows[0].x, rows[0].y, { button: 'right' }); await sleep(500);
      r.rowMenu = await P(`return p.menus();`);
      await c.pressKey('Escape'); await sleep(200);
      r.fileBytesUnchanged = bytes('Footnotes.md');
    }
    // The pane with the note switched to reading view, and what it shows then.
    await P(`return await p.mode('preview');`); await sleep(800);
    r.paneInPreview = await pane(false);
    return r;
  });

  // Second pass at the clicks: the note scrolled to its end with the caret on
  // the last line first, so a jump to the top shows; what the pointer hit and
  // where focus went, by the leaf that holds it.
  const where = () => P(`
    const v = p.leaf().view; const m = v.getMode(); const a = document.activeElement;
    const lc = a && a.closest('.workspace-leaf-content');
    const sc = v.containerEl.querySelector(m === 'preview' ? '.markdown-preview-view' : '.cm-scroller');
    const o = { mode: m, scrollTop: sc && Math.round(sc.scrollTop), activeIn: lc && lc.getAttribute('data-type'), active: p.describe(a),
      flashing: Array.from(document.querySelectorAll('.is-flashing')).map(e => p.describe(e)), log: window.__clog.splice(0) };
    if (m !== 'preview') { o.cursor = v.editor.getCursor(); o.sel = v.editor.getSelection(); o.line = v.editor.getLine(o.cursor.line); }
    return o;`);
  for (const mode of ['source', 'live', 'preview']) {
    await step('f06-click2-' + mode, async (r) => {
      await ev(`window.__clog = []; if (!window.__clogOn) { window.__clogOn = true; for (const t of ['mousedown','click','dblclick']) document.addEventListener(t, (e) => { window.__clog.push({ t, target: (e.target.className && e.target.className.baseVal === undefined ? e.target.className : e.target.tagName) + '', prevented: e.defaultPrevented }); }, true); } return true;`);
      r.state = await P(`return await p.mode('${mode}');`);
      const targets = await P(`
        let l = null; app.workspace.iterateAllLeaves(x => { if (x.view.getViewType() === 'footnotes') l = x; });
        const items = Array.from(l.view.containerEl.querySelectorAll('.footnote-list-item'));
        return items.map((it, i) => { const id = it.querySelector('.footnote-id').getBoundingClientRect(); const ct = it.querySelector('.footnote-content p, .footnote-content').getBoundingClientRect(); const ln = it.querySelector('.markdown-embed-link'); const lr = ln && ln.getBoundingClientRect();
          return { i, text: it.innerText.replace(/\\n+/g, ' | ').slice(0, 60), id: [id.x + id.width / 2, id.y + id.height / 2], content: [ct.x + Math.min(30, ct.width / 2), ct.y + ct.height / 2], link: lr && lr.width ? [lr.x + lr.width / 2, lr.y + lr.height / 2] : null }; });`);
      r.targets = targets;
      r.clicks = [];
      for (const t of targets.slice(0, 4)) {
        for (const part of ['id', 'content']) {
          await P(`const v = p.leaf().view; if (v.getMode() !== 'preview') { const e = v.editor; e.setCursor({ line: e.lineCount() - 1, ch: 0 }); e.scrollIntoView({ from: { line: e.lineCount() - 1, ch: 0 }, to: { line: e.lineCount() - 1, ch: 0 } }); } else { const s = v.containerEl.querySelector('.markdown-preview-view'); s.scrollTop = s.scrollHeight; } await p.sleep(400); window.__clog = []; return true;`);
          const before = await where();
          const hit = await ev(`const e = document.elementFromPoint(${t[part][0]}, ${t[part][1]}); return e && (e.className + '') ;`);
          await c.clickAt(t[part][0], t[part][1]);
          await sleep(150);
          const quick = await where();
          await sleep(1000);
          const settled = await where();
          r.clicks.push({ row: t.text, part, hit, before, quick, settled });
          await c.pressKey('Escape'); await sleep(150);
        }
      }
      return r;
    }, 120000);
  }

  await step('f08-open-link-button', async (r) => {
    await ev(`window.__clog = window.__clog || []; return true;`);
    r.idStyles = await ev(`return Array.from(document.querySelectorAll('.footnotes-view .footnote-id')).map(e => { const s = getComputedStyle(e); return { cls: e.className, color: s.color, opacity: s.opacity, decoration: s.textDecorationLine, cursor: s.cursor, minWidth: s.minWidth, align: s.textAlign }; });`);
    for (const mode of ['source', 'live', 'preview']) {
      await P(`return await p.mode('${mode}');`);
      const res = [];
      for (const i of [0, 1, 3]) {
        await P(`const v = p.leaf().view; if (v.getMode() !== 'preview') { const e = v.editor; e.setCursor({ line: 0, ch: 0 }); e.scrollTo(0, 0); } else { v.containerEl.querySelector('.markdown-preview-view').scrollTop = 0; } await p.sleep(400); return true;`);
        const hov = await P(`const it = document.querySelectorAll('.footnotes-view .footnote-list-item')[${i}]; const r = it.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2];`);
        await c.mouseMove(hov[0], hov[1]); await sleep(400);
        const lk = await P(`const it = document.querySelectorAll('.footnotes-view .footnote-list-item')[${i}]; const l = it.querySelector('.markdown-embed-link'); const r = l.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, w: r.width, opacity: getComputedStyle(l).opacity, aria: l.getAttribute('aria-label') };`);
        const leavesBefore = await P(`return p.allLeaves().length;`);
        await c.clickAt(lk.x, lk.y); await sleep(1200);
        const st = await P(`const v = p.leaf().view; const m = v.getMode(); const a = document.activeElement; const lc = a && a.closest('.workspace-leaf-content');
          const sc = v.containerEl.querySelector(m === 'preview' ? '.markdown-preview-view' : '.cm-scroller');
          const o = { mode: m, scrollTop: sc && Math.round(sc.scrollTop), activeIn: lc && lc.getAttribute('data-type'), activeLeaf: app.workspace.activeLeaf && app.workspace.activeLeaf.view.getViewType(), leaves: p.allLeaves().map(l => l.type + '@' + l.where + ':' + l.display), flashing: Array.from(document.querySelectorAll('.is-flashing')).map(e => p.describe(e)) };
          if (m !== 'preview') { o.cursor = v.editor.getCursor(); o.sel = v.editor.getSelection(); o.line = v.editor.getLine(o.cursor.line); }
          return o;`);
        res.push({ i, link: lk, leavesBefore, after: st });
        await c.pressKey('Escape'); await sleep(200);
      }
      r[mode] = res;
    }
    return r;
  }, 120000);

  await step('f07-edit-in-pane', async (r) => {
    await P(`return await p.mode('live');`);
    const before = bytes('Footnotes.md');
    const t = await P(`
      let l = null; app.workspace.iterateAllLeaves(x => { if (x.view.getViewType() === 'footnotes') l = x; });
      const ct = l.view.containerEl.querySelector('.footnote-list-item .footnote-content p').getBoundingClientRect();
      return [ct.x + ct.width - 4, ct.y + ct.height / 2];`);
    await c.clickAt(t[0], t[1]); await sleep(500);
    r.afterClick = await P(`const a = document.activeElement; const lc = a && a.closest('.workspace-leaf-content'); return { active: p.describe(a), activeIn: lc && lc.getAttribute('data-type'), editing: Array.from(document.querySelectorAll('.footnotes-view .cm-editor')).length };`);
    await c.pressKey('End'); await c.typeText(' EDITED', 30); await sleep(300);
    r.afterType = await P(`const a = document.activeElement; const lc = a && a.closest('.workspace-leaf-content'); return { active: p.describe(a), activeIn: lc && lc.getAttribute('data-type'), mainValueHasEdit: p.leaf().view.editor.getValue().includes('EDITED') };`);
    r.disk = await pollChange('Footnotes.md', before, 5000);
    if (r.disk.changed) {
      const i = r.disk.bytes.indexOf('EDITED');
      r.diskAround = r.disk.bytes.slice(Math.max(0, i - 80), i + 40);
    }
    // Hover over a row: a page preview?
    await c.mouseMove(t[0] - 40, t[1]); await sleep(1500);
    r.hover = await ev(`return { popover: Array.from(document.querySelectorAll('.popover, .hover-popover')).map(e => e.className), tooltip: (document.querySelector('.tooltip') || {}).textContent || null };`);
    return r;
  });
}

// ===========================================================================
if (SCEN === 'ws') {
  const WSJ = '.obsidian/workspaces.json';
  const snapshot = async (r, label) => {
    r[label] = { workspaces: bytes(WSJ), workspace: bytes('.obsidian/workspace.json'), plugin: await ev(`const i = app.internalPlugins.plugins.workspaces.instance; return { active: i.activeWorkspace, names: Object.keys(i.workspaces || {}) };`) };
  };
  await step('w01-commands-open', async (r) => {
    r.workspacesJson = bytes(WSJ);
    r.plugin = await ev(`const i = app.internalPlugins.plugins.workspaces.instance; return { keys: Object.keys(i), active: i.activeWorkspace, workspaces: i.workspaces };`);
    await P(`return await p.open('A.md', 'live');`);
    for (const id of ['workspaces:open-modal', 'workspaces:save', 'workspaces:load', 'workspaces:save-and-load']) {
      const ran = await runCmd(id); await sleep(700);
      r[id] = { ran, modal: await modal(), active: await P(`return p.active();`) };
      await cleanup(); await sleep(300);
    }
    return r;
  });

  // Each save goes through the manage modal: type the name into its input, Enter.
  const saveViaModal = async (r, name) => {
    await runCmd('workspaces:open-modal'); await sleep(700);
    r.modalBefore = await modal();
    r.activeBefore = await P(`return p.active();`);
    await replaceTyped(name);
    r.typed = await modal();
    const prev = bytes(WSJ);
    // cdp.mjs's Enter is a rawKeyDown (no keypress event). Try it, then a
    // keyDown that also raises keypress, then the Save button; record which took.
    await c.pressKey('Enter'); await sleep(400);
    r.afterEnter = await modal();
    r.disk = await pollChange(WSJ, prev, 3000);
    r.savedBy = r.disk.changed ? 'rawKeyDown Enter' : null;
    if (!r.disk.changed) {
      await enterWithKeypress(); await sleep(400);
      r.afterKeypressEnter = await modal();
      r.disk = await pollChange(WSJ, prev, 3000);
      if (r.disk.changed) r.savedBy = 'keyDown Enter (with keypress)';
    }
    if (!r.disk.changed) {
      const hit = await c.clickElement('.modal-container .modal .list-item button.list-item-part');
      await sleep(400);
      r.afterSaveClick = await modal();
      r.disk = await pollChange(WSJ, prev, 4000);
      if (r.disk.changed) r.savedBy = 'Save button';
      r.saveHit = !!hit;
    }
    r.activeAfter = await P(`return p.active();`);
  };

  await step('w02-save-one', async (r) => {
    await P(`return await p.open('A.md', 'live');`);
    await sleep(2500);
    await saveViaModal(r, 'One');
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  await step('w02b-save-command-with-active', async (r) => {
    // With "One" active and the layout unchanged, workspaces:save again.
    await sleep(1500);
    const prev = bytes(WSJ);
    r.ran = await runCmd('workspaces:save'); await sleep(700);
    r.modal = await modal();
    r.notices = await ev(`return Array.from(document.querySelectorAll('.notice')).map(n => n.textContent);`);
    r.disk = await pollChange(WSJ, prev, 4000);
    r.identical = r.disk.bytes === prev;
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  await step('w03-save-two', async (r) => {
    r.layout = await P(`
      const leaf = app.workspace.getLeaf('split', 'vertical');
      await leaf.openFile(app.vault.getAbstractFileByPath('B.md'));
      app.workspace.setActiveLeaf(leaf, { focus: true }); await p.sleep(800);
      app.workspace.leftSplit.collapse(); await p.sleep(300);
      return p.allLeaves().map(l => l.type + '@' + l.where + ':' + l.display);`);
    await sleep(2500);
    await saveViaModal(r, 'Two');
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  const layout = () => P(`return { leaves: p.allLeaves().map(l => l.type + '@' + l.where + ':' + l.display), leftCollapsed: app.workspace.leftSplit.collapsed, rightCollapsed: app.workspace.rightSplit.collapsed, active: app.workspace.activeLeaf && app.workspace.activeLeaf.getDisplayText() };`);
  await step('w04-load-one', async (r) => {
    await sleep(2500);
    const prevWs = bytes('.obsidian/workspace.json');
    r.wsBefore = prevWs;
    const prev = bytes(WSJ);
    r.ran = await runCmd('workspaces:load'); await sleep(600);
    r.modal = await modal();
    await c.typeText('One', 30); await sleep(300);
    r.typed = await modal();
    const t = Date.now();
    await c.pressKey('Enter');
    let changedAt = null;
    while (Date.now() - t < 8000) { if (bytes('.obsidian/workspace.json') !== prevWs) { changedAt = Date.now() - t; break; } await sleep(50); }
    r.workspaceRewriteMs = changedAt;
    await sleep(1500);
    r.layoutAfter = await layout();
    r.workspacesChanged = bytes(WSJ) !== prev;
    r.notices = await ev(`return Array.from(document.querySelectorAll('.notice')).map(n => n.textContent);`);
    await snapshot(r, 'state');
    return r;
  });

  await step('w05-manage-rows', async (r) => {
    await runCmd('workspaces:open-modal'); await sleep(700);
    r.modal = await modal();
    r.styles = await ev(`const m = document.querySelector('.modal-container .modal'); const o = {}; for (const [k, s] of Object.entries({ modal: '.modal', title: '.modal-title', input: '.modal-content > .list-item input', save: '.modal-content > .list-item button', row: '.mod-manage-workspaces .list-item', name: '.mod-manage-workspaces .list-item-part.mod-extended > span', flair: '.flair.mod-pop', desc: '.list-item-desc', load: '.list-item-actions button', del: '.list-item-actions .clickable-icon' })) { const e = document.querySelector('.modal-container ' + s) || (s === '.modal' ? m : null); if (!e) { o[k] = null; continue; } const cs = getComputedStyle(e); const b = e.getBoundingClientRect(); o[k] = { rect: [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)], fontSize: cs.fontSize, fontWeight: cs.fontWeight, color: cs.color, bg: cs.backgroundColor, padding: cs.padding, radius: cs.borderRadius, textTransform: cs.textTransform }; } return o;`);
    // A double click on a row's name: does anything become editable?
    const nm = await c.rect('.modal-container .mod-manage-workspaces .list-item-part.mod-extended > span', 1);
    if (nm) { await c.clickAt(nm.cx, nm.cy, { clickCount: 2 }); await sleep(500); r.afterDblClickName = { active: await P(`return p.active();`), modal: await modal(), editable: await ev(`return Array.from(document.querySelectorAll('.modal-container [contenteditable="true"], .modal-container input')).map(e => e.className + ':' + (e.value || e.textContent));`) }; }
    // Hover the delete control for its tooltip.
    const dl = await c.rect('.modal-container .list-item-actions .clickable-icon');
    if (dl) { await c.mouseMove(dl.cx, dl.cy); await sleep(1200); r.deleteTooltip = await ev(`const t = document.querySelector('.tooltip'); return t ? t.textContent : null;`); }
    return r;
  });

  await step('w06-overwrite-one', async (r) => {
    // One is active; change the layout, then save under the existing name "One".
    await P(`await p.open('C.md', 'live'); return true;`);
    await sleep(2500);
    r.layout = await layout();
    await saveViaModal(r, 'One');
    r.after = await modal();
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  await step('w07-save-under-new-name', async (r) => {
    // There is no rename control; saving the active layout under a new name is the nearest thing.
    await saveViaModal(r, 'Three');
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  await step('w08-save-and-load', async (r) => {
    // Active: Three. Change the layout, then save-and-load "Two".
    await P(`await p.open('A.md', 'live'); return true;`);
    await sleep(2500);
    r.layoutBefore = await layout();
    const prev = bytes(WSJ); const prevWs = bytes('.obsidian/workspace.json');
    r.ran = await runCmd('workspaces:save-and-load'); await sleep(700);
    r.modal = await modal();
    r.diskAtOpen = await pollChange(WSJ, prev, 1500);
    await c.typeText('Two', 30); await sleep(300);
    r.typed = await modal();
    await c.pressKey('Enter'); await sleep(300);
    r.after = await modal();
    r.disk = await pollChange(WSJ, prev, 4000);
    r.wsChanged = await pollChange('.obsidian/workspace.json', prevWs, 4000);
    r.layoutAfter = await layout();
    r.notices = await ev(`return Array.from(document.querySelectorAll('.notice')).map(n => n.textContent);`);
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  await step('w09-load-button', async (r) => {
    await runCmd('workspaces:open-modal'); await sleep(700);
    const prevWs = bytes('.obsidian/workspace.json');
    const t = await ev(`const rows = Array.from(document.querySelectorAll('.modal-container .mod-manage-workspaces .list-item')); const row = rows.find(x => x.querySelector('.list-item-part.mod-extended > span').textContent === 'One'); if (!row) return null; const b = row.querySelector('.list-item-actions button').getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
    if (t) { await c.clickAt(t.x, t.y); await sleep(600); }
    r.after = await modal();
    r.wsChanged = await pollChange('.obsidian/workspace.json', prevWs, 4000);
    r.layoutAfter = await layout();
    await cleanup();
    await snapshot(r, 'state');
    return r;
  });

  await step('w10-delete', async (r) => {
    // Delete "Two" (not active), then "One" (active).
    for (const name of ['Two', 'One']) {
      await runCmd('workspaces:open-modal'); await sleep(700);
      const prev = bytes(WSJ);
      const t = await ev(`const rows = Array.from(document.querySelectorAll('.modal-container .mod-manage-workspaces .list-item')); const row = rows.find(x => x.querySelector('.list-item-part.mod-extended > span').textContent === ${JSON.stringify(name)}); if (!row) return null; const b = row.querySelector('.list-item-actions .clickable-icon').getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 };`);
      const o = { target: t };
      if (t) { await c.clickAt(t.x, t.y); await sleep(600); }
      o.after = await modal();
      o.disk = await pollChange(WSJ, prev, 4000);
      o.plugin = await ev(`const i = app.internalPlugins.plugins.workspaces.instance; return { active: i.activeWorkspace, names: Object.keys(i.workspaces || {}) };`);
      r[name] = o;
      await cleanup();
    }
    await snapshot(r, 'state');
    return r;
  });
}

// ===========================================================================
if (SCEN === 'rh') {
  const NOTE = '# Parent\n\n## Old\n\nUnder old. Self [[#Old]] and [[#Old|self alias]] and [[Note#Old]].\n\nSetext Old\n----------\n\nUnder setext.\n';
  const LINKER = [
    'A [[Note#Old]]',
    'B [[Note#Old|alias]]',
    'C ![[Note#Old]]',
    'D [x](Note.md#Old)',
    'E [[Note#Parent#Old]]',
    'F [[Note#Old|Old]]',
    'G [[Note#old]]',
    'H [[Note#Old|Note > Old]]',
    'I [y](<Note.md#Old>)',
    'J [[Note#Setext Old]] and [z](Note.md#Setext%20Old)',
    'K [[Note]]',
    '',
  ].join('\n');
  const seedBoth = async (alwaysUpdate) => {
    await ev(`app.vault.setConfig('alwaysUpdateLinks', ${JSON.stringify(alwaysUpdate)}); return true;`);
    await P(`await app.vault.modify(app.vault.getAbstractFileByPath('Linker.md'), ${JSON.stringify(LINKER)}); return true;`);
    await P(`return await p.open('Note.md', 'live');`);
    const ok = await P(`return await p.seed('Note.md', ${JSON.stringify(NOTE)}, 1500);`);
    // let the metadata cache resolve the new links
    await sleep(800);
    const cache = await ev(`const c = app.metadataCache.getCache('Note.md'); return c && (c.headings || []).map(h => h.heading + '@' + h.position.start.line);`);
    return { seedOk: ok, headings: cache };
  };
  const placeCaret = (line, ch) => P(`const e = p.ed(); e.focus(); e.setCursor({ line: ${line}, ch: ${ch} }); await p.sleep(150); return e.getCursor();`);
  /** Seed, put the caret on `line`, run, type `name` (null: just record and Escape), Enter, read disk. */
  const renameCase = async (r, { line = 2, ch = 4, name, mode = 'live', alwaysUpdate = true, answer, commit }) => {
    r.seed = await seedBoth(alwaysUpdate);
    if (mode !== 'live') r.mode = await P(`return await p.mode('${mode}');`);
    if (mode !== 'preview') r.caret = await placeCaret(line, ch);
    const noteBefore = bytes('Note.md'); const linkerBefore = bytes('Linker.md');
    r.ran = await runCmd('editor:rename-heading'); await sleep(600);
    r.modal = await modal();
    r.activeAtOpen = await P(`return p.active();`);
    if (!r.modal || name === null) { await cleanup(); r.note = bytes('Note.md'); r.linker = bytes('Linker.md'); r.noteUnchanged = r.note === noteBefore; return; }
    if (name !== undefined) await replaceTyped(name);
    r.typed = await modal();
    // A rawKeyDown Enter (no keypress) first; the textarea commits on keypress.
    await c.pressKey('Enter'); await sleep(400);
    r.afterRawEnter = (await modal()) ? 'modal still open' : 'closed';
    if (r.afterRawEnter !== 'closed') {
      if (commit === 'click') { const hit = await c.clickElement('.modal.mod-file-rename button.mod-cta'); r.committedBy = hit ? 'Save button' : 'no Save button'; }
      else { await enterWithKeypress(); r.committedBy = 'keyDown Enter (with keypress)'; }
    } else r.committedBy = 'rawKeyDown Enter';
    await sleep(700);
    r.afterEnter = await modal();
    if (r.afterEnter && answer) {
      // A follow-up modal (alwaysUpdateLinks off): press the button whose text matches.
      const pt = await ev(`const bs = Array.from(document.querySelectorAll('.modal-container:last-child button')); const b = bs.find(x => ${answer}.test(x.textContent)); if (!b) return null; const r = b.getBoundingClientRect(); return { x: r.x + r.width/2, y: r.y + r.height/2, text: b.textContent };`);
      r.answered = pt;
      if (pt) { await c.clickAt(pt.x, pt.y); await sleep(700); }
      r.afterAnswer = await modal();
    }
    r.noteDisk = await pollChange('Note.md', noteBefore, 5000);
    r.linkerDisk = await pollChange('Linker.md', linkerBefore, 3000);
    r.editorAfter = await P(`const v = p.leaf().view; return v.getMode() === 'preview' ? null : { cursor: v.editor.getCursor(), value: v.editor.getValue() };`);
    r.activeAfter = await P(`return p.active();`);
    await cleanup();
  };

  await step('h01-availability', async (r) => {
    r.seed = await seedBoth(true);
    const cases = { liveOffHeading: ['live', 4, 2], liveOnHeading: ['live', 2, 4], liveOnSetextText: ['live', 6, 3], liveOnSetextUnderline: ['live', 7, 2], sourceOnHeading: ['source', 2, 4], sourceOffHeading: ['source', 4, 2], previewAny: ['preview', 0, 0] };
    for (const [k, [mode, line, ch]] of Object.entries(cases)) {
      await P(`return await p.mode('${mode}');`);
      if (mode !== 'preview') await placeCaret(line, ch);
      const check = await P(`
        const cmd = app.commands.commands['editor:rename-heading']; const v = p.leaf().view;
        let viaCheck = null; try { if (cmd.editorCheckCallback && v.editor) viaCheck = cmd.editorCheckCallback(true, v.editor, v); else if (cmd.checkCallback) viaCheck = cmd.checkCallback(true); } catch (e) { viaCheck = 'ERR ' + e; }
        return { viaCheck, mode: v.getMode() };`);
      const items = await palette('rename this heading');
      r[k] = { ...check, palette: items };
    }
    r.cmd = await P(`return p.cmd('editor:rename-heading');`);
    return r;
  });

  await step('h02-modal-and-plain-rename', async (r) => { await renameCase(r, { name: 'New' }); return r; });
  await step('h03-modal-escape', async (r) => { await renameCase(r, { name: null }); return r; });
  await step('h04-setext', async (r) => { await renameCase(r, { line: 6, ch: 3, name: 'Setext New' }); return r; });
  await step('h05-source-mode', async (r) => { await renameCase(r, { mode: 'source', name: 'Src New' }); return r; });
  await step('h05b-save-button', async (r) => { await renameCase(r, { name: 'Clicked', commit: 'click' }); return r; });
  await step('h05c-multiline-paste', async (r) => {
    // A newline in the name: Shift+Enter in the textarea.
    r.seed = await seedBoth(true); r.caret = await placeCaret(2, 4);
    await runCmd('editor:rename-heading'); await sleep(600);
    await replaceTyped('Line'); await c.pressKey('Enter', { shift: true }); await sleep(100); await c.typeText('Two', 25); await sleep(200);
    r.typed = await modal();
    const nb = bytes('Note.md');
    await enterWithKeypress(); await sleep(700);
    r.afterEnter = await modal();
    r.noteDisk = await pollChange('Note.md', nb, 4000);
    await cleanup();
    return r;
  });
  await step('h06-unchanged', async (r) => { await renameCase(r, { name: 'Old' }); return r; });
  await step('h07-empty', async (r) => { await renameCase(r, { name: '' }); return r; });
  const odd = { hash: 'Ha#sh', pipe: 'Pi|pe', open: 'Br[ack', close: 'Br]ack', caret: 'Ca^ret', space: 'Two Words' };
  for (const [k, v] of Object.entries(odd)) await step('h08-odd-' + k, async (r) => { await renameCase(r, { name: v }); return r; });
  await step('h09-noupdate-prompt-update', async (r) => { await renameCase(r, { name: 'New', alwaysUpdate: false, answer: '/^Update/' }); r.appJson = bytes('.obsidian/app.json'); return r; });
  await step('h10-noupdate-prompt-dont', async (r) => { await renameCase(r, { name: 'New', alwaysUpdate: false, answer: "/^Don|^Do not|^No/" }); r.appJson = bytes('.obsidian/app.json'); return r; });
  await step('h11-noupdate-prompt-escape', async (r) => { await renameCase(r, { name: 'New', alwaysUpdate: false }); r.appJson = bytes('.obsidian/app.json'); return r; });
}

// ===========================================================================
if (SCEN === 'al') {
  const SHAPES = {
    none: 'Body only.\n',
    empty: '---\naliases: []\n---\nBody.\n',
    list: '---\naliases:\n  - one\n  - two\n---\nBody.\n',
    scalar: '---\naliases: foo\n---\nBody.\n',
    around: '---\ntitle: T\naliases:\n  - one\ntags: x\n---\nBody.\n',
    otherKeys: '---\ntitle: T\ntags: x\n---\nBody.\n',
  };
  const propsDom = () => P(`const v = p.leaf().view; const m = v.containerEl.querySelector('.metadata-container'); return m ? { cls: m.className, text: m.innerText.slice(0, 400), html: m.outerHTML.slice(0, 5000) } : null;`);
  const aliasCase = async (r, content, mode, value) => {
    await P(`return await p.open('Alias.md', 'live');`);
    r.seedOk = await P(`return await p.seed('Alias.md', ${JSON.stringify(content)}, 1200);`);
    if (mode !== 'live') r.mode = await P(`return await p.mode('${mode}');`);
    if (mode !== 'preview') await P(`const e = p.ed(); e.focus(); e.setCursor({ line: e.lineCount() - 1, ch: 0 }); await p.sleep(150); return true;`);
    await sleep(2500);
    const before = bytes('Alias.md');
    r.diskBefore = before;
    r.ran = await runCmd('markdown:add-alias'); await sleep(300);
    r.activeAfterRun = await P(`return p.active();`);
    r.focusWhere = await P(`const a = document.activeElement; const b = a.getBoundingClientRect(); const v = p.leaf().view;
      return { rect: [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)], visible: !!(a.offsetWidth || a.offsetHeight),
        inSource: !!a.closest('.markdown-source-view'), inReading: !!a.closest('.markdown-reading-view'), inCm: !!a.closest('.cm-editor'), inModal: !!a.closest('.modal'), inLeaf: (a.closest('.workspace-leaf-content') || { getAttribute: () => null }).getAttribute('data-type'), rightCollapsed: app.workspace.rightSplit.collapsed, fileProps: app.workspace.getLeavesOfType('file-properties').map(l => l.getViewState()),
        sourceState: v.getState().source, mode: v.getMode(), viewMode: v.getState().mode, sourceViewCls: (v.containerEl.querySelector('.markdown-source-view') || {}).className };`);
    r.diskAfterRun = await pollChange('Alias.md', before, 3500);
    r.editorAfterRun = await P(`const v = p.leaf().view; return { mode: v.getMode(), value: v.getMode() === 'preview' ? v.data : v.editor.getValue() };`);
    r.propsAfterRun = await propsDom();
    r.modal = await modal();
    if (value !== null) {
      await c.typeText(value, 30); await sleep(300);
      r.activeAfterType = await P(`return p.active();`);
      const mid = bytes('Alias.md');
      await c.pressKey('Enter'); await sleep(300);
      r.activeAfterEnter = await P(`return p.active();`);
      r.disk = await pollChange('Alias.md', mid, 4000);
      r.committedBy = r.disk.changed ? 'rawKeyDown Enter' : null;
      if (!r.disk.changed) {
        r.propsAfterRawEnter = await propsDom();
        await enterWithKeypress(); await sleep(300);
        r.activeAfterKeypressEnter = await P(`return p.active();`);
        r.disk = await pollChange('Alias.md', mid, 4000);
        if (r.disk.changed) r.committedBy = 'keyDown Enter (with keypress)';
      }
      r.propsAfterEnter = await propsDom();
      r.editorAfter = await P(`const v = p.leaf().view; return v.getMode() === 'preview' ? null : { cursor: v.editor.getCursor(), value: v.editor.getValue() };`);
      await c.pressKey('Escape'); await sleep(300);
      r.activeAfterEscape = await P(`return p.active();`);
      r.diskFinal = await pollChange('Alias.md', r.disk.bytes, 3000);
    }
  };
  await step('a00-cmd', async (r) => { r.cmd = await P(`return p.cmd('markdown:add-alias');`); return r; });
  for (const [k, content] of Object.entries(SHAPES)) await step('a01-live-' + k, async (r) => { await aliasCase(r, content, 'live', 'NewAlias'); return r; });
  await step('a02-source-none', async (r) => { await aliasCase(r, SHAPES.none, 'source', 'NewAlias'); return r; });
  await step('a02-source-list', async (r) => { await aliasCase(r, SHAPES.list, 'source', 'NewAlias'); return r; });
  await step('a03-preview-none', async (r) => { await aliasCase(r, SHAPES.none, 'preview', 'NewAlias'); return r; });
  await step('a03-preview-list', async (r) => { await aliasCase(r, SHAPES.list, 'preview', 'NewAlias'); return r; });
  await step('a04-live-none-nothing-typed', async (r) => { await aliasCase(r, SHAPES.none, 'live', null); await sleep(500); await c.pressKey('Escape'); await sleep(400); r.afterEscape = await pollChange('Alias.md', r.diskAfterRun.bytes, 3000); r.activeAfterEscape = await P(`return p.active();`); return r; });
  await step('a05-palette', async (r) => {
    await P(`return await p.open('Alias.md', 'live');`);
    r.live = await palette('add alias');
    await P(`return await p.mode('source');`); r.source = await palette('add alias');
    await P(`return await p.mode('preview');`); r.preview = await palette('add alias');
    return r;
  });
}

save();
console.log(JSON.stringify({ scen: SCEN, steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
