//! Diagrams held to what Obsidian 1.15.1 (Pele 1.13.0) drew for the same
//! fences, read out of its DOM by `tools/obsidian-probe/mermaid.mjs` and
//! reduced by `mermaid-fixture.py`: the size of each diagram, where each
//! text sits, the boxes of the shapes, the colours in both base themes, and
//! the error each broken fence shows.

use std::collections::HashMap;

use scoria_mermaid::{render, set_fonts, Anchor, Diagram, Fonts, Rgba, Segment, Shape, Theme};
use serde::Deserialize;

const REGULAR: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-SemiBold.ttf");
const ITALIC: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-Italic.ttf");
const BOLD_ITALIC: &[u8] = include_bytes!("../../scoria/assets/fonts/Inter-BoldItalic.ttf");

#[derive(Deserialize)]
struct Fixture {
    cases: HashMap<String, String>,
    themes: HashMap<String, HashMap<String, Option<Observed>>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Observed {
    Error { error: String },
    Drawn { size: [f64; 2], items: Vec<Item> },
}

#[derive(Deserialize, Debug)]
struct Item {
    tag: String,
    g: Option<String>,
    #[serde(rename = "box")]
    bbox: [f64; 4],
    fill: Option<String>,
    stroke: Option<String>,
    text: Option<String>,
}

fn fixture() -> Fixture {
    set_fonts(Fonts {
        regular: REGULAR,
        bold: Some(BOLD),
        italic: Some(ITALIC),
        bold_italic: Some(BOLD_ITALIC),
        mono: None,
    });
    serde_json::from_str(include_str!("data/obsidian-1.15.1.json")).unwrap()
}

fn theme(name: &str) -> Theme {
    match name {
        "moonstone" => Theme::light(),
        _ => Theme::dark(),
    }
}

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// The box a path covers, from its points (control points included, which
/// is what a browser's getBoundingClientRect gives for straight edges and
/// close to it for curves).
fn bounds(segs: &[Segment]) -> [f64; 4] {
    let mut xs = vec![];
    let mut ys = vec![];
    for s in segs {
        match *s {
            Segment::Move(x, y) | Segment::Line(x, y) => {
                xs.push(x);
                ys.push(y);
            }
            Segment::Quad(_, _, x, y) => {
                xs.push(x);
                ys.push(y);
            }
            Segment::Cubic(_, _, _, _, x, y) => {
                xs.push(x);
                ys.push(y);
            }
            Segment::Close => {}
        }
    }
    let f = |v: &[f64], max: bool| {
        v.iter()
            .copied()
            .fold(if max { f64::MIN } else { f64::MAX }, |a, b| {
                if max {
                    a.max(b)
                } else {
                    a.min(b)
                }
            })
    };
    let (x0, y0, x1, y1) = (f(&xs, false), f(&ys, false), f(&xs, true), f(&ys, true));
    [x0, y0, x1 - x0, y1 - y0]
}

/// The diagrams and the error cases that must match: every case the probe
/// recorded.
const LAYOUT_TOLERANCE: f64 = 2.0;

/// How far a case may be off, in pixels: flowcharts and sequence diagrams
/// 6; the others, whose boxes hold more words in capitals (ER) or members
/// (class) and so gather more of the per-word difference, 12.
fn tolerance(name: &str) -> f64 {
    if name.starts_with("flow") || name.starts_with("sequence") {
        6.0
    } else {
        12.0
    }
}

struct Report {
    lines: Vec<String>,
    worst_size: f64,
    worst_text: f64,
    worst_shape: f64,
}

fn compare(name: &str, d: &Diagram, size: [f64; 2], items: &[Item], r: &mut Report) {
    let dw = (d.width - size[0]).abs().max((d.height - size[1]).abs());
    // The one known gap: a label holding 日本語 is a line taller in
    // Obsidian, whose fallback face for it has a taller line box.
    let cjk = name == "flow-long-label";
    if !cjk {
        r.worst_size = r.worst_size.max(dw / tolerance(name));
    }
    // Text: Obsidian's text box centre x against each run's middle (an
    // anchored run's middle is where the painter centres it).
    let mut text_dev = 0f64;
    let mut runs: Vec<(String, f64, f64)> = d
        .shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Text(t) => {
                let text: String = t.spans.iter().map(|s| s.text.as_str()).collect();
                let w: f64 = t.spans.iter().map(|s| s.measured).sum();
                let mid = match t.anchor {
                    Anchor::Start => t.x + w / 2.0,
                    Anchor::Middle => t.x,
                    Anchor::End => t.x - w / 2.0,
                };
                Some((text, mid, t.y))
            }
            _ => None,
        })
        .collect();
    for it in items.iter().filter(|i| i.tag == "text") {
        let Some(text) = &it.text else { continue };
        let cx = it.bbox[0] + it.bbox[2] / 2.0;
        let cy = it.bbox[1] + it.bbox[3] / 2.0;
        // Obsidian's <text> holds a label's lines as tspans; match on the
        // first run whose text starts the element's.
        let best = runs
            .iter()
            .enumerate()
            .filter(|(_, (t, _, _))| !t.is_empty() && text.starts_with(t.as_str()))
            .min_by(|a, b| {
                let da = (a.1 .1 - cx).abs() + (a.1 .2 - cy).abs();
                let db = (b.1 .1 - cx).abs() + (b.1 .2 - cy).abs();
                da.total_cmp(&db)
            })
            .map(|(i, _)| i);
        match best {
            Some(i) => {
                let (_, x, _) = runs.remove(i);
                // A multi-line label's box is centred on its widest line,
                // which may not be the first; count x only for one line.
                if !text.contains('\n') {
                    text_dev = text_dev.max((x - cx).abs());
                }
            }
            None => r.lines.push(format!("  {name}: no run for text {text:?}")),
        }
    }
    r.worst_text = r.worst_text.max(text_dev);
    // Shapes: each filled node shape of Obsidian's against the nearest of
    // ours.
    let ours: Vec<[f64; 4]> = d
        .shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Path { segments, .. } => Some(bounds(segments)),
            _ => None,
        })
        .collect();
    let mut shape_dev = 0f64;
    for it in items
        .iter()
        .filter(|i| matches!(i.tag.as_str(), "rect" | "polygon" | "circle" | "ellipse"))
        .filter(|i| i.bbox[2] > 0.5 && i.bbox[3] > 0.5)
        .filter(|i| i.fill.is_some() || i.stroke.is_some())
    {
        let b = it.bbox;
        let dev = ours
            .iter()
            .map(|o| {
                (o[0] - b[0])
                    .abs()
                    .max((o[1] - b[1]).abs())
                    .max((o[2] - b[2]).abs())
                    .max((o[3] - b[3]).abs())
            })
            .fold(f64::MAX, f64::min);
        if dev > LAYOUT_TOLERANCE {
            r.lines.push(format!(
                "  {name}: {} {:?} {:?} off by {dev:.2}",
                it.tag, it.g, b
            ));
        }
        shape_dev = shape_dev.max(dev);
    }
    if !cjk {
        r.worst_shape = r.worst_shape.max(shape_dev / tolerance(name));
    }
    r.lines.push(format!(
        "{name}: size {:.2}x{:.2} vs {:.2}x{:.2} (off {dw:.2}), text x off {text_dev:.2}, shapes off {shape_dev:.2}",
        d.width, d.height, size[0], size[1]
    ));
}

/// Prints how close each case comes; run with `--nocapture` to read it.
#[test]
fn layout_matches_obsidian() {
    let fx = fixture();
    let mut names: Vec<_> = fx.cases.keys().cloned().collect();
    names.sort();
    let mut r = Report {
        lines: vec![],
        worst_size: 0.0,
        worst_text: 0.0,
        worst_shape: 0.0,
    };
    let mut failures = vec![];
    for name in &names {
        let src = &fx.cases[name];
        let Some(Some(obs)) = fx.themes["moonstone"].get(name) else {
            continue;
        };
        let got = render(src, 700.0, &Theme::light());
        match (obs, got) {
            (Observed::Error { error }, Err(e)) => {
                if &e.message != error {
                    failures.push(format!("{name}: error {:?} vs {:?}", e.message, error));
                }
            }
            (Observed::Error { error }, Ok(_)) => {
                failures.push(format!("{name}: drew, Obsidian showed {error:?}"))
            }
            (Observed::Drawn { .. }, Err(e)) => {
                failures.push(format!("{name}: error {:?}", e.message))
            }
            (Observed::Drawn { size, items }, Ok(d)) => compare(name, &d, *size, items, &mut r),
        }
    }
    println!("{}", r.lines.join("\n"));
    println!(
        "worst (in tolerances): size {:.2}, shape {:.2}; text {:.2} px",
        r.worst_size, r.worst_shape, r.worst_text
    );
    assert!(failures.is_empty(), "{failures:#?}");
    // Text is measured in Inter 4.0's static faces where Obsidian measures
    // in its InterVariable at optical size 16: a word comes out up to a
    // pixel apart, and a box with several words a few. Held to that.
    assert!(
        r.worst_size <= 1.0,
        "a diagram's size is off by {} tolerances",
        r.worst_size
    );
    assert!(
        r.worst_shape <= 1.0,
        "a shape is off by {} tolerances",
        r.worst_shape
    );
    assert!(r.worst_text <= 7.0, "a text is off by {}", r.worst_text);
}

/// Both themes paint the colours Obsidian's variables give: node fill and
/// border, edge line, text.
#[test]
fn colours_match_obsidian() {
    let fx = fixture();
    for (tname, cases) in &fx.themes {
        let Some(Some(Observed::Drawn { items, .. })) = cases.get("flow-td") else {
            panic!("no flow-td in {tname}");
        };
        let d = render(&fx.cases["flow-td"], 700.0, &theme(tname)).unwrap();
        let node = items
            .iter()
            .find(|i| i.tag == "rect" && i.g.as_deref() == Some("pele-node pele-shape-rect"))
            .unwrap();
        let edge = items
            .iter()
            .find(|i| i.g.as_deref() == Some("pele-edge") && i.stroke.is_some())
            .unwrap();
        let text = items.iter().find(|i| i.tag == "text").unwrap();
        let mut fills = vec![];
        let mut strokes = vec![];
        let mut texts = vec![];
        for s in &d.shapes {
            match s {
                Shape::Path { fill, stroke, .. } => {
                    fills.extend(fill.map(hex));
                    strokes.extend(stroke.as_ref().map(|s| hex(s.color)));
                }
                Shape::Text(t) => texts.push(hex(t.color)),
            }
        }
        assert!(
            fills.contains(node.fill.as_ref().unwrap()),
            "{tname} node fill"
        );
        assert!(
            strokes.contains(node.stroke.as_ref().unwrap()),
            "{tname} node border"
        );
        assert!(
            strokes.contains(edge.stroke.as_ref().unwrap()),
            "{tname} edge"
        );
        assert!(texts.contains(text.fill.as_ref().unwrap()), "{tname} text");
    }
}

/// The width the note gives a diagram is a layout input, as Pele's
/// `maxWidth` is: observed with the window narrowed so the column was 280
/// and then 80 pixels wide (s05). A flowchart running across turns to run
/// down under 640, a flowchart narrows its ranks, and a sequence diagram
/// wraps its messages.
#[test]
fn a_narrow_column_lays_out_as_obsidian_did() {
    let fx = fixture();
    let t = Theme::light();
    for (name, width, want) in [
        ("flow-lr-shapes", 280.0, [156.0, 1420.0]),
        ("flow-lr-shapes", 80.0, [156.0, 1420.0]),
        ("flow-td", 280.0, [184.0, 489.0]),
        ("flow-td", 80.0, [169.0, 441.0]),
        ("sequence", 280.0, [310.0, 528.0]),
        ("sequence", 80.0, [306.0, 528.0]),
    ] {
        let d = render(&fx.cases[name], width, &t).unwrap();
        let off = (d.width - want[0]).abs().max((d.height - want[1]).abs());
        assert!(
            off <= 6.0,
            "{name} at {width}: {}x{} vs {want:?}",
            d.width,
            d.height
        );
    }
}
