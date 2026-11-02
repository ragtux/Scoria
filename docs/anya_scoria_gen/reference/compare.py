"""Turn measurements into targets, and score a generated mark against them.

Standard library only, on purpose: the measuring half needs Blender to decode
images, but the scoring half is the part a test runs, and it should not need a
3D package installed to say whether the logo is in range.

  # build targets from a set of reference rocks
  python3 reference/compare.py aggregate ref.json -o reference/targets.json

  # score generated marks against them
  python3 reference/compare.py check gen.json -t reference/targets.json
"""

import argparse
import json
import statistics
import sys

# The metrics the silhouette is scored on, and how each is read.
#
# Every one is scale free, so a 1000px render and a 300px logo compare
# directly. The band energies are the heart of it: they say WHERE in the
# spectrum the outline keeps its detail, which is exactly the difference
# between a rock and a blob with four lumps.
METRICS = [
    ("bbox_aspect", "bbox aspect"),
    ("radial_std_rel", "radial std / mean"),
    ("shape_energy", "shape energy (h3+)"),
    ("equiv_sigma_deg", "equivalent sigma, deg"),
    ("band_energy.h3-h5", "band h3-h5"),
    ("band_energy.h6-h10", "band h6-h10"),
    ("band_energy.h11-h18", "band h11-h18"),
    ("band_energy.h19+", "band h19+"),
]

# How far outside the reference rocks' own spread a mark may sit before the
# gate complains. The reference set is small, so a hard min/max would be
# brittle and every new rock would move the gate.
#
# The slack is taken from each side's OWN distance to the median, not from the
# full width. Padding by a fraction of the full width is what a first cut does
# and it is wrong here: on a metric whose reference range spans an order of
# magnitude -- `band_energy.h11-h18` runs 0.033 to 0.296 -- half the width is
# larger than the minimum, the lower bound goes NEGATIVE, and the gate can
# never fail low. Those high-frequency bands are the whole point of the
# comparison, so a bound that cannot catch a mark fifty times under the
# reference is worse than no bound at all. Clamped at zero because every
# metric here is a non-negative quantity.
SLACK = 0.5


def dig(d, path):
    for k in path.split("."):
        if not isinstance(d, dict) or k not in d:
            return None
        d = d[k]
    return d


def aggregate(measurements):
    out = {"n": len(measurements), "metrics": {}}
    for key, _ in METRICS:
        vals = [v for v in (dig(m, key) for m in measurements) if isinstance(v, (int, float))]
        if not vals:
            continue
        lo, hi = min(vals), max(vals)
        med = statistics.median(vals)
        out["metrics"][key] = {
            "median": med,
            "min": lo,
            "max": hi,
            "lo": max(0.0, lo - SLACK * (med - lo)),
            "hi": hi + SLACK * (hi - med),
            "n": len(vals),
        }
    return out


def check(measurements, targets):
    rows, failed = [], 0
    for m in measurements:
        name = m.get("image", "?")
        for key, label in METRICS:
            t = targets["metrics"].get(key)
            v = dig(m, key)
            if t is None or v is None:
                continue
            ok = t["lo"] <= v <= t["hi"]
            # Reported as a RATIO to the nearest reference bound rather than
            # in units of the spread. On the high-frequency bands the spread
            # is itself an order of magnitude, so "0.1x the spread outside"
            # reads as a rounding error when the mark is three times under the
            # quietest reference rock. The ratio says what is actually wrong.
            if v < t["lo"]:
                off = (t["min"] / v) if v > 0 else float("inf")
                how = "under"
            elif v > t["hi"]:
                off = v / t["max"] if t["max"] > 0 else float("inf")
                how = "over"
            else:
                off, how = 0.0, ""
            rows.append((name, label, v, t, ok, (off, how)))
            failed += 0 if ok else 1
    return rows, failed


def print_rows(rows, targets):
    cur = None
    for name, label, v, t, ok, (off, how) in rows:
        if name != cur:
            cur = name
            print(f"\n{name}   (targets from {targets['n']} reference rocks)")
            print(f"  {'metric':<24}{'value':>9}{'pass band':>18}{'rocks min..max':>18}")
        mark = "ok " if ok else "OUT"
        # Both bands: the one the verdict is taken against, and the rocks'
        # own spread it was padded out from. Printing only the raw spread
        # made a value outside it that still read "ok" look like a bug.
        band = f"{t['lo']:.4g}..{t['hi']:.4g}"
        raw = f"{t['min']:.4g}..{t['max']:.4g}"
        extra = "" if ok else f"  {off:.1f}x {how} the rocks"
        print(f"  {label:<24}{v:>9.4g}{band:>18}{raw:>18}  {mark}{extra}")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)

    a = sub.add_parser("aggregate")
    a.add_argument("measurements")
    a.add_argument("-o", "--out", required=True)

    c = sub.add_parser("check")
    c.add_argument("measurements")
    c.add_argument("-t", "--targets", required=True)
    c.add_argument("--strict", action="store_true", help="exit 1 if anything is out of range")

    args = ap.parse_args()
    data = json.load(open(args.measurements))

    if args.cmd == "aggregate":
        t = aggregate(data)
        with open(args.out, "w") as f:
            json.dump(t, f, indent=2)
        print(f"[targets] {t['n']} rocks -> {args.out}")
        for k, v in t["metrics"].items():
            print(f"  {k:<22} median {v['median']:.4g}   range {v['min']:.4g}..{v['max']:.4g}")
        return

    targets = json.load(open(args.targets))
    rows, failed = check(data, targets)
    print_rows(rows, targets)
    print(f"\n{failed} metric(s) out of range across {len(data)} mark(s).")
    if failed and args.strict:
        sys.exit(1)


main()
