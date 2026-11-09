// What a rename or a move does to `bookmarks.json` on Obsidian 1.15.0:
// a bookmarked note renamed, moved into a folder, and moved by its folder; a
// heading and a block bookmark of it; a bookmarked folder renamed (and the
// notes inside it); a bookmark of an attachment; a delete. The file's bytes
// before and after each step, so the key order of a rewritten item is on
// record too.
// Phases: fixtures | run   (node bookmarks-rename.mjs <phase>)
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
const all = (() => { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } })();
all.bookmarksRename = all.bookmarksRename || {};
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
all.bookmarksRename[phase] = out;
const save = () => fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1));
const vp = (rel) => path.join(VAULT, rel);
const BM = '.obsidian/bookmarks.json';
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch { return null; } };
async function waitForChange(before, ms = 4000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (bytes(BM) !== before) { await sleep(400); return true; } await sleep(150); }
  return false;
}

if (phase === 'fixtures') {
  for (const d of ['Folder', 'Dest']) fs.mkdirSync(vp(d), { recursive: true });
  fs.writeFileSync(vp('Note.md'), '# Note\n\n## Head\n\nA block. ^blk\n');
  fs.writeFileSync(vp('Folder/Inner.md'), '# Inner\n');
  fs.writeFileSync(vp('pic.png'), Buffer.from('89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082', 'hex'));
  fs.writeFileSync(vp('Gone.md'), '# Gone\n');
  fs.writeFileSync(vp('.obsidian/core-plugins.json'), JSON.stringify({ 'file-explorer': true, bookmarks: true }, null, 2));
  const ct = 1792373000000;
  fs.writeFileSync(vp(BM), JSON.stringify({ items: [
    { type: 'file', ctime: ct, path: 'Note.md' },
    { type: 'file', ctime: ct + 1, path: 'Note.md', subpath: '#Head' },
    { type: 'file', ctime: ct + 2, path: 'Note.md', subpath: '#^blk', title: 'Titled block' },
    { type: 'folder', ctime: ct + 3, path: 'Folder' },
    { type: 'group', ctime: ct + 4, items: [{ type: 'file', ctime: ct + 5, path: 'Folder/Inner.md' }], title: 'G' },
    { type: 'file', ctime: ct + 6, path: 'pic.png' },
    { type: 'file', ctime: ct + 7, path: 'Gone.md' },
  ] }, null, 2));
  save();
  process.exit(0);
}

const c = await connect();
async function step(name, op) {
  out.order.push(name);
  const before = bytes(BM);
  try {
    await c.evaluate(op);
    const changed = await waitForChange(before);
    out.steps[name] = { ok: true, changed, after: bytes(BM), modal: await c.evaluate(`const m = document.querySelector('.modal-container .modal'); return m && m.innerText.slice(0, 200);`) };
    if (out.steps[name].modal) { await c.pressKey('Escape'); await sleep(300); }
  } catch (e) { out.steps[name] = { ok: false, error: String(e) }; }
  save();
}
out.before = bytes(BM);
const mv = (from, to) => `await app.fileManager.renameFile(app.vault.getAbstractFileByPath(${JSON.stringify(from)}), ${JSON.stringify(to)}); return true;`;
await step('s01-rename-note', mv('Note.md', 'Renamed.md'));
await step('s02-move-note', mv('Renamed.md', 'Dest/Renamed.md'));
await step('s03-rename-folder', mv('Folder', 'Folder2'));
await step('s04-move-folder', mv('Folder2', 'Dest/Folder2'));
await step('s05-rename-attachment', mv('pic.png', 'picture.png'));
await step('s06-delete-note', `await app.vault.trash(app.vault.getAbstractFileByPath('Gone.md'), true); return true;`);
c.close();
save();
process.exit(0);
