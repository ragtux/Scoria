// Settings -> Appearance: the three pickers Scoria has left as "not in Scoria
// yet" — Themes, CSS snippets and Accent color — plus the rest of the tab's
// chrome for comparison.
//
// 1.14 opens Settings in a popout window, so the rows are not in the vault
// window's document. Two handles are used together: the vault window's JS,
// which can query the popout through `app.setting.activeTab.containerEl`
// whatever document it lives in (the trick `frame-setting-key.mjs` found),
// and a bare CDP client attached to the popout's own page target, which is
// the only thing that can inject a real click or take its screenshot.
//
// Records, per surface: every `.setting-item`'s name, description, control
// and markup; the Themes dropdown's options against `app.customCss.themes`
// and what picking one writes to appearance.json; the Manage modal; the CSS
// snippets section's rows, their buttons and what each toggle writes, in
// order; whether a snippet dropped in the folder shows up without a reload;
// the accent picker's control and the bytes it writes; and the empty state
// of both sections with the folders removed.
//
// Phases: fixtures | run.   ONLY_STEPS=s04,s06 re-runs a step.
// SHOTS names the folder the PNGs go in (default beside the results file).
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const BASE = process.env.CDP_BASE || 'http://127.0.0.1:9222';
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const SHOTS = process.env.SHOTS || RESULTS.replace(/\.json$/, '-shots');
const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

const out = fs.existsSync(RESULTS) ? JSON.parse(fs.readFileSync(RESULTS, 'utf8')) : {};
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(out, null, 1));
const only = (process.env.ONLY_STEPS || '').split(',').filter(Boolean);
const step = async (id, fn) => {
  if (only.length && !only.includes(id)) return;
  try { out[id] = await fn(); } catch (e) { out[id] = { ERR: e.message }; }
  save();
  console.error('[appearance-tab] ' + id);
};

// ---- fixtures ------------------------------------------------------------
// Two themes and three snippets, so both sections have rows and the order
// the enabled list is written in can be read off three of them.
const THEMES = {
  Ember: '/* Ember */\n.theme-dark {\n  --interactive-accent: #c2410c;\n  --text-accent: #ea580c;\n}\n.theme-light {\n  --interactive-accent: #ea580c;\n}\n',
  Zephyr: '/* Zephyr */\nbody {\n  --background-primary: #0b1220;\n  --text-normal: #cfe3ff;\n}\n',
};
const SNIPPETS = {
  'wider.css': 'body { --file-line-width: 1100px; }\n',
  'dim.css': 'body { --text-faint: #444; }\n',
  'tabs.css': 'body { --tab-text-color-focused-active: #10b981; }\n',
};

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Alpha.md'), '# Alpha\n\nSome text with a [[Beta|link]] and `code`.\n');
  fs.writeFileSync(vp('Beta.md'), '# Beta\n');
  fs.writeFileSync(vp('.obsidian/appearance.json'), '{}\n');
  for (const [name, css] of Object.entries(THEMES)) {
    const dir = vp(path.join('.obsidian/themes', name));
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'theme.css'), css);
    fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify({
      name, version: '1.0.0', minAppVersion: '1.0.0', author: 'probe', authorUrl: 'https://example.invalid',
    }, null, 2) + '\n');
  }
  fs.mkdirSync(vp('.obsidian/snippets'), { recursive: true });
  for (const [name, css] of Object.entries(SNIPPETS)) fs.writeFileSync(vp(path.join('.obsidian/snippets', name)), css);
  // s17 needs its themes present at launch: Obsidian reads the themes
  // folder when the vault opens, and a folder added later is not picked up
  // by `requestReadThemes()` alone (seen 2026-11-02).
  if (process.env.THEME_IDENTITY) {
    fs.rmSync(vp('.obsidian/themes'), { recursive: true, force: true });
    const mk = (folder, manifestName, color) => {
      const dir = vp(path.join('.obsidian/themes', folder));
      fs.mkdirSync(dir, { recursive: true });
      fs.writeFileSync(path.join(dir, 'theme.css'), `body { --text-error: ${color}; }\n`);
      if (manifestName !== null) fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify({ name: manifestName, version: '1.0.0', minAppVersion: '1.0.0', author: 'probe' }, null, 2) + '\n');
    };
    mk('folder-name', 'Display Name', '#0000ff');  // folder != manifest name
    mk('Bare', null, '#ff00ff');                   // no manifest at all
    mk('Mismatch', 'Mismatch', '#00ffff');         // the control: they agree
  }
  out.fixtures = { themes: Object.keys(THEMES), snippets: Object.keys(SNIPPETS), themeIdentity: !!process.env.THEME_IDENTITY };
  save();
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

// ---- a bare client for the Settings popout -------------------------------
async function pageTargets() {
  const ts = await (await fetch(`${BASE}/json`)).json();
  return ts.filter((x) => x.type === 'page');
}

async function attach(wsUrl) {
  const ws = new WebSocket(wsUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let id = 0;
  const pending = new Map();
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const { res, rej } = pending.get(m.id);
      pending.delete(m.id);
      m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
    }
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
  const clickAt = async (x, y) => {
    for (const type of ['mousePressed', 'mouseReleased']) {
      await send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1, buttons: type === 'mousePressed' ? 1 : 0 });
      await sleep(40);
    }
  };
  const hover = async (x, y) => { await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, buttons: 0 }); await sleep(120); };
  const shot = async (name, clip) => {
    const r = await send('Page.captureScreenshot', clip ? { format: 'png', clip: { ...clip, scale: 1 } } : { format: 'png' });
    fs.mkdirSync(SHOTS, { recursive: true });
    const p = path.join(SHOTS, name);
    fs.writeFileSync(p, Buffer.from(r.data, 'base64'));
    return path.basename(p);
  };
  return { send, evaluate, clickAt, hover, shot, close: () => ws.close() };
}

// ---- what to read off a settings row -------------------------------------
// Runs in the VAULT window but queries the popout's document through
// `activeTab.containerEl`, so the rects are in the popout's viewport and the
// popout client can click them.
const ROOT = `const root = app.setting.activeTab && app.setting.activeTab.containerEl; if (!root) throw new Error('no activeTab.containerEl');`;
const HELPERS = `
  const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x:+b.x.toFixed(2), y:+b.y.toFixed(2), w:+b.width.toFixed(2), h:+b.height.toFixed(2) }; };
  const P = ['display','opacity','background-color','color','border','border-radius','padding','margin','width','height','font-size','font-weight','gap','box-shadow'];
  const cs = (el) => { if (!el) return null; const s = el.ownerDocument.defaultView.getComputedStyle(el); const o = {}; for (const p of P) o[p] = s.getPropertyValue(p); return o; };
  const icon = (el) => { const s = el && el.querySelector('svg'); return s ? (s.getAttribute('class') || '') : null; };
  const control = (it) => {
    const c = it.querySelector('.setting-item-control'); if (!c) return null;
    const parts = Array.from(c.children).map((el) => ({
      tag: el.tagName, cls: typeof el.className === 'string' ? el.className : '',
      aria: el.getAttribute('aria-label'), tip: el.getAttribute('data-tooltip-position'),
      text: (el.textContent || '').trim().slice(0, 120), icon: icon(el), rect: rect(el),
      type: el.getAttribute('type'), value: el.value === undefined ? null : el.value,
      placeholder: el.getAttribute('placeholder'),
      options: el.tagName === 'SELECT' ? Array.from(el.options).map((o) => ({ v: o.value, t: o.textContent })) : undefined,
      enabled: el.classList.contains('is-enabled') ? true : (el.classList.contains('checkbox-container') ? false : undefined),
      inner: el.innerHTML.length > 600 ? el.innerHTML.slice(0, 600) + '...' : el.innerHTML,
    }));
    return { cls: c.className, html: c.innerHTML.length > 1200 ? c.innerHTML.slice(0, 1200) + '...' : c.innerHTML, parts };
  };
  const row = (it) => ({
    name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(),
    desc: ((it.querySelector('.setting-item-description') || {}).textContent || '').trim().slice(0, 300),
    value: ((it.querySelector('.setting-item-value') || {}).textContent || '').trim() || null,
    heading: it.classList.contains('setting-item-heading'),
    cls: it.className, rect: rect(it), style: cs(it),
    control: control(it),
  });
  const rows = (root) => Array.from(root.querySelectorAll('.setting-item')).map(row);
`;

const vault = await connect();
out.vaultPath = await vault.vaultPath();
out.version = await vault.evaluate(`return { app: app.appId, version: (window.require && require('obsidian').apiVersion) || null, ua: navigator.userAgent };`).catch(() => null);
save();

// Open Settings on the Appearance tab, and find the popout's page target.
await vault.evaluate(`app.setting.open(); app.setting.openTabById('appearance'); return true;`);
await sleep(1500);

let pop = null;
{
  const targets = await pageTargets();
  out.targets = targets.map((t) => ({ title: t.title, url: t.url }));
  for (const t of targets) {
    let cand = null;
    try {
      cand = await attach(t.webSocketDebuggerUrl);
      const isSettings = await cand.evaluate(`return !!document.querySelector('.vertical-tab-content') && !document.querySelector('.workspace-split.mod-root');`);
      if (isSettings) { pop = cand; out.popoutTarget = { title: t.title, url: t.url }; break; }
      cand.close();
    } catch { if (cand) try { cand.close(); } catch {} }
  }
  // 1.14 may still render Settings as a modal inside the vault window; then
  // the vault window's own target is the one to click and capture in.
  if (!pop) {
    const ts = await pageTargets();
    for (const t of ts) {
      try {
        const cand = await attach(t.webSocketDebuggerUrl);
        if (await cand.evaluate(`return !!document.querySelector('.modal-container .vertical-tab-content');`)) {
          pop = cand; out.popoutTarget = { inVaultWindow: true, title: t.title, url: t.url }; break;
        }
        cand.close();
      } catch {}
    }
  }
}
out.foundSettingsTarget = !!pop;
save();
if (!pop) { console.error('[appearance-tab] no Settings window target'); vault.close(); process.exit(3); }

const rowsNow = () => vault.evaluate(`${ROOT}${HELPERS} return { tab: app.setting.activeTab.id, ownerIsMain: root.ownerDocument === document, rows: rows(root), html: root.innerHTML.length > 60000 ? root.innerHTML.slice(0, 60000) + '...' : root.innerHTML };`);
const findRow = (re) => vault.evaluate(`${ROOT}${HELPERS}
  const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => ${re}.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
  return it ? row(it) : null;`);
const appearanceJson = () => bytes('.obsidian/appearance.json');
const customCss = () => vault.evaluate(`const cc = app.customCss || {}; return {
  theme: cc.theme, themes: cc.themes ? Object.keys(cc.themes) : null, themeManifests: cc.themes || null,
  snippets: cc.snippets || null, enabledSnippets: cc.enabledSnippets ? Array.from(cc.enabledSnippets) : null,
  oldThemes: cc.oldThemes || null, keys: Object.keys(cc), extraStyleEls: cc.extraStyleEls ? cc.extraStyleEls.length : null,
  styleElIds: Array.from(document.head.querySelectorAll('style')).map((s) => ({ id: s.id || null, len: (s.textContent||'').length, head: (s.textContent||'').slice(0, 60) })),
}; `);
// Wait for appearance.json to change from `before`, then settle.
const waitWrite = async (before, ms = 3000) => {
  const end = Date.now() + ms;
  while (Date.now() < end && appearanceJson() === before) await sleep(120);
  await sleep(400);
  return appearanceJson();
};
const clickRect = async (r) => { if (!r) return false; await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2); await sleep(500); return true; };

// ---- s01 the whole tab ---------------------------------------------------
await step('s01-tab', async () => {
  const dump = await rowsNow();
  return {
    ...dump,
    customCss: await customCss(),
    appearanceJson: appearanceJson(),
    bodyClass: await vault.evaluate(`return document.body.className;`),
    shot: await pop.shot('appearance-tab.png'),
    sections: await vault.evaluate(`${ROOT}${HELPERS}
      return Array.from(root.children).map((el) => ({ tag: el.tagName, cls: typeof el.className === 'string' ? el.className : '', text: (el.textContent||'').trim().slice(0, 120), rect: rect(el) }));`),
  };
});

// ---- s02 the Themes row and its Manage modal -----------------------------
await step('s02-themes-row', async () => {
  const row = await findRow('/^themes$/i');
  const res = { row, customCss: await customCss() };
  // Every button in the row, with its icon and tooltip.
  res.buttons = await vault.evaluate(`${ROOT}${HELPERS}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    if (!it) return null;
    return Array.from(it.querySelectorAll('button, .clickable-icon')).map((b) => ({ tag: b.tagName, cls: b.className, text: (b.textContent||'').trim(), aria: b.getAttribute('aria-label'), icon: icon(b), rect: rect(b) }));`);
  if (res.row) res.shot = await pop.shot('themes-row.png', { x: Math.max(0, res.row.rect.x - 8), y: Math.max(0, res.row.rect.y - 8), width: res.row.rect.w + 16, height: res.row.rect.h + 16 });
  // The Manage button raises the theme chooser.
  const manage = (res.buttons || []).find((b) => /manage/i.test(b.text || ''));
  if (manage) {
    await clickRect(manage.rect);
    res.modal = await pop.evaluate(`
      const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x:+b.x.toFixed(2), y:+b.y.toFixed(2), w:+b.width.toFixed(2), h:+b.height.toFixed(2) }; };
      const m = document.querySelector('.modal-container .modal') || document.querySelector('.modal');
      if (!m) return null;
      return { cls: m.className, title: ((m.querySelector('.modal-title')||{}).textContent||'').trim(),
        rect: rect(m),
        items: Array.from(m.querySelectorAll('.setting-item, .theme-settings-item, .community-item')).map((it) => ({ cls: it.className, text: (it.textContent||'').trim().slice(0, 200), rect: rect(it) })),
        buttons: Array.from(m.querySelectorAll('button')).map((b) => ({ text: (b.textContent||'').trim(), cls: b.className, aria: b.getAttribute('aria-label') })),
        html: m.innerHTML.length > 12000 ? m.innerHTML.slice(0, 12000) + '...' : m.innerHTML };`);
    res.modalShot = await pop.shot('themes-manage-modal.png');
    await pop.evaluate(`for (const m of document.querySelectorAll('.modal-container')) m.remove(); return true;`);
    await sleep(400);
  } else {
    res.manageButton = null;
  }
  return res;
});

// ---- s03 picking a theme -------------------------------------------------
await step('s03-pick-theme', async () => {
  const tries = [];
  const pickViaSelect = async (value) => {
    const before = appearanceJson();
    const ok = await vault.evaluate(`${ROOT}
      const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
      const sel = it && it.querySelector('select');
      if (!sel) return false;
      sel.value = ${JSON.stringify(value)};
      sel.dispatchEvent(new Event('change', { bubbles: true }));
      return sel.value;`);
    const after = await waitWrite(before);
    return {
      requested: value, selectAccepted: ok, appearanceJson: after,
      cssTheme: await vault.evaluate(`return app.vault.getConfig ? app.vault.getConfig('cssTheme') : null;`).catch(() => null),
      customCss: await customCss(),
      bodyClass: await vault.evaluate(`return document.body.className;`),
      // The theme really applied: a property only the fixture declares.
      accentInVault: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--interactive-accent').trim();`),
      bgInVault: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--background-primary').trim();`),
      rowAfter: await findRow('/^themes$/i'),
    };
  };
  const options = await vault.evaluate(`${ROOT}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    const sel = it && it.querySelector('select');
    return sel ? Array.from(sel.options).map((o) => ({ v: o.value, t: o.textContent })) : null;`);
  for (const o of options || []) tries.push(await pickViaSelect(o.v));
  return { options, tries };
});

// ---- s04 the CSS snippets section ---------------------------------------
await step('s04-snippets', async () => {
  // Obsidian re-reads the folders when the tab is re-entered.
  await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(500);
  await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(900);
  const res = {};
  res.header = await findRow('/css snippets/i');
  res.headerButtons = await vault.evaluate(`${ROOT}${HELPERS}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /css snippets/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    if (!it) return null;
    return Array.from(it.querySelectorAll('button, .clickable-icon')).map((b) => ({ tag: b.tagName, cls: b.className, text: (b.textContent||'').trim(), aria: b.getAttribute('aria-label'), icon: icon(b), rect: rect(b) }));`);
  // Every row after the snippets heading: the file rows Obsidian builds.
  res.rows = await vault.evaluate(`${ROOT}${HELPERS}
    const all = Array.from(root.querySelectorAll('.setting-item'));
    const ix = all.findIndex((x) => /css snippets/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    if (ix < 0) return null;
    return all.slice(ix).map((it) => ({ ...row(it), buttons: Array.from(it.querySelectorAll('button, .clickable-icon')).map((b) => ({ cls: b.className, text: (b.textContent||'').trim(), aria: b.getAttribute('aria-label'), icon: icon(b), rect: rect(b) })) }));`);
  const r = res.header && res.header.rect;
  if (r) res.shot = await pop.shot('snippets-section.png', { x: Math.max(0, r.x - 8), y: Math.max(0, r.y - 8), width: r.w + 16, height: Math.min(600, (res.rows || []).length * 60 + 80) });
  res.customCss = await customCss();

  // Toggle each snippet on, in a deliberate order, then two off, reading the
  // written list each time: that is how `enabledCssSnippets` is ordered.
  const toggleRow = async (name) => {
    const before = appearanceJson();
    const hit = await vault.evaluate(`${ROOT}${HELPERS}
      const all = Array.from(root.querySelectorAll('.setting-item'));
      const it = all.find((x) => ((x.querySelector('.setting-item-name')||{}).textContent||'').trim() === ${JSON.stringify(name)});
      if (!it) return null;
      const t = it.querySelector('.checkbox-container');
      return t ? { rect: rect(t), wasEnabled: t.classList.contains('is-enabled') } : { noToggle: true, html: it.innerHTML.slice(0, 400) };`);
    if (!hit || hit.noToggle) return { name, hit };
    await clickRect(hit.rect);
    const after = await waitWrite(before);
    return {
      name, wasEnabled: hit.wasEnabled, appearanceJson: after,
      enabledSnippets: await vault.evaluate(`return app.customCss && app.customCss.enabledSnippets ? Array.from(app.customCss.enabledSnippets) : null;`),
      styleEls: await vault.evaluate(`return Array.from(document.head.querySelectorAll('style')).map((s) => ({ id: s.id || null, head: (s.textContent||'').slice(0, 60) }));`),
      lineWidth: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--file-line-width').trim();`),
      tabColor: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--tab-text-color-focused-active').trim();`),
      nowEnabled: await vault.evaluate(`${ROOT}${HELPERS}
        return Array.from(root.querySelectorAll('.setting-item')).map((it) => { const t = it.querySelector('.checkbox-container'); return t ? { name: ((it.querySelector('.setting-item-name')||{}).textContent||'').trim(), on: t.classList.contains('is-enabled') } : null; }).filter(Boolean);`),
    };
  };
  // The names Obsidian shows for a row are what we match on; fall back to
  // the file stem if the record shows the extension is kept.
  const names = (res.rows || []).filter((x) => x.control && /checkbox-container/.test(x.control.html || '')).map((x) => x.name);
  res.rowNames = names;
  res.tries = [];
  for (const n of ['tabs', 'wider', 'dim'].map((s) => names.find((x) => x === s || x === s + '.css')).filter(Boolean)) {
    res.tries.push(await toggleRow(n));
  }
  // and two back off, in the other order
  for (const n of ['wider', 'tabs'].map((s) => names.find((x) => x === s || x === s + '.css')).filter(Boolean)) {
    res.tries.push(await toggleRow(n));
  }
  return res;
});

// ---- s05 a snippet dropped in the folder while Settings is open ----------
await step('s05-snippet-added', async () => {
  const res = {};
  fs.writeFileSync(vp('.obsidian/snippets/late.css'), 'body { --text-error: #ff0055; }\n');
  await sleep(1500);
  res.withoutReload = await vault.evaluate(`${ROOT}
    return Array.from(root.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim());`);
  res.snippetsApi = await vault.evaluate(`return app.customCss ? app.customCss.snippets : null;`);
  // Re-enter the tab: does that alone pick it up?
  await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(400);
  await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(900);
  res.afterReenter = await vault.evaluate(`${ROOT}
    return Array.from(root.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim());`);
  // And the reload button, if the section has one.
  const reload = (out['s04-snippets'] && out['s04-snippets'].headerButtons || []).find((b) => /reload|refresh/i.test((b.aria || '') + ' ' + (b.text || '')));
  res.reloadButton = reload || null;
  if (reload) {
    const fresh = await vault.evaluate(`${ROOT}${HELPERS}
      const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /css snippets/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
      const b = it && Array.from(it.querySelectorAll('button, .clickable-icon')).find((x) => ${JSON.stringify(reload.aria || reload.text || '')} === ((x.getAttribute('aria-label')||'') || (x.textContent||'').trim()));
      return b ? rect(b) : null;`);
    await clickRect(fresh);
    res.afterReload = await vault.evaluate(`${ROOT}
      return Array.from(root.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim());`);
  }
  // The folder button must not be clicked: it would raise a file manager.
  res.folderButton = (out['s04-snippets'] && out['s04-snippets'].headerButtons || []).find((b) => /folder/i.test((b.aria || '') + ' ' + (b.icon || ''))) || null;
  return res;
});

// ---- s06 the accent colour -----------------------------------------------
await step('s06-accent', async () => {
  const res = { row: await findRow('/accent colo/i') };
  res.control = await vault.evaluate(`${ROOT}${HELPERS}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /accent colo/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    if (!it) return null;
    const c = it.querySelector('.setting-item-control');
    return { html: c.innerHTML, inputs: Array.from(c.querySelectorAll('input')).map((i) => ({ type: i.type, value: i.value, cls: i.className, rect: rect(i) })),
      buttons: Array.from(c.querySelectorAll('button, .clickable-icon')).map((b) => ({ cls: b.className, text: (b.textContent||'').trim(), aria: b.getAttribute('aria-label'), icon: icon(b), rect: rect(b) })) };`);
  res.tries = [];
  for (const hex of ['#e11d48', '#16a34a']) {
    const before = appearanceJson();
    const set = await vault.evaluate(`${ROOT}
      const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /accent colo/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
      const i = it && it.querySelector('input[type=color]');
      if (!i) return null;
      i.value = ${JSON.stringify(hex)};
      i.dispatchEvent(new Event('input', { bubbles: true }));
      i.dispatchEvent(new Event('change', { bubbles: true }));
      return i.value;`);
    const after = await waitWrite(before);
    res.tries.push({
      hex, set, appearanceJson: after,
      bodyStyle: await vault.evaluate(`return document.body.getAttribute('style');`),
      accentVars: await vault.evaluate(`const s = getComputedStyle(document.body); return { h: s.getPropertyValue('--accent-h').trim(), s: s.getPropertyValue('--accent-s').trim(), l: s.getPropertyValue('--accent-l').trim(), interactive: s.getPropertyValue('--interactive-accent').trim(), textAccent: s.getPropertyValue('--text-accent').trim() };`),
      accentConfig: await vault.evaluate(`return app.vault.getConfig ? app.vault.getConfig('accentColor') : null;`).catch(() => null),
    });
  }
  // The reset button beside it, if there is one.
  const reset = (res.control && res.control.buttons || []).find((b) => /default|reset|restore/i.test((b.aria || '') + ' ' + (b.text || '')));
  if (reset) {
    const before = appearanceJson();
    await clickRect(reset.rect);
    res.afterReset = { button: reset, appearanceJson: await waitWrite(before), bodyStyle: await vault.evaluate(`return document.body.getAttribute('style');`) };
  }
  res.shot = res.row ? await pop.shot('accent-row.png', { x: Math.max(0, res.row.rect.x - 8), y: Math.max(0, res.row.rect.y - 8), width: res.row.rect.w + 16, height: res.row.rect.h + 16 }) : null;
  return res;
});

// ---- s07 the rest of the tab, for the rows Scoria already has ------------
await step('s07-rest', async () => {
  const names = ['/base colo/i', '/interface font/i', '/text font/i', '/monospace font/i', '/font size/i'];
  const rows = {};
  for (const re of names) rows[re] = await findRow(re);
  return {
    rows,
    // The font rows are a search component in 1.14, not a plain text field.
    fontControlHtml: await vault.evaluate(`${ROOT}
      const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /interface font/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
      return it ? it.querySelector('.setting-item-control').innerHTML : null;`),
    tabList: await vault.evaluate(`return Array.from(app.setting.settingTabs || []).map((t) => ({ id: t.id, name: t.name }));`),
  };
});

// ---- s08 both sections with nothing installed ---------------------------
await step('s08-empty', async () => {
  // Turn the theme off first so removing its folder cannot leave a dangling pick.
  await vault.evaluate(`${ROOT}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    const sel = it && it.querySelector('select');
    if (sel) { sel.value = sel.options[0].value; sel.dispatchEvent(new Event('change', { bubbles: true })); }
    return true;`);
  await sleep(1200);
  fs.rmSync(vp('.obsidian/themes'), { recursive: true, force: true });
  fs.rmSync(vp('.obsidian/snippets'), { recursive: true, force: true });
  await sleep(1500);
  await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(400);
  await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(1200);
  const dump = await rowsNow();
  return {
    rows: dump.rows, html: dump.html,
    customCss: await customCss(),
    appearanceJson: appearanceJson(),
    shot: await pop.shot('appearance-tab-empty.png'),
  };
});

// ---- the second pass ----------------------------------------------------
// 1.14.5's Appearance tab is a drill-in: "CSS snippets" and the three font
// rows are `.setting-item.mod-navigable.tappable` that push a sub-page, so
// the first pass found no snippet rows at all. These steps click through.

// Everything in the popout's settings pane, whatever page it is showing.
const PAGE_JS = `
  const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x:+b.x.toFixed(2), y:+b.y.toFixed(2), w:+b.width.toFixed(2), h:+b.height.toFixed(2) }; };
  const icon = (el) => { const s = el && el.querySelector('svg'); return s ? (s.getAttribute('class') || '') : null; };
  const btns = (el) => Array.from(el.querySelectorAll('button, .clickable-icon, .checkbox-container')).map((b) => ({ tag: b.tagName, cls: b.className, text: (b.textContent||'').trim().slice(0,80), aria: b.getAttribute('aria-label'), disabled: b.getAttribute('aria-disabled'), icon: icon(b), on: b.classList.contains('is-enabled'), rect: rect(b) }));
  const row = (it) => ({
    name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(),
    desc: ((it.querySelector('.setting-item-description') || {}).textContent || '').trim().slice(0, 300),
    value: ((it.querySelector('.setting-item-value') || {}).textContent || '').trim() || null,
    heading: it.classList.contains('setting-item-heading'), cls: it.className, rect: rect(it),
    controlHtml: (() => { const c = it.querySelector('.setting-item-control'); return c ? (c.innerHTML.length > 900 ? c.innerHTML.slice(0,900)+'...' : c.innerHTML) : null; })(),
    buttons: btns(it),
  });
  const pane = document.querySelector('.vertical-tab-content-container') || document.querySelector('.modal-content') || document.body;
  const header = document.querySelector('.modal-setting-back-button') || document.querySelector('.setting-back-button') || document.querySelector('[class*="back-button"]');
`;
const page = () => pop.evaluate(`${PAGE_JS}
  return {
    back: header ? { cls: header.className, text: (header.textContent||'').trim(), icon: icon(header), rect: rect(header) } : null,
    title: ((document.querySelector('.vertical-tab-header-group-title') || {}).textContent || '').trim() || null,
    activeNav: Array.from(document.querySelectorAll('.vertical-tab-nav-item.is-active')).map((n) => (n.textContent||'').trim()),
    rows: Array.from(pane.querySelectorAll('.setting-item')).map(row),
    headings: Array.from(pane.querySelectorAll('.setting-item-heading')).map((h) => (h.textContent||'').trim()),
    paneCls: pane.className,
    html: pane.innerHTML.length > 40000 ? pane.innerHTML.slice(0, 40000) + '...' : pane.innerHTML,
  };`);
// Click a row of the page shown now, by its visible name.
const clickRowNamed = async (name) => {
  const r = await pop.evaluate(`${PAGE_JS}
    const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => ((x.querySelector('.setting-item-name')||{}).textContent||'').trim() === ${JSON.stringify(name)});
    return it ? rect(it) : null;`);
  if (!r) return false;
  await pop.clickAt(r.x + r.w - 24, r.y + r.h / 2);
  await sleep(700);
  return true;
};
const goBack = async () => {
  const r = await pop.evaluate(`${PAGE_JS} return header ? rect(header) : null;`);
  if (r) { await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2); await sleep(700); return 'button'; }
  await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(400);
  await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(900);
  return 'reenter';
};
const toAppearance = async () => {
  await vault.evaluate(`app.setting.open(); app.setting.openTabById('appearance'); return true;`);
  await sleep(1200);
};

// ---- s09 the CSS snippets sub-page --------------------------------------
await step('s09-snippets-page', async () => {
  await toAppearance();
  const res = {};
  res.opened = await clickRowNamed('CSS snippets');
  res.page = await page();
  res.shot = await pop.shot('snippets-page.png');
  res.snippetsApi = await vault.evaluate(`return app.customCss ? app.customCss.snippets : null;`);

  // Toggle three on in a deliberate order, then two off, reading the file
  // each time: that is how `enabledCssSnippets` is ordered and rewritten.
  const toggle = async (name) => {
    const before = appearanceJson();
    const hit = await pop.evaluate(`${PAGE_JS}
      const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => ((x.querySelector('.setting-item-name')||{}).textContent||'').trim() === ${JSON.stringify(name)});
      if (!it) return null;
      const t = it.querySelector('.checkbox-container');
      return t ? { rect: rect(t), wasEnabled: t.classList.contains('is-enabled') } : { noToggle: true, html: it.innerHTML.slice(0, 500) };`);
    if (!hit || hit.noToggle) return { name, hit };
    await pop.clickAt(hit.rect.x + hit.rect.w / 2, hit.rect.y + hit.rect.h / 2);
    const after = await waitWrite(before);
    return {
      name, wasEnabled: hit.wasEnabled, appearanceJson: after,
      enabledSnippets: await vault.evaluate(`return app.customCss && app.customCss.enabledSnippets ? Array.from(app.customCss.enabledSnippets) : null;`),
      styleEls: await vault.evaluate(`return Array.from(document.head.querySelectorAll('style')).map((s) => ({ id: s.id || null, head: (s.textContent||'').slice(0, 50) }));`),
      lineWidth: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--file-line-width').trim();`),
      tabColor: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--tab-text-color-focused-active').trim();`),
      nowOn: await pop.evaluate(`${PAGE_JS}
        return Array.from(pane.querySelectorAll('.setting-item')).map((it) => { const t = it.querySelector('.checkbox-container'); return t ? { name: ((it.querySelector('.setting-item-name')||{}).textContent||'').trim(), on: t.classList.contains('is-enabled') } : null; }).filter(Boolean);`),
    };
  };
  const names = (res.page.rows || []).filter((r) => (r.buttons || []).some((b) => /checkbox-container/.test(b.cls || ''))).map((r) => r.name);
  res.rowNames = names;
  res.tries = [];
  for (const want of ['tabs', 'wider', 'dim', 'wider', 'tabs']) {
    const n = names.find((x) => x === want || x === want + '.css');
    if (n) res.tries.push(await toggle(n));
  }
  res.enabledShot = await pop.shot('snippets-page-enabled.png');
  // A file dropped in the folder while the page is open.
  fs.writeFileSync(vp('.obsidian/snippets/late.css'), 'body { --text-error: #ff0055; }\n');
  await sleep(1800);
  res.afterDrop = await pop.evaluate(`${PAGE_JS} return Array.from(pane.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim());`);
  // And one deleted while enabled.
  fs.rmSync(vp('.obsidian/snippets/dim.css'), { force: true });
  await sleep(1800);
  res.afterDelete = {
    rows: await pop.evaluate(`${PAGE_JS} return Array.from(pane.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim());`),
    appearanceJson: appearanceJson(),
    enabledSnippets: await vault.evaluate(`return app.customCss && app.customCss.enabledSnippets ? Array.from(app.customCss.enabledSnippets) : null;`),
  };
  res.back = await goBack();
  res.backTo = await page();
  return res;
});

// ---- s10 a font sub-page -------------------------------------------------
await step('s10-font-page', async () => {
  await toAppearance();
  const res = {};
  res.opened = await clickRowNamed('Interface font');
  res.page = await page();
  res.shot = await pop.shot('interface-font-page.png');
  // What the page offers: a search field, a list of installed families, the
  // Obsidian-default row, and whatever a pick writes.
  res.detail = await pop.evaluate(`${PAGE_JS}
    const input = pane.querySelector('input[type=text], input[type=search], .search-input-container input');
    return {
      input: input ? { type: input.type, cls: input.className, placeholder: input.getAttribute('placeholder'), value: input.value, rect: rect(input) } : null,
      items: Array.from(pane.querySelectorAll('.setting-item, .suggestion-item, [class*="font"]')).slice(0, 60).map((el) => ({ cls: el.className, text: (el.textContent||'').trim().slice(0,80), rect: rect(el) })),
    };`);
  const before = appearanceJson();
  // Pick the second offered family, if the page lists any.
  res.pick = await pop.evaluate(`${PAGE_JS}
    const rows = Array.from(pane.querySelectorAll('.setting-item')).filter((x) => !x.classList.contains('setting-item-heading'));
    const it = rows[1]; if (!it) return null;
    return { name: ((it.querySelector('.setting-item-name')||{}).textContent||'').trim(), rect: rect(it) };`);
  if (res.pick) {
    await pop.clickAt(res.pick.rect.x + res.pick.rect.w / 2, res.pick.rect.y + res.pick.rect.h / 2);
    res.afterPick = { appearanceJson: await waitWrite(before), page: await page() };
    res.afterPickShot = await pop.shot('interface-font-page-picked.png');
  }
  res.back = await goBack();
  return res;
});

// ---- s11 the Manage modal, caught properly -------------------------------
// The first pass read `.modal` and got the Settings modal itself; the theme
// browser is appended after it, so take the last container.
await step('s11-manage-modal', async () => {
  await toAppearance();
  const r = await vault.evaluate(`${ROOT}${HELPERS}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    const b = it && Array.from(it.querySelectorAll('button')).find((x) => /manage/i.test(x.textContent||''));
    return b ? rect(b) : null;`);
  if (!r) return { noManageButton: true };
  const containersBefore = await pop.evaluate(`return document.querySelectorAll('.modal-container').length;`);
  await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2);
  await sleep(1400);
  const res = {
    containersBefore,
    containersAfter: await pop.evaluate(`return document.querySelectorAll('.modal-container').length;`),
    // The modal may be raised in the vault window rather than the popout.
    inVaultWindow: await vault.evaluate(`return document.querySelectorAll('.modal-container').length;`),
  };
  const dumpLast = `
    const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x:+b.x.toFixed(2), y:+b.y.toFixed(2), w:+b.width.toFixed(2), h:+b.height.toFixed(2) }; };
    const cs = document.querySelectorAll('.modal-container');
    const c = cs[cs.length - 1]; if (!c) return null;
    const m = c.querySelector('.modal');
    if (!m || /mod-settings/.test(m.className)) return { onlySettings: true, cls: m ? m.className : null };
    return { cls: m.className, title: ((m.querySelector('.modal-title')||{}).textContent||'').trim(), rect: rect(m),
      rows: Array.from(m.querySelectorAll('.setting-item, .community-item, .theme-list-item')).map((it) => ({ cls: it.className, text: (it.textContent||'').trim().slice(0, 200), rect: rect(it) })),
      buttons: Array.from(m.querySelectorAll('button, .clickable-icon')).map((b) => ({ text: (b.textContent||'').trim().slice(0,60), cls: b.className, aria: b.getAttribute('aria-label') })),
      html: m.innerHTML.length > 16000 ? m.innerHTML.slice(0, 16000) + '...' : m.innerHTML };`;
  res.popModal = await pop.evaluate(dumpLast);
  res.vaultModal = await vault.evaluate(dumpLast);
  res.shot = await pop.shot('themes-manage.png');
  await pop.evaluate(`const cs = document.querySelectorAll('.modal-container'); const c = cs[cs.length - 1]; if (c && !c.querySelector('.mod-settings')) c.remove(); return true;`);
  await vault.evaluate(`for (const c of document.querySelectorAll('.modal-container')) c.remove(); return true;`);
  await sleep(400);
  return res;
});

// ---- s12 the two rows whose text counts installed themes ----------------
await step('s12-counts', async () => {
  const read = async () => {
    await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(400);
    await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(1000);
    return {
      themes: await findRow('/^themes$/i'),
      community: await findRow('/current community themes/i'),
      snippets: await findRow('/css snippets/i'),
      api: await customCss(),
    };
  };
  const res = { withTwo: await read() };
  fs.rmSync(vp('.obsidian/themes/Zephyr'), { recursive: true, force: true });
  await sleep(1600);
  res.withOne = await read();
  fs.rmSync(vp('.obsidian/themes'), { recursive: true, force: true });
  fs.rmSync(vp('.obsidian/snippets'), { recursive: true, force: true });
  await sleep(1600);
  res.withNone = await read();
  res.emptyShot = await pop.shot('appearance-tab-no-themes.png');
  return res;
});

// ---- s13 what Manage opens ----------------------------------------------
// It raises no modal (s11 saw the container count stay at 0), so it must
// push a `.setting-page` the way the font rows do.
await step('s13-manage-page', async () => {
  await toAppearance();
  const r = await vault.evaluate(`${ROOT}${HELPERS}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    const b = it && Array.from(it.querySelectorAll('button')).find((x) => /manage/i.test(x.textContent||''));
    return b ? rect(b) : null;`);
  if (!r) return { noManageButton: true };
  await pop.clickAt(r.x + r.w / 2, r.y + r.h / 2);
  await sleep(1600);
  const res = { page: await page(), shot: await pop.shot('themes-manage-page.png') };
  res.pageShell = await pop.evaluate(`
    const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x:+b.x.toFixed(2), y:+b.y.toFixed(2), w:+b.width.toFixed(2), h:+b.height.toFixed(2) }; };
    const icon = (el) => { const s = el && el.querySelector('svg'); return s ? (s.getAttribute('class') || '') : null; };
    const p = document.querySelector('.setting-page');
    if (!p) return null;
    return {
      title: ((p.querySelector('.setting-page-title') || {}).textContent || '').trim(),
      back: (() => { const b = p.querySelector('.setting-page-back-button'); return b ? { cls: b.className, icon: icon(b), rect: rect(b) } : null; })(),
      desc: ((p.querySelector('.setting-page-desc') || {}).textContent || '').trim(),
      groups: Array.from(p.querySelectorAll('.setting-group')).map((g) => ({
        cls: g.className,
        heading: ((g.querySelector('.setting-item-heading .setting-item-name') || {}).textContent || '').trim(),
        search: !!g.querySelector('.search-input-container'),
        placeholder: (g.querySelector('input[type=search]') || {}).placeholder || null,
        items: Array.from(g.querySelectorAll('.setting-item')).map((it) => ({
          cls: it.className,
          name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(),
          desc: ((it.querySelector('.setting-item-description') || {}).textContent || '').trim().slice(0, 160),
          hidden: /display:\\s*none/.test(it.getAttribute('style') || ''),
          buttons: Array.from(it.querySelectorAll('button, .clickable-icon, .checkbox-container')).map((b) => ({ cls: b.className, text: (b.textContent||'').trim().slice(0,60), aria: b.getAttribute('aria-label'), icon: icon(b), on: b.classList.contains('is-enabled') })),
        })),
      })),
      html: p.innerHTML.length > 20000 ? p.innerHTML.slice(0, 20000) + '...' : p.innerHTML,
    };`);
  res.back = await goBack();
  return res;
});

// ---- s14 which order enabled snippets cascade in ------------------------
// s09 read the <style> elements as dim, tabs, wider after enabling in the
// order tabs, wider, dim: that says the applied order is the snippet list's
// (alphabetical), not `enabledCssSnippets`'s. Two snippets setting the same
// property settle it, since the later one in the document wins.
await step('s14-snippet-order', async () => {
  fs.mkdirSync(vp('.obsidian/snippets'), { recursive: true });
  for (const f of fs.readdirSync(vp('.obsidian/snippets'))) fs.rmSync(vp(path.join('.obsidian/snippets', f)), { force: true });
  fs.writeFileSync(vp('.obsidian/snippets/zz-last.css'), 'body { --text-error: #00ff00; }\n');
  fs.writeFileSync(vp('.obsidian/snippets/aa-first.css'), 'body { --text-error: #ff0000; }\n');
  fs.writeFileSync(vp('.obsidian/appearance.json'), '{}\n');
  await sleep(1500);
  await toAppearance();
  await clickRowNamed('CSS snippets');
  await sleep(600);
  // The page does not re-read the folder on its own (s09's afterDrop), so
  // the files written just now only appear once "Reload snippets" is hit.
  const reload = await pop.evaluate(`${PAGE_JS}
    const b = Array.from(pane.querySelectorAll('.clickable-icon')).find((x) => /reload/i.test(x.getAttribute('aria-label') || ''));
    return b ? rect(b) : null;`);
  if (reload) { await pop.clickAt(reload.x + reload.w / 2, reload.y + reload.h / 2); await sleep(1200); }
  const res = { reloadButton: reload, rows: (await page()).rows.map((r) => r.name) };
  const toggle = async (name) => {
    const before = appearanceJson();
    const hit = await pop.evaluate(`${PAGE_JS}
      const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => ((x.querySelector('.setting-item-name')||{}).textContent||'').trim() === ${JSON.stringify(name)});
      const t = it && it.querySelector('.checkbox-container');
      return t ? rect(t) : null;`);
    if (!hit) return { name, missing: true };
    await pop.clickAt(hit.x + hit.w / 2, hit.y + hit.h / 2);
    const after = await waitWrite(before);
    return {
      name, appearanceJson: after,
      enabledSnippets: await vault.evaluate(`return Array.from(app.customCss.enabledSnippets);`),
      styleOrder: await vault.evaluate(`return Array.from(document.head.querySelectorAll('style')).map((s) => (s.textContent||'').match(/#[0-9a-f]{6}/i)).map((m) => m ? m[0] : null).filter(Boolean);`),
      // The winner: green means zz-last is later in the document.
      textError: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--text-error').trim();`),
    };
  };
  // Enable the alphabetically-last one FIRST. If the applied order were the
  // enabled order, aa-first would win (red); if it is the list's, zz-last
  // wins (green).
  res.enabledZzThenAa = [await toggle('zz-last'), await toggle('aa-first')];
  // Now the other way round on a clean slate.
  for (const n of ['aa-first', 'zz-last']) await toggle(n);
  res.cleared = appearanceJson();
  res.enabledAaThenZz = [await toggle('aa-first'), await toggle('zz-last')];
  // And what the collapsed row's value reads with two enabled.
  res.back = await goBack();
  res.snippetsRow = await findRow('/css snippets/i');
  return res;
});

// ---- s15 the collapsed row's count --------------------------------------
// Every capture so far read "0 enabled" off the CSS snippets row, including
// with two enabled, so the value may simply be built once. Enable two, then
// leave Settings and come back.
await step('s15-snippet-count', async () => {
  fs.mkdirSync(vp('.obsidian/snippets'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/appearance.json'), JSON.stringify({ enabledCssSnippets: ['tabs', 'wider'] }, null, 2) + '\n');
  await sleep(1500);
  const res = {};
  await vault.evaluate(`app.setting.close(); return true;`); await sleep(600);
  await toAppearance();
  res.onOpen = await findRow('/css snippets/i');
  res.api = await customCss();
  // and with one of the two files gone
  fs.rmSync(vp('.obsidian/snippets/wider.css'), { force: true });
  await sleep(1500);
  await vault.evaluate(`app.setting.close(); return true;`); await sleep(600);
  await toAppearance();
  res.oneMissing = await findRow('/css snippets/i');
  res.apiAfter = await customCss();
  res.appearanceJson = appearanceJson();
  return res;
});

// ---- s16 the count's wording, enabled from inside the app ---------------
// s15 showed an external appearance.json write is not adopted while the app
// runs, so the count must be read after enabling through the page itself and
// re-entering the tab (the value element is built once per entry).
await step('s16-count-wording', async () => {
  await toAppearance();
  const res = { before: (await findRow('/css snippets/i')).value };
  await clickRowNamed('CSS snippets');
  await sleep(700);
  const names = (await page()).rows.map((r) => r.name).filter(Boolean);
  res.names = names;
  const toggle = async (name) => {
    const hit = await pop.evaluate(`${PAGE_JS}
      const it = Array.from(pane.querySelectorAll('.setting-item')).find((x) => ((x.querySelector('.setting-item-name')||{}).textContent||'').trim() === ${JSON.stringify(name)});
      const t = it && it.querySelector('.checkbox-container');
      return t ? rect(t) : null;`);
    if (!hit) return false;
    await pop.clickAt(hit.x + hit.w / 2, hit.y + hit.h / 2);
    await sleep(700);
    return true;
  };
  const reenter = async () => {
    await vault.evaluate(`app.setting.openTabById('general'); return true;`); await sleep(400);
    await vault.evaluate(`app.setting.openTabById('appearance'); return true;`); await sleep(900);
    return (await findRow('/css snippets/i')).value;
  };
  res.steps = [];
  await toggle(names[0]);
  res.steps.push({ enabled: 1, value: await reenter(), json: appearanceJson() });
  await clickRowNamed('CSS snippets'); await sleep(700);
  await toggle(names[1]);
  res.steps.push({ enabled: 2, value: await reenter(), json: appearanceJson() });
  // and with one of the two files removed from under it
  fs.rmSync(vp(path.join('.obsidian/snippets', names[1] + '.css')), { force: true });
  await sleep(1500);
  res.steps.push({ note: 'file of ' + names[1] + ' deleted', value: await reenter(), json: appearanceJson(), api: await customCss() });
  return res;
});

// ---- s17 does `cssTheme` hold the folder or the manifest's name? --------
// The loader has to find a theme's theme.css from the string in
// appearance.json, and the fixtures so far had folder == manifest name.
await step('s17-theme-identity', async () => {
  await toAppearance();
  const res = { api: await customCss() };
  res.options = await vault.evaluate(`${ROOT}
    const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
    const sel = it && it.querySelector('select');
    return sel ? Array.from(sel.options).map((o) => ({ v: o.value, t: o.textContent })) : null;`);
  res.tries = [];
  for (const o of res.options || []) {
    if (o.v === '') continue;
    const before = appearanceJson();
    await vault.evaluate(`${ROOT}
      const it = Array.from(root.querySelectorAll('.setting-item')).find((x) => /^themes$/i.test(((x.querySelector('.setting-item-name')||{}).textContent)||''));
      const sel = it.querySelector('select');
      sel.value = ${JSON.stringify(o.v)}; sel.dispatchEvent(new Event('change', { bubbles: true })); return sel.value;`);
    res.tries.push({
      option: o, appearanceJson: await waitWrite(before),
      textError: await vault.evaluate(`return getComputedStyle(document.body).getPropertyValue('--text-error').trim();`),
      customCssTheme: await vault.evaluate(`return app.customCss.theme;`),
    });
  }
  return res;
});

// ---- s18 Escape on a sub-page -------------------------------------------
// Does Escape back out of the CSS snippets page, or close Settings
// outright? Scoria's `escape_closes` takes the innermost thing open, and
// whether a page counts as one of those is Obsidian's answer to give.
await step('s18-escape-on-page', async () => {
  await toAppearance();
  await clickRowNamed('CSS snippets');
  await sleep(800);
  const onPage = await pop.evaluate(`${PAGE_JS} return { title: ((document.querySelector('.setting-page-title')||{}).textContent||'').trim(), rows: Array.from(pane.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim()) };`);
  await pop.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 });
  await pop.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 });
  await sleep(1000);
  const after = await pop.evaluate(`${PAGE_JS} return { title: ((document.querySelector('.setting-page-title')||{}).textContent||'').trim(), rows: Array.from(pane.querySelectorAll('.setting-item-name')).map((n) => n.textContent.trim()) };`).catch((e) => ({ ERR: e.message }));
  const settingsStillOpen = await vault.evaluate(`return !!(app.setting && app.setting.containerEl && app.setting.activeTab);`).catch(() => null);
  const targets = (await pageTargets()).map((t) => t.title);
  // And Escape again, from the tab's own rows.
  await pop.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }).catch(() => {});
  await pop.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }).catch(() => {});
  await sleep(1000);
  return {
    onPage, after, settingsStillOpen, targets,
    targetsAfterSecond: (await pageTargets()).map((t) => t.title),
  };
});

save();
await vault.evaluate(`app.setting.close(); return true;`).catch(() => {});
pop.close();
vault.close();
console.error('[appearance-tab] results: ' + RESULTS);
process.exit(0);
