// The audio player's extras: what Chromium's `<audio controls>` inside
// Obsidian draws beyond play, time, seek and mute — the volume slider and
// its hover, the overflow (⋮) menu and its playback speeds — where Space
// plays and pauses, whether two embeds of one file are two players, what
// happens to a playing embed scrolled away or left in a background tab, and
// Live Preview's margins around an audio embed. Real input over CDP.
//
//   s00  context: version, window
//   s01  an audio tab: the player's user-agent shadow tree (every part's
//        pseudo id, rect, text, aria-label) and a screenshot
//   s02  the volume button hovered: the slider's rect at 0, 150, 400 and
//        1000 ms; a click on the slider; the pointer leaving and when the
//        slider closes; the arrow keys on the focused slider
//   s03  the overflow menu: its rows, then Playback speed's rows, a speed
//        picked (playbackRate, preservesPitch), the row checked when the
//        menu opens again, Escape and an outside click
//   s04  Space: in the tab with the view focused, after a click on play,
//        after a click on the seek bar, the volume slider; in the reading
//        view with the caret nowhere and after a click on play; in Live
//        Preview with the caret in a line and after a click on play
//   s05  two embeds of one file: is playing one playing the other
//   s06  the margins: Live Preview's lines and the embed between them, with
//        and without blank lines, beside the reading view's
//   s07  a playing embed scrolled far away (reading view, Live Preview) and
//        a playing tab left for another tab
//
// MEDIA_DIR holds tone.mp3 (3 s) and long.mp3 (60 s). Screenshots go to
// <results>-shots/. ONLY_STEPS=<regex> narrows the run.
import fs from 'node:fs';
import path from 'node:path';
import { connect, sleep } from './cdp.mjs';

const phase = process.argv[2];
const VAULT = process.env.VAULT;
const RESULTS = process.env.RESULTS;
const MEDIA = process.env.MEDIA_DIR;
const ONLY = process.env.ONLY_STEPS ? new RegExp(process.env.ONLY_STEPS) : null;
if (!VAULT || !RESULTS) { console.error('VAULT and RESULTS env vars are required'); process.exit(2); }
if (!['fixtures', 'run', 'after-restart'].includes(phase)) { console.error('phase must be fixtures|run|after-restart'); process.exit(2); }
const SHOTS = RESULTS.replace(/\.json$/, '-shots');
fs.mkdirSync(SHOTS, { recursive: true });

const T0 = Date.now();
function loadResults() { try { return JSON.parse(fs.readFileSync(RESULTS, 'utf8')); } catch { return {}; } }
const all = loadResults();
const out = { startedAt: new Date().toISOString(), steps: {}, order: [] };
if (phase === 'run' && ONLY && all.run) Object.assign(out, { steps: all.run.steps || {}, order: all.run.order || [] });
all[phase] = out;
const save = () => { out.elapsedMs = Date.now() - T0; fs.writeFileSync(RESULTS, JSON.stringify(all, null, 1)); };
save();

async function step(name, fn, ms = 120000) {
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
  console.error(`[audio] ${name}: ${out.steps[name].ok ? 'ok' : 'FAILED ' + out.steps[name].error.split('\n')[0]}`);
  return out.steps[name];
}

const vp = (rel) => path.join(VAULT, rel);
const bytes = (rel) => { try { return fs.readFileSync(vp(rel), 'utf8'); } catch (e) { return e.code === 'ENOENT' ? null : 'ERR:' + e.message; } };

if (phase === 'fixtures') {
  fs.mkdirSync(vp('.obsidian'), { recursive: true });
  fs.writeFileSync(vp('.obsidian/app.json'), '{}');
  if (MEDIA) for (const f of fs.readdirSync(MEDIA)) fs.copyFileSync(path.join(MEDIA, f), vp(f));
  fs.writeFileSync(vp('Embed.md'), '# Embed\n\nA line before the player.\n\n![[long.mp3]]\n\nA line after the player.\n');
  fs.writeFileSync(vp('Twice.md'), '# Twice\n\n![[long.mp3]]\n\n![[long.mp3]]\n');
  fs.writeFileSync(vp('Margins.md'), 'Line one\n\n![[tone.mp3]]\n\nLine two\n![[tone.mp3]]\nLine three\n\n![[tone.mp3]]\n![[tone.mp3]]\nLine four\n');
  const filler = Array.from({ length: 400 }, (_, i) => `Filler paragraph ${i + 1}, long enough to take a line of its own in the note.`).join('\n\n');
  fs.writeFileSync(vp('Long.md'), `# Long\n\n![[long.mp3]]\n\n${filler}\n`);
  fs.writeFileSync(vp('Other.md'), '# Other\n\nAnother note.\n');
  process.exit(0);
}
if (phase === 'after-restart') { save(); process.exit(0); }

const c = await connect();
out.vaultPath = await c.vaultPath();
if (out.vaultPath.replace(/\/+$/, '') !== VAULT.replace(/\/+$/, '')) { console.error('wrong vault'); process.exit(3); }
await c.send('DOM.enable');
await c.send('CSS.enable');

await c.evaluate(`
window.__s = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  cls: (e) => e ? (typeof e.className === 'string' ? e.className : e.getAttribute('class')) : null,
  rect: (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return [Math.round(r.x * 10) / 10, Math.round(r.y * 10) / 10, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10]; },
  leaves() { const l = []; app.workspace.iterateRootLeaves((x) => l.push(x)); return l; },
  leaf() {
    const leaves = this.leaves();
    for (const l of leaves.slice(1)) l.detach();
    return leaves[0] || app.workspace.getLeaf(false);
  },
  async open(path, mode) {
    const leaf = this.leaf();
    if (!leaf.view.file || leaf.view.file.path !== path) await leaf.openFile(app.vault.getAbstractFileByPath(path));
    const vs = leaf.getViewState();
    if (vs.type === 'markdown') {
      if (mode === 'live') { vs.state.mode = 'source'; vs.state.source = false; }
      if (mode === 'preview') { vs.state.mode = 'preview'; }
      await leaf.setViewState(vs);
    }
    app.workspace.setActiveLeaf(leaf, { focus: true });
    await this.sleep(1200);
    return { file: leaf.view.file && leaf.view.file.path, type: leaf.view.getViewType(), state: leaf.getViewState().state };
  },
  root() { return app.workspace.activeLeaf.view.containerEl; },
  audios() { return Array.from(document.querySelectorAll('.workspace-leaf audio')).filter((a) => a.isConnected); },
  visibleAudios() { return Array.from(this.root().querySelectorAll('audio')).filter((a) => a.offsetParent); },
  st(a) { return a ? { paused: a.paused, t: Math.round(a.currentTime * 1000) / 1000, volume: a.volume, muted: a.muted, rate: a.playbackRate, pitch: a.preservesPitch, connected: a.isConnected } : null; },
  active() { const e = document.activeElement; return e ? { tag: e.tagName, cls: this.cls(e) } : null; },
};
return true;`);
const ev = (s) => c.evaluate(s);
const S = 'const S = window.__s;';
let shotN = 0;
async function shot(name) {
  try {
    const r = await c.send('Page.captureScreenshot', { format: 'png' });
    const f = `${String(++shotN).padStart(2, '0')}-${name}.png`;
    fs.writeFileSync(path.join(SHOTS, f), Buffer.from(r.data, 'base64'));
    return f;
  } catch (e) { return 'shot failed: ' + e.message; }
}
const open = (p, m) => ev(`${S} return await S.open(${JSON.stringify(p)}, ${JSON.stringify(m || null)});`);
const move = (x, y) => c.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: Math.round(x), y: Math.round(y) });
const space = async () => {
  await c.send('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32, nativeVirtualKeyCode: 32, text: ' ', unmodifiedText: ' ' });
  await c.send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32, nativeVirtualKeyCode: 32 });
  await sleep(400);
};
const pauseAll = () => ev(`${S} document.querySelectorAll('audio').forEach((a) => { a.pause(); a.currentTime = 0; a.playbackRate = 1; a.volume = 1; a.muted = false; }); return true;`);

// The user-agent shadow tree of the `index`th visible audio: every element
// with its pseudo id, rect, text, aria-label, and a few computed styles.
async function shadow(index = 0) {
  const { result } = await c.send('Runtime.evaluate', { expression: `window.__s.visibleAudios()[${index}] || null` });
  if (!result.objectId) return null;
  // The node ids below are only valid once the document has been asked for.
  await c.send('DOM.getDocument', { depth: 0 });
  const { node } = await c.send('DOM.describeNode', { objectId: result.objectId, depth: -1, pierce: true });
  const flat = [];
  const walk = async (n, depth) => {
    if (n.nodeType === 1) {
      const attrs = {}; for (let i = 0; i < (n.attributes || []).length; i += 2) attrs[n.attributes[i]] = n.attributes[i + 1];
      let box = null; let styles = null;
      try { const b = await c.send('DOM.getBoxModel', { backendNodeId: n.backendNodeId }); const q = b.model.border; box = [q[0], q[1], Math.round((q[2] - q[0]) * 10) / 10, Math.round((q[5] - q[1]) * 10) / 10]; } catch { box = null; }
      if (box) {
        try {
          const { nodeIds } = await c.send('DOM.pushNodesByBackendIdsToFrontend', { backendNodeIds: [n.backendNodeId] });
          const { computedStyle } = await c.send('CSS.getComputedStyleForNode', { nodeId: nodeIds[0] });
          const want = ['display', 'visibility', 'opacity', 'width', 'height', 'background-color', 'color', 'font-size', 'border-radius', 'padding', 'margin', 'transition', 'cursor', 'accent-color'];
          styles = Object.fromEntries(computedStyle.filter((p) => want.includes(p.name)).map((p) => [p.name, p.value]));
        } catch { styles = null; }
      }
      const text = (n.children || []).filter((k) => k.nodeType === 3).map((k) => k.nodeValue).join('').trim();
      flat.push({ depth, tag: n.localName, pseudo: attrs.pseudo, id: attrs.id, aria: attrs['aria-label'], role: attrs.role, type: attrs.type, cls: attrs.class, value: attrs.value, attrs, text: text || undefined, box, styles });
    }
    for (const k of [...(n.shadowRoots || []), ...(n.children || [])]) await walk(k, depth + 1);
  };
  await walk(node, 0);
  return flat;
}
const part = (tree, pseudo) => tree && tree.find((n) => n.pseudo === pseudo);
const centre = (b) => [b[0] + b[2] / 2, b[1] + b[3] / 2];
const visible = (tree) => (tree || []).filter((n) => n.box && n.box[2] > 0 && n.box[3] > 0 && !(n.styles && (n.styles.display === 'none' || n.styles.visibility === 'hidden'))).map((n) => ({ pseudo: n.pseudo || n.tag, box: n.box, text: n.text, aria: n.aria, opacity: n.styles && n.styles.opacity }));

await step('s00-context', async () => {
  out.version = await ev(`return { title: document.title, ua: navigator.userAgent, win: [innerWidth, innerHeight], dpr: devicePixelRatio };`);
  await ev(`app.workspace.leftSplit.collapse(); app.workspace.rightSplit.collapse(); return true;`);
  return { version: out.version };
});

await step('s01-shadow-tab', async () => {
  const o = await open('long.mp3');
  await sleep(1500);
  const tree = await shadow(0);
  return { open: o, audio: await ev(`${S} const a = S.visibleAudios()[0]; return { rect: S.rect(a), st: S.st(a), attrs: Array.from(a.attributes).map((x) => [x.name, x.value.slice(0, 60)]) };`), tree, shot: await shot('tab') };
});

await step('s02-volume', async () => {
  await open('long.mp3');
  await move(5, 300); await sleep(600);
  const res = {};
  const t0 = await shadow(0);
  res.before = visible(t0);
  const mute = part(t0, '-webkit-media-controls-mute-button');
  res.mute = mute && { box: mute.box, aria: mute.aria };
  if (!mute) return res;
  const [mx, my] = centre(mute.box);
  await move(mx, my);
  for (const ms of [0, 150, 400, 1000]) {
    await sleep(ms === 0 ? 30 : ms - (ms === 150 ? 30 : ms === 400 ? 150 : 400));
    res['hover_' + ms] = visible(await shadow(0));
  }
  res.hoverShot = await shot('volume-hover');
  const t1 = await shadow(0);
  const slider = part(t1, '-webkit-media-controls-volume-slider');
  res.slider = slider && { box: slider.box, attrs: slider.attrs, styles: slider.styles };
  if (slider && slider.box && slider.box[2] > 0) {
    // A click a quarter of the way along, then three quarters.
    for (const f of [0.25, 0.75]) {
      const x = slider.box[0] + slider.box[2] * f; const y = slider.box[1] + slider.box[3] / 2;
      await move(x, y); await sleep(100);
      await c.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
      await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
      await sleep(300);
      res['click_' + f] = await ev(`${S} return { st: S.st(S.visibleAudios()[0]), active: S.active() };`);
    }
    res.sliderShot = await shot('volume-slider-set');
    // Arrow keys on the focused slider.
    for (const k of ['ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown', 'Home', 'End']) {
      await c.pressKey(k); await sleep(250);
      res['key_' + k] = await ev(`${S} return S.st(S.visibleAudios()[0]);`);
    }
    // Drag from the left end to the middle.
    const y = slider.box[1] + slider.box[3] / 2;
    await move(slider.box[0] + 2, y);
    await c.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: slider.box[0] + 2, y, button: 'left', clickCount: 1 });
    for (let i = 1; i <= 8; i++) { await c.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: slider.box[0] + 2 + (slider.box[2] / 2 - 2) * i / 8, y, buttons: 1, button: 'left' }); await sleep(30); }
    await c.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: slider.box[0] + slider.box[2] / 2, y, button: 'left', clickCount: 1 });
    await sleep(200);
    res.dragHalf = await ev(`${S} return S.st(S.visibleAudios()[0]);`);
    // Mute with the slider at half: the slider shows 0? then unmute.
    await c.clickAt(mx, my); await sleep(300);
    res.muted = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), slider: (part(await shadow(0), '-webkit-media-controls-volume-slider') || {}).attrs };
    res.mutedShot = await shot('volume-muted');
    await c.clickAt(mx, my); await sleep(300);
    res.unmuted = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), slider: (part(await shadow(0), '-webkit-media-controls-volume-slider') || {}).attrs };
  }
  // The pointer leaves the player: when does the slider close?
  await move(5, 300);
  for (const ms of [0, 200, 500, 1500]) {
    await sleep(ms === 0 ? 30 : ms === 200 ? 170 : ms === 500 ? 300 : 1000);
    const s = part(await shadow(0), '-webkit-media-controls-volume-slider');
    res['leave_' + ms] = s && { box: s.box, styles: s.styles };
  }
  // The pointer over the player but not the button.
  const pan = part(t0, '-webkit-media-controls-panel') || part(t0, '-webkit-media-controls-enclosure');
  if (pan) { await move(pan.box[0] + 20, pan.box[1] + pan.box[3] / 2); await sleep(500); const s = part(await shadow(0), '-webkit-media-controls-volume-slider'); res.overPlay = s && { box: s.box }; }
  await pauseAll();
  return res;
});

// Hover alone (nothing clicked, so nothing in the player has focus): when
// the slider opens over the play button, the mute button and the overflow
// button, and when it closes after the pointer leaves. Then the slider
// against mute: a muted player's slider moved, and the slider taken to 0.
await step('s02b-volume-hover-only', async () => {
  await open('Other.md');
  await open('long.mp3');
  await c.clickAt(400, 600); await sleep(300);
  await move(5, 300); await sleep(600);
  const res = {};
  const t0 = await shadow(0);
  const mute = part(t0, '-webkit-media-controls-mute-button');
  const over = part(t0, '-internal-media-controls-overflow-button');
  const play = part(t0, '-webkit-media-controls-play-button');
  const width = async () => { const s = part(await shadow(0), '-webkit-media-controls-volume-slider'); return s && s.box ? s.box[2] : null; };
  await move(...centre(play.box)); await sleep(500); res.overPlay = await width();
  await move(...centre(over.box)); await sleep(500); res.overOverflow = await width();
  await move(...centre(mute.box));
  const open_ = [];
  for (let i = 0; i < 8; i++) { await sleep(50); open_.push(await width()); }
  res.opening = open_;
  await sleep(400);
  // Leaving to the right (the overflow button), then off the player.
  await move(...centre(over.box)); await sleep(300); res.thenOverflow = await width();
  await move(5, 300);
  const close = [];
  for (let i = 0; i < 12; i++) { await sleep(100); close.push(await width()); }
  res.closing = close;
  res.transition = (part(await shadow(0), '-webkit-media-controls-volume-control-container') || {}).styles;
  // Mute, then move the slider: does it unmute?
  await move(...centre(mute.box)); await sleep(600);
  await c.clickAt(...centre(mute.box)); await sleep(300);
  res.mutedFirst = await ev(`${S} return S.st(S.visibleAudios()[0]);`);
  const s1 = part(await shadow(0), '-webkit-media-controls-volume-slider');
  if (s1 && s1.box && s1.box[2] > 0) {
    const y = s1.box[1] + s1.box[3] / 2;
    await move(s1.box[0] + s1.box[2] * 0.5, y); await sleep(100);
    await c.clickAt(s1.box[0] + s1.box[2] * 0.5, y); await sleep(300);
    res.sliderWhileMuted = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), aria: (part(await shadow(0), '-webkit-media-controls-mute-button') || {}).aria };
    // The slider to the left end: volume 0, muted or not? and the button.
    await c.clickAt(s1.box[0] + 1, y); await sleep(300);
    res.sliderAtZero = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), aria: (part(await shadow(0), '-webkit-media-controls-mute-button') || {}).aria };
    res.zeroShot = await shot('volume-zero');
    // The mute button clicked at volume 0.
    await c.clickAt(...centre(mute.box)); await sleep(300);
    res.muteAtZero = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), aria: (part(await shadow(0), '-webkit-media-controls-mute-button') || {}).aria, slider: (part(await shadow(0), '-webkit-media-controls-volume-slider') || {}).attrs };
    await c.clickAt(...centre(mute.box)); await sleep(300);
    res.muteAtZero2 = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), aria: (part(await shadow(0), '-webkit-media-controls-mute-button') || {}).aria };
  }
  await pauseAll();
  return res;
});

await step('s03-overflow', async () => {
  await open('long.mp3');
  await move(5, 300); await sleep(400);
  const res = {};
  const t0 = await shadow(0);
  const btn = part(t0, '-internal-media-controls-overflow-button');
  res.button = btn && { box: btn.box, aria: btn.aria };
  if (!btn) { res.pseudos = t0.map((n) => n.pseudo).filter(Boolean); return res; }
  await c.clickAt(...centre(btn.box)); await sleep(500);
  const t1 = await shadow(0);
  res.menu = visible(t1);
  res.menuItems = t1.filter((n) => /overflow-menu|playback-speed|menu-list/.test(n.pseudo || '') || n.role === 'menuitem' || n.role === 'menuitemcheckbox' || n.role === 'menuitemradio').map((n) => ({ pseudo: n.pseudo, role: n.role, aria: n.aria, text: n.text, box: n.box, styles: n.styles && { display: n.styles.display, 'font-size': n.styles['font-size'], 'background-color': n.styles['background-color'], color: n.styles.color, 'border-radius': n.styles['border-radius'], padding: n.styles.padding } }));
  res.menuShot = await shot('overflow-menu');
  const speed = t1.find((n) => n.pseudo === '-internal-media-controls-playback-speed-button' || /speed/i.test(n.aria || '') && n.box && n.box[2] > 0);
  res.speedRow = speed && { pseudo: speed.pseudo, aria: speed.aria, box: speed.box };
  if (speed && speed.box && speed.box[2] > 0) {
    await c.clickAt(...centre(speed.box)); await sleep(500);
    const t2 = await shadow(0);
    res.speedList = t2.filter((n) => /playback-speed|menu-list|overflow-menu/.test(n.pseudo || '') || /menuitem/.test(n.role || '')).map((n) => ({ pseudo: n.pseudo, role: n.role, aria: n.aria, text: n.text, box: n.box, checked: n.attrs['aria-checked'], styles: n.styles && { display: n.styles.display, 'font-size': n.styles['font-size'], color: n.styles.color } }));
    res.speedVisible = visible(t2);
    res.speedShot = await shot('speed-list');
    const pick = t2.find((n) => (n.aria === '1.5' || n.text === '1.5') && n.box && n.box[2] > 0) || t2.find((n) => /1\.5/.test((n.aria || '') + (n.text || '')) && n.box && n.box[2] > 0);
    res.pick = pick && { pseudo: pick.pseudo, aria: pick.aria, text: pick.text, box: pick.box };
    if (pick) {
      await c.clickAt(...centre(pick.box)); await sleep(500);
      res.afterPick = { st: await ev(`${S} return S.st(S.visibleAudios()[0]);`), visible: visible(await shadow(0)) };
      // Open again: the menu's row says the speed? the list checks it?
      await c.clickAt(...centre(btn.box)); await sleep(500);
      const t3 = await shadow(0);
      res.menuAgain = visible(t3);
      res.menuAgainShot = await shot('overflow-menu-again');
      const speed2 = t3.find((n) => n.pseudo === '-internal-media-controls-playback-speed-button');
      if (speed2 && speed2.box && speed2.box[2] > 0) {
        await c.clickAt(...centre(speed2.box)); await sleep(500);
        const t4 = await shadow(0);
        res.speedAgain = t4.filter((n) => /menuitem/.test(n.role || '') || /playback-speed/.test(n.pseudo || '')).map((n) => ({ pseudo: n.pseudo, aria: n.aria, text: n.text, checked: n.attrs['aria-checked'], box: n.box }));
        res.speedAgainShot = await shot('speed-list-again');
        await c.pressKey('Escape'); await sleep(400);
        res.afterEscape = visible(await shadow(0)).filter((n) => /menu|speed/.test(n.pseudo));
      }
    }
  }
  // An outside click closes the menu?
  await c.clickAt(...centre(btn.box)); await sleep(400);
  res.openForOutside = visible(await shadow(0)).filter((n) => /menu|speed/.test(n.pseudo)).length;
  await c.clickAt(5, 600); await sleep(400);
  res.afterOutside = visible(await shadow(0)).filter((n) => /menu|speed/.test(n.pseudo));
  // The speed when the file is played after it was set, and if a new tab of the same file starts at 1.
  res.rateKept = await ev(`${S} return S.st(S.visibleAudios()[0]);`);
  await pauseAll();
  return res;
});

await step('s04-space', async () => {
  const res = {};
  const rd = () => ev(`${S} return { st: S.visibleAudios().map((a) => S.st(a)), active: S.active() };`);
  // In the tab: a click on the view's empty space, then Space.
  await open('long.mp3'); await pauseAll();
  const t0 = await shadow(0);
  const play = part(t0, '-webkit-media-controls-play-button');
  const timeline = part(t0, '-webkit-media-controls-timeline');
  const mute = part(t0, '-webkit-media-controls-mute-button');
  res.parts = { play: play && play.box, timeline: timeline && timeline.box, mute: mute && mute.box };
  await c.clickAt(400, 600); await sleep(300);
  res.tab_viewClick = await rd();
  await space(); res.tab_viewClick_space = await rd();
  // A click on the player's panel but not on a button.
  await pauseAll();
  if (timeline) {
    const curr = part(t0, '-webkit-media-controls-current-time-display');
    if (curr) { await c.clickAt(...centre(curr.box)); await sleep(300); res.tab_timeClick = await rd(); await space(); res.tab_timeClick_space = await rd(); await space(); res.tab_timeClick_space2 = await rd(); }
  }
  await pauseAll();
  if (play) {
    await c.clickAt(...centre(play.box)); await sleep(500); res.tab_play = await rd();
    await space(); res.tab_play_space = await rd();
    await space(); res.tab_play_space2 = await rd();
    // Enter on the focused play button, and k.
    await pauseAll();
    await c.pressKey('Enter'); await sleep(400); res.tab_play_enter = await rd();
  }
  await pauseAll();
  if (timeline) {
    await c.clickAt(timeline.box[0] + timeline.box[2] * 0.3, timeline.box[1] + timeline.box[3] / 2); await sleep(400); res.tab_seek = await rd();
    await space(); res.tab_seek_space = await rd();
    await space(); res.tab_seek_space2 = await rd();
    for (const k of ['ArrowRight', 'ArrowLeft']) { await c.pressKey(k); await sleep(300); res['tab_seek_' + k] = await rd(); }
  }
  await pauseAll();
  // Tab key from the view into the player.
  await c.clickAt(400, 600); await sleep(200);
  const tabs = [];
  for (let i = 0; i < 6; i++) { await c.pressKey('Tab'); await sleep(200); tabs.push(await ev(`${S} const e = document.activeElement; return { tag: e && e.tagName, cls: S.cls(e), aria: e && e.getAttribute && e.getAttribute('aria-label') };`)); }
  res.tab_tabOrder = tabs;
  await pauseAll();

  // Reading view: the caret nowhere, a click on a paragraph, Space.
  await open('Embed.md', 'preview'); await sleep(800); await pauseAll();
  const p = await ev(`${S} const p = Array.from(S.root().querySelectorAll('.markdown-preview-view p')).find((x) => /before/.test(x.textContent)); return S.rect(p);`);
  if (p) { await c.clickAt(p[0] + 30, p[1] + p[3] / 2); await sleep(300); res.read_textClick = await rd(); await space(); res.read_textClick_space = await rd(); }
  const r0 = await shadow(0);
  const rplay = part(r0, '-webkit-media-controls-play-button');
  if (rplay) {
    await c.clickAt(...centre(rplay.box)); await sleep(500); res.read_play = await rd();
    await space(); res.read_play_space = await rd();
    res.read_scroll = await ev(`${S} const s = S.root().querySelector('.markdown-preview-view'); return s && s.scrollTop;`);
    if (p) { await c.clickAt(p[0] + 30, p[1] + p[3] / 2); await sleep(300); res.read_textAfter = await rd(); await space(); res.read_textAfter_space = await rd(); }
  }
  await pauseAll();

  // Live Preview: the caret in a line, Space; then a click on play, Space.
  await open('Embed.md', 'live'); await sleep(800); await pauseAll();
  await ev(`${S} const ed = app.workspace.activeEditor.editor; ed.setCursor({ line: 2, ch: 0 }); ed.focus(); return true;`);
  await sleep(300);
  await space();
  res.live_caret_space = { ...(await rd()), line2: await ev(`return app.workspace.activeEditor.editor.getLine(2);`) };
  await ev(`const ed = app.workspace.activeEditor.editor; if (ed.getLine(2).startsWith(' ')) ed.replaceRange('', { line: 2, ch: 0 }, { line: 2, ch: 1 }); return true;`);
  await sleep(300);
  const l0 = await shadow(0);
  const lplay = part(l0, '-webkit-media-controls-play-button');
  if (lplay) {
    await c.clickAt(...centre(lplay.box)); await sleep(500);
    res.live_play = { ...(await rd()), cursor: await ev(`return app.workspace.activeEditor.editor.getCursor();`) };
    await space();
    res.live_play_space = { ...(await rd()), doc: await ev(`return app.workspace.activeEditor.editor.getValue();`) };
    await space();
    res.live_play_space2 = { ...(await rd()), doc: await ev(`return app.workspace.activeEditor.editor.getValue();`) };
  }
  await pauseAll();
  // Restore the note if Space wrote into it.
  await ev(`const ed = app.workspace.activeEditor.editor; return true;`);
  return res;
});

await step('s05-two-embeds', async () => {
  await open('Twice.md', 'preview'); await sleep(1200); await pauseAll();
  const res = {};
  const t = await shadow(0);
  const play = part(t, '-webkit-media-controls-play-button');
  await c.clickAt(...centre(play.box)); await sleep(1200);
  res.reading = await ev(`${S} return S.visibleAudios().map((a) => S.st(a));`);
  res.shot = await shot('two-embeds');
  await pauseAll();
  await open('Twice.md', 'live'); await sleep(1200);
  const t2 = await shadow(1);
  const play2 = part(t2, '-webkit-media-controls-play-button');
  if (play2) { await c.clickAt(...centre(play2.box)); await sleep(1200); res.live = await ev(`${S} return S.visibleAudios().map((a) => S.st(a));`); }
  await pauseAll();
  return res;
});

await step('s06-margins', async () => {
  const res = {};
  await open('Margins.md', 'live'); await sleep(1500);
  await ev(`${S} const ed = app.workspace.activeEditor.editor; ed.setCursor({ line: 0, ch: 0 }); return true;`);
  await sleep(600);
  res.live = await ev(`${S}
    const content = S.root().querySelector('.cm-content');
    const kids = Array.from(content.children);
    const cs = (e, keys) => { const s = getComputedStyle(e); return Object.fromEntries(keys.map((k) => [k, s.getPropertyValue(k)])); };
    return kids.map((e) => ({ tag: e.tagName, cls: S.cls(e), rect: S.rect(e), text: e.classList.contains('cm-line') ? e.textContent.slice(0, 30) : undefined,
      styles: cs(e, ['margin-top', 'margin-bottom', 'padding-top', 'padding-bottom', 'line-height', 'height', 'display']),
      embed: (() => { const m = e.querySelector('.internal-embed'); return m ? { cls: S.cls(m), rect: S.rect(m), styles: cs(m, ['margin-top', 'margin-bottom', 'padding-top', 'padding-bottom', 'display']), audio: (() => { const a = m.querySelector('audio'); return a ? { rect: S.rect(a), styles: cs(a, ['margin-top', 'margin-bottom', 'display', 'vertical-align', 'height']) } : null; })() } : null; })() }));`);
  res.liveShot = await shot('margins-live');
  await open('Margins.md', 'preview'); await sleep(1500);
  res.reading = await ev(`${S}
    const sizer = S.root().querySelector('.markdown-preview-sizer');
    const cs = (e, keys) => { const s = getComputedStyle(e); return Object.fromEntries(keys.map((k) => [k, s.getPropertyValue(k)])); };
    return Array.from(sizer.children).map((d) => ({ cls: S.cls(d), rect: S.rect(d), kids: Array.from(d.children).map((k) => ({ tag: k.tagName, cls: S.cls(k), rect: S.rect(k), text: k.textContent.slice(0, 30), styles: cs(k, ['margin-top', 'margin-bottom', 'line-height']) })) }));`);
  res.readingShot = await shot('margins-reading');
  return res;
});

await step('s07-away', async () => {
  const res = {};
  const playFirst = async () => { const t = await shadow(0); const pl = part(t, '-webkit-media-controls-play-button'); await c.clickAt(...centre(pl.box)); await sleep(800); };
  // Reading view, scrolled to the bottom.
  await open('Long.md', 'preview'); await sleep(1500); await pauseAll();
  await playFirst();
  res.read_playing = await ev(`${S} return S.audios().map((a) => S.st(a));`);
  await ev(`${S} const s = S.root().querySelector('.markdown-preview-view'); s.scrollTop = s.scrollHeight; return true;`);
  await sleep(2500);
  res.read_scrolled = await ev(`${S} return { all: S.audios().map((a) => S.st(a)), inDom: S.root().querySelectorAll('audio').length };`);
  await ev(`${S} const s = S.root().querySelector('.markdown-preview-view'); s.scrollTop = 0; return true;`);
  await sleep(1500);
  res.read_back = await ev(`${S} return { all: S.audios().map((a) => S.st(a)), inDom: S.root().querySelectorAll('audio').length };`);
  await pauseAll();
  // Live Preview, scrolled to the bottom.
  await open('Long.md', 'live'); await sleep(1500);
  await ev(`${S} app.workspace.activeEditor.editor.setCursor({ line: 0, ch: 0 }); return true;`); await sleep(500);
  await playFirst();
  res.live_playing = await ev(`${S} return S.audios().map((a) => S.st(a));`);
  await ev(`${S} const s = S.root().querySelector('.cm-scroller'); s.scrollTop = s.scrollHeight; return true;`);
  await sleep(2500);
  res.live_scrolled = await ev(`${S} return { all: S.audios().map((a) => S.st(a)), inDom: S.root().querySelectorAll('audio').length };`);
  await pauseAll();
  await ev(`${S} const s = S.root().querySelector('.cm-scroller'); s.scrollTop = 0; return true;`);
  // A playing tab, then another tab made active; then the first tab's file replaced.
  await open('long.mp3'); await sleep(800);
  await playFirst();
  await ev(`const l = app.workspace.getLeaf('tab'); await l.openFile(app.vault.getAbstractFileByPath('Other.md')); app.workspace.setActiveLeaf(l, { focus: true }); return true;`);
  await sleep(1500);
  res.tab_background = await ev(`${S} return S.audios().map((a) => S.st(a));`);
  // Switching between reading view and Live Preview with a note's embed playing.
  await open('Embed.md', 'preview'); await sleep(1000); await pauseAll();
  await playFirst();
  await ev(`${S} const l = app.workspace.activeLeaf; const vs = l.getViewState(); vs.state.mode = 'source'; vs.state.source = false; await l.setViewState(vs); return true;`);
  await sleep(1200);
  res.modeSwitch = await ev(`${S} return S.audios().map((a) => ({ ...S.st(a), shown: !!a.offsetParent }));`);
  await pauseAll();
  return res;
});

save();
c.close();
process.exit(0);
