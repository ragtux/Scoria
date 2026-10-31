//! The SVG writer.
//!
//! Number formatting is part of the spec, not a detail: `coord_decimals`
//! places, trailing zeros trimmed, and anything under half a quantum snapped
//! to exactly zero so no `-0.000` ever reaches the file. A `-0.000` breaks
//! byte-identity between targets that differ in how they propagate the sign of
//! zero, and it looks like a bug to anyone reading the file.
//!
//! What the document contains, in paint order: an optional background rect,
//! the facets (one path each), the vesicles, then the outline ribbon as ONE
//! path with two subpaths and `fill-rule="evenodd"`. No gradients, no strokes,
//! no filters, no opacity - the artwork uses none and neither do we.

use std::fmt::Write;

use crate::bezier::Cubic;
use crate::body::Scene;
use crate::math::V2;
use crate::Params;

/// Smallest magnitude that survives formatting, for 2, 3 and 4 decimals.
/// Written out rather than computed, because `powi` lowers to an LLVM
/// intrinsic whose result is not guaranteed identical across targets.
fn quantum(decimals: u32) -> f64 {
    match decimals {
        2 => 5e-3,
        4 => 5e-5,
        _ => 5e-4,
    }
}

/// Append one formatted coordinate to `out`.
///
/// Writes in place rather than returning a `String`: a logo carries a few
/// thousand coordinates, and the allocation per number was a third of the
/// time spent writing the document.
pub fn push_num(out: &mut String, x: f64, decimals: u32) {
    let snap = quantum(decimals).max(5e-4);
    // A non-finite coordinate is a bug the gates catch; formatting it as zero
    // here only keeps the file parseable if one ever slips through.
    let v = if !x.is_finite() || x.abs() < snap {
        0.0
    } else {
        x
    };
    let start = out.len();
    let _ = write!(out, "{:.*}", decimals as usize, v);
    if out[start..].contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
    // Rounding can still hand back a signed zero at two decimals.
    if &out[start..] == "-0" {
        out.truncate(start);
        out.push('0');
    }
}

/// Format one coordinate: fixed decimals, trailing zeros trimmed, and no
/// negative zero. The snap threshold is half a quantum, never below the
/// spec's 5e-4.
pub fn fmt(x: f64, decimals: u32) -> String {
    let mut s = String::with_capacity(16);
    push_num(&mut s, x, decimals);
    s
}

/// Append one point as `x,y`.
fn push_pt(out: &mut String, p: V2, d: u32) {
    push_num(out, p.x, d);
    out.push(',');
    push_num(out, p.y, d);
}

/// A closed subpath: `M` the first point, one `C`, then every triple.
///
/// The command letter is not repeated - SVG takes an implicit `C` for each
/// following triple, which is the single biggest saving in the file.
pub fn subpath(cubics: &[Cubic], d: u32) -> String {
    if cubics.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(cubics.len() * 48);
    out.push('M');
    push_pt(&mut out, cubics[0].p0, d);
    out.push('C');
    for (i, c) in cubics.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        push_pt(&mut out, c.c1, d);
        out.push(' ');
        push_pt(&mut out, c.c2, d);
        out.push(' ');
        push_pt(&mut out, c.p3, d);
    }
    out.push('Z');
    out
}

/// The viewBox: the silhouette's true bbox plus `margin * R0`.
///
/// Measured off the artwork as a sanity check: the rock touches all four
/// canvas edges with under 0.35 units of slack, so the box is fitted to the
/// shape afterwards rather than the shape being drawn into a fixed box.
pub fn view_box(scene: &Scene, p: &Params) -> (f64, f64, f64, f64) {
    let pts = crate::bezier::flatten(&scene.outer_cubics, 6);
    let mut lo = pts[0];
    let mut hi = pts[0];
    for q in &pts {
        lo.x = lo.x.min(q.x);
        lo.y = lo.y.min(q.y);
        hi.x = hi.x.max(q.x);
        hi.y = hi.y.max(q.y);
    }
    let m = p.margin * scene.r0;
    (
        lo.x - m,
        lo.y - m,
        (hi.x - lo.x) + 2.0 * m,
        (hi.y - lo.y) + 2.0 * m,
    )
}

/// The header comment: the seed and every non-default parameter, so any
/// generated file can be regenerated from itself.
///
/// XML forbids a double hyphen inside a comment, so this never writes `--`
/// and the regeneration hint names the flags instead of spelling them.
fn header(scene: &Scene, p: &Params) -> String {
    let mut s = String::from("anya-scoria-gen | seed=");
    s.push_str(&scene.seed.to_string());
    for (k, v) in p.deltas() {
        if k == "seed" {
            continue;
        }
        s.push(' ');
        s.push_str(k);
        s.push('=');
        s.push_str(&v);
    }
    s.push_str(" | regenerate: scoriagen with seed and one set flag per pair above");
    // Belt and braces: a value can never contain a double hyphen today, but a
    // comment that does would make the file unparseable.
    s.replace("--", "- -")
}

/// Render the whole document.
pub fn render(scene: &Scene, p: &Params) -> String {
    let d = p.coord_decimals;
    let (vx, vy, vw, vh) = view_box(scene, p);
    let mut out = String::with_capacity(32 * 1024);

    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" ");
    out.push_str(&format!(
        "width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">\n",
        fmt(vw, d),
        fmt(vh, d),
        fmt(vx, d),
        fmt(vy, d),
        fmt(vw, d),
        fmt(vh, d)
    ));
    out.push_str("<!-- ");
    out.push_str(&header(scene, p));
    out.push_str(" -->\n");

    if let Some(bg) = p.background {
        out.push_str(&format!(
            "<rect id=\"background\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            fmt(vx, d),
            fmt(vy, d),
            fmt(vw, d),
            fmt(vh, d),
            bg.hex()
        ));
    }

    // 1. FACETS, on a ring dilated by w0/2 so their outermost seam falls
    //    under the ribbon, over an underlay in the mid tone so no seam
    //    between two of them can show through either.
    out.push_str("<g id=\"facets\">\n");
    out.push_str(&format!(
        "<path id=\"body-fill\" fill=\"{}\" d=\"{}\"/>\n",
        scene.ramp.mid.hex(),
        subpath(&scene.root_cubics, d)
    ));
    for (i, f) in scene.facets.iter().enumerate() {
        out.push_str(&format!(
            "<path id=\"facet-{:02}\" fill=\"{}\" d=\"{}\"/>\n",
            i,
            scene.ramp.tone(f.tone).hex(),
            subpath(&f.cubics, d)
        ));
    }
    out.push_str("</g>\n");

    // 2. VESICLES. Opaque, so they punch through the facets.
    out.push_str("<g id=\"vesicles\">\n");
    for (i, v) in scene.vesicles.iter().enumerate() {
        out.push_str(&format!(
            "<path id=\"vesicle-{:02}\" fill=\"{}\" d=\"{}\"/>\n",
            i,
            p.ink.hex(),
            subpath(&v.cubics, d)
        ));
    }
    out.push_str("</g>\n");

    // 3. THE RELIEF MARKS. Same ink as the ribbon and drawn just before it,
    //    so where a mark's rim end runs into the ink band the two merge with
    //    no join to render - which is how the artwork's own mark reads, its
    //    nearest point measuring 0.05 units from the outline.
    if !scene.relief.is_empty() {
        out.push_str("<g id=\"relief\">\n");
        for (i, m) in scene.relief.iter().enumerate() {
            out.push_str(&format!(
                "<path id=\"relief-{:02}\" fill=\"{}\" d=\"{}\"/>\n",
                i,
                p.ink.hex(),
                subpath(&m.cubics, d)
            ));
        }
        out.push_str("</g>\n");
    }

    // 4. THE RIBBON, last. Two subpaths, the inner one wound the other way AND
    //    marked even-odd: the path then renders identically under either fill
    //    rule, which protects it from tools that drop the attribute.
    out.push_str(&format!(
        "<path id=\"outline\" fill=\"{}\" fill-rule=\"evenodd\" d=\"{}{}\"/>\n",
        p.ink.hex(),
        subpath(&scene.outer_cubics, d),
        subpath(&scene.inner_cubics, d)
    ));
    if let Some(rim) = &scene.rim {
        out.push_str(&format!(
            "<path id=\"rim\" fill=\"{}\" fill-rule=\"evenodd\" d=\"{}{}\"/>\n",
            p.ink.hex(),
            subpath(&rim.outer_cubics, d),
            subpath(&rim.inner_cubics, d)
        ));
    }

    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_zero_never_reaches_the_file() {
        assert_eq!(fmt(-0.0, 3), "0");
        assert_eq!(fmt(-0.0001, 3), "0");
        assert_eq!(fmt(-0.00049, 3), "0");
        assert_eq!(fmt(0.0, 3), "0");
        assert!(!fmt(-0.0004, 3).starts_with('-'));
    }

    #[test]
    fn trailing_zeros_are_trimmed() {
        assert_eq!(fmt(2.0, 3), "2");
        assert_eq!(fmt(1.5, 3), "1.5");
        assert_eq!(fmt(1.2500, 3), "1.25");
        assert_eq!(fmt(-12.3456, 3), "-12.346");
        assert_eq!(fmt(-12.3456, 2), "-12.35");
    }

    #[test]
    fn the_snap_scales_with_the_decimal_count() {
        // At two decimals, 0.004 would format as "0.00"; it must snap first so
        // the sign cannot survive.
        assert_eq!(fmt(-0.004, 2), "0");
        assert_eq!(fmt(-0.004, 4), "-0.004");
    }

    #[test]
    fn a_subpath_is_closed_and_uses_one_command_letter() {
        let c = Cubic {
            p0: V2::new(0.0, 0.0),
            c1: V2::new(1.0, 0.0),
            c2: V2::new(2.0, 1.0),
            p3: V2::new(2.0, 2.0),
        };
        let c2 = Cubic {
            p0: c.p3,
            c1: V2::new(2.0, 3.0),
            c2: V2::new(0.0, 1.0),
            p3: c.p0,
        };
        let d = subpath(&[c, c2], 3);
        assert!(d.starts_with("M0,0C"), "{d}");
        assert!(d.ends_with('Z'), "{d}");
        assert_eq!(d.matches('C').count(), 1, "implicit repetition: {d}");
        assert_eq!(d.matches('M').count(), 1);
    }

    #[test]
    fn the_header_can_never_break_the_xml_comment() {
        let s = "a--b".replace("--", "- -");
        assert!(!s.contains("--"));
    }
}
