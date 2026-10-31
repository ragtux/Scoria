//! Faceting: recursive flow-field bisection of the body, then three tones by
//! area quantile.
//!
//! The artwork's interior is a SHARD SUBDIVISION - a plate dropped once. Long
//! near-straight chords meet at Y and T junctions, adjacent cells share an
//! exact boundary, and the cell areas spread about 7:1. The generator
//! reproduces that by always splitting the largest remaining region with a cut
//! traced from its centroid along the same flow field that leans the
//! silhouette's lumps.
//!
//! # Why there is no clipping in this file
//!
//! A facet's boundary is made of exactly two kinds of piece: arcs of the
//! parent's boundary, copied vertex for vertex, and the cut, which stopped the
//! instant it crossed that boundary. Both are inside the body because of how
//! the points were obtained, not because anything checked afterwards. The five
//! rules that keep it true (SPEC.md, `clipping`):
//!
//! 1. copy the parent's `f64` vertices, never re-evaluate `r(theta)`;
//! 2. compute each crossing once and store it in both children;
//! 3. one child gets the cut array, the other its exact reverse;
//! 4. smooth each PIECE on its own, so a shared edge yields the same cubic
//!    in both facets (see [`Region::breaks`]);
//! 5. build the root on the ring dilated by `+w0/2` so the ribbon, drawn last,
//!    covers the tiling's outer seam.

use crate::bezier::{catmull_rom_closed, catmull_rom_open, clamp_handles, Cubic};
use crate::field::Field;
use crate::math::{
    closed_len, point_in_polygon, polygon_area, polygon_centroid, polyline_len, seg_intersect,
    sin_cos, total_turning, PI, V2,
};
use crate::noise::{Fbm, Perlin};
use crate::rng::Pcg32;
use crate::solid::{DepthMap, Light};
use crate::Params;

/// Euler step for a cut trace, as a fraction of R0. 0.01*300 = 3 canonical
/// units, 0.3% of the canvas - squarely inside Hobbs's 0.1-0.5% band, below
/// which a trace develops sharp points.
const EULER_STEP_FRAC: f64 = 0.01;
/// Step budget per arm. An arm that has not left the region by here is looping
/// inside it, and the cut is abandoned.
const MAX_STEPS: u32 = 400;
/// Jitters of the seed point before a region is given up on.
const JITTER_ATTEMPTS: u32 = 6;
/// Seed jitter, as a fraction of the region's equivalent radius.
const JITTER_FRAC: f64 = 0.18;
/// Half the angle between the two cut families, radians (23 degrees).
///
/// Measured: the length-weighted direction histogram of the artwork's 53 long
/// facet edges is bimodal - 60 to 75 degrees carries 432 units and 105 to 120
/// carries 470, two families about 46 degrees apart, with the least mass near
/// horizontal. Tracing the field alone gives ONE family, because the field
/// barely turns across a single body at `field_freq` 0.55, and the rock then
/// tiles into parallel stripes instead of shards. Offsetting each cut into
/// one lobe or the other restores the criss-cross while leaving the field in
/// charge of the overall lean.
const CUT_FAMILY_HALF_ANGLE: f64 = 0.401_425_727_958_696_1;
/// Jitter on the family angle, radians (+-8 degrees), so the two families are
/// lobes rather than two exact directions.
const CUT_FAMILY_JITTER: f64 = 0.139_626_340_159_546_36;
/// 220 degrees in radians. A flow trace that turns less than this cannot cross
/// itself - the cheap sufficient test SPEC.md prescribes. Shared with the
/// quality gate, which re-asserts it on what was accepted.
pub const MAX_CUT_TURNING: f64 = 3.839_724_354_387_525_3;
/// cos(12 deg). A junction thinner than 12 degrees antialiases into a coloured
/// hairline that reads as a rendering bug, so the cut may not meet the
/// boundary flatter than this.
pub const MIN_JUNCTION_COS: f64 = 0.978_147_600_733_805_7;
/// Emitted chord along a cut, as a fraction of R0: 0.06*300 = 18 canonical
/// units, which is the artwork's measured median facet chord (8.5 to 11.4
/// artwork units) carried over. The Euler walk emits a point every 3 units;
/// keeping all of them would be six times the file for no visible gain, and
/// Douglas-Peucker instead of a uniform resample would leave the spacing
/// jumping between 3 and 40 units - which is precisely where uniform
/// Catmull-Rom overshoots.
const CUT_SEG_FRAC: f64 = 0.06;
/// Broadphase cell for the cut-crossing grid, as a fraction of R0.
const CUT_GRID_CELL_FRAC: f64 = 0.08;
/// fbm depth of the tone map. Three octaves at logo scale; a fourth is finer
/// than one facet.
const TONE_OCTAVES: u32 = 3;
/// Spatial frequency of the tone map, per R0.
const TONE_NOISE_FREQ: f64 = 0.9;
/// Seed points tried per cut when the surface is steering it.
///
/// `facet_relief` orients a cut along the solid's iso-brightness line, but an
/// orientation alone does not say WHERE to put the line, and a contour drawn
/// through the wrong level separates nothing. Scoring a spread of candidate
/// seeds by how close their brightness is to the region's own mean picks the
/// level that halves the region's brightness, so the chord through it is the
/// region's median contour and the two children land either side of a real
/// step in the shading instead of either side of an arbitrary line.
const SEED_CANDIDATES: u32 = 12;
/// Jitter of those candidates about the area centroid, as a fraction of the
/// region's equivalent radius.
///
/// Wider than [`JITTER_FRAC`], which only has to break ties: this one has to
/// reach far enough across the region to find the brightness level it wants.
/// The area and sliver gates still decide what is acceptable, so reaching
/// further costs nothing but a rejected cut.
const SEED_SPREAD: f64 = 0.45;
/// Samples of the brightness a facet's tone is averaged over: its centroid
/// plus one part-way toward each of this many boundary points.
///
/// A single sample at the centroid judges a facet spanning a brightness ramp
/// by one point of it, and the largest facet is 7 times the smallest, so the
/// largest is exactly where that is least defensible.
const TONE_SAMPLES: usize = 8;
/// How far from the centroid toward a boundary point those samples sit. Short
/// of the boundary, so a sample cannot land on the far side of a shared edge
/// and pick up the neighbour's brightness instead.
const TONE_SAMPLE_REACH: f64 = 0.55;
/// Share of the facet area each tone must end up carrying. The gate asks for
/// 6% of BODY area, and the facets tile a slightly larger region, so 7% here
/// clears it with margin.
const TONE_FLOOR_FRAC: f64 = 0.07;

/// Uniform-grid broadphase over the segments of a closed polyline.
///
/// `Vec<Vec<u32>>`, never a `HashMap`: Rust's default hasher is seeded per
/// process, so a hashed container would change iteration order between runs on
/// the same machine and quietly break determinism.
pub struct SegGrid {
    lo: V2,
    sx: f64,
    sy: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl SegGrid {
    /// Bucket every segment of the closed loop `pts` into cells of about
    /// `cell` units. The grid is capped at 256x256 so an abusive bbox cannot
    /// allocate without bound; the effective cell grows instead.
    pub fn new(pts: &[V2], cell: f64) -> SegGrid {
        let mut lo = pts[0];
        let mut hi = pts[0];
        for q in pts {
            lo.x = lo.x.min(q.x);
            lo.y = lo.y.min(q.y);
            hi.x = hi.x.max(q.x);
            hi.y = hi.y.max(q.y);
        }
        let cell = cell.max(1e-6);
        let span_x = (hi.x - lo.x).max(1e-9);
        let span_y = (hi.y - lo.y).max(1e-9);
        let nx = ((span_x / cell).ceil() as i64).clamp(1, 256) as usize;
        let ny = ((span_y / cell).ceil() as i64).clamp(1, 256) as usize;
        let mut g = SegGrid {
            lo,
            sx: span_x / nx as f64,
            sy: span_y / ny as f64,
            nx,
            ny,
            cells: vec![Vec::new(); nx * ny],
        };
        let n = pts.len();
        let stride = g.nx;
        for i in 0..n {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            let (x0, x1) = g.span_x(a.x.min(b.x), a.x.max(b.x));
            let (y0, y1) = g.span_y(a.y.min(b.y), a.y.max(b.y));
            for iy in y0..=y1 {
                for ix in x0..=x1 {
                    g.cells[iy * stride + ix].push(i as u32);
                }
            }
        }
        g
    }

    /// Cell index range covered by an x interval.
    fn span_x(&self, a: f64, b: f64) -> (usize, usize) {
        let i0 = ((a - self.lo.x) / self.sx).floor();
        let i1 = ((b - self.lo.x) / self.sx).floor();
        (
            i0.clamp(0.0, (self.nx - 1) as f64) as usize,
            i1.clamp(0.0, (self.nx - 1) as f64) as usize,
        )
    }

    /// Cell index range covered by a y interval.
    fn span_y(&self, a: f64, b: f64) -> (usize, usize) {
        let i0 = ((a - self.lo.y) / self.sy).floor();
        let i1 = ((b - self.lo.y) / self.sy).floor();
        (
            i0.clamp(0.0, (self.ny - 1) as f64) as usize,
            i1.clamp(0.0, (self.ny - 1) as f64) as usize,
        )
    }

    /// Segment indices that could meet the segment `a`-`b`, deduplicated and
    /// in ascending order. A segment appears in every cell its bbox touches,
    /// so the dedupe is required: a duplicate would be counted as a second
    /// crossing and abort a perfectly good cut.
    pub fn query(&self, a: V2, b: V2, out: &mut Vec<u32>) {
        out.clear();
        let (x0, x1) = self.span_x(a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = self.span_y(a.y.min(b.y), a.y.max(b.y));
        for iy in y0..=y1 {
            for ix in x0..=x1 {
                out.extend_from_slice(&self.cells[iy * self.nx + ix]);
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

/// A region of the interior, mid-subdivision.
///
/// The point loop is closed implicitly (no repeated last point). `breaks` is
/// what makes the shared-edge guarantee work: it records where one boundary
/// PIECE ends and the next begins, and each piece is smoothed on its own.
#[derive(Clone, Debug)]
pub struct Region {
    /// The closed boundary, in order. Vertices are copied from the parent -
    /// never re-evaluated.
    pub pts: Vec<V2>,
    /// Indices where a new piece starts, strictly ascending, first always 0.
    /// EMPTY means the region is still one smooth closed loop (only the root
    /// ever is), which is smoothed with the closed Catmull-Rom instead.
    pub breaks: Vec<usize>,
}

impl Region {
    /// The boundary split into pieces, each including both of its endpoints so
    /// consecutive pieces join exactly. Empty for an unsplit root.
    pub fn pieces(&self) -> Vec<Vec<V2>> {
        let n = self.pts.len();
        if self.breaks.len() < 2 || n == 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(self.breaks.len());
        for j in 0..self.breaks.len() {
            let s = self.breaks[j];
            let e = self.breaks[(j + 1) % self.breaks.len()];
            let mut piece = Vec::new();
            let mut i = s;
            loop {
                piece.push(self.pts[i]);
                if i == e {
                    break;
                }
                i = (i + 1) % n;
            }
            out.push(piece);
        }
        out
    }

    /// Signed-area magnitude.
    pub fn area(&self) -> f64 {
        polygon_area(&self.pts).abs()
    }
}

/// One finished facet: a tone and the cubics that draw it.
#[derive(Clone, Debug)]
pub struct Facet {
    /// The boundary polyline the cubics were fitted to.
    pub pts: Vec<V2>,
    /// Piece junctions in `pts`; see [`Region::breaks`].
    pub breaks: Vec<usize>,
    /// The emitted outline, piece by piece, closed.
    pub cubics: Vec<Cubic>,
    /// Index into `cubics` where each piece starts; used by the sliver-angle
    /// gate, which measures the smoothed tangents rather than the polyline.
    pub piece_starts: Vec<usize>,
    /// Area centroid, the input to the light function.
    pub centroid: V2,
    /// Area magnitude.
    pub area: f64,
    /// 0 shadow, 1 mid, 2 highlight.
    pub tone: usize,
    /// Mean Lambert brightness of the solid's real surface over this facet.
    /// The tone is a quantisation of this; the gate checks the two agree.
    pub lambert: f64,
    /// How many handles the Bezier clamp caught on this facet.
    pub clamped: usize,
}

/// What the subdivision did, for the quality gates and `--time`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutReport {
    /// Cuts that validated and were applied.
    pub made: u32,
    /// Cuts abandoned after all six jitters failed.
    pub skipped: u32,
    /// Worst accepted crossing separation, as a fraction of the region
    /// perimeter. 1.0 when nothing was cut.
    pub min_sep_frac: f64,
    /// Worst accepted cut turning, radians.
    pub max_turning: f64,
}

impl Default for CutReport {
    fn default() -> Self {
        CutReport {
            made: 0,
            skipped: 0,
            min_sep_frac: 1.0,
            max_turning: 0.0,
        }
    }
}

/// One traced arm of a cut. The crossing point itself is the last entry of
/// `pts`, so it is stored once and shared from there.
struct Arm {
    /// Points after the seed, ending exactly on the crossing.
    pts: Vec<V2>,
    /// Index of the boundary segment it left through.
    seg: usize,
}

/// Subdivide `root` into `p.facet_cuts + 1` facets and tone them.
///
/// `rng` is the facet stream and `tone_rng` the tone stream: they are separate
/// so that re-tuning the light does not reshuffle the cuts.
#[allow(clippy::too_many_arguments)]
pub fn build(
    root: &[V2],
    p: &Params,
    field: &Field,
    perlin: &Perlin,
    depth: &DepthMap,
    light: &Light,
    rng: &mut Pcg32,
    tone_rng: &mut Pcg32,
    c: V2,
    r0: f64,
    body_area: f64,
) -> (Vec<Facet>, CutReport) {
    let mut regions = vec![Region {
        pts: root.to_vec(),
        breaks: Vec::new(),
    }];
    // A region whose six jitters all failed is not worth retrying on the next
    // cut: without this the biggest region swallows every remaining cut and
    // the logo comes out with three facets.
    let mut blocked = vec![false];
    let mut report = CutReport::default();

    for _ in 0..p.facet_cuts {
        // ALWAYS THE LARGEST. Random selection is the classic BSP failure: one
        // huge region and a litter of slivers. Strict `>` keeps the lowest
        // index on a tie, which two regions genuinely can hit.
        let mut best: Option<usize> = None;
        let mut best_area = 0.0f64;
        for (i, reg) in regions.iter().enumerate() {
            if blocked[i] {
                continue;
            }
            let a = reg.area();
            if best.is_none() || a > best_area {
                best = Some(i);
                best_area = a;
            }
        }
        let Some(bi) = best else { break };
        match try_cut(
            &regions[bi],
            p,
            field,
            depth,
            light,
            rng,
            c,
            r0,
            body_area,
            &mut report,
        ) {
            Some((c1, c2)) => {
                regions[bi] = c1;
                regions.push(c2);
                blocked.push(false);
            }
            None => {
                blocked[bi] = true;
                report.skipped += 1;
            }
        }
    }

    (
        tone(regions, p, perlin, depth, light, tone_rng, c, r0),
        report,
    )
}

/// Try to bisect one region, re-jittering the seed point up to six times.
#[allow(clippy::too_many_arguments)]
fn try_cut(
    region: &Region,
    p: &Params,
    field: &Field,
    depth: &DepthMap,
    light: &Light,
    rng: &mut Pcg32,
    c: V2,
    r0: f64,
    body_area: f64,
    report: &mut CutReport,
) -> Option<(Region, Region)> {
    let pts = &region.pts;
    let n = pts.len();
    if n < 12 {
        return None;
    }
    let area = polygon_area(pts).abs();
    let r_region = (area / PI).sqrt();
    let centroid = polygon_centroid(pts);
    let perim = closed_len(pts);
    let h = EULER_STEP_FRAC * r0;
    let grid = SegGrid::new(pts, (CUT_GRID_CELL_FRAC * r0).max(4.0 * h));
    let prefix = prefix_lengths(pts);

    // The level the cut wants to sit on: the region's own mean brightness,
    // so the chord through it halves the region's shading.
    let want_b = if p.facet_relief > 0.0 {
        Some(mean_brightness(pts, centroid, depth, light))
    } else {
        None
    };

    for _ in 0..JITTER_ATTEMPTS {
        // The AREA centroid, not the vertex mean: the vertex mean drifts
        // toward whichever part of the boundary is sampled densest.
        let g = match want_b {
            // Steered: try a spread of seeds and keep the one sitting
            // closest to the level the cut should separate.
            Some(b0) => {
                let mut best: Option<(f64, V2)> = None;
                for _ in 0..SEED_CANDIDATES {
                    let q = centroid
                        + V2::new(
                            rng.range(-SEED_SPREAD, SEED_SPREAD) * r_region,
                            rng.range(-SEED_SPREAD, SEED_SPREAD) * r_region,
                        );
                    if !point_in_polygon(pts, q) {
                        continue;
                    }
                    let err = (depth.brightness(q, light) - b0).abs();
                    // Strictly better, so the FIRST candidate wins a tie and
                    // the choice does not depend on iteration order.
                    if best.is_none_or(|(e, _)| err < e) {
                        best = Some((err, q));
                    }
                }
                match best {
                    Some((_, q)) => q,
                    None => {
                        continue;
                    }
                }
            }
            None => {
                centroid
                    + V2::new(
                        rng.range(-JITTER_FRAC, JITTER_FRAC) * r_region,
                        rng.range(-JITTER_FRAC, JITTER_FRAC) * r_region,
                    )
            }
        };
        // A child region is not star-shaped, so the fast polar test does not
        // apply here - this is the crossing-number test.
        if !point_in_polygon(pts, g) {
            continue;
        }
        // One lobe or the other, re-rolled with the jitter. The offset is
        // applied to the blended direction at every step, not baked into the
        // frozen one: at `facet_curviness` 1.0 the frozen direction carries
        // no weight at all, so a family baked in there would vanish after the
        // first step and every cut would come out parallel again.
        let lobe = if rng.below(2) == 0 { -1.0 } else { 1.0 };
        let family =
            lobe * (CUT_FAMILY_HALF_ANGLE + rng.range(-CUT_FAMILY_JITTER, CUT_FAMILY_JITTER));
        // WHERE THE SURFACE GETS ITS SAY. The cut's base direction is the
        // flow field at the seed leaned toward the solid's iso-brightness
        // line there, by `facet_relief`. An iso-line is a contour of the real
        // lit surface, so a cut along one means the flat tone either side of
        // it is the quantisation of that surface, and the boundary lands
        // where the shading would have stepped anyway. A cut across the
        // gradient instead reads as damage to the rock rather than as its
        // form - which is most of the difference between faceting that looks
        // designed and faceting that looks shattered.
        //
        // Sampled ONCE, at the seed, and not followed step by step. Following
        // it was the first design and it does not work: a level set of a
        // smooth function either leaves the region or closes into a loop, and
        // between the loops around every brightness extremum and the rapid
        // rotation near every saddle, a long trace wanders. Measured, with
        // the direction re-read every 3-unit step: the median cut's total
        // absolute turning went from 17 degrees to 235, against the 220 the
        // no-self-intersection test allows, and 916 cuts in 200 seeds failed
        // it however hard the heading was cone-clamped and rate-limited -
        // because a bounded heading can still oscillate inside its bound
        // forever, and absolute turning is what the test measures.
        //
        // Taking the direction once is also the better match to the drawing,
        // whose facet edges are long near-straight chords: measured, 27 to 80
        // percent of each facet's perimeter runs at least 30 units while
        // deviating under 6 percent from its chord.
        let flow = field.dir((g - c) * (1.0 / r0));
        let straight = match depth.iso_dir(g, light, crate::solid::GRAD_H_FRAC * r0) {
            Some(iso) if p.facet_relief > 0.0 => {
                // Orient the iso-line with the flow before blending: the
                // perpendicular of a gradient carries a sign the field does
                // not, and blending the two with opposite signs would cancel
                // them instead of leaning one toward the other.
                let iso = if iso.dot(flow) < 0.0 { -iso } else { iso };
                let mixed = flow + (iso - flow) * p.facet_relief;
                if mixed.len2() > 1e-18 {
                    mixed.norm()
                } else {
                    flow
                }
            }
            _ => flow,
        };
        let arm = |sign: f64| {
            trace_arm(TraceCfg {
                start: g,
                straight,
                sign,
                region: pts,
                grid: &grid,
                field,
                c,
                r0,
                h,
                curviness: p.facet_curviness,
                family,
            })
        };
        let Some(fwd) = arm(1.0) else {
            continue;
        };
        let Some(bwd) = arm(-1.0) else {
            continue;
        };

        let mut cut: Vec<V2> = Vec::with_capacity(bwd.pts.len() + fwd.pts.len() + 1);
        cut.extend(bwd.pts.iter().rev().copied());
        cut.push(g);
        cut.extend(fwd.pts.iter().copied());

        // Resample ONCE, here, before the split: both children then inherit
        // the same array (one of them reversed), so the shared edge stays
        // bit-identical. Resampling per child would risk the two sides
        // landing on different points and opening a hairline.
        let mut cut = resample_open(&cut, CUT_SEG_FRAC * r0);
        if cut.len() < 3 {
            continue;
        }

        let turning = total_turning(&cut);
        if !turning.is_finite() || turning >= MAX_CUT_TURNING {
            continue;
        }

        let (mut ia, mut ib) = (bwd.seg, fwd.seg);
        if ia == ib {
            // Both ends on one boundary segment: the cut clipped a crumb.
            continue;
        }
        if ia > ib {
            std::mem::swap(&mut ia, &mut ib);
            cut.reverse();
        }
        let xa = cut[0];
        let xb = cut[cut.len() - 1];

        // ANTI-SLIVER 1: the crossings must be far apart along the boundary,
        // measured the short way. A long thin corner wedge passes the area
        // test and fails this one.
        let sa = prefix[ia] + (xa - pts[ia]).len();
        let sb = prefix[ib] + (xb - pts[ib]).len();
        let along = (sa - sb).abs();
        let sep = along.min(perim - along) / perim;
        if sep < p.facet_min_chord_sep {
            continue;
        }

        // ANTI-SLIVER 2: the cut may not graze the boundary. Checked on the
        // polyline here to save a split; the gate re-checks the smoothed
        // tangents, which is what a renderer actually draws.
        if !junction_ok(pts, ia, xa, cut[1]) || !junction_ok(pts, ib, xb, cut[cut.len() - 2]) {
            continue;
        }

        let (c1, c2) = split(region, ia, xa, ib, xb, &cut);
        if c1.pts.len() < 4 || c2.pts.len() < 4 {
            continue;
        }
        let floor = p.facet_min_area * body_area;
        if c1.area() < floor || c2.area() < floor {
            continue;
        }

        report.made += 1;
        report.min_sep_frac = report.min_sep_frac.min(sep);
        report.max_turning = report.max_turning.max(turning);
        return Some((c1, c2));
    }
    None
}

/// Everything one [`trace_arm`] call needs. A struct rather than fourteen
/// positional arguments, which is both unreadable and easy to transpose.
struct TraceCfg<'a> {
    /// Seed point, the region's jittered centroid.
    start: V2,
    /// The field direction frozen at the seed.
    straight: V2,
    /// +1 for the forward arm, -1 for the backward one.
    sign: f64,
    /// The region boundary the arm must stop on.
    region: &'a [V2],
    /// Broadphase over that boundary's segments.
    grid: &'a SegGrid,
    /// The flow field.
    field: &'a Field,
    /// Body centre.
    c: V2,
    /// Canonical mean radius.
    r0: f64,
    /// Euler step length.
    h: f64,
    /// Flow blend: 0 a straight chord, 1 a live flow curve.
    curviness: f64,
    /// Rotation into one of the two measured cut families.
    family: f64,
}

/// Hobbs's fixed-step Euler walk, from the seed until it leaves the region.
///
/// `sign` is +1 for the forward arm and -1 for the backward one. The direction
/// is a blend of `straight` - which `try_cut` has already leaned toward the
/// solid's crease - against the live field: at `curviness` 0 the cut is a
/// straight chord, at 1 a full flow curve, above 1 it over-rotates. VECTORS
/// are blended, never angles - interpolating angles spins a full turn
/// wherever the field crosses +-PI.
fn trace_arm(cfg: TraceCfg<'_>) -> Option<Arm> {
    let TraceCfg {
        start,
        straight,
        sign,
        region,
        grid,
        field,
        c,
        r0,
        h,
        curviness,
        family,
    } = cfg;
    let n = region.len();
    let mut pts: Vec<V2> = Vec::with_capacity(64);
    let mut p = start;
    let mut cand: Vec<u32> = Vec::with_capacity(16);
    for _ in 0..MAX_STEPS {
        let live = field.dir((p - c) * (1.0 / r0));
        let mut blend = straight + (live - straight) * curviness;
        if blend.len2() < 1e-18 {
            // Over-rotation can cancel the blend exactly; fall back rather
            // than step nowhere and burn the budget.
            blend = straight;
        }
        let pn = p + blend.norm().rot(family) * (h * sign);
        grid.query(p, pn, &mut cand);
        let mut hits = 0u32;
        let mut hit: Option<(f64, usize)> = None;
        for &si in &cand {
            let i = si as usize;
            if let Some((t, _u)) = seg_intersect(p, pn, region[i], region[(i + 1) % n]) {
                hits += 1;
                let better = match hit {
                    None => true,
                    Some((best_t, _)) => t < best_t,
                };
                if better {
                    hit = Some((t, i));
                }
            }
        }
        if hits > 1 {
            // Two crossings in one step: the arm clipped a concave corner and
            // the split would produce a garbage polygon that still fills.
            return None;
        }
        if let Some((t, i)) = hit {
            pts.push(p + (pn - p) * t);
            return Some(Arm { pts, seg: i });
        }
        pts.push(pn);
        p = pn;
    }
    None
}

/// Does the cut meet the boundary at more than 12 degrees, on both sides?
///
/// The two interior angles at a junction sum to a straight angle, so a single
/// test on the absolute cosine covers both children.
fn junction_ok(pts: &[V2], seg: usize, x: V2, next: V2) -> bool {
    let n = pts.len();
    let t = (pts[(seg + 1) % n] - pts[seg]).norm();
    let d = (next - x).norm();
    if t.len2() < 1e-18 || d.len2() < 1e-18 {
        return false;
    }
    t.dot(d).abs() <= MIN_JUNCTION_COS
}

/// Uniform-arclength resample of an OPEN polyline at about `spacing` apart,
/// with both endpoints kept exactly.
///
/// The endpoints are the cut's two boundary crossings, which must survive bit
/// for bit - they are shared with the sibling region. The interior is evenly
/// spaced because uniform Catmull-Rom overshoots wherever the spacing changes
/// fast, and the handle clamp then fires on every short interval.
fn resample_open(pts: &[V2], spacing: f64) -> Vec<V2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let total = polyline_len(pts);
    if total <= 0.0 || spacing <= 0.0 {
        return pts.to_vec();
    }
    let steps = ((total / spacing).round() as i64).max(2) as usize;
    let step = total / steps as f64;
    let mut out = Vec::with_capacity(steps + 1);
    out.push(pts[0]);
    let mut seg = 0usize;
    let mut acc = 0.0;
    let mut seg_len = (pts[1] - pts[0]).len();
    for k in 1..steps {
        let target = k as f64 * step;
        while seg + 2 < n && acc + seg_len < target {
            acc += seg_len;
            seg += 1;
            seg_len = (pts[seg + 1] - pts[seg]).len();
        }
        let t = if seg_len > 0.0 {
            ((target - acc) / seg_len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(pts[seg].lerp(pts[seg + 1], t));
    }
    out.push(pts[n - 1]);
    out
}

/// Arclength from `pts[0]` to each vertex.
fn prefix_lengths(pts: &[V2]) -> Vec<f64> {
    let mut out = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    for i in 0..pts.len() {
        out.push(acc);
        acc += (pts[(i + 1) % pts.len()] - pts[i]).len();
    }
    out
}

/// Replace a region with its two children.
///
/// `ia < ib` are the boundary segments the cut crosses, `xa`/`xb` the crossing
/// points, and `cut` the full cut polyline running from `xa` to `xb`. Child 1
/// gets the boundary arc `ia+1 ..= ib` and the REVERSED cut; child 2 gets the
/// rest of the boundary and the cut as traced. Both get the same crossing
/// points, bit for bit - recomputing them per child is how adjacent facets
/// grow sub-pixel gaps that antialias into hairlines.
fn split(parent: &Region, ia: usize, xa: V2, ib: usize, xb: V2, cut: &[V2]) -> (Region, Region) {
    let pts = &parent.pts;
    let n = pts.len();
    let interior = &cut[1..cut.len() - 1];

    let mut p1 = Vec::with_capacity(ib - ia + 2 + interior.len());
    p1.push(xa);
    p1.extend_from_slice(&pts[ia + 1..=ib]);
    p1.push(xb);
    p1.extend(interior.iter().rev().copied());
    let mut b1 = vec![0usize];
    for &k in &parent.breaks {
        if k > ia && k <= ib {
            b1.push(k - ia);
        }
    }
    b1.push(ib - ia + 1);

    let mut p2 = Vec::with_capacity(n + ia - ib + 2 + interior.len());
    p2.push(xb);
    p2.extend_from_slice(&pts[ib + 1..n]);
    p2.extend_from_slice(&pts[0..=ia]);
    p2.push(xa);
    p2.extend(interior.iter().copied());
    let mut b2 = vec![0usize];
    for &k in &parent.breaks {
        if k > ib {
            b2.push(k - ib);
        }
    }
    for &k in &parent.breaks {
        if k <= ia {
            b2.push(n - ib + k);
        }
    }
    b2.push(n + ia - ib + 1);

    (
        Region {
            pts: p1,
            breaks: b1,
        },
        Region {
            pts: p2,
            breaks: b2,
        },
    )
}

/// Smooth a region piece by piece and return the cubics plus where each piece
/// starts.
///
/// Each piece uses the OPEN Catmull-Rom, whose end handles are one-sided
/// differences. That is what makes a shared cut come out as the same cubic in
/// both facets: the two children hold the same points in opposite order, and
/// the open form is antisymmetric under reversal.
fn smooth(region: &Region, tau: f64) -> (Vec<Cubic>, Vec<usize>) {
    if region.breaks.len() < 2 {
        return (catmull_rom_closed(&region.pts, tau), vec![0]);
    }
    let mut cubics = Vec::new();
    let mut starts = Vec::with_capacity(region.breaks.len());
    for piece in region.pieces() {
        if piece.len() < 2 {
            continue;
        }
        starts.push(cubics.len());
        cubics.extend(catmull_rom_open(&piece, tau));
    }
    (cubics, starts)
}

/// Make sure no tone comes out vestigial.
///
/// Quantiles cannot leave a tone EMPTY in principle, but with a dozen regions
/// they can leave one holding a single 3% facet where the quota asked for
/// 10%, and a scoria with a vestigial highlight is the two-tone logo the gate
/// exists to reject. Promote whole regions from the mid tone, in light order
/// (the lightest join the highlight, the darkest join the shadow), until each
/// end carries its floor. Deterministic, and it keeps the quantile ordering
/// intact: tone is still monotone in the light function.
fn enforce_tone_floor(tones: &mut [usize], areas: &[f64], order: &[usize], total: f64) {
    let floor = TONE_FLOOR_FRAC * total;
    let share = |tones: &[usize], t: usize| -> f64 {
        tones
            .iter()
            .enumerate()
            .filter(|(_, &x)| x == t)
            .map(|(i, _)| areas[i])
            .sum()
    };
    // The highlight pulls from the light end of the mid tone.
    for _ in 0..order.len() {
        if share(tones, 2) >= floor || share(tones, 1) <= floor {
            break;
        }
        match order.iter().rev().find(|&&i| tones[i] == 1) {
            Some(&i) => tones[i] = 2,
            None => break,
        }
    }
    // The shadow pulls from the dark end.
    for _ in 0..order.len() {
        if share(tones, 0) >= floor || share(tones, 1) <= floor {
            break;
        }
        match order.iter().find(|&&i| tones[i] == 1) {
            Some(&i) => tones[i] = 0,
            None => break,
        }
    }
}

/// Mean Lambert brightness of the solid's real surface over one region.
///
/// Sampled at the centroid and part-way toward [`TONE_SAMPLES`] boundary
/// points, so a facet spanning a brightness ramp is judged by its area rather
/// than by one point of it.
fn mean_brightness(pts: &[V2], centroid: V2, depth: &DepthMap, light: &Light) -> f64 {
    let mut acc = depth.brightness(centroid, light);
    let mut n = 1.0;
    let step = (pts.len() / TONE_SAMPLES).max(1);
    for q in pts.iter().step_by(step).take(TONE_SAMPLES) {
        acc += depth.brightness(centroid + (*q - centroid) * TONE_SAMPLE_REACH, light);
        n += 1.0;
    }
    acc / n
}

/// Assign the three tones by AREA QUANTILE over the light function.
///
/// The light function is Lambert shading of the SOLID'S OWN SURFACE NORMAL,
/// blended by `tone_lambert` against the flat positional ramp the 2D
/// construction used, and then by `tone_noise` against mineral variation.
/// That is the half of the depth read the cut directions do not carry: tone
/// becomes a function of ORIENTATION, so two facets lying on the same plane
/// of the rock get the same tone however far apart they are, and two facets
/// across a fold get different ones however close. A positional ramp cannot
/// do either - it can only make one side of the image darker than the other,
/// which is why it needed so much noise to stop reading as a gradient.
///
/// `tone_lambert` 0 recovers the old behaviour exactly, which is what makes
/// the change measurable rather than a matter of taste.
///
/// Quantiles, not a value threshold: a threshold on the light value can yield
/// zero highlight facets on some seeds, which is a whole class of bad output
/// removed by one line.
#[allow(clippy::too_many_arguments)]
fn tone(
    regions: Vec<Region>,
    p: &Params,
    perlin: &Perlin,
    depth: &DepthMap,
    light: &Light,
    tone_rng: &mut Pcg32,
    c: V2,
    r0: f64,
) -> Vec<Facet> {
    let fbm = Fbm::new(tone_rng, TONE_OCTAVES, p.lacunarity, p.gain);
    // light_dir 1.90 rad points down and slightly left in SVG coordinates: the
    // artwork is lit from BELOW, which is what makes it read as glowing lava
    // rather than as a lit pebble. Not an error to fix.
    let (ls, lc) = sin_cos(p.light_dir);
    let u_light = V2::new(lc, ls);

    let mut centroids = Vec::with_capacity(regions.len());
    let mut areas = Vec::with_capacity(regions.len());
    let mut lambert = Vec::with_capacity(regions.len());
    let mut score = Vec::with_capacity(regions.len());
    for reg in &regions {
        let cen = polygon_centroid(&reg.pts);
        let rel = (cen - c) * (1.0 / r0);
        let ramp = u_light.dot(rel);
        let lam = mean_brightness(&reg.pts, cen, depth, light);
        let shade = (1.0 - p.tone_lambert) * ramp + p.tone_lambert * lam;
        // Unit-RMS, so `tone_noise` really does blend the shading against
        // something of comparable size rather than against a whisper.
        let n =
            crate::body::unit_fbm(fbm.at(perlin, rel.x * TONE_NOISE_FREQ, rel.y * TONE_NOISE_FREQ));
        centroids.push(cen);
        areas.push(reg.area());
        lambert.push(lam);
        score.push((1.0 - p.tone_noise) * shade + p.tone_noise * n);
    }

    let mut order: Vec<usize> = (0..regions.len()).collect();
    // total_cmp with an index tie-break: partial_cmp().unwrap() panics on NaN
    // and leaves ties unordered, and two regions genuinely can tie.
    order.sort_by(|&a, &b| score[a].total_cmp(&score[b]).then(a.cmp(&b)));

    let total: f64 = areas.iter().sum();
    let q0 = p.tone_fractions[0] * total;
    let q1 = (p.tone_fractions[0] + p.tone_fractions[1]) * total;
    let mut tones = vec![1usize; regions.len()];
    let mut acc = 0.0;
    for &i in &order {
        // Bin on the MIDPOINT of the region's slice of the cumulative area, so
        // one large region does not get pushed a whole tone by its own size.
        let mid = acc + 0.5 * areas[i];
        acc += areas[i];
        tones[i] = if mid < q0 {
            0
        } else if mid < q1 {
            1
        } else {
            2
        };
    }

    enforce_tone_floor(&mut tones, &areas, &order, total);

    let mut out = Vec::with_capacity(regions.len());
    for (i, reg) in regions.into_iter().enumerate() {
        let (mut cubics, piece_starts) = smooth(&reg, p.cr_tension);
        let clamped = clamp_handles(&mut cubics);
        out.push(Facet {
            pts: reg.pts,
            breaks: reg.breaks,
            cubics,
            piece_starts,
            centroid: centroids[i],
            area: areas[i],
            tone: tones[i],
            lambert: lambert[i],
            clamped,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A regular n-gon of radius 100 about the origin, as a stand-in root.
    fn ngon(n: usize) -> Vec<V2> {
        (0..n)
            .map(|i| {
                let a = crate::math::TAU * i as f64 / n as f64;
                let (s, c) = sin_cos(a);
                V2::new(100.0 * c, 100.0 * s)
            })
            .collect()
    }

    #[test]
    fn split_conserves_area() {
        let pts = ngon(32);
        let parent = Region {
            pts: pts.clone(),
            breaks: Vec::new(),
        };
        let (ia, ib) = (2usize, 18usize);
        let xa = pts[ia].lerp(pts[ia + 1], 0.5);
        let xb = pts[ib].lerp(pts[ib + 1], 0.5);
        let cut = vec![xa, xa.lerp(xb, 0.5), xb];
        let (c1, c2) = split(&parent, ia, xa, ib, xb, &cut);
        let total = parent.area();
        let sum = c1.area() + c2.area();
        assert!(
            (total - sum).abs() < 1e-9 * total,
            "parent {total} vs children {sum}"
        );
    }

    #[test]
    fn the_shared_edge_is_one_curve_in_both_children() {
        let pts = ngon(32);
        let parent = Region {
            pts: pts.clone(),
            breaks: Vec::new(),
        };
        let (ia, ib) = (3usize, 20usize);
        let xa = pts[ia].lerp(pts[ia + 1], 0.37);
        let xb = pts[ib].lerp(pts[ib + 1], 0.61);
        let cut: Vec<V2> = (0..=8).map(|k| xa.lerp(xb, k as f64 / 8.0)).collect();
        let (c1, c2) = split(&parent, ia, xa, ib, xb, &cut);

        // The cut piece is the LAST piece of each child.
        let p1 = c1.pieces();
        let p2 = c2.pieces();
        let cut1 = p1.last().expect("child 1 has a cut piece");
        let cut2 = p2.last().expect("child 2 has a cut piece");
        assert_eq!(cut1.len(), cut2.len());
        for (a, b) in cut1.iter().zip(cut2.iter().rev()) {
            // Bit-identical, not merely close: the whole hairline argument
            // rests on these being the same f64s.
            assert_eq!(a.x, b.x);
            assert_eq!(a.y, b.y);
        }
        // ... and therefore the smoothed cubics agree too.
        let s1 = catmull_rom_open(cut1, 0.5);
        let s2 = catmull_rom_open(cut2, 0.5);
        assert_eq!(s1.len(), s2.len());
        for (a, b) in s1.iter().zip(s2.iter().rev()) {
            assert_eq!(a.p0.x, b.p3.x);
            assert_eq!(a.p0.y, b.p3.y);
            assert_eq!(a.p3.x, b.p0.x);
            assert_eq!(a.c1.x, b.c2.x);
            assert_eq!(a.c2.x, b.c1.x);
        }
    }

    #[test]
    fn pieces_cover_the_loop_exactly_once() {
        let pts = ngon(24);
        let parent = Region {
            pts: pts.clone(),
            breaks: Vec::new(),
        };
        let (ia, ib) = (1usize, 13usize);
        let xa = pts[ia].lerp(pts[ia + 1], 0.5);
        let xb = pts[ib].lerp(pts[ib + 1], 0.5);
        let cut = vec![xa, xa.lerp(xb, 0.4), xa.lerp(xb, 0.7), xb];
        let (c1, c2) = split(&parent, ia, xa, ib, xb, &cut);
        for child in [&c1, &c2] {
            let n = child.pts.len();
            let pieces = child.pieces();
            // Every piece shares its endpoints with its neighbours, so the
            // total length over-counts by exactly one per piece.
            let covered: usize = pieces.iter().map(|p| p.len() - 1).sum();
            assert_eq!(covered, n, "pieces must tile the loop");
            assert!(
                child.breaks.windows(2).all(|w| w[0] < w[1]),
                "breaks ascend"
            );
            assert_eq!(child.breaks[0], 0);
            assert!(child.breaks.iter().all(|&b| b < n));
        }
    }

    #[test]
    fn breaks_are_inherited_through_a_second_split() {
        let pts = ngon(40);
        let parent = Region {
            pts: pts.clone(),
            breaks: Vec::new(),
        };
        let (ia, ib) = (2usize, 22usize);
        let xa = pts[ia].lerp(pts[ia + 1], 0.5);
        let xb = pts[ib].lerp(pts[ib + 1], 0.5);
        let cut: Vec<V2> = (0..=6).map(|k| xa.lerp(xb, k as f64 / 6.0)).collect();
        let (c1, _c2) = split(&parent, ia, xa, ib, xb, &cut);
        assert_eq!(c1.breaks.len(), 2, "one boundary arc plus one cut");

        // Cut child 1 again, across both of its pieces.
        let n1 = c1.pts.len();
        let (ja, jb) = (3usize, n1 - 3);
        let ya = c1.pts[ja].lerp(c1.pts[ja + 1], 0.5);
        let yb = c1.pts[jb].lerp(c1.pts[jb + 1], 0.5);
        let cut2: Vec<V2> = (0..=5).map(|k| ya.lerp(yb, k as f64 / 5.0)).collect();
        let (g1, g2) = split(&c1, ja, ya, jb, yb, &cut2);
        for child in [&g1, &g2] {
            assert!(child.breaks.len() >= 2);
            assert!(
                child.breaks.windows(2).all(|w| w[0] < w[1]),
                "{:?}",
                child.breaks
            );
            assert!(child.breaks.iter().all(|&b| b < child.pts.len()));
            let covered: usize = child.pieces().iter().map(|p| p.len() - 1).sum();
            assert_eq!(covered, child.pts.len());
        }
        assert!(
            (c1.area() - g1.area() - g2.area()).abs() < 1e-9 * c1.area(),
            "area is conserved through the second split too"
        );
    }

    #[test]
    fn junction_angle_rejects_a_grazing_cut() {
        let pts = ngon(32);
        let x = pts[0].lerp(pts[1], 0.5);
        // Along the edge: grazing, must be rejected.
        let grazing = x.lerp(pts[1], 0.9);
        assert!(!junction_ok(&pts, 0, x, grazing));
        // Toward the centre: a right angle, must be accepted.
        assert!(junction_ok(&pts, 0, x, V2::new(0.0, 0.0)));
    }

    #[test]
    fn seg_grid_finds_every_crossing() {
        let pts = ngon(64);
        let grid = SegGrid::new(&pts, 20.0);
        let mut out = Vec::new();
        // A ray from the centre must cross exactly one boundary segment. The
        // offset is deliberately not a round fraction: 16 divides 64, so any
        // tidy offset puts the ray through a vertex, where it meets two
        // segments at their shared endpoint and proves nothing.
        for k in 0..16 {
            let a = crate::math::TAU * (k as f64 + 0.31) / 16.0;
            let (s, c) = sin_cos(a);
            let far = V2::new(200.0 * c, 200.0 * s);
            grid.query(V2::new(0.0, 0.0), far, &mut out);
            let n = pts.len();
            let hits = out
                .iter()
                .filter(|&&i| {
                    seg_intersect(
                        V2::new(0.0, 0.0),
                        far,
                        pts[i as usize],
                        pts[(i as usize + 1) % n],
                    )
                    .is_some()
                })
                .count();
            assert_eq!(hits, 1, "ray {k} crossed {hits} segments");
        }
    }

    #[test]
    fn seg_grid_query_is_deduplicated() {
        let pts = ngon(48);
        let grid = SegGrid::new(&pts, 5.0);
        let mut out = Vec::new();
        grid.query(V2::new(-120.0, -120.0), V2::new(120.0, 120.0), &mut out);
        let mut sorted = out.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(out, sorted, "a duplicate would read as a second crossing");
    }
}
