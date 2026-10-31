//! Structural invariants of the emitted document, over a run of 200 seeds.
//!
//! The gates in `quality.rs` check the GEOMETRY before it is written. These
//! check the TEXT after it is written, which is a different question: a
//! defect in the writer - a dropped `Z`, a stray `stroke`, a tone that never
//! reached the file - passes every geometric gate there is.
//!
//! Everything here is asserted on the default parameters, which are the ones
//! the CLI and the README promise work.

use anya_scoria_gen::{generate, palette::Ramp, Params};

/// Seeds swept by every test in this file.
///
/// 200 at roughly 2 ms a logo is well under a second in release and a few
/// seconds in a debug test build - cheap enough to leave on, wide enough that
/// a one-in-fifty defect cannot hide.
const SEEDS: u64 = 200;

/// Every default-parameter seed produces a document.
///
/// The strongest promise the crate makes. `generate` re-rolls internally and
/// only returns `Err` when every roll in `retry_budget` failed the same gate,
/// which means the parameter set is infeasible - and the default set must
/// never be.
#[test]
fn every_default_seed_succeeds() {
    let mut failures = Vec::new();
    for seed in 0..SEEDS {
        if let Err(e) = generate(&Params {
            seed,
            ..Params::default()
        }) {
            failures.push(format!("seed {seed}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {SEEDS} default seeds failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// All three tone colours appear in every document.
///
/// Tones are assigned by area quantile precisely so none can come out empty,
/// but the quantile runs on regions and the writer runs on paths; this is the
/// assertion that the two agree. A two-tone scoria is a different, worse
/// logo.
#[test]
fn all_three_tones_reach_the_file() {
    let p = Params::default();
    let ramp = Ramp::build(
        p.palette_hue,
        p.palette_sat,
        p.palette_light,
        p.tone_contrast,
    );
    let want = [ramp.shadow.hex(), ramp.mid.hex(), ramp.highlight.hex()];
    for seed in 0..SEEDS {
        let doc = doc(seed);
        for hex in &want {
            assert!(
                doc.contains(hex.as_str()),
                "seed {seed}: tone {hex} never reached the file"
            );
        }
    }
}

/// Exactly one outline path, and it carries exactly two subpaths.
///
/// The ribbon is a filled annulus, not a stroke: one compound path whose
/// outer subpath is the silhouette and whose inner subpath is the same curve
/// offset inward and wound the other way. Two `M` commands, no more and no
/// fewer. One would mean the inner loop was lost and the logo fills solid
/// black; three would mean a stray contour got into the ribbon.
#[test]
fn the_outline_is_one_path_with_two_subpaths() {
    for seed in 0..SEEDS {
        let doc = doc(seed);
        assert_eq!(
            doc.matches("id=\"outline\"").count(),
            1,
            "seed {seed}: expected exactly one outline path"
        );
        let d = outline_d(&doc).unwrap_or_else(|| panic!("seed {seed}: no outline d attribute"));
        assert_eq!(
            d.matches('M').count(),
            2,
            "seed {seed}: the outline must have exactly two subpaths"
        );
        assert!(
            doc.contains("fill-rule=\"evenodd\""),
            "seed {seed}: the outline needs fill-rule=evenodd, or it fills solid"
        );
    }
}

/// Every subpath is explicitly closed.
///
/// A fill renders an open subpath as though it were closed, so an unclosed
/// path looks right and then breaks the moment anyone strokes it, offsets it
/// or opens it in Inkscape. The count of `Z` must equal the count of `M`.
#[test]
fn every_subpath_is_closed() {
    for seed in 0..SEEDS {
        let doc = doc(seed);
        for (i, d) in path_data(&doc).iter().enumerate() {
            let opens = d.matches('M').count();
            let closes = d.matches('Z').count();
            assert_eq!(
                opens, closes,
                "seed {seed}, path {i}: {opens} subpaths but {closes} closes"
            );
            assert!(opens > 0, "seed {seed}, path {i}: empty path data");
        }
    }
}

/// Nothing in the document is stroked.
///
/// All 24 elements of the artwork are solid-fill paths; the heavy outline is
/// geometry, not a stroke. A stroke would also break the promise that the
/// output drops into the artist's working file and scales non-uniformly
/// without distorting.
#[test]
fn nothing_is_stroked_and_nothing_is_transparent() {
    for seed in 0..SEEDS {
        let doc = doc(seed);
        for forbidden in ["stroke", "opacity", "<linearGradient", "filter=", "<image"] {
            assert!(
                !doc.contains(forbidden),
                "seed {seed}: the document contains `{forbidden}`"
            );
        }
    }
}

/// No non-finite, no negative zero and no scientific notation in the
/// coordinates.
///
/// `-0.000` breaks byte-identity between targets that differ in sign-of-zero
/// propagation and reads as a bug to anyone opening the file; `NaN` or `inf`
/// in a `d` attribute makes the whole path vanish in some renderers and
/// renders garbage in others; `1e-7` is legal SVG but several tools mis-parse
/// it, and its appearance would mean the writer stopped using fixed decimals.
///
/// Scanned over the PATH DATA, not the whole document - `id="vesicle-00"`
/// contains the substring `e-`, which is how the first version of this test
/// failed.
#[test]
fn no_nan_no_inf_no_negative_zero() {
    for seed in 0..SEEDS {
        let doc = doc(seed);
        for (i, d) in path_data(&doc).iter().enumerate() {
            for forbidden in ["NaN", "nan", "inf", "-0.000", "e-", "e+", "E-", "E+"] {
                assert!(
                    !d.contains(forbidden),
                    "seed {seed}, path {i}: the path data contains `{forbidden}`"
                );
            }
        }
    }
}

/// The document is a well-formed SVG with a fitted viewBox.
///
/// The mark bleeds to the edge of its box - measured under 0.35 units of
/// slack on any side of the artwork - so the viewBox is fitted to the
/// silhouette after the fact rather than insetting it.
#[test]
fn the_document_is_well_formed() {
    for seed in 0..SEEDS {
        let doc = doc(seed);
        assert!(
            doc.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
            "seed {seed}: missing or malformed root element"
        );
        assert!(
            doc.trim_end().ends_with("</svg>"),
            "seed {seed}: document is not closed"
        );
        assert!(
            doc.contains("viewBox=\""),
            "seed {seed}: no viewBox, so the mark will not scale"
        );
        // A hole, a facet and the outline at the very least.
        assert!(
            doc.matches("<path").count() >= 8,
            "seed {seed}: suspiciously few paths"
        );
    }
}

/// The holes vary in size, in the finished file.
///
/// Measured: equivalent diameters 80.8 down to 26.2, a 3.08x range, and the
/// spread is what stops the holes reading as polka dots rather than as gas.
/// The geometric gate asserts this on each vesicle's own ring; this asserts
/// the writer did not drop the big ones on the way to the file.
///
/// Size is measured as the diagonal of the hole's coordinate bounding box.
/// The obvious proxy - how many cubics the hole carries - does not work: the
/// emitter clamps a vesicle to at least 12 segments, so on seed 4 three of
/// the seven holes report an identical count and the apparent range collapses
/// below the real one.
///
/// This is deliberately a WEAKER assertion than the geometric
/// `quality::vesicle_spread` gate, which requires 2.0. Two reasons, both
/// measured over these 200 seeds. The bounding box is not faithful to area -
/// a kidney-shaped small hole has a large box - and the gate itself runs on
/// the packer's TARGET radius rather than on the finished outline, so the
/// emitted shapes' equivalent-radius ratio bottoms out at 1.49 even though
/// every logo passed a gate asking for 2.0. The observed floor on the box
/// diagonal is 1.55 (p05 1.99, median 2.93), so 1.5 is a real assertion with
/// a hair of room, and it still catches the defect the test is for: seven
/// identical dots, which reads as polka dot rather than as rock.
#[test]
fn the_holes_vary_in_size() {
    for seed in 0..SEEDS {
        let doc = doc(seed);
        let n = doc.matches("id=\"vesicle-").count();
        // Three, not four. How many holes a rock has is a Poisson draw now,
        // floored at three because "a scoria with two holes does not read
        // as scoria" - so three is a legitimate outcome rather than a
        // failure, and with the holes as large as they now are it reads.
        assert!(
            n >= 3,
            "seed {seed}: only {n} holes reached the file; a scoria with two holes is a pebble"
        );
        let mut spans: Vec<f64> = (0..n)
            .filter_map(|i| {
                let at = doc.find(&format!("id=\"vesicle-{i:02}\""))?;
                let rest = &doc[at..];
                let d = rest.find(" d=\"")? + 4;
                let end = rest[d..].find('"')? + d;
                Some(bbox_gmean(&rest[d..end]))
            })
            .collect();
        spans.sort_by(f64::total_cmp);
        let (small, big) = (spans[0], spans[spans.len() - 1]);
        assert!(
            small > 0.0,
            "seed {seed}: a hole has no extent at all ({spans:?})"
        );
        assert!(
            big >= 1.5 * small,
            "seed {seed}: the holes barely vary in size (mean extents {spans:?})"
        );
    }
}

/// One finished document, at the default parameters.
fn doc(seed: u64) -> String {
    generate(&Params {
        seed,
        ..Params::default()
    })
    .unwrap_or_else(|e| panic!("seed {seed} failed to generate: {e}"))
}

/// The `d` attribute of the outline path.
fn outline_d(doc: &str) -> Option<&str> {
    let at = doc.find("id=\"outline\"")?;
    let rest = &doc[at..];
    let d = rest.find(" d=\"")? + 4;
    let end = rest[d..].find('"')? + d;
    Some(&rest[d..end])
}

/// Diagonal of the bounding box of every coordinate in a `d` attribute.
///
/// A crude parse - every number in the string, taken in x, y order - which is
/// exactly right here, because a cubic's control points and its endpoints all
/// lie inside the hull the box is measuring.
fn bbox_gmean(d: &str) -> f64 {
    let mut nums = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_digit() || ch == '.' || (ch == '-' && cur.is_empty()) {
            cur.push(ch);
        } else {
            if let Ok(v) = cur.parse::<f64>() {
                nums.push(v);
            }
            cur.clear();
        }
    }
    if let Ok(v) = cur.parse::<f64>() {
        nums.push(v);
    }
    let (mut lo_x, mut hi_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut lo_y, mut hi_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for (i, v) in nums.iter().enumerate() {
        if i % 2 == 0 {
            lo_x = lo_x.min(*v);
            hi_x = hi_x.max(*v);
        } else {
            lo_y = lo_y.min(*v);
            hi_y = hi_y.max(*v);
        }
    }
    let (w, h) = (hi_x - lo_x, hi_y - lo_y);
    if w.is_finite() && h.is_finite() {
        // The GEOMETRIC MEAN of the bbox sides, not its diagonal. For an
        // ellipse of semi-axes a and b the bbox is 2a by 2b, so this is
        // 2*sqrt(a*b) - proportional to the equivalent radius and blind to
        // elongation. The diagonal is not, and holes are deliberately
        // elongated by the tilt of the surface under them now, so a small
        // hole lying on a steep patch measures as large a diagonal as a big
        // one lying flat. The test wants to know whether the holes vary in
        // SIZE.
        (w * h).sqrt()
    } else {
        0.0
    }
}

/// Every `d` attribute in the document.
fn path_data(doc: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = doc;
    while let Some(at) = rest.find(" d=\"") {
        rest = &rest[at + 4..];
        let Some(end) = rest.find('"') else { break };
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}
