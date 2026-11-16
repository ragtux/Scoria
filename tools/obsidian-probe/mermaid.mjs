// Mermaid diagrams: what Obsidian draws for a ```mermaid fence, in the reading
// view, Live Preview, an embed and a hover popover, under both base themes.
//
//   s00  context: version, the renderer it bundles (`pele`, `loadMermaid`),
//        the --mermaid-* and --pele-* variables in each theme
//   s01  reading view, every case, in the light theme and the dark one: the
//        block's markup, the svg's size and viewBox, every drawn element with
//        its box and computed paint, every text with its box and font; a
//        screenshot per case (SHOTS)
//   s02  Live Preview: the widget around the diagram, the edit button, the
//        caret on the fence (the source comes back)
//   s03  an embed of a note holding a diagram, and a hover popover over a link
//        to it
//   s04  the hover controls on a diagram in the reading view, and what a
//        click on them does (Pele's zoom)
//   s05  a narrow pane: how the width the pane gives changes the layout
//
// `SHOTS` names a folder for PNGs. `ONLY_STEPS=s01` re-runs a step.
// Phases: fixtures | run   (node mermaid.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const SHOTS = process.env.SHOTS || null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }

// The cases. Each is one note holding one fence; the tests of scoria-mermaid
// read the sources back from the results file.
const CASES = {
  'flow-td': 'flowchart TD\n    A[Start] --> B{Is it?}\n    B -->|Yes| C[OK]\n    C --> D[Rethink]\n    D --> B\n    B ---->|No| E[End]',
  'flow-lr-shapes': 'flowchart LR\n    a[Rect] --> b(Round)\n    b --> c([Stadium])\n    c --> d[[Subroutine]]\n    d --> e[(Database)]\n    e --> f((Circle))\n    f --> g>Flag]\n    g --> h{Rhombus}\n    h --> i{{Hexagon}}\n    i --> j[/Parallel/]\n    j --> k[\\Alt\\]\n    k --> l[/Trapezoid\\]\n    l --> m[\\Inverse/]\n    m --> n(((Double)))',
  'flow-edges': 'graph LR\n    A --- B\n    B --> C\n    C -.-> D\n    D ==> E\n    E --o F\n    F --x G\n    G <--> H\n    H -- text --> I\n    I -. dotted .-> J\n    J == thick ==> K\n    K ~~~ L',
  'flow-bt': 'graph BT\n    A --> B\n    A --> C\n    B --> D\n    C --> D',
  'flow-rl': 'graph RL\n    A[One] --> B[Two] --> C[Three]',
  'flow-subgraph': 'flowchart TB\n    c1-->a2\n    subgraph one\n    a1-->a2\n    end\n    subgraph two\n    b1-->b2\n    end\n    subgraph three\n    c1-->c2\n    end\n    one --> two\n    three --> two\n    two --> c2',
  'flow-style': 'flowchart LR\n    A:::hot --> B\n    B --> C\n    classDef hot fill:#f96,stroke:#333,stroke-width:4px\n    style C fill:#bbf,stroke:#f66,stroke-width:2px,color:#fff,stroke-dasharray: 5 5\n    linkStyle 0 stroke:#ff3,stroke-width:4px',
  'flow-long-label': 'flowchart TD\n    A["A rather long label that has to wrap across more than one line of text"] --> B["Short"]\n    B --> C["Unicode: café, naïve, 日本語"]\n    C --> D["**bold** and *italic*"]',
  'sequence': 'sequenceDiagram\n    participant Alice\n    participant Bob\n    Alice->>John: Hello John, how are you?\n    loop Healthcheck\n        John->>John: Fight against hypochondria\n    end\n    Note right of John: Rational thoughts <br/>prevail!\n    John-->>Alice: Great!\n    John->>Bob: How about you?\n    Bob-->>John: Jolly good!',
  'sequence-full': 'sequenceDiagram\n    autonumber\n    actor U as User\n    participant S as Server\n    U->>+S: Request\n    S-->>-U: Response\n    U-)S: Async\n    U-xS: Lost\n    alt is ok\n        S->>U: fine\n    else is not\n        S->>U: sorry\n    end\n    opt maybe\n        U->>S: again\n    end\n    par one\n        U->>S: a\n    and two\n        U->>S: b\n    end\n    Note over U,S: Across both',
  'class': 'classDiagram\n    Animal <|-- Duck\n    Animal <|-- Fish\n    Animal : +int age\n    Animal : +String gender\n    Animal: +isMammal()\n    class Duck{\n        +String beakColor\n        +swim()\n        +quack()\n    }\n    class Fish{\n        -int sizeInFeet\n        -canEat()\n    }',
  'state': 'stateDiagram-v2\n    [*] --> Still\n    Still --> [*]\n    Still --> Moving\n    Moving --> Still\n    Moving --> Crash\n    Crash --> [*]',
  'gantt': 'gantt\n    title A Gantt Diagram\n    dateFormat YYYY-MM-DD\n    section Section\n        A task          :a1, 2024-01-01, 30d\n        Another task    :after a1, 20d\n    section Another\n        Task in Another :2024-01-12, 12d\n        another task    :24d',
  'pie': 'pie title Pets adopted by volunteers\n    "Dogs" : 386\n    "Cats" : 85\n    "Rats" : 15',
  'er': 'erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER ||--|{ LINE-ITEM : contains\n    CUSTOMER }|..|{ DELIVERY-ADDRESS : uses',
  'mindmap': 'mindmap\n  root((mindmap))\n    Origins\n      Long history\n      Popularisation\n    Research\n      On effectiveness\n    Tools\n      Pen and paper\n      Mermaid',
  'journey': 'journey\n    title My working day\n    section Go to work\n      Make tea: 5: Me\n      Go upstairs: 3: Me\n    section Go home\n      Go downstairs: 5: Me',
  'gitgraph': 'gitGraph\n    commit\n    branch develop\n    checkout develop\n    commit\n    checkout main\n    merge develop',
  'error-syntax': 'flowchart TD\n    A --> B -->\n    ((',
  'error-unknown': 'notADiagram\n    A --> B',
  'empty': '',
  'flow-init': '%%{init: {"theme": "forest"}}%%\nflowchart LR\n    A --> B',
  'flow-frontmatter': '---\ntitle: Titled flow\n---\nflowchart LR\n    A --> B',
  'flow-click': 'flowchart LR\n    A --> B\n    click A "https://example.com" "Tooltip"',
};

const vp = (rel) => path.join(VAULT, rel);
const note = (name) => `M/${name}`;

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.mkdirSync(vp('M'), { recursive: true });
  for (const [name, src] of Object.entries(CASES)) {
    fs.writeFileSync(vp(`M/${name}.md`), `# ${name}\n\n` + '```mermaid\n' + src + (src ? '\n' : '') + '```\n\nAfter.\n');
  }
  fs.writeFileSync(vp('Embeds.md'), '# Embeds\n\n![[M/flow-td]]\n\nA link to [[M/sequence]].\n');
  fs.writeFileSync(vp('.obsidian/app.json'), JSON.stringify({ livePreview: true }));
  // PELE_FONT=Inter sets the diagrams' face by a snippet, so a layout can be
  // compared with one measured in the face Scoria draws in.
  const appearance = { theme: 'moonstone' };
  if (process.env.PELE_FONT) {
    fs.mkdirSync(vp('.obsidian/snippets'), { recursive: true });
    fs.writeFileSync(vp('.obsidian/snippets/pele-font.css'), `body { --pele-font: ${process.env.PELE_FONT}, sans-serif; }\n`);
    appearance.enabledCssSnippets = ['pele-font'];
  }
  fs.writeFileSync(vp('.obsidian/appearance.json'), JSON.stringify(appearance));
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, 'page-preview': true }));
  process.exit(0);
}
if (phase === 'after-restart') process.exit(0);

const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
all.cases = CASES;
all.steps = all.steps || {};
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
const ONLY = process.env.ONLY_STEPS ? process.env.ONLY_STEPS.split(',') : null;
async function step(name, fn) {
  if (ONLY && !ONLY.some((p) => name.startsWith(p))) return;
  const t = Date.now();
  let timer;
  const timeout = new Promise((_, rej) => { timer = setTimeout(() => rej(new Error('step timeout')), 240000); });
  try { all.steps[name] = { ok: true, ...(await Promise.race([fn(), timeout])) }; }
  catch (e) { all.steps[name] = { ok: false, error: String(e && e.stack ? e.stack : e) }; }
  finally { clearTimeout(timer); }
  all.steps[name].ms = Date.now() - t;
  save();
  console.error(`[mermaid] ${name}: ${all.steps[name].ok ? 'ok' : 'FAILED ' + all.steps[name].error.split('\n')[0]}`);
}

const c = await connect();
const ev = (s) => c.evaluate(s);
const vault = await c.vaultPath();
if (vault.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault ' + vault); process.exit(3); }
await c.send('Emulation.setDeviceMetricsOverride', { width: 1400, height: 1000, deviceScaleFactor: 1, mobile: false });

await ev(`
window.__m = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  r: (e) => { if (!e) return null; const b = e.getBoundingClientRect(); return { x: +b.x.toFixed(2), y: +b.y.toFixed(2), w: +b.width.toFixed(2), h: +b.height.toFixed(2) }; },
  vars(el) {
    const cs = getComputedStyle(el); const o = {};
    for (const s of Array.from(document.styleSheets)) { let rules; try { rules = s.cssRules; } catch { continue; }
      const walk = (list) => { for (const r of Array.from(list)) { if (r.style) for (const p of Array.from(r.style)) if (/^--(mermaid|pele)/.test(p)) o[p] = cs.getPropertyValue(p).trim(); if (r.cssRules) walk(r.cssRules); } };
      walk(rules); }
    return o;
  },
  // Where each --mermaid-*/--pele-* variable is declared, and as what.
  decls() {
    const o = [];
    for (const s of Array.from(document.styleSheets)) { let rules; try { rules = s.cssRules; } catch { continue; }
      const walk = (list) => { for (const r of Array.from(list)) { if (r.style) for (const p of Array.from(r.style)) if (/^--(mermaid|pele)/.test(p)) o.push({ sel: r.selectorText, p, v: r.style.getPropertyValue(p).trim() }); if (r.cssRules) walk(r.cssRules); } };
      walk(rules); }
    return o;
  },
  async open(file, mode) {
    const leaf = app.workspace.getLeaf(false);
    const f = app.vault.getAbstractFileByPath(file);
    await leaf.openFile(f, { state: { mode: mode === 'reading' ? 'preview' : 'source', source: mode === 'source' } });
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(900);
    return leaf;
  },
  // Every drawn thing of an svg, in document order, with its box in the
  // svg's own units and its computed paint.
  draw(svg) {
    const out = [];
    const root = svg.getBoundingClientRect();
    const vb = svg.viewBox && svg.viewBox.baseVal ? svg.viewBox.baseVal : null;
    const k = vb && vb.width ? root.width / vb.width : 1;
    for (const e of svg.querySelectorAll('rect,path,polygon,polyline,line,circle,ellipse,text,foreignObject')) {
      const b = e.getBoundingClientRect(); const cs = getComputedStyle(e);
      const d = { tag: e.tagName, cls: this.cls(e), x: +((b.x - root.x) / k).toFixed(2), y: +((b.y - root.y) / k).toFixed(2), w: +(b.width / k).toFixed(2), h: +(b.height / k).toFixed(2),
        fill: cs.fill, stroke: cs.stroke, sw: cs.strokeWidth, dash: cs.strokeDasharray, op: cs.opacity };
      if (e.tagName === 'text' || e.tagName === 'foreignObject') { d.text = e.textContent; d.font = cs.fontFamily; d.size = cs.fontSize; d.weight = cs.fontWeight; d.style = cs.fontStyle; d.anchor = cs.textAnchor; d.color = cs.color; }
      if (e.tagName === 'path') d.d = (e.getAttribute('d') || '').slice(0, 600);
      if (e.tagName === 'rect') { d.rx = e.getAttribute('rx'); }
      const parent = e.closest('g[class]'); d.g = parent ? this.cls(parent) : null;
      out.push(d);
    }
    return out;
  },
  block(root) {
    const svg = root.querySelector('svg');
    const holder = svg ? svg.parentElement : null;
    const err = root.querySelector('.mermaid-error, .pele-error, [class*=error]');
    const r = {
      sectionCls: this.cls(root), sectionRect: this.r(root),
      holderCls: holder ? this.cls(holder) : null, holderTag: holder ? holder.tagName : null, holderRect: this.r(holder),
      holderStyle: holder ? (() => { const cs = getComputedStyle(holder); return { padding: cs.padding, margin: cs.margin, bg: cs.backgroundColor, border: cs.border, radius: cs.borderRadius, textAlign: cs.textAlign, display: cs.display, overflow: cs.overflow }; })() : null,
      html: root.outerHTML.slice(0, 1500), text: (root.innerText || '').slice(0, 800),
    };
    if (svg) {
      const cs = getComputedStyle(svg);
      r.svg = { rect: this.r(svg), viewBox: svg.getAttribute('viewBox'), width: svg.getAttribute('width'), height: svg.getAttribute('height'), style: svg.getAttribute('style'), cls: this.cls(svg),
        font: cs.fontFamily, size: cs.fontSize, color: cs.color, maxWidth: cs.maxWidth, attrs: Array.from(svg.attributes).map((a) => a.name + '=' + a.value.slice(0, 200)) };
      r.svgSource = svg.outerHTML.length < 400000 ? svg.outerHTML : svg.outerHTML.slice(0, 400000);
      r.draw = this.draw(svg);
      r.vars = this.vars(svg);
    }
    if (err) { const cs = getComputedStyle(err); r.error = { cls: this.cls(err), html: err.outerHTML.slice(0, 3000), text: err.innerText, color: cs.color, bg: cs.backgroundColor, font: cs.fontFamily, size: cs.fontSize, padding: cs.padding, border: cs.border, radius: cs.borderRadius, rect: this.r(err) }; }
    return r;
  },
  previewBlock() {
    const view = app.workspace.activeLeaf.view;
    const el = view.containerEl.querySelector('.markdown-reading-view .markdown-preview-sizer');
    const sections = Array.from(el.children);
    const h = sections.findIndex((s) => s.querySelector('h1'));
    const sec = sections.find((s, i) => i > h && !s.classList.contains('mod-header') && !s.querySelector('h1') && (s.querySelector('svg,pre,[class*=mermaid],[class*=pele]')));
    return { sizer: this.r(el), sectionsCls: sections.map((s) => this.cls(s)), block: sec ? this.block(sec) : null };
  },
};
return true;`);

async function shot(name, clip) {
  if (!SHOTS) return null;
  fs.mkdirSync(SHOTS, { recursive: true });
  const p = { format: 'png' };
  if (clip) p.clip = { x: clip.x, y: clip.y, width: Math.max(1, clip.w), height: Math.max(1, clip.h), scale: 2 };
  const r = await c.send('Page.captureScreenshot', p);
  const f = path.join(SHOTS, name + '.png');
  fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
  return f;
}
const setTheme = async (t) => { await ev(`app.changeTheme('${t}'); return true;`); await sleep(700); };

await step('s00', async () => {
  const o = await ev(`return {
    title: document.title, apiVersion: (window.app && app.appVersion) || null, ua: navigator.userAgent,
    hasPele: typeof window.pele, peleKeys: window.pele ? Object.keys(window.pele) : null,
    hasLoadMermaid: typeof window.loadMermaid, mermaidGlobal: typeof window.mermaid,
    scripts: Array.from(document.scripts).map((s) => s.src).filter(Boolean),
  };`);
  try { o.loadMermaid = await ev(`if (typeof loadMermaid !== 'function') return null; const m = await loadMermaid(); return { type: typeof m, keys: m ? Object.keys(m).slice(0, 40) : null, version: m && (m.version || (m.mermaidAPI && m.mermaidAPI.version)) || null };`); } catch (e) { o.loadMermaid = String(e); }
  o.themes = {};
  for (const t of ['moonstone', 'obsidian']) {
    await setTheme(t);
    o.themes[t] = await ev(`const b = document.body; const cs = getComputedStyle(b); return { vars: __m.vars(b), fontText: cs.getPropertyValue('--font-text').trim(), fontInterface: cs.getPropertyValue('--font-interface').trim(), fontMono: cs.getPropertyValue('--font-monospace').trim(), bgPrimary: cs.getPropertyValue('--background-primary').trim(), textNormal: cs.getPropertyValue('--text-normal').trim(), textMuted: cs.getPropertyValue('--text-muted').trim(), accent: cs.getPropertyValue('--interactive-accent').trim(), fontTextSize: cs.getPropertyValue('--font-text-size').trim(), lineWidth: cs.getPropertyValue('--file-line-width').trim() };`);
  }
  await setTheme('moonstone');
  o.decls = await ev(`return __m.decls();`);
  // Is there a Mermaid setting anywhere?
  o.configKeys = await ev(`const k = Object.keys(app.vault.config || {}); return k.filter((x) => /mermaid|diagram|pele/i.test(x));`);
  return o;
});

await step('s01', async () => {
  const res = {};
  for (const t of ['moonstone', 'obsidian']) {
    await setTheme(t);
    res[t] = {};
    for (const name of Object.keys(CASES)) {
      await ev(`await __m.open(${JSON.stringify(note(name) + '.md')}, 'reading'); return true;`);
      await sleep(600);
      const b = await ev(`return __m.previewBlock();`);
      res[t][name] = b;
      const rect = b.block && (b.block.svg ? b.block.svg.rect : b.block.sectionRect);
      if (rect) b.shot = await shot(`${t}-${name}`, { x: rect.x - 8, y: rect.y - 8, w: rect.w + 16, h: Math.min(rect.h + 16, 990) });
    }
  }
  await setTheme('moonstone');
  return res;
});

await step('s02', async () => {
  const res = {};
  for (const name of ['flow-td', 'sequence', 'error-syntax', 'error-unknown']) {
    await ev(`await __m.open(${JSON.stringify(note(name) + '.md')}, 'live'); return true;`);
    await sleep(800);
    res[name] = await ev(`
      const v = app.workspace.activeLeaf.view; const cm = v.containerEl.querySelector('.cm-content');
      const w = cm.querySelector('.cm-embed-block, .cm-preview-code-block, [class*=mermaid]');
      const o = { widget: w ? __m.block(w) : null, widgetCls: w ? __m.cls(w) : null, lines: Array.from(cm.children).map((l) => __m.cls(l) + ' | ' + (l.textContent || '').slice(0, 60)) };
      if (w) { const btn = w.querySelector('.edit-block-button'); o.edit = btn ? { cls: __m.cls(btn), rect: __m.r(btn), aria: btn.getAttribute('aria-label') } : null; }
      return o;`);
    const r = res[name].widget && res[name].widget.sectionRect;
    if (r) res[name].shot = await shot(`live-${name}`, { x: r.x - 8, y: r.y - 8, w: r.w + 16, h: Math.min(r.h + 16, 990) });
    // Put the caret on the fence: the source shows.
    res[name].caret = await ev(`const ed = app.workspace.activeLeaf.view.editor; ed.focus(); ed.setCursor({ line: 3, ch: 0 }); await __m.sleep(600);
      const cm = app.workspace.activeLeaf.view.containerEl.querySelector('.cm-content');
      return { widget: !!cm.querySelector('.cm-embed-block svg, .cm-preview-code-block svg'), lines: Array.from(cm.children).slice(0, 12).map((l) => __m.cls(l) + ' | ' + (l.textContent || '').slice(0, 60)) };`);
  }
  return res;
});

await step('s03', async () => {
  const res = {};
  await ev(`await __m.open('Embeds.md', 'reading'); return true;`);
  await sleep(1200);
  res.embed = await ev(`const v = app.workspace.activeLeaf.view.containerEl; const e = v.querySelector('.markdown-reading-view .internal-embed'); if (!e) return null; const svg = e.querySelector('svg.pele');
    return { cls: __m.cls(e), rect: __m.r(e), svg: svg ? { rect: __m.r(svg), viewBox: svg.getAttribute('viewBox') } : null, html: e.outerHTML.slice(0, 1200) };`);
  if (res.embed) res.embed.shot = await shot('embed', { x: res.embed.rect.x - 8, y: res.embed.rect.y - 8, w: res.embed.rect.w + 16, h: Math.min(res.embed.rect.h + 16, 990) });
  const link = await ev(`const a = app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-reading-view a.internal-link[data-href="M/sequence"]'); return a ? __m.r(a) : null;`);
  if (link) {
    await c.mouseMove(link.x + link.w / 2, link.y + link.h / 2);
    await sleep(1800);
    res.popover = await ev(`const p = document.querySelector('.popover.hover-popover'); if (!p) return null; const svg = p.querySelector('svg.pele');
      return { rect: __m.r(p), svg: svg ? { rect: __m.r(svg), viewBox: svg.getAttribute('viewBox') } : null, block: (() => { const s = svg && svg.closest('.el-pre, div'); return s ? __m.cls(s) : null; })() };`);
    if (res.popover) res.popover.shot = await shot('popover', { x: res.popover.rect.x, y: res.popover.rect.y, w: res.popover.rect.w, h: res.popover.rect.h });
    await c.mouseMove(5, 500); await sleep(800);
  }
  return res;
});

await step('s04', async () => {
  const res = {};
  await ev(`await __m.open('M/flow-td.md', 'reading'); return true;`);
  await sleep(800);
  const r = await ev(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-reading-view svg.pele'); return __m.r(s.parentElement);`);
  await c.mouseMove(r.x + r.w / 2, r.y + r.h / 2);
  await sleep(700);
  res.hover = await ev(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-reading-view svg.pele'); const sec = s.closest('.el-pre') || s.parentElement.parentElement;
    return { html: sec.outerHTML.replace(/<svg[\\s\\S]*<\\/svg>/, '<svg/>').slice(0, 4000), buttons: Array.from(sec.querySelectorAll('button, .clickable-icon, [aria-label]')).map((b) => ({ tag: b.tagName, cls: __m.cls(b), aria: b.getAttribute('aria-label'), rect: __m.r(b), vis: getComputedStyle(b).opacity + '/' + getComputedStyle(b).display })) };`);
  res.hoverShot = await shot('hover-flow-td', { x: r.x - 8, y: r.y - 8, w: r.w + 16, h: r.h + 16 });
  // A wheel and a drag over the diagram: does it zoom or pan?
  const before = await ev(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-reading-view svg.pele'); return { vb: s.getAttribute('viewBox'), tr: Array.from(s.querySelectorAll('g')).slice(0, 2).map((g) => g.getAttribute('transform')), scroll: app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-preview-view').scrollTop };`);
  await c.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: Math.round(r.x + r.w / 2), y: Math.round(r.y + r.h / 2), deltaX: 0, deltaY: -120, modifiers: 2 });
  await sleep(500);
  const afterCtrlWheel = await ev(`const s = app.workspace.activeLeaf.view.containerEl.querySelector('.markdown-reading-view svg.pele'); return { vb: s.getAttribute('viewBox'), tr: Array.from(s.querySelectorAll('g')).slice(0, 2).map((g) => g.getAttribute('transform')), style: s.getAttribute('style') };`);
  res.zoom = { before, afterCtrlWheel };
  // Click each button found.
  res.clicks = [];
  for (const b of (res.hover.buttons || []).slice(0, 4)) {
    if (!b.rect || !b.rect.w) continue;
    await c.mouseMove(r.x + r.w / 2, r.y + r.h / 2); await sleep(300);
    await c.clickAt(b.rect.x + b.rect.w / 2, b.rect.y + b.rect.h / 2);
    await sleep(800);
    res.clicks.push({ aria: b.aria, mode: await ev(`return app.workspace.activeLeaf.view.getMode ? app.workspace.activeLeaf.view.getMode() : null;`), modal: await ev(`const m = document.querySelector('.modal-container, .pele-zoom, [class*=zoom]'); return m ? __m.cls(m) + ' ' + (m.innerText || '').slice(0, 100) : null;`) });
    await c.pressKey('Escape'); await sleep(300);
    await ev(`await __m.open('M/flow-td.md', 'reading'); return true;`);
  }
  return res;
});

await step('s05', async () => {
  const res = {};
  for (const w of [700, 500]) {
    await c.send('Emulation.setDeviceMetricsOverride', { width: w, height: 1000, deviceScaleFactor: 1, mobile: false });
    await sleep(900);
    res[w] = {};
    for (const name of ['flow-lr-shapes', 'flow-td', 'sequence']) {
      await ev(`await __m.open(${JSON.stringify(note(name) + '.md')}, 'reading'); return true;`);
      await sleep(700);
      res[w][name] = await ev(`const b = __m.previewBlock(); return { sizer: b.sizer, svg: b.block && b.block.svg ? { rect: b.block.svg.rect, viewBox: b.block.svg.viewBox } : null };`);
    }
  }
  await c.send('Emulation.setDeviceMetricsOverride', { width: 1400, height: 1000, deviceScaleFactor: 1, mobile: false });
  return res;
});

c.close();
process.exit(0);
