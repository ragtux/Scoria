// Settings -> Appearance, second visit: the three font pages and the two rows
// `appearance-tab.mjs` recorded but did not click.
//
// The font pages: what one, two and three picks write to `interfaceFontFamily`
// (separator, quoting, order), what a family with a space in its name looks
// like, what typing a name that is not installed offers, what the Delete
// button and the drag handle do to the list, what deleting the last one
// writes, and the same for `textFontFamily` and `monospaceFontFamily`.
//
// Quick font size adjustment: the key the toggle writes, and what Ctrl+wheel
// over a note does to `baseFontSize` (step, clamps, write) with it on and off.
//
// Custom app icon: Choose is a file chooser; the popout's chooser is
// intercepted (`Page.setInterceptFileChooserDialog`) and handed a PNG, and
// the Electron IPC calls the renderer makes are logged, then every file the
// click changed — in the vault and in the user-data dir — is diffed.
//
// Same two-handle trick as appearance-tab.mjs: the vault window's `app` finds
// the popout's rows, a bare client on the popout's target clicks them.
//
// Phases: fixtures | run.  Env: VAULT, RESULTS, UDD (the --user-data-dir),
// CDP_BASE. ONLY_STEPS=s01,s02 re-runs steps.
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const UDD = process.env.UDD || null;
const BASE = process.env.CDP_BASE || 'http://127.0.0.1:9222';
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const SHOTS = process.env.SHOTS || RESULTS.replace(/\.json$/, '-shots');
const vp = (rel) => path.join(VAULT, rel);
const bytes = (abs) => { try { return fs.readFileSync(abs, 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
const appearanceJson = () => bytes(vp('.obsidian/appearance.json'));

const out = fs.existsSync(RESULTS) ? JSON.parse(fs.readFileSync(RESULTS, 'utf8')) : {};
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(out, null, 1));
const only = (process.env.ONLY_STEPS || '').split(',').filter(Boolean);
const step = async (id, fn) => {
  if (only.length && !only.includes(id)) return;
  try { out[id] = await fn(); } catch (e) { out[id] = { ERR: e.message, stack: e.stack }; }
  save();
  console.error('[font-pages] ' + id);
};

// A 64x64 opaque PNG, written by hand (no dependencies).
function png(w, h, rgb) {
  const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = (buf) => { let c = 0xffffffff; for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([len, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) raw.set(rgb, y * (w * 3 + 1) + 1 + x * 3);
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
const ICON = path.join(path.dirname(VAULT), 'probe-icon.png');

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), '# Alpha\n\nSome text to zoom.\n\n```js\nconst x = 1;\n```\n');
  // QUICK_ON=1 starts with Quick font size adjustment on, for s09 alone.
  fs.writeFileSync(vp('.obsidian/appearance.json'), process.env.QUICK_ON ? '{\n  "baseFontSizeAction": true\n}' : '{}\n');
  fs.writeFileSync(ICON, png(64, 64, [0xe8, 0x5d, 0x04]));
  out.fixtures = { icon: ICON };
  save();
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

// ---- every file under a folder, with its size and mtime --------------------
const listTree = (root) => {
  const res = {};
  const walk = (dir) => {
    let ents = [];
    try { ents = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const e of ents) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) { if (!/^(Cache|Code Cache|GPUCache|DawnGraphiteCache|DawnWebGPUCache|blob_storage|Crashpad)$/.test(e.name)) walk(p); continue; }
      try { const s = fs.statSync(p); res[path.relative(root, p)] = { size: s.size, mtime: s.mtimeMs }; } catch {}
    }
  };
  walk(root);
  return res;
};
const treeDiff = (a, b) => {
  const d = { added: [], removed: [], changed: [] };
  for (const k of Object.keys(b)) if (!(k in a)) d.added.push(k); else if (a[k].size !== b[k].size || a[k].mtime !== b[k].mtime) d.changed.push(k);
  for (const k of Object.keys(a)) if (!(k in b)) d.removed.push(k);
  return d;
};
const snapshot = () => ({ vault: listTree(VAULT), udd: UDD ? listTree(UDD) : {} });
const diffSnap = (a, b) => ({ vault: treeDiff(a.vault, b.vault), udd: treeDiff(a.udd, b.udd) });
const textOf = (d) => {
  const o = {};
  for (const k of [...d.vault.added, ...d.vault.changed]) o['vault/' + k] = bytes(path.join(VAULT, k));
  for (const k of [...d.udd.added, ...d.udd.changed]) if (/\.json$/.test(k) || !/\./.test(path.basename(k))) { const b = bytes(path.join(UDD, k)); o['udd/' + k] = b && b.length < 4000 ? b : (b ? b.slice(0, 400) + '...' : b); }
  return o;
};

// ---- a bare client for the Settings popout --------------------------------
async function pageTargets() {
  const ts = await (await fetch(`${BASE}/json`)).json();
  return ts.filter((x) => x.type === 'page');
}
async function attach(wsUrl) {
  const ws = new WebSocket(wsUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let id = 0;
  const pending = new Map();
  const listeners = [];
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const { res, rej } = pending.get(m.id);
      pending.delete(m.id);
      m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
    } else if (m.method) for (const l of listeners) l(m);
  };
  const send = (method, params = {}) => new Promise((res, rej) => {
    const i = ++id;
    pending.set(i, { res, rej });
    ws.send(JSON.stringify({ id: i, method, params }));
    setTimeout(() => { if (pending.has(i)) { pending.delete(i); rej(new Error(method + ' timeout')); } }, 15000);
  });
  const evaluate = async (expr) => {
    const r = await send('Runtime.evaluate', { expression: `(async () => { ${expr} })()`, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) {
      const d = r.exceptionDetails;
      throw new Error((d.exception && d.exception.description) || d.text || JSON.stringify(d).slice(0, 400));
    }
    return r.result.value;
  };
  // A window behind another one stops answering input events (Electron
  // throttles an occluded renderer), so every click raises its window first.
  const clickAt = async (x, y) => {
    await send('Page.bringToFront').catch(() => null);
    await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, buttons: 0 });
    for (const type of ['mousePressed', 'mouseReleased']) {
      await send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1, buttons: type === 'mousePressed' ? 1 : 0 });
      await sleep(40);
    }
  };
  const drag = async (x1, y1, x2, y2, steps = 14) => {
    await send('Page.bringToFront').catch(() => null);
    await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x1, y: y1, buttons: 0 });
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: x1, y: y1, button: 'left', clickCount: 1, buttons: 1 });
    for (let i = 1; i <= steps; i++) {
      await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x1 + ((x2 - x1) * i) / steps, y: y1 + ((y2 - y1) * i) / steps, button: 'left', buttons: 1 });
      await sleep(50);
    }
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: x2, y: y2, button: 'left', clickCount: 1, buttons: 0 });
  };
  const typeText = async (s) => { for (const ch of s) { await send('Input.dispatchKeyEvent', { type: 'keyDown', text: ch, unmodifiedText: ch, key: ch }); await send('Input.dispatchKeyEvent', { type: 'keyUp', key: ch }); await sleep(25); } };
  const shot = async (name) => {
    const r = await send('Page.captureScreenshot', { format: 'png' });
    fs.mkdirSync(SHOTS, { recursive: true });
    fs.writeFileSync(path.join(SHOTS, name), Buffer.from(r.data, 'base64'));
    return name;
  };
  return { send, evaluate, clickAt, drag, typeText, shot, on: (fn) => listeners.push(fn), close: () => ws.close() };
}

const vault = await connect();
out.vaultPath = await vault.vaultPath();
out.version = await vault.evaluate(`let v = null; try { v = require('electron').ipcRenderer.sendSync('version'); } catch {} return { version: v, ua: navigator.userAgent, platform: process.platform };`).catch((e) => String(e));
save();
await vault.evaluate(`await app.workspace.openLinkText('Alpha', '', false); return true;`).catch(() => null);
await sleep(800);

const findPopout = async () => {
  for (const t of await pageTargets()) {
    let cand = null;
    try {
      cand = await attach(t.webSocketDebuggerUrl);
      if (await cand.evaluate(`return !!document.querySelector('.vertical-tab-content') && !document.querySelector('.workspace-split.mod-root');`)) return cand;
      cand.close();
    } catch { if (cand) try { cand.close(); } catch {} }
  }
  return null;
};
await vault.evaluate(`app.setting.open(); app.setting.openTabById('appearance'); return true;`);
await sleep(1500);
let pop = await findPopout();
out.foundPopout = !!pop;
save();
if (!pop) { console.error('[font-pages] no Settings popout'); process.exit(3); }

const PAGE_JS = `
  const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x:+b.x.toFixed(2), y:+b.y.toFixed(2), w:+b.width.toFixed(2), h:+b.height.toFixed(2) }; };
  const pane = document.querySelector('.vertical-tab-content-container') || document.body;
  const nameOf = (it) => ((it.querySelector('.setting-item-name')||{}).textContent||'').trim();
  const visible = (el) => el && el.offsetParent !== null && getComputedStyle(el).display !== 'none';
`;
// The page as data: description, the applied list, the installed list.
const page = () => pop.evaluate(`${PAGE_JS}
  const title = ((pane.querySelector('.setting-page-title')||{}).textContent||'').trim() || null;
  const desc = ((pane.querySelector('.setting-page-desc')||{}).textContent||'').trim() || null;
  const groups = Array.from(pane.querySelectorAll('.setting-group'));
  const g = groups.map((grp) => ({
    cls: grp.className,
    search: (() => { const i = grp.querySelector('input'); return i ? { placeholder: i.getAttribute('placeholder'), value: i.value } : null; })(),
    items: Array.from(grp.querySelectorAll('.setting-item')).map((it) => ({
      name: nameOf(it), cls: it.className, shown: visible(it),
      style: (it.querySelector('.setting-item-name')||{}).getAttribute ? it.querySelector('.setting-item-name').getAttribute('style') : null,
      buttons: Array.from(it.querySelectorAll('.clickable-icon')).map((b) => ({ aria: b.getAttribute('aria-label'), cls: b.className })),
    })),
  }));
  return { title, desc, groups: g.map((x) => ({ ...x, items: x.items.length > 40 ? [...x.items.filter((i) => i.shown).slice(0, 30), { more: x.items.length }] : x.items })), installedCount: g[1] ? g[1].items.length : null };`);
const pageHtml = () => pop.evaluate(`${PAGE_JS} const p = pane.querySelector('.setting-page'); const h = p ? p.innerHTML : pane.innerHTML; return h.length > 8000 ? h.slice(0, 8000) + '...' : h;`);
const clickRowNamed = async (name) => {
  const r = await pop.evaluate(`${PAGE_JS}
    const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => nameOf(x) === ${JSON.stringify(name)} && visible(x));
    if (it) it.scrollIntoView({ block: 'center' });
    return it ? rect(it) : null;`);
  if (!r) return false;
  await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2);
  await sleep(700);
  return true;
};
// Click a family in the installed list (the second group).
const pickFamily = async (name) => {
  const r = await pop.evaluate(`${PAGE_JS}
    const groups = Array.from(pane.querySelectorAll('.setting-group'));
    const g = groups[groups.length - 1];
    const it = Array.from(g.querySelectorAll('.setting-item')).find((x) => nameOf(x) === ${JSON.stringify(name)} && visible(x));
    if (!it) return null;
    it.scrollIntoView({ block: 'center' });
    return rect(it);`);
  if (!r) return { name, found: false };
  const before = appearanceJson();
  await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2);
  return { name, appearanceJson: await waitWrite(before), page: await page() };
};
const waitWrite = async (before, ms = 3000) => {
  const end = Date.now() + ms;
  while (Date.now() < end && appearanceJson() === before) await sleep(120);
  await sleep(400);
  return appearanceJson();
};
// Click the n-th applied row's button by aria-label.
const clickApplied = async (name, aria) => {
  const r = await pop.evaluate(`${PAGE_JS}
    const g = pane.querySelector('.setting-group');
    const it = Array.from(g.querySelectorAll('.setting-item')).find((x) => nameOf(x) === ${JSON.stringify(name)});
    const b = it && Array.from(it.querySelectorAll('.clickable-icon')).find((b) => b.getAttribute('aria-label') === ${JSON.stringify(aria)});
    return b ? rect(b) : null;`);
  return r;
};
const appliedRects = () => pop.evaluate(`${PAGE_JS}
  const g = pane.querySelector('.setting-group');
  return Array.from(g.querySelectorAll('.setting-item')).map((it) => ({ name: nameOf(it), rect: rect(it), handle: rect(it.querySelector('.mod-drag-handle')) }));`);
const goBack = async () => {
  const r = await pop.evaluate(`${PAGE_JS} const b = pane.querySelector('.setting-page-back-button'); return b ? rect(b) : null;`);
  if (r) { await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2); await sleep(700); return true; }
  return false;
};
const toAppearance = async () => {
  await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(300);
  await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(900);
};
const installedNames = () => pop.evaluate(`${PAGE_JS}
  const groups = Array.from(pane.querySelectorAll('.setting-group'));
  const g = groups[groups.length - 1];
  return Array.from(g.querySelectorAll('.setting-item')).map((it) => ({ name: nameOf(it), shown: visible(it), style: it.querySelector('.setting-item-name').getAttribute('style') }));`);

// The installed list fills in asynchronously after the page opens.
const waitList = async (ms = 15000) => {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    const n = await pop.evaluate(`${PAGE_JS} const groups = Array.from(pane.querySelectorAll('.setting-group')); const g = groups[groups.length - 1]; return g ? g.querySelectorAll('.setting-item').length : 0;`);
    if (n > 1) return Date.now() - t0;
    await sleep(250);
  }
  return null;
};

// ---- s01 the Interface font page, empty --------------------------------
await step('s01-interface-empty', async () => {
  await toAppearance();
  const res = { opened: await clickRowNamed('Interface font') };
  res.waitedMs = await waitList();
  res.page = await page();
  res.html = await pageHtml();
  res.installed = await installedNames();
  res.shot = await pop.shot('interface-empty.png');
  // How the collapsed row on the tab reads, for later comparison.
  return res;
});

// ---- s02 three picks, one with a space in its name ----------------------
await step('s02-three-picks', async () => {
  const names = (await installedNames()).map((x) => x.name).filter(Boolean);
  const want = ['Segoe UI', 'Arial', 'Courier New', 'Georgia', 'Verdana'].filter((n) => names.includes(n)).slice(0, 3);
  const res = { want, picks: [] };
  for (const n of want) res.picks.push(await pickFamily(n));
  res.html = await pageHtml();
  res.config = await vault.evaluate(`return app.vault.getConfig('interfaceFontFamily');`);
  res.cssVar = await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--font-interface-override').trim() + ' | ' + getComputedStyle(document.body).getPropertyValue('--font-interface').trim();`);
  res.bodyStyle = await vault.evaluate(`return document.body.getAttribute('style');`);
  res.shot = await pop.shot('interface-three.png');
  return res;
});

// ---- s03 a name typed into the search box -------------------------------
await step('s03-typed-name', async () => {
  const res = {};
  const r = await pop.evaluate(`${PAGE_JS}
    const groups = Array.from(pane.querySelectorAll('.setting-group'));
    const i = groups[groups.length - 1].querySelector('input'); if (!i) return null; i.scrollIntoView({ block: 'center' }); return rect(i);`);
  if (!r) return { noInput: true };
  await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2);
  await pop.typeText('Probe Sans');
  await sleep(500);
  res.page = await page();
  res.installedShown = (await installedNames()).filter((x) => x.shown);
  res.shot = await pop.shot('interface-typed.png');
  // The typed name is offered as the first (otherwise hidden) row; pick it.
  const shown = res.installedShown.find((x) => /Probe Sans/.test(x.name));
  if (shown) {
    const before = appearanceJson();
    await clickRowNamed(shown.name);
    res.afterPick = { appearanceJson: await waitWrite(before), page: await page() };
  }
  // A typed name that matches an installed family, partially.
  await pop.evaluate(`${PAGE_JS} const groups = Array.from(pane.querySelectorAll('.setting-group')); const i = groups[groups.length - 1].querySelector('input'); i.focus(); i.select(); return true;`);
  await pop.typeText('cour');
  await sleep(500);
  res.partial = (await installedNames()).filter((x) => x.shown);
  // Enter in the search box.
  await pop.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' });
  await pop.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  await sleep(700);
  res.afterEnter = { appearanceJson: appearanceJson(), page: await page() };
  // Clear the search.
  await pop.evaluate(`${PAGE_JS} const groups = Array.from(pane.querySelectorAll('.setting-group')); const i = groups[groups.length - 1].querySelector('input'); i.value=''; i.dispatchEvent(new Event('input', {bubbles:true})); return true;`);
  await sleep(300);
  return res;
});

// ---- s04 the drag handle: last row to the top ----------------------------
await step('s04-reorder', async () => {
  const res = { before: await appliedRects(), json: appearanceJson() };
  const rows = res.before;
  if (rows.length < 2 || !rows[rows.length - 1].handle) return { ...res, skipped: 'fewer than two rows' };
  const last = rows[rows.length - 1].handle;
  const first = rows[0].rect;
  const before = appearanceJson();
  await pop.drag(last.x + last.w / 2, last.y + last.h / 2, last.x + last.w / 2, first.y + 4);
  res.after = { appearanceJson: await waitWrite(before), rows: await appliedRects() };
  res.shot = await pop.shot('interface-reordered.png');
  return res;
});

// ---- s05 Delete, the middle one then the rest -----------------------------
await step('s05-delete', async () => {
  const res = { tries: [] };
  for (let guard = 0; guard < 6; guard++) {
    const rows = await appliedRects();
    if (!rows.length) break;
    const victim = rows[Math.floor(rows.length / 2)].name;
    const r = await clickApplied(victim, 'Delete');
    if (!r) { res.tries.push({ victim, noButton: true }); break; }
    const before = appearanceJson();
    await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2);
    res.tries.push({ victim, appearanceJson: await waitWrite(before), config: await vault.evaluate(`return app.vault.getConfig('interfaceFontFamily');`), page: await page() });
  }
  res.emptyShot = await pop.shot('interface-emptied.png');
  await goBack();
  return res;
});

// ---- s06 the collapsed rows on the tab, with values set ------------------
await step('s06-text-and-mono', async () => {
  const res = {};
  for (const row of ['Text font', 'Monospace font']) {
    await toAppearance();
    const r = { opened: await clickRowNamed(row) };
    r.waitedMs = await waitList();
    r.page = await page();
    const names = (await installedNames()).map((x) => x.name);
    const two = ['Consolas', 'Cascadia Mono', 'Georgia', 'Times New Roman', 'Courier New'].filter((n) => names.includes(n)).slice(0, row === 'Text font' ? 2 : 1);
    r.picks = [];
    for (const n of two) r.picks.push(await pickFamily(n));
    r.shot = await pop.shot(row.replace(/ /g, '-').toLowerCase() + '.png');
    await goBack();
    res[row] = r;
  }
  await sleep(500);
  res.tabRows = await pop.evaluate(`${PAGE_JS}
    return Array.from(pane.querySelectorAll('.setting-item')).filter((it) => /font/i.test(nameOf(it))).map((it) => ({ name: nameOf(it), value: ((it.querySelector('.setting-item-value')||{}).textContent||'').trim(), desc: ((it.querySelector('.setting-item-description')||{}).textContent||'').trim(), cls: it.className, html: it.innerHTML.slice(0, 1200) }));`);
  res.tabShot = await pop.shot('appearance-fonts-set.png');
  res.appearanceJson = appearanceJson();
  return res;
});

// ---- s07 Quick font size adjustment -------------------------------------
// `Input.dispatchMouseEvent` with `mouseWheel` never returned on Windows (the
// browser waits for an ack the throttled renderer does not send), so the
// wheel is a WheelEvent dispatched on the note: Obsidian's listener is plain
// JS and reads `ctrlKey` and `deltaY` off it.
const wheel = async (deltaY, ctrl, n = 1) => {
  for (let i = 0; i < n; i++) {
    await vault.evaluate(`const el = document.querySelector('.workspace-leaf.mod-active .view-content') || document.querySelector('.view-content');
      const b = el.getBoundingClientRect();
      const ev = new WheelEvent('wheel', { deltaY: ${deltaY}, deltaMode: 0, ctrlKey: ${ctrl}, clientX: b.x + b.width / 2, clientY: b.y + b.height / 2, bubbles: true, cancelable: true });
      const target = document.elementFromPoint(b.x + b.width / 2, b.y + b.height / 2) || el;
      target.dispatchEvent(ev);
      return ev.defaultPrevented;`);
    await sleep(120);
  }
  await sleep(1200);
};
const fontState = () => vault.evaluate(`
  let zoom = null; try { zoom = require('electron').webFrame.getZoomFactor(); } catch {}
  return { baseFontSize: app.vault.getConfig('baseFontSize'), cssVar: getComputedStyle(document.body).getPropertyValue('--font-text-size').trim(), zoom, config: app.vault.config ? Object.keys(app.vault.config) : null };`);
await step('s07-quick-font-size', async () => {
  await toAppearance();
  const res = {};
  res.row = await pop.evaluate(`${PAGE_JS}
    const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => /quick font size/i.test(nameOf(x)));
    if (!it) return null; it.scrollIntoView({ block: 'center' });
    const t = it.querySelector('.checkbox-container');
    return { name: nameOf(it), desc: ((it.querySelector('.setting-item-description')||{}).textContent||'').trim(), on: t && t.classList.contains('is-enabled'), toggle: rect(t) };`);
  res.offState = await fontState();
  // The toggle.
  const snapA = snapshot();
  const before = appearanceJson();
  const t = res.row && res.row.toggle;
  if (t) await pop.clickAt(t.x + t.w / 2, t.y + t.h / 2);
  res.toggleOn = { appearanceJson: await waitWrite(before) };
  await sleep(800);
  const snapB = snapshot();
  res.toggleOn.diff = diffSnap(snapA, snapB);
  res.toggleOn.text = textOf(res.toggleOn.diff);
  res.toggleOn.localStorage = await vault.evaluate(`const o = {}; for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); if (/font|zoom|size/i.test(k)) o[k] = localStorage.getItem(k); } return o;`);
  // Close Settings: wheel events into a window behind the popout do not return.
  await vault.evaluate(`app.setting.close(); return true;`); await sleep(800);
  // Ctrl+wheel with it on: up 3 notches, then down 6, one at a time.
  res.onWheel = [];
  for (const [dy, n] of [[-120, 1], [-120, 1], [-120, 1], [120, 1], [120, 1], [120, 1], [120, 1], [120, 1], [120, 1]]) {
    await wheel(dy, true, n);
    res.onWheel.push({ dy, appearanceJson: appearanceJson(), state: await fontState() });
  }
  // A small delta (a trackpad's), and a plain wheel without Ctrl.
  await wheel(-10, true, 1);
  res.smallDelta = { appearanceJson: appearanceJson(), state: await fontState() };
  await wheel(-120, false, 2);
  res.plainWheel = { appearanceJson: appearanceJson(), state: await fontState() };
  // Clamps: far up, then far down.
  await wheel(-120, true, 40);
  res.clampUp = { appearanceJson: appearanceJson(), state: await fontState() };
  await wheel(120, true, 60);
  res.clampDown = { appearanceJson: appearanceJson(), state: await fontState() };
  // The slider on the tab follows?
  await vault.evaluate(`app.setting.open(); app.setting.openTabById('appearance'); return true;`); await sleep(1500);
  pop.close(); pop = await findPopout();
  await toAppearance();
  res.sliderRow = await pop.evaluate(`${PAGE_JS}
    const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => nameOf(x) === 'Font size');
    if (!it) return null; const s = it.querySelector('input.slider');
    return { value: s && s.value, min: s && s.min, max: s && s.max, step: s && s.step, text: ((it.querySelector('.slider-value')||{}).textContent||'').trim() };`);
  // Toggle off again.
  const r2 = await pop.evaluate(`${PAGE_JS}
    const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => /quick font size/i.test(nameOf(x)));
    const t = it && it.querySelector('.checkbox-container'); if (t) t.scrollIntoView({ block: 'center' }); return t ? rect(t) : null;`);
  const before2 = appearanceJson();
  if (r2) await pop.clickAt(r2.x + r2.w / 2, r2.y + r2.h / 2);
  res.toggleOff = { appearanceJson: await waitWrite(before2) };
  // And Ctrl+wheel with it off.
  await vault.evaluate(`app.setting.close(); return true;`); await sleep(800);
  await wheel(-120, true, 2);
  res.offWheel = { appearanceJson: appearanceJson(), state: await fontState() };
  await vault.evaluate(`app.setting.open(); app.setting.openTabById('appearance'); return true;`); await sleep(1500);
  pop.close(); pop = await findPopout();
  return res;
});

// ---- s08 Custom app icon -------------------------------------------------
await step('s08-custom-icon', async () => {
  const res = {};
  try { await s08(res); } catch (e) { res.ERR = e.message; }
  return res;
});
async function s08(res) {
  await toAppearance();
  const rowInfo = () => pop.evaluate(`${PAGE_JS}
    const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => /custom app icon/i.test(nameOf(x)));
    if (!it) return null; it.scrollIntoView({ block: 'center' });
    return { name: nameOf(it), desc: ((it.querySelector('.setting-item-description')||{}).textContent||'').trim(),
      html: it.innerHTML.slice(0, 2500),
      controls: Array.from(it.querySelectorAll('button, .clickable-icon')).map((b) => ({ tag: b.tagName, text: (b.textContent||'').trim(), aria: b.getAttribute('aria-label'), cls: b.className, shown: visible(b), rect: rect(b) })) };`);
  res.row = await rowInfo();
  if (!res.row) return res;
  // Log the IPC the click makes, in both windows, and answer a native open
  // dialog with the probe's PNG: a real dialog is modal and would leave the
  // popout deaf to every later input event.
  const HOOK = `
    const e = require('electron'); const ipc = e.ipcRenderer; window.__ipcLog = window.__ipcLog || [];
    if (!ipc.__wrapped) {
      ipc.__wrapped = true;
      for (const m of ['send', 'sendSync', 'invoke']) { const orig = ipc[m].bind(ipc); ipc[m] = (...a) => { try { window.__ipcLog.push({ m, a: JSON.stringify(a).slice(0, 400) }); } catch {} return orig(...a); }; }
    }
    const fake = ${JSON.stringify(ICON)};
    const patch = (dialog, where) => {
      if (!dialog || dialog.__patched) return;
      dialog.__patched = true;
      dialog.showOpenDialog = async (...a) => { window.__ipcLog.push({ m: where + '.showOpenDialog', a: JSON.stringify(a.filter((x) => !x || !x.id)).slice(0, 600) }); return { canceled: false, filePaths: [fake] }; };
      dialog.showOpenDialogSync = (...a) => { window.__ipcLog.push({ m: where + '.showOpenDialogSync', a: JSON.stringify(a.filter((x) => !x || !x.id)).slice(0, 600) }); return [fake]; };
    };
    const found = [];
    try { const r = require('@electron/remote'); patch(r.dialog, '@electron/remote'); found.push('@electron/remote'); } catch (err) { found.push('no @electron/remote: ' + err.message.slice(0, 80)); }
    try { if (e.remote) { patch(e.remote.dialog, 'electron.remote'); found.push('electron.remote'); } } catch {}
    return found;`;
  res.hookPop = await pop.evaluate(HOOK).catch((e) => 'ERR ' + String(e));
  res.hookVault = await vault.evaluate(HOOK).catch((e) => 'ERR ' + String(e));
  // Intercept a file chooser, if the button raises an <input type=file>.
  await pop.send('Page.enable');
  await pop.send('DOM.enable');
  await pop.send('Page.setInterceptFileChooserDialog', { enabled: true }).catch((e) => (res.interceptErr = String(e)));
  let chooser = null;
  pop.on((m) => { if (m.method === 'Page.fileChooserOpened') chooser = m.params; });
  const snapA = snapshot();
  const choose = res.row.controls.find((c) => /choose/i.test(c.text));
  if (choose) await pop.clickAt(choose.rect.x + choose.rect.w / 2, choose.rect.y + choose.rect.h / 2);
  await sleep(1500);
  res.chooser = chooser;
  if (chooser && chooser.backendNodeId) {
    await pop.send('DOM.setFileInputFiles', { files: [ICON], backendNodeId: chooser.backendNodeId }).catch((e) => (res.setFilesErr = String(e)));
  }
  await sleep(2000);
  res.ipcPop = await pop.evaluate(`return window.__ipcLog || null;`).catch((e) => String(e));
  res.ipcVault = await vault.evaluate(`return window.__ipcLog || null;`).catch((e) => String(e));
  const snapB = snapshot();
  res.chooseDiff = diffSnap(snapA, snapB);
  res.chooseText = textOf(res.chooseDiff);
  res.rowAfter = await rowInfo();
  res.shot = await pop.shot('custom-icon-chosen.png');
  // Restore default, if it is now shown.
  const restore = (res.rowAfter && res.rowAfter.controls || []).find((c) => /restore/i.test(c.aria || '') && c.shown);
  if (restore) {
    const snapC = snapshot();
    await pop.clickAt(restore.rect.x + restore.rect.w / 2, restore.rect.y + restore.rect.h / 2);
    await sleep(1500);
    const snapD = snapshot();
    res.restoreDiff = diffSnap(snapC, snapD);
    res.restoreText = textOf(res.restoreDiff);
    res.ipcPopAfterRestore = await pop.evaluate(`return window.__ipcLog || null;`).catch((e) => String(e));
    res.rowAfterRestore = await rowInfo();
  }
}


// ---- s09 where a Ctrl+wheel size goes --------------------------------
await step('s09-wheel-persist', async () => {
  const res = {};
  const probeKeys = `const hits = {}; const seen = new Set();
    const walk = (o, p, d) => { if (!o || typeof o !== 'object' || d > 3 || seen.has(o)) return; seen.add(o);
      for (const k of Object.keys(o)) { let v; try { v = o[k]; } catch { continue; }
        if (/fontsize|basefont/i.test(k)) hits[p + '.' + k] = typeof v === 'object' ? JSON.stringify(v).slice(0, 100) : v;
        if (v && typeof v === 'object' && !(v instanceof Node) && !Array.isArray(v)) walk(v, p + '.' + k, d + 1); } };
    walk(app.vault, 'vault', 0); walk(app.customCss || {}, 'customCss', 0);
    const ls = {}; for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); ls[k] = (localStorage.getItem(k) || '').slice(0, 120); }
    return { hits, ls, bodyStyle: document.body.getAttribute('style') };`;
  await vault.evaluate(`app.setting.close(); return true;`).catch(() => null); await sleep(800);
  res.before = await vault.evaluate(probeKeys);
  const snapA = snapshot();
  await wheel(-120, true, 2);
  res.soon = { appearanceJson: appearanceJson(), probe: await vault.evaluate(probeKeys) };
  await sleep(8000);
  res.after = await vault.evaluate(probeKeys);
  // A wheel handler may save when Ctrl is let go rather than per notch.
  await vault.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'Control', code: 'ControlLeft', windowsVirtualKeyCode: 17, modifiers: 2 }).catch(() => null);
  await vault.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Control', code: 'ControlLeft', windowsVirtualKeyCode: 17, modifiers: 0 }).catch(() => null);
  await vault.evaluate(`document.dispatchEvent(new KeyboardEvent('keyup', { key: 'Control', code: 'ControlLeft', bubbles: true })); window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Control', code: 'ControlLeft', bubbles: true })); return true;`).catch(() => null);
  await sleep(3000);
  res.afterCtrlUp = { appearanceJson: appearanceJson(), body: await vault.evaluate(`return document.body.getAttribute('style');`) };
  // And across a clean quit.
  try { await vault.quit(); res.quit = true; } catch (e) { res.quit = String(e); }
  await sleep(1500);
  res.afterQuit = { appearanceJson: appearanceJson(), diff: diffSnap(snapA, snapshot()).vault };
  res.appearanceJson = appearanceJson();
  const snapB = snapshot();
  res.diff = diffSnap(snapA, snapB);
  res.text = textOf(res.diff);
  return res;
});

out.finalAppearanceJson = appearanceJson();
save();
pop.close();
vault.close();
process.exit(0);
