"""Build and render a scoria chunk in Blender, as ground truth to measure.

This is a MEASURING INSTRUMENT, not part of the generator. Nothing the Rust
crate does depends on Blender being installed; this script exists to produce a
render whose silhouette, vesicles and shading can be measured, so that the
constants in SPEC.md Part 1 describe a real three-dimensional rock instead of a
flat drawing.

Run:
  blender --background --python reference/model.py -- --seed 7 --out out/ref

Outputs, into `--out`:
  colour.png    the Lambert render, lit the way the crate lights its solid
  passes.exr    multilayer: Combined, Depth, Normal
  truth.npz     the vesicles actually placed, and the camera mapping, so the
                measurement script can score what it recovers against them
"""

import argparse
import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import noise  # noqa: E402

import bmesh  # noqa: E402
import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402

# The crate's own light, from Params::default(). `light_dir` is an angle in SVG
# coordinates, where y points DOWN, so the sign flips coming into Blender.
LIGHT_DIR = 2.30
LIGHT_ELEV = 0.22

# The crate's base ellipsoid: in-plane aspect, and the depth axis as a fraction
# of the short in-plane one. Params::default() aspect 1.50, depth 0.72.
ASPECT = 1.50
DEPTH = 0.72


def clear_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def ico(subdiv, radius=1.0):
    me = bpy.data.meshes.new("rock")
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=subdiv, radius=radius)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new("rock", me)
    bpy.context.collection.objects.link(obj)
    return obj


def verts_of(obj):
    me = obj.data
    a = np.empty(len(me.vertices) * 3, dtype=np.float64)
    me.vertices.foreach_get("co", a)
    return a.reshape(-1, 3)


def set_verts(obj, p):
    obj.data.vertices.foreach_set("co", p.ravel().astype(np.float32))
    obj.data.update()


def displace(obj, seed, lump_amp, lump_freq, crag_amp, crag_freq, ridged):
    """Push every vertex along its own radius by a two-band fBm.

    The low band is the rock's overall lumpiness; the high band is the crag
    detail that the current generator's band limiter removes. They are kept
    separate so each can be measured and tuned on its own.
    """
    p = verts_of(obj)
    d = p / np.linalg.norm(p, axis=1, keepdims=True)

    perm_lump = noise._perm(seed)
    perm_crag = noise._perm(seed + 1009)

    lump = noise.fbm3(d * lump_freq, perm_lump, octaves=4, gain=0.5)

    crag = noise.fbm3(d * crag_freq, perm_crag, octaves=4, gain=0.6)
    if ridged:
        # Scoria fractures along sharp arrises rather than rolling dunes.
        # Folding the noise at zero turns smooth extrema into creases, which
        # is the feature the reference drawing's outline is full of.
        crag = 1.0 - 2.0 * np.abs(crag)

    r = 1.0 + lump_amp * lump + crag_amp * crag
    set_verts(obj, d * r[:, None])


def ellipsoid(obj, aspect, depth):
    a = math.sqrt(aspect)
    b = 1.0 / math.sqrt(aspect)
    p = verts_of(obj)
    p[:, 0] *= a
    p[:, 1] *= b
    p[:, 2] *= b * depth
    set_verts(obj, p)


def vesicle_centres(obj, seed, count, rmin, rmax, power, embed):
    """Place gas bubbles against the surface, radii drawn from a power law.

    A real vesicle is a frozen bubble that the fracture surface cuts through,
    so each is a sphere sunk `embed` of its own radius below the surface: that
    is what leaves a sharp circular rim rather than a soft dimple.
    """
    rng = np.random.default_rng(seed + 7717)
    p = verts_of(obj)
    n = p.shape[0]

    u = rng.random(count)
    # Inverse-CDF of p(r) ~ r^-power over [rmin, rmax].
    if abs(power - 1.0) < 1e-9:
        r = rmin * (rmax / rmin) ** u
    else:
        k = 1.0 - power
        r = (rmin**k + u * (rmax**k - rmin**k)) ** (1.0 / k)

    idx = rng.choice(n, size=count, replace=False)
    base = p[idx]
    nrm = base / np.linalg.norm(base, axis=1, keepdims=True)
    centre = base - nrm * (r[:, None] * (1.0 - embed))
    return centre, r


def cut_vesicles(obj, centre, radius, subdiv=3):
    """One joined cutter mesh, one boolean. Ninety separate booleans on a
    ten-thousand-vertex rock is minutes; this is seconds.

    The spheres accumulate into a single bmesh through `create_icosphere`'s own
    `matrix`, rather than being built one at a time and merged through a Mesh:
    round-tripping each sphere through `to_mesh` overwrites the mesh instead of
    appending to it, and leaves a cutter that swallows the whole rock.
    """
    bm = bmesh.new()
    for c, r in zip(centre, radius):
        bmesh.ops.create_icosphere(
            bm,
            subdivisions=subdiv,
            radius=float(r),
            matrix=Matrix.Translation((float(c[0]), float(c[1]), float(c[2]))),
        )
    me = bpy.data.meshes.new("cutter")
    bm.to_mesh(me)
    bm.free()

    cutter = bpy.data.objects.new("cutter", me)
    bpy.context.collection.objects.link(cutter)

    m = obj.modifiers.new("vesicles", "BOOLEAN")
    m.operation = "DIFFERENCE"
    # MANIFOLD, not EXACT. The cutter is a union of spheres that deliberately
    # overlap -- neighbouring bubbles intersect in real scoria -- and EXACT
    # returns an EMPTY mesh on a self-intersecting cutter rather than an error.
    m.solver = "MANIFOLD"
    m.object = cutter

    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=m.name)
    bpy.data.objects.remove(cutter, do_unlink=True)


def shade_smooth(obj, angle_deg=38.0):
    """Smooth the surface but keep the vesicle rims sharp.

    The boolean leaves the bubble walls meeting the outer surface at a hard
    arris, which is exactly the edge a vesicle reads by. Smoothing everything
    rounds those away and the holes turn into dimples; smoothing only below an
    angle keeps them.
    """
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(angle_deg))


def light_vector(angle=LIGHT_DIR, elev=LIGHT_ELEV):
    s, c = math.sin(angle), math.cos(angle)
    # SVG y is down; Blender y is up.
    v = Vector((c, -s, elev))
    v.normalize()
    return v


def setup_scene(obj, res_x, res_y, samples, light_angle, light_elev):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = samples
    # Direct light only: the crate shades with a bare Lambert term, so bounce
    # light here would be a difference in the instrument, not in the rock.
    scene.cycles.max_bounces = 0
    scene.cycles.use_denoising = False
    scene.render.resolution_x = res_x
    scene.render.resolution_y = res_y
    scene.render.film_transparent = True

    mat = bpy.data.materials.new("rock")
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    bsdf = nt.nodes.new("ShaderNodeBsdfDiffuse")
    bsdf.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    obj.data.materials.append(mat)

    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    # Fit the long axis with a margin, mirroring the crate's canvas padding.
    cam_data.ortho_scale = 2.0 * math.sqrt(ASPECT) * 1.30
    cam = bpy.data.objects.new("cam", cam_data)
    cam.location = (0.0, 0.0, 10.0)
    cam.rotation_euler = (0.0, 0.0, 0.0)
    bpy.context.collection.objects.link(cam)
    scene.camera = cam

    sun_data = bpy.data.lights.new("sun", type="SUN")
    sun_data.energy = math.pi
    sun_data.angle = 0.0
    sun = bpy.data.objects.new("sun", sun_data)
    d = light_vector(light_angle, light_elev)
    sun.rotation_euler = Vector((0.0, 0.0, 1.0)).rotation_difference(d).to_euler()
    sun.location = d * 10.0
    bpy.context.collection.objects.link(sun)

    vl = bpy.context.view_layer
    vl.use_pass_z = True
    vl.use_pass_normal = True
    return cam


def render(out_dir):
    """Render twice: a PNG to look at, and a multilayer EXR to measure.

    Setting the output format to multilayer EXR makes Blender write every
    enabled view-layer pass on its own, which is why there is no compositor
    here -- the compositor's Python API moved in 5.0 and this needs none of it.
    """
    scene = bpy.context.scene
    os.makedirs(out_dir, exist_ok=True)

    scene.render.filepath = os.path.join(out_dir, "colour.png")
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    bpy.ops.render.render(write_still=True)

    scene.render.filepath = os.path.join(out_dir, "passes.exr")
    # `media_type` gates the format enum in 5.x: the multilayer EXR entry is
    # not selectable until the output is declared a multi-layer image.
    scene.render.image_settings.media_type = "MULTI_LAYER_IMAGE"
    scene.render.image_settings.file_format = "OPEN_EXR_MULTILAYER"
    scene.render.image_settings.color_depth = "32"
    bpy.ops.render.render(write_still=True)


def main():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--out", default="out/ref")
    ap.add_argument("--subdiv", type=int, default=7)
    ap.add_argument("--lump-amp", type=float, default=0.26)
    ap.add_argument("--lump-freq", type=float, default=1.7)
    ap.add_argument("--crag-amp", type=float, default=0.090)
    ap.add_argument("--crag-freq", type=float, default=5.0)
    ap.add_argument("--ridged", type=int, default=1)
    ap.add_argument("--vesicles", type=int, default=160)
    ap.add_argument("--vmin", type=float, default=0.035)
    ap.add_argument("--vmax", type=float, default=0.200)
    ap.add_argument("--vpow", type=float, default=2.2)
    ap.add_argument("--embed", type=float, default=0.45)
    ap.add_argument("--res", type=int, default=1400)
    ap.add_argument("--samples", type=int, default=64)
    ap.add_argument("--light-dir", type=float, default=LIGHT_DIR)
    ap.add_argument("--light-elev", type=float, default=LIGHT_ELEV)
    ap.add_argument("--smooth-angle", type=float, default=38.0)
    a = ap.parse_args(argv)

    clear_scene()
    obj = ico(a.subdiv)
    displace(obj, a.seed, a.lump_amp, a.lump_freq, a.crag_amp, a.crag_freq, bool(a.ridged))
    ellipsoid(obj, ASPECT, DEPTH)

    centre, radius = vesicle_centres(obj, a.seed, a.vesicles, a.vmin, a.vmax, a.vpow, a.embed)
    cut_vesicles(obj, centre, radius)
    shade_smooth(obj, a.smooth_angle)

    res_y = int(round(a.res / ASPECT))
    cam = setup_scene(obj, a.res, res_y, a.samples, a.light_dir, a.light_elev)
    render(a.out)

    ortho = cam.data.ortho_scale
    np.savez(
        os.path.join(a.out, "truth.npz"),
        centre=centre,
        radius=radius,
        ortho_scale=ortho,
        res=np.array([a.res, res_y]),
        aspect=ASPECT,
        depth=DEPTH,
        light=np.array(list(light_vector(a.light_dir, a.light_elev))),
        params=np.array([a.lump_amp, a.lump_freq, a.crag_amp, a.crag_freq, a.vpow]),
    )
    print(f"[model] {len(obj.data.vertices)} verts, {a.vesicles} vesicles -> {a.out}")


main()
