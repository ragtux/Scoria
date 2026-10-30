// Bases property ids: which keys Obsidian's writer normalizes, and how. On
// 2026-10-30 Obsidian 1.14.5 rewrote the demo vault's `Rocks.base`: every
// `note.x` in `order` and `sort` came back bare `x`, while `properties` kept
// its `note.silica` key and a header drag wrote `columnSize` keyed
// `note.silica`. The 2026-10-25 run (bases.mjs) never put a qualified id in
// `order` or `sort`, so the strip was never observed there. This script asks
// the question directly, one fixture with every id-bearing key in both
// spellings:
//
//   - which of `order`, `sort`, `groupBy`, `summaries`, `columnSize`, `image`
//     and the `properties` keys drop `note.` on save, and which keep it;
//   - whether `file.` and `formula.` ids are touched;
//   - whether a `columnSize` keyed `note.status` still sizes the `status`
//     column once `order` says `status`;
//   - what key a header drag writes for a column `order` names bare, and
//     whether a stale `note.`-keyed entry for the same column survives.
//
// Phases: fixtures | run   (node bases-ids.mjs <phase>), through run-block.sh.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run'].includes(phase)) { console.error('phase must be fixtures|run'); process.exit(2); }

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
all.basesIds = all.basesIds || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.basesIds[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn) {
  out.order.push(name);
  const t = Date.now();
  const partial = {};
  try { const v = await fn(partial); out.steps[name] = { ok: true, ms: Date.now() - t, ...(v && typeof v === 'object' ? v : { value: v }) }; }
  catch (e) { out.steps[name] = { ok: false, ms: Date.now() - t, error: String(e && e.stack ? e.stack : e), partial }; }
  save();
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };
async function waitForWrite(rel, before, timeoutMs = 12000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    const now = bytes(rel);
    if (now !== before) { await sleep(600); return { changed: true, text: bytes(rel) }; }
    await sleep(200);
  }
  return { changed: false, text: bytes(rel) };
}

// ---- fixtures -------------------------------------------------------------
const note = (name, fm, body) => [name, `---\n${fm}\n---\n${body}\n`];
const NOTES = Object.fromEntries([
  note('Scoria.md', 'type: rock\nstatus: todo\nprice: 3\ndensity: 1.21\ncover: pic.png', '# Scoria'),
  note('Pumice.md', 'type: rock\nstatus: done\nprice: 5\ndensity: 0.64\ncover: pic.png', '# Pumice'),
  note('Basalt.md', 'type: rock\nstatus: todo\nprice: 1\ndensity: 2.9', '# Basalt'),
]);

// Every id-bearing key, the qualified spelling first. The `Bare` view is the
// control: ids already bare, and a `columnSize` keyed bare.
const IDS = `properties:
  note.price:
    displayName: Price
  status:
    displayName: Status
  formula.ppu:
    displayName: Per unit
formulas:
  ppu: price / density
filters:
  and:
    - type == "rock"
views:
  - type: table
    name: Qualified
    order:
      - file.name
      - note.status
      - note.price
      - note.density
      - formula.ppu
      - file.ext
    sort:
      - property: note.price
        direction: DESC
      - property: formula.ppu
        direction: ASC
      - property: file.name
        direction: ASC
    groupBy:
      property: note.status
      direction: ASC
    summaries:
      note.price: Average
      formula.ppu: Sum
    columnSize:
      file.name: 120
      note.status: 150
      note.price: 160
  - type: cards
    name: Pics
    order:
      - file.name
      - note.status
    image: note.cover
  - type: table
    name: Bare
    order:
      - file.name
      - status
      - price
    sort:
      - property: price
        direction: DESC
    columnSize:
      status: 150
`;

if (phase === 'fixtures') {
  await step('fixtures', async () => {
    fs.mkdirSync(vp('.obsidian'), { recursive: true });
    fs.writeFileSync(vp('.obsidian/app.json'), '{}');
    fs.writeFileSync(vp('Ids.base'), IDS);
    fs.writeFileSync(vp('pic.png'), '');
    for (const [name, text] of Object.entries(NOTES)) fs.writeFileSync(vp(name), text);
    return { vaultFiles: fs.readdirSync(VAULT).sort() };
  });
  process.exit(0);
}

// ---- run ------------------------------------------------------------------
const c = await connect();
const vault = await c.vaultPath();
if (vault.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault open: ' + vault); c.close(); process.exit(3); }
out.vault = vault;
out.version = await c.evaluate(`return { title: document.title, ua: navigator.userAgent };`);

const openBase = async (p) => {
  await c.evaluate(`app.workspace.detachLeavesOfType('bases'); app.workspace.detachLeavesOfType('markdown'); return true;`);
  await sleep(1500);
  await c.evaluate(`
    let f = null;
    for (let i = 0; i < 40 && !f; i++) {
      f = app.vault.getAbstractFileByPath(${JSON.stringify(p)});
      if (!f) await new Promise((r) => setTimeout(r, 500));
    }
    if (!f) throw new Error('missing ' + ${JSON.stringify(p)});
    const leaf = app.workspace.getLeaf('tab');
    await leaf.openFile(f);
    app.workspace.setActiveLeaf(leaf, { focus: true });
    return true;`);
  await sleep(3000);
};
const closeActive = async () => { await c.evaluate(`app.workspace.activeLeaf.detach(); return true;`); await sleep(2500); };

const modelDump = () => c.evaluate(`
  const v = app.workspace.activeLeaf && app.workspace.activeLeaf.view;
  if (!v) return { noView: true };
  const safe = (f) => { try { return f(); } catch (e) { return 'ERR:' + e.message; } };
  const q = v.query;
  return {
    viewType: safe(() => v.getViewType()),
    serializable: safe(() => JSON.parse(JSON.stringify(q.getSerializable()))),
    toString: safe(() => q.toString()),
    viewName: safe(() => v.controller ? v.controller.viewName : null),
    // the live view objects: what the model holds for each id-bearing key
    views: safe(() => q.views.map((w) => ({
      name: w.name, type: w.type,
      order: JSON.parse(JSON.stringify(w.order === undefined ? null : w.order)),
      sort: JSON.parse(JSON.stringify(w.sort === undefined ? null : w.sort)),
      groupBy: JSON.parse(JSON.stringify(w.groupBy === undefined ? null : w.groupBy)),
      keys: Object.keys(w),
    }))),
  };`);

const headers = () => c.evaluate(`
  return Array.from(document.querySelectorAll('.bases-table-header')).map((e) => {
    const b = e.getBoundingClientRect();
    return { text: e.textContent.trim(), w: Math.round(b.width), prop: e.dataset ? (e.dataset.property || e.dataset.id || null) : null, attrs: Array.from(e.attributes).map((a) => a.name + '=' + a.value).join(' ').slice(0, 200) }; });`);

// 01 — open: the model as parsed, the bytes untouched, and whether the
// `note.status: 150` width applies to the status column.
await step('s01-open', async (p) => {
  p.before = bytes('Ids.base');
  await openBase('Ids.base');
  p.model = await modelDump();
  p.afterOpen = bytes('Ids.base');
  p.rewrittenOnOpen = p.afterOpen !== p.before;
  p.headers = await headers();
  return p;
});

// 02 — one API edit, the same path s10 of bases.mjs took, and the bytes it
// writes: this is the answer to which keys normalize.
await step('s02-save', async (p) => {
  p.before = bytes('Ids.base');
  p.setter = await c.evaluate(`
    const q = app.workspace.activeLeaf.view.query;
    try { q.views[0].setLimit(7); return { ok: true }; } catch (e) { return { ok: false, error: e.message }; }`);
  const w = await waitForWrite('Ids.base', p.before, 12000);
  p.wrote = w.changed;
  p.after = w.text;
  p.model = await modelDump();
  p.headers = await headers();
  return p;
});

// 03 — a header drag on the status column (index 1), whose `order` entry is
// now whatever s02 wrote: the key `columnSize` gets, and whether the fixture's
// `note.status: 150` survives beside it.
await step('s03-drag-status', async (p) => {
  p.before = bytes('Ids.base');
  const rz = await c.evaluate(`
    const els = Array.from(document.querySelectorAll('.bases-table-header-resizer'));
    const el = els[1]; if (!el) return null;
    const b = el.getBoundingClientRect();
    return { x: b.x + b.width / 2, y: b.y + b.height / 2, count: els.length };`);
  p.resizer = rz;
  if (!rz) return { ...p, note: 'no second .bases-table-header-resizer' };
  await c.drag(rz.x, rz.y, rz.x + 90, rz.y, { steps: 12, pauseMs: 40 });
  await sleep(1200);
  const w = await waitForWrite('Ids.base', p.before, 12000);
  p.wrote = w.changed;
  p.after = w.text;
  p.headers = await headers();
  return p;
});

// 04 — close, and the final bytes.
await step('s04-close', async (p) => {
  await closeActive();
  await sleep(2000);
  p.final = bytes('Ids.base');
  return p;
});

save();
c.close();
process.exit(0);
