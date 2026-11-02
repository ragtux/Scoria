# anya-scoria-gen

Procedurally generated scoria logos: a bumpy 3D rock, drawn flat in four
inks - packed gas holes, three-tone faceting, a heavy outline and one thin
relief mark - out as SVG. Zero dependencies, deterministic to the byte.

![the mark this aims at](../img/anya_v0_base.svg)

## What it is

Anya drew the scoria mark by hand: a lumpy volcanic chunk with a heavy dark
outline, flat three-tone orange faceting and seven gas holes. That drawing,
`../img/anya_v0_base.svg`, was measured path by path - silhouette spectrum,
ribbon width, hole sizes and orientations, facet tiling, the exact three inks -
and this crate is the generator those measurements specify, in the spirit of
Tyler Hobbs's *Fidenza*.

It exists so the mark can move: a seed per release, per page, per sticker
sheet, all unmistakably the same hand. One `u64` in, one logo out, the same
bytes on every machine.

**There is a real rock underneath.** Each seed builds a 3D solid - an
ellipsoid displaced along its radii by noise sampled on the sphere - and
everything you see is derived from it in orthographic projection: the
silhouette is its apparent contour, the three flat tones are a quantisation
of its actual surface shading, the facet cuts lean along its iso-brightness
lines, and the thin relief mark is traced down a real crease of it. The
flatness is a rendering choice applied to real geometry, which is what lets
the tones describe the shape rather than merely sit on it.

**[`SPEC.md`](SPEC.md) is the authority.** Part 1 is the measurements, Part 2
the algorithm with its constants, quality gates and named pitfalls, Part 3
what building it proved wrong. Where this README and the spec disagree, the
spec wins; where the spec and the artwork disagree, measure the artwork
again.

## Quickstart

```sh
cargo run --release -- --seed 7 > rock.svg          # one logo to stdout
cargo run --release -- --preset feral --out rock.svg
cargo run --release -- --count 500 --out ./batch --time
cargo run --release -- --list-params
```

As a library:

```rust
use anya_scoria_gen::{generate, Params};

let svg = generate(&Params { seed: 42, ..Params::anya() }).expect("a clean logo");
```

Presets: `anya` (the drawing; the CLI default), `tame` (corporate: smooth, few
holes, nearly straight cuts), `feral` (deep lumps, many holes, cuts that visibly
curve). `Params::default()` is the spec's parameter table verbatim, which is
`anya` but with a 4% margin and no second rim - the drawing itself bleeds to the
canvas edge.

One dial drives seven knobs together, for callers who do not want to learn the
other forty:

```rust
let mut p = anya_scoria_gen::Params::default();
p.set("tameness", "0.8").unwrap();
p.apply_tameness();   // only touches knobs you did not set yourself
```

## Design, in five lines

- **The silhouette is a 3D object's outline, and still polar.** It is
  `max over u of R(dir(theta, u)) * cos u`, the apparent contour of the
  solid. That is still a single-valued `r(theta) > 0`, so it is still
  star-shaped about its centre and therefore still cannot self-intersect -
  the theorem the whole crate rests on survives the move to 3D, because the
  projection of a star-shaped body is star-shaped. It is what turns the
  outline ribbon from a polygon-offset problem into the subtraction
  `r - w(theta)`, and now a near-side bump genuinely pokes out past the base
  ellipse, which a 2D radial profile cannot do.
- **The shading is the solid's own.** A depth map of the front surface -
  built by marching the parameterisation from rim to pole, not by inverting
  anything - gives a real normal under any image point. A facet's tone is
  the Lambert brightness of that normal, quantised by area quantile into the
  measured 30/60/10 split. So two facets on the same plane of the rock get
  the same tone however far apart they are, and two across a fold get
  different ones however close.
- **One flow field does three jobs.** It warps the silhouette's noise lookup,
  orients each gas hole's long axis where the surface has no opinion, and
  sets the facet cuts' heading. That shared origin is why a seed reads as
  one rock rather than three unrelated effects.
- **The holes are a packing, not a scatter.** A weighted Voronoi diagram is
  relaxed over the body, so they spread evenly without sitting on a grid;
  the weights are the drawn sizes, so the measured size hierarchy survives
  the relaxation; and the largest size goes to the darkest seed, so the
  holes shade the form rather than being sprinkled over it.
- **Facets are cut, not placed.** Recursive bisection always splits the largest
  remaining region; children copy the parent's `f64` vertices and share the cut
  array bit for bit, so adjacent facets have literally the same edge and no
  antialiased hairline can appear between them. Tones are assigned by area
  quantile, never by threshold, so all three always appear.
- **Determinism is a hard requirement.** Nothing calls `sin`, `cos`, `atan2`,
  `powf`, `exp` or `hypot` from libm - those differ in the last ulp between
  glibc, musl, MSVC and Apple, and one ulp propagates into a different third
  decimal in the output. Nothing iterates a `HashMap`, whose order changes
  between runs. Sorting is `total_cmp` with an index tie-break. f64 throughout.

Quality gates run before anything is written: star-shapedness, silhouette
amplitude and bounding box, ribbon evenness and neck width, facet area and
sliver angles, all three tones present AND in the order the solid's own
shading puts them, the depth map facing the right way, relief marks of the
right width, vesicle clearance, coverage and size spread, no
self-intersection, nothing non-finite.
A failure re-rolls the whole logo from a fresh split of the seed; on exhaustion
the generator returns `Err` naming the gate that kept failing, rather than
emitting known-bad art.

## Where this departs from SPEC.md

The spec is the contract and most of the defaults below are its table
verbatim. Each departure below was measured rather than guessed, and the code
carries the reasoning at the point of use. SPEC.md Part 3 has the full list,
including everything the 3D stage proved wrong; these are the ones a reader
of the parameter table most needs to know about.

Four of them are places where SPEC.md contradicts its own Part 1
measurements. Part 1 is what was measured off the drawing and Part 2 is the
algorithm inferred from it, so where they disagree Part 1 wins.

- **fBm scale.** The amplitude-normalised sum has an RMS of 0.195, so
  `lump_amp` meant a fifth of what the spec says. Rescaled to unit RMS and
  soft-limited; the silhouette now measures a relative radial std of 0.21 and
  an Rmin/Rmean of 0.54 against the artwork's 0.25 and 0.52.
- **`lump_freq` is lobes per turn, not a noise radius.** A circle of radius
  *f* in 2D noise meets about π·*f* periods, so reading the parameter as the
  radius gave eight lobes where it says 2.6, leaving the measured dominant
  harmonic at k=3 empty.
- **The limiter's kinks are rounded.** It is a min/max filter and leaves a
  48-degree corner wherever it bites; the artwork has no corner sharper than
  a 19-unit radius. Six binomial passes at 720 samples remove the kinks and
  leave the spectrum unchanged to three decimals.
- **The inner contour's eroder is capped.** Eroding `r_in` can only widen the
  ribbon, and at the outer contour's own budget it swelled the band by half -
  failing the evenness the spec measures as the brand-defining property.
- **Cuts come in two families.** Tracing the field alone gives one direction
  per seed and tiles the rock in parallel stripes; the artwork's long edges
  are bimodal, two families 46 degrees apart.
- **The re-roll ladder is mixed, not added.** `splitmix64(seed + attempt)`
  makes seed 5's second re-roll identical to seed 6's first, so two seeds can
  emit the same logo.
- **The facet root sits on the ribbon's MIDLINE, not half a width outside
  it.** Part 2 says to dilate it to `r + w/2` so "the ribbon covers the
  overshoot". It cannot: the ribbon occupies `[r - w, r]`, so anything past
  `r` is outside it and nothing paints over it. Built that way the tiling
  showed as a bright orange fringe all the way round the outside of the black
  outline, on every seed. At `r - w/2` the facets stop halfway through the
  ink and the ribbon covers them with half a width to spare, which is the
  seam class the instruction was reaching for.
- **`vesicle_size_pow` is 1, not 2.** Part 2's table asks for a square law
  "matching the measured clustering"; Part 1 measures seven equivalent radii
  that are near-uniform over their range, says in words "do NOT use a steep
  power law here", and says the distribution is biased toward the *upper*
  half. The square law biases the other way and held total coverage at 0.152
  against the measured 0.207.
- **Vesicle radii are drawn stratified, and `vesicle_rmax` is the measured
  0.240.** Seven independent uniforms land within 2x of each other about a
  third of the time - the polka-dot layout the spread gate then has to reject
  - while the artwork's seven span 3.08x in a clear two-big / three-mid /
  two-small hierarchy. Drawing one radius per `1/count` band of the range
  reproduces that by construction instead of by re-rolling for it. It also
  retires a workaround: `vesicle_rmax` had been raised to 0.320 because the
  packer never reached the top of its range, and with the range actually
  used, 0.320 overshoots. Back at the measured 0.240 the generator went from
  5 failures in 300 seeds to none, and from 5.0 re-rolls per logo to 0.5.
- **Big holes are bitten; the spec has no such step.** Part 1 measures the
  three largest holes at circularity 0.405, 0.459 and 0.587 - an L with a
  hooked tail, a bitten kidney - and Part 2 expects radial fbm to deliver
  them. It cannot: a star-shaped polar blob sits near 0.79 however much
  amplitude it is given, because a lobe is a bump and a bump is not a bite.
  So `vesicle_bite` carves one or two deep concave notches into the big
  holes, which brings them to 0.53-0.67. Three things about it were each
  wrong on the first try and are each pinned by a test now: a wide window
  does not bite, it scales one flank down into a teardrop that measures
  *more* circular; three evenly spaced bites make a rounded triangle that
  measures back up; and the bite has to aim across the long axis, because a
  bite into the end of an elongated hole only makes it shorter. Small holes
  are left alone - the four smallest measure 0.70-0.87 and are correctly
  convex.
- **The silhouette is band-limited by 40 smoothing passes, not 6.** The
  log-Lipschitz limiter is purely erosive, so a deep notch survives as a V
  with a corner at the bottom, where every concave notch in the drawing is
  rounded. Six passes at 720 samples is a 1.2-degree window - far too narrow
  to touch a notch 20 degrees wide - and left the sharpest generated convex
  corner at 0.045 of the mean radius against the artwork's 0.113. On a sheet
  those read as spikes. Forty passes is a Gaussian of sigma 2.5 degrees:
  it lifts the sharpest corner to 0.09-0.10 and attenuates harmonic k by
  `exp(-(k*sigma)^2/2)`, which is under a tenth at h12 and gone by h40 -
  the measured spectrum exactly, real energy to h12 and 0.004 above it.
  Notably this is *not* reachable through `gain` or `lump_octaves`: sweeping
  both moved the h13-h16 energy hardly at all, because the excess was never
  fbm detail, it was a handful of sharp features being broadband.
- **`facet_curviness` is 0.25, not 1.00.** Part 1 measures the facets' long
  edges deviating under 6% from their chord, with the longest running 145
  units dead straight. A full flow trace sweeps far more than that - measured
  over twelve seeds, median deviation runs 7.7% at curviness 0 to 12.7% at
  1.0 - and the tone boundaries come out as soft sweeping bands instead of
  the artwork's shards.
- **Two gates the spec does not have: silhouette amplitude and bbox aspect.**
  `lump_amp` sets the *expected* radial amplitude, not the realised one; the
  fbm scatters around it by a factor of three, so at the measured 0.22 the
  radial std ran 0.10 to 0.33 across seeds and the bottom of that range is an
  egg. Likewise `aspect` sets the base ellipse, but the lumps are what the
  viewBox is finally fitted to, and the realised box ran 1.09 to 2.05 against
  the artwork's 1.499. Both are gates on the ROLL rather than the parameters:
  the amplitude band is a ratio of `lump_amp`, so `--preset feral` still gets
  what it asks for. With them the realised aspect runs 1.25 to 1.84, median
  1.50.
- **The silhouette amplitude gate measures against the BASE ELLIPSE, not
  the mean radius.** `lump_amp` is documented and measured as a fraction of
  the base ellipse radius, and an ellipse's radius varies a great deal on its
  own - 0.19 at the pre-stretched default, before a single lump. Measuring
  about the mean therefore measures the aspect and the lumps together, which
  makes the gate a second, worse `bbox_aspect` and leaves `lump_amp` almost
  no authority: the three presets ask for 0.120, 0.201 and 0.300 and measured
  0.199, 0.217 and 0.240, a 2.5x request compressed into a 1.2x response, so
  `tame` read as a splat and `feral` as an egg. Dividing by the base ellipse
  first gives 0.087, 0.131 and 0.174.
- **`light_dir` is 2.30, not the spec table's 1.90.** Part 1 measures the
  tone axis directly - the three tones' visible-pixel centroids run from
  (276.9, 135.3) to (179.0, 244.9), the unit vector (-0.666, +0.746), which
  is 2.30 rad. The 1.90 was inferred from where the lightest facets sit, and
  it costs accuracy: it puts the shadow tone's mean position at 0.399 against
  the measured 0.302, where 2.30 puts it at 0.368.
- **`tone_noise` stays at 0.35 even though less looks better.** Less noise
  reads as more three-dimensional by eye - larger, cleaner tone regions, a
  sharper terminator. The artwork's tones overlap by a full quartile, and
  matching that overlap is what makes a generated rock belong to the same
  family. Against Part 1's measured tone positions, 0.20 puts the three means
  0.013, 0.058 and 0.078 off while 0.35 puts them 0.066, 0.040 and 0.015 off,
  and 0.35 reproduces the interquartile widths too. The measurement overruled
  the eye.
- **An iso-brightness line is leaned on, never followed.** Re-reading the
  solid's iso-brightness direction at every Euler step is the obvious way to
  make a facet boundary follow the shading, and it does not work: a level set
  either leaves the region or closes into a loop, and the median cut's total
  absolute turning went from 17 degrees to 235 against the 220 the
  no-self-intersection test allows. Neither a cone about the starting heading
  nor a per-step rate limit helps, because a heading can oscillate inside a
  bound forever. The direction is read once, at the seed.
- **The holes are laid out, not scattered.** Rejection sampling leaves the
  composition to the dice - clumps and bald patches, with the measured
  spread reached only by re-rolling for it. A relaxed weighted Voronoi
  diagram gives it by construction: every hole sits in the middle of its
  own territory, sized by the territory it claimed. Fidenza's collision
  loop stays as the FALLBACK, because a relaxed layout is optimised for
  even territory rather than for clearance and some holes do not fit where
  it puts them - placing only at the relaxed position cost the clean-roll
  rate 160/200 to 106 and coverage 0.166 to 0.150, while relaxed-first and
  darts-after beats both at 164/200 and 0.169.
- **The shading deals the hole sizes out rather than scaling them.** A hole
  is the darkest ink on the page, so where the holes are big the area reads
  dark; the darkest seed therefore takes the largest size. Dealing rather
  than scaling keeps the multiset of sizes exactly as drawn, so the
  measured 3.08x hierarchy survives by construction - scaling multiplied
  the hierarchy by a second factor and the two crossed, leaving the
  realised sizes spanning 2.0x where the draw asked for 3.3x.
- **The holes are not Voronoi CELLS, though.** The diagram decides where
  they go and how big; the outlines stay curvy blobs. Part 1's sharpest
  measured distinction is between the two shape families, and polygonal
  holes would be the clearest possible way to break it.
- **The relief mark's width follows the fold.** A monoline is what the
  artwork's one sample is, and beside holes this large it reads as a
  scratch. The line now swells to 1.5x its nominal width where the fold is
  sharp and thins to 0.45x where it softens, on a nominal raised from 0.58
  to 0.76 of the outline - so a mark's widest point measures 11 to 19
  canonical units against the outline's 17.4.
- **Fewer, larger holes, and the count is drawn rather than set.**
  `vesicle_count` is the MEAN of a Poisson draw now, truncated to
  `[3, 2*mean+4]`, and it is 5 against the artwork's measured 7 with the
  radii raised to match. A departure from Part 1 rather than a correction
  to it - but the count of events in a volume is Poisson, a constant count
  is the same tell as a constant lobe count, and it measured better on
  every axis the crate gates: coverage 0.147 to 0.166, largest hole 90 to
  116 units, clean rolls 77% to 80%. It also fixed `--preset feral`, which
  had been failing seeds since before the 3D stage.
- **A hole lies ON the surface.** Its long axis turns to the local contour
  and it foreshortens by the tilt, both weighted by how tilted the surface
  under it actually is - so a hole near the rim stretches along the rim and
  one on a face-on patch keeps the flow field's character. Measured over
  120 seeds, the ellipse aspect runs 1.68 / 1.72 / 2.01 / 2.52 and the
  axis-to-contour alignment 0.648 / 0.699 / 0.770 / 0.869 as the surface
  tilts from face-on to the rim.
- **The relief mark is traced, not taken from a cut.** Lifting it from a
  facet cut and truncating it at `relief_len` is convenient and gives every
  logo the same mark: measured, 14 of 16 got a stroke of exactly 164.2 units
  and 9.41 width deviating 2-7% from its chord, the same object rotated. A
  fold in a lumpy rock is not a chord. The mark now follows the crest of the
  surface's own fold - the eigenvector of the smaller principal curvature of
  the brightness - from the rim until the crease fades, which gives 98-229
  units and 7.3-11.1 width at 3-23% deviation, with the mean length landing
  on the artwork's. Length, curvature and waist all come from the rock.
- **The projection fills notches.** The maximum over tilt can only add
  radius, and it adds most where the displacement dipped, so Rmin/Rmean comes
  out at 0.625 against the artwork's 0.52 - the generated bites are shallower
  than the drawing's. This is the clearest remaining gap against Part 1 and
  it is not fixed.
- **The tameness dial sizes the holes against their REACH.** A hole's
  half-extent is `r * sqrt(aspect) * (1 + amp * AMP_BIG)`, and that is what
  has to fit; the dial moves both the count and the amplitude, so it scales
  the whole size range by `1/sqrt(count)` and by the amplitude inflation.
  Its feral end is 9 holes rather than 11, which also fixes an off-by-one -
  `lerp(11, 5)` rounds to 8 at the default `tameness` 0.55, where the spec
  says the dial reproduces the artwork's measured 7, and `lerp(9, 5)` gives
  exactly 7.
- **The release build's self-intersection test covers the facets only.**
  SPEC.md lists every contour, then two sections earlier proves that the
  polar ones cannot self-intersect - a star-shaped curve meets every ray from
  its centre exactly once, and `star_shaped` and `vesicle_health` assert the
  hypothesis. Only the facets, which are traced rather than polar, fall
  outside the theorem. The full sweep was 96% of the entire gate suite's cost
  and about a quarter of the generator. Debug builds, which is what
  `cargo test` runs, still check every contour.

## Output and cost

About 90 KB per logo at the default resolutions (mean over 500).

**500 logos in 5.04 seconds single-threaded, 10.1 ms each** - measured three
times on one core, spread under 1.3%, 0 failures, 158 re-rolls over the 500.

| where it goes | per logo |
|---|---|
| silhouette and depth map | ~5.9 ms, the single largest item |
| SVG writer | ~1.7 ms |
| facets, packing, relief | ~1.2 ms |
| quality gates | ~0.4 ms |
| re-rolls | 0.32 extra attempts average; a rejected roll bails after the silhouette |

The 3D stage is most of it, and it is the count of noise evaluations rather
than anything slow: roughly 20,900 radius evaluations for the silhouette
(720 angles x 29 samples of tilt) and 8,800 for the depth map, each a
4-octave 3D fBm at about 160 ns. 3D Perlin itself measures 26.4 ns a call.
The 2D generator this replaced ran at 4.2 ms; the extra 5.9 is the rock.

Both remaining levers were left alone deliberately. The silhouette's coarse
scan could run at two octaves instead of four and the refinement at all
four, for about 10%; that is complexity against a budget nothing is pressing
on. The writer's 1.7 ms is float formatting, and a hand-rolled fixed-point
formatter would take most of it - but byte-identical output is a hard
promise and a new formatter is the riskiest possible place to touch it.

## Preset health

`generate` re-rolls internally and only returns `Err` when every roll in
`retry_budget` (20) failed the same gate, which means the parameter set is
infeasible rather than unlucky. Measured over 120 sequential seeds, beside
the same sweep on the 2D generator this replaced:

| preset | failed | re-rolls | failed (2D) | re-rolls (2D) |
|---|---|---|---|---|
| `anya` (the default) | 0 / 120 | 45 | 0 / 120 | 70 |
| `tame` | 0 / 120 | 51 | 0 / 120 | 113 |
| `feral` | 0 / 120 | 199 | 9 / 120 | 626 |

`feral` is the corner of the parameter space - `vesicle_amp` 0.42,
`vesicle_aspect_max` 2.40 and `slope_limit` 1.80 all at once - and it used
to fail seeds outright, on the vesicle coverage gate reporting a genuine
geometric infeasibility: what has to fit is a hole's reach,
`r * sqrt(aspect) * (1 + amp)`, and the dial was asking for nine big lumpy
holes in a body that could not hold them. Drawing the count and compensating
the radii per roll by `sqrt(mean / drawn)` removed that: there is no longer a
setting of the dial that asks the packer for more than the body can take. It
is still the expensive end - 199 re-rolls against 45 - and callers who want
speed should use the default.

The test for this is a RATE over a sweep, not one seed: a single-seed check
passes unchanged with 95% of seeds failing, which is what it did while
`feral` was quietly exhausting its budget on one seed in thirty.

## Parameters

Every one is settable by name: `--set key=value`, repeatable, applied after
`--preset`. The same table is `Params::describe()` and `--list-params`.

| name | type | default | range | effect |
|---|---|---|---|---|
| `seed` | u64 | `0` | any u64 | The whole image; sequential seeds are unrelated. |
| `tameness` | f64 | `0.55` | 0 ..= 1 | 0 feral, 1 corporate; drives seven other knobs. |
| `aspect` | f64 | `1.50` | 1.0 ..= 2.2 | Width/height of the base ellipse; 1.0 a pebble, 2.2 a shard. |
| `r0` | f64 | `300` | canonical | Canonical mean radius; every other length is a fraction of it. |
| `margin` | f64 | `0.04` | 0 ..= 0.15 | viewBox padding around the true bbox, as a fraction of R0. |
| `lump_amp` | f64 | `0.22` | 0.05 ..= 0.35 | Silhouette chaos: 0.05 an egg, 0.22 the artwork. |
| `lump_freq` | f64 | `2.6` | 1.5 ..= 5.0 | How many primary lumps go round; above 4 reads as texture. |
| `lump_octaves` | u32 | `4` | 1 ..= 6 | fbm depth for the silhouette; the spectrum supports 4. |
| `lacunarity` | f64 | `2.03` | 1.80 ..= 2.40 | Per-octave frequency step; not 2.0, which grows grid ridges. |
| `gain` | f64 | `0.45` | 0.30 ..= 0.60 | Per-octave amplitude step; higher is crustier. |
| `warp_amt` | f64 | `0.22` | 0 ..= 0.60 | How far the field displaces the noise lookup; lumps lean. |
| `field_freq` | f64 | `0.55` | 0.15 ..= 1.50 | Field frequency per R0; 0.55 is one swing across the body. |
| `field_angle_range` | f64 | `1.0` | 0.15 ..= 1.50 | Hobbs's lambda; 0.35 is a combed, graphic field. |
| `field_bias` | f64 | `0.0` | 0 ..= 2*PI | Rotates lumps, vesicle axes and cuts together. |
| `field_quantize` | f64? | `none` | none or 0.01 ..= PI | Snap field angles to a multiple; reads crystalline. |
| `slope_limit` | f64 | `1.10` | 0.50 ..= 2.00 | kappa in \|d(ln r)/d theta\|; the anti-cusp guard. |
| `ring_samples` | u32 | `720` | 256 ..= 2048 | Samples of r(theta) before the arclength resample. |
| `bezier_points` | u32 | `256` | 96 ..= 512 | Emitted cubics on the outer contour; 96 reads hand-drawn. |
| `cr_tension` | f64 | `0.50` | 0.35 ..= 0.60 | Catmull-Rom tau; lower is crisper, higher bulges. |
| `outline_weight` | f64 | `0.058` | 0.020 ..= 0.100 | Ribbon width over R0; the brand-defining number. |
| `inner_rim` | f64 | `0.0` | 0 ..= 0.030 | Optional second thin ribbon. The artist's file has one at 0.0147 but it is fully hidden under the main ribbon; off matches the drawing. |
| `facet_cuts` | u32 | `11` | 4 ..= 20 | Bisections; regions = cuts + 1. |
| `facet_curviness` | f64 | `0.25` | 0 ..= 1.50 | 0 a straight chord, 1 a live flow trace. |
| `facet_min_area` | f64 | `0.015` | 0.005 ..= 0.050 | Anti-sliver: smallest allowed child, over body area. |
| `facet_min_chord_sep` | f64 | `0.08` | 0.03 ..= 0.20 | Anti-wedge: crossing separation over the perimeter. |
| `tone_fractions` | f64x3 | `0.30,0.60,0.10` | each 0.06 ..= 0.80, sum 1 | Area quantiles for shadow / mid / highlight. |
| `light_dir` | f64 | `1.90` | 0 ..= 2*PI | Tone gradient direction; 1.90 rad lights from BELOW. |
| `tone_noise` | f64 | `0.35` | 0 ..= 1 | Ramp versus fbm in the light function. |
| `palette_hue` | f64 | `13.0` | 0 ..= 360 | Shadow hue in degrees; moves the whole family. |
| `palette_sat` | f64 | `1.00` | 0.50 ..= 1.00 | HSL saturation for all three tones. |
| `palette_light` | f64 | `0.565` | 0.30 ..= 0.75 | HSL lightness of the MID tone. |
| `tone_contrast` | f64 | `1.00` | 0.40 ..= 2.00 | Scales both deltas off the mid tone. |
| `ink` | rgb | `#231f20` | any | Near-black for the ribbon and the vesicles. |
| `background` | rgb? | `none` | none or any | None emits no rect, which is what a logo wants. |
| `vesicle_count` | u32 | `7` | 0 ..= 20 | Target gas holes; the packer may place fewer. |
| `vesicle_rmin` | f64 | `0.070` | 0.030 ..= 0.120 | Smallest vesicle radius over R0. |
| `vesicle_rmax` | f64 | `0.240` | 0.100 ..= 0.400 | Largest vesicle radius over R0. |
| `vesicle_size_pow` | u32 | `1` | 1 ..= 3 | Exponent in the u^pow size law; integer, so no powf. |
| `vesicle_bite` | f64 | `0.60` | 0 ..= 0.70 | Concave bite carved into the big holes; 0 leaves potatoes. |
| `vesicle_amp` | f64 | `0.30` | 0.10 ..= 0.50 | Vesicle outline lumpiness; scaled by size inside. |
| `vesicle_octaves` | u32 | `3` | 2 ..= 5 | fbm depth per vesicle; small ones use one less. |
| `vesicle_aspect_max` | f64 | `2.40` | 1.0 ..= 3.2 | Elongation of the largest vesicle. |
| `vesicle_align` | f64 | `0.85` | 0 ..= 1 | How strongly a vesicle's long axis follows the field. |
| `vesicle_edge_clear` | f64 | `0.055` | 0.030 ..= 0.120 | Gap to the INNER boundary; floored at 1.2*outline_weight. |
| `vesicle_edge_ratio` | f64 | `0.80` | 0.000 ..= 1.500 | Gap to the inner boundary as a multiple of the hole's own radius. |
| `vesicle_gap` | f64 | `0.090` | 0.040 ..= 0.200 | Vesicle-to-vesicle gap; floored at 0.45*min radius. |
| `vesicle_attempts` | u32 | `220` | 50 ..= 1000 | Placement tries per vesicle before dropping it. |
| `attempts_total` | u32 | `2000` | 500 ..= 20000 | Global placement budget. |
| `consec_fail_stop` | u32 | `150` | 40 ..= 600 | Stop packing after this many consecutive failures. |
| `coord_decimals` | u32 | `3` | 2 ..= 4 | Decimal places in emitted coordinates. |
| `retry_budget` | u32 | `20` | 1 ..= 100 | Whole-logo re-rolls before returning Err. |

## Layout

```
src/math.rs     V2 and V3, hand-rolled sin/cos, the diamond pseudo-angle, polygon maths
src/rng.rs      SplitMix64 + PCG32, five named streams
src/noise.rs    Perlin in 2D and 3D, fBm, the sphere-sampled fBm
src/solid.rs    the 3D rock: the displaced ellipsoid, its projected contour, the depth map
src/field.rs    the one flow field
src/ring.rs     the star-shaped r(theta) ring and its limiter
src/bezier.rs   Catmull-Rom to cubics, handle clamp, self-intersection
src/params.rs   the parameter surface, tameness, presets, validation
src/palette.rs  HSL, hex, the three-tone ramp
src/body.rs     silhouette, ribbon, and the pipeline
src/facet.rs    recursive bisection, iso-brightness steering, tone quantiles
src/relief.rs   the thin interior crease marks
src/vesicle.rs  collision packing
src/quality.rs  the gates
src/svg.rs      the document writer
src/main.rs     the CLI

tests/determinism.rs  same seed -> same bytes; stream separation
tests/quality.rs      structural invariants of the emitted text, 200 seeds
```

## A note on the build

This crate sits inside the scoria repository but **outside** its cargo
workspace - the empty `[workspace]` table in `Cargo.toml` is what detaches it.
So `cargo test --workspace` at the repository root never builds it, and it
never pulls gpui. Build and test it from this directory:

```sh
cargo build && cargo test
cargo fmt --all && cargo clippy --all-targets -- -D warnings
```

A cold build is under two seconds, because there is nothing to compile but this.
