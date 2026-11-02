# The reference rock

A measuring instrument, not part of the generator. Nothing in `anya_scoria_gen`
depends on anything in this directory: the crate still builds and runs with no
dependencies, still produces byte-identical SVGs, and still needs only `cargo`.

## Why it exists

Every constant in SPEC.md Part 1 was measured off Anya's drawing, which is a
flat cartoon. The generator reproduces it faithfully, and that is the problem:
asked to look like a three-dimensional rock, there was nothing three-dimensional
to aim at. This builds one — a real scoria chunk in Blender, lit the way the
crate lights its solid — and measures it, so Part 1 can describe geology.

## The three pieces

| file | runs in | does |
|---|---|---|
| `model.py` | Blender | displaces an icosphere, cuts gas bubbles out of it, renders it orthographic |
| `measure.py` | Blender | turns an RGBA image into scale-free numbers: silhouette spectrum, shading, limb falloff |
| `compare.py` | python3 | aggregates measurements into `targets.json`, and scores marks against it |

`measure.py` runs in Blender only because that is the interpreter here with
numpy, and because it decodes PNG and EXR without an imaging library. It takes
any RGBA image with a transparent background, so the SAME code measures a
Blender render and an Inkscape rasterisation of a generated SVG. That is the
point: the numbers the generator is tuned to and the numbers it is scored on
come out of one function.

## Running it

```sh
reference/run.sh rocks 6          # re-render the reference set, re-aggregate targets.json
reference/run.sh check out/*.svg  # rasterise, measure and score generated marks
STRICT=1 reference/run.sh check out/*.svg   # exit 1 if anything is out of range
```

`targets.json` is checked in, so `check` works without Blender ever being run
again. Re-run `rocks` only to deliberately move the target, and say in the
commit message what moved and why.

## What it measures, and why those things

The silhouette is reduced to `r(theta)` about its centroid and Fourier
transformed. Everything reported is a RATIO, so a 1000px render and a 300px
logo compare directly.

- **`band_energy`** — where in the spectrum the outline keeps its detail, with
  h1 and h2 removed. This is the heart of it. h2 IS the base ellipse and on an
  elongated rock it carries 93% of the energy; leaving it in makes every band
  fraction a restatement of the aspect ratio.
- **`equiv_sigma_deg`** — the Gaussian roll-off that fits the measured decay,
  in degrees. Directly comparable to what `RING_SMOOTH_PASSES` sets.
- **`shape_energy`** — total outline wiggle, h3 and up.
- **`shading`** — the brightness histogram the three flat tones quantise, and
  `shadow_frac`, the share of the face getting no direct light at all.
- **`limb`** — brightness against fractional distance to the rim, measured on
  LIT pixels only; averaged over everything it measures where the cast shadow
  fell and stays flat however hard the limb darkens.
- **`vesicles`** — taken from the bubbles as PLACED, not detected in pixels: a
  hole and the terminator are equally dark and cannot be told apart downstream.

## Traps worth knowing

- **`inkscape -b none` leaves the background OPAQUE.** Use `-y 0`. With an
  opaque background every measurement describes the image rectangle, which is
  perfectly point-symmetric, so the odd harmonics come out at 1e-18 and look
  like a dramatic finding about the generator. They are not.
- **The boolean solver must be `MANIFOLD`.** The cutter is a union of spheres
  that deliberately overlap, because neighbouring bubbles intersect in real
  scoria. `EXACT` returns an EMPTY MESH on a self-intersecting cutter rather
  than an error, and the render comes out blank.
- **Smooth by angle, not `shade_smooth`.** The boolean leaves the bubble walls
  meeting the outer surface at a hard arris, which is the edge a vesicle reads
  by. Smoothing everything turns the holes into dimples.
- **Multilayer EXR needs `image_settings.media_type = "MULTI_LAYER_IMAGE"`**
  before the format enum will accept `OPEN_EXR_MULTILAYER`. There is no
  compositor here on purpose; its Python API moved in Blender 5.0.

Built against Blender 5.1.1.
