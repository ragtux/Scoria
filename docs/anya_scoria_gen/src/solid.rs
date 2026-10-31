//! The 3D rock the drawing is a view of.
//!
//! Everything downstream — the silhouette, the facet cuts, the three flat
//! tones, the relief marks — is derived from one bumpy solid seen in
//! orthographic projection down the `+z` axis. That is the whole point of this
//! module: the flatness of the output is a *rendering* choice applied to real
//! geometry, not a drawing trick, so the tones agree with the shape because
//! they are computed from it.
//!
//! # The solid
//!
//! A radial displacement of an ellipsoid, in exactly the form the 2D
//! silhouette used to be:
//!
//! ```text
//!   R(d) = rho_ellipsoid(d) * (1 + amp * F(d))        d a unit 3-vector
//!   S(d) = C + R(d) * d
//! ```
//!
//! with `F` the sphere-sampled fBm of [`SphereFbm`]. `R` is a single-valued
//! positive function of direction, so the solid is star-shaped about `C`.
//!
//! # Why the projection keeps the crate's one theorem
//!
//! The 2D construction this replaces rested on a theorem: a curve written as
//! `r(theta) * (cos theta, sin theta)` with `r > 0` single-valued is
//! star-shaped and therefore cannot self-intersect, which is what makes the
//! outline ribbon a scalar subtraction and the inside-test O(1). Projecting a
//! 3D body would normally throw that away. It does not here, and the argument
//! is three lines:
//!
//! 1. Every direction whose image-plane projection points along `theta` is
//!    `V3::dir(theta, u)` for exactly one `u` in `[-PI/2, PI/2]`, and that
//!    point's projected radius is `R * cos u`.
//! 2. The solid is star-shaped about `C`, so the segment from `C` to any
//!    surface point lies inside it; projection maps that segment to the
//!    segment from the projected centre to the projected point. So the
//!    projected region is star-shaped about the projected centre.
//! 3. Its radius at image angle `theta` is therefore exactly
//!    `max over u of R(dir(theta, u)) * cos u` — single-valued and positive.
//!
//! So the silhouette is still `r(theta)`, still star-shaped, still
//! non-self-intersecting by construction, and the ribbon is still a
//! subtraction. The silhouette is now the apparent contour of a solid rather
//! than a noise curve, and a near-side bump genuinely pokes out past the base
//! ellipse, which is the thing a 2D radial profile cannot do.
//!
//! # The depth map
//!
//! Shading and crease-finding need the front surface as a height field over
//! the image, which means inverting `R(dir(theta, u)) * cos u = rho` for `u`.
//! Doing that per query by bisection costs ten noise evaluations a time and
//! there are thousands of queries. Instead [`DepthMap`] MARCHES the
//! parameterisation: for each grid angle it walks `u` from the silhouette's
//! own argmax to the pole, which sweeps the front surface from rim to centre
//! in order, and records the surface point at each fixed fraction of the
//! silhouette radius. One noise evaluation per sample, no inversion at all,
//! and the result lands exactly on the silhouette at the rim because it is
//! built from the same `u` the silhouette's maximisation found.
//!
//! Normals come from the grid's own neighbours — the stored points are a
//! quadrilateral mesh in `(theta, t)`, so two differences give two tangents.
//! That costs no further noise evaluations and, unlike differencing the depth
//! in image space, it stays well conditioned at the rim, where the surface is
//! tangent to the view and `dz/dx` is infinite.

use crate::math::{atan2, sin_cos, PI, TAU, V2, V3};
use crate::noise::{Perlin, SphereFbm};

/// How far out of the image plane the silhouette search looks, in radians.
///
/// `R * cos u` at `u = 1.2` is 0.36 of `R`, so a maximum could only hide past
/// this if some direction's radius were nearly three times the equator's —
/// impossible while `amp < 0.5`, and [`Solid::silhouette`] asserts the
/// maximum is interior in debug builds.
const U_LIMIT: f64 = 1.2;

/// Coarse samples of `u` per silhouette angle.
///
/// `R * cos u` is NOT unimodal in `u`, and that is the feature rather than a
/// problem: a second maximum is a near-side bump poking past the base
/// ellipse, which is exactly the 3D read a 2D radial profile cannot produce.
/// So the search has to scan rather than climb. 19 samples put the stride at
/// 0.126 rad, which resolves the third octave of the displacement (wavelength
/// about 0.3 rad at the default `lump_freq`); the fourth carries `gain^3` =
/// 0.09 of the amplitude, about two units of radius, and the ring smoothing
/// that follows attenuates what is missed well below that.
const U_COARSE: usize = 19;

/// Golden-section refinements after the coarse scan.
///
/// Eight steps shrink the 0.126-rad bracket to 0.0072 rad. Near a smooth
/// maximum that is a relative radius error of 2.6e-5 — 0.008 units at
/// `R0 = 300`, four times the output quantum but two orders under the
/// smoothing that follows, and deterministic either way.
const U_REFINE: usize = 8;

/// `1/phi`, the golden-section ratio. A literal, so no `sqrt` runs per call.
const INV_PHI: f64 = 0.618_033_988_749_894_8;

/// Spacing of the brightness gradient's central difference, as a fraction of
/// R0.
///
/// 0.04 is twelve canonical units, which is wider than a depth-map cell and
/// deliberately so. A bilinear sample of a grid is only C0, so its central
/// difference is piecewise constant per cell and its DIRECTION jumps at every
/// cell boundary. A difference narrower than a cell measures that jump; one
/// wider than a cell averages over it. Measured at six units: the chatter
/// accumulated enough absolute turning along a 3-unit Euler walk to fail the
/// no-self-intersection bound on 917 cuts out of 200 seeds, with the cut
/// still looking locally plausible - the direction oscillated rather than
/// wandered, so no single step was wrong.
///
/// Shared by the cut tracer and the relief pass, because a cut that curved to
/// one scale while the crease was scored at another would disagree with
/// itself about where the fold is.
pub const GRAD_H_FRAC: f64 = 0.04;

/// Radial grid nodes per angle in a [`DepthMap`], plus one for the centre.
///
/// Sized against the gradient step rather than against the image: at 24 rows
/// on a body of mean radius R0 the nodes sit about 12 units apart, which is
/// [`GRAD_H_FRAC`] exactly, so the central difference always spans a cell and
/// never measures the interpolation's own seams. Finer costs one noise
/// evaluation per added node and resolves nothing the difference can see.
const DEPTH_RAD: usize = 24;

/// Binomial smoothing passes over the normal grid.
///
/// The band-limiter for the shading, and the companion to the wide gradient
/// step above. The marched mesh resolves the displacement's finest octave at
/// a scale finer than a facet, so the raw normals carry variation that the
/// three-tone quantisation cannot represent and that only shows up as
/// instability in which tone a facet lands in and which way a cut leans. Two
/// passes over a 240 x 33 grid is a window of about three degrees of arc -
/// the same order as the silhouette's own smoothing, and for the same reason.
const NORMAL_SMOOTH_PASSES: usize = 2;

/// Samples of `u` the depth march takes per angle.
///
/// Twice the radial resolution, so every radial node is bracketed by a step
/// of the march and the grid has no holes to fill. A fold — a near bump
/// occluding a farther part of the surface along the same ray — resolves
/// correctly because the march keeps the LARGEST depth at each node.
const DEPTH_MARCH: usize = 48;

/// A bumpy 3D rock: an ellipsoid displaced along its own radii.
#[derive(Clone, Debug)]
pub struct Solid {
    /// Semi-axis along image `x`.
    pub a: f64,
    /// Semi-axis along image `y`.
    pub b: f64,
    /// Semi-axis along the view direction `z`.
    pub c: f64,
    /// Displacement amplitude as a fraction of the ellipsoid radius.
    pub amp: f64,
    /// Displacement lobes per turn around the equator.
    pub lobes: f64,
    /// Floor on the radius, as a fraction of the ellipsoid's own radius.
    /// Keeps `R > 0` — and with it star-shapedness — under any amplitude.
    pub floor: f64,
    /// The displacement field, sampled on the unit sphere.
    pub fbm: SphereFbm,
    /// Amplitude of the flow-field domain warp on the noise lookup, in
    /// noise-space units.
    pub warp: f64,
}

impl Solid {
    /// The base ellipsoid's radius in unit direction `d`.
    #[must_use]
    pub fn ellipsoid(&self, d: V3) -> f64 {
        // 1/sqrt((dx/a)^2 + (dy/b)^2 + (dz/c)^2): the ellipsoid in polar form.
        let qx = d.x / self.a;
        let qy = d.y / self.b;
        let qz = d.z / self.c;
        let q = (qx * qx + qy * qy + qz * qz).sqrt();
        if q > 0.0 {
            1.0 / q
        } else {
            0.0
        }
    }

    /// The displaced radius in unit direction `d`.
    ///
    /// `warp_dir` displaces the noise LOOKUP, not the geometry, so the result
    /// is still a scalar function of direction and the solid is still
    /// star-shaped. This is the same decision the 2D silhouette made, and it
    /// is the one the projection theorem in this module's documentation
    /// depends on.
    #[must_use]
    pub fn radius_warped(&self, perlin: &Perlin, d: V3, warp_dir: V3) -> f64 {
        let rho = self.ellipsoid(d);
        let f = crate::body::unit_fbm(self.fbm.at(perlin, d, self.lobes, warp_dir * self.warp));
        (rho * (1.0 + self.amp * f)).max(rho * self.floor)
    }

    /// [`Solid::radius_warped`] with no warp — the form every consumer but
    /// the silhouette uses, since the warp is a function of the boundary
    /// estimate and only the silhouette has one.
    #[must_use]
    pub fn radius(&self, perlin: &Perlin, d: V3) -> f64 {
        self.radius_warped(perlin, d, V3::new(0.0, 0.0, 0.0))
    }

    /// The surface point in unit direction `d`, relative to the centre.
    #[must_use]
    pub fn surface(&self, perlin: &Perlin, d: V3) -> V3 {
        d * self.radius(perlin, d)
    }

    /// The projected silhouette radius at image angle `theta`, and the `u`
    /// that achieves it.
    ///
    /// `max over u of R(dir(theta, u)) * cos u` — see the module docs for why
    /// this is the apparent contour and why it is single-valued. A coarse
    /// scan finds the right lobe, golden section refines inside it.
    ///
    /// `warp_dir` is evaluated once by the caller at its unwarped boundary
    /// estimate and held fixed across the search, so the warp cannot make the
    /// maximisation non-deterministic in the search path.
    #[must_use]
    pub fn silhouette(&self, perlin: &Perlin, theta: f64, warp_dir: V3) -> (f64, f64) {
        let (st, ct) = sin_cos(theta);
        let f = |u: f64| -> f64 {
            let d = V3::dir_at(st, ct, u);
            // cos u is the projection factor; dir() already paid for it, but
            // recomputing from the direction costs nothing and keeps the two
            // in exact agreement.
            let cu = (d.x * d.x + d.y * d.y).sqrt();
            self.radius_warped(perlin, d, warp_dir) * cu
        };

        let step = 2.0 * U_LIMIT / (U_COARSE - 1) as f64;
        let mut best = 0usize;
        let mut best_v = f64::NEG_INFINITY;
        let mut vals = [0.0f64; U_COARSE];
        for (i, v) in vals.iter_mut().enumerate() {
            *v = f(-U_LIMIT + step * i as f64);
            if *v > best_v {
                best_v = *v;
                best = i;
            }
        }
        debug_assert!(
            best > 0 && best < U_COARSE - 1,
            "the silhouette maximum escaped U_LIMIT at theta {theta}"
        );

        // Golden section on the bracket either side of the coarse winner. The
        // bracket is guaranteed to contain a maximum of the sampled sequence;
        // on the rare multi-modal bracket this converges to one of them,
        // which is still a point ON the surface, so the silhouette can only
        // be understated by the fourth octave's amplitude.
        let mut lo = -U_LIMIT + step * (best.saturating_sub(1)) as f64;
        let mut hi = -U_LIMIT + step * (best + 1).min(U_COARSE - 1) as f64;
        let mut x1 = hi - (hi - lo) * INV_PHI;
        let mut x2 = lo + (hi - lo) * INV_PHI;
        let mut f1 = f(x1);
        let mut f2 = f(x2);
        for _ in 0..U_REFINE {
            if f1 < f2 {
                lo = x1;
                x1 = x2;
                f1 = f2;
                x2 = lo + (hi - lo) * INV_PHI;
                f2 = f(x2);
            } else {
                hi = x2;
                x2 = x1;
                f2 = f1;
                x1 = hi - (hi - lo) * INV_PHI;
                f1 = f(x1);
            }
        }
        let (u, v) = if f1 >= f2 { (x1, f1) } else { (x2, f2) };
        // The coarse winner is kept if refinement somehow did worse, so the
        // returned radius is never below a value actually sampled.
        if best_v > v {
            (best_v, -U_LIMIT + step * best as f64)
        } else {
            (v, u)
        }
    }
}

/// A 3D light direction, pre-normalised.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// The unit direction light arrives FROM.
    pub dir: V3,
}

impl Light {
    /// From the image-plane angle the artwork was measured at and an
    /// elevation toward the viewer.
    ///
    /// `angle` is in SVG coordinates, so the measured 1.90 rad points down
    /// and slightly left: the artwork is lit from BELOW, which is what makes
    /// it read as glowing lava rather than as a lit pebble. `elev` is the
    /// component toward the viewer; keeping it small holds the light grazing,
    /// which is what spreads the three tones across the body instead of
    /// flooding the whole face with the mid tone.
    #[must_use]
    pub fn new(angle: f64, elev: f64) -> Light {
        let (s, c) = sin_cos(angle);
        Light {
            dir: V3::new(c, s, elev).norm(),
        }
    }

    /// Lambert brightness of a surface normal, in `[-1, 1]`.
    #[must_use]
    pub fn shade(&self, n: V3) -> f64 {
        n.dot(self.dir)
    }
}

/// The front surface of a [`Solid`], as a polar mesh over the image.
///
/// Node `(i, k)` is the surface point whose projection lies at image angle
/// `theta_i` and radius `k / DEPTH_RAD` of the silhouette radius there. So
/// `k = 0` is the pole facing the viewer, `k = DEPTH_RAD` is the rim, and the
/// grid is aligned to the shape rather than to the image axes — which is what
/// keeps the normals well conditioned at the rim.
#[derive(Clone, Debug)]
pub struct DepthMap {
    /// Image-plane centre the polar grid is measured from.
    pub c: V2,
    /// Angular resolution.
    pub n_theta: usize,
    /// Silhouette radius at each grid angle.
    pub rho: Vec<f64>,
    /// Surface points, `n_theta * (DEPTH_RAD + 1)`, row-major by angle.
    pts: Vec<V3>,
    /// Outward unit normals at the same nodes.
    nrm: Vec<V3>,
}

impl DepthMap {
    /// Radial nodes per angle, excluding the centre.
    #[must_use]
    pub fn n_rad() -> usize {
        DEPTH_RAD
    }

    /// Build the map by marching `u` from the rim to the pole.
    ///
    /// `rho_at` and `u_at` are the silhouette's own radius and argmax,
    /// sampled at the caller's angular resolution and interpolated here, so
    /// the rim row of the mesh lands exactly on the emitted silhouette rather
    /// than on a second, slightly different evaluation of it.
    ///
    /// `scale` is the factor the caller applied to the silhouette after
    /// building it. A uniform scale commutes with the whole construction -
    /// scaling a star-shaped solid scales its projection by the same factor
    /// and leaves the argmax `u` and every surface normal untouched - so
    /// passing it here is enough to keep the map and the drawn outline the
    /// same surface.
    ///
    /// One inexactness worth naming: `rho_at` has been through the limiter
    /// and the band-limiting smoothing passes, so it is not exactly the
    /// solid's own projected radius any more, and the rim row is pinned to
    /// `t = 1` rather than to wherever the unsmoothed surface point projects.
    /// That is the right way round. The map is parameterised by the outline a
    /// viewer sees, so `t = 1` is the DRAWN rim and the normal there is the
    /// normal of the surface that was drawn.
    #[must_use]
    pub fn build(
        solid: &Solid,
        perlin: &Perlin,
        c: V2,
        n_theta: usize,
        rho_at: &[f64],
        u_at: &[f64],
        scale: f64,
    ) -> DepthMap {
        let n_theta = n_theta.max(8);
        let rows = DEPTH_RAD + 1;
        let mut pts = vec![V3::new(0.0, 0.0, 0.0); n_theta * rows];
        let mut rho = Vec::with_capacity(n_theta);

        for i in 0..n_theta {
            let theta = TAU * i as f64 / n_theta as f64;
            let (st, ct) = sin_cos(theta);
            let rho_i = lerp_wrapped(rho_at, theta);
            let u_star = lerp_wrapped(u_at, theta);
            rho.push(rho_i);

            // The march. `u` from the silhouette's argmax to the pole sweeps
            // the front surface from rim to centre; `z` grows with `u` all
            // the way, so writing the LAST crossing of each radial node keeps
            // the nearest surface where a bump folds over a farther one.
            let row = &mut pts[i * rows..(i + 1) * rows];
            let mut seen = [false; DEPTH_RAD + 1];
            let mut prev_t = 1.0f64;
            let mut prev_p = {
                let d = V3::dir_at(st, ct, u_star);
                d * (solid.radius(perlin, d) * scale)
            };
            // The rim node is exactly the silhouette point, by construction.
            row[DEPTH_RAD] = prev_p;
            seen[DEPTH_RAD] = true;

            for j in 1..=DEPTH_MARCH {
                let u = u_star + (PI / 2.0 - u_star) * (j as f64 / DEPTH_MARCH as f64);
                let d = V3::dir_at(st, ct, u);
                let p = d * (solid.radius(perlin, d) * scale);
                let t = if rho_i > 0.0 {
                    (p.xy().len() / rho_i).min(1.0)
                } else {
                    0.0
                };
                // Every radial node this step straddles, in either direction.
                let (t_lo, t_hi) = if t < prev_t { (t, prev_t) } else { (prev_t, t) };
                let k_lo = (t_lo * DEPTH_RAD as f64).ceil().max(0.0) as usize;
                let k_hi = (t_hi * DEPTH_RAD as f64).floor().min(DEPTH_RAD as f64) as usize;
                for k in k_lo..=k_hi.min(DEPTH_RAD) {
                    let target = k as f64 / DEPTH_RAD as f64;
                    let span = t - prev_t;
                    let w = if span.abs() > 1e-12 {
                        ((target - prev_t) / span).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let q = prev_p + (p - prev_p) * w;
                    // Last write wins, which is the nearest surface: `z`
                    // increases along the march.
                    if !seen[k] || q.z > row[k].z {
                        row[k] = q;
                        seen[k] = true;
                    }
                }
                prev_t = t;
                prev_p = p;
            }

            // The pole. Reached exactly at u = PI/2, but assert the march
            // filled everything rather than trusting it: an unfilled node
            // would read as a dent in the shading that nothing explains.
            for k in 0..=DEPTH_RAD {
                if !seen[k] {
                    // Linear fill from the nearest filled neighbours; the
                    // march's resolution makes this unreachable in practice.
                    let lo = (0..=k).rev().find(|&q| seen[q]);
                    let hi = (k..=DEPTH_RAD).find(|&q| seen[q]);
                    row[k] = match (lo, hi) {
                        (Some(l), Some(h)) if h > l => {
                            let w = (k - l) as f64 / (h - l) as f64;
                            row[l] + (row[h] - row[l]) * w
                        }
                        (Some(l), _) => row[l],
                        (_, Some(h)) => row[h],
                        _ => V3::new(0.0, 0.0, 0.0),
                    };
                }
            }
        }

        let mut nrm = normals(&pts, n_theta, rows);
        for _ in 0..NORMAL_SMOOTH_PASSES {
            smooth_normals(&mut nrm, n_theta, rows);
        }
        DepthMap {
            c,
            n_theta,
            rho,
            pts,
            nrm,
        }
    }

    /// The raw node grids, for the gates: surface points and their outward
    /// normals, in the same order.
    #[must_use]
    pub fn nodes(&self) -> (&[V3], &[V3]) {
        (&self.pts, &self.nrm)
    }

    /// Bilinear sample of the surface point and outward normal under image
    /// point `q`.
    ///
    /// Normals are interpolated as VECTORS and renormalised. Interpolating an
    /// angle instead spins a full turn wherever the field crosses `±PI`,
    /// which is the single most common flow-field bug and is invisible in a
    /// debug render of the field.
    #[must_use]
    pub fn sample(&self, q: V2) -> (V3, V3) {
        let rows = DEPTH_RAD + 1;
        let rel = q - self.c;
        let mut th = atan2(rel.y, rel.x);
        if !th.is_finite() {
            th = 0.0;
        }
        if th < 0.0 {
            th += TAU;
        }
        let fi = th * (self.n_theta as f64 / TAU);
        let i0 = (fi.floor() as usize) % self.n_theta;
        let i1 = (i0 + 1) % self.n_theta;
        let wi = fi - fi.floor();

        let rho = self.rho[i0] + (self.rho[i1] - self.rho[i0]) * wi;
        let t = if rho > 1e-9 {
            (rel.len() / rho).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let fk = t * DEPTH_RAD as f64;
        let k0 = (fk.floor() as usize).min(DEPTH_RAD);
        let k1 = (k0 + 1).min(DEPTH_RAD);
        let wk = fk - fk.floor();

        let bl = |g: &[V3]| -> V3 {
            let p00 = g[i0 * rows + k0];
            let p01 = g[i0 * rows + k1];
            let p10 = g[i1 * rows + k0];
            let p11 = g[i1 * rows + k1];
            let a = p00 + (p01 - p00) * wk;
            let b = p10 + (p11 - p10) * wk;
            a + (b - a) * wi
        };
        (bl(&self.pts), bl(&self.nrm).norm())
    }

    /// Lambert brightness under image point `q`.
    #[must_use]
    pub fn brightness(&self, q: V2, light: &Light) -> f64 {
        light.shade(self.sample(q).1)
    }

    /// Depth under image point `q`.
    #[must_use]
    pub fn depth(&self, q: V2) -> f64 {
        self.sample(q).0.z
    }

    /// The image-plane gradient of the brightness at `q`, by central
    /// difference over `h`.
    #[must_use]
    pub fn brightness_grad(&self, q: V2, light: &Light, h: f64) -> V2 {
        let dx = self.brightness(q + V2::new(h, 0.0), light)
            - self.brightness(q - V2::new(h, 0.0), light);
        let dy = self.brightness(q + V2::new(0.0, h), light)
            - self.brightness(q - V2::new(0.0, h), light);
        V2::new(dx, dy) * (0.5 / h)
    }

    /// How hard the lit surface FOLDS under `q`, and which way the fold runs.
    ///
    /// A fold is a crease: a line along which the surface's brightness
    /// barely changes, with the brightness falling away sharply to either
    /// side of it. That is a statement about second derivatives, so this
    /// takes the Hessian of the brightness by finite difference and
    /// diagonalises it. The larger principal curvature in magnitude says how
    /// sharp the crease is; the eigenvector of the SMALLER one says which
    /// way it runs, because the crest of a ridge is the direction the
    /// brightness changes least along.
    ///
    /// Why the second derivative and not the gradient: the brightness
    /// gradient is largest near the rim, where the surface rolls away from
    /// the viewer, and it is large there whether or not anything is
    /// happening. Scoring a fold by `|grad B|` therefore finds the rim,
    /// every time. A smooth roll-off is not a crease however steep it is.
    ///
    /// Returns `None` where the surface is too flat to have a direction.
    #[must_use]
    pub fn fold(&self, q: V2, light: &Light, h: f64) -> Option<Fold> {
        let b = |dx: f64, dy: f64| self.brightness(q + V2::new(dx * h, dy * h), light);
        let b0 = b(0.0, 0.0);
        let inv = 1.0 / (h * h);
        let hxx = (b(1.0, 0.0) - 2.0 * b0 + b(-1.0, 0.0)) * inv;
        let hyy = (b(0.0, 1.0) - 2.0 * b0 + b(0.0, -1.0)) * inv;
        let hxy = (b(1.0, 1.0) - b(1.0, -1.0) - b(-1.0, 1.0) + b(-1.0, -1.0)) * (0.25 * inv);

        // Symmetric 2x2 eigenvalues, closed form: no trigonometry, one sqrt.
        let half_tr = 0.5 * (hxx + hyy);
        let d = 0.5 * (hxx - hyy);
        let disc = (d * d + hxy * hxy).sqrt();
        let (lo, hi) = (half_tr - disc, half_tr + disc);
        // The eigenvector of whichever is SMALLER in magnitude - the crest.
        let small = if lo.abs() <= hi.abs() { lo } else { hi };
        let strength = if lo.abs() <= hi.abs() { hi } else { lo }.abs();
        if !strength.is_finite() || strength <= 0.0 {
            return None;
        }
        // `(hxy, lambda - hxx)` is an eigenvector unless both vanish, in
        // which case the Hessian is isotropic and `(lambda - hyy, hxy)` is
        // the one that survives.
        let v = V2::new(hxy, small - hxx);
        let along = if v.len2() > 1e-24 {
            v.norm()
        } else {
            let w = V2::new(small - hyy, hxy);
            if w.len2() > 1e-24 {
                w.norm()
            } else {
                return None;
            }
        };
        Some(Fold { strength, along })
    }

    /// The unit direction along which the brightness does not change.
    ///
    /// This is the direction a facet cut follows, and it is the whole of the
    /// harmonisation between shape and shading: a boundary drawn along an
    /// iso-brightness line means the flat tone either side of it is the
    /// quantisation of a real lit surface, so the cut falls exactly where the
    /// shading would have stepped anyway. A cut across the gradient instead
    /// reads as damage to the rock rather than as its form.
    ///
    /// Returns `None` where the gradient is too small to orient — a flat
    /// plateau, where any direction is an iso-line and the caller should keep
    /// whatever it already had.
    #[must_use]
    pub fn iso_dir(&self, q: V2, light: &Light, h: f64) -> Option<V2> {
        let g = self.brightness_grad(q, light, h);
        let l = g.len();
        if l < 1e-6 {
            return None;
        }
        Some(g.perp() * (1.0 / l))
    }
}

/// A crease in the lit surface: how sharp, and which way it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fold {
    /// The larger principal curvature of the brightness, in magnitude. Units
    /// of 1/length^2, so a caller comparing folds across logos should scale
    /// it by `R0^2`.
    pub strength: f64,
    /// Unit direction along the crest, up to sign - the crest has no
    /// inherent heading, so a caller walking one has to carry its own.
    pub along: V2,
}

/// Mesh normals from the stored points.
///
/// The grid is a quadrilateral mesh on the surface, so two neighbour
/// differences give two tangents and their cross product gives the normal,
/// with no further noise evaluations and nothing to invert. Differencing the
/// depth in IMAGE space instead would be simpler to write and wrong where it
/// matters: at the rim the surface is tangent to the view, `dz/dx` is
/// infinite, and a finite difference there flattens the normal toward the
/// viewer exactly where the rock should be turning away.
fn normals(pts: &[V3], n_theta: usize, rows: usize) -> Vec<V3> {
    let mut out = vec![V3::new(0.0, 0.0, 1.0); pts.len()];
    for i in 0..n_theta {
        let ip = (i + 1) % n_theta;
        let im = (i + n_theta - 1) % n_theta;
        for k in 0..rows {
            let p = pts[i * rows + k];
            // Angular tangent wraps; radial is one-sided at both ends.
            let t_theta = pts[ip * rows + k] - pts[im * rows + k];

            // THE RIM IS EXACT, not differenced. At the apparent contour the
            // surface is tangent to the view, so its normal is perpendicular
            // to the view direction - it lies flat in the image plane,
            // pointing out of the silhouette. That is a closed form:
            // `view x contour_tangent`, which with the view down `+z` is just
            // the in-plane perpendicular of the contour.
            //
            // Differencing it instead is wrong in a way that matters. The
            // last row has only a one-sided radial neighbour, and near the
            // rim the surface is folding over, so the chord to the row
            // inside is nowhere near tangent - the cross product comes out
            // tilted, often with the normal facing slightly AWAY from the
            // viewer. Measured: that put 91 of 120 rolls at the `anya`
            // preset outside the depth map's own sanity gate, on the row
            // that carries the rim's shading and every relief mark's rim
            // end.
            if k == rows - 1 {
                let t = t_theta.xy();
                let n = if t.len2() > 1e-18 {
                    let perp = t.perp().norm();
                    // Outward: away from the body's centre, which the
                    // projected point measures from the origin of this grid.
                    let outward = if perp.dot(p.xy()) < 0.0 { -perp } else { perp };
                    V3::new(outward.x, outward.y, 0.0)
                } else {
                    p.norm()
                };
                out[i * rows + k] = n;
                continue;
            }

            let t_rad = if k == 0 {
                pts[i * rows + 1] - p
            } else {
                pts[i * rows + k + 1] - pts[i * rows + k - 1]
            };
            // The radial tangent degenerates at the pole, where every angle
            // meets; fall back to the point's own direction, which is the
            // normal of a sphere there and within a lump of it for a solid.
            let n = if t_rad.len2() < 1e-18 || t_theta.len2() < 1e-18 {
                p.norm()
            } else {
                V3::surface_normal(t_rad, t_theta, p)
            };
            out[i * rows + k] = n;
        }
    }
    out
}

/// One five-tap binomial pass over the normal grid, wrapping in angle and
/// clamped at both radial ends.
///
/// Vectors are averaged and renormalised, never angles: averaging angles
/// spins a full turn wherever the field crosses +-PI, and the result is
/// invisible in a debug render of the field.
fn smooth_normals(nrm: &mut [V3], n_theta: usize, rows: usize) {
    let src = nrm.to_vec();
    let at = |i: usize, k: isize| -> V3 {
        let kk = k.clamp(0, rows as isize - 1) as usize;
        src[i * rows + kk]
    };
    for i in 0..n_theta {
        let ip = (i + 1) % n_theta;
        let im = (i + n_theta - 1) % n_theta;
        for k in 0..rows {
            let ki = k as isize;
            // Separable 1-2-1 in each direction, applied as one 9-tap stencil.
            let sum = at(i, ki) * 4.0
                + (at(ip, ki) + at(im, ki) + at(i, ki + 1) + at(i, ki - 1)) * 2.0
                + (at(ip, ki + 1) + at(ip, ki - 1) + at(im, ki + 1) + at(im, ki - 1));
            nrm[i * rows + k] = sum.norm();
        }
    }
}

/// Linear interpolation of a wrapped, uniformly sampled function of angle.
fn lerp_wrapped(v: &[f64], theta: f64) -> f64 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return v[0];
    }
    let mut th = theta;
    while th < 0.0 {
        th += TAU;
    }
    let f = (th % TAU) * (n as f64 / TAU);
    let i0 = (f.floor() as usize) % n;
    let i1 = (i0 + 1) % n;
    let w = f - f.floor();
    v[i0] + (v[i1] - v[i0]) * w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::cos;
    use crate::rng::{stream, Pcg32};

    fn sphere(r: f64) -> (Solid, Perlin) {
        let mut rng = Pcg32::new(7, stream::BLOB);
        let perlin = Perlin::new(&mut rng);
        let fbm = SphereFbm::new(&mut rng, 4, 2.03, 0.45);
        let solid = Solid {
            a: r,
            b: r,
            c: r,
            amp: 0.0,
            lobes: 2.6,
            floor: 0.25,
            warp: 0.0,
            fbm,
        };
        (solid, perlin)
    }

    fn lump(seed: u64, amp: f64) -> (Solid, Perlin) {
        let mut rng = Pcg32::new(seed, stream::BLOB);
        let perlin = Perlin::new(&mut rng);
        let fbm = SphereFbm::new(&mut rng, 4, 2.03, 0.45);
        let solid = Solid {
            a: 367.4,
            b: 244.9,
            c: 220.0,
            amp,
            lobes: 2.6,
            floor: 0.25,
            warp: 0.22,
            fbm,
        };
        (solid, perlin)
    }

    /// With no displacement the solid IS a sphere, so its silhouette is the
    /// sphere's radius at every angle and the maximum sits at u = 0. If the
    /// projection factor were ever applied the wrong way round this is the
    /// test that catches it.
    #[test]
    fn a_sphere_projects_to_its_own_radius() {
        let (s, p) = sphere(300.0);
        for i in 0..32 {
            let th = TAU * i as f64 / 32.0;
            let (rho, u) = s.silhouette(&p, th, V3::new(0.0, 0.0, 0.0));
            assert!((rho - 300.0).abs() < 0.02, "rho {rho} at theta {th}");
            assert!(u.abs() < 0.02, "the maximum is not on the equator: {u}");
        }
    }

    /// The projected outline of an ellipsoid is the ellipse of its two
    /// in-plane axes, whatever its depth. That is what lets `aspect` keep
    /// meaning the image's aspect after the move to 3D.
    #[test]
    fn an_ellipsoid_projects_to_its_in_plane_ellipse() {
        let mut rng = Pcg32::new(3, stream::BLOB);
        let perlin = Perlin::new(&mut rng);
        let fbm = SphereFbm::new(&mut rng, 4, 2.03, 0.45);
        let (a, b) = (367.4, 244.9);
        for &c in &[90.0, 244.9, 500.0] {
            let s = Solid {
                a,
                b,
                c,
                amp: 0.0,
                lobes: 2.6,
                floor: 0.25,
                warp: 0.0,
                fbm: fbm.clone(),
            };
            for i in 0..24 {
                let th = TAU * i as f64 / 24.0;
                let (st, ct) = sin_cos(th);
                let want = a * b / ((b * ct) * (b * ct) + (a * st) * (a * st)).sqrt();
                let (got, _) = s.silhouette(&perlin, th, V3::new(0.0, 0.0, 0.0));
                assert!(
                    (got - want).abs() < 0.05 * want.max(1.0) / 100.0 + 0.1,
                    "c {c} theta {th}: got {got} want {want}"
                );
            }
        }
    }

    /// The theorem the crate rests on, exercised on the displaced solid: the
    /// projected radius is positive and single-valued at every angle, which
    /// is what star-shapedness means here.
    #[test]
    fn the_silhouette_is_positive_and_seamless() {
        for seed in 0..6u64 {
            let (s, p) = lump(seed, 0.30);
            let at = |th: f64| s.silhouette(&p, th, V3::new(0.0, 0.0, 0.0)).0;
            for i in 0..180 {
                let th = TAU * i as f64 / 180.0;
                assert!(at(th) > 0.0, "seed {seed}: non-positive radius");
            }
            // theta and theta + TAU are the same direction, exactly.
            let (a, b) = (at(0.0), at(TAU));
            assert!((a - b).abs() < 1e-9, "seed {seed}: seam {a} vs {b}");
        }
    }

    /// A displaced solid's silhouette must actually differ from its base
    /// ellipse, and by about the amplitude asked for — otherwise the 3D stage
    /// is an expensive no-op.
    #[test]
    fn displacement_reaches_the_silhouette() {
        let (s, p) = lump(11, 0.30);
        let mut rel = Vec::new();
        for i in 0..360 {
            let th = TAU * i as f64 / 360.0;
            let (got, _) = s.silhouette(&p, th, V3::new(0.0, 0.0, 0.0));
            let (st, ct) = sin_cos(th);
            let base = s.a * s.b / ((s.b * ct) * (s.b * ct) + (s.a * st) * (s.a * st)).sqrt();
            rel.push(got / base - 1.0);
        }
        let mean: f64 = rel.iter().sum::<f64>() / rel.len() as f64;
        let var: f64 = rel.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / rel.len() as f64;
        assert!(
            var.sqrt() > 0.04,
            "the silhouette barely moved: std {}",
            var.sqrt()
        );
        // The maximum picks the biggest bump along each great circle, so it
        // can only sit at or above the mean radius of the displacement.
        assert!(mean > -0.02, "projection lost radius: mean {mean}");
    }

    /// The front surface of a sphere, sampled by the depth map, has the
    /// analytic depth `sqrt(R^2 - rho^2)` and the normal of its own radius.
    #[test]
    fn the_depth_map_of_a_sphere_is_a_hemisphere() {
        let (s, p) = sphere(300.0);
        let n_theta = 180;
        let mut rho = Vec::new();
        let mut us = Vec::new();
        for i in 0..n_theta {
            let th = TAU * i as f64 / n_theta as f64;
            let (r, u) = s.silhouette(&p, th, V3::new(0.0, 0.0, 0.0));
            rho.push(r);
            us.push(u);
        }
        let dm = DepthMap::build(&s, &p, V2::new(0.0, 0.0), n_theta, &rho, &us, 1.0);
        for i in 0..17 {
            for k in 1..9 {
                let th = TAU * i as f64 / 17.0;
                let q = V2::new(cos(th), crate::math::sin(th)) * (300.0 * k as f64 / 10.0);
                let (pt, n) = dm.sample(q);
                let want_z = (300.0 * 300.0 - q.len2()).max(0.0).sqrt();
                assert!(
                    (pt.z - want_z).abs() < 4.0,
                    "depth {} want {want_z} at r {}",
                    pt.z,
                    q.len()
                );
                let want_n = V3::new(q.x, q.y, want_z).norm();
                assert!(n.z > 0.0, "normal points away from the viewer");
                assert!(
                    n.dot(want_n) > 0.995,
                    "normal {n:?} want {want_n:?} at r {}",
                    q.len()
                );
            }
        }
    }

    /// At the rim the surface is tangent to the view, so the normal must lie
    /// in the image plane — the cue that makes a flat-shaded lump read as
    /// round rather than as a sticker.
    #[test]
    fn normals_turn_away_at_the_rim() {
        let (s, p) = sphere(300.0);
        let n_theta = 180;
        let (mut rho, mut us) = (Vec::new(), Vec::new());
        for i in 0..n_theta {
            let th = TAU * i as f64 / n_theta as f64;
            let (r, u) = s.silhouette(&p, th, V3::new(0.0, 0.0, 0.0));
            rho.push(r);
            us.push(u);
        }
        let dm = DepthMap::build(&s, &p, V2::new(0.0, 0.0), n_theta, &rho, &us, 1.0);
        for i in 0..12 {
            let th = TAU * i as f64 / 12.0;
            let d = V2::new(cos(th), crate::math::sin(th));
            let (_, n_rim) = dm.sample(d * (300.0 * 0.995));
            let (_, n_mid) = dm.sample(d * (300.0 * 0.2));
            assert!(
                n_rim.z < 0.22,
                "rim normal still faces the viewer: {n_rim:?}"
            );
            assert!(n_mid.z > 0.9, "centre normal should face the viewer");
        }
    }

    /// The iso-brightness direction is perpendicular to the gradient, which
    /// is what makes a cut along it leave the shading unbroken.
    #[test]
    fn iso_dir_is_perpendicular_to_the_gradient() {
        let (s, p) = lump(5, 0.26);
        let n_theta = 180;
        let (mut rho, mut us) = (Vec::new(), Vec::new());
        for i in 0..n_theta {
            let th = TAU * i as f64 / n_theta as f64;
            let (r, u) = s.silhouette(&p, th, V3::new(0.0, 0.0, 0.0));
            rho.push(r);
            us.push(u);
        }
        let dm = DepthMap::build(&s, &p, V2::new(0.0, 0.0), n_theta, &rho, &us, 1.0);
        let light = Light::new(1.90, 0.45);
        let h = 6.0;
        let mut tested = 0;
        for i in 0..24 {
            for k in 1..6 {
                let th = TAU * i as f64 / 24.0;
                let q = V2::new(cos(th), crate::math::sin(th)) * (120.0 * k as f64 / 5.0);
                if let Some(d) = dm.iso_dir(q, &light, h) {
                    let g = dm.brightness_grad(q, &light, h);
                    assert!((d.len() - 1.0).abs() < 1e-12);
                    assert!(
                        d.dot(g).abs() < 1e-9 * g.len().max(1.0),
                        "not perpendicular: {}",
                        d.dot(g)
                    );
                    tested += 1;
                }
            }
        }
        assert!(tested > 50, "the gradient was flat almost everywhere");
    }
}
