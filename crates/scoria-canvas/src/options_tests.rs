//! The Canvas settings as Obsidian 1.15.0 acted on them (2026-11-16,
//! `tools/obsidian-probe/canvas-settings.mjs`).

use crate::options::*;
use crate::{Handle, Point, Rect, MIN_NODE};
use scoria_formats::config::{js_number, CanvasConfig};

/// The probe's two cards: A, 100 by 100, dragged about under B.
const B: Rect = Rect::new(400.0, 0.0, 200.0, 100.0);

fn a_at(x: f64, y: f64) -> Rect {
    Rect::new(x, y, 100.0, 100.0)
}

/// Where A lands when the pointer leaves it at `x` (moved sideways only),
/// snapping to objects alone.
fn land_x(x: f64, scale: f64) -> f64 {
    let options = Options {
        snap_to_grid: false,
        ..Options::default()
    };
    let c = drag_correction(
        &options,
        Point::new(x, 300.0),
        &[a_at(x, 300.0)],
        &[B],
        scale,
        (true, false),
    );
    x + c.x
}

#[test]
fn absent_keys_and_unknown_strings_are_obsidians_defaults() {
    let o = Options::default();
    assert!(o.snap_to_grid && o.snap_to_objects);
    assert_eq!(o.wheel, WheelBehavior::Pan);
    assert_eq!(o.mod_drag, ModDrag::Menu);
    assert_eq!(o.card_labels, CardLabels::Always);
    assert_eq!(o.zoom_breakpoint, 0.0);
    let o = Options::from_config(&CanvasConfig {
        snap_to_grid: Some(false),
        default_wheel_behavior: Some("zoom".into()),
        default_mod_drag_behavior: Some("group".into()),
        card_label_visibility: Some("bogus".into()),
        zoom_breakpoint: js_number(0.5),
        ..Default::default()
    });
    assert!(!o.snap_to_grid && o.snap_to_objects);
    assert_eq!(o.wheel, WheelBehavior::Zoom);
    assert_eq!(o.mod_drag, ModDrag::Group);
    assert_eq!(o.card_labels, CardLabels::Always);
    assert_eq!(o.zoom_breakpoint, 0.5);
}

#[test]
fn ctrl_swaps_what_the_wheel_does() {
    let pan = Options::default();
    assert!(!pan.wheel_zooms(false));
    assert!(pan.wheel_zooms(true));
    let zoom = Options {
        wheel: WheelBehavior::Zoom,
        ..pan
    };
    assert!(zoom.wheel_zooms(false));
    assert!(!zoom.wheel_zooms(true));
}

#[test]
fn the_zoom_threshold_moves_where_card_content_hides() {
    // The probe's binary searches: -1.7, -2.2, -1.0, and never at 2.39.
    for (breakpoint, boundary) in [(0.0, -1.7), (0.5, -2.2), (-0.7, -1.0)] {
        let o = Options {
            zoom_breakpoint: breakpoint,
            ..Options::default()
        };
        assert!((o.content_threshold() - boundary).abs() < 1e-9);
        assert!(o.content_hidden(boundary - 0.01));
        assert!(!o.content_hidden(boundary + 0.01));
    }
    let max = Options {
        zoom_breakpoint: 2.39,
        ..Options::default()
    };
    assert!(!max.content_hidden(crate::ZOOM_MIN));
}

#[test]
fn card_names_follow_show_card_names() {
    let mut o = Options::default();
    assert!(o.shows_card_label(false));
    o.card_labels = CardLabels::Hover;
    assert!(!o.shows_card_label(false));
    assert!(o.shows_card_label(true));
    o.card_labels = CardLabels::Never;
    assert!(!o.shows_card_label(true));
}

#[test]
fn cards_snap_to_each_others_lines_within_fifteen_pixels() {
    // Scale 1: left on left, centre on centre, right on right, left on
    // right, right on left, and nothing at 15.5 (which rounds to whole).
    for (x, want) in [
        (395.0, 400.0),
        (412.0, 400.0),
        (414.5, 400.0),
        (415.5, 416.0),
        (445.0, 450.0),
        (455.0, 450.0),
        (495.0, 500.0),
        (597.0, 600.0),
        (296.0, 300.0),
        (304.0, 300.0),
    ] {
        assert_eq!(land_x(x, 1.0), want, "x {x} at scale 1");
    }
    // The distance is on screen: 7.5 units at scale 2, 30 at 0.5.
    assert_eq!(land_x(407.5, 2.0), 400.0);
    assert_eq!(land_x(408.0, 2.0), 408.0);
    assert_eq!(land_x(424.0, 0.5), 400.0);
    assert_eq!(land_x(427.0, 0.5), 450.0);
    assert_eq!(land_x(470.0, 0.25), 450.0);
}

#[test]
fn an_axis_the_drag_did_not_move_along_does_not_snap() {
    // A at x 405, five units off B's left, dragged straight up: x stays.
    let o = Options::default();
    let c = drag_correction(
        &o,
        Point::new(405.0, 260.0),
        &[a_at(405.0, 260.0)],
        &[B],
        1.0,
        (false, true),
    );
    assert_eq!((405.0 + c.x, 260.0 + c.y), (405.0, 260.0));
    // The grid likewise: 7 + 30 lands on 40, the unmoved 307 stays.
    let grid = Options {
        snap_to_objects: false,
        ..o
    };
    let c = drag_correction(&grid, Point::new(37.0, 307.0), &[], &[], 1.0, (true, false));
    assert_eq!((37.0 + c.x, 307.0 + c.y), (40.0, 307.0));
    let c = drag_correction(&grid, Point::new(37.0, 309.0), &[], &[], 1.0, (true, true));
    assert_eq!((37.0 + c.x, 309.0 + c.y), (40.0, 300.0));
}

#[test]
fn objects_win_over_the_grid_and_alt_leaves_whole_units() {
    let o = Options::default();
    // 413 is 13 from B's left: the object, not the grid's 420.
    let c = drag_correction(
        &o,
        Point::new(413.0, 300.0),
        &[a_at(413.0, 300.0)],
        &[B],
        1.0,
        (true, false),
    );
    assert_eq!(413.0 + c.x, 400.0);
    let free = o.without_snapping();
    let c = drag_correction(
        &free,
        Point::new(403.0, 300.0),
        &[a_at(403.0, 300.0)],
        &[B],
        1.0,
        (true, false),
    );
    assert_eq!(403.0 + c.x, 403.0);
}

#[test]
fn groups_are_not_snapped_to() {
    use scoria_formats::canvas::Canvas;
    let canvas: Canvas = serde_json::from_str(
        r#"{"nodes":[
            {"id":"a","type":"text","text":"A","x":405,"y":300,"width":100,"height":100},
            {"id":"b","type":"text","text":"B","x":400,"y":0,"width":200,"height":100},
            {"id":"g","type":"group","label":"G","x":900,"y":300,"width":300,"height":300}
        ],"edges":[]}"#,
    )
    .unwrap();
    let a = canvas.nodes.iter().position(|n| n.id == "a").unwrap();
    assert_eq!(snap_targets(&canvas, &[a]), vec![B]);
}

#[test]
fn a_resized_side_snaps_to_objects_then_the_grid() {
    let o = Options::default();
    let origin = Rect::new(100.0, 200.0, 100.0, 100.0);
    // The right side dragged to 396: B's left at 400.
    let target = Handle::Right.resize(&origin, 196.0, 0.0, MIN_NODE);
    let r = snap_resize(&o, Handle::Right, &origin, &target, &[B], 15.0, MIN_NODE);
    assert_eq!((r.x, r.width), (100.0, 300.0));
    // Grid only: 7..110 widened by 25 ends at 140; left by -25 starts at -20.
    let grid = Options {
        snap_to_objects: false,
        ..o
    };
    let origin = Rect::new(7.0, 307.0, 103.0, 107.0);
    let target = Handle::Right.resize(&origin, 25.0, 0.0, MIN_NODE);
    let r = snap_resize(&grid, Handle::Right, &origin, &target, &[], 15.0, MIN_NODE);
    assert_eq!((r.x, r.width), (7.0, 133.0));
    let target = Handle::Left.resize(&origin, -25.0, 0.0, MIN_NODE);
    let r = snap_resize(&grid, Handle::Left, &origin, &target, &[], 15.0, MIN_NODE);
    assert_eq!((r.x, r.right()), (-20.0, 110.0));
    let target = Handle::Bottom.resize(&origin, 0.0, 25.0, MIN_NODE);
    let r = snap_resize(&grid, Handle::Bottom, &origin, &target, &[], 15.0, MIN_NODE);
    assert_eq!((r.y, r.height), (307.0, 133.0));
    // Neither (Alt): whole units.
    let target = Handle::Right.resize(&origin, 25.4, 0.0, MIN_NODE);
    let r = snap_resize(
        &o.without_snapping(),
        Handle::Right,
        &origin,
        &target,
        &[B],
        15.0,
        MIN_NODE,
    );
    assert_eq!(r.width, 128.0);
}

#[test]
fn guides_run_through_every_point_on_a_shared_line() {
    // A dropped at (400, 150) under B: x 400 from 0 to 250 with four dots,
    // x 500 from B's centre (y 50) to 250 with three.
    let g = guides(&[a_at(400.0, 150.0)], &[B]);
    let line = |x: f64| g.iter().find(|g| g.from.x == x && g.to.x == x).unwrap();
    assert_eq!((line(400.0).from.y, line(400.0).to.y), (0.0, 250.0));
    assert_eq!(line(400.0).points.len(), 4);
    assert_eq!((line(500.0).from.y, line(500.0).to.y), (50.0, 250.0));
    assert_eq!(line(500.0).points.len(), 3);
    assert_eq!(g.len(), 2);
    // Snapped onto B's row: the two vertical lines, and y 0, 50, 100 across,
    // the middle one between the two centres.
    let g = guides(&[a_at(400.0, 0.0)], &[B]);
    let row = |y: f64| g.iter().find(|g| g.from.y == y && g.to.y == y).unwrap();
    assert_eq!((row(0.0).from.x, row(0.0).to.x), (400.0, 600.0));
    assert_eq!((row(50.0).from.x, row(50.0).to.x), (450.0, 500.0));
    assert_eq!((row(100.0).from.x, row(100.0).to.x), (400.0, 600.0));
    assert_eq!(g.len(), 5);
    // Nothing shared, nothing drawn.
    assert!(guides(&[a_at(0.0, 300.0)], &[B]).is_empty());
}
