// Settings → the Daily notes and Templates core-plugin tabs: every row's
// name, description, control and default as Obsidian shows them, and what
// each control writes to `daily-notes.json` / `templates.json` — the key, the
// bytes, and what clearing a field writes back. What the two "Options not
// shown yet" rows in Scoria's Settings wait on.
//
// Reaches the rows through `app.setting.activeTab.containerEl`, as
// frame-setting-key.mjs does; its own script because the Settings UI has hung
// this harness's CDP endpoint before.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT, RESULTS = process.env.RESULTS;
const vp = (rel) => path.join(VAULT, rel);
if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('Templates'), { recursive: true });
  fs.mkdirSync(vp('Journal'), { recursive: true });
  fs.writeFileSync(vp('Templates/Daily.md'), '# {{date}}\n');
  fs.writeFileSync(vp('Note.md'), 'x\n');
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'command-palette': true, 'daily-notes': true, 'templates': true }, null, 2));
  fs.writeFileSync(RESULTS, '{}');
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const c = await connect();
const out = { vault: await c.vaultPath(), startedAt: new Date().toISOString(), tabs: {} };
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(out, null, 1));
const cfg = (name) => { try { return fs.readFileSync(vp('.obsidian/' + name), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
save();

out.version = await c.evaluate(`return { ua: navigator.userAgent, title: document.title };`);

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
      const txt = it.querySelector('input[type="text"], input[type="search"], input:not([type])');
      if (txt) return { kind: 'text', value: txt.value, placeholder: txt.getAttribute('placeholder'), classes: txt.className };
      const btn = it.querySelector('button');
      if (btn) return { kind: 'button', text: btn.textContent };
      return { kind: 'none' };
    })(),
    hidden: it.offsetParent === null,
    classes: it.className,
    controlClasses: (it.querySelector('.setting-item-control') || {}).className || null,
  }));`;

const openTab = async (id) => c.evaluate(`
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  app.setting.open();
  app.setting.openTabById(${JSON.stringify(id)});
  await sleep(1200);
  const tab = app.setting.activeTab;
  return { tabId: tab.id, tabName: tab.name,
    navItems: Array.from(app.setting.containerEl.querySelectorAll('.vertical-tab-nav-item')).map((x) => x.textContent.trim()) };`);

/** Type into the text field of the row named `name` and read the file. */
const typeInto = async (file, name, value) => {
  const before = cfg(file);
  const r = await c.evaluate(`
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const el = app.setting.activeTab.containerEl;
    const row = Array.from(el.querySelectorAll('.setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === ${JSON.stringify(name)});
    if (!row) return { noRow: true };
    const field = row.querySelector('input');
    if (!field) return { noField: true };
    field.focus();
    field.value = ${JSON.stringify(value)};
    field.dispatchEvent(new Event('input', { bubbles: true }));
    field.dispatchEvent(new Event('change', { bubbles: true }));
    field.blur();
    await sleep(1500);
    return { fieldValue: field.value,
      description: ((row.querySelector('.setting-item-description') || {}).textContent || '').trim() };`);
  return { ...r, before, after: cfg(file), changed: cfg(file) !== before };
};

/** Click the toggle of the row named `name` and read the file. */
const clickToggle = async (file, name) => {
  const before = cfg(file);
  const r = await c.evaluate(`
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const el = app.setting.activeTab.containerEl;
    const row = Array.from(el.querySelectorAll('.setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === ${JSON.stringify(name)});
    if (!row) return { noRow: true };
    const tog = row.querySelector('.checkbox-container');
    if (!tog) return { noToggle: true };
    tog.click();
    await sleep(1500);
    return { on: tog.classList.contains('is-enabled') };`);
  return { ...r, before, after: cfg(file), changed: cfg(file) !== before };
};

// ---- Daily notes ----------------------------------------------------------
{
  const t = { file: 'daily-notes.json' };
  t.open = await openTab('daily-notes');
  t.rows = await c.evaluate(ROWS);
  t.fileBefore = cfg(t.file);
  save();
  // The Date format dropdown: what each option writes, and which rows it
  // shows. `Custom` is the question: does picking it write anything, and
  // does the Custom format row come and go with it?
  t.dropdown = {};
  const dd = t.rows.find((r) => r.control.kind === 'dropdown');
  if (dd) {
    for (const opt of [...dd.control.options, dd.control.options[0]]) {
      const before = cfg(t.file);
      const picked = await c.evaluate(`
        const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
        const el = app.setting.activeTab.containerEl;
        const row = Array.from(el.querySelectorAll('.setting-item')).find((it) => (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim() === ${JSON.stringify(dd.name)});
        const sel = row.querySelector('select');
        sel.value = ${JSON.stringify(opt.value)};
        sel.dispatchEvent(new Event('change', { bubbles: true }));
        await sleep(1500);
        return { selectValue: sel.value,
          visible: Array.from(el.querySelectorAll('.setting-item')).map((it) => ({
            name: (((it.querySelector('.setting-item-name') || {}).textContent) || '').trim(),
            shown: it.offsetParent !== null, display: getComputedStyle(it).display,
            text: (() => { const i = it.querySelector('input'); return i ? { value: i.value, placeholder: i.getAttribute('placeholder') } : null; })() })) };`);
      t.dropdown[opt.value] = { label: opt.label, picked, before, after: cfg(t.file), changed: cfg(t.file) !== before };
      save();
    }
  }
  const textRows = t.rows.filter((r) => r.control.kind === 'text').map((r) => r.name);
  const toggles = t.rows.filter((r) => r.control.kind === 'toggle').map((r) => r.name);
  t.typed = {};
  const values = ['YYYY-MM-DD dddd', 'Journal', 'Templates/Daily'];
  for (let i = 0; i < textRows.length; i++) {
    t.typed[textRows[i]] = await typeInto(t.file, textRows[i], values[i] || 'x');
    save();
  }
  t.toggled = {};
  for (const name of toggles) {
    t.toggled[name] = { on: await clickToggle(t.file, name) };
    t.toggled[name].off = await clickToggle(t.file, name);
    save();
  }
  // Clearing: what an emptied field writes.
  t.cleared = {};
  for (const name of textRows) {
    t.cleared[name] = await typeInto(t.file, name, '');
    save();
  }
  t.rowsAfter = await c.evaluate(ROWS);
  t.fileAfter = cfg(t.file);
  out.tabs['daily-notes'] = t;
  save();
}

// ---- Templates ------------------------------------------------------------
{
  const t = { file: 'templates.json' };
  t.open = await openTab('templates');
  t.rows = await c.evaluate(ROWS);
  t.fileBefore = cfg(t.file);
  save();
  const textRows = t.rows.filter((r) => r.control.kind === 'text').map((r) => r.name);
  const toggles = t.rows.filter((r) => r.control.kind === 'toggle').map((r) => r.name);
  t.typed = {};
  const values = ['Templates', 'DD.MM.YYYY', 'HH:mm:ss'];
  for (let i = 0; i < textRows.length; i++) {
    t.typed[textRows[i]] = await typeInto(t.file, textRows[i], values[i] || 'x');
    save();
  }
  t.toggled = {};
  for (const name of toggles) {
    t.toggled[name] = { on: await clickToggle(t.file, name) };
    t.toggled[name].off = await clickToggle(t.file, name);
    save();
  }
  t.cleared = {};
  for (const name of textRows) {
    t.cleared[name] = await typeInto(t.file, name, '');
    save();
  }
  t.rowsAfter = await c.evaluate(ROWS);
  t.fileAfter = cfg(t.file);
  out.tabs.templates = t;
  save();
}

await c.evaluate(`app.setting.close(); return true;`);
await sleep(500);
out.finishedAt = new Date().toISOString();
save();
c.close();
process.exit(0);
