// The Note composer, Unique note creator and Random note commands, which
// [[Development#The unrunnable commands, 2026-11-09]] left waiting on a probe:
//
//   note-composer:merge-file      the merged bytes under Enter / Shift+Enter, the
//                                 separator, two frontmatters, the links to the
//                                 source, where the source goes per trashOption,
//                                 the confirmation, Ctrl+Enter's new note
//   note-composer:split-file      the prompt, both notes' bytes per
//   note-composer:extract-heading replacementText and link format, the folder,
//                                 the heading line, the default name, templates
//   zk-prefixer, insert-unique-link   the settings keys, the created path and
//                                 bytes, two runs in one minute, folder and
//                                 template, the inserted text
//   random-note                   current tab or new, the open note redrawn?,
//                                 the lone-note notice, the ribbon button
//
// Bytes are read from disk by the node side after the save debounce, never
// from the editor. Modals are driven with real CDP keys. Each scenario needs
// its own vault, so SCEN picks the fixtures and the steps:
//
//   SCEN=settings  the two settings tabs, each row set and the file diffed
//   SCEN=merge     merges (default)
//   SCEN=split     split-file and extract-heading
//   SCEN=zk        the unique note creator
//   SCEN=random2   random-note in a two-note vault
//   SCEN=random1   random-note in a one-note vault
//   SCEN=random0   random-note with no Markdown note at all
//
// ONLY=<regex> runs a subset of a scenario's steps (fixtures always run whole).
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SCEN = process.env.SCEN || 'merge';
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 540000;
const vp = (rel) => path.join(VAULT, rel);
const disk = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
/** Every file under the vault (not .obsidian), path -> bytes. */
const tree = () => {
  const o = {};
  const walk = (d) => {
    for (const e of fs.readdirSync(path.join(VAULT, d), { withFileTypes: true })) {
      const rel = d ? d + '/' + e.name : e.name;
      if (rel === '.obsidian') continue;
      if (e.isDirectory()) { o[rel + '/'] = '<dir>'; walk(rel); } else o[rel] = fs.readFileSync(path.join(VAULT, rel), 'utf8');
    }
  };
  walk('');
  return o;
};
const cfgFiles = () => {
  const o = {};
  for (const n of fs.readdirSync(vp('.obsidian'))) if (n.endsWith('.json') && n !== 'workspace.json') o[n] = disk('.obsidian/' + n);
  return o;
};
const diffTrees = (a, b) => {
  const d = {};
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) if (a[k] !== b[k]) d[k] = { before: a[k] === undefined ? null : a[k], after: b[k] === undefined ? null : b[k] };
  return d;
};

const write = (rel, s) => { fs.mkdirSync(path.dirname(vp(rel)), { recursive: true }); fs.writeFileSync(vp(rel), s); };
const CORE = { 'file-explorer': true, 'command-palette': true, switcher: true, canvas: true, 'note-composer': true, 'zk-prefixer': true, 'random-note': true, 'page-preview': false };

// A system-trash merge puts the source in the person's real trash: give it a
// name of its own so it can be found there and nowhere else.
const SYS_TAG = 'nc-probe-' + (process.env.SYS_TAG || 'x');

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  write('.obsidian/app.json', '{}');
  write('.obsidian/core-plugins.json', JSON.stringify(CORE, null, 2));
  if (SCEN === 'settings') {
    write('Note.md', 'x\n');
    write('Templates/Composer.md', '{{fromTitle}}|{{newTitle}}|{{content}}\n');
    write('Templates/Unique.md', '# {{title}}\n');
  } else if (SCEN === 'merge') {
    // Trailing-newline matrix: s = source ends in \n, t = target ends in \n.
    for (const [tag, s, t] of [['yy', 1, 1], ['yn', 1, 0], ['ny', 0, 1], ['nn', 0, 0]]) {
      for (const dir of ['app', 'pre']) {
        write(`m-${dir}-${tag}-src.md`, 'src line 1\nsrc line 2' + (s ? '\n' : ''));
        write(`m-${dir}-${tag}-dst.md`, 'dst line 1\ndst line 2' + (t ? '\n' : ''));
      }
    }
    // Frontmatter: distinct keys and clashing keys, each way.
    for (const dir of ['app', 'pre']) {
      write(`f-${dir}-distinct-src.md`, '---\nsrckey: s\n---\nsrc body\n');
      write(`f-${dir}-distinct-dst.md`, '---\ndstkey: d\n---\ndst body\n');
      write(`f-${dir}-clash-src.md`, '---\ntitle: from src\ntags:\n  - s1\n  - shared\naliases: [sa]\nonly: s\n---\nsrc body\n');
      write(`f-${dir}-clash-dst.md`, '---\ntitle: from dst\ntags:\n  - d1\n  - shared\naliases: [da]\n---\ndst body\n');
      write(`f-${dir}-srconly-src.md`, '---\nsrckey: s\n---\nsrc body\n');
      write(`f-${dir}-srconly-dst.md`, 'dst body\n');
    }
    // Links into the source, from a third note and from the source and target themselves.
    write('l-src.md', '# H\nsrc body ^blk\n\nself [[l-src#H]]\n');
    write('l-dst.md', 'dst body links [[l-src]]\n');
    write('l-links.md', [
      'wiki [[l-src]]', 'heading [[l-src#H]]', 'alias [[l-src|x]]', 'block [[l-src#^blk]]', 'embed ![[l-src]]',
      'md [x](l-src.md)', 'md heading [x](l-src.md#H)', 'md embed ![x](l-src.md)', 'path [[l-src.md]]', '',
    ].join('\n'));
    // Where the source goes, per trashOption.
    write('t-local-src.md', 'local\n'); write('t-local-dst.md', 'dst\n');
    write('t-none-src.md', 'none\n'); write('t-none-dst.md', 'dst\n');
    write(`${SYS_TAG}-src.md`, 'system\n'); write('t-system-dst.md', 'dst\n');
    // The confirmation, cancelled and accepted.
    write('c-src.md', 'confirm src\n'); write('c-dst.md', 'confirm dst\n');
    // Ctrl+Enter: a target that does not exist.
    write('n-src.md', 'new src\n');
    write('Sub/n-sub-src.md', 'sub src\n');
    // A merge with a template set.
    write('Templates/Composer.md', 'T:{{fromTitle}}|{{newTitle}}|{{date:YYYY}}|{{content}}|END\n');
    write('Templates/NoContent.md', 'T:{{fromTitle}}|{{newTitle}}|END\n');
    write('tpl-src.md', 'tpl src\n'); write('tpl-dst.md', 'tpl dst\n');
    write('tpl2-src.md', 'tpl2 src\n'); write('tpl2-dst.md', 'tpl2 dst\n');
    write('Canvas.canvas', '{"nodes":[],"edges":[]}');
    write('anchor.md', 'anchor\n');
    write('w-app-src.md', '\n\nsrc after two blank lines\n\n'); write('w-app-dst.md', 'dst with trailing blank lines\n\n\n');
    write('w-pre-src.md', '\n\nsrc after two blank lines\n\n'); write('w-pre-dst.md', '\n\ndst after two blank lines\n');
    write('w-crlf-src.md', 'crlf src 1\r\ncrlf src 2\r\n'); write('w-crlf-dst.md', 'crlf dst 1\r\ncrlf dst 2\r\n');
    write('a-src.md', 'always src\n'); write('a-dst.md', 'always dst\n'); write('a-links.md', 'see [[a-src]]\n');
    for (const k of ['k3a', 'k3b', 'k3c', 'k3e']) write(k + '-src.md', k + ' src\n');
    write('Sub/k3d-src.md', 'k3d src\n'); write('k3-existing.md', 'existing\n');
    write('k-src.md', 'ctrl src\n'); write('k2-src.md', 'ctrl2 src\n');
    write('h-src.md', 'h src\n'); write('h-dst.md', 'h dst\n');
    write('e-src.md', 'e src\n'); write('e-dst.md', 'e dst\n');
    write('d-src.md', 'dont ask src\n'); write('d-dst.md', 'dont ask dst\n');
    write('u-src.md', 'no update src\n'); write('u-dst.md', 'dst\n'); write('u-links.md', 'see [[u-src]] and [x](u-src.md)\n');
    write('Sub/sub-src.md', 'sub src\n'); write('Sub/sub-links.md', 'see [[sub-src]] and [[Sub/sub-src]]\n'); write('Other/sub-dst.md', 'other dst\n');
  } else if (SCEN === 'split') {
    for (const k of ['link-wiki', 'embed-wiki', 'none-wiki', 'link-md', 'embed-md']) write(`s-${k}.md`, 'before\nPICK ME\nafter\n');
    write('Sub/s-folder.md', 'before\nPICK ME\nafter\n');
    write('s-existing.md', 'before\nPICK ME\nafter\n');
    write('s-existing-target.md', 'existing target\n');
    write('s-multiline.md', 'before\nline one\nline two\nafter\n');
    write('h-top.md', '# Zulu\ntop body\n## Yankee\nmid body\n### Xray\ndeep body\n## Whiskey\nnext body\n');
    write('h-mid.md', '# Zulu\ntop body\n## Yankee\nmid body\n### Xray\ndeep body\n## Whiskey\nnext body\n');
    write('h-last.md', '# Zulu\ntop body\n## Victor\nlast body\n\n\n');
    write('h-body.md', 'intro\n## Uniform\nsec body\n\n## Tango\nafter body\n');
    write('h-none.md', 'no heading here\n');
    write('h-eof.md', 'pre\n## Oscar\noscar body');
    write('h-tpl.md', 'pre\n## Mike\nmike body\n## Lima\n');
    write('h-odd.md', 'pre\n## Sierra/Romeo: Quebec? *em* [[Link]]\nbody\n');
    write('h-new.md', '# Zulu\ntop body\n## Papa\npapa body\n');
    write('Inbox/keep.md', 'keep\n');
    write('Sub/s-sub-md.md', 'before\nPICK ME\nafter\n');
    write('tpl-split.md', 'before\nPICK ME\nafter\n');
    write('tpl-split2.md', 'before\nPICK ME\nafter\n');
    write('Templates/Composer.md', 'T:{{fromTitle}}|{{newTitle}}|{{date:YYYY}}|{{content}}|END\n');
    write('Templates/NoContent.md', 'T:{{fromTitle}}|{{newTitle}}|END\n');
    write('Canvas.canvas', '{"nodes":[],"edges":[]}');
  } else if (SCEN === 'zk') {
    write('Home.md', 'home\n');
    write('Link.md', 'insert here: \n');
    write('Templates/Unique.md', 'title={{title}} date={{date}} time={{time}} d2={{date:YYYY}}\n');
    write('Zettel/keep.md', 'keep\n');
    write('Canvas.canvas', '{"nodes":[],"edges":[]}');
  } else if (SCEN === 'random2') {
    write('One.md', 'one\n'); write('Two.md', 'two\n');
  } else if (SCEN === 'random1') {
    write('Only.md', 'only\n');
  } else if (SCEN === 'random0') {
    write('Canvas.canvas', '{"nodes":[],"edges":[]}');
  }
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
const out = all[SCEN + (ONLY ? ':' + process.env.ONLY : '')] = { startedAt: new Date().toISOString(), steps: {}, order: [] };
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const c = await connect();
const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error('step timeout ' + ms + 'ms: ' + what)), ms))]);

await c.evaluate(`
window.__p = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  notices: [],
  watch() {
    if (this.obs) return;
    this.obs = new MutationObserver((ms) => { for (const m of ms) for (const n of m.addedNodes) {
      if (n.nodeType === 1 && (n.classList.contains('notice') || n.querySelector && n.querySelector('.notice'))) {
        const el = n.classList.contains('notice') ? n : n.querySelector('.notice');
        this.notices.push({ t: Date.now(), text: el.textContent, html: el.innerHTML.slice(0, 400) });
      } } });
    this.obs.observe(document.body, { childList: true, subtree: true });
  },
  takeNotices() { const n = this.notices; this.notices = []; return n; },
  mdLeaf() { const ls = app.workspace.getLeavesOfType('markdown'); return ls.length ? ls[0] : null; },
  async open(file, mode) {
    let f = null;
    for (let i = 0; i < 30 && !f; i++) { f = app.vault.getAbstractFileByPath(file); if (!f) await this.sleep(200); }
    if (!f) throw new Error('no file ' + file);
    // One leaf throughout: reuse the active one (the poison trap).
    let leaf = app.workspace.activeLeaf;
    if (!leaf || !leaf.view || leaf.view.getViewType() === 'empty' && false) leaf = app.workspace.getLeaf(false);
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(350);
    if (mode && leaf.view.getViewType() === 'markdown') {
      const st = leaf.getViewState();
      st.state = Object.assign({}, st.state, mode === 'preview' ? { mode: 'preview' } : { mode: 'source', source: false });
      await leaf.setViewState(st);
      await this.sleep(250);
    }
    return this.where();
  },
  where() {
    const leaves = [];
    app.workspace.iterateRootLeaves((l) => leaves.push({ type: l.view.getViewType(), file: l.view.file ? l.view.file.path : null, active: l === app.workspace.activeLeaf, mode: l.view.getMode ? l.view.getMode() : null }));
    const a = app.workspace.activeLeaf;
    const h = a && a.history;
    // iterateRootLeaves missed a freshly opened tab twice; count every way.
    const all = [];
    for (const t of ['markdown', 'empty', 'canvas']) for (const l of app.workspace.getLeavesOfType(t)) all.push({ type: t, file: l.view.file ? l.view.file.path : null, active: l === a, root: l.getRoot() === app.workspace.rootSplit });
    const tabHeaders = Array.from(document.querySelectorAll('.workspace-split.mod-root .workspace-tab-header')).map((x) => ({ title: x.getAttribute('aria-label') || x.textContent.trim(), active: x.classList.contains('is-active') }));
    return { leaves, allLeaves: all, tabHeaders, active: a && a.view.file ? a.view.file.path : null, activeType: a ? a.view.getViewType() : null,
      back: h && h.backHistory ? h.backHistory.map((x) => x.title || (x.state && x.state.state && x.state.state.file) || null) : null,
      forward: h && h.forwardHistory ? h.forwardHistory.map((x) => x.title || (x.state && x.state.state && x.state.state.file) || null) : null };
  },
  ed() { const v = app.workspace.activeLeaf.view; return v && v.editor; },
  caret(spec) {
    const ed = this.ed(); ed.focus();
    if (spec.sel) ed.setSelection(spec.sel[0], spec.sel[1]); else ed.setCursor(spec.cursor || { line: 0, ch: 0 });
    return { value: ed.getValue(), sel: ed.getSelection(), from: ed.getCursor('from'), to: ed.getCursor('to') };
  },
  describe(el) { return el ? { tag: el.tagName, cls: String(el.className), text: (el.textContent || '').trim().slice(0, 200) } : null; },
  modals() {
    return Array.from(document.querySelectorAll('.modal-container')).map((m) => {
      const root = m.querySelector('.modal') || m;
      const q = (s) => Array.from(root.querySelectorAll(s));
      return {
        cls: String(root.className),
        title: (root.querySelector('.modal-title') || {}).textContent || null,
        content: (root.querySelector('.modal-content') || {}).textContent || null,
        contentHtml: ((root.querySelector('.modal-content') || {}).innerHTML || '').slice(0, 1500),
        inputs: q('input, textarea').map((i) => ({ cls: String(i.className), placeholder: i.placeholder, value: i.value, type: i.type })),
        buttons: q('button').map((b) => ({ text: b.textContent.trim(), cls: String(b.className), focused: document.activeElement === b, aria: b.getAttribute('aria-label') })),
        checkboxes: q('input[type=checkbox], .checkbox-container').map((x) => ({ cls: String(x.className), checked: x.checked, label: (x.closest('label') || x.parentElement || {}).textContent })),
        items: q('.suggestion-item').map((x) => ({ cls: String(x.className), text: x.textContent.trim().slice(0, 200), html: x.innerHTML.slice(0, 400) })),
        empty: (root.querySelector('.suggestion-empty') || {}).textContent || null,
        instructions: q('.prompt-instruction').map((x) => ({ command: (x.querySelector('.prompt-instruction-command') || {}).textContent, text: x.textContent.trim(), html: x.innerHTML })),
        promptHtml: (root.querySelector('.prompt') ? root.outerHTML : '').slice(0, 3000),
      };
    });
  },
  active() { return this.describe(document.activeElement); },
  closeModals() { let n = 0; for (const m of Array.from(document.querySelectorAll('.modal-container'))) { const b = m.querySelector('.modal-close-button'); if (b) b.click(); else m.remove(); n++; } return n; },
  cmd(id) { const x = app.commands.commands[id]; if (!x) return null;
    return { name: x.name, icon: x.icon || null, callback: !!x.callback, checkCallback: !!x.checkCallback, editorCallback: !!x.editorCallback, editorCheckCallback: !!x.editorCheckCallback,
      hotkeys: x.hotkeys || null, custom: app.hotkeyManager.getHotkeys ? app.hotkeyManager.getHotkeys(id) || null : null, defaults: app.hotkeyManager.getDefaultHotkeys ? app.hotkeyManager.getDefaultHotkeys(id) || null : null }; },
  plugin(id) { const p = app.internalPlugins.plugins[id]; return p ? { enabled: p.enabled, options: p.instance && p.instance.options ? JSON.parse(JSON.stringify(p.instance.options)) : null } : null; },
};
window.__p.watch();
return true;`);

const P = (expr) => c.evaluate(`return await (async () => { const p = window.__p; ${expr} })();`);
const modals = () => P(`return p.modals();`);
const setOpt = (k, v) => P(`const i = app.internalPlugins.plugins['note-composer'].instance; i.options[${JSON.stringify(k)}] = ${JSON.stringify(v)}; return JSON.parse(JSON.stringify(i.options));`);
const setApp = (k, v) => P(`app.vault.setConfig(${JSON.stringify(k)}, ${JSON.stringify(v)}); return app.vault.getConfig(${JSON.stringify(k)});`);
const closeModals = async () => { const n = await P(`return p.closeModals();`); await sleep(250); return n; };

async function step(name, fn, ms = 90000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  out.steps[name] = { ok: null, partial };
  try { await withTimeout(fn(partial), ms, name); out.steps[name] = { ok: true, ms: Date.now() - t, ...partial }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...partial }; }
  try { const left = await closeModals(); if (left) out.steps[name].modalsLeftClosed = left; } catch { /* ignore */ }
  try { await c.evaluate(`document.querySelectorAll('.menu').forEach((m) => m.remove()); return true;`); } catch { /* ignore */ }
  save();
  return out.steps[name];
}

/** Type into the open prompt's input the way a person does: clear it, then real keys. */
async function typePrompt(text) {
  await P(`const i = document.querySelector('.modal-container .prompt-input, .modal-container input'); if (i) { i.focus(); i.select(); } return !!i;`);
  await c.pressKey('Backspace');
  await c.typeText(text, 25);
  await sleep(400);
}
const selectedItem = () => P(`const s = document.querySelector('.modal-container .suggestion-item.is-selected'); return s ? s.textContent.trim() : null;`);

// ---------------------------------------------------------------------------
await step('s00-context', async (r) => {
  r.vaultPath = await c.vaultPath();
  r.version = await c.evaluate(`return { apiVersion: String(app.apiVersion), ua: navigator.userAgent, title: document.title, electron: process.versions.electron };`);
  r.commands = await P(`const ids = ['note-composer:merge-file', 'note-composer:split-file', 'note-composer:extract-heading', 'zk-prefixer', 'insert-unique-link', 'insert-unique-link', 'random-note', 'random-note:open'];
    const o = {}; for (const id of ids) o[id] = p.cmd(id); return o;`);
  r.matchingIds = await P(`return Object.keys(app.commands.commands).filter((k) => /note-composer|zk|unique|random/i.test(k)).map((k) => [k, app.commands.commands[k].name]);`);
  r.plugins = await P(`return { 'note-composer': p.plugin('note-composer'), 'zk-prefixer': p.plugin('zk-prefixer'), 'random-note': p.plugin('random-note') };`);
  r.config = { app: disk('.obsidian/app.json'), files: cfgFiles() };
  r.ribbon = await P(`return Array.from(document.querySelectorAll('.workspace-ribbon .side-dock-ribbon-action, .side-dock-actions > *')).map((b, i) => ({ i, aria: b.getAttribute('aria-label'), cls: String(b.className), svg: b.querySelector('svg') ? b.querySelector('svg').getAttribute('class') : null, parent: String(b.parentElement.className), hidden: b.offsetParent === null }));`);
  r.ribbonHtml = await P(`return Array.from(document.querySelectorAll('.workspace-ribbon')).map((x) => ({ cls: String(x.className), html: x.outerHTML.slice(0, 6000) }));`);
  r.ribbonConfig = await P(`return app.workspace.leftRibbon && app.workspace.leftRibbon.items ? app.workspace.leftRibbon.items.map((x) => ({ id: x.id, title: x.title, icon: x.icon, hidden: x.hidden })) : null;`);
});

/** Open the palette, type `query`, read the rows it lists, close it. */
async function palette(query) {
  await P(`app.commands.executeCommandById('command-palette:open'); return true;`);
  await sleep(400);
  await c.typeText(query, 20);
  await sleep(400);
  const m = await modals();
  await c.pressKey('Escape'); await sleep(250);
  return m.length ? m[m.length - 1].items.map((x) => x.text) : null;
}

/** Which of our commands the palette offers, in the current state. */
async function availability(r, label) {
  r[label] = { where: await P(`return p.where();`), composer: await palette('Note composer'), unique: await palette('Unique note'), random: await palette('random note') };
}

// ===========================================================================
if (SCEN === 'settings') {
  const ROWS = `
    const el = app.setting.activeTab.containerEl;
    return Array.from(el.querySelectorAll('.setting-item')).map((it) => ({
      name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(),
      descriptionText: ((it.querySelector('.setting-item-description') || {}).textContent || '').trim(),
      descriptionHtml: ((it.querySelector('.setting-item-description') || {}).innerHTML || '').trim(),
      controlHtml: ((it.querySelector('.setting-item-control') || {}).innerHTML || '').slice(0, 800),
      hidden: it.offsetParent === null,
    }));`;
  const openTab = (id) => c.evaluate(`
    app.setting.open(); app.setting.openTabById(${JSON.stringify(id)});
    await new Promise((r) => setTimeout(r, 1200));
    return { id: app.setting.activeTab.id, name: app.setting.activeTab.name };`);
  const rowEval = (name, body) => c.evaluate(`
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const el = app.setting.activeTab.containerEl;
    const row = Array.from(el.querySelectorAll('.setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === ${JSON.stringify(name)});
    if (!row) return { noRow: true };
    ${body}`);
  const typeInto = (name, value) => rowEval(name, `
    const f = row.querySelector('input[type=text], input:not([type])'); if (!f) return { noField: true };
    f.focus(); f.value = ${JSON.stringify(value)};
    f.dispatchEvent(new Event('input', { bubbles: true })); f.dispatchEvent(new Event('change', { bubbles: true })); f.blur();
    await sleep(1500);
    return { value: f.value, description: ((row.querySelector('.setting-item-description') || {}).textContent || '').trim() };`);
  const selectRow = (name, value) => rowEval(name, `
    const s = row.querySelector('select'); if (!s) return { noSelect: true };
    s.value = ${JSON.stringify(value)}; s.dispatchEvent(new Event('change', { bubbles: true }));
    await sleep(1500); return { value: s.value };`);
  const toggleRow = (name) => rowEval(name, `
    const t = row.querySelector('.checkbox-container'); if (!t) return { noToggle: true };
    t.click(); await sleep(1500); return { on: t.classList.contains('is-enabled') };`);
  /** The file picker: click it for real, then see what opens. */
  const pickFile = async (name, query) => {
    const r = {};
    r.button = await rowEval(name, `
      const b = row.querySelector('.combobox-button, button, .setting-item-control > *'); if (!b) return { noButton: true };
      b.scrollIntoView({ block: 'center' }); await sleep(200);
      const x = b.getBoundingClientRect(); return { cls: String(b.className), text: b.textContent, x: x.x + x.width / 2, y: x.y + x.height / 2, w: x.width };`);
    if (!r.button || r.button.noRow || r.button.noButton) return r;
    // A real click at the button's rect lands on the empty tab behind the
    // Settings modal (elementFromPoint says `.empty-state`) in this harness, so
    // the button is clicked through its own click(), recorded as such.
    r.hit = await c.evaluate(`const e = document.elementFromPoint(${r.button.x}, ${r.button.y}); return e ? { tag: e.tagName, cls: String(e.className), win: [innerWidth, innerHeight] } : null;`);
    await c.evaluate(`document.querySelectorAll('.suggestion-container.combobox').forEach((x) => x.remove()); return true;`);
    // What opened it on the first run: a real click (which hit the empty tab), then click().
    await c.clickAt(r.button.x, r.button.y); await sleep(700);
    r.openAfterRealClick = await c.evaluate(`return document.querySelectorAll('.suggestion-container.combobox').length;`);
    for (let i = 0; i < 3; i++) {
      if (await c.evaluate(`return document.querySelectorAll('.suggestion-container.combobox').length;`)) break;
      await rowEval(name, `row.querySelector('.combobox-button').click(); return true;`);
      await sleep(900);
      r.jsClicks = i + 1;
    }
    for (const k of ['Enter', 'Space', 'ArrowDown']) {
      if (await c.evaluate(`return document.querySelectorAll('.suggestion-container.combobox').length;`)) break;
      await rowEval(name, `row.querySelector('.combobox-button').focus(); return true;`);
      await c.pressKey(k); await sleep(900);
      r.keyTried = (r.keyTried || []).concat(k);
    }
    r.opened = await c.evaluate(`return Array.from(document.querySelectorAll('.suggestion-container.combobox')).map((x) => ({ cls: String(x.className), html: x.outerHTML.slice(0, 2500), placeholder: (x.querySelector('input') || {}).placeholder, items: Array.from(x.querySelectorAll('.suggestion-item')).map((i) => i.textContent.trim()) }));`);
    r.activeAfterClick = await P(`return p.active();`);
    return r;
  };

  for (const [id, file] of [['note-composer', 'note-composer.json'], ['zk-prefixer', 'zk-prefixer.json']]) {
    await step('s01-tab-' + id, async (r) => {
      r.fileBefore = disk('.obsidian/' + file);
      r.open = await openTab(id);
      r.rows = await c.evaluate(ROWS);
      r.optionsBefore = await P(`return p.plugin(${JSON.stringify(id)});`);
      save();
      r.actions = [];
      const rec = async (what, v) => { r.actions.push({ what, v, file: disk('.obsidian/' + file), options: (await P(`return p.plugin(${JSON.stringify(id)});`)).options }); save(); };
      if (id === 'note-composer') {
        for (const v of ['embed', 'none', 'link']) await rec('replacementText=' + v, await selectRow('Text after extraction', v));
        await rec('toggle confirm', await toggleRow('Confirm file merge'));
        await rec('toggle confirm', await toggleRow('Confirm file merge'));
        const pk = await pickFile('Template file location', 'Composer');
        r.picker = pk; save();
        // If a suggestion list opened, type the template's name and pick it with real keys.
        if (pk.opened && pk.opened.length) {
          await c.typeText('Composer', 25); await sleep(500);
          r.pickerTyped = await c.evaluate(`return Array.from(document.querySelectorAll('.suggestion-container .suggestion-item, .modal-container .suggestion-item')).map((i) => ({ text: i.textContent.trim(), sel: i.classList.contains('is-selected') }));`);
          await c.pressKey('Enter'); await sleep(1500);
          await rec('template picked', await rowEval('Template file location', `return { html: row.querySelector('.setting-item-control').innerHTML.slice(0, 800) };`));
          await rec('template cleared', await rowEval('Template file location', `const x = row.querySelector('.combobox-clear-button'); if (!x) return { noClear: true }; x.click(); await sleep(1500); return { html: row.querySelector('.setting-item-control').innerHTML.slice(0, 400) };`));
        }
      } else {
        await rec('folder', await typeInto('New file location', 'Zettel'));
        const pk = await pickFile('Template file location', 'Unique');
        r.picker = pk; save();
        if (pk.opened && pk.opened.length) {
          await c.typeText('Unique', 25); await sleep(500);
          r.pickerTyped = await c.evaluate(`return Array.from(document.querySelectorAll('.suggestion-container .suggestion-item, .modal-container .suggestion-item')).map((i) => ({ text: i.textContent.trim(), sel: i.classList.contains('is-selected') }));`);
          await c.pressKey('Enter'); await sleep(1500);
          await rec('template picked', await rowEval('Template file location', `return { html: row.querySelector('.setting-item-control').innerHTML.slice(0, 800) };`));
          await rec('template cleared', await rowEval('Template file location', `const x = row.querySelector('.combobox-clear-button'); if (!x) return { noClear: true }; x.click(); await sleep(1500); return { html: row.querySelector('.setting-item-control').innerHTML.slice(0, 400) };`));
        }
        await rec('format', await typeInto('Unique prefix format', 'YYYY-MM-DD-HHmmss'));
        await rec('format cleared', await typeInto('Unique prefix format', ''));
        await rec('folder cleared', await typeInto('New file location', ''));
      }
      r.rowsAfter = await c.evaluate(ROWS);
      r.fileAfter = disk('.obsidian/' + file);
      await c.evaluate(`app.setting.close(); return true;`);
      await sleep(400);
    }, 120000);
  }
  await step('s02-all-config', async (r) => { r.files = cfgFiles(); });
}

// ===========================================================================
/**
 * Run a merge from `src` (opened) into the row `query` picks, pressing `key`
 * with `mods`. A confirmation is recorded and answered with `answer`
 * ('cta' | 'cancel' | 'escape'). Returns everything a case needs.
 */
async function answerUpdateLinks(choice) {
  const m = await P(`const ms = Array.from(document.querySelectorAll('.modal')).filter((x) => /Update links/.test((x.querySelector('.modal-title') || {}).textContent || ''));
    if (!ms.length) return null;
    const x = ms[ms.length - 1];
    return { title: x.querySelector('.modal-title').textContent, content: (x.querySelector('.modal-content') || {}).innerHTML, buttons: Array.from(x.querySelectorAll('button')).map((b) => ({ text: b.textContent, cls: String(b.className), focused: document.activeElement === b })) };`);
  if (!m) return null;
  m.clicked = await P(`const ms = Array.from(document.querySelectorAll('.modal')).filter((x) => /Update links/.test((x.querySelector('.modal-title') || {}).textContent || ''));
    const b = Array.from(ms[ms.length - 1].querySelectorAll('button')).find((b) => b.textContent === ${JSON.stringify(choice)}); if (!b) return null; b.click(); return b.textContent;`);
  await sleep(1200);
  m.after = await modals();
  return m;
}

async function merge(r, src, query, mods = {}, { answer = 'cta', files = [], openMode = 'source', prep, updateLinks = 'Just once' } = {}) {
  const before = tree();
  // An unrelated note first, so the leaf's history says whether the merge
  // opened the target or merely went back.
  r.anchor = await P(`return await p.open('anchor.md', 'source');`);
  r.opened = await P(`return await p.open(${JSON.stringify(src)}, ${JSON.stringify(openMode)});`);
  if (prep) await prep();
  await P(`p.takeNotices(); return true;`);
  r.ran = await P(`return app.commands.executeCommandById('note-composer:merge-file');`);
  await sleep(600);
  r.prompt = await modals();
  save();
  await typePrompt(query);
  r.listed = (await modals()).map((m) => m.items.map((i) => i.text));
  r.selected = await selectedItem();
  await c.pressKey('Enter', mods);
  await sleep(900);
  r.afterKey = await modals();
  save();
  if (r.afterKey.length) {
    if (answer === 'tick') {
      const ticked = await P(`const cb = document.querySelector('.modal input[type=checkbox]'); if (!cb) return null; cb.click(); return cb.checked;`);
      await sleep(300);
      const clicked = await P(`const b = document.querySelector('.modal-container .mod-cta'); if (!b) return null; const t = b.textContent; b.click(); return t;`);
      r.answered = { with: 'tick', ticked, clicked };
    } else if (answer === 'cta') {
      const clicked = await P(`const b = document.querySelector('.modal-container .mod-cta'); if (!b) return null; const t = b.textContent; b.click(); return t;`);
      r.answered = { with: 'cta', clicked };
    } else if (answer === 'cancel') {
      const clicked = await P(`const bs = Array.from(document.querySelectorAll('.modal-container button')).filter((b) => !b.classList.contains('mod-cta')); if (!bs.length) return null; const t = bs[0].textContent; bs[0].click(); return t;`);
      r.answered = { with: 'cancel', clicked };
    } else { await c.pressKey('Escape'); r.answered = { with: 'escape' }; }
    await sleep(900);
    r.afterAnswer = await modals();
  }
  // Renaming away the source raises "Update links" when alwaysUpdateLinks is
  // unset; the merge waits on it (its Merge button stays mod-loading), and a
  // modal closed with its × leaves the composer hung for every later merge.
  r.linkPrompt = await answerUpdateLinks(updateLinks);
  await sleep(2600); // out-wait the save debounce
  r.notices = await P(`return p.takeNotices();`);
  r.where = await P(`return p.where();`);
  r.changed = diffTrees(before, tree());
  r.bytes = {}; for (const f of files) r.bytes[f] = disk(f);
  save();
}

if (SCEN === 'merge') {
  await step('s01-merge-prompt', async (r) => {
    r.opened = await P(`return await p.open('c-src.md', 'source');`);
    r.ran = await P(`return app.commands.executeCommandById('note-composer:merge-file');`);
    await sleep(700);
    r.prompt = await modals();
    r.activeElement = await P(`return p.active();`);
    await c.typeText('c-d', 25); await sleep(400);
    r.typed = await modals();
    await c.pressKey('Escape'); await sleep(300);
    r.afterEscape = await modals();
  });
  await step('s02-confirm-cancel', async (r) => {
    await merge(r, 'c-src.md', 'c-dst', {}, { answer: 'cancel', files: ['c-src.md', 'c-dst.md'] });
  });
  await step('s03-confirm-escape', async (r) => {
    await merge(r, 'c-src.md', 'c-dst', {}, { answer: 'escape', files: ['c-src.md', 'c-dst.md'] });
  });
  await step('s04-confirm-accept', async (r) => {
    await merge(r, 'c-src.md', 'c-dst', {}, { answer: 'cta', files: ['c-src.md', 'c-dst.md'] });
  });
  for (const tag of ['yy', 'yn', 'ny', 'nn']) {
    await step(`s05-append-${tag}`, async (r) => { await merge(r, `m-app-${tag}-src.md`, `m-app-${tag}-dst`, {}, { files: [`m-app-${tag}-src.md`, `m-app-${tag}-dst.md`] }); });
    await step(`s06-prepend-${tag}`, async (r) => { await merge(r, `m-pre-${tag}-src.md`, `m-pre-${tag}-dst`, { shift: true }, { files: [`m-pre-${tag}-src.md`, `m-pre-${tag}-dst.md`] }); });
  }
  for (const k of ['distinct', 'clash', 'srconly']) {
    await step(`s07-fm-append-${k}`, async (r) => { await merge(r, `f-app-${k}-src.md`, `f-app-${k}-dst`, {}, { files: [`f-app-${k}-dst.md`] }); });
    await step(`s07-fm-prepend-${k}`, async (r) => { await merge(r, `f-pre-${k}-src.md`, `f-pre-${k}-dst`, { shift: true }, { files: [`f-pre-${k}-dst.md`] }); });
  }
  await step('s08-links', async (r) => {
    r.linksBefore = disk('l-links.md');
    await merge(r, 'l-src.md', 'l-dst', {}, { files: ['l-links.md', 'l-dst.md', 'l-src.md'] });
    r.app = disk('.obsidian/app.json');
  });
  for (const opt of ['local', 'none', 'system']) {
    await step(`s09-trash-${opt}`, async (r) => {
      const src = opt === 'system' ? `${SYS_TAG}-src.md` : `t-${opt}-src.md`;
      const sysTrash = path.join(os.homedir(), '.local/share/Trash/files');
      const sysBefore = fs.existsSync(sysTrash) ? fs.readdirSync(sysTrash).filter((n) => n.startsWith(SYS_TAG)) : null;
      await merge(r, src, `t-${opt}-dst`, {}, { files: [src, `t-${opt}-dst.md`], prep: () => P(`app.vault.setConfig('trashOption', ${JSON.stringify(opt)}); return true;`) });
      r.trashOption = await P(`return app.vault.getConfig('trashOption');`);
      r.dotTrash = fs.existsSync(vp('.trash')) ? fs.readdirSync(vp('.trash')).map((n) => [n, disk('.trash/' + n)]) : null;
      const sysAfter = fs.existsSync(sysTrash) ? fs.readdirSync(sysTrash).filter((n) => n.startsWith(SYS_TAG)) : null;
      r.systemTrash = { before: sysBefore, after: sysAfter };
      r.app = disk('.obsidian/app.json');
    });
  }
  await step('s08b-links-do-not-update', async (r) => {
    await merge(r, 'u-src.md', 'u-dst', {}, { files: ['u-links.md', 'u-dst.md'], updateLinks: 'Do not update' });
  });
  await step('s08c-links-across-folders', async (r) => {
    await merge(r, 'Sub/sub-src.md', 'sub-dst', {}, { files: ['Sub/sub-links.md', 'Other/sub-dst.md'] });
  });
  await step('s10-ctrl-enter-new', async (r) => {
    await merge(r, 'n-src.md', 'n-brand-new', { ctrl: true }, { files: ['n-src.md', 'n-brand-new.md'] });
  });
  await step('s11-ctrl-enter-from-subfolder', async (r) => {
    await merge(r, 'Sub/n-sub-src.md', 'n-sub-new', { ctrl: true }, { files: ['Sub/n-sub-src.md', 'Sub/n-sub-new.md', 'n-sub-new.md'] });
  });
  await step('s12-template', async (r) => {
    r.setOpt = await P(`const i = app.internalPlugins.plugins['note-composer'].instance; i.options.template = 'Templates/Composer.md'; return JSON.parse(JSON.stringify(i.options));`);
    await merge(r, 'tpl-src.md', 'tpl-dst', {}, { files: ['tpl-dst.md'] });
  });
  await step('s13-template-no-content', async (r) => {
    r.setOpt = await P(`const i = app.internalPlugins.plugins['note-composer'].instance; i.options.template = 'Templates/NoContent.md'; return JSON.parse(JSON.stringify(i.options));`);
    await merge(r, 'tpl2-src.md', 'tpl2-dst', {}, { files: ['tpl2-dst.md'] });
    await P(`app.internalPlugins.plugins['note-composer'].instance.options.template = ''; return true;`);
  });
  await step('s14-dont-ask-again', async (r) => {
    r.before = disk('.obsidian/note-composer.json');
    await merge(r, 'd-src.md', 'd-dst', {}, { answer: 'tick', files: ['d-src.md', 'd-dst.md'] });
    r.after = disk('.obsidian/note-composer.json');
    r.options = await P(`return p.plugin('note-composer');`);
  });
  await step('s05-append-whitespace', async (r) => { await merge(r, 'w-app-src.md', 'w-app-dst', {}, { files: ['w-app-dst.md'] }); });
  await step('s06-prepend-whitespace', async (r) => { await merge(r, 'w-pre-src.md', 'w-pre-dst', { shift: true }, { files: ['w-pre-dst.md'] }); });
  await step('s05-append-crlf', async (r) => { await merge(r, 'w-crlf-src.md', 'w-crlf-dst', {}, { files: ['w-crlf-dst.md'] }); });
  await step('s17-always-update', async (r) => {
    r.set = await setApp('alwaysUpdateLinks', true);
    await merge(r, 'a-src.md', 'a-dst', {}, { files: ['a-links.md', 'a-dst.md'] });
    r.app = disk('.obsidian/app.json');
    await setApp('alwaysUpdateLinks', false);
  });
  await step('s18-system-trash-control', async (r) => {
    // Is a plain delete under trashOption "system" also landing in .trash here?
    r.set = await setApp('trashOption', 'system');
    const name = SYS_TAG + '-control.md';
    await c.evaluate(`await app.vault.create(${JSON.stringify(name)}, 'control\\n'); return true;`);
    await sleep(800);
    r.trashed = await c.evaluate(`const f = app.vault.getAbstractFileByPath(${JSON.stringify(name)}); await app.fileManager.trashFile(f); return true;`);
    await sleep(1500);
    r.inVault = disk(name);
    r.inDotTrash = disk('.trash/' + name);
    const sysTrash = path.join(os.homedir(), '.local/share/Trash/files');
    r.inSystemTrash = fs.existsSync(sysTrash) ? fs.readdirSync(sysTrash).filter((n) => n.startsWith(SYS_TAG)) : null;
    r.env = { DBUS: process.env.DBUS_SESSION_BUS_ADDRESS || null, XDG_CURRENT_DESKTOP: process.env.XDG_CURRENT_DESKTOP || null };
    r.electronTrash = await c.evaluate(`try { const p = require('path').join(app.vault.adapter.basePath, 'probe-trash-direct.md'); require('fs').writeFileSync(p, 'x'); await require('electron').shell.trashItem(p); return 'ok'; } catch (e) { return 'error: ' + e.message; }`).catch((e) => 'eval error: ' + e.message);
  });
  await step('s19-ctrl-enter-variants', async (r) => {
    // pressKey('Enter', {ctrl}) sends text '\r' with the modifier, which did not
    // create anything in the first run. Try the bare rawKeyDown without text.
    r.opened = await P(`return await p.open('k-src.md', 'source');`);
    const before = tree();
    r.ran = await P(`return app.commands.executeCommandById('note-composer:merge-file');`);
    await sleep(600);
    await typePrompt('k-brand-new');
    r.typed = await modals();
    const ev = { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13, modifiers: 2 };
    await c.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...ev });
    await c.send('Input.dispatchKeyEvent', { type: 'keyUp', ...ev });
    await sleep(1200);
    r.afterRawCtrlEnter = await modals();
    if (r.afterRawCtrlEnter.length && r.afterRawCtrlEnter[0].inputs.some((i) => i.value === 'k-brand-new')) {
      // Still the prompt: try a keyDown with text, ctrl held (the meta-less Linux path).
      await c.send('Input.dispatchKeyEvent', { type: 'keyDown', ...ev, text: '\r', unmodifiedText: '\r' });
      await c.send('Input.dispatchKeyEvent', { type: 'keyUp', ...ev });
      await sleep(1200);
      r.afterKeyDownCtrlEnter = await modals();
    }
    const m = await modals();
    if (m.length && m[0].title === 'Merge file') { r.confirm = m[0]; await P(`document.querySelector('.modal-container .mod-cta').click(); return true;`); await sleep(1000); }
    r.linkPrompt = await answerUpdateLinks('Just once');
    await sleep(2600);
    r.where = await P(`return p.where();`);
    r.changed = diffTrees(before, tree());
    r.notices = await P(`return p.takeNotices();`);
  });
  await step('s19b-ctrl-enter-diagnostics', async (r) => {
    // Does a CDP Ctrl+Enter reach a prompt at all? The quick switcher's own
    // Ctrl+Enter opens the pick in a new tab.
    r.opened = await P(`return await p.open('anchor.md', 'source');`);
    await P(`app.commands.executeCommandById('switcher:open'); return true;`);
    await sleep(600);
    r.switcherPrompt = (await modals()).map((m) => m.instructions.map((i) => i.text));
    await typePrompt('h-dst');
    await c.pressKey('Enter', { ctrl: true });
    await sleep(1000);
    r.switcherAfter = { modals: (await modals()).length, where: await P(`return p.where();`) };
    await closeModals();
    // Detach any extra tab the switcher made, keep one leaf.
    await P(`const ls = app.workspace.getLeavesOfType('markdown'); for (const l of ls.slice(1)) l.detach(); app.workspace.setActiveLeaf(ls[0], { focus: true }); return true;`);
    await sleep(300);
    // The merge prompt again, Ctrl+Enter now as a DOM KeyboardEvent on the input.
    r.opened2 = await P(`return await p.open('k2-src.md', 'source');`);
    const before = tree();
    await P(`app.commands.executeCommandById('note-composer:merge-file'); return true;`);
    await sleep(600);
    await typePrompt('k2-brand-new');
    r.domKey = await P(`const i = document.querySelector('.modal-container .prompt-input');
      const ev = new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13, ctrlKey: true, bubbles: true, cancelable: true });
      i.dispatchEvent(ev); await p.sleep(1200); return { prevented: ev.defaultPrevented, modals: p.modals().map((m) => [m.title, m.inputs.map((x) => x.value)]) };`);
    const m = await modals();
    if (m.length && m[0].title === 'Merge file') { r.confirm = m[0]; await P(`document.querySelector('.modal-container .mod-cta').click(); return true;`); await sleep(1000); }
    r.linkPrompt = await answerUpdateLinks('Just once');
    await sleep(2600);
    r.where = await P(`return p.where();`);
    r.changed = diffTrees(before, tree());
    r.notices = await P(`return p.takeNotices();`);
  });
  await step('s19c-ctrl-enter-console', async (r) => {
    // Ctrl+Enter is taken (defaultPrevented) yet nothing appears: listen to the console.
    r.log = [];
    await c.send('Runtime.enable');
    const off1 = c.on('Runtime.exceptionThrown', (e) => r.log.push({ exception: (e.exceptionDetails.exception && e.exceptionDetails.exception.description || e.exceptionDetails.text || '').slice(0, 800) }));
    const off2 = c.on('Runtime.consoleAPICalled', (e) => r.log.push({ type: e.type, args: e.args.map((a) => a.value !== undefined ? a.value : a.description).join(' ').slice(0, 800) }));
    r.variants = [];
    for (const [label, src, query, mods] of [
      ['ctrl, no match', 'k3a-src.md', 'k3-brand-new', { ctrl: true }],
      ['ctrl, exact match', 'k3b-src.md', 'k3-existing', { ctrl: true }],
      ['ctrl, partial match', 'k3c-src.md', 'k3-exi', { ctrl: true }],
      ['ctrl, partial match, source in Sub/', 'Sub/k3d-src.md', 'k3-exi', { ctrl: true }],
      ['ctrl+shift, partial match', 'k3e-src.md', 'k3-exi', { ctrl: true, shift: true }],
    ]) {
      await P(`return await p.open(${JSON.stringify(src)}, 'source');`);
      const before = tree();
      await P(`app.commands.executeCommandById('note-composer:merge-file'); return true;`);
      await sleep(600);
      await typePrompt(query);
      const listed = (await modals()).map((m) => [m.items.map((i) => i.text), m.empty]);
      await c.pressKey('Enter', mods);
      await sleep(1500);
      const m = await modals();
      const v = { label, query, listed, after: m.map((x) => [x.title, x.content, x.inputs.map((i) => i.value)]) };
      if (m.length && m[0].title === 'Merge file') { await P(`document.querySelector('.modal-container .mod-cta').click(); return true;`); await sleep(1000); }
      v.linkPrompt = await answerUpdateLinks('Just once');
      await sleep(2000);
      await closeModals();
      v.changed = diffTrees(before, tree());
      v.where = await P(`return p.where();`);
      v.notices = await P(`return p.takeNotices();`);
      r.variants.push(v); save();
    }
    off1(); off2();
  });
  await step('s16-no-history', async (r) => {
    // A fresh tab whose only history is the empty tab: where does the leaf go?
    r.fresh = await P(`const old = app.workspace.getLeavesOfType('markdown'); const l = app.workspace.getLeaf('tab'); for (const o of old) o.detach(); app.workspace.setActiveLeaf(l, { focus: true }); await p.sleep(300); return p.where();`);
    const before = tree();
    r.opened = await P(`return await p.open('h-src.md', 'source');`);
    r.ran = await P(`return app.commands.executeCommandById('note-composer:merge-file');`);
    await sleep(600);
    await typePrompt('h-dst');
    r.selected = await selectedItem();
    await c.pressKey('Enter');
    await sleep(900);
    r.afterKey = await modals();
    await P(`const b = document.querySelector('.modal-container .mod-cta'); if (b) b.click(); return true;`);
    await sleep(900);
    r.linkPrompt = await answerUpdateLinks('Just once');
    await sleep(2600);
    r.where = await P(`return p.where();`);
    r.changed = diffTrees(before, tree());
  });
  await step('s15-after-dont-ask', async (r) => {
    await merge(r, 'e-src.md', 'e-dst', {}, { files: ['e-src.md', 'e-dst.md'] });
  });
  await step('s20-availability', async (r) => {
    await P(`return await p.open('anchor.md', 'source');`);
    await availability(r, 'source');
    await P(`return await p.open('l-dst.md', 'preview');`);
    await availability(r, 'reading');
    await P(`return await p.open('Canvas.canvas');`);
    await availability(r, 'canvas');
    await P(`app.workspace.activeLeaf.setViewState({ type: 'empty' }); return true;`); await sleep(400);
    await availability(r, 'empty');
  });
}

// ===========================================================================
/**
 * Split/extract: open `src`, place the caret/selection, run `cmd`, type
 * `name` (or keep the default if null), press Enter with `mods`.
 */
async function extract(r, src, cmd, caret, name, mods = {}, files = []) {
  const before = tree();
  r.opened = await P(`return await p.open(${JSON.stringify(src)}, 'source');`);
  await sleep(300);
  r.caret = await P(`return p.caret(${JSON.stringify(caret)});`);
  await P(`p.takeNotices(); return true;`);
  r.ran = await P(`return app.commands.executeCommandById(${JSON.stringify(cmd)});`);
  await sleep(700);
  r.prompt = await modals();
  save();
  if (r.prompt.length) {
    if (name !== null) await typePrompt(name);
    r.typed = await modals();
    r.selected = await selectedItem();
    await c.pressKey('Enter', mods);
    await sleep(900);
    r.afterKey = await modals();
    if (r.afterKey.length) {
      const clicked = await P(`const b = document.querySelector('.modal-container .mod-cta'); if (!b) return null; const t = b.textContent; b.click(); return t;`);
      r.answered = clicked; await sleep(800);
    }
  }
  await sleep(2600);
  r.notices = await P(`return p.takeNotices();`);
  r.where = await P(`return p.where();`);
  r.editorAfter = await P(`const e = p.ed(); return e ? { value: e.getValue(), cursor: e.getCursor() } : null;`);
  r.changed = diffTrees(before, tree());
  r.bytes = {}; for (const f of files) r.bytes[f] = disk(f);
  save();
}
const PICK = { sel: [{ line: 1, ch: 0 }, { line: 1, ch: 7 }] };

if (SCEN === 'split') {
  await step('s00-options', async (r) => { r.options = await P(`return p.plugin('note-composer');`); r.app = disk('.obsidian/app.json'); });
  const cases = [['link-wiki', 'link', false], ['embed-wiki', 'embed', false], ['none-wiki', 'none', false], ['link-md', 'link', true], ['embed-md', 'embed', true]];
  for (const [k, rt, md] of cases) {
    await step(`s01-split-${k}`, async (r) => {
      r.opt = await setOpt('replacementText', rt);
      r.md = await setApp('useMarkdownLinks', md);
      await extract(r, `s-${k}.md`, 'note-composer:split-file', PICK, `new-${k}`, {}, [`s-${k}.md`, `new-${k}.md`]);
    });
  }
  await setOpt('replacementText', 'link').catch(() => {}); await setApp('useMarkdownLinks', false).catch(() => {});
  for (const loc of ['root', 'current', 'folder']) {
    await step(`s02-folder-${loc}`, async (r) => {
      r.loc = await setApp('newFileLocation', loc);
      if (loc === 'folder') r.folder = await setApp('newFileFolderPath', 'Inbox');
      await c.evaluate(`const f = app.vault.getAbstractFileByPath('Sub/s-folder.md'); await app.vault.modify(f, 'before\\nPICK ME\\nafter\\n'); return true;`);
      await sleep(500);
      await extract(r, 'Sub/s-folder.md', 'note-composer:split-file', PICK, `folder-${loc}`, {}, ['Sub/s-folder.md']);
    });
  }
  await setApp('newFileLocation', 'root').catch(() => {});
  await step('s03-split-into-existing', async (r) => {
    await extract(r, 's-existing.md', 'note-composer:split-file', PICK, 's-existing-target', {}, ['s-existing.md', 's-existing-target.md']);
  });
  await step('s03b-split-into-existing-shift', async (r) => {
    await c.evaluate(`const f = app.vault.getAbstractFileByPath('s-existing.md'); await app.vault.modify(f, 'before\\nPICK ME\\nafter\\n'); return true;`);
    await sleep(500);
    await extract(r, 's-existing.md', 'note-composer:split-file', PICK, 's-existing-target', { shift: true }, ['s-existing.md', 's-existing-target.md']);
  });
  await step('s04-split-multiline-default-name', async (r) => {
    await extract(r, 's-multiline.md', 'note-composer:split-file', { sel: [{ line: 1, ch: 0 }, { line: 2, ch: 8 }] }, null, {}, ['s-multiline.md']);
  });
  await step('s05-split-no-selection', async (r) => {
    await extract(r, 's-multiline.md', 'note-composer:split-file', { cursor: { line: 0, ch: 2 } }, null, {}, ['s-multiline.md']);
  });
  await step('s06-heading-top-default', async (r) => {
    await extract(r, 'h-top.md', 'note-composer:extract-heading', { cursor: { line: 0, ch: 3 } }, null, {}, ['h-top.md', 'Zulu.md']);
  });
  await step('s07-heading-nested-default', async (r) => {
    await extract(r, 'h-mid.md', 'note-composer:extract-heading', { cursor: { line: 2, ch: 4 } }, null, {}, ['h-mid.md', 'Yankee.md']);
  });
  await step('s07b-heading-last', async (r) => {
    await extract(r, 'h-last.md', 'note-composer:extract-heading', { cursor: { line: 2, ch: 4 } }, null, {}, ['h-last.md', 'Victor.md']);
  });
  await step('s07c-heading-typed-name', async (r) => {
    await extract(r, 'h-new.md', 'note-composer:extract-heading', { cursor: { line: 2, ch: 1 } }, 'typed-heading-name', {}, ['h-new.md', 'typed-heading-name.md', 'Papa.md']);
  });
  await step('s08-heading-middle', async (r) => {
    await extract(r, 'h-body.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 3 } }, null, {}, ['h-body.md', 'Uniform.md']);
  });
  await step('s08b-heading-caret-in-body', async (r) => {
    await extract(r, 'h-body.md', 'note-composer:extract-heading', { cursor: { line: 5, ch: 3 } }, null, {}, ['h-body.md']);
  });
  await step('s08c-heading-eof-no-newline', async (r) => {
    await extract(r, 'h-eof.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 3 } }, null, {}, ['h-eof.md', 'Oscar.md']);
  });
  await step('s08d-heading-with-template', async (r) => {
    r.opt = await setOpt('template', 'Templates/Composer.md');
    await extract(r, 'h-tpl.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 3 } }, null, {}, ['h-tpl.md', 'Mike.md']);
    await setOpt('template', '');
  });
  await step('s09-heading-none', async (r) => {
    await extract(r, 'h-none.md', 'note-composer:extract-heading', { cursor: { line: 0, ch: 3 } }, null, {}, ['h-none.md']);
  });
  await step('s10-heading-odd-name', async (r) => {
    await extract(r, 'h-odd.md', 'note-composer:extract-heading', { cursor: { line: 1, ch: 4 } }, null, {}, ['h-odd.md']);
  });
  await step('s10b-split-md-from-subfolder', async (r) => {
    r.md = await setApp('useMarkdownLinks', true);
    await extract(r, 'Sub/s-sub-md.md', 'note-composer:split-file', PICK, 'md-from-sub', {}, ['Sub/s-sub-md.md', 'md-from-sub.md']);
    await setApp('useMarkdownLinks', false);
  });
  await step('s11-template', async (r) => {
    r.opt = await setOpt('template', 'Templates/Composer.md');
    await extract(r, 'tpl-split.md', 'note-composer:split-file', PICK, 'tpl-new', {}, ['tpl-split.md', 'tpl-new.md']);
  });
  await step('s12-template-no-content', async (r) => {
    r.opt = await setOpt('template', 'Templates/NoContent.md');
    await extract(r, 'tpl-split2.md', 'note-composer:split-file', PICK, 'tpl-new2', {}, ['tpl-split2.md', 'tpl-new2.md']);
    await setOpt('template', '');
  });
  await step('s20-availability', async (r) => {
    await P(`return await p.open('s-existing-target.md', 'source');`);
    await availability(r, 'source');
    await P(`p.ed().setSelection({line:0,ch:0},{line:0,ch:5}); return true;`);
    await availability(r, 'sourceWithSelection');
    await P(`return await p.open('h-body.md', 'source');`);
    await P(`p.ed().setCursor({ line: 1, ch: 2 }); return true;`);
    await availability(r, 'sourceCaretOnHeading');
    await P(`p.ed().setSelection({ line: 1, ch: 0 }, { line: 2, ch: 3 }); return true;`);
    await availability(r, 'sourceSelectionFromHeading');
    await P(`p.ed().setCursor({ line: 2, ch: 2 }); return true;`);
    await availability(r, 'sourceCaretUnderHeading');
    await P(`return await p.open('s-existing-target.md', 'preview');`);
    await availability(r, 'reading');
    await P(`return await p.open('Canvas.canvas');`);
    await availability(r, 'canvas');
  });
}

// ===========================================================================
const zkOpt = (o) => P(`const i = app.internalPlugins.plugins['zk-prefixer'].instance; Object.assign(i.options, ${JSON.stringify(o)}); return JSON.parse(JSON.stringify(i.options));`);
async function zk(r, cmd, files = []) {
  const before = tree();
  r.whereBefore = await P(`return p.where();`);
  await P(`p.takeNotices(); return true;`);
  r.at = new Date().toISOString();
  r.ran = await P(`return app.commands.executeCommandById(${JSON.stringify(cmd)});`);
  await sleep(900);
  r.modals = await modals();
  await sleep(2600);
  r.notices = await P(`return p.takeNotices();`);
  r.where = await P(`return p.where();`);
  r.editor = await P(`const e = p.ed(); return e ? { value: e.getValue(), cursor: e.getCursor(), sel: e.getSelection() } : null;`);
  r.changed = diffTrees(before, tree());
  r.bytes = {}; for (const f of files) r.bytes[f] = disk(f);
  save();
}
if (SCEN === 'zk') {
  await step('s00-options', async (r) => { r.options = await P(`return p.plugin('zk-prefixer');`); r.files = cfgFiles(); });
  await step('s01-create', async (r) => { await P(`return await p.open('Home.md', 'source');`); await zk(r, 'zk-prefixer'); });
  await step('s02-create-again', async (r) => { await zk(r, 'zk-prefixer'); });
  await step('s03-create-third', async (r) => { await zk(r, 'zk-prefixer'); });
  await step('s04-folder-template', async (r) => {
    r.opt = await zkOpt({ folder: 'Zettel', template: 'Templates/Unique.md' });
    await P(`return await p.open('Home.md', 'source');`);
    await zk(r, 'zk-prefixer');
  });
  await step('s04b-missing-folder', async (r) => {
    r.opt = await zkOpt({ folder: 'NoSuchFolder', template: '' });
    await P(`return await p.open('Home.md', 'source');`);
    await zk(r, 'zk-prefixer');
  });
  await step('s04c-format', async (r) => {
    r.opt = await zkOpt({ folder: '', template: '', format: 'YYYY-MM-DD [note] HHmmss' });
    await P(`return await p.open('Home.md', 'source');`);
    await zk(r, 'zk-prefixer');
    r.second = {}; await zk(r.second, 'zk-prefixer');
    await zkOpt({ format: '' });
  });
  await step('s04d-new-tab-pref', async (r) => {
    // A pinned tab: does the note still replace it?
    await P(`return await p.open('Home.md', 'source');`);
    r.pinned = await P(`app.workspace.activeLeaf.setPinned(true); return app.workspace.activeLeaf.getViewState().pinned;`);
    await zk(r, 'zk-prefixer');
    await P(`for (const l of app.workspace.getLeavesOfType('markdown')) l.setPinned(false); const ls = app.workspace.getLeavesOfType('markdown'); for (const l of ls.slice(1)) l.detach(); return true;`);
  });
  await step('s05-insert-link', async (r) => {
    await zkOpt({ folder: '', template: '' });
    await P(`return await p.open('Link.md', 'source');`); await sleep(300);
    r.caret = await P(`return p.caret({ cursor: { line: 0, ch: 13 } });`);
    await zk(r, 'insert-unique-link', ['Link.md']);
  });
  await step('s06-insert-link-md', async (r) => {
    r.md = await setApp('useMarkdownLinks', true);
    await P(`return await p.open('Link.md', 'source');`); await sleep(300);
    r.caret = await P(`const e = p.ed(); e.setCursor({ line: 0, ch: e.getLine(0).length }); return e.getCursor();`);
    await zk(r, 'insert-unique-link', ['Link.md']);
    await setApp('useMarkdownLinks', false);
  });
  await step('s07-insert-link-selection', async (r) => {
    await P(`return await p.open('Link.md', 'source');`); await sleep(300);
    r.caret = await P(`return p.caret({ sel: [{ line: 0, ch: 0 }, { line: 0, ch: 6 }] });`);
    await zk(r, 'insert-unique-link', ['Link.md']);
  });
  await step('s08-insert-link-twice', async (r) => {
    await P(`return await p.open('Link.md', 'source');`); await sleep(300);
    r.caret = await P(`const e = p.ed(); e.setCursor({ line: 0, ch: e.getLine(0).length }); return e.getCursor();`);
    await zk(r, 'insert-unique-link', ['Link.md']);
    r.second = {}; await zk(r.second, 'insert-unique-link', ['Link.md']);
  });
  await step('s09-insert-link-folder', async (r) => {
    r.opt = await zkOpt({ folder: 'Zettel', template: 'Templates/Unique.md' });
    await P(`return await p.open('Link.md', 'source');`); await sleep(300);
    r.caret = await P(`const e = p.ed(); e.setCursor({ line: 0, ch: e.getLine(0).length }); return e.getCursor();`);
    await zk(r, 'insert-unique-link', ['Link.md']);
    r.mdToo = {}; await setApp('useMarkdownLinks', true); await zk(r.mdToo, 'insert-unique-link', ['Link.md']); await setApp('useMarkdownLinks', false);
    await zkOpt({ folder: '', template: '' });
  });
  await step('s20-availability', async (r) => {
    await P(`return await p.open('Home.md', 'source');`);
    await availability(r, 'source');
    await P(`return await p.open('Home.md', 'preview');`);
    await availability(r, 'reading');
    await P(`return await p.open('Canvas.canvas');`);
    await availability(r, 'canvas');
    await P(`app.workspace.activeLeaf.setViewState({ type: 'empty' }); return true;`); await sleep(400);
    await availability(r, 'empty');
  });
}

// ===========================================================================
if (SCEN.startsWith('random')) {
  await step('s01-ribbon', async (r) => {
    r.ribbon = await P(`return Array.from(document.querySelectorAll('.workspace-ribbon .side-dock-actions > *, .workspace-ribbon .side-dock-settings > *')).map((b, i) => ({ i, aria: b.getAttribute('aria-label'), cls: String(b.className), svg: b.querySelector('svg') ? b.querySelector('svg').getAttribute('class') : null, html: b.outerHTML.slice(0, 300) }));`);
    r.ribbonItems = await P(`return app.workspace.leftRibbon && app.workspace.leftRibbon.items ? app.workspace.leftRibbon.items.map((x) => ({ id: x.id, title: x.title, icon: x.icon, hidden: x.hidden })) : null;`);
  });
  await step('s02-runs', async (r) => {
    const first = SCEN === 'random2' ? 'One.md' : SCEN === 'random1' ? 'Only.md' : null;
    if (first) r.opened = await P(`return await p.open(${JSON.stringify(first)}, 'source');`);
    r.runs = [];
    const n = SCEN === 'random2' ? 20 : 3;
    for (let i = 0; i < n; i++) {
      await P(`p.takeNotices(); return true;`);
      const before = await P(`return p.where();`);
      const ran = await P(`return app.commands.executeCommandById('random-note');`);
      await sleep(600);
      const after = await P(`return p.where();`);
      r.runs.push({ before: before.active, ran, after: after.active, leaves: after.leaves.length, all: after.allLeaves, tabs: after.tabHeaders, leafList: after.leaves, notices: await P(`return p.takeNotices();`), modals: (await modals()).length });
      save();
    }
  }, 120000);
  await step('s03-ribbon-click', async (r) => {
    const sel = await P(`const b = Array.from(document.querySelectorAll('.workspace-ribbon .side-dock-ribbon-action')).find((x) => /random/i.test(x.getAttribute('aria-label') || '')); if (!b) return null; b.setAttribute('data-probe', 'random'); return b.getAttribute('aria-label');`);
    r.label = sel;
    if (sel) {
      r.before = await P(`return p.where();`);
      await P(`p.takeNotices(); return true;`);
      await c.clickElement('[data-probe=random]');
      await sleep(700);
      r.after = await P(`return p.where();`);
      r.notices = await P(`return p.takeNotices();`);
      // A middle and a ctrl click: does the ribbon open a new tab then?
      await c.clickElement('[data-probe=random]', { mods: { ctrl: true } });
      await sleep(700);
      r.afterCtrl = await P(`return p.where();`);
    }
  });
  await step('s20-availability', async (r) => {
    await availability(r, 'start');
    await P(`app.workspace.activeLeaf.setViewState({ type: 'empty' }); return true;`); await sleep(400);
    await availability(r, 'empty');
  });
}

save();
console.log(JSON.stringify({ scen: SCEN, steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
