"""Deterministic 3D Perlin fBm in numpy.

Shared by the model builder and anything else that needs the same field.
Written out rather than taken from Blender's own textures because the
reference has to be rebuildable byte for byte on another machine, and
Blender's procedural textures move between versions.
"""

import numpy as np

_F3 = 1.0 / 3.0
_G3 = 1.0 / 6.0


def _perm(seed):
    rng = np.random.default_rng(seed)
    p = rng.permutation(256).astype(np.int32)
    return np.concatenate([p, p])


def _fade(t):
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0)


def _grad(h, x, y, z):
    """The 12 edge-midpoint gradients of Perlin's improved noise."""
    h = h & 15
    u = np.where(h < 8, x, y)
    v = np.where(h < 4, y, np.where((h == 12) | (h == 14), x, z))
    return np.where(h & 1, -u, u) + np.where(h & 2, -v, v)


def perlin3(x, y, z, perm):
    xi = np.floor(x).astype(np.int32) & 255
    yi = np.floor(y).astype(np.int32) & 255
    zi = np.floor(z).astype(np.int32) & 255
    xf, yf, zf = x - np.floor(x), y - np.floor(y), z - np.floor(z)
    u, v, w = _fade(xf), _fade(yf), _fade(zf)

    a = perm[xi] + yi
    aa, ab = perm[a] + zi, perm[a + 1] + zi
    b = perm[xi + 1] + yi
    ba, bb = perm[b] + zi, perm[b + 1] + zi

    def lerp(t, p, q):
        return p + t * (q - p)

    x1 = lerp(u, _grad(perm[aa], xf, yf, zf), _grad(perm[ba], xf - 1, yf, zf))
    x2 = lerp(u, _grad(perm[ab], xf, yf - 1, zf), _grad(perm[bb], xf - 1, yf - 1, zf))
    y1 = lerp(v, x1, x2)

    x3 = lerp(u, _grad(perm[aa + 1], xf, yf, zf - 1), _grad(perm[ba + 1], xf - 1, yf, zf - 1))
    x4 = lerp(u, _grad(perm[ab + 1], xf, yf - 1, zf - 1), _grad(perm[bb + 1], xf - 1, yf - 1, zf - 1))
    y2 = lerp(v, x3, x4)

    return lerp(w, y1, y2)


def fbm3(p, perm, octaves=5, freq=1.0, lacunarity=2.0, gain=0.5):
    """Sum `octaves` of Perlin noise. Returns roughly [-1, 1]."""
    total = np.zeros(p.shape[0])
    amp, norm = 1.0, 0.0
    f = freq
    for i in range(octaves):
        # Each octave is offset so the lattices do not share their zero set,
        # which otherwise leaves a visible grid of still points.
        o = 17.3 * i
        total += amp * perlin3(p[:, 0] * f + o, p[:, 1] * f + o, p[:, 2] * f + o, perm)
        norm += amp
        amp *= gain
        f *= lacunarity
    return total / norm
