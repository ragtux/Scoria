// What a full path typed with its `.md` does in Obsidian's file pickers
// (2026-11-22): the Note composer's "Merge with" modal, the template file
// pickers of Note composer, Unique note creator and Daily notes in Settings,
// and the quick switcher. For each query: the rows listed (text, the
// `suggestion-highlight` spans, which is selected) and what Enter does — the
// merge's confirmation text, the template the picker writes, the file the
// switcher opens.
// Phases: fixtures | run   (node composer-picker-path.mjs <phase>)
// Run with `OBSIDIAN_ARGS=--ozone-platform=x11` in a headless sway.
// `ONLY=<regex>` picks steps.
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const vp = (rel) => path.join(VAULT, rel);
const write = (rel, s) => { fs.mkdirSync(path.dirname(vp(rel)), { recursive: true }); fs.writeFileSync(vp(rel), s); };
const disk = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch { return null; } };

const FILES = [
  'Folder/Note.md', 'Note.md', 'Folder/Notebook.md', 'Folder/Sub/Note.md', 'Other/Folder/Note.md',
  'Folder Note md.md', 'Folder/Note.png', 'Folder/Note.canvas', 'Source.md', 'Templates/T.md',
];

if (phase === 'fixtures') {
  write('.obsidian/app.json', '{}');
  write('.obsidian/core-plugins.json', JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, 'command-palette': true,
    'note-composer': true, 'zk-prefixer': true, 'daily-notes': true, 'page-preview': false,
  }, null, 2));
  write('.obsidian/note-composer.json', JSON.stringify({ askBeforeMerging: true }, null, 2));
  for (const rel of FILES) {
    write(rel, rel.endsWith('.png') ? 'not really a png' : rel.endsWith('.canvas') ? '{}' : `# ${path.basename(rel, '.md')}\n`);
  }
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const ONLY = process.env.ONLY ? new RegExp(process.env.ONLY) : null;
const out = all['run' + (ONLY ? ':' + process.env.ONLY : '')] = { startedAt: new Date().toISOString(), steps: {}, order: [] };
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

const c = await connect();
out.vaultPath = await c.evaluate(`return app.vault.adapter.basePath;`);
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }
out.title = await c.evaluate(`return document.title;`);
out.version = await c.evaluate(`return { ua: navigator.userAgent, api: (window.require && require('obsidian').apiVersion) || null };`).catch(() => null);
out.getFiles = await c.evaluate(`return app.vault.getFiles().map((f) => f.path);`);

async function step(name, fn, ms = 240000) {
  if (ONLY && !ONLY.test(name)) return null;
  out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  try {
    await Promise.race([fn(partial), new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); })]);
    out.steps[name] = { ok: true, ms: Date.now() - t, ...partial };
  } catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), ...partial }; }
  finally { clearTimeout(timer); }
  try { await c.pressKey('Escape'); } catch { /* ignore */ }
  save();
  console.error(`[picker-path] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + String(out.steps[name].error).split('\n')[0]}`);
}

const QUERIES = ['Folder/Note.md', 'folder/note.md', 'FOLDER/NOTE.MD', 'Note.md', 'Folder/Note', 'older/Note.md', 'Folder/Note.m', 'Folder/Note.png', 'Folder/Notebook.md'];

// Rows of an open suggestion list in `root` (a selector), with highlights.
const ROWS = (root) => `const x = document.querySelector(${JSON.stringify(root)}); if (!x) return null;
  return { input: (x.querySelector('input') || {}).value ?? null,
    empty: (x.querySelector('.suggestion-empty') || {}).textContent || null,
    rows: Array.from(x.querySelectorAll('.suggestion-item')).slice(0, 12).map((it) => ({
      text: it.textContent.trim(), sel: it.classList.contains('is-selected'),
      hl: Array.from(it.querySelectorAll('.suggestion-highlight')).map((h) => h.textContent),
      html: it.innerHTML.slice(0, 500) })) };`;

const openNote = async (p) => { await c.evaluate(`const l = app.workspace.getLeavesOfType('markdown')[0] || app.workspace.getLeaf(false); await l.openFile(app.vault.getAbstractFileByPath(${JSON.stringify(p)})); app.workspace.setActiveLeaf(l, { focus: true }); return true;`); await sleep(500); };
const closeModals = async () => { for (let i = 0; i < 3; i++) { await c.pressKey('Escape'); await sleep(250); } };

// Type into the focused prompt input with real keys.
async function typeQuery(s, q) { await s.typeText(q, 25); await sleep(500); }

await step('s01-merge', async (r) => {
  r.queries = {};
  for (const q of QUERIES) {
    await openNote('Source.md');
    await c.evaluate(`app.commands.executeCommandById('note-composer:merge-file'); return true;`); await sleep(800);
    await typeQuery(c, q);
    const listed = await c.evaluate(ROWS('.modal-container .prompt'));
    await c.pressKey('Enter'); await sleep(900);
    const after = await c.evaluate(`return Array.from(document.querySelectorAll('.modal-container .modal')).map((m) => ({ title: (m.querySelector('.modal-title') || {}).textContent || null, content: (m.querySelector('.modal-content') || {}).textContent || null, prompt: !!m.closest('.prompt') || m.classList.contains('prompt') }));`);
    r.queries[q] = { listed, after };
    await closeModals();
    save();
  }
  r.sourceAfter = disk('Source.md');
});

await step('s02-switcher', async (r) => {
  r.queries = {};
  for (const q of QUERIES) {
    await openNote('Source.md');
    await c.evaluate(`app.commands.executeCommandById('switcher:open'); return true;`); await sleep(800);
    await typeQuery(c, q);
    const listed = await c.evaluate(ROWS('.modal-container .prompt'));
    await c.pressKey('Enter'); await sleep(900);
    const opened = await c.evaluate(`const f = app.workspace.getActiveFile(); return { active: f ? f.path : null, modal: !!document.querySelector('.modal-container') };`);
    r.queries[q] = { listed, opened };
    await closeModals();
    save();
  }
});

/** Open Settings on tab `id` and connect to its window (a page target of its own). */
async function settings(id) {
  const before = await pageTargets();
  await c.evaluate(`app.setting.open(); app.setting.openTabById(${JSON.stringify(id)}); return true;`);
  await sleep(1800);
  const after = await pageTargets();
  let url = after.find((u) => !before.includes(u));
  if (!url) {
    for (const u of after) {
      const s = await connect(u);
      const t = await s.evaluate(`return document.title;`).catch(() => null);
      if (t === 'Settings') { url = u; s.close(); break; }
      s.close();
    }
  }
  if (!url) return { s: c, popout: false };
  return { s: await connect(url), popout: true };
}
const closeSettings = async () => { await c.evaluate(`app.setting.close(); return true;`); await sleep(600); };

async function templatePicker(r, tab, file) {
  r.queries = {};
  for (const q of QUERIES) {
    const { s, popout } = await settings(tab);
    const BTN = `const row = Array.from(document.querySelectorAll('.vertical-tab-content .setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === 'Template file location'); const b = row && row.querySelector('.combobox-button'); if (!b) return null; b.scrollIntoView({ block: 'center' }); const rr = b.getBoundingClientRect(); return { cx: rr.x + rr.width / 2, cy: rr.y + rr.height / 2, label: (b.querySelector('.combobox-button-label') || {}).textContent };`;
    const b = await s.evaluate(BTN);
    if (!b) { r.queries[q] = { noButton: true }; if (popout) s.close(); await closeSettings(); continue; }
    await s.clickAt(b.cx, b.cy); await sleep(800);
    let opened = await s.evaluate(`return !!document.querySelector('.suggestion-container');`);
    if (!opened) {
      await s.evaluate(`const row = Array.from(document.querySelectorAll('.vertical-tab-content .setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === 'Template file location'); row.querySelector('.combobox-button').click(); return true;`);
      await sleep(800);
    }
    await typeQuery(s, q);
    const listed = await s.evaluate(ROWS('.suggestion-container'));
    await s.pressKey('Enter'); await sleep(1200);
    const stillOpen = await s.evaluate(`return !!document.querySelector('.suggestion-container');`);
    r.queries[q] = { listed, stillOpen, file: disk('.obsidian/' + file), label: (await s.evaluate(BTN) || {}).label ?? null };
    if (stillOpen) { await s.pressKey('Escape'); await sleep(300); }
    // Reset the template so the next query starts from nothing.
    await c.evaluate(`const p = app.internalPlugins.plugins[${JSON.stringify(tab)}].instance; if (p.options) { p.options.template = ''; } return true;`);
    if (popout) s.close();
    await closeSettings();
    save();
  }
}
await step('s03-picker-note-composer', (r) => templatePicker(r, 'note-composer', 'note-composer.json'));
await step('s04-picker-zk-prefixer', (r) => templatePicker(r, 'zk-prefixer', 'zk-prefixer.json'));
await step('s05-picker-daily-notes', (r) => templatePicker(r, 'daily-notes', 'daily-notes.json'));

out.finishedAt = new Date().toISOString();
save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
