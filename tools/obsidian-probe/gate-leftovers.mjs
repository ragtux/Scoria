// The four observations the Writer gate's week left open
// ([[Writer gate run#The verdict, 2026-11-09]]), in one block:
//
//   s01  Mod+B's second press over whole bullet lines: does Obsidian strip
//        `- **text**` back to `- text`, and what does one whole bullet line
//        selected get?
//   s02  the numbered toggle's second press over lines that were bullets:
//        bare lines or the bullets back? Both the flat shape and the nested
//        one day 6 was run on.
//   s03  `theme:switch` ("Change theme..."): what it opens, what it lists
//        with one community theme installed, and what picking a row writes
//        to appearance.json.
//   s04  the "Update links" prompt: which of its three buttons carries
//        `mod-cta`, which the explorer-dnd probe recorded by label only.
//
// Seeds go through the vault (`app.vault.modify`) as editing-commands.mjs
// does, and commands are dispatched the way the palette does it.
//
// Phases: fixtures | run   (node gate-leftovers.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }

const T0 = Date.now();
const BUDGET_MS = 300000;

function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.gateLeftovers = all.gateLeftovers || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.gateLeftovers[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn) {
  out.order.push(name);
  if (Date.now() - T0 > BUDGET_MS) { out.steps[name] = { ok: false, error: 'skipped: time budget exhausted' }; save(); return null; }
  const t = Date.now();
  const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const THEME = 'Probe Theme';

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Probe.md'), 'seed\n');
  fs.writeFileSync(vp('Alpha.md'), '# Alpha\n');
  fs.writeFileSync(vp('Linker.md'), 'See [[Alpha]] and [[Alpha|the alias]].\n');
  // alwaysUpdateLinks absent: Obsidian asks (day 2 of the gate).
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/appearance.json'), '{}\n');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'command-palette': true, switcher: true,
  }, null, 2));
  const dir = vp(path.join('.obsidian/themes', THEME));
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'theme.css'), 'body { --text-error: rgb(1, 2, 3); }\n');
  fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify({
    name: THEME, version: '1.0.0', minAppVersion: '1.0.0', author: 'probe',
  }, null, 2) + '\n');
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

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
  async open(file) {
    const f = app.vault.getAbstractFileByPath(file);
    const ls = app.workspace.getLeavesOfType('markdown');
    const leaf = ls.length ? ls[0] : app.workspace.getLeaf(false);
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(400);
    const st = leaf.getViewState();
    st.state = Object.assign({}, st.state, { mode: 'source', source: false });
    await leaf.setViewState(st);
    await this.sleep(220);
    return { leaves: app.workspace.getLeavesOfType('markdown').length, file: this.view().file.path, state: leaf.getViewState().state };
  },
  async seed(spec) {
    const view = this.view();
    const ed = this.ed();
    await app.vault.modify(view.file, spec.content);
    await this.sleep(spec.settle || 140);
    let seedOk = ed.getValue() === spec.content;
    if (!seedOk) { ed.setValue(spec.content); await this.sleep(100); seedOk = ed.getValue() === spec.content; }
    ed.focus();
    if (spec.sel) ed.setSelection(spec.sel[0], spec.sel[1]);
    else ed.setCursor(spec.cursor || { line: 0, ch: 0 });
    const r = this.read();
    r.seedOk = seedOk;
    return r;
  },
  read() {
    const ed = this.ed();
    return { value: ed.getValue(), cursor: ed.getCursor(), from: ed.getCursor('from'), to: ed.getCursor('to'), sel: ed.getSelection() };
  },
  // Seed, then run the command n times, reading after each.
  async cycle(spec) {
    const before = await this.seed(spec);
    const steps = [];
    for (let i = 0; i < spec.n; i++) {
      let ran = null, err = null;
      try { ran = app.commands.executeCommandById(spec.cmd); } catch (e) { err = String(e); }
      await this.sleep(160);
      const r = this.read(); r.ran = ran; if (err) r.err = err;
      steps.push(r);
    }
    return { in: spec.content, caret: spec.sel || spec.cursor, seedOk: before.seedOk, out: steps.map((s) => s.value), steps };
  },
  describe(el) {
    if (!el) return null;
    return { tag: el.tagName, cls: el.className, text: (el.textContent || '').trim().slice(0, 120) };
  },
  modal() {
    const m = document.querySelector('.modal-container') || document.querySelector('.modal');
    if (!m) return null;
    const root = m.querySelector('.modal') || m;
    const q = (s) => Array.from(root.querySelectorAll(s));
    return {
      containerCls: m.className,
      cls: root.className,
      title: this.describe(root.querySelector('.modal-title')),
      content: (root.querySelector('.modal-content') || {}).textContent,
      inputs: q('input').map((i) => ({ cls: i.className, placeholder: i.placeholder, value: i.value, type: i.type })),
      buttons: q('button').map((b) => this.describe(b)),
      prompt: this.describe(root.querySelector('.prompt')),
      items: q('.suggestion-item').map((x) => ({ cls: x.className, text: x.textContent.trim().slice(0, 120), html: x.innerHTML.slice(0, 300) })),
    };
  },
  suggest() {
    const s = document.querySelector('.suggestion-container');
    if (!s) return null;
    return { cls: s.className, items: Array.from(s.querySelectorAll('.suggestion-item')).map((x) => ({ cls: x.className, text: x.textContent.trim().slice(0, 120) })) };
  },
};
return true;`);

const cycle = (content, caret, cmd, n) => {
  const spec = { content, cmd, n };
  if (Array.isArray(caret)) spec.sel = caret; else spec.cursor = caret;
  return c.evaluate(`return await window.__p.cycle(${JSON.stringify(spec)});`);
};
const modal = () => c.evaluate(`return window.__p.modal();`);
const esc = async () => { await c.pressKey('Escape'); await sleep(160); };
const readCfg = (rel) => c.evaluate(`return await app.vault.adapter.read(${JSON.stringify(rel)});`);

await step('s00-context', async (r) => {
  r.opened = await c.evaluate(`app.workspace.leftSplit.collapse(); return await window.__p.open('Probe.md');`);
  r.versions = await c.evaluate(`return { apiVersion: String(app.apiVersion), appVersion: String(app.appVersion) };`);
  r.commands = await c.evaluate(`
    const ids = ['theme:switch', 'theme:use-dark', 'theme:use-light', 'editor:toggle-bold', 'editor:toggle-numbered-list'];
    const o = {}; for (const id of ids) { const cmd = app.commands.commands[id]; o[id] = cmd ? { name: cmd.name, hasCallback: !!cmd.callback, hasEditorCallback: !!cmd.editorCallback, hasCheck: !!cmd.checkCallback } : null; } return o;`);
  r.themes = await c.evaluate(`return { installed: Object.keys(app.customCss.themes || {}), theme: app.customCss.theme, setting: app.vault.getConfig('cssTheme') };`);
  return r;
});

await step('s01-bold-second-press-over-bullets', async (r) => {
  // The day-6 shape: two whole bullet lines, Mod+B twice.
  r.twoBullets = await cycle('- one\n- two', [{ line: 0, ch: 0 }, { line: 1, ch: 5 }], 'editor:toggle-bold', 2);
  // Seeded as what the first press leaves, one press: the answer does not
  // depend on the selection the first press left behind.
  r.overBoldBullets = await cycle('- **one**\n- **two**', [{ line: 0, ch: 0 }, { line: 1, ch: 9 }], 'editor:toggle-bold', 1);
  // A single whole bullet line selected, twice (unprobed on day 6).
  r.oneBullet = await cycle('- one', [{ line: 0, ch: 0 }, { line: 0, ch: 5 }], 'editor:toggle-bold', 2);
  // A single bullet line with the selection on the text alone, twice.
  r.oneBulletText = await cycle('- one', [{ line: 0, ch: 2 }, { line: 0, ch: 5 }], 'editor:toggle-bold', 2);
  // Task and numbered markers, which the wrap is said to skip the same way.
  r.tasks = await cycle('- [ ] one\n- [x] two', [{ line: 0, ch: 0 }, { line: 1, ch: 9 }], 'editor:toggle-bold', 2);
  r.numbered = await cycle('1. one\n2. two', [{ line: 0, ch: 0 }, { line: 1, ch: 6 }], 'editor:toggle-bold', 2);
  r.italicsTwoBullets = await cycle('- one\n- two', [{ line: 0, ch: 0 }, { line: 1, ch: 5 }], 'editor:toggle-italics', 2);
  return r;
});

await step('s02-numbered-second-press-over-bullets', async (r) => {
  r.flat = await cycle('- a\n- b\n- c\n- d', [{ line: 0, ch: 0 }, { line: 3, ch: 3 }], 'editor:toggle-numbered-list', 2);
  r.nested = await cycle('- a\n\t- b\n\t- c\n- d', [{ line: 0, ch: 0 }, { line: 3, ch: 3 }], 'editor:toggle-numbered-list', 2);
  r.nestedThrice = await cycle('- a\n\t- b\n\t- c\n- d', [{ line: 0, ch: 0 }, { line: 3, ch: 3 }], 'editor:toggle-numbered-list', 3);
  // The bullet toggle over numbered lines, for the symmetric rule.
  r.bulletOverNumbered = await cycle('1. a\n2. b', [{ line: 0, ch: 0 }, { line: 1, ch: 4 }], 'editor:toggle-bullet-list', 2);
  return r;
});

await step('s03-theme-switch', async (r) => {
  r.before = await readCfg('.obsidian/appearance.json');
  r.ran = await c.evaluate(`return app.commands.executeCommandById('theme:switch');`);
  await sleep(500);
  r.modal = await modal();
  r.suggest = await c.evaluate(`return window.__p.suggest();`);
  r.activeElement = await c.evaluate(`return window.__p.describe(document.activeElement);`);
  // Pick the community theme by its row.
  const n = r.modal && r.modal.items ? r.modal.items.length : 0;
  r.itemCount = n;
  const ix = r.modal && r.modal.items ? r.modal.items.findIndex((i) => i.text.includes(THEME)) : -1;
  r.themeRow = ix;
  if (ix >= 0) {
    for (let i = 0; i < ix; i++) { await c.pressKey('ArrowDown'); await sleep(80); }
    r.selectedBeforeEnter = await c.evaluate(`const s = document.querySelector('.suggestion-item.is-selected'); return s ? s.textContent.trim() : null;`);
    await c.pressKey('Enter');
    await sleep(700);
    r.afterPick = { modal: await modal(), appearance: await readCfg('.obsidian/appearance.json'), theme: await c.evaluate(`return { theme: app.customCss.theme, setting: app.vault.getConfig('cssTheme'), bodyClass: document.body.className };`) };
    // And back to the default through the same command.
    await c.evaluate(`return app.commands.executeCommandById('theme:switch');`);
    await sleep(500);
    r.secondOpen = await modal();
    const dx = r.secondOpen && r.secondOpen.items ? r.secondOpen.items.findIndex((i) => !i.text.includes(THEME)) : -1;
    r.defaultRow = dx;
    if (dx >= 0) {
      for (let i = 0; i < dx; i++) { await c.pressKey('ArrowDown'); await sleep(80); }
      await c.pressKey('Enter');
      await sleep(700);
      r.afterDefault = { appearance: await readCfg('.obsidian/appearance.json'), theme: await c.evaluate(`return { theme: app.customCss.theme, setting: app.vault.getConfig('cssTheme') };`) };
    } else { await esc(); }
  } else {
    await esc();
  }
  r.closed = await modal();
  return r;
});

await step('s04-update-links-buttons', async (r) => {
  r.alwaysUpdateLinks = await c.evaluate(`return app.vault.getConfig('alwaysUpdateLinks');`);
  r.linkerBefore = await readCfg('Linker.md');
  // The basename changes, so a link would change and the prompt is raised.
  r.rename = await c.evaluate(`
    const f = app.vault.getAbstractFileByPath('Alpha.md');
    app.fileManager.renameFile(f, 'Alpha 1.md');
    await window.__p.sleep(900);
    return { modal: window.__p.modal(), files: app.vault.getMarkdownFiles().map((x) => x.path).sort() };`);
  r.ctaButtons = await c.evaluate(`
    const m = document.querySelector('.modal'); if (!m) return null;
    return Array.from(m.querySelectorAll('button')).map((b) => ({ text: b.textContent.trim(), cls: b.className, cta: b.classList.contains('mod-cta'), focused: document.activeElement === b }));`);
  r.activeElement = await c.evaluate(`return window.__p.describe(document.activeElement);`);
  // Enter with nothing focused in particular: which button does it fire?
  await c.pressKey('Enter');
  await sleep(900);
  r.afterEnter = { modal: await modal(), linker: await readCfg('Linker.md'), app: await readCfg('.obsidian/app.json') };
  r.modalAfterEnter = await modal();
  if (r.modalAfterEnter) await esc();
  return r;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
