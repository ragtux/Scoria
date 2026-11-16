// Spellcheck: which text Obsidian's editor hands Chromium's checker, what the
// squiggle looks like, what the right-click menu offers on a misspelled word,
// where "Add to dictionary" writes, and the Settings languages row.
// Real input over CDP (Input.dispatchMouseEvent / dispatchKeyEvent).
//
//   s00  context: version, the spellcheck config, Electron's spellchecker
//        state (webFrame, the session's languages), `spellcheck` attributes
//        inside `.cm-content`
//   s01  per context (plain, heading, code, links, URLs, tags, frontmatter,
//        math, HTML, digits, ALL-CAPS, ...): a unique misspelling in each;
//        a real right click on it, and the menu it raises — in Live Preview
//        and Source mode — plus `webFrame.isWordMisspelled` on the bare word
//   s02  the misspelled-word menu: rows, icons, sections; a word with no
//        suggestions; picking a suggestion (the bytes after)
//   s03  "Add to dictionary": the file it writes under the data directory,
//        and that the word is no longer flagged; then the reading view
//   s04  typing: when the squiggle appears (a word being typed, after a
//        space, after moving away) — screenshots at intervals
//   s05  the look: a clipped screenshot of a squiggled word, at 1x and 2x
//   s06  the Settings → Editor spellcheck rows: the languages row's markup,
//        what picking a language writes (app.json, localStorage, Preferences)
//   s07  spellcheck off: the attribute and the squiggles
//
// Screenshots go to <results>-shots/. ONLY_STEPS=<regex> narrows the run.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const UDD = process.env.USER_DATA_DIR || null;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });
const vp = (rel) => path.join(VAULT, rel);
const bytes = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

// One unique misspelling per context. `word` is what is right-clicked.
const CASES = [
  ['plain', 'recieve'],
  ['heading', 'acommodate'],
  ['bold', 'seperate'],
  ['italic', 'definately'],
  ['strike', 'tommorow'],
  ['highlight', 'begginer'],
  ['inline-code', 'occurence'],
  ['fenced-code', 'neccessary'],
  ['fenced-code-js-comment', 'wierdly'],
  ['indented-code', 'priviledge'],
  ['wikilink-target', 'Untill'],
  ['wikilink-alias', 'aliass'],
  ['wikilink-heading', 'Headding'],
  ['embed-target', 'Imaage'],
  ['md-link-text', 'goverment'],
  ['md-link-url', 'existance'],
  ['bare-url', 'begining'],
  ['autolink-url', 'calender'],
  ['email', 'exampel'],
  ['tag', 'arguement'],
  ['nested-tag', 'neglegence'],
  ['frontmatter-key', 'concensus'],
  ['frontmatter-value', 'existense'],
  ['frontmatter-list', 'Whereever'],
  ['inline-math', 'harrass'],
  ['block-math', 'embarass'],
  ['html-attr', 'publically'],
  ['html-text', 'enviroment'],
  ['comment', 'embarasment'],
  ['callout-type', 'notte'],
  ['callout-title', 'persue'],
  ['callout-body', 'wich'],
  ['block-id', 'bloock'],
  ['footnote-ref', 'fnoot'],
  ['footnote-def', 'definitionn'],
  ['list-item', 'liist'],
  ['task-item', 'taask'],
  ['quote', 'quoote'],
  ['table-cell', 'tabble'],
  ['digits-trailing', 'zorp42'],
  ['digits-inner', 'abc123xyz'],
  ['digits-leading', '2ndd'],
  ['all-caps', 'MISPELED'],
  ['title-case', 'Mispeled'],
  ['camel-case', 'camelCaseWrod'],
  ['apostrophe', "dont't"],
  ['hyphenated', 'awarness'],
  ['underscore', 'snake_case_wrod'],
  ['short', 'teh'],
  ['inline-field', 'inlinne'],
  ['entity', 'entitty'],
];

const NOTE = `---
concensus: existense
aliases:
  - Whereever
---
# acommodate heading

Plain recieve text.

Some **seperate** and *definately* and ~~tommorow~~ and ==begginer== here.

Inline \`occurence\` code.

\`\`\`
neccessary
\`\`\`

\`\`\`js
// wierdly
\`\`\`

    priviledge indented

Links [[Untill]] and [[Other|aliass]] and [[Other#Headding]] and ![[Imaage.png]].

A [goverment](https://example.com/existance) link and https://example.com/begining and <https://example.org/calender> and someone@exampel.com here.

Tags #arguement and #proj/neglegence here.

Math $harrass$ inline.

$$
embarass
$$

HTML <span title="publically">enviroment</span> here.

Comment %%embarasment%% here.

> [!notte] persue title
> wich body

Block line ^bloock

Footnote[^fnoot] here.

[^fnoot]: definitionn

- liist item
- [ ] taask

> quoote

| tabble | b |
| --- | --- |
| c | d |

Digits zorp42 and abc123xyz and 2ndd here.

Caps MISPELED and Mispeled and camelCaseWrod here.

Apostrophe dont't and self-awarness and snake_case_wrod and teh here.

Field inlinne:: value and &entitty; here.

Suggest wrold and qzxqzxqzvv here.

Typing line
`;

const NOTE_TYPED = NOTE;

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('Spell.md'), NOTE);
  fs.writeFileSync(vp('Other.md'), '# Other\n\nThe other note.\n');
  // Auto-pair off, so the word-rule lines are typed character for character.
  fs.writeFileSync(vp('.obsidian/app.json'), JSON.stringify({ autoPairBrackets: false, autoPairMarkdown: false }, null, 2));
  fs.writeFileSync(vp('Typed.md'), '');
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all.run = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 300000) {
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
  console.error(`[spell] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }
const ev = (s) => c.evaluate(s);
// A headless compositor without a keyboard never activates the window, and
// Chromium does not spellcheck a frame that does not have focus.
await c.send('Emulation.setFocusEmulationEnabled', { enabled: true }).catch(() => null);
out.hasFocus = await ev(`return document.hasFocus();`);
// In the headless session the renderer's dictionary is not loaded at launch
// (isWordMisspelled says false for every word, nothing is squiggled) until
// the session's checker is switched off and on and its languages set again.
out.kick = await ev(`
  const s = require('electron').remote.session.defaultSession; const wf = require('electron').webFrame;
  const before = wf.isWordMisspelled('recieve');
  const langs = s.getSpellCheckerLanguages();
  s.setSpellCheckerEnabled(false); await new Promise((r) => setTimeout(r, 300)); s.setSpellCheckerEnabled(true);
  s.setSpellCheckerLanguages(langs);
  await new Promise((r) => setTimeout(r, 4000));
  return { before, after: wf.isWordMisspelled('recieve'), langs };`);

const installHelpers = () => ev(`
window.__s = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
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
  ed() { return this.leaf().view.editor; },
  root(mode) { return document.querySelector(mode === 'preview' ? '.workspace-leaf.mod-active .markdown-reading-view' : '.workspace-leaf.mod-active .cm-editor'); },
  // Viewport rect of the n-th occurrence of a word in the root's text.
  wordRect(word, mode, nth = 0) {
    const root = this.root(mode); if (!root) return { noRoot: true };
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let n, k = 0;
    while ((n = walker.nextNode())) {
      let i = -1;
      while ((i = n.textContent.indexOf(word, i + 1)) >= 0) {
        if (k++ < nth) continue;
        const range = document.createRange(); range.setStart(n, i); range.setEnd(n, i + word.length);
        const r = range.getBoundingClientRect();
        if (!r.width) continue;
        let el = n.parentElement; const chain = [];
        for (let d = 0; el && d < 6 && !el.classList.contains('cm-content'); d++, el = el.parentElement) {
          chain.push({ tag: el.tagName.toLowerCase(), cls: el.getAttribute('class'), spellcheck: el.getAttribute('spellcheck'), ce: el.getAttribute('contenteditable') });
        }
        return { x: r.x, y: r.y, w: r.width, h: r.height, cx: r.x + r.width / 2, cy: r.y + r.height / 2, chain, effectiveSpellcheck: n.parentElement.spellcheck, editable: n.parentElement.isContentEditable };
      }
    }
    return { notFound: true };
  },
  // Scroll the n-th occurrence of a word into the middle of the editor and
  // park the caret on the nearest blank line, so Live Preview renders it.
  async reveal(word) {
    const ed = this.ed(); const text = ed.getValue();
    const at = text.indexOf(word); if (at < 0) return null;
    const pos = ed.offsetToPos(at);
    let park = pos.line + 1;
    while (park < ed.lineCount() && ed.getLine(park).trim() !== '') park++;
    if (park >= ed.lineCount()) park = ed.lastLine();
    ed.focus();
    ed.setCursor({ line: park, ch: 0 });
    ed.scrollIntoView({ from: pos, to: pos }, true);
    await this.sleep(400);
    return { pos, park };
  },
  menus() {
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
          cls: it.className,
        };
      }),
    }));
  },
};
return true;`);
await installHelpers();
// The text already on screen was "checked" before the dictionary arrived and
// is never looked at again; a reload of the renderer, now that the browser
// side holds the dictionary, checks the note from scratch.
await c.send('Page.reload', {});
await sleep(1500);
for (let i = 0; i < 60; i++) {
  const ready = await ev(`return !!(window.app && app.workspace && app.workspace.layoutReady);`).catch(() => false);
  if (ready) break;
  await sleep(500);
}
await sleep(1500);
await c.send('Emulation.setFocusEmulationEnabled', { enabled: true }).catch(() => null);
await installHelpers();
out.afterReload = await ev(`return { misspelled: require('electron').webFrame.isWordMisspelled('recieve') };`);
const S = 'const S = window.__s;';
let shotN = 0;
async function shot(name, clip) {
  try {
    const r = await c.send('Page.captureScreenshot', clip ? { format: 'png', clip: { ...clip, scale: clip.scale || 1 } } : { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
const open = (m, file = 'Spell.md') => ev(`${S} return await S.open(${JSON.stringify(file)}, '${m}');`);
const wordRect = (w, m, nth = 0) => ev(`${S} return S.wordRect(${JSON.stringify(w)}, '${m}', ${nth});`);
const menus = () => ev(`${S} return S.menus();`);
const esc = async () => { await c.pressKey('Escape'); await sleep(300); };
const focusEnd = () => ev(`${S} const ed = S.ed(); ed.focus(); ed.setCursor({ line: ed.lastLine(), ch: 0 }); return ed.getCursor();`);
const webFrame = (w) => ev(`const wf = require('electron').webFrame; return { misspelled: wf.isWordMisspelled(${JSON.stringify(w)}), suggestions: wf.getWordSuggestions(${JSON.stringify(w)}) };`);
const rightClick = async (rect) => {
  await c.clickAt(rect.cx, rect.cy, { button: 'right' });
  await sleep(800);
  const m = await menus();
  const sel = await ev(`${S} const ed = S.ed(); return ed ? { selection: ed.getSelection(), cursor: ed.getCursor() } : null;`).catch(() => null);
  return { menus: m, after: sel };
};
const spellRows = (m) => {
  if (!m || !m.length) return null;
  return m[0].items.filter((i) => i.section === 'spellcheck' || /dictionary|suggestion/i.test(i.title || ''));
};

// Ad hoc: EVAL_FILE names a file of renderer JS to run (exploration only).
if (process.env.EVAL_FILE) {
  await step('sx-eval', async () => {
    await open('live'); await focusEnd(); await sleep(2000);
    return { value: await ev(fs.readFileSync(process.env.EVAL_FILE, 'utf8')), shot: await shot('eval') };
  });
}

// What makes Chromium check text that was already there when the note opened.
await step('sx2-existing-text', async (r) => {
  await open('source'); await focusEnd(); await sleep(3000);
  r.idle = await shot('existing-idle');
  const rect = await wordRect('zorp42', 'source');
  await c.clickAt(rect.cx, rect.cy); await sleep(3000);
  r.afterClick = await shot('existing-after-click');
  for (let i = 0; i < 6; i++) { await c.pressKey('ArrowUp'); await sleep(150); }
  await sleep(2500);
  r.afterArrows = await shot('existing-after-arrows');
  await c.pressKey('PageUp'); await sleep(2500);
  r.afterPageUp = await shot('existing-after-pageup');
  // Insert the note as typed input: an empty note, then Input.insertText.
  await ev(`${S} const ed = S.ed(); await app.vault.modify(S.leaf().view.file, ''); await S.sleep(300); ed.focus(); ed.setCursor({ line: 0, ch: 0 }); return true;`);
  await sleep(500);
  await c.send('Input.insertText', { text: NOTE_TYPED });
  await sleep(3000);
  r.value = await ev(`${S} return S.ed().getValue();`);
  r.afterInsert = await shot('existing-after-insert');
  return r;
});

await step('s00-context', async () => {
  const version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio, apiVersion: (() => { try { return require('obsidian').apiVersion; } catch { return null; } })() };`);
  out.version = version;
  const cfg = await ev(`return { spellcheck: app.vault.getConfig('spellcheck'), spellcheckLanguages: app.vault.getConfig('spellcheckLanguages'), spellcheckDictionary: app.vault.getConfig('spellcheckDictionary') };`);
  const electron = await ev(`
    const e = require('electron');
    const o = { electronKeys: Object.keys(e) };
    try { o.webFrameKeys = Object.getOwnPropertyNames(Object.getPrototypeOf(e.webFrame)).filter((k) => /spell|word/i.test(k)); } catch (x) { o.webFrameErr = String(x); }
    try { const r = e.remote || require('@electron/remote'); o.remote = !!r; const s = r.session.defaultSession;
      o.languages = s.getSpellCheckerLanguages(); o.available = s.availableSpellCheckerLanguages; o.words = await s.listWordsInSpellCheckerDictionary();
      o.enabled = s.isSpellCheckerEnabled ? s.isSpellCheckerEnabled() : null; } catch (x) { o.remoteErr = String(x); }
    o.localStorage = Object.keys(localStorage).filter((k) => /spell|lang|dict/i.test(k)).map((k) => [k, localStorage.getItem(k)]);
    return o;`);
  await open('live');
  await focusEnd();
  await sleep(1500);
  const attrs = await ev(`
    const root = document.querySelector('.workspace-leaf.mod-active .cm-editor');
    const content = root.querySelector('.cm-content');
    const flagged = Array.from(root.querySelectorAll('[spellcheck]')).map((e) => ({ tag: e.tagName.toLowerCase(), cls: e.getAttribute('class'), spellcheck: e.getAttribute('spellcheck'), text: (e.textContent || '').slice(0, 60) }));
    return { content: { spellcheck: content.getAttribute('spellcheck'), autocorrect: content.getAttribute('autocorrect'), autocapitalize: content.getAttribute('autocapitalize'), lang: content.getAttribute('lang') || document.documentElement.lang }, flagged };`);
  const preferences = UDD ? (() => { try { const p = JSON.parse(bytes(path.join(UDD, 'Preferences'))); return { spellcheck: p.spellcheck || null, intl: p.intl || null }; } catch (e) { return 'ERR ' + e.message; } })() : null;
  const customDict = UDD ? bytes(path.join(UDD, 'Custom Dictionary.txt')) : null;
  const shotFile = await shot('live-overview');
  return { version, cfg, electron, attrs, preferences, customDict, shotFile };
});

for (const m of ['live', 'source']) {
  await step(`s01-contexts-${m}`, async (r) => {
    r.opened = await open(m);
    await focusEnd();
    await sleep(2500);
    r.shot = await shot(`contexts-${m}`);
    r.cases = {};
    for (const [name, word] of CASES) {
      const bare = word.replace(/[^A-Za-z0-9_'-]/g, '');
      const rec = { word, webFrame: await webFrame(bare).catch((e) => String(e)) };
      rec.revealed = await ev(`${S} return await S.reveal(${JSON.stringify(word)});`);
      await sleep(1200);
      const rect = await wordRect(word, m);
      rec.rect = rect;
      if (rect && rect.cx) {
        const rc = await rightClick(rect);
        rec.menu = rc.menus.length ? rc.menus[0].items.map((i) => i.separator ? '---' : i.title) : null;
        rec.spellRows = spellRows(rc.menus);
        rec.after = rc.after;
        if (rc.menus.length) await esc();
      }
      r.cases[name] = rec;
    }
    return r;
  }, 600000);
}

// Chromium marks only what is typed (see s01's note): these steps type their
// text into an empty note first, character by character, then right click.
const typeFresh = async (m, text) => {
  await open(m, 'Typed.md');
  await ev(`${S} const ed = S.ed(); await app.vault.modify(S.leaf().view.file, ''); await S.sleep(300); ed.focus(); ed.setCursor({ line: 0, ch: 0 }); return true;`);
  await sleep(400);
  await c.typeText(text, 25);
  await sleep(1500);
  return ev(`${S} return S.ed().getValue();`);
};
const menuAt = async (word, m, nth = 0) => {
  const rect = await wordRect(word, m, nth);
  if (!rect || !rect.cx) return { rect };
  const rc = await rightClick(rect);
  const items = rc.menus.length ? rc.menus[0].items : [];
  if (rc.menus.length) await esc();
  return { items, after: rc.after };
};
const summary = (items) => (items || []).map((i) => i.separator ? '---' : `${i.title}${i.section ? ' [' + i.section + ']' : ''}${i.icon ? ' <' + i.icon + '>' : ''}${i.disabled ? ' (disabled)' : ''}`);

// Word rules: which typed tokens Chromium flags in a plain paragraph.
const WORDS = ['zorp42', 'abc123xyz', '2ndd', '42nd', '1st', '3rd', 'MISPELED', 'Mispeled', 'mIspeled', 'camelCaseWrod', 'dont', "dont't", "don't", 'don’t', "recieve's", 'self-awarness', 'snake_case_wrod', 'teh', 'Imaage.png', 'e.g.', 'naive', 'naïve', 'café', 'café', 'x', 'I', 'i', 'Zorpville', 'ZORPVILLE', 'wrold.', 'qzxqzxqzvv', 'it’s', 'recieve’s', 'v2', 'ABCs', 'URLs', 'mp3', 'COVID-19', 'web3', 'hello_world', 'foo.bar', 'a.b.c', 'Mc­Donald', "o'clock", "'quoted'", "rock'n'roll", 'zorp·ville', 'été', 'straße', 'Dr.', 'Mr', 'iPhone', 'macOS', 'GitHub', 'JavaScript', 'e-mail', 'co-operate', 'zz-top', 'top-zz'];
// Right click the middle of [from, to) of the editor's text, by offset.
const menuAtOffset = async (from, to) => {
  const rect = await ev(`${S} const cm = S.ed().cm; const a = cm.coordsAtPos(${from}), b = cm.coordsAtPos(${to}, -1);
    if (!a || !b) return null; return { cx: (a.left + b.right) / 2, cy: (a.top + a.bottom) / 2 };`);
  if (!rect) return { rect };
  const rc = await rightClick(rect);
  const items = rc.menus.length ? rc.menus[0].items : [];
  if (rc.menus.length) await esc();
  return { items, after: rc.after };
};
await step('s01b-word-rules', async (r) => {
  const line = WORDS.join(' ') + ' end';
  r.value = await typeFresh('source', line + '\n');
  r.shot = await shot('word-rules');
  r.words = {};
  let at = 0;
  for (const w of WORDS) {
    const start = line.indexOf(w, at); at = start + w.length;
    // Each part between separators, clicked on its own.
    const parts = [];
    const re = /[^\-_.·\s]+/g; let m;
    while ((m = re.exec(w))) parts.push([m[0], start + m.index, start + m.index + m[0].length]);
    const res = {};
    for (const [p, f, t] of parts.length ? parts : [[w, start, start + w.length]]) {
      const got = await menuAtOffset(f, t);
      const rows = summary(got.items);
      const sep = rows.indexOf('---');
      res[p] = { flagged: !rows.length ? null : !/^Add link/.test(rows[0]), head: rows.slice(0, Math.max(sep, 1) + 1), selection: got.after && got.after.selection };
    }
    r.words[w] = res;
  }
  return r;
}, 900000);

await step('s02-menu', async (r) => {
  r.value = await typeFresh('live', 'Suggest wrold and qzxqzxqzvv and recieve here.\n');
  for (const w of ['wrold', 'qzxqzxqzvv', 'recieve']) {
    const rect = await wordRect(w, 'live');
    await c.clickAt(rect.cx, rect.cy, { button: 'right' });
    await sleep(800);
    const m = await menus();
    r[w] = { rect, rows: m.length ? summary(m[0].items) : null, raw: m, after: await ev(`${S} const ed = S.ed(); return { selection: ed.getSelection(), cursor: ed.getCursor() };`), shot: await shot(`menu-${w}`) };
    await esc();
  }
  // The same word in Source mode.
  await open('source', 'Typed.md');
  await sleep(500);
  const rs = await menuAt('wrold', 'source');
  r.sourceRows = summary(rs.items);
  // Pick the first suggestion for "wrold".
  await open('live', 'Typed.md');
  const rect = await wordRect('wrold', 'live');
  await c.clickAt(rect.cx, rect.cy, { button: 'right' }); await sleep(800);
  r.picked = await ev(`
    const items = Array.from(document.querySelectorAll('.menu .menu-item'));
    const first = items[0]; const title = first && first.querySelector('.menu-item-title').textContent;
    first && first.click(); return title;`);
  await sleep(800);
  r.after = await ev(`${S} const ed = S.ed(); return { value: ed.getValue(), cursor: ed.getCursor(), selection: ed.getSelection() };`);
  await c.pressKey('z', { ctrl: true });
  await sleep(400);
  r.afterUndo = await ev(`${S} const ed = S.ed(); return { value: ed.getValue(), cursor: ed.getCursor() };`);
  // A selection over a misspelled word, then the right click inside it.
  await ev(`${S} const ed = S.ed(); const v = ed.getValue(); const i = v.indexOf('recieve'); ed.setSelection(ed.offsetToPos(i), ed.offsetToPos(i + 7)); ed.focus(); return true;`);
  const rr = await wordRect('recieve', 'live');
  await c.clickAt(rr.cx, rr.cy, { button: 'right' }); await sleep(800);
  const ms = await menus();
  r.withSelection = ms.length ? summary(ms[0].items) : null;
  await esc();
  return r;
});

await step('s03-add-to-dictionary', async (r) => {
  r.value = await typeFresh('live', 'Add qzxqzxqzvv and Qzxqzxqzvv and QZXQZXQZVV here.\n');
  r.before = UDD ? bytes(path.join(UDD, 'Custom Dictionary.txt')) : null;
  const rect = await wordRect('qzxqzxqzvv', 'live');
  await c.clickAt(rect.cx, rect.cy, { button: 'right' }); await sleep(800);
  const m0 = await menus();
  r.menu = m0.length ? summary(m0[0].items) : null;
  r.clicked = await ev(`
    const it = Array.from(document.querySelectorAll('.menu .menu-item')).find((i) => /dictionary/i.test(i.textContent));
    if (!it) return null; const t = it.textContent.trim(); it.click(); return t;`);
  await sleep(2000);
  r.after = UDD ? bytes(path.join(UDD, 'Custom Dictionary.txt')) : null;
  r.udd = UDD ? fs.readdirSync(UDD).filter((f) => /dict|spell/i.test(f)) : null;
  r.vaultFiles = fs.readdirSync(vp('.obsidian'));
  r.appJson = bytes(vp('.obsidian/app.json'));
  r.webFrameAfter = await webFrame('qzxqzxqzvv');
  r.words = await ev(`try { const s = require('electron').remote.session.defaultSession; return await s.listWordsInSpellCheckerDictionary(); } catch (e) { return String(e); }`);
  r.shotAfter = await shot('after-add');
  for (const w of ['qzxqzxqzvv', 'Qzxqzxqzvv', 'QZXQZXQZVV']) {
    const res = await menuAt(w, 'live');
    r['menuAfter ' + w] = summary(res.items).slice(0, 6);
  }
  r.caseVariants = { upper: await webFrame('QZXQZXQZVV'), title: await webFrame('Qzxqzxqzvv') };
  // Is there a way back? Remove from dictionary via the session, as a check of what Obsidian offers.
  // Reading view: does the right click there say anything?
  await open('preview', 'Typed.md');
  await sleep(1000);
  const rr = await wordRect('Add', 'preview');
  if (rr && rr.cx) {
    await c.clickAt(rr.cx, rr.cy, { button: 'right' }); await sleep(800);
    const mm = await menus();
    r.readingMenu = mm.length ? summary(mm[0].items) : null;
    await esc();
  }
  return r;
});

await step('s04-typing', async (r) => {
  await open('source');
  await ev(`${S} const ed = S.ed(); ed.focus(); const l = ed.lastLine(); ed.setCursor({ line: l, ch: ed.getLine(l).length }); return true;`);
  await sleep(500);
  const lineRect = async () => ev(`${S} const ed = S.ed(); const lines = document.querySelectorAll('.workspace-leaf.mod-active .cm-line'); const el = lines[lines.length - 1]; const b = el.getBoundingClientRect(); return { x: b.x, y: b.y - 4, width: Math.min(500, b.width), height: b.height + 8 };`);
  await c.typeText(' wrold', 80);
  const clip = await lineRect();
  r.clip = clip;
  r.shots = [];
  r.shots.push(['mid-word 0ms', await shot('typing-midword-0', clip)]);
  await sleep(700); r.shots.push(['mid-word 700ms', await shot('typing-midword-700', clip)]);
  await sleep(2500); r.shots.push(['mid-word 3200ms', await shot('typing-midword-3200', clip)]);
  await c.typeText(' ', 80);
  r.shots.push(['space 0ms', await shot('typing-space-0', clip)]);
  await sleep(300); r.shots.push(['space 300ms', await shot('typing-space-300', clip)]);
  await sleep(1000); r.shots.push(['space 1300ms', await shot('typing-space-1300', clip)]);
  // Back into the word and edit it to a correct one.
  await c.pressKey('ArrowLeft'); await c.pressKey('ArrowLeft'); await sleep(300);
  r.shots.push(['caret back in word', await shot('typing-back-in-word', clip)]);
  await c.pressKey('Backspace'); await c.pressKey('Backspace'); await c.pressKey('Backspace');
  await c.typeText('orl', 80); await sleep(800);
  r.line = await ev(`${S} const ed = S.ed(); return ed.getLine(ed.lastLine());`);
  r.shots.push(['fixed to world', await shot('typing-fixed', clip)]);
  // A misspelling typed and then left with an arrow key, no space.
  await c.pressKey('End'); await c.typeText(' thsi', 80); await sleep(300);
  await c.pressKey('ArrowUp'); await sleep(800);
  r.shots.push(['left with ArrowUp', await shot('typing-left-up', clip)]);
  r.lineAfter = await ev(`${S} const ed = S.ed(); return ed.getLine(ed.lastLine());`);
  return r;
});

await step('s05-look', async (r) => {
  for (const m of ['source', 'live']) {
    r[m] = {};
    r[m].value = await typeFresh(m, 'Plain recieve and **seperate** and # no.\n');
    const rect = await wordRect('recieve', m);
    r[m].rect = rect;
    r[m].font = await ev(`const el = document.querySelector('.workspace-leaf.mod-active .cm-line'); const cs = getComputedStyle(el); return { fontSize: cs.fontSize, lineHeight: cs.lineHeight, fontFamily: cs.fontFamily, color: cs.color };`);
    const clip = { x: rect.x - 4, y: rect.y - 2, width: rect.w + 8, height: rect.h + 6 };
    r[m].shot1x = await shot(`look-${m}-1x`, { ...clip, scale: 1 });
    r[m].shot4x = await shot(`look-${m}-4x`, { ...clip, scale: 4 });
    const b = await wordRect('seperate', m);
    r[m].boldShot4x = await shot(`look-${m}-bold-4x`, { x: b.x - 4, y: b.y - 2, width: b.w + 8, height: b.h + 6, scale: 4 });
  }
  // Light theme.
  await ev(`document.body.classList.remove('theme-dark'); document.body.classList.add('theme-light'); return true;`);
  await sleep(800);
  const rect3 = await wordRect('recieve', 'live');
  r.shotLight = await shot('look-light-4x', { x: rect3.x - 4, y: rect3.y - 2, width: rect3.w + 8, height: rect3.h + 6, scale: 4 });
  await ev(`document.body.classList.remove('theme-light'); document.body.classList.add('theme-dark'); return true;`);
  // A device scale of 2.
  await c.send('Emulation.setDeviceMetricsOverride', { width: 0, height: 0, deviceScaleFactor: 2, mobile: false });
  await sleep(1200);
  const rect2 = await wordRect('recieve', 'live');
  r.shotDpr2 = await shot('look-dpr2', { x: rect2.x - 4, y: rect2.y - 2, width: rect2.w + 8, height: rect2.h + 6, scale: 1 });
  await c.send('Emulation.clearDeviceMetricsOverride', {});
  await sleep(500);
  return r;
});

await step('s06-settings', async (r) => {
  const prefs = () => { if (!UDD) return null; try { const p = JSON.parse(bytes(path.join(UDD, 'Preferences'))); return { spellcheck: p.spellcheck || null, intl: p.intl || null }; } catch (e) { return 'ERR ' + e.message; } };
  r.appJsonBefore = bytes(vp('.obsidian/app.json'));
  r.prefsBefore = prefs();
  await ev(`app.setting.open(); app.setting.openTabById('editor'); return true;`);
  await sleep(1000);
  r.rows = await ev(`
    const el = app.setting.activeTab.containerEl;
    return Array.from(el.querySelectorAll('.setting-item')).filter((it) => /spell|language|dictionar/i.test(it.textContent)).map((it) => ({
      name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(),
      description: ((it.querySelector('.setting-item-description') || {}).textContent || '').trim(),
      html: it.outerHTML.slice(0, 3000), hidden: it.offsetParent === null }));`);
  r.shot = await shot('settings-editor');
  // The cog beside the Spellcheck toggle.
  r.cog = await ev(`
    const el = app.setting.activeTab.containerEl;
    const it = Array.from(el.querySelectorAll('.setting-item')).find((i) => ((i.querySelector('.setting-item-name') || {}).textContent || '').trim() === 'Spellcheck');
    const b = it && it.querySelector('.extra-setting-button'); if (!b) return null;
    const aria = b.getAttribute('aria-label'); b.click(); await new Promise((r) => setTimeout(r, 800));
    const docs = [document, el.ownerDocument]; const found = [];
    for (const d of docs) for (const m of d.querySelectorAll('.modal, .menu, .popover')) found.push(m);
    const m = found[found.length - 1];
    const rows = m ? Array.from(m.querySelectorAll('.setting-item, .menu-item, li, .mod-cta, button, input, textarea')).map((x) => ({ tag: x.tagName.toLowerCase(), cls: x.className, text: (x.innerText || x.value || '').slice(0, 200), placeholder: x.getAttribute('placeholder') })) : null;
    return { aria, sameDoc: el.ownerDocument === document, count: found.length, modal: m ? { cls: m.className, title: ((m.querySelector('.modal-title') || {}).textContent || '').trim(), html: m.outerHTML.slice(0, 6000), text: m.innerText.slice(0, 2000), rows } : null };`);
  // Remove the word from the modal, then read the file and the emptied modal.
  r.customBeforeRemove = UDD ? bytes(path.join(UDD, 'Custom Dictionary.txt')) : null;
  r.removed = await ev(`
    const d = app.setting.activeTab.containerEl.ownerDocument;
    const b = d.querySelector('.spellchecker-dictionary-remove-button'); if (!b) return null;
    b.click(); await new Promise((r) => setTimeout(r, 1500));
    const m = d.querySelector('.modal'); const cs = (e) => { if (!e) return null; const c = getComputedStyle(e); return { w: e.getBoundingClientRect().width, h: e.getBoundingClientRect().height, font: c.fontSize, color: c.color, bg: c.backgroundColor, pad: c.padding }; };
    return { html: m ? m.outerHTML.slice(0, 2000) : null, text: m ? m.innerText : null, modal: cs(m) };`);
  await sleep(1000);
  r.customAfterRemove = UDD ? bytes(path.join(UDD, 'Custom Dictionary.txt')) : null;
  r.sessionWordsAfterRemove = await ev(`try { return await require('electron').remote.session.defaultSession.listWordsInSpellCheckerDictionary(); } catch (e) { return String(e); }`);
  await esc();
  await ev(`app.setting.open(); app.setting.openTabById('editor'); return true;`); await sleep(600);
  // The languages control: a select? buttons? Record and try to add one.
  r.control = await ev(`
    const el = app.setting.activeTab.containerEl;
    const it = Array.from(el.querySelectorAll('.setting-item')).find((i) => /languages/i.test((i.querySelector('.setting-item-name') || {}).textContent || ''));
    if (!it) return null;
    const sel = it.querySelector('select');
    return { select: sel ? { value: sel.value, options: Array.from(sel.options).map((o) => [o.value, o.textContent]) } : null,
      buttons: Array.from(it.querySelectorAll('button, .clickable-icon')).map((b) => ({ text: b.textContent.trim(), aria: b.getAttribute('aria-label'), cls: b.className })) };`);
  if (r.control && r.control.select) {
    const opt = r.control.select.options.find((o) => /^de/i.test(o[0])) || r.control.select.options[1];
    r.picked = opt;
    r.setResult = await c.selectSet('.vertical-tab-content select', opt[0]).catch((e) => String(e));
    // The select may not be the first on the tab; set it by row instead.
    r.setResult2 = await ev(`
      const el = app.setting.activeTab.containerEl;
      const it = Array.from(el.querySelectorAll('.setting-item')).find((i) => /languages/i.test((i.querySelector('.setting-item-name') || {}).textContent || ''));
      const sel = it.querySelector('select'); sel.value = ${JSON.stringify(opt[0])}; sel.dispatchEvent(new Event('change', { bubbles: true }));
      return sel.value;`);
    await sleep(2500);
    r.rowsAfter = await ev(`
      const el = app.setting.activeTab.containerEl;
      return Array.from(el.querySelectorAll('.setting-item')).filter((it) => /languages/i.test(it.textContent)).map((it) => it.outerHTML.slice(0, 3000));`);
    r.shotAfter = await shot('settings-editor-after');
  }
  r.appJsonAfter = bytes(vp('.obsidian/app.json'));
  r.prefsAfter = prefs();
  r.sessionLanguages = await ev(`try { const s = (require('electron').remote || require('@electron/remote')).session.defaultSession; return s.getSpellCheckerLanguages(); } catch (e) { return String(e); }`);
  r.localStorage = await ev(`return Object.keys(localStorage).filter((k) => /spell|lang|dict/i.test(k)).map((k) => [k, localStorage.getItem(k)]);`);
  r.cfg = await ev(`return { spellcheckLanguages: app.vault.getConfig('spellcheckLanguages') };`);
  // Spellcheck off: the rows that remain.
  await ev(`app.vault.setConfig('spellcheck', false); return true;`);
  await sleep(800);
  await ev(`app.setting.openTabById('editor'); return true;`); await sleep(800);
  r.rowsOff = await ev(`
    const el = app.setting.activeTab.containerEl;
    return Array.from(el.querySelectorAll('.setting-item')).filter((it) => /spell|language/i.test(it.textContent)).map((it) => ({ name: ((it.querySelector('.setting-item-name') || {}).textContent || '').trim(), hidden: it.offsetParent === null }));`);
  await ev(`app.vault.setConfig('spellcheck', true); app.setting.close(); return true;`);
  await sleep(500);
  return r;
});

await step('s07-off', async (r) => {
  r.value = await typeFresh('source', 'Off recieve here.\n');
  r.shotOn = await shot('spellcheck-on');
  await ev(`app.vault.setConfig('spellcheck', false); return true;`);
  await sleep(1500);
  r.attr = await ev(`return document.querySelector('.workspace-leaf.mod-active .cm-content').getAttribute('spellcheck');`);
  r.shotOff = await shot('spellcheck-off');
  const res = await menuAt('recieve', 'source');
  r.menu = summary(res.items);
  await ev(`app.vault.setConfig('spellcheck', true); return true;`);
  await sleep(1500);
  r.shotBackOn = await shot('spellcheck-back-on');
  r.attrBack = await ev(`return document.querySelector('.workspace-leaf.mod-active .cm-content').getAttribute('spellcheck');`);
  return r;
});

save();
c.close();
process.exit(0);
