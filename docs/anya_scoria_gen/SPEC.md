# The scoria mark, measured and specified

What this file is: the measurements taken off Anya's `docs/img/anya_v0_base.svg`,
and the algorithm the generator in this crate implements from them. Every number
below was measured, not estimated. When the generator and this file disagree,
measure the artwork again and fix whichever is wrong.

Source art: `../img/anya_v0_base.svg` — 457.13974 x 304.92206, 24 solid-fill
`<path>`s, no strokes, no gradients, five inks.

---

## Part 1 — the artwork, measured

### Canvas
- 457.13974 x 304.92206, aspect 1.49919
- x 0.00 .. 456.80, y 0.20 .. 304.60 (456.80 x 304.40 units). The rock touches all four canvas edges; there is effectively NO margin (max 0.35 units of slack on any side). Fit the viewBox to the silhouette rather than insetting it.

### Palette

| hex | role | share of silhouette |
|---|---|---|
| `#ff5721` | facet / mid orange (cls-2, 5 paths) - the workhorse tone, spread over the whole body | 38.6% |
| `#ee3300` | facet / dark orange-red (cls-1, 5 paths) - shadow side, upper-right | 21.5% |
| `#ff6f21` | facet / light orange (cls-3, 4 paths) - highlight, bottom and bottom-left | 6.6% |
| `#231f20` | vesicles (cls-4, 7 paths) - the dark holes | 19.8% |
| `#221f1f` | outline ribbon (path15) plus one interior crack mark (path23) | 13.4% |

Notes per ink:

- `#ff5721` — HSV 14.6deg, S87, V100. Visible area 36225 units^2 = 38.6% of the silhouette, 57.8% of all orange. Visible-pixel centroid (218.7, 175.7), i.e. within 15 units of the body centroid - this tone is spatially neutral.
- `#ee3300` — HSV 12.9deg, S100, V93. Visible area 20207 units^2 = 21.5% of the silhouette, 32.3% of orange. Visible-pixel centroid (276.9, 135.3): 44 units right of and 37 units above the body centroid. 4 of its 5 paths sit at x>278; the 5th (path16) is the central cell and the single largest facet at 9420 units^2 visible.
- `#ff6f21` — HSV 21.1deg, S87, V100. Only 6220 units^2 = 6.6% of the silhouette, 9.9% of orange - the highlight is RARE. Visible-pixel centroid (179.0, 244.9): 54 units left of and 73 units BELOW the body centroid. All 4 patches lie at y>209; three hug the bottom rim.
- `#231f20` — Visible 18601 units^2 = 19.8% of the silhouette, 22.6% of the interior inside the ribbon. A different near-black from the outline (#231f20 vs #221f1f) - a 1-2/255 difference, invisible at any size but a real two-ink distinction in the file.
- `#221f1f` — Ribbon 12114 units^2 (12.8% of the silhouette) + crack 469 units^2 (0.5%). A sixth #221f1f path (path14, 2561 flattened points, a 3107 units^2 hairline ribbon) is painted first and measured exactly 0 visible pixels - fully occluded by path15. Dead geometry; do not reproduce it.

### Silhouette

A wide, flat-bottomed, bottom-heavy lump filling the whole 457x305 frame edge to edge. The top-left rises in one long smooth 120-unit hump; the top-centre peaks near (285,3) with a small notch beside it; the right side steps down through two shoulders with a deep concave bite at (456,202); the bottom is long, nearly straight and gently scalloped. Measured area 94884 units^2 = 68.1% of the canvas. Outer perimeter 1279.5 units against 1092.0 for an equal-area circle (ratio 1.172) - the boundary is only 17% longer than a circle, so this is a LUMP, not a jagged shard. Only 46 of 2048 radial rays from the centroid cross the boundary more than once, so the shape is 97.8% star-shaped about its centroid and a radius-profile model is valid.

- centroid (233.293, 172.138) viewBox units - 4.7 units right of and 19.7 units BELOW the canvas centre (228.57, 152.46). Angle convention for radius_profile_64: index k is theta = 2*pi*k/64 from the +x axis with +y pointing DOWN (SVG), so k=0 east/right, k=16 south/down, k=32 west/left, k=48 north/up.
- mean radius 169.52, min 100.65, max 240.35
- lobes: 14, flat bottom: True

**Corners.** ROUNDED everywhere - not one sharp corner on the silhouette. Circumradius measured over a 4-unit arc span: median convex radius 23.9 units, 10th percentile 12.6, absolute minimum convex radius 19.1 at the sharpest bump (303.1, 49.3). Concave notches are equally soft: median concave radius 31.6, tightest 12.5 at (66.9, 294.3). 33.4% of the perimeter is convex by arc length, 66.6% concave. Practical rule: clamp every boundary curvature radius to >= 12 units (0.026 of width, 0.071 of mean radius). 14 convex lobes detected with R<250 (radii 19.1, 19.5, 19.8, 23.8, 29.2, 30.8, 33.6, 38.1, 38.2, 39.9, 43.7, 66.3, 104.1, 118.1) against 17 concave notches - one feature every ~41 units of arc. A human reads about 10-12 distinct bumps, since the four with R>60 are too gentle to register. Harmonic spectrum of the normalised profile: h1 0.047, h2 0.261, h3 0.155, h4 0.030, h5 0.036, h6 0.043, h7 0.023, h8 0.018, h9 0.036, h10 0.029, h11 0.014, h12 0.014, and everything above h12 collapses to a mean of 0.004. RMS of h>=4 is 0.063; RMS of h>=8 is 0.041.

**Radial profile**, 64 radii at equal angles from the centroid, normalised to mean 1.0
(index k is theta = 2*pi*k/64 from +x with +y down; k=0 east, k=16 south, k=32 west, k=48 north):

```
  1.3023  1.3204  1.2622  1.2536  1.2858  1.3256  1.3121  1.2188
  1.0809  0.7646  0.6889  0.6106  0.6097  0.6220  0.6194  0.6004
  0.5945  0.6175  0.6645  0.7279  0.7973  0.8679  0.9371  0.9994
  1.0510  1.1120  1.2144  1.2345  1.4031  1.4125  1.4004  1.3079
  1.2561  1.2166  1.1378  1.0200  0.9879  0.9683  0.9521  0.9288
  0.8781  0.8460  0.8411  0.8528  0.8769  0.9076  0.9372  0.9592
  0.9758  0.9995  1.0329  0.9972  0.9027  0.8367  0.8525  0.9039
  0.9712  1.0485  1.0948  1.0996  1.0853  1.0744  1.1101  1.2292
```

### Outline ribbon

- ink `#221f1f`, filled ribbon (not a stroke): True
- thickness 9.5–10.86 units, 2.18% of the canvas width

Measured at 1280 points spaced 1 unit apart along the outer contour as the nearest distance to the inner contour: median 9.96, mean 9.89, stdev 0.71, p1 6.09, p99 10.42, max 10.86. Excluding the single seam artefact the whole distribution lies in 9.50-10.86, i.e. +-7% about 10.0 - the ribbon is effectively a CONSTANT 10.0-unit inward offset of the silhouette. Checked per 30-degree sector about the centroid: every sector mean falls in 9.81-10.10, so there is no taper and no calligraphic modulation anywhere. 10.0 units = 0.02188 of the 457.14 width = 0.059 of the mean radius. The ribbon is a single <path> with ONE subpath: the outer silhouette loop, then the inner loop wound the other way, joined by a zero-width slit near (75.5, 128.3) on the left flank. There is no 'z' command anywhere in the file; every path relies on implicit closure. Net signed area 94884 - 82415 = 12469 units^2. Build it as outer_loop + reverse(offset(outer_loop, -10.0)) in one path - visually identical to a 20-unit stroke clipped to the silhouette, but keeps the output stroke-free like the original.

### Vesicles

- count 7, 19.8% of the silhouette area
- equivalent diameters (units, descending): 80.79, 76.56, 62.79, 56.15, 50.94, 33.33, 26.23
- bbox aspect ratios: 1.52, 1.22, 2.37, 1.06, 1.23, 1.92, 2.17
- min clearance to the body edge 7.56 units; min gap between two vesicles 17.38 units

**Shape.** Smooth closed all-cubic blobs - 95 bezier segments across the 7 paths, median chord 13.4 units, LONGEST chord only 53.4, median deviation-from-chord 6.6% of chord length. Not a single long straight run: only 19% of their segments are straighter than 3% deviation, against 38-48% for the orange facets. That is the sharpest measurable difference between the two shape families in the drawing. Vesicles are flat solid fills with NO rim, NO outline and NO highlight.

**Lobes.** Concave runs per outline (separate inward-curving stretches, sampled every 2 units), in descending-size order: 5, 8, 4, 3, 2, 3, 2. Isoperimetric circularity 4*pi*A/P^2 in the same order: 0.459, 0.587, 0.405, 0.872, 0.797, 0.732, 0.700. The two largest are strongly re-entrant (0.41-0.59, 5-8 lobes - path19 is an L/boomerang with a hooked tail, path22 a kidney bitten out on its left side); mid-size ones are near-convex rounded polygons (0.73-0.87). Big vesicles are lumpy, small vesicles are smooth: that correlation is strong and must be reproduced.

**Orientation.** PCA major-axis angles (degrees mod 180, 0 = horizontal), descending size: 9.3, 64.9, 14.9, 7.8, 62.1, 80.6, 12.5. Taking only the 4 genuinely elongated ones (bbox aspect > 1.5): 9.3, 14.9, 80.6, 12.5 - three of four lie within 15 degrees of HORIZONTAL, aligned with the rock's long axis, which matches flow-flattened vesicles in real scoria. With n=4 this is suggestive, not proven. The two round ones (aspect 1.06, 1.22) have meaningless angles. Recommendation: draw the major axis from a wide distribution centred on 0 degrees, applied only when eccentricity exceeds ~1.5.

**Placement.** All 7 sit well inside the body - none breaks the rim, none touches another. Minimum clearance to the OUTER silhouette: 72.5, 47.2, 37.8, 35.8, 32.3, 19.8, 17.9 (min 17.9 = 0.039 of width). Minimum clearance to the INNER edge of the ribbon (the orange boundary): 62.6, 37.2, 27.9, 26.0, 22.3, 9.9, 7.6 (min 7.6 = 0.017 of width, 0.76x the ribbon thickness). Minimum gap between any two vesicles 17.38 (path21-path24), then 22.59, 34.74, 37.40, 43.82, 45.25; median of all 21 pairwise gaps 71.23, max 178.69. Centroids: (98.8,205.9) (360.6,172.8) (287.5,235.2) (166.4,132.2) (244.6,77.1) (262.1,167.0) (158.5,269.2). They SPREAD rather than cluster: the vesicle set's centroid is (227.9, 178.6), within 7 units of the body centroid, and they occupy all four quadrants. Packing rule: reject a candidate unless clearance to the body boundary is >= 0.8x its own equivalent radius AND >= 7.5 units, and its gap to every placed vesicle is >= 17 units.

### Facets

Fourteen flat-filled polygonal cells (5 x #ee3300, 5 x #ff5721, 4 x #ff6f21) completely tiling the interior of the outline ribbon. Visible areas: 9634, 9420, 8505, 7094, 6622, 4370, 3661, 3237, 2230, 1885, 1660, 1537, 1461, 1337 units^2 (largest/smallest 7.2x, mean 4475). Their total 62652, plus vesicles 18601, crack 469 and ribbon 12114, comes to 93836 = 98.9% of the 94884-unit silhouette, the missing 1.1% being antialiasing at seams - so the tiling is gap-free. Each cell has 5-15 corners (median 8.5) where direction turns more than 25 degrees.

**How they tile.** A SHARD / CRACK SUBDIVISION, like a plate dropped once - not stripes, not a radial fan, not Voronoi. Straight chords cut across the interior and meet at Y and T junctions. 7 of the 53 long edges (>=30 units) are DUPLICATED to within 12 units by a neighbouring facet: adjacent cells share an exact boundary rather than overlapping at random - e.g. (144,103)->(151,246) appears in both path11 (#ff5721) and path16 (#ee3300) with only 4.7 units of total endpoint mismatch, and (322,179)->(365,65) is shared by path1 and path20. Of the 106 long-edge endpoints, 19% land within 5 units of the body boundary, 32% within 12, 53% within 25, median 22.3 - so roughly half the cuts run edge-to-edge and terminate on the rim and half die at an interior junction. I explicitly TESTED and REJECTED the 'wedges radiating from an off-centre point' hypothesis: the length-weighted least-squares concurrency point is (219.0, 178.1) but the median perpendicular miss of the 53 long edges to it is 95.6 units, 0.56 of the mean radius, no better than fitting the centroid (99.5) or the canvas centre (101.7). The edges genuinely do not concur. Length-weighted direction histogram (mod 180 deg) is bimodal: 60-75 deg carries 432 units and 105-120 deg carries 470, with secondary mass at 30-45 (238) and 135-150 (250) and the least near horizontal (165-180: 65). Generator recipe: recursively split the interior polygon with straight chords whose direction is drawn mostly from the two steep families, stop at ~14 cells with a roughly 7:1 large-to-small area spread, then fillet every resulting corner with a short cubic.

**Edges.** Long near-straight chords joined by short rounded corners. Per cell, 27-80% of the perimeter (median ~49%) is segments at least 30 units long deviating less than 6% from their chord; the longest are 145.0, 142.8, 122.2, 121.6, 93.1, 86.0, 79.4, 76.4 units. The rest of the perimeter is short segments - median chord across all orange paths 8.5-11.4 units - that fillet the corners, so the cells read as polygons but every vertex is softened by a ~5-12 unit radius. Every facet is a single closed subpath of cubics; nothing in the file uses a stroke.

**Where the light is.** BOTTOM-LEFT. This is the counterintuitive part and must not be flipped. Projecting every orange pixel onto the unit axis u = (-0.666, +0.746) (pointing down-and-left on screen) and normalising the body extent to 0..1 gives mean positions 0.302 for #ee3300 (darkest), 0.486 for #ff5721 (mid), 0.695 for #ff6f21 (lightest) - a clean monotonic ramp. Equivalently the three visible-pixel centroids (276.9,135.3) dark, (218.7,175.7) mid, (179.0,244.9) light lie near a straight line from upper-right to lower-left. The interquartile ranges overlap heavily (dark 0.14-0.45, mid 0.31-0.68, light 0.51-0.92), so tone assignment is a NOISY gradient, not a hard threshold: score each cell by its centroid's projection on u, add noise of about +-0.17 (one quartile), then bin by the area quota 32% dark / 58% mid / 10% light of the orange area.

**Relation to the silhouette.** The facet set is clipped exactly to the inner edge of the outline ribbon - every cell stops either on that curve or on another cell, so a boundary cell has one curved edge and several straight ones. Facets are painted BEFORE the vesicles, which punch through them: the biggest facets lose 25-45% of their drawn area to later paths (path11 is 13468 units^2 as drawn but 8505 visible; path20 is 13811 drawn / 9634 visible). Z-order is also interleaved with the ribbon - 9 of the 14 facets are painted after path15 - but all are drawn to stay inside it, so the ribbon is never overpainted.

### Paint order

1. path1 #ee3300 facet (upper right)
2. path2 #ff5721 facet (upper centre)
3. path3 #ee3300 facet (right flank)
4. path4 #ee3300 facet (lower right)
5. path5 #ee3300 facet (top)
6. path6 #ff6f21 facet (bottom-right highlight)
7. path7 #ff5721 facet (bottom centre)
8. path8 #ff6f21 facet (bottom-left corner highlight)
9. path9 #ff6f21 facet (bottom-left highlight)
10. path10 #ff5721 facet (upper left)
11. path11 #ff5721 facet (left flank)
12. path12 #231f20 VESICLE (upper centre, eqd 50.9)
13. path13 #231f20 VESICLE (bottom left, eqd 26.2, smallest)
14. path14 #221f1f DEAD hairline ribbon - measured 0 visible pixels, fully occluded by path15; omit it
15. path15 #221f1f OUTLINE RIBBON (the silhouette as a filled 10-unit annulus)
16. path16 #ee3300 facet (central cell, largest single facet)
17. path17 #ff6f21 facet (centre-bottom highlight)
18. path18 #231f20 VESICLE (centre left, eqd 56.2, the roundest)
19. path19 #231f20 VESICLE (left, eqd 80.8, largest, boomerang)
20. path20 #ff5721 facet (right flank, largest drawn facet)
21. path21 #231f20 VESICLE (centre, eqd 33.3)
22. path22 #231f20 VESICLE (right, eqd 76.6, kidney)
23. path23 #221f1f interior crack mark (a ~34-unit curved hairline ribbon near x=300, y=47..125, 469 units^2 visible - the only interior line work in the drawing)
24. path24 #231f20 VESICLE (bottom centre, eqd 62.8, aspect 2.37, most elongated)

### What a generator must get right

- The silhouette's lumpiness spectrum is BAND-LIMITED and this is the single most important thing to get right. Harmonics 2 and 3 carry 0.261 and 0.155 of the mean radius (the wide, bottom-heavy overall mass); harmonics 4-12 average 0.027; harmonics 13-40 average 0.004, indistinguishable from flattening error. A generator using fBm noise with a 1/f tail will produce visible crinkle above h12 that this artwork does not have. Build the outline from ~12-16 radial control points interpolated with smooth cubics, not from noise.
- Corners are never sharp. Minimum convex curvature radius anywhere on the silhouette is 19.1 units, 10th percentile 12.6; concave notches bottom out at 12.5. Clamp every boundary radius to >= 12 units (0.026 of width). An angular silhouette is wrong - the OUTLINE is soft and the INTERIOR is angular, the inverse of the usual instinct.
- The outline is a constant-width ribbon, not tapered or calligraphic. 1280 measurements give median 9.96, stdev 0.71, full non-seam range 9.50-10.86, and every 30-degree sector mean between 9.81 and 10.10. Emit it as outer_loop + reversed inward-offset loop in a single fill path with no 'z' and no stroke, exactly as the original. Thickness is 0.0218 of the canvas width; keep that ratio if the canvas size changes.
- Vesicle equivalent diameters are 80.8, 76.6, 62.8, 56.2, 50.9, 33.3, 26.2 - only a 3.08x range, with four of seven in a tight 50-81 band. Do NOT use a steep power law here; v3 used a 9.6x range and reads as a different, pock-marked rock. Match 7 +- 2 vesicles with eqd/width in 0.057-0.177, biased toward the upper half, covering 19-23% of the interior.
- Vesicle lumpiness scales with size: circularity 0.41-0.59 with 4-8 concave lobes for the three largest, 0.70-0.87 with 2-3 lobes for the four smallest. Giving every vesicle the same lobe count is the most obvious tell of a naive generator.
- Keep the two shape families lexically distinct. Vesicle outlines have no segment longer than 53 units and a median 6.6% deviation from chord; facet outlines contain chords up to 145 units straighter than 6%. Curvy holes in an angular mosaic - if either side drifts toward the other the image stops reading as scoria.
- Facets are a crack subdivision with SHARED straight boundaries, not independently placed overlapping blobs: 7 of 53 long edges appear in two adjacent cells agreeing to within 12 units at both endpoints. Generate by recursive chord-splitting of the interior polygon (14 cells, visible-area range 1337-9634, a 7.2x spread), not by scattering shapes. The 'radiating from an off-centre point' model was tested and fails - the best concurrency point misses the edges by a median 95.6 units, no better than the centroid.
- Cut directions are bimodal and STEEP. Length-weighted, 60-75 deg and 105-120 deg carry 432 and 470 units against 65 units for 165-180 deg. Near-horizontal cuts are almost absent, which keeps the eye moving up and down across a wide shape. Draw split directions from two lobes centred near 67 and 113 degrees.
- The light comes from the LOWER LEFT, not the upper left. Along u = (-0.666, +0.746) the normalised mean positions are 0.302 (#ee3300) / 0.486 (#ff5721) / 0.695 (#ff6f21). Assign tone as a noisy projection on that axis with about +-0.17 of quartile overlap between neighbouring tones, holding the area quota near 32% dark / 58% mid / 10% light of the orange. The highlight must stay rare - at 6.6% of the silhouette, spreading it evenly destroys the read.
- Three flat fills only: no gradients, no strokes, no opacity, no filters. All 24 elements are solid-fill <path>s. The oranges are a tight ramp (hue 12.9 / 14.6 / 21.1 deg, V 93 / 100 / 100) warming toward yellow as they lighten; do not widen it into brown-to-amber.
- The shape fills the frame edge to edge: bbox 0.00-456.80 x 0.20-304.60 inside a 457.14 x 304.92 viewBox, under 0.35 units of slack on any side. Fit the viewBox to the generated silhouette after the fact.
- What changed from v3 to v0: v3 was a nearly square rock (canvas aspect 1.213, body 57.2% of canvas) rendered with 76 linear gradients from #7a1e02 to #f7a24a, a hairline black outline of only ~0.0055 of its width (2.1% of body area), and 15 rimmed vesicle holes spanning a 9.6x diameter range with white specular highlights. v0 is landscape (aspect 1.499, body 68.1% of canvas), flat-shaded in three oranges, with a 4x heavier outline (0.0218 of width, 12.8% of body area), and replaces outlined holes with 7 unrimmed solid blobs over a narrow 3.1x size range. v0 is also the LUMPIER outline despite the simpler shading - RMS of harmonics >= 4 is 0.063 against v3's 0.047, and h2 nearly doubles (0.261 vs 0.144). Target v0: flatter colour, heavier line, fewer and bigger holes, wider and lumpier body.
- What I could not measure: the facet-to-facet adjacency graph and the exact order of the crack-splitting recursion are not recoverable from flattened geometry. I measured that 7 shared edges exist and that 53% of cut endpoints die at interior junctions, but the generative tree that produced them is inferred, not observed. Likewise the vesicle orientation finding rests on only 4 elongated samples.

---

## Part 2 — the algorithm

### Fidenza, ported

**The angle field is the only source of direction.** Hobbs builds a 2D grid covering the canvas plus ~50% margin, resolution ~0.5-1% of width, storing one angle per node: angle = 2*PI*lambda*noise(x*f, y*f) + bias. Everything in the piece reads its heading from that one field, so unrelated elements still agree with each other.

  *Here:* ONE field alpha(x,y) per seed, evaluated in body-space normalised by R0. It does three jobs: (1) domain-warps the silhouette's radius lookup, (2) sets each vesicle's long-axis orientation, (3) is traced directly to produce the facet cut curves. Do not introduce a second field for any of them. Evaluate it pointwise - at ~4k lookups per logo a precomputed grid is not worth the cache; keep Hobbs's grid only as an option behind `field_grid: Option<u32>`.

**Curve tracing by fixed-step Euler walk.** From a start point, repeatedly p += h*(cos alpha(p), sin alpha(p)) for num_steps. Hobbs sets h to 0.1-0.5% of image width so no sharp points appear, and notes num_steps is the strongest texture knob: short walks read as fur, long walks as fluid.

  *Here:* h = 0.01*R0 (3 units in canonical space, 0.3% of the 1000-unit canvas - squarely in Hobbs's band). Used only for facet cut curves: trace from the region centroid in BOTH directions until each arm exits the region, max 400 steps per arm. Not used for the silhouette (see blob_shape - a traced open curve cannot be made closed without a seam).

**Collision detection is what creates the density.** Each cSeg is tested against already-placed geometry through a spatial partition (`sctrs`); a growing curve stops when it would touch. Hobbs separately uses circle packing to choose start positions because it is 'pretty evenly spaced out, but with enough random variation that it's more relaxed' than a grid and less clumpy than uniform random.

  *Here:* Verbatim for the vesicles: largest-first placement, uniform-grid broadphase, reject on overlap-plus-gap, plus an edge-clearance test against the eroded body. Measured in the artwork: minimum vesicle-to-vesicle gap 17.4 units (0.103*R0), minimum vesicle-to-inner-edge clearance 11.3 units (0.067*R0, i.e. 1.14x the ribbon width). Those two numbers become the defaults.

**Weighted, curated discrete choices.** Fidenza's `wc()` picks from weighted tables; 31+ named HSL colours grouped into 13+ curated schemes, with rarity tiers. Nothing continuous is left to chance where a human eye would notice the bad half of the range.

  *Here:* Three hand-tuned oranges as the default scheme, measured off the artwork: #ee3300 (13.0deg,100%,46.7%), #ff5721 (14.6,100%,56.5), #ff6f21 (21.2,100%,56.5). Note the family rule: the highlight is NOT lighter than the mid - it is hue-rotated +6.6deg toward yellow at identical L. Generate alternates by moving H0 only, keeping the two deltas.

**A seeded LCG, never the platform RNG.** Fidenza's `rnd()` is a hand-rolled LCG seeded from the token hash, so the same token renders identically 'on different machines at different times'.

  *Here:* Same requirement, stricter bar - we need byte-identical SVG text, so the determinism audit has to extend past the PRNG into libm and float formatting. See the prng and pitfalls sections.

**The parameter surface IS the artwork.** Fidenza exposes turbulence, scale, density, colour scheme, 'spiral' / 'jumbled' / 'super-blocked' and a lot more as named traits, and a single piece is a point in that space. Hobbs's advice is to 'understand them top to bottom, then loosen up'.

  *Here:* 44 named parameters below, each with a default measured off the artwork where measurable, plus one `tameness` scalar that drives seven of them together so a caller who wants 'calmer' does not have to understand the other six.

**Fixed render budget, single pass.** The whole Fidenza render is ~1.2s in p5, single pass, noLoop(). No iterative relaxation, no physics settling.

  *Here:* Single pass, no relaxation. Target is 1-3 ms (roughly 400x less work than Fidenza: ~3k noise evals for the ring, ~5k for the vesicles, ~9k for the cut traces). A 500-logo batch is then under 2 seconds single-threaded.

### Noise

**recommended_kind**

2D Perlin gradient noise (classic Ken Perlin improved-noise), hand-rolled, with a 512-entry permutation table seeded from the PRNG and 8 fixed unit gradients. Wrapped in fbm with configurable octaves/lacunarity/gain.

**rationale**

Hobbs uses Perlin and the look depends on it. Value noise is 10 lines shorter but its gradient discontinuities at cell corners produce axis-aligned creases that are plainly visible as blockiness on a lumpy silhouette at logo scale - the one place we cannot afford an artifact. Simplex buys nothing at 2D (the speed win only matters at 4D+) and costs the skew/unskew machinery. Perlin 2D is ~45 lines, needs no trig, no sqrt and no table of floats beyond 8 hard-coded gradients, so it is trivially bit-exact. The 8 gradients are (1,0),(-1,0),(0,1),(0,-1),(s,s),(s,-s),(-s,s),(-s,-s) with s = 0.70710678118654752440 written as a literal - equal magnitudes, no sqrt, and no directional bias.

**octaves**

Default 4 for the silhouette, 3 for vesicles, 3 for the tone map. Lacunarity 2.03 (NOT 2.0) and gain 0.45. Normalise by the amplitude sum so `lump_amp` means exactly what it says: amp_o = gain^o, F = sum(amp_o * perlin(p * f0 * lac^o + off_o)) / sum(amp_o), giving F in roughly [-1,1]. Octave offsets off_o are drawn from the PRNG in [-4096, 4096] so octaves are decorrelated. Justification for the octave count: the measured radial spectrum of the artwork is k=3 -> 0.131R, k=6 -> 0.034R, k=9..11 -> ~0.03R, and everything from k=12 up is below 0.01R - i.e. four meaningful octaves from a base of ~2.6 cycles/turn (2.6, 5.3, 10.8, 21.9) and nothing beyond. A 5th octave would only add sub-0.5-unit wobble that the 3-decimal output quantises away.

**zero_dep_feasible**: True

**implementation_sketch**

fn fade(t) = t*t*t*(t*(t*6.0 - 15.0) + 10.0)   // quintic, C2 continuous - the cubic 3t^2-2t^3 shows second-derivative seams on a smooth silhouette
fn lerp(a,b,t) = a + t*(b-a)
fn grad(h: u8, x, y) -> f64 { let g = GRAD8[(h & 7) as usize]; g.0*x + g.1*y }

fn perlin(perm: &[u8;512], x: f64, y: f64) -> f64 {
  let xi = x.floor(); let yi = y.floor();
  let xf = x - xi;    let yf = y - yi;
  let X = (xi as i64 & 255) as usize; let Y = (yi as i64 & 255) as usize;
  let u = fade(xf); let v = fade(yf);
  let aa = perm[perm[X]   as usize + Y];     let ab = perm[perm[X]   as usize + Y + 1];
  let ba = perm[perm[X+1] as usize + Y];     let bb = perm[perm[X+1] as usize + Y + 1];
  let x1 = lerp(grad(aa, xf, yf),       grad(ba, xf-1.0, yf),       u);
  let x2 = lerp(grad(ab, xf, yf-1.0),   grad(bb, xf-1.0, yf-1.0),   u);
  (lerp(x1, x2, v) * 1.41421356237309504880).clamp(-1.0, 1.0)   // *sqrt(2): raw 2D Perlin peaks at +-0.7071
}

// permutation: identity 0..255, Fisher-Yates with the PRNG, then duplicate into [512].
// Use `for i in (1..256).rev() { let j = rng.below(i as u32 + 1) as usize; perm.swap(i, j); }`
// rng.below MUST be the unbiased Lemire/rejection form, not `% n` - `% n` is still deterministic
// but it skews the table and makes the noise subtly directional.

// PERIODIC fbm in theta - the whole trick for a seamless closed silhouette:
fn fbm_ring(theta_cos: f64, theta_sin: f64, p: &Params, perm: &[u8;512]) -> f64 {
  let (mut f, mut a, mut sum, mut acc) = (p.lump_freq, 1.0, 0.0, 0.0);
  for o in 0..p.lump_octaves {
    let (ox, oy) = p.off[o];
    // sample on a CIRCLE of radius f in noise space -> exactly periodic in theta, no seam, ever
    acc += a * perlin(perm, theta_cos * f + ox, theta_sin * f + oy);
    sum += a; a *= p.gain; f *= p.lacunarity;
  }
  acc / sum
}

// Flow field, in body-space units of R0:
fn field_dir(p: &Params, perm: &[u8;512], x: f64, y: f64) -> (f64, f64) {
  let n = fbm2(perm, x * p.field_freq, y * p.field_freq, 3, p);   // in [-1,1]
  let a = p.field_bias + PI * p.field_angle_range * n;
  (cos_poly(a), sin_poly(a))
}

### PRNG and determinism

**recommended**

PCG32 (XSH-RR, 64-bit state / 64-bit increment) seeded through SplitMix64, with an unbiased bounded-integer draw and an exact 53-bit float draw. ~30 lines, pure u64 wrapping arithmetic, no floats in the state.

**rationale**

Hobbs's LCG is adequate but a bare LCG has correlated low bits, and we draw a LOT of low-entropy decisions (an octave offset, then a vesicle radius, then a grid cell) where the correlation shows up as visible structure - e.g. vesicle sizes drifting with position. PCG32's output permutation fixes that for 10 extra lines and no measurable cost. SplitMix64 on the way in means seed 0, 1, 2 give completely unrelated images, which matters a lot for a batch generator where people will use sequential seeds. All arithmetic is `wrapping_mul`/`wrapping_add` on u64 - identical on every target. Crucially the state never touches a float, so there is no rounding-mode or x87 exposure. Draw streams must be consumed in a fixed order: give each subsystem its OWN PCG stream (different increment), seeded as `split(seed ^ CONST_BLOB)`, `... ^ CONST_VESICLE`, `... ^ CONST_FACET`, `... ^ CONST_TONE`, so that changing `vesicle_count` does not reshuffle the silhouette. That stream separation is the thing that makes a parameter surface explorable.

**sketch**

struct Pcg32 { state: u64, inc: u64 }

fn splitmix64(x: &mut u64) -> u64 {
  *x = x.wrapping_add(0x9E3779B97F4A7C15);
  let mut z = *x;
  z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
  z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
  z ^ (z >> 31)
}

impl Pcg32 {
  fn new(seed: u64, stream: u64) -> Self {
    let mut s = seed;
    let st = splitmix64(&mut s);
    let mut r = Pcg32 { state: 0, inc: (stream << 1) | 1 };
    r.next_u32(); r.state = r.state.wrapping_add(st); r.next_u32(); r
  }
  fn next_u32(&mut self) -> u32 {
    let old = self.state;
    self.state = old.wrapping_mul(6364136223846793005).wrapping_add(self.inc);
    let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
    let rot = (old >> 59) as u32;
    xorshifted.rotate_right(rot)
  }
  fn next_u64(&mut self) -> u64 { ((self.next_u32() as u64) << 32) | self.next_u32() as u64 }
  /// exact, platform-independent: 53 significand bits scaled by 2^-53
  fn f64(&mut self) -> f64 { ((self.next_u64() >> 11) as f64) * (1.0 / 9007199254740992.0) }
  fn range(&mut self, lo: f64, hi: f64) -> f64 { lo + (hi - lo) * self.f64() }
  /// unbiased bounded int (Lemire). NEVER use `% n`.
  fn below(&mut self, n: u32) -> u32 {
    let mut m = (self.next_u32() as u64) * (n as u64);
    let mut l = m as u32;
    if l < n {
      let t = n.wrapping_neg() % n;
      while l < t { m = (self.next_u32() as u64) * (n as u64); l = m as u32; }
    }
    (m >> 32) as u32
  }
}

// --- the other half of determinism: trig. f64 +,-,*,/,sqrt are IEEE-754 exact and identical
// everywhere; sin/cos/atan2/powf are libm and DIFFER between glibc, musl, MSVC and Apple.
// Hand-roll them. Degree-11 odd minimax sin on [-PI/4, PI/4] after octant reduction:
const S1: f64 = -1.66666666666666307295e-1; const S2: f64 =  8.33333333332211858878e-3;
const S3: f64 = -1.98412698295895385996e-4; const S4: f64 =  2.75573136213857245213e-6;
const S5: f64 = -2.50507477628578072866e-8; const S6: f64 =  1.58962301576546568060e-10;
fn sin_kernel(x: f64) -> f64 { let z = x*x;
  x + x*z*(S1 + z*(S2 + z*(S3 + z*(S4 + z*(S5 + z*S6))))) }
const C1: f64 =  4.16666666666666019037e-2; const C2: f64 = -1.38888888888741095749e-3;
const C3: f64 =  2.48015872894767294178e-5; const C4: f64 = -2.75573143513906633035e-7;
const C5: f64 =  2.08757232129817482790e-9; const C6: f64 = -1.13596475577881948265e-11;
fn cos_kernel(x: f64) -> f64 { let z = x*x;
  1.0 - 0.5*z + z*z*(C1 + z*(C2 + z*(C3 + z*(C4 + z*(C5 + z*C6))))) }
// range-reduce: n = (x * (2/PI)).round() as i64 (f64::round is exact, not libm);
// r = x - n*PI_HI - n*PI_LO (two-part PI for accuracy); dispatch on n & 3.
// Accuracy ~1 ulp, which is far beyond what 3-decimal SVG output can show.

// atan2 is NOT needed anywhere - replace it with the exact 'diamond' pseudo-angle, which is
// strictly monotone in theta and uses only abs and one divide:
fn pangle(dx: f64, dy: f64) -> f64 {  // -> [0, 4), monotone with atan2
  let t = dy / (dx.abs() + dy.abs() + 1e-300);
  if dx < 0.0 { 2.0 - t } else if dy < 0.0 { 4.0 + t } else { t }
}
// powf is not needed either: the vesicle size law uses an integer exponent (u*u), so it is
// plain multiplication.

### The blob

**method**

POLAR RADIUS WITH DOMAIN-WARPED PERIODIC fBm over an elliptical base. Not a noise-advected circle, and not a traced flow curve closed back on itself - both of those can self-intersect and neither can be guaranteed closed without a seam. The polar form buys a theorem (see self_intersection_guard) and makes the ribbon offset exact (see outline_ribbon), which is worth more than the extra wiggle freedom.

Construction, in canonical space (canvas 1000 wide, R0 = 300, centre C):

1. BASE ELLIPSE. semi-axes a = R0*sqrt(aspect), b = R0/sqrt(aspect) so the geometric mean radius is exactly R0 for any aspect. rho(theta) = a*b / sqrt((b*cos t)^2 + (a*sin t)^2).  [measured: a~228, b~152 on R0=169 -> aspect 1.50, which is also the canvas aspect]

2. SAMPLE RING. N = 720 samples, theta_i = 2*PI*i/N, with ct = cos_poly(theta_i), st = sin_poly(theta_i).

3. DOMAIN WARP (this is where the flow field enters). Do NOT move the geometric point - move the NOISE LOOKUP. Evaluate the field at the unwarped boundary estimate q_i = C + rho_i*(ct, st), scaled to R0 units, get (dx, dy) = field_dir(q_i/R0), and offset the noise-space sample:
     u_i = ct*lump_freq + warp_amt*dx ,   v_i = st*lump_freq + warp_amt*dy
   Then F_i = fbm over octaves at (u_i, v_i) as in the noise sketch (each octave multiplies the circle radius by lacunarity and adds its own offset; the warp term is added once, unscaled, so it shears the low octaves most - which is what makes lumps LEAN along the field instead of just getting noisier).

4. RADIUS. r_i = rho_i * (1.0 + lump_amp * F_i).  [measured lump_amp 0.22: relative radial std 0.25, Rmin/Rmean 0.52, Rmax/Rmean 1.40]

5. LOG-LIPSCHITZ LIMITER (section b).

6. RESAMPLE TO UNIFORM ARCLENGTH. Walk the ring accumulating chord length, then emit M = 256 points at equal arclength by linear interpolation. This is not cosmetic: uniform Catmull-Rom overshoots and loops exactly where sample spacing changes fast, which on a polar blob is every deep notch. Resampling removes the failure mode instead of patching it. After resampling, re-record each point's pseudo-angle for the binary-searchable inside-test.

7. CATMULL-ROM -> CUBIC BEZIER, closed, indices mod M.

Why this beats a noise-advected circle: advection (p_i += h*field(p_i), iterated) accumulates until neighbouring points cross, and the crossing is invisible until you fill the path. Why it beats tracing a long flow curve back to its start: the join is a tangent discontinuity you then have to hide, and the 'closes up' condition is not something you can guarantee - you can only retry until it happens, which is slow and non-uniform across the parameter space.

**smoothing**

UNIFORM CATMULL-ROM, converted segment-by-segment to cubic Bezier, after the arclength resample.

For the segment from P_i to P_{i+1}, with neighbours P_{i-1} and P_{i+2} (all indices mod M):

    B0 = P_i
    B1 = P_i     + tau * (P_{i+1} - P_{i-1}) / 3
    B2 = P_{i+1} - tau * (P_{i+2} - P_i)     / 3
    B3 = P_{i+1}

with tau = 0.5, which is the Catmull-Rom cardinal scale. Substituting tau = 0.5 gives the familiar EXACT coefficient 1/6:

    B1 = P_i     + (P_{i+1} - P_{i-1}) / 6
    B2 = P_{i+1} - (P_{i+2} - P_i)     / 6

Sanity check that this is right: on a circle of radius R sampled every dtheta, the handle length is |P_{i+1} - P_{i-1}|/6 = (2R*sin(dtheta))/6 ~= R*dtheta/3, and the exact circular-arc Bezier handle is (4/3)*tan(dtheta/4)*R ~= R*dtheta/3. They agree to O(dtheta^3), so a circle comes out visually exact at M=256 (max radial error ~2e-7 * R, i.e. 6e-5 units - two orders below the 0.001 output quantum).

Expose tau in [0.35, 0.60]. tau below 0.5 tightens toward the polyline (crisper, more 'cut'); above 0.55 the handles start to bulge at high-curvature samples. Default 0.5.

If an implementer skips the arclength resample (step 6), they MUST use centripetal Catmull-Rom instead, Barry-Goldman form with alpha = 0.5:
    d1 = |P_i - P_{i-1}|^0.5 ,  d2 = |P_{i+1} - P_i|^0.5 ,  d3 = |P_{i+2} - P_{i+1}|^0.5
    B1 = ( d1*d1*P_{i+1} - d2*d2*P_{i-1} + (2*d1*d1 + 3*d1*d2 + d2*d2)*P_i ) / (3*d1*(d1 + d2))
    B2 = ( d3*d3*P_i     - d2*d2*P_{i+2} + (2*d3*d3 + 3*d3*d2 + d2*d2)*P_{i+1} ) / (3*d3*(d3 + d2))
Centripetal CR is provably cusp-free and loop-free for any point sequence, which is why it is the correct fallback. Guard d1,d2,d3 >= 1e-9. Recommendation stands on the uniform form plus resampling: fewer divides, and the exact-circle property above.

HANDLE CLAMP (always on, both forms). Let d = |P_{i+1} - P_i|. If |B1 - P_i| > 0.5*d, rescale B1 toward P_i to length 0.5*d; same for B2 toward P_{i+1}. On well-behaved data the ratio is 1/3, so the clamp never fires; when it does it has caught data that would have produced a loop.

**self_intersection_guard**

THE THEOREM: a curve written as r(theta)*(cos theta, sin theta) with r(theta) > 0 single-valued is star-shaped about C, and a star-shaped curve cannot self-intersect - every ray from C meets it exactly once. So the guard is not a cleanup pass, it is an invariant maintained by construction. There are exactly four ways to break it, and each has one specific defence:

1. r <= 0. Clamp r_i = max(r_i, 0.25*R0) BEFORE the limiter. With lump_amp <= 0.35 and the normalised fbm in [-1,1], r >= 0.65*rho anyway, so this only fires on an abusive parameter set.

2. The warp makes r multi-valued. It cannot, because the warp is applied to the NOISE-SPACE sample point, not to the output geometry. r stays a scalar function of theta. This is the single most important design decision in the whole blob construction and it is why I warp the lookup rather than advecting the points.

3. Too-steep radial slope produces a near-cusp that the Bezier turns into a loop. Defence: a LOG-LIPSCHITZ LIMITER enforcing

       | d(ln r)/d(theta) |  <=  kappa          (default kappa = 1.10)

   equivalently |r'| <= kappa*r, i.e. the boundary never makes an angle steeper than atan(kappa) = 47.7deg with the radial direction. Implement as two wrap-around sweeps per direction over the sampled ring, which converges because the constraint is a one-sided Lipschitz bound on ln r:

       let g = kappa * (2.0*PI / N as f64);          // per-sample budget
       let cap = 1.0 + g;   let flo = 1.0 / (1.0 + g);
       // erode spikes: forward then backward, twice
       for _ in 0..2 {
         for i in 0..N { let j = (i + N - 1) % N; r[i] = r[i].min(r[j] * cap); }
         for i in (0..N).rev() { let j = (i + 1) % N; r[i] = r[i].min(r[j] * cap); }
       }
       // fill notches: same, the other way
       for _ in 0..2 {
         for i in 0..N { let j = (i + N - 1) % N; r[i] = r[i].max(r[j] * flo); }
         for i in (0..N).rev() { let j = (i + 1) % N; r[i] = r[i].max(r[j] * flo); }
       }

   O(N), branch-free in the inner loop, strictly monotone (erosion only lowers, dilation only raises, and after the pair both bounds hold to within one pass). TRAP TO DOCUMENT: the base ellipse itself has max|r'/r| = (aspect^2 - 1)/(2*aspect) - 0.417 at the default aspect 1.50, 0.75 at aspect 2.0. If kappa is set below that, the limiter silently circularises the blob and the aspect parameter stops working. Enforce kappa >= (aspect^2 - 1)/(2*aspect) + 0.40 at parameter-validation time and say so in the error.

4. The Bezier interpolation loops between samples even though the polyline is clean. Defence: the handle clamp above, plus the turn-angle check - if the angle between consecutive chords exceeds 60deg anywhere, M was too low; double M and rebuild (at M=256 on a kappa=1.10 blob the measured maximum turn is ~14deg, so this never fires at defaults).

The inner ribbon contour needs its own version of the same invariant - see outline_ribbon.

**flow_field_influence**

The field appears in three places and nowhere else, which is what makes one seed look like one rock rather than three unrelated effects.

(1) SILHOUETTE: warps the fbm lookup (step 3 above) by warp_amt * (cos alpha, sin alpha). Effect: lumps shear and lean in a consistent direction, so the blob reads as a thing that flowed and froze - which is physically what scoria is - rather than as a cloud. At warp_amt = 0 you get isotropic lumpiness; at 0.45 the silhouette visibly combs one way. Default 0.22.

(2) VESICLES: each vesicle's long axis is set to alpha at its own centre, blended by vesicle_align (default 0.85) against a per-vesicle random angle. This is the 'shares the field's character' requirement, and it is what makes the big elongated holes in the artwork (circularity 0.405, 0.458) look like gas that stretched in a flow rather than like random smears. The vesicle's own fbm is sampled on its own small circle with a per-vesicle noise offset, so shapes differ, but the orientation is coherent across the whole body.

(3) FACET CUTS: the cut curve IS a flow-field trace (Hobbs's Euler walk), so facet boundaries curve the same way the lumps lean. This is the strongest single contributor to the 'deliberate' look, because a facet edge that parallels the nearest silhouette bulge reads as structure, and one that cuts across it reads as damage.

Field parameters: field_freq 0.55 per R0 (so roughly one full angular swing across the body - low frequency is right; at 1.5 the facet cuts turn into squiggles), field_angle_range 1.0 mapping fbm [-1,1] to alpha in [bias - PI, bias + PI]. Lower field_angle_range (0.35) gives Hobbs's 'combed' directional field; that is the knob for a tamer, more graphic rock. Hobbs's non-continuous variant is worth exposing too: snapping alpha to multiples of PI/6 gives exactly the 'sculpted, rocky forms' he describes, and is one line - `alpha = (alpha / step).round() * step` behind `field_quantize: Option<f64>`, default None.

### The outline ribbon

**recommended**

EXPLICIT FILLED RIBBON as a two-subpath compound path with fill-rule="evenodd" and the inner subpath wound opposite to the outer - but generated by RADIAL OFFSET WITH SLOPE CORRECTION, not by any general polygon-offsetting algorithm.

The construction, in full:

1. Perpendicular half-widths. Target perceived stroke width w0 = outline_weight * R0 = 0.058 * 300 = 17.4 canonical units. [measured: ribbon area 12507 over centreline length ~1263 gives 9.90 units on R0 = 169.2, i.e. 0.0585 * R0]

2. Radial derivative from the sampled ring, central difference with wraparound, dtheta = 2*PI/N:
       rp_i = (r[(i+1) % N] - r[(i+N-1) % N]) / (2.0 * dtheta)

3. SLOPE CORRECTION - the reason a naive radial offset looks wrong. The boundary normal makes an angle phi with the radial direction where tan(phi) = rp/r, so a radial step of w produces a PERPENDICULAR band of only w*cos(phi) = w/sqrt(1 + (rp/r)^2). To get a visually even band you must therefore inflate the radial step:

       k_i = sqrt(1.0 + (rp_i / r_i) * (rp_i / r_i))       // in [1, sqrt(1+kappa^2)]
       w_i = w0 * k_i.min(1.60)

   At the default kappa = 1.10 the limiter already bounds k_i <= sqrt(1 + 1.21) = 1.487, so the 1.60 cap is a safety net that never fires - the two constants were chosen to interlock. Without this correction the band visibly thins on every steep flank, which is exactly the tell that says 'generated'.

4. Outer contour: r_out_i = r_i. Inner contour: r_in_i = max(r_i - w_i, 0.15 * R0). Both are polar functions of the SAME theta grid, so both are star-shaped by the same theorem, so NEITHER can self-intersect and they cannot cross each other (r_in < r_out pointwise by construction). This is the whole argument for the polar silhouette: offsetting becomes a subtraction.

5. Run the log-Lipschitz limiter a second time on r_in alone (erosion pass only). Subtracting a varying w_i can locally steepen the inner curve past kappa even though r_out was compliant.

6. Arclength-resample each contour independently (the inner one is shorter and wants fewer points - M_in = max(128, M_out * len_in/len_out)), Catmull-Rom each to Bezier, emit as:
       <path id="outline" fill-rule="evenodd" fill="#231f20" d="M... C... Z M... C... Z"/>
   with the outer subpath CW and the inner CCW (reverse the inner point array before emitting). Writing both the correct winding AND fill-rule="evenodd" is belt and braces: the path then renders identically under either fill rule, which protects you from downstream tools that drop the attribute.

7. Z-order: facets, then vesicles, then the outline last. The ribbon on top hides the facet tiling's outermost seams.

Optional second rim: the artwork actually carries a second, thinner concentric ribbon (path 13, width 2.49 units = 0.0147 * R0) just inside the main one. Expose it as `inner_rim` (default 0.0, set 0.015 to match the artist) built by the identical construction at r_in and r_in - inner_rim*R0*k_i.

**rationale**

Against `stroke-width` (simplest, and wrong here): a stroke is not geometry. In Inkscape the user cannot node-edit the outline's shape independently of the body, non-uniform scaling distorts the stroke, and - fatally - a 17-unit stroke on a boundary with a notch narrower than 35 units self-overlaps into a pinch or a bowtie at the join, which is precisely the glitch the brief forbids. It also disagrees with the artist's own file, which uses a filled ribbon, so output would not drop into her working file cleanly.

Against generic polygon offsetting (Clipper-style, or marching along normals): correct offsetting requires detecting and removing self-intersection loops at concavities whose radius of curvature is smaller than the offset distance. That is the error-prone part the brief flags, it needs a robust segment-intersection and loop-removal pass (hundreds of lines, numerically delicate, and a determinism hazard because the loop-removal order depends on float comparisons), and zero-dep means writing all of it.

The radial offset sidesteps the entire problem class. r_in(theta) = r(theta) - w(theta) is a scalar subtraction on a single-valued function; there is no topology to resolve, no loops to remove, no ordering to get wrong, it is O(N), and non-self-intersection is inherited from the same star-shapedness theorem that protects the outer curve. The slope correction recovers the one thing the naive radial offset loses - even perceived width - exactly, in closed form, for one sqrt per sample.

The residual inaccuracy is that a radial offset is not a true constant-distance offset in the second order (the band is very slightly wider on convex flanks than concave ones, by O(w^2 * curvature)). At w0/R0 = 0.058 that error is under 0.3% of the band width - about 0.05 units, well below the 0.001-unit output quantum times any plausible viewing scale. It is invisible, and it is the correct trade for provable cleanliness.

**Options weighed**

- stroke-width on the body path - 1 line, but not editable as geometry, distorts under non-uniform scale, and self-overlaps at any notch narrower than 2*w0. Rejected.
- Stroke-then-outline via a general offsetter with loop removal - correct in principle, several hundred lines of delicate geometry, a determinism hazard in the loop-removal ordering, and the failure mode is a visible bowtie. Rejected on zero-dep and robustness.
- Radial offset r - w(theta) with the slope correction w = w0*sqrt(1 + (r'/r)^2), emitted as a two-subpath even-odd compound path. O(N), provably non-self-intersecting, exactly even perceived width, fully node-editable in Inkscape. RECOMMENDED.
- Two independent fbm contours (outer and inner generated separately) - gives a hand-drawn varying-weight line, but the two can cross and the width is uncontrolled. Only viable as a deliberate 'wobbly ink' style behind a flag, with the crossing test as a reject.

### Vesicle packing

**algorithm**

Fidenza's collision loop, largest-first. (1) Draw `vesicle_count` target radii up front, r_k = r_min + (r_max - r_min) * u_k^p with p = 2 (integer power - no powf, and the square biases toward small, which matches the measured distribution: r_eff 13.1, 16.7, 25.5, 28.1, 31.4, 38.3, 40.4 on R0=169, i.e. clustered low with two big ones). (2) Sort descending with `sort_by(|a,b| b.total_cmp(a).then(ia.cmp(&ib)))` - largest-first placement is what lifts achieved coverage from ~12% to the measured ~21% without any relaxation pass. (3) For each radius in turn, up to `attempts_per = 220` tries: sample a centre uniformly in the body's bounding box, reject immediately if it fails the inside-test, then build the candidate's own polar blob (its shape depends on its centre, because the long axis follows the field there - so you must build before you can test), then run the clearance and overlap tests. On success, insert into the grid and emit. On exhaustion, drop that vesicle and continue to the next smaller one. (4) Stop early on `consec_fail_stop = 150` consecutive global failures or when `attempts_total = 2000` is spent. Dropping rather than shrinking is deliberate: a shrunk-to-fit vesicle reads as dirt, a missing one reads as composition.

**collision_test**

Three tests, cheapest first.

A) INSIDE / EDGE CLEARANCE, against the body. The body is star-shaped about C by construction, so this is O(1) per probe with no polygon walk: for a probe point q, compute pa = pangle(q.x-C.x, q.y-C.y), binary-search the ring's precomputed monotone pseudo-angle array to get the bracketing indices (i, i+1), linearly interpolate r_body between r_i and r_{i+1} in pseudo-angle, and require |q-C| <= r_body - clearance. Probe the candidate's own M=24 boundary samples plus its centre. clearance = max(1.2*w0, 0.055*R0) = max(21.0, 16.5) = 21.0 canonical units; the artwork's tightest is 11.3/169*300 = 20.1, so 21.0 is the measured floor rounded up.

B) BROADPHASE, uniform grid. cell = 2 * r_max_vesicle (156 canonical units); grid covers the body bbox; `cells: Vec<Vec<u16>>` indexed by iy*nx+ix, never a HashMap. A candidate with bounding radius Rb at centre c queries every cell overlapped by the square [c - (Rb + r_max + gap), c + (Rb + r_max + gap)] - typically 4 cells, at most 9. Insert the accepted vesicle into every cell its own bbox touches (duplicates across cells are fine; dedupe is not needed because the narrowphase is idempotent).

C) NARROWPHASE, star-shaped blob vs star-shaped blob. Both vesicles are polar about their own centres, so an exact-enough separation test along the centre line costs two radius lookups: let d = |c_j - c_i|, phi_ij = direction from c_i to c_j, phi_ji = its negation. Collide iff d < Ri(phi_ij) + Rj(phi_ji) + gap. Because a lumpy blob is not convex this can miss a grazing contact on a side lobe, so make it strictly safe by taking the MAX radius over a +-18deg window (5 samples, stride 9deg) around each direction - 10 lookups total, still O(1), and conservative (it can only over-reject). gap = max(0.09*R0, 0.45*min(ri,rj)) = max(27.0, ...) canonical; measured minimum pairwise gap is 17.4/169*300 = 30.9, consistent.

**acceleration**

One uniform grid, `Vec<Vec<u16>>`, cell size 2*r_max. No quadtree, no k-d tree, no BVH - with 7 to 20 items the tree build costs more than the brute force it replaces, and the grid is the structure that stays O(1) if someone cranks `vesicle_count` to 60. A second, independent uniform grid of cell size 3*w0 is built over the ring's own segments and reused by the self-intersection quality check; share the grid struct, not the instance. Determinism note: cell contents are Vec-ordered by insertion, so iteration order is the placement order on every platform.

**size_distribution**

r_k = r_min + (r_max - r_min) * u^2, u ~ U(0,1), with r_min = 0.070*R0 = 21.0 and r_max = 0.260*R0 = 78.0 canonical units. The 3.71x radius ratio reproduces the measured 3.1x equivalent-radius span with a little headroom; the apparent ~7x the eye reads in the artwork comes from ANISOTROPY, not from the radius law - the measured max-caliper span is 42.4 to 136.0 because the big vesicles are elongated (circularity 0.405 and 0.458) while the small ones are compact (0.796, 0.872). So couple aspect to size: aspect_k = 1.0 + (vesicle_aspect_max - 1.0) * (r_k - r_min)/(r_max - r_min) * rng.range(0.6, 1.0). Big ones stretch, small ones stay round, and the caliper range lands near 7x on its own. Target total vesicle area 0.12..0.26 of body area (measured 0.207) - this is a quality gate, not an input.

**failure_budget**

attempts_per_vesicle = 220, attempts_total = 2000, consec_fail_stop = 150. Rationale: at the measured 21% coverage with clearance, the acceptance rate for the LARGEST vesicle in an already-seeded body is empirically around 1 in 40 to 1 in 120, so 220 gives it roughly a 90% chance; past that the body genuinely has no room and more tries only burn time. The total budget caps the worst case at about 0.4 ms. If fewer than `ceil(0.6 * vesicle_count)` vesicles were placed the whole logo is rejected and re-rolled from the retry budget - a scoria with two holes does not read as scoria.

### Facets

**recommended**

RECURSIVE FLOW-FIELD BISECTION OF THE BODY REGION, always splitting the largest remaining region, then assigning the three tones by area quantiles of a light function. Default 11 cuts -> 12 regions. [measured: the artwork has 14 facet paths in 3 tones, several of which are adjacent same-tone, so the perceived facet count is ~9]

FULL ALGORITHM.

STATE: a Vec<Region>, each Region a closed polyline (Vec<[f64;2]>), seeded with ONE region - the body ring dilated by +w0/2 (see clipping, point 5). Regions are never holes and never multiply-connected, which is the property the whole scheme rests on.

PER CUT (repeat facet_cuts times):
  1. Pick the region with the largest |signed area|; tie-break on index. Always-largest-first is what produces the deliberate, well-proportioned look - random-region selection produces one huge region and a litter of slivers, which is the classic BSP failure.
  2. Seed point G = region area-centroid (the polygon centroid, not the vertex mean - the vertex mean drifts toward dense sampling), jittered by rng.range(-0.18, 0.18) * R_region in each axis, where R_region = sqrt(|A| / PI). Reject and re-jitter if G falls outside the region (use the crossing-number test; the region is not star-shaped so the fast polar test does not apply here).
  3. Trace the cut in BOTH directions from G with Hobbs's Euler walk, h = 0.01 * R0 = 3.0 units:
        forward:  p += h * facet_curviness_blend(field_dir(p/R0), straight_dir)
        backward: p -= h * (same, evaluated at the new p)
     where straight_dir is the field direction frozen at G, and facet_curviness in [0,1.5] lerps between the frozen direction (0 = a perfectly straight chord) and the live field (1 = a full flow curve); above 1.0 it over-rotates for a more agitated cut. Stop each arm when the segment p->p_next crosses the region boundary; solve the crossing exactly by 2D segment-segment intersection and keep the parameter. Abort the cut if an arm exceeds 400 steps without exiting.
  4. Concatenate reverse(backward arm) + [G] + forward arm -> the cut polyline, with exact crossing points at both ends.
  5. VALIDATE the cut (any failure -> re-roll the jitter, up to 6 attempts, then skip this cut and move on):
       - exactly 2 boundary crossings total (each arm exits once). An arm that re-enters is detected by counting crossings along the whole arm, not just the last step.
       - the cut polyline does not self-intersect: cheap sufficient test, total absolute turning < 220deg; exact test via the segment grid if you want it.
       - the two crossing points are at least 0.08 * region_perimeter apart along the boundary, measured the short way. This is the anti-sliver rule.
       - neither child's area < facet_min_area * body_area (default 0.015). [measured smallest facet in the artwork: 1755/90648 = 1.9% of body area]
  6. SPLIT. With crossing points X_a (between boundary indices ia, ia+1) and X_b (between ib, ib+1), and ia < ib:
        child1 = [X_a] + B[ia+1 ..= ib] + [X_b] + reverse(cut interior)
        child2 = [X_b] + B[ib+1 ..] + B[.. = ia] + [X_a] + cut interior
     Replace the parent with child1 and push child2. Both children are closed polylines built from original boundary points plus the cut - they compose, so the recursion works on children exactly as on the root, which is the property that makes this a 20-line split and not a clipping library.

TONE ASSIGNMENT:
  L_r = (1 - tone_noise) * ( u_light . (centroid_r - C) / R0 ) + tone_noise * fbm3(centroid_r / R0 * 0.9)
  with u_light = (cos light_dir, sin light_dir), light_dir default 1.90 rad. [measured: the artwork's lightest tone sits at the bottom edge (y 208-298 of 305) and the darkest at top and right - light from below, which is unusual and is exactly why the thing reads as glowing lava rather than as a lit pebble. Keep it.]
  Sort regions by L_r ascending with an index tie-break and `total_cmp`. Walk the sorted list accumulating area and assign tone 0 until 0.30 of body area is covered, tone 1 until 0.90, tone 2 for the rest. [measured tone area split: #ee3300 27026 / #ff5721 54640 / #ff6f21 8982 of 90648 = 0.298 / 0.603 / 0.099]
  AREA QUANTILES, not value thresholds. A threshold on L_r can produce an output with zero highlight facets; quantiles cannot. That is a one-line choice that eliminates a whole class of bad output.

OPTIONAL MERGE: adjacent same-tone regions can be left as separate paths (simpler, and Inkscape users can recolour one facet) or merged. Recommend NOT merging - the path count stays ~12 either way and merging needs boundary-adjacency bookkeeping for no visual gain.

**rationale**

Weighing the four candidates the brief names:

- Noise-thresholded tone map quantised to 3 levels. This is the most physically convincing and the most expensive: you need marching squares on a sampled grid, then contour tracing, then contour-to-Bezier fitting, then clipping the contours to the body. Worse, iso-contours of a noise field routinely produce hairline necks and detached specks at the threshold - precisely the slivers the brief forbids - so you then need an area filter and a neck filter, and the output is 40+ paths. Rejected on both cleanliness and code size.

- Voronoi-ish subdivision clipped to the body. Needs a Voronoi/Delaunay implementation (Fortune or Bowyer-Watson, both nontrivial and both determinism-sensitive in their degeneracy handling) AND a general clipper for the body boundary. It is also the wrong LOOK: Voronoi cells are convex and uniformly sized, which reads as a cracked mud flat, not as the long sweeping planes of a fractured rock.

- Straight chords across the body. Right look family, trivially clean, and it is partly what the artist did - her facet paths do contain straight `l` segments (4 in path 1, 3 in path 2) mixed with curves. But pure straight chords read as a low-poly render, and a straight chord through a concave blob frequently produces 4+ crossings, so you need the general multi-component split anyway.

- Flow-field-traced bisection. RECOMMENDED because it is straight chords plus the one thing that makes them belong to this rock: `facet_curviness = 0` degenerates exactly to straight chords, so it is a strict superset of the option above. The crossing count is 2 by construction (each Euler arm exits once and is validated), so the split never needs multi-component face tracing. It reuses the field, the noise and the Euler walk that already exist for the silhouette - zero new machinery. Cost is ~9k field evaluations per logo, under 0.3 ms. And always-split-the-largest gives the balanced, intentional proportions that make faceting look designed rather than shattered.

**clipping**

NO CLIPPING IS PERFORMED, AND NONE IS NEEDED. That is the point of the construction and it is worth stating explicitly to the implementer, because the instinct is to reach for a clipper.

Every facet's boundary consists of exactly two kinds of piece: (a) ARCS OF THE BODY BOUNDARY, copied vertex-for-vertex from the parent region's point array, and (b) THE CUT CURVE, which was traced from an interior point and stopped at the exact moment it crossed the boundary. Piece (a) is on the body by definition. Piece (b) is inside the body by the stopping condition. So the facet is inside the body, exactly, with no computation - the containment is a property of how the points were obtained, not something verified afterward.

The five consequences the implementer must honour to keep it true:

1. COPY, NEVER RE-EVALUATE. A child region's boundary arc must be the parent's actual f64 vertices, memcpy'd. If you instead re-sample the analytic r(theta) at the child's angles you get a different value in the last bits, and adjacent facets develop sub-pixel gaps that antialias into visible hairlines. This is the single most likely implementation bug.

2. SHARE THE CROSSING POINTS. X_a and X_b are computed once and stored in BOTH children bit-identically. Do not recompute the intersection per child.

3. SHARE THE CUT. child2 gets the cut array, child1 gets its reverse - same f64 values in opposite order, so the shared edge is geometrically identical.

4. SMOOTH CONSISTENTLY ACROSS THE SHARED EDGE. Both children run the same Catmull-Rom with the same tau, but the neighbour points differ at the junctions, so the two Beziers along a shared edge will differ slightly near the ends. Fix: smooth each PIECE independently with clamped end tangents (the first and last handle of a piece uses the one-sided difference, not the wrapped neighbour), and reverse the already-smoothed cut for child1. Then the shared edge is literally the same cubic, reversed, and no seam can exist. Reversing a cubic is B0..B3 -> B3,B2,B1,B0.

5. OVERSIZE AGAINST THE RIBBON. Build the facet root region on the ring DILATED by +w0/2 (r_i + 0.5*w_i), and draw the ribbon last. The facets then run half a ribbon-width past the silhouette and the ribbon covers the overshoot, so no antialiasing hairline can appear between the facet tiling and the outline no matter how a renderer rounds. This costs one extra array and removes the last seam class.

Facets do NOT need to avoid the vesicles - vesicles are opaque and painted after.

### Parameters

| name | type | default | range | effect |
|---|---|---|---|---|
| `seed` | `u64` | `0` | any u64 | The whole image. Passed through SplitMix64 and split into 5 independent PCG32 streams (blob, vesicle, facet, tone, noise-perm) so that changing one subsystem's parameters does not reshuffle the others. Sequential seeds give unrelated images. |
| `tameness` | `f64` | `0.55` | 0.0 ..= 1.0 | The one knob a non-expert touches. 0 = feral, 1 = corporate. Drives seven others by linear interpolation, applied BEFORE any explicit override: lump_amp = lerp(0.30, 0.12, t); lump_octaves = round(lerp(5, 3, t)); slope_limit = lerp(1.80, 0.80, t); warp_amt = lerp(0.45, 0.10, t); vesicle_count = round(lerp(11, 5, t)); vesicle_amp = lerp(0.42, 0.18, t); facet_curviness = lerp(1.00, 0.25, t). The default 0.55 reproduces the artwork's measured values. |
| `aspect` | `f64` | `1.50` | 1.0 ..= 2.2 | Width/height of the base ellipse; semi-axes a = R0*sqrt(aspect), b = R0/sqrt(aspect) so mean radius is R0 regardless. 1.0 reads as a pebble, 2.2 as a shard. Measured 1.50, which equals the artwork's canvas aspect (457/305). |
| `r0` | `f64` | `300.0` | fixed (canonical) | The canonical mean radius. Fix it at 300 inside a 1000-unit canvas and express every other length as a fraction of it; then all the defaults below are readable numbers and the viewBox is round. Rescale only at output time if a caller wants different units. |
| `margin` | `f64` | `0.04` | 0.0 ..= 0.15 | Padding around the blob's true bbox, as a fraction of R0, used to compute the viewBox. The artwork bleeds to the edge (margin 0); 0.04 is better for a logo that will sit next to text. |
| `lump_amp` | `f64` | `0.22` | 0.05 ..= 0.35 | Radial fbm amplitude as a fraction of the base ellipse radius. The primary silhouette-chaos knob. 0.05 is an egg, 0.22 is the artwork (measured relative radial std 0.25, Rmin/Rmean 0.52, Rmax/Rmean 1.40), 0.35 starts fighting the slope limiter and the blob circularises. |
| `lump_freq` | `f64` | `2.6` | 1.5 ..= 5.0 | Base angular frequency - roughly how many primary lumps go round. Measured: the artwork's dominant non-aspect harmonic is k=3 at 0.131*R, so 2.6 with octave spread lands there. Above 4 the silhouette loses its big gestural bulges and starts to read as texture. |
| `lump_octaves` | `u32` | `4` | 1 ..= 6 | fbm depth for the silhouette. Measured spectrum supports exactly 4 (energy dies below 0.01*R past k=11). A 5th octave adds wobble finer than the 0.001 output quantum can represent at logo scale - pure cost. |
| `lacunarity` | `f64` | `2.03` | 1.80 ..= 2.40 | Per-octave frequency multiplier. Deliberately NOT 2.0: at exactly 2.0 the Perlin lattices of successive octaves align on the same nodes and the sum develops faint grid-aligned ridges. 2.03 decorrelates them for free. |
| `gain` | `f64` | `0.45` | 0.30 ..= 0.60 | Per-octave amplitude multiplier. 0.45 matches the measured falloff (k=3 -> 0.131R, k=6 -> 0.034R). Higher = crustier, lower = smoother big forms. |
| `warp_amt` | `f64` | `0.22` | 0.0 ..= 0.60 | How far the flow field displaces the noise LOOKUP (not the geometry). 0 = isotropic lumpiness; 0.45 = lumps visibly lean and shear one way, reading as frozen flow. Above 0.6 the fbm decorrelates between adjacent theta and the silhouette gets noisy. |
| `field_freq` | `f64` | `0.55` | 0.15 ..= 1.50 | Flow field spatial frequency, in units of 1/R0. 0.55 means roughly one swing across the body - low is right. At 1.5 the facet cut curves become squiggles and the vesicle orientations stop agreeing with each other, losing the coherence that is the whole point of the field. |
| `field_angle_range` | `f64` | `1.0` | 0.15 ..= 1.50 | Hobbs's lambda. Maps fbm [-1,1] to alpha in [bias - PI*lambda, bias + PI*lambda]. 1.0 = full turbulence; 0.35 = a combed, strongly directional field (tamer, more graphic); above 1.0 the field winds past a full turn and produces vortices. |
| `field_bias` | `f64` | `0.0` | 0 ..= 2*PI | Constant added to every field angle. Rotates the whole directional character - the lean of the lumps, the vesicle long axes and the facet cuts all turn together. The cheapest way to make two seeds feel like a matched pair. |
| `field_quantize` | `Option<f64>` | `None` | Some(PI/4) ..= Some(PI/12) | Hobbs's non-continuous distortion: snap alpha to a multiple of this. He reports it gives 'more sculpted, rocky forms'. Some(PI/6) makes facet cuts prefer six directions and reads distinctly more crystalline/basaltic. Off by default because it fights the organic silhouette. |
| `slope_limit` | `f64` | `1.10` | 0.50 ..= 2.00 | kappa in the log-Lipschitz clamp \|d(ln r)/d theta\| <= kappa. The non-self-intersection and anti-cusp guard. MUST satisfy kappa >= (aspect^2-1)/(2*aspect) + 0.40 (0.82 at aspect 1.50) or the limiter eats the aspect ratio and circularises the blob - validate and error, do not silently clamp. Also caps the ribbon's width-correction factor at sqrt(1+kappa^2) = 1.487. |
| `ring_samples` | `u32` | `720` | 256 ..= 2048 | Samples of r(theta) before arclength resampling. 720 = one every half degree; the limiter's per-sample budget scales with it so the limit itself is resolution-independent. Below 256 the limiter quantises visibly. |
| `bezier_points` | `u32` | `256` | 96 ..= 512 | Points after arclength resampling, i.e. the number of cubic segments in the emitted outer contour. 256 keeps max deviation from the sampled ring under 1e-4*R0 at the default kappa, and keeps the `d` attribute near 10 KB. 96 gives a deliberately coarser, more 'drawn' outline. |
| `cr_tension` | `f64` | `0.50` | 0.35 ..= 0.60 | tau in B1 = P_i + tau*(P_{i+1}-P_{i-1})/3. 0.50 is true Catmull-Rom (the 1/6 coefficient) and reproduces a circle to within 2e-7*R. Lower tightens toward the polyline (crisper); above 0.55 handles bulge at high-curvature samples. |
| `outline_weight` | `f64` | `0.058` | 0.020 ..= 0.100 | Perceived ribbon width as a fraction of R0. Measured 0.0585 on the artwork (9.90 units over R0=169.2). This is the single most brand-defining number in the file - it is what makes the thing read as a sticker rather than as an illustration. Below 0.03 it stops reading at favicon size. |
| `inner_rim` | `f64` | `0.0` | 0.0 ..= 0.030 | Optional second concentric ribbon inside the first, as a fraction of R0. The artwork has one at 0.0147 (path 13, 2.49 units). Off by default; set 0.015 to match the artist exactly. |
| `facet_cuts` | `u32` | `11` | 4 ..= 20 | Number of bisections; regions = cuts + 1. The artwork has 14 facet paths with several adjacent same-tone, so a perceived ~9-12. Above 16 the regions get small enough that the 3-tone quantisation turns into dither. |
| `facet_curviness` | `f64` | `1.00` | 0.0 ..= 1.50 | Blend between a frozen straight chord (0, giving the low-poly look and exactly the artwork's `l` segments) and a live flow-field trace (1). Above 1.0 the cut over-rotates relative to the field and reads agitated. This knob alone swings the piece from geological to geometric. |
| `facet_min_area` | `f64` | `0.015` | 0.005 ..= 0.050 | Reject any cut whose smaller child is below this fraction of body area. The anti-sliver gate. Measured smallest facet in the artwork is 0.019, so 0.015 leaves a little room. |
| `facet_min_chord_sep` | `f64` | `0.08` | 0.03 ..= 0.20 | Minimum separation of the two cut crossings, as a fraction of the region's perimeter measured the short way round. Stops a cut from clipping a thin wedge off a corner - a different sliver mode from the area gate, because a long thin wedge can pass the area test. |
| `tone_fractions` | `[f64; 3]` | `[0.30, 0.60, 0.10]` | each 0.06 ..= 0.80, must sum to 1 | Target area share of shadow / mid / highlight, applied as quantiles over the light-sorted regions so all three tones always appear. Measured exactly: 0.298 / 0.603 / 0.099. The dominance of the mid tone is what keeps the piece from looking busy. |
| `light_dir` | `f64` | `1.90` | 0 ..= 2*PI | Direction of the tone gradient, radians, in SVG coordinates (y down). 1.90 rad points down-and-slightly-left, i.e. the highlight sits at the bottom. Measured: the artwork's three lightest facets all sit in y 180-298 of 305 while the darkest cluster at top and right. Lighting from below is what makes it read as glowing lava rather than as a lit stone - it is a deliberate artistic choice, not an error to fix. |
| `tone_noise` | `f64` | `0.35` | 0.0 ..= 1.0 | Blend of the light function between a pure directional ramp (0, very orderly, reads as a lit solid) and pure fbm (1, scattered, reads as mineral variation). 0.35 keeps a readable light direction while breaking up the banding. |
| `palette_hue` | `f64` | `13.0` | 0 ..= 360 | Base hue in degrees for the three-tone ramp. 13 is the measured shadow hue of #ee3300. Moving this alone transports the whole family coherently (e.g. 205 gives an ice-blue obsidian variant). |
| `palette_sat` | `f64` | `1.00` | 0.50 ..= 1.00 | HSL saturation for all three tones. The artwork is 100% on all three - the flatness of the saturation is part of why it reads as a mark rather than a render. Below 0.8 it starts to look like a photo trace. |
| `palette_light` | `f64` | `0.565` | 0.30 ..= 0.75 | HSL lightness of the MID tone, which the other two are defined against. Measured 0.565 (both #ff5721 and #ff6f21). |
| `tone_contrast` | `f64` | `1.00` | 0.40 ..= 2.00 | Scales both deltas off the mid tone: shadow = (H - 1.6*c deg, S, L - 0.098*c), mid = (H + 0*, S, L), highlight = (H + 6.6*c deg, S, L). Note the measured structure - the highlight is NOT lighter than the mid, it is hue-rotated +6.6deg toward yellow at identical L, and the shadow is darker at nearly the same hue. Preserving that asymmetry is what keeps recoloured palettes looking like the same artist's hand. At c=1.0 the three come out as #ee3300 / #ff5721 / #ff6f21. |
| `ink` | `Rgb` | `#231f20` | any | The near-black used for the outline ribbon and the vesicles. Measured; the artwork actually carries two near-identical inks (#231f20 and #221f1f, an Illustrator rich-black artefact) - use one. True #000 is noticeably harsher at small sizes. |
| `background` | `Option<Rgb>` | `None` | None or any | None emits no background rect, which is what a logo wants. Some(c) emits a full-viewBox rect as the first element. |
| `vesicle_count` | `u32` | `7` | 0 ..= 20 | Target number of gas holes. Measured exactly 7. This is the target, not a guarantee - the packer drops any it cannot place; falling below 60% of target rejects the whole logo. |
| `vesicle_rmin` | `f64` | `0.070` | 0.030 ..= 0.120 | Smallest vesicle equivalent radius as a fraction of R0. Measured 13.1/169.2 = 0.077. Below 0.05 small vesicles read as dust specks, not holes. |
| `vesicle_rmax` | `f64` | `0.260` | 0.100 ..= 0.400 | Largest vesicle equivalent radius as a fraction of R0. Measured 40.4/169.2 = 0.239. The r_max/r_min ratio of 3.7 plus size-coupled anisotropy reproduces the ~7x span the eye reads in max caliper (measured 42.4 to 136.0). |
| `vesicle_size_pow` | `u32` | `2` | 1 ..= 3 | r = r_min + (r_max - r_min) * u^pow. INTEGER so it is plain multiplication - powf is a libm call and a determinism hazard. 2 biases toward small, matching the measured clustering (five of seven below the midpoint). |
| `vesicle_amp` | `f64` | `0.30` | 0.10 ..= 0.50 | Radial fbm amplitude for a vesicle's own outline, fraction of its radius. Higher than the body's 0.22 because the measured vesicles are markedly lumpier than the silhouette (circularity 0.405 to 0.872). This is what stops them looking like drilled holes. |
| `vesicle_octaves` | `u32` | `3` | 2 ..= 5 | fbm depth per vesicle. 3 is enough at their size; a 4th octave's detail is below 1 unit on the smallest vesicle. |
| `vesicle_aspect_max` | `f64` | `2.40` | 1.0 ..= 3.2 | Elongation of the largest vesicle; aspect is interpolated by size so small vesicles stay round. Measured: max-caliper / 2*r_eff is 2.13 on the most elongated, 1.68 on the next. This parameter, not the radius law, is what creates the dramatic size impression. |
| `vesicle_align` | `f64` | `0.85` | 0.0 ..= 1.0 | How strongly a vesicle's long axis follows the flow field at its centre versus a random angle. High values are what make the holes look like gas that stretched in a single flow. At 0 the vesicles stop belonging to the rock. |
| `vesicle_edge_clear` | `f64` | `0.055` | 0.030 ..= 0.120 | Minimum gap from a vesicle to the body's INNER (post-ribbon) boundary, fraction of R0, floored at 1.2*outline_weight*R0 so it always scales with the ribbon. Measured tightest: 11.3/169.2 = 0.067, and 1.2*0.058 = 0.070 - the floor is the binding constraint at defaults, which is the right coupling. |
| `vesicle_gap` | `f64` | `0.090` | 0.040 ..= 0.200 | Minimum vesicle-to-vesicle gap, fraction of R0, additionally floored at 0.45*min(ri,rj). Measured tightest pair: 17.4/169.2 = 0.103. Too small and two vesicles read as one broken hole; too large and the packing loosens and the rock looks under-vesiculated. |
| `vesicle_attempts` | `u32` | `220` | 50 ..= 1000 | Placement tries per vesicle before giving up on it. 220 gives the hardest (largest, placed first into a constrained body) roughly a 90% success rate at defaults. |
| `attempts_total` | `u32` | `2000` | 500 ..= 20000 | Global placement budget; caps worst-case packing at about 0.4 ms. |
| `consec_fail_stop` | `u32` | `150` | 40 ..= 600 | End the packing loop after this many consecutive failures across all remaining vesicles - the body is full. |
| `coord_decimals` | `u32` | `3` | 2 ..= 4 | Decimal places in emitted coordinates. 3 on a 1000-unit viewBox means a 1e-3 unit quantum, invisible at any size, and keeps the file near 25 KB. 2 is visibly faceted on the outline and can collapse a thin facet to zero area - if you set 2, re-run the degenerate-area check AFTER quantisation. |
| `retry_budget` | `u32` | `20` | 1 ..= 100 | Whole-logo re-rolls if any quality rule fails. Each retry advances the seed stream (seed' = splitmix64(seed + attempt)), never re-uses it. On exhaustion the generator returns Err rather than emitting known-bad art - a generator that can emit ugly output is not finished. |

### Quality gates

Every one of these runs before an SVG is emitted. A failure re-rolls the logo.

- SILHOUETTE STAR-SHAPEDNESS. Assert r_i > 0.25*R0 for all i, and after the limiter assert |ln(r_i) - ln(r_{i-1})| <= kappa*dtheta*1.0001 for all i with wraparound. These two together are a PROOF that the outer contour does not self-intersect, so no intersection test is needed on it. Cheap enough to leave on in release.
- RIBBON POSITIVITY AND EVENNESS. r_in_i = r_i - w_i must satisfy r_in_i >= 0.15*R0 for all i (else the body pinches shut in a notch and the ribbon fills it with a black blob). Perceived width w_i / sqrt(1 + (rp_i/r_i)^2) must land in [0.75, 1.30] * w0 for every i after the k-cap; if not, the cap fired and kappa is too high for this outline_weight - reject and re-roll.
- RIBBON NECK CHECK. No two ring samples that are more than 0.10*perimeter apart ALONG the curve may be closer than 2.4*w0 in space. This catches the one case the star-shape theorem does not: a deep notch whose two walls are both compliant individually but which the ribbon still closes across, producing a solid black wedge. Test with the segment uniform grid, O(N).
- BEZIER SANITY, every emitted contour. Each handle length <= 0.5 * its chord (clamp, and count the clamps - more than 2% of segments clamped means the sampling is too coarse). Turn angle between consecutive chords <= 60deg. No two consecutive points closer than 1e-6 units (degenerate segment). Every contour ends in Z and has at least 4 segments.
- FACET AREA FLOOR. Every region's |signed area| >= facet_min_area * body_area (default 1.5%; measured smallest in the artwork 1.9%). Re-check AFTER coordinate quantisation if coord_decimals < 3.
- FACET SLIVER ANGLE. At each of the two junctions where the cut meets the boundary, the interior angle of each child must be >= 12deg. A wedge thinner than that antialiases into a coloured hairline that reads as a rendering bug. Measure using the tangent of the smoothed Bezier pieces, not the raw polyline.
- FACET CUT SEPARATION. The two crossing points must be >= facet_min_chord_sep (0.08) of the region perimeter apart the short way. Catches long thin corner wedges that pass the area test.
- CUT VALIDITY. Exactly 2 boundary crossings; the cut polyline's total absolute turning < 220deg (sufficient condition for no self-intersection on a flow trace); no arm exceeded 400 Euler steps.
- ALL THREE TONES PRESENT. Because tones are assigned by area quantile this is structural, but assert it anyway: each tone must cover >= 6% of body area and own >= 1 region. A two-tone scoria is a different, worse logo.
- VESICLE CLEARANCE. Every vesicle boundary sample (M=24) satisfies |q - C| <= r_body(pangle(q-C)) - max(1.2*w0, 0.055*R0). Every pair satisfies the centre-line separation test with gap = max(0.09*R0, 0.45*min(ri,rj)), evaluated over a +-18deg window so a side-lobe graze cannot slip through.
- VESICLE SHAPE HEALTH. Each vesicle's own r_v(theta) > 0.35 * its mean (no pinched or figure-eight hole), and its own log-Lipschitz limiter ran with the same kappa.
- VESICLE COVERAGE BAND. Total vesicle area / body area must land in [0.12, 0.26]. Measured 0.207. Below 0.12 it is a pebble, above 0.26 it is a sponge. Also require at least ceil(0.6 * vesicle_count) placed.
- SIZE SPREAD. The ratio of largest to smallest PLACED vesicle equivalent radius must be >= 2.0. If the packer happened to drop both big ones you get seven identical dots, which reads as polka dot, not as rock. Re-roll.
- NO DEGENERATE OR NON-FINITE OUTPUT. Every emitted coordinate is finite; no |signed area| below 1e-6; format -0.0 as 0.000 (a `-0.000` in the output breaks byte-identity between platforms that differ in sign-of-zero propagation, and looks like a bug to anyone reading the file).
- EXPLICIT SELF-INTERSECTION TEST on every emitted contour (outer, inner, each facet, each vesicle) via the segment uniform grid, excluding shared endpoints. This is O(n) and should always pass given the construction - it is there to catch an implementation error, not a design one. Treat a failure as a bug, not as a re-roll trigger.
- RETRY DISCIPLINE. Any failure re-rolls the whole logo from a fresh split of the seed, up to retry_budget. On exhaustion return Err with the name of the rule that kept failing - that error message is how a caller discovers that their parameter combination is infeasible (typically outline_weight too high against slope_limit, or vesicle_count too high against vesicle_rmax).

### Pitfalls

Each of these was a real trap found while specifying the algorithm.

- LIBM IS THE DETERMINISM HOLE, NOT THE PRNG. f64 +, -, *, / and sqrt are IEEE-754 exact and bit-identical everywhere. sin, cos, atan2, powf, exp, hypot are libm and DIFFER in the last ulp between glibc, musl, MSVC and Apple's libm. One ulp propagates through the limiter and the arclength resample into a different 3rd decimal, and your byte-identical guarantee is gone. Hand-roll sin/cos (polynomials given above), replace atan2 with the exact diamond pseudo-angle, replace powf with integer powers, and replace hypot with sqrt(x*x+y*y). Add a CI test that renders seeds 0..99 on two targets and diffs the bytes.
- INTERPOLATING ANGLES INSTEAD OF VECTORS in the flow field. If you cache alpha on a grid and bilinearly interpolate alpha, every cell where the field crosses +-PI produces a full-turn spin across one cell and a visible kink in the cut curve. Interpolate (cos alpha, sin alpha) as a vector and renormalise. This is the single most common flow-field bug and it is invisible in a debug render of the field.
- SEAM-HACKING THE PERIODIC NOISE. Sampling fbm along a line in theta and cross-fading the ends produces a visible crease at theta=0 that no amount of blending removes. Sample on a CIRCLE in 2D noise space - periodicity is then exact and free, for every octave, at any frequency.
- LACUNARITY EXACTLY 2.0 with Perlin. Successive octaves land on the same lattice nodes and the sum grows faint axis-aligned ridges. Use 2.03.
- `% n` FOR BOUNDED RANDOM INTEGERS. Still deterministic, but biased, and in the Fisher-Yates shuffle of the Perlin permutation table that bias makes the noise measurably directional. Use the Lemire rejection form.
- HashMap / HashSet ANYWHERE. Rust's default hasher is randomly seeded per process, so iteration order changes between runs on the same machine. Vec plus indices everywhere - the vesicle grid, the region list, the tone sort. This is the easiest way to silently lose determinism, because it is intermittent.
- SORTING WITH partial_cmp().unwrap(). Panics on NaN and gives an unspecified order on ties. Use `a.total_cmp(&b).then(ia.cmp(&ib))` - the index tie-break matters, because two regions genuinely can have equal area.
- f32 ANYWHERE. Mixing f32 and f64 invites x87 excess-precision differences on i686 and FMA contraction differences on aarch64. f64 throughout, and if you target i686 set the SSE2 math path.
- GENERAL POLYGON OFFSETTING for the ribbon. At any concavity whose curvature radius is smaller than w0 the naive offset self-intersects into a loop, and the loop removal is the hard part. The polar construction (r - w) makes this impossible by not having the problem.
- stroke-width FOR THE OUTLINE. Renders fine at first glance, then: a notch narrower than 2*w0 self-overlaps at the join; non-uniform scaling in Inkscape distorts it; and the user cannot node-edit the outline as geometry. It also does not match the artist's file, so output will not drop into her working document.
- FORGETTING TO REVERSE THE INNER SUBPATH. With fill-rule="nonzero" and both subpaths wound the same way, the ribbon fills solid and you get a black blob with no body. Emit the inner reversed AND set fill-rule="evenodd", so the path is correct under either rule - some downstream tools drop the attribute.
- CATMULL-ROM ON NON-UNIFORMLY SPACED SAMPLES. Uniform CR overshoots exactly where spacing changes fastest, which on a polar blob is in every deep notch, and the overshoot becomes a visible loop. Resample to uniform arclength first, or switch to centripetal CR. Do not try to fix it by lowering tau - that flattens the whole curve to hide one defect.
- RE-EVALUATING r(theta) FOR A CHILD FACET instead of copying the parent's vertices. Different last bits, sub-pixel gaps between adjacent facets, visible hairlines after antialiasing. Copy the f64s; share the cut array and the crossing points bit-identically between the two children.
- FACETS DRAWN EXACTLY TO THE SILHOUETTE. Even with perfect geometry, a renderer antialiases the facet edge and the ribbon edge independently and a 1-pixel light seam appears between them. Build the facet root on the ring dilated by +w0/2 and draw the ribbon last.
- RANDOM REGION SELECTION IN THE BSP. Produces one huge region and a litter of slivers - the classic failure. Always split the largest remaining region; it is one linear scan and it is what makes the faceting look designed.
- THRESHOLDING THE TONE MAP INSTEAD OF QUANTILING IT. A value threshold can yield zero highlight facets on some seeds. Area quantiles over the light-sorted regions cannot.
- PLACING VESICLES AGAINST THE OUTER BOUNDARY rather than the eroded inner one. They bite into the ribbon and the logo looks chewed. The clearance floor must be tied to outline_weight, not a fixed constant, or it silently breaks the moment someone thickens the outline.
- SHRINKING A VESICLE TO MAKE IT FIT. A shrunk-to-fit hole reads as dirt. Drop it; a missing vesicle reads as composition.
- QUANTISING OUTPUT TO 2 DECIMALS. Visible faceting on the outline at large sizes, and a thin facet can collapse to zero area after rounding - which passes your pre-quantisation area check and fails in the file. Use 3, or re-run the area check after formatting.
- `-0.000` IN THE OUTPUT. Breaks byte-identity across targets that differ in sign-of-zero propagation, and looks like a bug. Snap |x| < 5e-4 to exactly 0.0 before formatting.
- A CUT CURVE THAT RE-ENTERS THE REGION. Gives 4 crossings and your 2-crossing split code produces a garbage polygon that still fills and still looks almost right. Count crossings over the whole arm, not just the last step, and abort the cut.
- PRECOMPUTING THE FLOW FIELD GRID BY REFLEX. Hobbs needs it because he takes millions of steps; we take about 4k lookups per logo, so a 128x96 grid costs 4x more noise evaluations than evaluating directly and adds bilinear interpolation error. Direct evaluation is both faster and more accurate here. Keep the grid behind an option for the case where someone cranks facet_cuts and ring_samples.
- ONE SHARED PRNG STREAM. If vesicle placement draws from the same stream as the silhouette, changing vesicle_count reshuffles the rock. Five separate streams off one SplitMix64 split; that is what makes the parameter surface explorable rather than a slot machine.
- ASSUMING THE HIGHLIGHT IS LIGHTER. The measured palette's highlight has the SAME HSL lightness as the mid and differs only by a +6.6deg hue rotation toward yellow. Implementing the ramp as three lightness steps gives a competent, generic, wrong-looking gradient. Keep the asymmetry.
- SETTING slope_limit BELOW THE ELLIPSE'S OWN SLOPE. At aspect 1.50 the base ellipse already needs kappa >= 0.417; a kappa of 0.5 leaves almost no budget for lumps and a kappa of 0.3 silently circularises the blob so the aspect parameter appears broken. Validate kappa >= (aspect^2-1)/(2*aspect) + 0.40 and return a named error.

---

## Part 3 — corrections found while implementing

Part 2 was written before any of it ran. These are the places the spec was
wrong, found by building it and looking at the output. Each is also recorded
at the code that implements it.

- **The facet root goes on the ribbon's midline `r − w/2`, not `r + w/2`.**
  Part 2 says to dilate the facet root outward so "the ribbon covers the
  overshoot". It cannot: the ribbon occupies `[r − w, r]`, so anything past
  `r` is outside it and nothing paints over it. Built that way, every seed
  showed a bright orange fringe outside the black outline.

- **`vesicle_size_pow` is 1, not 2.** Part 2's table asks for a square law
  "matching the measured clustering", but Part 1 measures radii that are
  near-uniform over their range and says in words not to use a steep power
  law. The square law held coverage at 0.152 against the measured 0.207.

- **Vesicle radii must be drawn stratified.** Independent uniforms land
  within 2× of each other about a third of the time; the artwork's seven
  span 3.08× in a two-big / three-mid / two-small hierarchy. One draw per
  `1/count` band produces that by construction. This also retires the
  `vesicle_rmax` 0.320 workaround — with the range actually reachable, the
  measured 0.240 is correct, and the generator went from 5 failures in 300
  seeds to none and from 5.0 re-rolls per logo to 0.5.

- **Radial fbm cannot produce the measured big holes; they have to be
  bitten.** Part 1 measures the three largest at circularity 0.405, 0.459
  and 0.587 and Part 2 expects amplitude to deliver that. A star-shaped
  polar blob sits near 0.79 however much amplitude it is given, because a
  lobe is a bump and a bump is not a bite. One or two deep concave notches,
  cut *across* the long axis, bring it to 0.53–0.67. Three natural-looking
  variants are all wrong: a wide window scales one flank into a teardrop
  that measures *more* circular; three evenly spaced bites make a rounded
  triangle that measures back up; and a bite aimed along the long axis only
  shortens the hole.

- **The silhouette needs a real band-limiter, and `gain` / `lump_octaves`
  are not it.** The log-Lipschitz limiter is purely erosive, so a deep notch
  survives as a V with a corner at the bottom, where every concave notch in
  the drawing is rounded. Sweeping `gain` and `lump_octaves` barely moved
  the h13–h16 energy, because the excess was never fbm detail — it was a
  few sharp features being broadband. Forty binomial passes (sigma ≈ 2.5°,
  against the six Part 2 implies) lift the sharpest convex corner from 0.045
  to 0.09–0.10 of the mean radius, against the artwork's 0.113, and
  attenuate harmonic k by `exp(-(k*sigma)^2/2)` — under a tenth at h12 and
  gone by h40, which is the measured spectrum.

- **The dilate half of the log-Lipschitz limiter is dead code.** `r_i ≥
  r_j·flo` and `r_j ≤ r_i·cap` are the same inequality when `flo = 1/cap`,
  so the erode sweeps already enforce both. The operator is purely erosive:
  a notch is never filled, it drags its neighbourhood down to meet it.

- **Two gates Part 2 does not have.** `lump_amp` sets the *expected* radial
  amplitude, not the realised one, and the realised bbox aspect is fitted to
  the lumps rather than to the base ellipse. Both needed their own gate for
  the family to stay on-brand across seeds.

- **Still open.** The cross-platform half of the determinism claim is
  untested: the suite proves output is a pure function of its inputs within
  one process, but proving byte-identity across glibc, musl, MSVC and Apple
  needs a CI job that diffs them.
