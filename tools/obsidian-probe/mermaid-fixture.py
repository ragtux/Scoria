#!/usr/bin/env python3
"""Reduce a mermaid.mjs result to the fixture scoria-mermaid's tests read.

    python3 tools/obsidian-probe/mermaid-fixture.py \
        tools/obsidian-probe/results/2026-11-15-mermaid.json \
        crates/scoria-mermaid/tests/data/obsidian-1.15.1.json

Keeps, per theme and case: the svg's viewBox size, and every drawn element's
tag, group class, box (in the svg's units), paint and text; or the error
message Obsidian showed instead.
"""
import json
import re
import sys


def rgb(v):
    if not v or v == 'none':
        return None
    m = re.match(r'rgba?\(([\d.]+), ([\d.]+), ([\d.]+)(?:, ([\d.]+))?\)', v)
    if not m:
        return v
    r, g, b = (int(float(m.group(i))) for i in (1, 2, 3))
    a = float(m.group(4)) if m.group(4) else 1.0
    if a == 0:
        return None
    s = '#%02x%02x%02x' % (r, g, b)
    return s if a == 1 else s + '%02x' % round(a * 255)


def main(src, dst):
    d = json.load(open(src))
    out = {'title': d['steps']['s00'].get('title'), 'cases': d['cases'], 'themes': {}}
    for theme, cases in d['steps']['s01'].items():
        if theme in ('ok', 'ms'):
            continue
        tv = out['themes'][theme] = {}
        for name, v in cases.items():
            b = v.get('block') or {}
            if b.get('error'):
                tv[name] = {'error': b['error']['text']}
                continue
            svg = b.get('svg')
            if not svg:
                tv[name] = None
                continue
            vb = [float(x) for x in svg['viewBox'].split()]
            items = []
            for e in b['draw']:
                it = {'tag': e['tag'], 'g': e.get('g'), 'box': [e['x'], e['y'], e['w'], e['h']]}
                f, s = rgb(e.get('fill')), rgb(e.get('stroke'))
                if f:
                    it['fill'] = f
                if s:
                    it['stroke'] = s
                if e.get('text') is not None:
                    it['text'] = e['text']
                items.append(it)
            tv[name] = {'size': vb[2:], 'items': items}
    json.dump(out, open(dst, 'w'), indent=0, ensure_ascii=False)
    open(dst, 'a').write('\n')


if __name__ == '__main__':
    main(*sys.argv[1:3])
