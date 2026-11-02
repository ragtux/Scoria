// Syntax highlighting: step 3 of docs/scoria/Theming.md. Two engines have to
// be told apart before anything is built —
//
//   the reading view highlights with Prism.js, so a rendered block is
//   `span.token.<name>` and a theme styles `.token.keyword`;
//   the editor is CodeMirror, which has its own tokens and its own classes,
//
// and Scoria paints both from one model, so what each one emits, and which
// `--code-*` variable each class resolves to, decides the whole design.
//
// Records:
//   langs      every key of `Prism.languages`, with the aliases folded onto
//              the grammar they share, and the component list Obsidian ships
//   reading    per language: every `span.token` with its classes, its text,
//              its computed colour/weight/style and the winning cascade
//              declaration (so `--code-keyword` comes back with var() intact)
//   editor     the same block in Live Preview and in source mode, read off
//              CodeMirror's own spans
//   map        the collapsed class -> custom property table, per theme
//
// Phases: fixtures | run.   ONLY_STEPS=s02,s04 re-runs a step.
// SHOTS names the folder the PNGs go in.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const SHOTS = process.env.SHOTS || RESULTS.replace(/\.json$/, '-shots');
const vp = (rel) => path.join(VAULT, rel);

const out = fs.existsSync(RESULTS) ? JSON.parse(fs.readFileSync(RESULTS, 'utf8')) : {};
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(out, null, 1));
const only = (process.env.ONLY_STEPS || '').split(',').filter(Boolean);
const step = async (id, fn) => {
  if (only.length && !only.includes(id)) return;
  try { out[id] = await fn(); } catch (e) { out[id] = { ERR: String(e && e.stack ? e.stack : e) }; }
  save();
  console.error('[code-highlight] ' + id);
};

// ---- the sample blocks ---------------------------------------------------
// One fence per language, each written to exercise as many of Prism's token
// names as a short sample can: comments, strings, numbers, keywords,
// functions, operators, punctuation, properties, tags.
const SAMPLES = {
  js: `// a comment\nimport { thing } from "./mod.js";\nconst n = 42, s = 'text', re = /ab+c/g;\nexport default function greet(name = \`world\`) {\n  /* block */\n  if (name && n > 3) return \`hi \${name}\`;\n  return null;\n}\nclass A extends B { #priv = 1; get x() { return this.#priv; } }\n`,
  typescript: `type Id = string | number;\ninterface User<T> { id: Id; tags: T[]; }\nenum Kind { One = 1, Two }\nexport const f = async (u: User<string>): Promise<void> => {\n  await Promise.all(u.tags.map((t) => console.log(t)));\n};\n`,
  python: `# comment\nfrom os import path\nimport sys\n\n@decorator\nclass Thing(Base):\n    """A docstring."""\n    def __init__(self, n: int = 0) -> None:\n        self.n = n or 0x1f\n        self.s = f"n is {n}"\n\n    async def go(self):\n        for i in range(10):\n            if i % 2 == 0 and not self.n:\n                yield i\n`,
  rust: `// comment\nuse std::collections::HashMap;\n\n#[derive(Debug, Clone)]\npub struct Point { pub x: f64, y: Option<u8> }\n\nimpl Point {\n    pub fn new(x: f64) -> Self {\n        let s = "text"; let c = 'c'; let n = 0xff_u8;\n        Self { x, y: None }\n    }\n}\nfn main() { let mut m: HashMap<&str, i32> = HashMap::new(); m.insert("a", 1); }\n`,
  json: `{\n  "name": "scoria",\n  "version": 3,\n  "ok": true,\n  "none": null,\n  "list": [1, 2.5, -3e4],\n  "nested": { "k": "v" }\n}\n`,
  yaml: `# comment\ntitle: A title\ncount: 3\nflag: true\nempty: null\nlist:\n  - one\n  - two\nnested:\n  key: "quoted"\n  other: 'single'\n`,
  html: `<!DOCTYPE html>\n<!-- comment -->\n<html lang="en">\n  <body class="a b" data-x='1'>\n    <p id=bare>Text &amp; more</p>\n    <script>var x = 1;</script>\n    <style>.c { color: red }</style>\n  </body>\n</html>\n`,
  css: `/* comment */\n@media (min-width: 40rem) {\n  :root { --tok: #fff; }\n  .cls > a:hover::before { content: "x"; color: var(--tok); margin: 0 auto !important; }\n}\n`,
  bash: `#!/usr/bin/env bash\n# comment\nset -euo pipefail\nNAME="world"\nfor f in *.md; do\n  if [[ -f "$f" ]]; then\n    echo "hi \${NAME}" | tr a-z A-Z > /dev/null\n  fi\ndone\n`,
  sql: `-- comment\nSELECT u.id, COUNT(*) AS n\nFROM users AS u\nLEFT JOIN posts p ON p.user_id = u.id\nWHERE u.name LIKE 'a%' AND u.age >= 18\nGROUP BY u.id HAVING COUNT(*) > 2\nORDER BY n DESC;\n`,
  go: `// comment\npackage main\n\nimport "fmt"\n\ntype T struct { N int }\n\nfunc (t *T) Run(xs []string) (int, error) {\n\tfor i, x := range xs {\n\t\tif x == "" { continue }\n\t\tfmt.Printf("%d %q\\n", i, x)\n\t}\n\treturn t.N, nil\n}\n`,
  java: `// comment\npackage app;\n\nimport java.util.List;\n\npublic class Main {\n    private static final int N = 0x10;\n    @Override\n    public String toString() { return "n=" + N; }\n    public static void main(String[] args) throws Exception {\n        for (String a : args) System.out.println(a);\n    }\n}\n`,
  c: `/* comment */\n#include <stdio.h>\n#define MAX 10\n\nstatic int add(int a, int b) {\n    char *s = "text";\n    return a + b > MAX ? 1 : 0;\n}\n`,
  cpp: `// comment\n#include <vector>\nnamespace app {\ntemplate <typename T>\nclass Holder {\npublic:\n    explicit Holder(T v) : v_(std::move(v)) {}\nprivate:\n    T v_;\n};\n}\n`,
  diff: `--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n context\n-removed line\n+added line\n`,
  markdown: `# Heading\n\nA **bold** and *italic* with \`code\` and a [link](http://x).\n\n- item\n`,
  toml: `# comment\ntitle = "x"\n[table]\nn = 3\nflag = true\nlist = [1, 2]\n`,
  xml: `<?xml version="1.0"?>\n<!-- comment -->\n<root attr="v">\n  <child/>\n</root>\n`,
  shell: `echo "alias of bash?"\n`,
  jsx: `const A = () => <div className="x">{value}</div>;\n`,
  none: `plain text, no language\n`,
  'scoria-made-up': `whatever this is\n`,
};

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  const body = ['# Code', ''];
  for (const [lang, code] of Object.entries(SAMPLES)) {
    body.push(`## ${lang}`, '', '```' + lang, code.replace(/\n$/, ''), '```', '');
  }
  fs.writeFileSync(vp('Code.md'), body.join('\n') + '\n');
  // One note per language too: the reading view only builds what is near the
  // viewport, and a single long note would report a fraction of its blocks.
  fs.mkdirSync(vp('Each'), { recursive: true });
  for (const [lang, code] of Object.entries(SAMPLES)) {
    const safe = lang.replace(/[^A-Za-z0-9_-]/g, '_');
    fs.writeFileSync(vp(`Each/${safe}.md`), `# ${lang}\n\n` + '```' + lang + '\n' + code.replace(/\n$/, '') + '\n```\n');
  }
  out.fixtures = { langs: Object.keys(SAMPLES) };
  save();
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const c = await connect();
out.vaultPath = await c.vaultPath();
save();

// ---- the renderer-side library ------------------------------------------
// Cascade winners, as theme-registry.mjs works them out: the declaration a
// property really comes from, with its `var()` intact.
const LIB = `
  const W = window, D = document;
  const spec = (sel) => {
    let a = 0, b = 0, cc = 0;
    const s = sel.replace(/::?[a-z-]+\\([^)]*\\)/g, (m) => (m.startsWith('::') ? '' : m.startsWith(':where') ? '' : m.slice(m.indexOf('(') + 1, -1)));
    const t = s.replace(/\\[[^\\]]*\\]/g, () => { b++; return ' '; }).replace(/::[a-z-]+/g, () => { cc++; return ' '; }).replace(/:[a-z-]+/g, () => { b++; return ' '; }).replace(/#[\\w-]+/g, () => { a++; return ' '; }).replace(/\\.[\\w-]+/g, () => { b++; return ' '; });
    cc += (t.match(/(^|[\\s>+~(,])[a-zA-Z][\\w-]*/g) || []).length;
    return [a, b, cc];
  };
  const cmp = (x, y) => { for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i]; return 0; };
  let RULES = null;
  const allRules = () => {
    if (RULES) return RULES;
    RULES = []; let order = 0;
    const walk = (list, sheet) => { for (const r of Array.from(list)) {
      if ('selectorText' in r && r.style) RULES.push({ r, sel: r.selectorText, order: order++, sheet });
      else if ('conditionText' in r && r.cssRules && r.constructor.name === 'CSSMediaRule') { if (W.matchMedia(r.conditionText).matches) walk(r.cssRules, sheet); }
      else if (r.cssRules) walk(r.cssRules, sheet);
    } };
    for (const s of Array.from(D.styleSheets)) { let rules; try { rules = s.cssRules; } catch { continue; } walk(rules, s.href || (s.ownerNode && s.ownerNode.id) || 'inline'); }
    return RULES;
  };
  const matches = (el, sel) => { try { return el.matches(sel); } catch { return false; } };
  const PROPS = ['color', 'font-weight', 'font-style', 'background-color', 'text-decoration-line', 'opacity'];
  const winner = (el, prop) => {
    let best = null;
    for (const { r, sel, order } of allRules()) {
      const v = r.style.getPropertyValue(prop); if (!v) continue;
      const parts = sel.split(/,(?![^(]*\\))/).map((p) => p.trim());
      let hit = null;
      for (const p of parts) if (matches(el, p)) { const sp = spec(p); if (!hit || cmp(sp, hit.sp) > 0) hit = { p, sp }; }
      if (!hit) continue;
      const imp = r.style.getPropertyPriority(prop) === 'important' ? 1 : 0;
      if (!best || imp > best.imp || (imp === best.imp && (cmp(hit.sp, best.sp) > 0 || (cmp(hit.sp, best.sp) === 0 && order > best.order)))) {
        best = { selector: hit.p, value: v.trim(), imp, sp: hit.sp, order };
      }
    }
    return best ? { from: best.selector, value: best.value, important: !!best.imp } : null;
  };
  // A token span, as the engine left it: what it is, what it says, what it looks like.
  const tok = (el) => {
    const cs = W.getComputedStyle(el);
    const o = {
      cls: typeof el.className === 'string' ? el.className.trim() : '',
      text: (el.textContent || '').slice(0, 60),
      color: cs.getPropertyValue('color').trim(),
      weight: cs.getPropertyValue('font-weight').trim(),
      style: cs.getPropertyValue('font-style').trim(),
    };
    const w = winner(el, 'color');
    if (w) o.colorFrom = w;
    const bw = winner(el, 'font-weight'); if (bw) o.weightFrom = bw;
    const bs = winner(el, 'font-style'); if (bs) o.styleFrom = bs;
    // Nested tokens: Prism nests, e.g. .token.tag > .token.attr-name.
    const kids = Array.from(el.children).filter((k) => k.classList && k.classList.contains('token'));
    if (kids.length) o.kids = kids.map(tok);
    return o;
  };
`;

// ---- s01 the language table ---------------------------------------------
await step('s01-languages', async () => {
  return c.evaluate(`
    const P = window.Prism || (window.require && (() => { try { return require('prismjs'); } catch { return null; } })()) || null;
    if (!P) {
      // Not a global in 1.14.5. Record what else might carry it, so the
      // next pass knows where to look; the list of languages is settled by
      // rendering instead (step s08).
      return { noPrism: true,
        windowKeys: Object.keys(window).filter((k) => /prism|highlight|token/i.test(k)),
        hasRequire: !!window.require,
        moduleKeys: (() => { try { return Object.keys(require.cache || {}).filter((k) => /prism/i.test(k)).slice(0, 20); } catch { return null; } })(),
      };
    }
    const Prism = P;
    const L = Prism.languages;
    const keys = Object.keys(L).filter((k) => L[k] && typeof L[k] === 'object');
    // Aliases share the grammar object; fold them onto the first key that has it.
    const seen = new Map(); const canonical = {}; const aliases = {};
    for (const k of keys) {
      const g = L[k];
      if (seen.has(g)) { const first = seen.get(g); (aliases[first] = aliases[first] || []).push(k); }
      else { seen.set(g, k); canonical[k] = Object.keys(g); }
    }
    return {
      count: keys.length,
      keys: keys.sort(),
      canonical: Object.fromEntries(Object.entries(canonical).map(([k, v]) => [k, v])),
      aliases,
      plain: Prism.plainTextGrammar ? true : false,
      version: Prism.version || null,
      manual: Prism.manual,
      hooks: Object.keys(Prism.hooks && Prism.hooks.all || {}),
    };`);
});

// ---- s02 the reading view, per language ---------------------------------
// One leaf, reused. Detaching every markdown leaf first leaves no tab group
// and `getLeaf(true)` then throws "No tab group found"; and a second leaf is
// poison anyway, since every `.markdown-reading-view` selector would match
// the stale one.
const openNote = async (rel) => {
  await c.evaluate(`
    const f = app.vault.getAbstractFileByPath(${JSON.stringify(rel)});
    if (!f) throw new Error('no ' + ${JSON.stringify(rel)});
    const leaves = app.workspace.getLeavesOfType('markdown');
    for (const extra of leaves.slice(1)) extra.detach();
    const leaf = leaves[0] || app.workspace.getLeaf(true);
    await leaf.openFile(f, { state: { mode: 'preview' } });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    return true;`);
  await sleep(900);
};

const readBlocks = () => c.evaluate(LIB + `
  const out = [];
  for (const pre of Array.from(D.querySelectorAll('.markdown-reading-view pre'))) {
    const code = pre.querySelector('code');
    if (!code) continue;
    const spans = Array.from(code.querySelectorAll('span.token'));
    // Only the outermost: nested ones come back under kids.
    const top = spans.filter((s) => !s.parentElement.classList.contains('token'));
    out.push({
      preClass: pre.className, codeClass: code.className,
      text: (code.textContent || '').slice(0, 400),
      codeColor: W.getComputedStyle(code).getPropertyValue('color').trim(),
      codeColorFrom: winner(code, 'color'),
      preBg: W.getComputedStyle(pre).getPropertyValue('background-color').trim(),
      preBgFrom: winner(pre, 'background-color'),
      tokens: top.map(tok),
      tokenCount: spans.length,
      html: code.innerHTML.length > 4000 ? code.innerHTML.slice(0, 4000) + '...' : code.innerHTML,
    });
  }
  return out;`);

const LANGS = Object.keys(SAMPLES);
await step('s02-reading-dark', async () => {
  await c.evaluate(`app.changeTheme('obsidian'); return true;`);
  await sleep(600);
  const byLang = {};
  for (const lang of LANGS) {
    const safe = lang.replace(/[^A-Za-z0-9_-]/g, '_');
    await openNote(`Each/${safe}.md`);
    byLang[lang] = await readBlocks();
  }
  return { theme: 'obsidian', byLang };
});

await step('s03-reading-light', async () => {
  await c.evaluate(`app.changeTheme('moonstone'); return true;`);
  await sleep(800);
  const byLang = {};
  for (const lang of LANGS) {
    const safe = lang.replace(/[^A-Za-z0-9_-]/g, '_');
    await openNote(`Each/${safe}.md`);
    byLang[lang] = await readBlocks();
  }
  await c.evaluate(`app.changeTheme('obsidian'); return true;`);
  await sleep(600);
  return { theme: 'moonstone', byLang };
});

// ---- s04 the editor: Live Preview and source ----------------------------
// CodeMirror is a different engine with different class names; Scoria paints
// both from one model, so the two have to be compared directly.
const openEditor = async (rel, source) => {
  await c.evaluate(`
    const f = app.vault.getAbstractFileByPath(${JSON.stringify(rel)});
    const leaves = app.workspace.getLeavesOfType('markdown');
    for (const extra of leaves.slice(1)) extra.detach();
    const leaf = leaves[0] || app.workspace.getLeaf(true);
    await leaf.openFile(f, { state: { mode: 'source', source: ${source ? 'true' : 'false'} } });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    return true;`);
  await sleep(1000);
};

const readEditorSpans = () => c.evaluate(LIB + `
  const cm = D.querySelector('.markdown-source-view .cm-content');
  if (!cm) return { noEditor: true };
  const lines = Array.from(cm.querySelectorAll('.cm-line'));
  const rows = lines.map((ln) => ({
    cls: ln.className,
    text: (ln.textContent || '').slice(0, 120),
    spans: Array.from(ln.querySelectorAll('span')).filter((s) => !s.parentElement.closest('span')).map((s) => ({
      cls: typeof s.className === 'string' ? s.className.trim() : '',
      text: (s.textContent || '').slice(0, 60),
      color: W.getComputedStyle(s).getPropertyValue('color').trim(),
      weight: W.getComputedStyle(s).getPropertyValue('font-weight').trim(),
      style: W.getComputedStyle(s).getPropertyValue('font-style').trim(),
      colorFrom: winner(s, 'color'),
      kids: Array.from(s.querySelectorAll('span')).map((k) => ({ cls: typeof k.className === 'string' ? k.className.trim() : '', text: (k.textContent || '').slice(0, 40) })),
    })),
  }));
  // The classes CodeMirror put on the document, with the colour each resolves to.
  const classes = {};
  for (const s of Array.from(cm.querySelectorAll('span'))) {
    const cls = typeof s.className === 'string' ? s.className.trim() : '';
    if (!cls || classes[cls]) continue;
    classes[cls] = { color: W.getComputedStyle(s).getPropertyValue('color').trim(), from: winner(s, 'color'), sample: (s.textContent || '').slice(0, 30) };
  }
  return { rows, classes, lineCount: lines.length };`);

await step('s04-editor-live', async () => {
  const picked = ['js', 'python', 'rust', 'json', 'yaml', 'css', 'bash'];
  const byLang = {};
  for (const lang of picked) {
    await openEditor(`Each/${lang}.md`, false);
    byLang[lang] = await readEditorSpans();
  }
  return { mode: 'live preview', byLang };
});

await step('s05-editor-source', async () => {
  const picked = ['js', 'python', 'rust', 'json', 'yaml', 'css', 'bash'];
  const byLang = {};
  for (const lang of picked) {
    await openEditor(`Each/${lang}.md`, true);
    byLang[lang] = await readEditorSpans();
  }
  // Which CodeMirror modes Obsidian has loaded, and how it picks one.
  const modes = await c.evaluate(`
    const cm = window.CodeMirror;
    const legacy = cm && cm.modes ? Object.keys(cm.modes) : null;
    const mimes = cm && cm.mimeModes ? Object.keys(cm.mimeModes) : null;
    return { hasGlobalCodeMirror: !!cm, legacy, mimes, findMode: !!(cm && cm.findModeByName) };`);
  return { mode: 'source', byLang, modes };
});

// ---- s06 the collapsed class -> variable map ----------------------------
await step('s06-map', async () => {
  const collapse = (record) => {
    const map = {};
    for (const [lang, blocks] of Object.entries(record.byLang || {})) {
      const walk = (t) => {
        const key = t.cls;
        if (key) {
          const e = (map[key] = map[key] || { langs: [], color: t.color, from: t.colorFrom || null, weight: t.weight, style: t.style, samples: [] });
          if (!e.langs.includes(lang)) e.langs.push(lang);
          if (e.samples.length < 3) e.samples.push(t.text);
        }
        for (const k of t.kids || []) walk(k);
      };
      for (const b of blocks) for (const t of b.tokens || []) walk(t);
    }
    return map;
  };
  const dark = out['s02-reading-dark'];
  const light = out['s03-reading-light'];
  return {
    dark: dark && !dark.ERR ? collapse(dark) : null,
    light: light && !light.ERR ? collapse(light) : null,
  };
});

// ---- s07 a shot of each, for the eye ------------------------------------
await step('s07-shots', async () => {
  const made = [];
  fs.mkdirSync(SHOTS, { recursive: true });
  for (const theme of ['obsidian', 'moonstone']) {
    await c.evaluate(`app.changeTheme(${JSON.stringify(theme)}); return true;`);
    await sleep(700);
    for (const lang of ['js', 'python', 'rust', 'css']) {
      await openNote(`Each/${lang}.md`);
      const r = await c.send('Page.captureScreenshot', { format: 'png' });
      const name = `${lang}-${theme}.png`;
      fs.writeFileSync(path.join(SHOTS, name), Buffer.from(r.data, 'base64'));
      made.push(name);
    }
  }
  await c.evaluate(`app.changeTheme('obsidian'); return true;`);
  return { made };
});

// ---- s08 which language names Obsidian actually highlights -------------
// Prism is not reachable as an object in 1.14.5, so the list is settled by
// rendering: a fence per candidate name, and whether any `span.token` came
// out of it. The reading view only builds what is near the viewport, so the
// note is scrolled through and the blocks merged.
const CANDIDATES = [
  'abap','actionscript','ada','apacheconf','apl','applescript','arduino','asciidoc','asm6502','aspnet','autohotkey','autoit',
  'bash','basic','batch','bbcode','bison','brainfuck','bro','c','clike','clojure','cmake','coffeescript','cpp','crystal','csharp','csp','css','csv',
  'd','dart','diff','django','dns-zone-file','docker','dockerfile','ebnf','eiffel','elixir','elm','erb','erlang','excel-formula',
  'fortran','fsharp','gherkin','git','glsl','gml','go','graphql','groovy','haml','handlebars','haskell','haxe','hcl','hlsl','http','hpkp','hsts',
  'ichigojam','icon','inform7','ini','io','j','java','javadoc','javascript','javastacktrace','jolie','jq','js','jsdoc','json','json5','jsonp','jsstacktrace','jsx','julia',
  'keyman','kotlin','latex','latte','less','lilypond','liquid','lisp','livescript','llvm','lolcode','lua',
  'makefile','markdown','markup','markup-templating','matlab','mel','mizar','mongodb','monkey','moonscript','n1ql','n4js','nand2tetris-hdl','nasm','neon','nginx','nim','nix','nsis',
  'objectivec','ocaml','opencl','oz','parigp','parser','pascal','pascaligo','pcaxis','peoplecode','perl','php','phpdoc','plsql','powerquery','powershell','processing','prolog','properties','protobuf','pug','puppet','pure','purebasic','purescript','python',
  'q','qml','qore','r','racket','reason','regex','renpy','rest','rip','roboconf','robotframework','ruby','rust',
  'sas','sass','scala','scheme','scss','shell','shell-session','smalltalk','smarty','solidity','solution-file','soy','sparql','splunk-spl','sqf','sql','stan','stylus','swift',
  'systemd','t4-cs','t4-templating','tap','tcl','textile','toml','tsx','tt2','twig','typescript','ts','unrealscript','vala','vbnet','velocity','verilog','vhdl','vim','visual-basic','wasm','wiki','xeora','xml','xojo','xquery','yaml','yang','zig',
];
await step('s08-highlighted-languages', async () => {
  const body = ['# Languages', ''];
  for (const lang of CANDIDATES) body.push('```' + lang, 'x = 1 // "s" /* c */ # c -- c', '```', '');
  fs.writeFileSync(vp('Languages.md'), body.join('\n') + '\n');
  await sleep(1200);
  await openNote('Languages.md');
  const seen = {};
  // Scroll the whole note, merging what each viewport builds.
  for (let i = 0; i < 60; i++) {
    const got = await c.evaluate(`
      const out = {};
      for (const pre of Array.from(document.querySelectorAll('.markdown-reading-view pre'))) {
        const code = pre.querySelector('code'); if (!code) continue;
        const m = /language-([A-Za-z0-9_-]+)/.exec(code.className || '');
        const spans = Array.from(code.querySelectorAll('span.token'));
        out[m ? m[1] : '(none)'] = {
          preClass: pre.className, codeClass: code.className,
          tokens: spans.length,
          classes: Array.from(new Set(spans.map((s) => (typeof s.className === 'string' ? s.className.trim() : '')))).slice(0, 12),
        };
      }
      return out;`);
    Object.assign(seen, got);
    const atEnd = await c.evaluate(`
      const sc = document.querySelector('.markdown-reading-view .markdown-preview-view');
      if (!sc) return true;
      const before = sc.scrollTop;
      sc.scrollTop = before + sc.clientHeight * 0.8;
      return sc.scrollTop <= before + 1;`);
    await sleep(250);
    if (atEnd) break;
  }
  const highlighted = Object.entries(seen).filter(([, v]) => v.tokens > 0).map(([k]) => k).sort();
  const plain = Object.entries(seen).filter(([, v]) => v.tokens === 0).map(([k]) => k).sort();
  return { tried: CANDIDATES.length, seen: Object.keys(seen).length, highlighted, plain, detail: seen };
});

// ---- s09 Obsidian's own rules for .token and .cm-* ---------------------
// The sample-driven map (s06) only shows the classes these samples happened
// to produce. The rule list is the authority: every selector Obsidian
// declares a colour, weight or style for, with the declaration, so a token
// no sample reached is still known.
await step('s09-rules', async () => {
  // Obsidian loads Prism's stylesheet with the component, so a block has to
  // have been rendered before the `.token` rules are in the document at all
  // (a run of this step on its own found 0 of them and 157 CodeMirror ones).
  for (const lang of ['js', 'python', 'css', 'markdown', 'diff', 'yaml', 'html', 'bash', 'rust', 'sql']) {
    await openNote(`Each/${lang}.md`);
  }
  return c.evaluate(`
    const out = { prism: [], cm: [], vars: {} };
    const walk = (list, sheet) => {
      for (const r of Array.from(list)) {
        if (r.cssRules && !('selectorText' in r)) { walk(r.cssRules, sheet); continue; }
        if (!('selectorText' in r) || !r.style) continue;
        const sel = r.selectorText || '';
        const isPrism = /\.token\b/.test(sel);
        const isCm = /\.cm-[a-z]/i.test(sel);
        if (!isPrism && !isCm) continue;
        const decl = {};
        for (let i = 0; i < r.style.length; i++) {
          const prop = r.style[i];
          if (!/^(color|font-weight|font-style|background|background-color|text-decoration|opacity)$/.test(prop)) continue;
          decl[prop] = r.style.getPropertyValue(prop);
        }
        if (!Object.keys(decl).length) continue;
        (isPrism ? out.prism : out.cm).push({ selector: sel, decl, sheet: String(sheet).slice(-40) });
      }
    };
    for (const s of Array.from(document.styleSheets)) {
      let rules; try { rules = s.cssRules; } catch { continue; }
      walk(rules, s.href || (s.ownerNode && s.ownerNode.id) || 'inline');
    }
    // And what each --code-* computes to, in this theme.
    const cs = getComputedStyle(document.body);
    for (const name of ['--code-normal','--code-comment','--code-keyword','--code-string','--code-function','--code-value','--code-operator','--code-punctuation','--code-property','--code-tag','--code-important','--code-background','--code-size']) {
      out.vars[name] = cs.getPropertyValue(name).trim();
    }
    // Diagnostics: a run of this step found 157 cm- rules and no token
    // ones, so count what is actually there.
    out.diag = (() => {
      let all = 0, withToken = 0; const sample = [];
      for (const sh of Array.from(document.styleSheets)) {
        let rules; try { rules = sh.cssRules; } catch (e) { sample.push('THREW:' + String(sh.href).slice(-30)); continue; }
        const walk = (list) => { for (const r of Array.from(list)) {
          if (r.cssRules) { walk(r.cssRules); continue; }
          if (!('selectorText' in r)) continue;
          all++;
          if (/token/.test(r.selectorText || '')) { withToken++; if (sample.length < 14) sample.push(r.selectorText + ' {' + r.style.cssText.slice(0, 60) + '}'); }
        } };
        walk(rules);
      }
      return { all, withToken, sample, sheets: document.styleSheets.length, prismEls: document.querySelectorAll('span.token').length };
    })();
    return out;`);
});

// ---- s10 what Obsidian paints each class, asked directly ---------------
// The `.token` rules are not reachable through document.styleSheets (s09
// found 184 CodeMirror rules and none of Prism's, with 39 token spans on
// screen), so the question is put to the renderer instead: put a span of
// each candidate class inside a real rendered block and read back what it
// computes to. That answers it for every class, including the ones no
// sample happened to produce.
const CLASSES = [
  'atrule','attr-name','attr-value','bold','boolean','builtin','cdata','char','class-name','comment',
  'constant','deleted','doctype','entity','function','important','inserted','italic','keyword','namespace',
  'number','operator','parameter','prefix','prolog','property','property-access','punctuation','regex','selector',
  'string','symbol','tag','url','variable','unchanged','coord','interpolation','template-string','maybe-class-name',
  'key','macro','arrow','annotation','decorator','directive','control-flow','module','expression','rule',
  'combinator','pseudo-class','pseudo-element','unit','hexcode','color','line','name','script','style',
  'doctype-tag','attr-equals','punctuation-chain','triple-quoted-string','shebang','title','list','table','null','nil',
];
await step('s10-class-colours', async () => {
  await openNote('Each/js.md');
  const byTheme = {};
  for (const theme of ['obsidian', 'moonstone']) {
    await c.evaluate('app.changeTheme(' + JSON.stringify(theme) + '); return true;');
    await sleep(700);
    byTheme[theme] = await c.evaluate(`
      const code = document.querySelector('.markdown-reading-view pre > code');
      if (!code) return { noBlock: true };
      const cs0 = getComputedStyle(document.body);
      const vars = {};
      for (const n of ['--code-normal','--code-comment','--code-keyword','--code-string','--code-function','--code-value','--code-operator','--code-punctuation','--code-property','--code-tag','--code-important']) vars[n] = cs0.getPropertyValue(n).trim();
      // A colour string to the variable that holds it, so the answer comes
      // back as a token name rather than a hex.
      const toVar = (() => {
        const probe = document.createElement('span');
        code.appendChild(probe);
        const named = {};
        for (const [n, v] of Object.entries(vars)) { probe.style.color = v; named[getComputedStyle(probe).color] = n; }
        probe.remove();
        return (col) => named[col] || null;
      })();
      const names = ${JSON.stringify(CLASSES)};
      const out = {};
      const base = (() => { const s = document.createElement('span'); s.className = 'token'; code.appendChild(s); const cs = getComputedStyle(s); const r = { color: cs.color, weight: cs.fontWeight, style: cs.fontStyle }; s.remove(); return r; })();
      for (const name of names) {
        const span = document.createElement('span');
        span.className = 'token ' + name;
        span.textContent = 'x';
        code.appendChild(span);
        const cs = getComputedStyle(span);
        const color = cs.color, weight = cs.fontWeight, style = cs.fontStyle;
        span.remove();
        out[name] = {
          color, weight, style,
          token: toVar(color),
          styled: color !== base.color || weight !== base.weight || style !== base.style,
        };
      }
      return { base, vars, out };
    `);
  }
  await c.evaluate("app.changeTheme('obsidian'); return true;");
  return byTheme;
});

save();
c.close();
console.error('[code-highlight] results: ' + RESULTS);
process.exit(0);
