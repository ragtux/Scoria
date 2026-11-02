"""Measure a rock image, so SPEC.md Part 1 can describe a real one.

Runs inside Blender's Python, because that is the interpreter on this machine
with numpy, and because it can decode both PNG and EXR without a third-party
imaging library:

  blender --background --python reference/measure.py -- IMAGE [IMAGE ...]

Every measurement here is defined on an RGBA image with a transparent
background, so the same code measures a Blender render and an Inkscape
rasterisation of a generated SVG. That is the whole point: the numbers the
generator is tuned to and the numbers it is scored on come out of one function.
"""

import argparse
import json
import math
import os
import sys

import numpy as np

import bpy

# Angular resolution of the silhouette profile r(theta). 720 matches the
# crate's own ring sampling, so harmonic indices mean the same thing on both
# sides of the comparison.
NTHETA = 720

# Harmonics reported individually. Beyond ~h40 a rasterised outline is
# measuring its own antialiasing rather than the shape.
NHARM = 48


def load_rgba(path):
    """Decode an image to a top-down float array of shape (H, W, 4)."""
    img = bpy.data.images.load(os.path.abspath(path))
    w, h = img.size
    buf = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(buf)
    bpy.data.images.remove(img)
    # Blender hands back rows bottom-up.
    return buf.reshape(h, w, 4)[::-1]


def silhouette_profile(alpha):
    """`r(theta)` of the opaque region about its centroid, plus the centroid.

    Taken as the LARGEST radius in each angular bin rather than the first
    boundary crossing: a vesicle that breaks the rim, or a speck of stray
    coverage, then cannot cut the profile short.
    """
    h, w = alpha.shape
    mask = alpha > 0.5
    ys, xs = np.nonzero(mask)
    cy, cx = ys.mean(), xs.mean()

    dx, dy = xs - cx, ys - cy
    r = np.hypot(dx, dy)
    th = np.arctan2(dy, dx) % (2.0 * math.pi)
    b = np.minimum((th / (2.0 * math.pi) * NTHETA).astype(np.int64), NTHETA - 1)

    prof = np.zeros(NTHETA)
    np.maximum.at(prof, b, r)
    # An empty bin can only happen on a very small image; fill it from its
    # neighbours so the FFT never sees a zero spike.
    if (prof == 0).any():
        idx = np.nonzero(prof)[0]
        prof = np.interp(np.arange(NTHETA), idx, prof[idx], period=NTHETA)
    return prof, (cx, cy), mask


def harmonics(prof):
    """Normalised harmonic amplitudes of the radial profile.

    `|c_k| / c_0` is scale free, so a 1100px render and a 300px logo are
    directly comparable.
    """
    c = np.fft.rfft(prof) / len(prof)
    amp = np.abs(c)
    return amp[1 : NHARM + 1] / amp[0]


def deellipsed(amp):
    """The spectrum with h1 and h2 zeroed.

    h2 IS the base ellipse and on any elongated rock it carries more energy
    than every other harmonic together -- measured here, 93% of it. Leaving it
    in makes every band fraction a restatement of the aspect ratio and hides
    the lump and crag bands the generator is actually being tuned on. h1 goes
    with it: a pure h1 is the profile's centre being off, not a shape.
    """
    a = amp.copy()
    a[:2] = 0.0
    return a


def equivalent_sigma(amp):
    """The Gaussian smoothing, in DEGREES, whose roll-off matches this spectrum.

    A binomial pass on the ring attenuates harmonic k by about
    `exp(-(k*sigma)^2/2)`, so fitting sigma to the measured decay says directly
    how much band limiting the drawing has -- which is the constant
    `RING_SMOOTH_PASSES` sets. Fitted over h6..h24, below the noise floor and
    above the few harmonics that carry the gross shape.
    """
    k = np.arange(1, len(amp) + 1)
    lo, hi = 6, min(24, len(amp))
    kk, aa = k[lo - 1 : hi], amp[lo - 1 : hi]
    good = aa > 1e-9
    if good.sum() < 4:
        return float("nan")
    # log|c_k| = log A - (k*sigma)^2 / 2, so regress log amplitude on k^2.
    y = np.log(aa[good])
    x = kk[good] ** 2
    slope = np.polyfit(x, y, 1)[0]
    if slope >= 0:
        return 0.0
    sigma_rad = math.sqrt(-2.0 * slope)
    return math.degrees(sigma_rad)


def band_energy(amp):
    """Fraction of the profile's wiggle energy in four harmonic bands."""
    e = amp**2
    tot = e.sum()
    if tot <= 0:
        return {}
    bands = {"h3-h5": (3, 5), "h6-h10": (6, 10), "h11-h18": (11, 18), "h19+": (19, len(amp))}
    return {k: float(e[a - 1 : b].sum() / tot) for k, (a, b) in bands.items()}


def roughness(prof):
    """How hard the outline turns, in the two measures the crate gates on."""
    lp = np.log(prof)
    d = np.abs(np.diff(lp, append=lp[:1])) * (NTHETA / (2.0 * math.pi))
    return {
        "radial_std_rel": float(prof.std() / prof.mean()),
        "dlogr_dtheta_p99": float(np.percentile(d, 99)),
        "dlogr_dtheta_max": float(d.max()),
        "rmin_over_mean": float(prof.min() / prof.mean()),
        "rmax_over_mean": float(prof.max() / prof.mean()),
    }


def bbox_aspect(mask):
    ys, xs = np.nonzero(mask)
    return float((xs.max() - xs.min() + 1) / (ys.max() - ys.min() + 1))


def luminance(rgb):
    return 0.2126 * rgb[..., 0] + 0.7152 * rgb[..., 1] + 0.4126 * rgb[..., 2]


def shading(rgba, mask):
    """How the light actually lands: the histogram the three flat tones quantise.

    `shadow_frac` is the share of the visible face receiving no direct light at
    all. The crate has no cast shadows, so this is the number that says how
    much of the real rock its darkest tone is standing in for.
    """
    lum = luminance(rgba[..., :3])[mask]
    if lum.size == 0:
        return {}
    q = [1, 5, 10, 25, 50, 75, 90, 95, 99]
    out = {f"lum_p{p}": float(np.percentile(lum, p)) for p in q}
    out["lum_mean"] = float(lum.mean())
    out["shadow_frac"] = float((lum < 0.02).mean())
    # Equal-area three-tone split: where the thresholds fall if each tone is to
    # cover a third of the face.
    out["tercile_lo"] = float(np.percentile(lum, 100.0 / 3.0))
    out["tercile_hi"] = float(np.percentile(lum, 200.0 / 3.0))
    return out


def limb_falloff(rgba, mask, prof, centre, nbin=12):
    """Mean brightness against fractional distance from the centre to the rim.

    `t = |p - c| / r(theta_p)`, so t = 1 is the apparent contour whatever the
    shape. A real rock darkens toward the limb because the surface turns away;
    this curve is how much, and it is what a limb treatment in the mark has to
    reproduce.
    """
    h, w = mask.shape
    cx, cy = centre
    ys, xs = np.nonzero(mask)
    dx, dy = xs - cx, ys - cy
    r = np.hypot(dx, dy)
    th = np.arctan2(dy, dx) % (2.0 * math.pi)
    b = np.minimum((th / (2.0 * math.pi) * NTHETA).astype(np.int64), NTHETA - 1)
    t = np.clip(r / np.maximum(prof[b], 1e-9), 0.0, 1.0)

    lum = luminance(rgba[..., :3])[ys, xs]
    edges = np.linspace(0.0, 1.0, nbin + 1)
    idx = np.clip(np.digitize(t, edges) - 1, 0, nbin - 1)

    # Averaged over everything, the curve measures how the cast shadow happens
    # to be placed and stays flat however hard the limb actually darkens. The
    # falloff is a property of the LIT surface, so the mean is taken there and
    # the shadow is reported separately as its own curve.
    lit = lum > 0.02
    means, shadow = [], []
    for i in range(nbin):
        sel = idx == i
        sl = sel & lit
        means.append(float(lum[sl].mean()) if sl.any() else float("nan"))
        shadow.append(float((~lit[sel]).mean()) if sel.any() else float("nan"))
    return {"t_edges": edges.tolist(), "lum_mean": means, "shadow_frac": shadow}


def rim_light(rgba, mask, prof, centre, light_xy, t_lo=0.85, nbin=8):
    """Brightness at the rim, binned by how the rim faces the light.

    The whole-ring limb curve averages the lit side against the shadowed side
    and comes out nearly flat, which reads as "this rock has no limb effect".
    It has a strong one; it is just ANTISYMMETRIC. At the apparent contour the
    surface normal is the outward in-plane perpendicular, so the rim's facing
    is `radial . light`, and brightness should rise with it. This is the
    measurement that says whether a rim light belongs in the mark, how bright,
    and over how much of the ring.
    """
    cx, cy = centre
    ys, xs = np.nonzero(mask)
    dx, dy = xs - cx, ys - cy
    r = np.hypot(dx, dy)
    th = np.arctan2(dy, dx) % (2.0 * math.pi)
    b = np.minimum((th / (2.0 * math.pi) * NTHETA).astype(np.int64), NTHETA - 1)
    t = r / np.maximum(prof[b], 1e-9)

    lum = luminance(rgba[..., :3])[ys, xs]
    # The outward radial direction, which at the rim IS the surface normal.
    rad = np.stack([dx, dy], axis=1) / np.maximum(r, 1e-9)[:, None]
    facing = rad @ np.asarray(light_xy, dtype=float)

    rim = t >= t_lo
    core = t < 0.6
    edges = np.linspace(-1.0, 1.0, nbin + 1)
    idx = np.clip(np.digitize(facing, edges) - 1, 0, nbin - 1)

    out = {"facing_edges": edges.tolist(), "core_lum": float(lum[core].mean()) if core.any() else None}
    means, lit = [], []
    for i in range(nbin):
        sel = rim & (idx == i)
        means.append(float(lum[sel].mean()) if sel.any() else float("nan"))
        lit.append(float((lum[sel] > 0.02).mean()) if sel.any() else float("nan"))
    out["rim_lum"] = means
    out["rim_lit_frac"] = lit
    return out


def vesicles_from_truth(npz_path):
    """Vesicle statistics from the bubbles actually placed, not from pixels.

    Detecting holes in the render would confuse them with the terminator, which
    is just as dark. The placed set is exact, and it is also what the generator
    has to reproduce: a count, a size law, and an elongation that follows the
    surface.
    """
    if not os.path.exists(npz_path):
        return {}
    d = np.load(npz_path)
    c, r = d["centre"], d["radius"]
    n = np.linalg.norm(c, axis=1)
    nz = np.abs(c[:, 2]) / np.maximum(n, 1e-9)

    # A bubble cut by a surface tilted away from the camera projects to an
    # ellipse whose minor axis is foreshortened by the normal's z component.
    # Only the front half is visible.
    # A bubble whose surface is nearly edge-on to the camera projects to a
    # slit of zero width: its "elongation" runs to a hundred and tells you
    # nothing about what is drawable. Only bubbles tilted less than ~78deg
    # from the view are counted, which is the set the mark would ever draw.
    NZ_FLOOR = 0.20
    front = (c[:, 2] > 0) & (nz > NZ_FLOOR)
    elong = 1.0 / np.maximum(nz[front], NZ_FLOOR)

    lr = np.log(r)
    # Power-law exponent by maximum likelihood on a bounded Pareto.
    rmin, rmax = r.min(), r.max()
    alpha = 1.0 + len(r) / max(np.sum(lr - math.log(rmin)), 1e-9)

    return {
        "count_total": int(len(r)),
        "count_front": int(front.sum()),
        "nz_floor": NZ_FLOOR,
        "r_min": float(rmin),
        "r_max": float(rmax),
        "r_median": float(np.median(r)),
        "r_ratio": float(rmax / rmin),
        "pareto_alpha": float(alpha),
        "elong_p50": float(np.percentile(elong, 50)),
        "elong_p90": float(np.percentile(elong, 90)),
        "elong_max": float(elong.max()),
        "nz_p10": float(np.percentile(nz[front], 10)),
        "nz_p50": float(np.percentile(nz[front], 50)),
    }


def measure(path):
    rgba = load_rgba(path)
    alpha = rgba[..., 3]
    prof, centre, mask = silhouette_profile(alpha)
    amp = harmonics(prof)

    de = deellipsed(amp)
    out = {
        "image": os.path.basename(path),
        "size": [int(rgba.shape[1]), int(rgba.shape[0])],
        "coverage": float(mask.mean()),
        "bbox_aspect": bbox_aspect(mask),
        "equiv_sigma_deg": equivalent_sigma(amp),
        "band_energy": band_energy(de),
        "shape_energy": float(np.sqrt((de**2).sum())),
        "harmonics": [float(a) for a in amp[:24]],
    }
    out.update(roughness(prof))
    out["shading"] = shading(rgba, mask)
    out["limb"] = limb_falloff(rgba, mask, prof, centre)

    truth = os.path.join(os.path.dirname(os.path.abspath(path)), "truth.npz")
    if os.path.exists(truth):
        lv = np.load(truth)["light"]
        # SVG y runs down and the render's rows were flipped to match, so the
        # light's y flips coming back the other way.
        out["rim"] = rim_light(rgba, mask, prof, centre, (lv[0], -lv[1]))
    v = vesicles_from_truth(truth)
    if v:
        out["vesicles"] = v
    return out


def report(m):
    L = []
    a = L.append
    a(f"## {m['image']}  ({m['size'][0]}x{m['size'][1]})")
    a("")
    a("### Silhouette")
    a(f"  bbox aspect          {m['bbox_aspect']:.3f}")
    a(f"  radial std / mean    {m['radial_std_rel']:.4f}")
    a(f"  rmin/mean            {m['rmin_over_mean']:.3f}")
    a(f"  rmax/mean            {m['rmax_over_mean']:.3f}")
    a(f"  |dlog r/dtheta| p99  {m['dlogr_dtheta_p99']:.3f}")
    a(f"  equivalent sigma     {m['equiv_sigma_deg']:.2f} deg   <- what the band limiter must be")
    a(f"  shape energy         {m['shape_energy']:.4f}   (rms of h3+, the lump and crag)")
    a("  band energy (h3+)    " + "  ".join(f"{k} {v:.3f}" for k, v in m["band_energy"].items()))
    a("  harmonics h1..h12    " + " ".join(f"{x:.4f}" for x in m["harmonics"][:12]))
    a("  harmonics h13..h24   " + " ".join(f"{x:.4f}" for x in m["harmonics"][12:24]))
    s = m.get("shading", {})
    if s:
        a("")
        a("### Shading")
        a(f"  mean luminance       {s['lum_mean']:.3f}")
        a(f"  pure-shadow fraction {s['shadow_frac']:.3f}")
        a(f"  equal-third splits   {s['tercile_lo']:.3f} / {s['tercile_hi']:.3f}")
        a("  percentiles          " + "  ".join(f"p{p} {s[f'lum_p{p}']:.3f}" for p in (5, 25, 50, 75, 95)))
    lb = m.get("limb", {})
    if lb:
        a("")
        a("### Limb falloff (mean luminance by t = r/r_rim)")
        e, v, sf = lb["t_edges"], lb["lum_mean"], lb["shadow_frac"]
        a("  lit mean   " + "  ".join(f"{(e[i]+e[i+1])/2:.2f}:{v[i]:.3f}" for i in range(len(v))))
        a("  in shadow  " + "  ".join(f"{(e[i]+e[i+1])/2:.2f}:{sf[i]:.3f}" for i in range(len(v))))
    rm = m.get("rim", {})
    if rm:
        a("")
        a("### Rim brightness by facing (radial . light), t >= 0.85")
        e, v, lf = rm["facing_edges"], rm["rim_lum"], rm["rim_lit_frac"]
        a("  mean lum   " + "  ".join(f"{(e[i]+e[i+1])/2:+.2f}:{v[i]:.3f}" for i in range(len(v))))
        a("  lit frac   " + "  ".join(f"{(e[i]+e[i+1])/2:+.2f}:{lf[i]:.3f}" for i in range(len(v))))
        a(f"  core mean  {rm['core_lum']:.3f}  (t < 0.6)")
    ve = m.get("vesicles", {})
    if ve:
        a("")
        a("### Vesicles (as placed)")
        a(f"  count total / drawable  {ve['count_total']} / {ve['count_front']}  (nz > {ve['nz_floor']})")
        a(f"  radius min..max      {ve['r_min']:.4f} .. {ve['r_max']:.4f}  (ratio {ve['r_ratio']:.2f})")
        a(f"  Pareto alpha         {ve['pareto_alpha']:.2f}")
        a(f"  elongation p50/p90   {ve['elong_p50']:.2f} / {ve['elong_p90']:.2f}  (max {ve['elong_max']:.2f})")
        a(f"  normal nz p10/p50    {ve['nz_p10']:.3f} / {ve['nz_p50']:.3f}")
    return "\n".join(L)


def main():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("images", nargs="+")
    ap.add_argument("--json", default=None, help="write all measurements here")
    a = ap.parse_args(argv)

    out = []
    for p in a.images:
        m = measure(p)
        out.append(m)
        print(report(m))
        print()
    if a.json:
        with open(a.json, "w") as f:
            json.dump(out, f, indent=2)
        print(f"[measure] wrote {a.json}")


main()
