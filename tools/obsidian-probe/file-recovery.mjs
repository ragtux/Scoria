// file-recovery:open and the File recovery core plugin.
//
//   s00  context: version, the commands' names, icons, hotkeys; when the
//        palette offers file-recovery:open; the plugin's options as loaded
//        (file-recovery.json absent)
//   s01  where snapshots live: indexedDB.databases(), each store's name,
//        key path and indexes, and a record's keys and value types
//   s02  when a snapshot is taken: edits through app.vault.modify, before
//        and after the interval, and through the editor
//   s03  the modal: its DOM, the list's rows, a row's preview, the buttons,
//        and Copy / Restore (the bytes written)
//   s04  the Settings tab: the rows, then each number field changed the way
//        typing does (input + change events), file-recovery.json after each,
//        odd values (0, empty, -1, 2.5, abc), "Clear" and "View"
//
// Phases: fixtures | run   (FROM_STEP=s03 runs that step and later)
import fs from 'node:fs';
import path from 'node:path';
import { connect, pageTargets, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const FROM = process.env.FROM_STEP || null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && FROM && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 180000) {
  if (FROM && name < FROM) return null;
  if (!out.order.includes(name)) out.order.push(name);
  const t = Date.now();
  const partial = {};
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), ms); });
  try { const v = await Promise.race([fn(partial), timeout]); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  finally { clearTimeout(timer); }
  save();
  console.error(`[file-recovery] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
async function pollChange(rel, prev, timeout = 6000) {
  const t = Date.now();
  while (Date.now() - t < timeout) { const b = bytes(rel); if (b !== prev) { await sleep(300); return { changed: true, ms: Date.now() - t, bytes: bytes(rel) }; } await sleep(100); }
  return { changed: false, bytes: bytes(rel) };
}

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, backlink: true, 'outgoing-link': true,
    'command-palette': true, 'file-recovery': true, 'page-preview': true, canvas: true,
  }, null, 2));
  fs.writeFileSync(vp('Note.md'), 'Version one.\n');
  fs.writeFileSync(vp('Other.md'), 'Other note.\n');
  fs.writeFileSync(vp('Board.canvas'), JSON.stringify({ nodes: [], edges: [] }));
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }
const ev = (s) => c.evaluate(s);

await ev(`
window.__r = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; },
  skeleton(e, d = 0, max = 8) {
    if (!e) return null;
    const tag = e.tagName.toLowerCase();
    if (['path', 'line', 'circle', 'rect', 'polyline', 'polygon'].includes(tag)) return null;
    const o = { tag, cls: e.getAttribute('class') || undefined };
    for (const a of ['aria-label', 'type', 'placeholder', 'value', 'role']) { const v = e.getAttribute(a); if (v !== null) o[a] = v; }
    if (tag === 'input') o.prop = e.value;
    if (!e.children.length) { const t = (e.textContent || '').trim(); if (t) o.text = t.slice(0, 160); }
    if (d < max && e.children.length) o.children = Array.from(e.children).map((c) => this.skeleton(c, d + 1, max)).filter(Boolean);
    return o;
  },
  offered(id) { return app.commands.listCommands().some((x) => x.id === id); },
  async open(path) {
    const ls = []; app.workspace.iterateRootLeaves((l) => ls.push(l));
    for (const l of ls.slice(1)) l.detach();
    const leaf = ls[0] || app.workspace.getLeaf(false);
    if (path === null) await leaf.setViewState({ type: 'empty', state: {} });
    else await leaf.openFile(app.vault.getAbstractFileByPath(path));
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(600);
    return true;
  },
  async dbs() {
    const list = indexedDB.databases ? await indexedDB.databases() : [];
    const res = [];
    for (const d of list) {
      const info = { name: d.name, version: d.version, stores: [] };
      try {
        const db = await new Promise((ok, no) => { const r = indexedDB.open(d.name); r.onsuccess = () => ok(r.result); r.onerror = () => no(r.error); });
        for (const s of Array.from(db.objectStoreNames)) {
          const tx = db.transaction(s, 'readonly'); const st = tx.objectStore(s);
          const recs = await new Promise((ok) => { const out = []; const cur = st.openCursor(); cur.onsuccess = () => { const c = cur.result; if (!c || out.length >= 50) return ok(out); out.push({ key: c.key, value: c.value }); c.continue(); }; cur.onerror = () => ok(out); });
          const count = await new Promise((ok) => { const r = st.count(); r.onsuccess = () => ok(r.result); r.onerror = () => ok(null); });
          info.stores.push({ name: s, keyPath: st.keyPath, autoIncrement: st.autoIncrement, indexes: Array.from(st.indexNames).map((n) => { const i = st.index(n); return { name: n, keyPath: i.keyPath, unique: i.unique, multiEntry: i.multiEntry }; }), count,
            sample: recs.slice(0, 50).map((r) => ({ key: r.key, value: (r.value && typeof r.value === 'object') ? Object.fromEntries(Object.entries(r.value).map(([k, v]) => [k, typeof v === 'string' ? (v.length > 200 ? v.slice(0, 200) + '…' : v) : v])) : r.value })) });
        }
        db.close();
      } catch (e) { info.error = String(e); }
      res.push(info);
    }
    return res;
  },
  backupStore(dbs) {
    return dbs.filter((d) => d.stores.some((s) => /backup|recovery|snapshot/i.test(s.name + ' ' + d.name)));
  },
};
return true;`);
const R = 'const R = window.__r;';
let shotN = 0;
async function shot(name, cl = c) {
  try {
    const r = await cl.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight] };`);
  const cmds = await ev(`const o = {}; for (const id of Object.keys(app.commands.commands).filter((k) => k.startsWith('file-recovery:'))) { const x = app.commands.commands[id]; o[id] = { name: x.name, icon: x.icon || null, keys: (app.hotkeyManager.defaultKeys || {})[id] || null, hasCheck: !!x.checkCallback, hasCallback: !!x.callback }; } return o;`);
  const plugin = await ev(`const p = app.internalPlugins.getPluginById('file-recovery'); const i = p && p.instance; return p ? { enabled: p.enabled, name: i && i.name, desc: i && i.description, options: i && i.options ? JSON.parse(JSON.stringify(i.options)) : null, instanceKeys: i ? Object.keys(i).sort() : null } : null;`);
  const offered = {};
  offered.note = await ev(`${R} await R.open('Note.md'); return R.offered('file-recovery:open');`);
  offered.canvas = await ev(`${R} await R.open('Board.canvas'); return R.offered('file-recovery:open');`);
  offered.empty = await ev(`${R} await R.open(null); return R.offered('file-recovery:open');`);
  return { version: out.version, cmds, plugin, offered, configFile: bytes('.obsidian/file-recovery.json') };
});

await step('s01-store', async () => {
  const dbs = await ev(`${R} return await R.dbs();`);
  return { dbs };
});

await step('s02-snapshots', async () => {
  const res = {};
  const recs = async () => ev(`${R} const d = await R.dbs(); return d.map((x) => ({ name: x.name, stores: x.stores.map((s) => ({ name: s.name, count: s.count, keys: s.sample.map((r) => r.key), values: s.sample.map((r) => r.value) })) }));`);
  res.before = await recs();
  await ev(`${R} await R.open('Note.md'); return true;`);
  await ev(`await app.vault.modify(app.vault.getAbstractFileByPath('Note.md'), 'Version two.\\n'); return true;`); await sleep(2500);
  res.afterModify1 = await recs();
  await ev(`await app.vault.modify(app.vault.getAbstractFileByPath('Note.md'), 'Version three.\\n'); return true;`); await sleep(2500);
  res.afterModify2 = await recs();
  // Through the editor: type at the end.
  await ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor(ed.lastLine(), ed.getLine(ed.lastLine()).length); return true;`);
  await c.typeText(' Typed'); await sleep(4000);
  res.afterTyping = await recs();
  res.noteBytes = bytes('Note.md');
  // Another file.
  await ev(`await app.vault.modify(app.vault.getAbstractFileByPath('Other.md'), 'Other changed.\\n'); return true;`); await sleep(2500);
  res.afterOther = await recs();
  // Interval 0? Shorten the plugin's interval in memory is not observable as a file; leave it.
  return res;
});

await step('s03-modal', async () => {
  const res = {};
  await ev(`window.__r.open('Note.md'); return true;`); await sleep(800);
  res.ran = await ev(`return app.commands.executeCommandById('file-recovery:open');`); await sleep(1200);
  res.dom = await ev(`${R} const m = document.querySelector('.modal-container'); return m ? R.skeleton(m, 0, 10) : null;`);
  res.shot = await shot('modal');
  // The rows of the list.
  res.rows = await ev(`${R} return Array.from(document.querySelectorAll('.modal-container .suggestion-item, .modal-container .file-recovery-list *[class*=item], .modal-container .tree-item-self, .modal-container .nav-file-title')).map((e) => ({ cls: R.cls(e), text: (e.textContent || '').trim().slice(0, 120), rect: R.rect(e) }));`);
  // Type in the search box if there is one.
  const input = await c.rect('.modal-container input');
  if (input) {
    await c.clickAt(input.cx, input.cy); await sleep(300);
    await c.typeText('Note'); await sleep(800);
    res.afterSearch = await ev(`${R} const m = document.querySelector('.modal-container'); return R.skeleton(m, 0, 10);`);
    res.searchShot = await shot('modal-search');
    await c.pressKey('Enter'); await sleep(1200);
    res.afterEnter = await ev(`${R} const m = document.querySelector('.modal-container'); return m ? R.skeleton(m, 0, 12) : null;`);
    res.enterShot = await shot('modal-file');
  }
  // Buttons in the modal now.
  res.buttons = await ev(`${R} return Array.from(document.querySelectorAll('.modal-container button, .modal-container .clickable-icon')).map((b) => ({ cls: R.cls(b), text: (b.textContent || '').trim(), aria: b.getAttribute('aria-label'), rect: R.rect(b) }));`);
  // Click the oldest snapshot row if a list of versions shows.
  const versions = await ev(`${R} return Array.from(document.querySelectorAll('.modal-container [class*=recovery] [class*=item], .modal-container .file-recovery-history-item, .modal-container .tree-item-self')).map((e) => ({ cls: R.cls(e), text: (e.textContent || '').trim(), rect: R.rect(e) }));`);
  res.versions = versions;
  if (versions.length) {
    const v = versions[versions.length - 1];
    await c.clickAt(v.rect[0] + v.rect[2] / 2, v.rect[1] + v.rect[3] / 2); await sleep(900);
    res.afterVersionClick = await ev(`${R} const m = document.querySelector('.modal-container'); return m ? R.skeleton(m, 0, 12) : null;`);
    res.versionShot = await shot('modal-version');
  }
  // Restore, if a button says so.
  const before = bytes('Note.md');
  const restore = await ev(`${R} const b = Array.from(document.querySelectorAll('.modal-container button')).find((x) => /restore/i.test(x.textContent)); if (!b) return null; const r = b.getBoundingClientRect(); return { text: b.textContent.trim(), cx: r.x + r.width / 2, cy: r.y + r.height / 2 };`);
  res.restoreButton = restore;
  if (restore) {
    await c.clickAt(restore.cx, restore.cy); await sleep(1200);
    res.afterRestoreUi = await ev(`${R} return { modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.textContent || '').trim().slice(0, 300)), notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent) };`);
    res.afterRestoreShot = await shot('after-restore');
    // A confirmation?
    const confirm = await ev(`const b = Array.from(document.querySelectorAll('.modal-container button')).find((x) => /restore|yes|confirm/i.test(x.textContent)); if (!b) return null; const r = b.getBoundingClientRect(); return { text: b.textContent.trim(), cx: r.x + r.width / 2, cy: r.y + r.height / 2 };`);
    if (confirm) { await c.clickAt(confirm.cx, confirm.cy); await sleep(1200); res.confirmClicked = confirm; }
    res.restored = await pollChange('Note.md', before, 5000);
  }
  await c.pressKey('Escape'); await sleep(400); await c.pressKey('Escape'); await sleep(400);
  return res;
});

await step('s04-settings', async () => {
  const res = {};
  await ev(`app.setting.open(); app.setting.openTabById('file-recovery'); return true;`); await sleep(1500);
  const targets = await pageTargets();
  res.targets = targets.length;
  // The tab's DOM wherever it lives.
  res.dom = await ev(`${R} const el = app.setting.activeTab && app.setting.activeTab.containerEl; return el ? { doc: el.ownerDocument === document ? 'main' : 'other', skeleton: R.skeleton(el, 0, 9) } : null;`);
  const setField = async (index, value) => ev(`const el = app.setting.activeTab.containerEl; const inputs = Array.from(el.querySelectorAll('input')); const i = inputs[${index}]; if (!i) return null; i.focus(); i.value = ${JSON.stringify(value)}; i.dispatchEvent(new Event('input', { bubbles: true })); i.dispatchEvent(new Event('change', { bubbles: true })); i.blur(); return { placeholder: i.placeholder, type: i.type, value: i.value };`);
  const rows = await ev(`const el = app.setting.activeTab.containerEl; return Array.from(el.querySelectorAll('.setting-item')).map((s) => ({ name: (s.querySelector('.setting-item-name') || {}).textContent, desc: (s.querySelector('.setting-item-description') || {}).textContent, inputs: Array.from(s.querySelectorAll('input')).map((i) => ({ type: i.type, value: i.value, placeholder: i.placeholder, min: i.min, max: i.max, step: i.step })), buttons: Array.from(s.querySelectorAll('button')).map((b) => ({ text: b.textContent, cls: b.className })) }));`);
  res.rows = rows;
  const seq = [[0, '10'], [1, '30'], [0, '0'], [0, ''], [0, '-1'], [0, '2.5'], [0, 'abc'], [1, '0'], [1, ''], [0, '5'], [1, '7']];
  res.seq = [];
  let prev = bytes('.obsidian/file-recovery.json');
  for (const [ix, v] of seq) {
    const set = await setField(ix, v);
    const ch = await pollChange('.obsidian/file-recovery.json', prev, 4000);
    prev = ch.bytes;
    const opts = await ev(`return JSON.parse(JSON.stringify(app.internalPlugins.getPluginById('file-recovery').instance.options));`);
    const shown = await ev(`return Array.from(app.setting.activeTab.containerEl.querySelectorAll('input')).map((i) => i.value);`);
    res.seq.push({ field: ix, typed: v, set, file: ch, options: opts, shown });
  }
  // "View" opens the modal? "Clear" asks?
  const clickButton = async (text) => ev(`const el = app.setting.activeTab.containerEl; const b = Array.from(el.querySelectorAll('button')).find((x) => x.textContent.trim() === ${JSON.stringify(text)}); if (!b) return null; b.click(); return true;`);
  res.view = await clickButton('View'); await sleep(1200);
  res.afterView = await ev(`return { modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.querySelector('.modal-title') || m).textContent.trim().slice(0, 200)), settingOpen: !!document.querySelector('.modal.mod-settings') };`);
  res.afterViewShot = await shot('after-view');
  await c.pressKey('Escape'); await sleep(500);
  await ev(`app.setting.open(); app.setting.openTabById('file-recovery'); return true;`); await sleep(1000);
  const dbBefore = await ev(`${R} const d = await R.dbs(); return d.map((x) => ({ name: x.name, counts: x.stores.map((s) => [s.name, s.count]) }));`);
  res.clear = await clickButton('Clear'); await sleep(1200);
  res.afterClearUi = await ev(`return { modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.textContent || '').trim().slice(0, 300)), buttons: Array.from(document.querySelectorAll('.modal-container button')).map((b) => b.textContent.trim()), notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent) };`);
  res.afterClearShot = await shot('after-clear');
  const yes = await ev(`const bs = Array.from(document.querySelectorAll('.modal-container button')); const b = bs.find((x) => /clear|delete|yes|confirm/i.test(x.textContent) && x.classList.contains('mod-warning')) || bs.find((x) => x.classList.contains('mod-cta') || x.classList.contains('mod-warning')); if (!b) return null; const t = b.textContent.trim(); b.click(); return t;`);
  res.confirmed = yes; await sleep(1200);
  res.dbAfterClear = await ev(`${R} const d = await R.dbs(); return d.map((x) => ({ name: x.name, counts: x.stores.map((s) => [s.name, s.count]) }));`);
  res.dbBeforeClear = dbBefore;
  res.notices = await ev(`return Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent);`);
  res.finalFile = bytes('.obsidian/file-recovery.json');
  await ev(`app.setting.close(); return true;`);
  return res;
});

// s04's synthetic input events did not reach the Settings window's fields
// (it is a window of its own, a second CDP target). Real keys there.
await step('s05-settings-typed', async () => {
  const res = {};
  const before = await pageTargets();
  await ev(`app.setting.open(); app.setting.openTabById('file-recovery'); return true;`); await sleep(2000);
  const after = await pageTargets();
  const url = after.find((u) => !before.includes(u)) || after.find((u, i) => i > 0);
  res.settingsTarget = !!url;
  if (!url) return res;
  const s = await connect(url);
  res.title = await s.evaluate(`return document.title;`);
  const inputs = () => s.evaluate(`return Array.from(document.querySelectorAll('.vertical-tab-content input[type=number]')).map((i) => { const r = i.getBoundingClientRect(); return { value: i.value, cx: r.x + r.width / 2, cy: r.y + r.height / 2 }; });`);
  const seq = [[0, '10'], [1, '30'], [0, '0'], [0, ''], [0, '-1'], [0, '2.5'], [1, '0'], [1, ''], [1, '0.5'], [0, '5'], [1, '7']];
  res.seq = [];
  let prev = bytes('.obsidian/file-recovery.json');
  for (const [ix, v] of seq) {
    const ins = await inputs();
    const i = ins[ix];
    await s.clickAt(i.cx, i.cy); await sleep(200);
    await s.pressKey('a', { ctrl: true }); await sleep(100);
    await s.pressKey('Backspace'); await sleep(100);
    if (v) await s.typeText(v);
    await sleep(300);
    const ch = await pollChange('.obsidian/file-recovery.json', prev, 4000);
    await s.pressKey('Tab'); await sleep(300);
    const ch2 = ch.changed ? ch : await pollChange('.obsidian/file-recovery.json', prev, 3000);
    prev = ch2.bytes;
    res.seq.push({ field: ix, typed: v, file: ch2, afterTab: !ch.changed, options: await ev(`return JSON.parse(JSON.stringify(app.internalPlugins.getPluginById('file-recovery').instance.options));`), shown: (await inputs()).map((x) => x.value) });
  }
  // View: a real click.
  const btn = async (text) => s.evaluate(`const b = Array.from(document.querySelectorAll('.vertical-tab-content button')).find((x) => x.textContent.trim() === ${JSON.stringify(text)}); if (!b) return null; const r = b.getBoundingClientRect(); return { cx: r.x + r.width / 2, cy: r.y + r.height / 2 };`);
  const clear = await btn('Clear');
  if (clear) {
    await s.clickAt(clear.cx, clear.cy); await sleep(1200);
    res.afterClear = await s.evaluate(`return { modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.textContent || '').trim().slice(0, 400)), buttons: Array.from(document.querySelectorAll('.modal-container button')).map((b) => ({ text: b.textContent.trim(), cls: b.className })) };`);
    res.afterClearMain = await ev(`return { modals: Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.textContent || '').trim().slice(0, 400)), notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent) };`);
    res.afterClearShot = await shot('settings-clear', s);
    // Cancel first if there is a confirmation, then do it for real.
    const cancel = await s.evaluate(`const b = Array.from(document.querySelectorAll('.modal-container button')).find((x) => /cancel/i.test(x.textContent)); if (!b) return null; b.click(); return b.textContent.trim();`);
    res.cancelled = cancel; await sleep(600);
    res.dbAfterCancel = await ev(`${R} const d = await R.dbs(); return d.filter((x) => /backup/.test(x.name)).map((x) => x.stores.map((st) => [st.name, st.count]));`);
    if (cancel) {
      await s.clickAt(clear.cx, clear.cy); await sleep(1000);
      res.confirm = await s.evaluate(`const b = Array.from(document.querySelectorAll('.modal-container button')).find((x) => !/cancel/i.test(x.textContent)); if (!b) return null; const t = b.textContent.trim(); const c = b.className; b.click(); return { text: t, cls: c };`);
      await sleep(1200);
    }
    res.dbAfterClear = await ev(`${R} const d = await R.dbs(); return d.filter((x) => /backup/.test(x.name)).map((x) => x.stores.map((st) => [st.name, st.count]));`);
    res.noticesAfterClear = await ev(`return Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent);`);
  }
  const view = await btn('View');
  if (view) {
    await s.clickAt(view.cx, view.cy); await sleep(1500);
    res.afterView = { settingsModals: await s.evaluate(`return Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.textContent || '').trim().slice(0, 200));`).catch((e) => 'gone: ' + e.message),
      mainModals: await ev(`return Array.from(document.querySelectorAll('.modal-container')).map((m) => (m.querySelector('.modal-title') || m).textContent.trim().slice(0, 200));`),
      targets: (await pageTargets()).length };
    res.afterViewShot = await shot('after-view-main');
  }
  try { s.close(); } catch { /* ignore */ }
  res.finalFile = bytes('.obsidian/file-recovery.json');
  return res;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
