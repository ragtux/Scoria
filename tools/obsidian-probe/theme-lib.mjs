// The renderer-side half of the theme probes: the CSSOM walk that finds the
// cascade-winning declaration of each painted property, `var()` intact.
// Shared by theme-registry.mjs and theme-plugins.mjs, which prepend it to each
// `evaluate`: a string the renderer runs, not a module it loads.
export const LIB = `
  // D is the document being read: the vault window's, or a popout's (the
  // Settings window in 1.14) when the caller sets __DOC first. Styles come
  // from that document's own window and sheets; instanceof cannot cross
  // windows, so rules are told apart by their fields.
  const D = (typeof __DOC !== 'undefined' && __DOC) || document;
  const W = D.defaultView || window;
  const rect = (el) => { if (!el) return null; const b = el.getBoundingClientRect(); return { x: +b.x.toFixed(2), y: +b.y.toFixed(2), w: +b.width.toFixed(2), h: +b.height.toFixed(2) }; };
  const PROPS = ['color', 'background-color', 'background-image', 'border-color', 'border-top-color', 'border-bottom-color', 'border-left-color', 'border-right-color', 'outline-color', 'fill', 'stroke', 'opacity', 'font-size', 'font-weight', 'font-family', 'line-height', 'box-shadow', 'text-decoration-color', 'caret-color', 'border-radius', 'border-width', 'padding', 'margin', 'width', 'height'];
  const COLORISH = ['color', 'background-color', 'background-image', 'border-color', 'border-top-color', 'border-bottom-color', 'border-left-color', 'border-right-color', 'outline-color', 'fill', 'stroke', 'opacity', 'font-size', 'font-weight', 'line-height', 'box-shadow', 'text-decoration-color', 'caret-color', 'border', 'border-top', 'border-bottom', 'border-left', 'border-right', 'outline', 'background', 'font', 'text-decoration'];
  // Specificity as (ids, classes+attrs+pseudo-classes, elements). :is/:where/:not are approximated by their argument.
  const spec = (sel) => {
    let a = 0, b = 0, c = 0;
    const s = sel.replace(/::?[a-z-]+\\([^)]*\\)/g, (m) => (m.startsWith('::') ? '' : m.startsWith(':where') ? '' : m.slice(m.indexOf('(') + 1, -1)));
    const t = s.replace(/\\[[^\\]]*\\]/g, () => { b++; return ' '; }).replace(/::[a-z-]+/g, () => { c++; return ' '; }).replace(/:[a-z-]+/g, () => { b++; return ' '; }).replace(/#[\\w-]+/g, () => { a++; return ' '; }).replace(/\\.[\\w-]+/g, () => { b++; return ' '; });
    c += (t.match(/(^|[\\s>+~(,])[a-zA-Z][\\w-]*/g) || []).length;
    return [a, b, c];
  };
  const cmp = (x, y) => { for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i]; return 0; };
  // Every style rule in document order, media queries that match included, with its sheet's href.
  let RULES = null;
  const allRules = () => {
    if (RULES) return RULES;
    RULES = [];
    let order = 0;
    const walk = (list, sheet) => { for (const r of Array.from(list)) {
      if ('selectorText' in r && r.style) RULES.push({ r, sel: r.selectorText, order: order++, sheet });
      else if ('conditionText' in r && r.cssRules && r.constructor.name === 'CSSMediaRule') { if (W.matchMedia(r.conditionText).matches) walk(r.cssRules, sheet); }
      else if (r.cssRules) walk(r.cssRules, sheet);
    } };
    for (const s of Array.from(D.styleSheets)) { let rules; try { rules = s.cssRules; } catch { continue; } walk(rules, s.href || (s.ownerNode && s.ownerNode.id) || 'inline'); }
    return RULES;
  };
  const matches = (el, sel) => { try { return el.matches(sel); } catch { return false; } };
  // Matching declarations for an element, per property: the winner by importance, specificity then order.
  const winners = (el, props) => {
    const best = {};
    const sels = [];
    for (const { r, sel, order, sheet } of allRules()) {
      // A selector list: the specificity is that of the matching complex selector.
      const parts = sel.split(/,(?![^(]*\\))/).map((p) => p.trim());
      let hit = null;
      for (const p of parts) if (matches(el, p)) { const sp = spec(p); if (!hit || cmp(sp, hit.sp) > 0) hit = { p, sp }; }
      if (!hit) continue;
      sels.push(hit.p);
      for (const prop of props) {
        const v = r.style.getPropertyValue(prop); if (!v) continue;
        const imp = r.style.getPropertyPriority(prop) === 'important' ? 1 : 0;
        const cand = { prop, selector: hit.p, value: v.trim(), important: !!imp, spec: hit.sp, order, sheet: String(sheet).slice(-40) };
        const cur = best[prop];
        if (!cur || imp > (cur.important ? 1 : 0) || (imp === (cur.important ? 1 : 0) && (cmp(hit.sp, cur.spec) > 0 || (cmp(hit.sp, cur.spec) === 0 && order > cur.order)))) best[prop] = cand;
      }
    }
    return { best, sels: Array.from(new Set(sels)).slice(0, 40) };
  };
  const describe = (el, { props = COLORISH, computed = PROPS } = {}) => {
    if (!el) return null;
    const cs = W.getComputedStyle(el);
    const comp = {}; for (const p of computed) comp[p] = cs.getPropertyValue(p);
    const { best, sels } = winners(el, props);
    const own = {}; for (const k of Object.keys(best)) { const b = best[k]; own[k] = { from: b.selector, value: b.value, important: b.important || undefined, sheet: b.sheet }; }
    // Where an inherited colour comes from.
    let inherited = null;
    if (!best.color) { let p = el.parentElement; let depth = 1; while (p) { const w = winners(p, ['color']).best.color; if (w) { inherited = { depth, element: p.tagName.toLowerCase() + (p.className && typeof p.className === 'string' ? '.' + p.className.trim().split(/\\s+/).slice(0, 4).join('.') : ''), from: w.selector, value: w.value }; break; } p = p.parentElement; depth++; } }
    return { tag: el.tagName.toLowerCase(), cls: typeof el.className === 'string' ? el.className.trim().slice(0, 160) : '', rect: rect(el), computed: comp, own, inherited, matched: sels };
  };
  const q = (sel, i = 0) => { const els = D.querySelectorAll(sel); return els[i] || null; };
  const html = (sel, n = 1200) => { const el = D.querySelector(sel); return el ? el.outerHTML.slice(0, n) : null; };
`;
