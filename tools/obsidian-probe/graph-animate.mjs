// graph:animate: the graph view's timelapse.
//
//   s00  context: version, the command's name, icon, hotkey; when the palette
//        offers it (a note active, the graph view active, a local graph)
//   s01  the graph open and settled: its nodes, and every file's ctime/mtime
//   s02  graph:animate with the graph active: the nodes sampled every 50 ms
//        (count and the order each first appears), screenshots on the way,
//        graph.json and workspace.json before and after
//   s03  the controls' "Start timelapse animation" button: the same?
//   s04  graph:animate again while one runs
//   s05  a local graph
//
// The fixtures are created one by one a second apart, in an order that is
// neither the names' nor the mtimes' (those are set in reverse), so the
// order of appearance says which one the timelapse follows.
// Screenshots go to <results>-shots/. FROM_STEP=s03 runs that step and later.
// Phases: fixtures | run
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

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
if (phase === 'fixtures') out.created = [];
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 150000) {
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
  console.error(`[graph-animate] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

// Creation order (one a second apart): neither alphabetical nor mtime order.
const CREATE = ['Mango', 'Apple', 'Zebra', 'Kiwi', 'Banana', 'Orphan', 'Cherry', 'Lemon'];
const LINKS = { Mango: ['Apple'], Apple: ['Zebra', 'Kiwi'], Zebra: [], Kiwi: ['Banana', 'Missing'], Banana: ['Mango'], Orphan: [], Cherry: ['Lemon', 'Apple'], Lemon: [] };

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({
    'file-explorer': true, 'global-search': true, switcher: true, graph: true, backlink: true,
    'outgoing-link': true, 'command-palette': true, 'file-recovery': true, 'page-preview': true,
  }, null, 2));
  const base = Date.now() / 1000;
  CREATE.forEach((n, i) => {
    const body = `# ${n}\n\n` + LINKS[n].map((l) => `[[${l}]]`).join(' ') + '\n';
    fs.writeFileSync(vp(n + '.md'), body);
    // mtimes run backwards: the first created is the most recently modified.
    fs.utimesSync(vp(n + '.md'), base, base + 100000 - i * 3600);
    const st = fs.statSync(vp(n + '.md'), { bigint: false });
    out.created.push({ name: n, birthtimeMs: st.birthtimeMs, ctimeMs: st.ctimeMs, mtimeMs: st.mtimeMs });
    const until = Date.now() + 1100; while (Date.now() < until) { /* spin a second so birth times differ */ }
  });
  save();
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }

await c.evaluate(`
window.__g = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  graphLeaf(type) { return app.workspace.getLeavesOfType(type || 'graph')[0] || null; },
  nodes(type) {
    const l = this.graphLeaf(type); if (!l || !l.view.renderer) return null;
    const r = l.view.renderer;
    return (r.nodes || []).map((n) => ({ id: n.id, type: n.type, x: Math.round(n.x || 0), y: Math.round(n.y || 0),
      shown: n.rendered !== undefined ? !!n.rendered : undefined, alpha: n.circle && n.circle.alpha !== undefined ? Math.round(n.circle.alpha * 100) / 100 : undefined }));
  },
  rendererKeys(type) { const l = this.graphLeaf(type); if (!l || !l.view.renderer) return null; return Object.keys(l.view.renderer).sort(); },
  viewKeys(type) { const l = this.graphLeaf(type); if (!l) return null; return Object.keys(l.view).sort(); },
  offered(id) { return app.commands.listCommands().some((x) => x.id === id); },
  files() { return app.vault.getFiles().map((f) => ({ path: f.path, ctime: f.stat.ctime, mtime: f.stat.mtime })).sort((a, b) => a.path < b.path ? -1 : 1); },
};
return true;`);
const ev = (s) => c.evaluate(s);
const G = 'const G = window.__g;';
let shotN = 0;
async function shot(name) {
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}

/** Sample the graph's nodes every `every` ms for `ms`, recording counts and first appearances. */
async function sampleAnimation(ms, type, shotsAt = []) {
  const t0 = Date.now();
  const firstSeen = {};
  const counts = [];
  const shots = [];
  let lastIds = '';
  let stableSince = null;
  while (Date.now() - t0 < ms) {
    const t = Date.now() - t0;
    const nodes = await ev(`${G} return G.nodes(${JSON.stringify(type || 'graph')});`);
    const ids = (nodes || []).map((n) => n.id);
    for (const id of ids) if (!(id in firstSeen)) firstSeen[id] = t;
    const shownCount = (nodes || []).filter((n) => n.shown).length;
    counts.push([t, ids.length, shownCount]);
    if (shotsAt.length && t >= shotsAt[0]) { shotsAt.shift(); shots.push([t, await shot((type || 'graph') + '-t' + t)]); }
    const key = ids.join('|');
    if (key !== lastIds) { lastIds = key; stableSince = t; }
    else if (t - stableSince > 4000 && ids.length) break;
    await sleep(50);
  }
  const order = Object.entries(firstSeen).sort((a, b) => a[1] - b[1]);
  // Compress counts to changes only.
  const changes = counts.filter((x, i) => i === 0 || x[1] !== counts[i - 1][1] || x[2] !== counts[i - 1][2]);
  return { order, changes, shots, samples: counts.length };
}

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight] };`);
  const cmds = await ev(`const o = {}; for (const id of ['graph:animate', 'graph:open', 'graph:open-local']) { const x = app.commands.commands[id]; o[id] = x ? { name: x.name, icon: x.icon || null, keys: (app.hotkeyManager.defaultKeys || {})[id] || null, hasCheck: !!x.checkCallback, hasCallback: !!x.callback } : null; } return o;`);
  const offered = {};
  offered.noteActive = await ev(`${G} const f = app.vault.getAbstractFileByPath('Mango.md'); await app.workspace.getLeaf(false).openFile(f); await G.sleep(600); return G.offered('graph:animate');`);
  await ev(`return app.commands.executeCommandById('graph:open');`); await sleep(2500);
  offered.graphActive = await ev(`${G} return { active: app.workspace.activeLeaf.view.getViewType(), offered: G.offered('graph:animate') };`);
  offered.graphNotActive = await ev(`${G} const md = app.workspace.getLeavesOfType('markdown')[0]; if (md) app.workspace.setActiveLeaf(md, { focus: true }); await G.sleep(500); return { active: app.workspace.activeLeaf.view.getViewType(), offered: G.offered('graph:animate') };`);
  return { version: out.version, cmds, offered, files: await ev(`${G} return G.files();`) };
});

await step('s01-settled', async () => {
  await ev(`${G} const l = G.graphLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  await sleep(3000);
  return {
    nodes: await ev(`${G} return G.nodes();`), rendererKeys: await ev(`${G} return G.rendererKeys();`), viewKeys: await ev(`${G} return G.viewKeys();`),
    controls: await ev(`return Array.from(document.querySelectorAll('.graph-controls-button')).map((b) => ({ cls: b.className, aria: b.getAttribute('aria-label'), svg: b.querySelector('svg') ? b.querySelector('svg').getAttribute('class') : null }));`),
    shot: await shot('settled'),
  };
});

await step('s02-command', async () => {
  await ev(`${G} const l = G.graphLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  await sleep(500);
  const graphBefore = bytes('.obsidian/graph.json');
  const wsBefore = bytes('.obsidian/workspace.json');
  const ran = await ev(`return app.commands.executeCommandById('graph:animate');`);
  const res = await sampleAnimation(30000, 'graph', [100, 600, 1500, 3000, 6000]);
  await sleep(2500);
  const ui = await ev(`return { animateBtn: Array.from(document.querySelectorAll('.graph-controls-button.mod-animate')).map((b) => ({ cls: b.className, aria: b.getAttribute('aria-label'), svg: b.querySelector('svg') && b.querySelector('svg').getAttribute('class') })), notices: Array.from(document.querySelectorAll('.notice')).map((n) => n.textContent) };`);
  return { ran, ...res, ui, graphChanged: bytes('.obsidian/graph.json') !== graphBefore, graphAfter: bytes('.obsidian/graph.json'), wsChanged: bytes('.obsidian/workspace.json') !== wsBefore, end: await shot('after-command') };
});

await step('s03-button', async () => {
  await ev(`${G} const l = G.graphLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); const c = document.querySelector('.graph-controls'); if (c && c.classList.contains('is-close')) { const o = document.querySelector('.graph-controls-button.mod-open'); if (o) o.click(); } return true;`);
  await sleep(600);
  const b = await c.rect('.graph-controls-button.mod-animate');
  if (!b) return { error: 'no animate button' };
  await c.mouseMove(b.cx, b.cy); await sleep(800);
  const tooltip = await ev(`return Array.from(document.querySelectorAll('.tooltip')).map((t) => t.textContent);`);
  await c.clickAt(b.cx, b.cy);
  const res = await sampleAnimation(30000, 'graph', [300, 2000]);
  // While running: what does the button read?
  return { rect: b, tooltip, ...res };
});

await step('s04-twice', async () => {
  await ev(`return app.commands.executeCommandById('graph:animate');`);
  await sleep(700);
  const mid = await ev(`${G} return G.nodes().length;`);
  const midBtn = await ev(`return Array.from(document.querySelectorAll('.graph-controls-button.mod-animate')).map((b) => ({ cls: b.className, aria: b.getAttribute('aria-label'), svg: b.querySelector('svg') && b.querySelector('svg').getAttribute('class') }));`);
  await ev(`return app.commands.executeCommandById('graph:animate');`);
  const res = await sampleAnimation(30000, 'graph', []);
  return { midCount: mid, midBtn, ...res };
});

const ensureGraph = async () => {
  const has = await ev(`return app.workspace.getLeavesOfType('graph').length > 0;`);
  if (!has) { await ev(`return app.commands.executeCommandById('graph:open');`); await sleep(3000); }
};

// A markdown leaf beside the graph, active: is the command still offered?
await step('s05-local', async () => {
  await ensureGraph();
  const beside = await ev(`${G} const g = G.graphLeaf(); const md = app.workspace.createLeafBySplit(g, 'vertical'); await md.openFile(app.vault.getAbstractFileByPath('Apple.md')); app.workspace.setActiveLeaf(md, { focus: true }); await G.sleep(600);
    return { active: app.workspace.activeLeaf.view.getViewType(), graphs: app.workspace.getLeavesOfType('graph').length, offered: G.offered('graph:animate') };`);
  out.besideGraph = beside;
  await ev(`return app.commands.executeCommandById('graph:open-local');`); await sleep(2500);
  const local = await ev(`${G} const l = G.graphLeaf('localgraph'); if (l) app.workspace.setActiveLeaf(l, { focus: true }); await G.sleep(400); return { active: app.workspace.activeLeaf.view.getViewType(), offered: G.offered('graph:animate'), nodes: G.nodes('localgraph') };`);
  await ev(`return app.commands.executeCommandById('graph:animate');`);
  const res = await sampleAnimation(20000, 'localgraph', [300]);
  return { beside, local, ...res };
});

// The clock: five notes made now with uneven gaps (0.5, 0.5, 6, 0.5 s),
// then the timelapse again. If the schedule follows ctime, the 6 s gap shows
// up; the total says whether the duration is fixed.
await step('s06-uneven', async () => {
  await ev(`for (const l of app.workspace.getLeavesOfType('localgraph')) l.detach(); for (const l of app.workspace.getLeavesOfType('markdown')) l.detach(); await new Promise((r) => setTimeout(r, 400)); return true;`);
  await ensureGraph();
  const gaps = [0, 500, 500, 6000, 500];
  const made = [];
  for (let i = 0; i < gaps.length; i++) {
    await sleep(gaps[i]);
    const name = `New${i + 1}`;
    fs.writeFileSync(vp(name + '.md'), `# ${name}\n\n[[Mango]]\n`);
    made.push({ name, birthtimeMs: fs.statSync(vp(name + '.md')).birthtimeMs });
  }
  await sleep(2500);
  const files = await ev(`${G} return G.files();`);
  await ev(`${G} const l = G.graphLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  await sleep(1500);
  await ev(`return app.commands.executeCommandById('graph:animate');`);
  const res = await sampleAnimation(40000, 'graph', [500, 2500]);
  return { made, files, ...res };
});

// The controls' button, with the panel open and the pointer over it.
await step('s07-button', async () => {
  await ensureGraph();
  await ev(`${G} const l = G.graphLeaf(); app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  const panel = await ev(`const c = document.querySelector('.graph-controls'); const r = c.getBoundingClientRect(); return { cls: c.className, rect: [r.x, r.y, r.width, r.height] };`);
  await c.mouseMove(panel.rect[0] + panel.rect[2] / 2, panel.rect[1] + 10); await sleep(800);
  const btns = await ev(`return Array.from(document.querySelectorAll('.graph-controls-button')).map((b) => { const r = b.getBoundingClientRect(); return { cls: b.className, aria: b.getAttribute('aria-label'), rect: [r.x, r.y, r.width, r.height], display: getComputedStyle(b).display, opacity: getComputedStyle(b).opacity }; });`);
  const b = btns.find((x) => /mod-animate/.test(x.cls) && x.rect[2] > 0);
  const res = { panel, btns, shot: await shot('controls-hover') };
  if (!b) return res;
  await c.mouseMove(b.rect[0] + b.rect[2] / 2, b.rect[1] + b.rect[3] / 2); await sleep(900);
  res.tooltip = await ev(`return Array.from(document.querySelectorAll('.tooltip')).map((t) => t.textContent);`);
  await c.clickAt(b.rect[0] + b.rect[2] / 2, b.rect[1] + b.rect[3] / 2);
  Object.assign(res, await sampleAnimation(40000, 'graph', [400]));
  return res;
});

save();
console.log(JSON.stringify({ steps: out.order.map((s) => [s, out.steps[s] && out.steps[s].ok]) }));
c.close();
process.exit(0);
