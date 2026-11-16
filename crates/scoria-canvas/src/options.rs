//! The Canvas plugin's settings (`canvas.json`) as the view acts on them, and
//! the snapping they switch: to the grid and to other cards.
//!
//! Observed in Obsidian 1.15.0 over CDP on 2026-11-16
//! (`tools/obsidian-probe/canvas-settings.mjs`, see `docs/scoria/
//! Specification.md`, "Observed Canvas settings, Obsidian 1.15.x"). Every
//! option takes effect at once in an open canvas, no reopening.

use scoria_formats::canvas::{Canvas, NodeType};
use scoria_formats::config::CanvasConfig;

use crate::{snap, Handle, Point, Rect, ZOOM_THRESHOLD};

/// How close, in screen pixels, a card's edge or centre line has to come to
/// another card's before it snaps there: a 14.5 px gap snapped and a 15.5 px
/// one did not, at scales 1, 2, 0.5 and 0.25 alike.
pub const SNAP_DISTANCE: f64 = 15.0;

/// "Default mouse wheel behavior".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelBehavior {
    /// A plain wheel pans and Ctrl (or Cmd) zooms.
    #[default]
    Pan,
    /// A plain wheel zooms and Ctrl (or Cmd) pans.
    Zoom,
}

/// "Default Ctrl + Drag behavior": what a Ctrl+drag on the background does
/// with the rectangle it drew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModDrag {
    /// Opens the creation menu for the rectangle.
    #[default]
    Menu,
    /// A text card filling the rectangle, open for editing.
    Card,
    /// The note picker; the pick fills the rectangle.
    Note,
    /// The media picker; the pick fills the rectangle.
    Media,
    /// The "Enter URL" prompt.
    Webpage,
    /// A group named `Untitled group` filling the rectangle, selected.
    Group,
}

/// "Show card names": the name above a file or link card. Group labels are
/// not affected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardLabels {
    #[default]
    Always,
    /// Shown only while the pointer is over the card itself; a selected
    /// card the pointer has left hides it again.
    Hover,
    /// Not drawn at all.
    Never,
}

/// The options the view reads, with Obsidian's defaults for absent keys and
/// for strings it does not know.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub snap_to_grid: bool,
    pub snap_to_objects: bool,
    pub wheel: WheelBehavior,
    pub mod_drag: ModDrag,
    pub card_labels: CardLabels,
    /// `zoomBreakpoint`, 0 by default.
    pub zoom_breakpoint: f64,
}

impl Default for Options {
    fn default() -> Options {
        Options::from_config(&CanvasConfig::default())
    }
}

impl Options {
    pub fn from_config(config: &CanvasConfig) -> Options {
        Options {
            snap_to_grid: config.snap_to_grid.unwrap_or(true),
            snap_to_objects: config.snap_to_objects.unwrap_or(true),
            wheel: match config.default_wheel_behavior.as_deref() {
                Some("zoom") => WheelBehavior::Zoom,
                _ => WheelBehavior::Pan,
            },
            mod_drag: match config.default_mod_drag_behavior.as_deref() {
                Some("card") => ModDrag::Card,
                Some("note") => ModDrag::Note,
                Some("media") => ModDrag::Media,
                Some("webpage") => ModDrag::Webpage,
                Some("group") => ModDrag::Group,
                _ => ModDrag::Menu,
            },
            card_labels: match config.card_label_visibility.as_deref() {
                Some("hover") => CardLabels::Hover,
                Some("never") => CardLabels::Never,
                _ => CardLabels::Always,
            },
            zoom_breakpoint: config
                .zoom_breakpoint
                .as_ref()
                .and_then(|n| n.as_f64())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0),
        }
    }

    /// Whether a wheel event zooms rather than pans: Ctrl (or Cmd) swaps
    /// what the plain wheel does.
    pub fn wheel_zooms(&self, ctrl: bool) -> bool {
        (self.wheel == WheelBehavior::Zoom) != ctrl
    }

    /// The zoom at and below which a card's content gives way to its
    /// placeholder: `-1.7 - zoomBreakpoint` (-1.7, -2.2 at 0.5, -1.0 at
    /// -0.7, below the zoom range at 2.39, so content always shows).
    pub fn content_threshold(&self) -> f64 {
        ZOOM_THRESHOLD - self.zoom_breakpoint
    }

    /// Whether cards show placeholders at `zoom`. The wrapper's own
    /// `mod-zoomed-out` -- which hides a file card's name -- stays at
    /// [`ZOOM_THRESHOLD`] whatever the setting says.
    pub fn content_hidden(&self, zoom: f64) -> bool {
        zoom <= self.content_threshold()
    }

    /// The same options with both kinds of snapping off: what holding Alt
    /// does to a drag or a resize.
    pub fn without_snapping(self) -> Options {
        Options {
            snap_to_grid: false,
            snap_to_objects: false,
            ..self
        }
    }

    /// Whether a file or link card's name is drawn, given whether the
    /// pointer is over that card.
    pub fn shows_card_label(&self, hovered: bool) -> bool {
        match self.card_labels {
            CardLabels::Always => true,
            CardLabels::Hover => hovered,
            CardLabels::Never => false,
        }
    }
}

// ---- snapping to objects -------------------------------------------------------
//
// A card offers three vertical lines (left, centre, right) and three
// horizontal ones (top, middle, bottom). While cards move, each moving card's
// lines are matched against every other card's, and the closest pair within
// `SNAP_DISTANCE / scale` sets the axis; the axes are independent, and an
// axis the drag has not moved along does not snap. Groups are neither moved
// against nor snapped to: a card 3 units from a group's frame stayed there.
// Snapping to objects wins over the grid when both are on; the grid takes an
// axis no object claims.

/// The x lines and y lines of a rectangle: left, centre, right; top,
/// middle, bottom.
fn lines(r: &Rect) -> ([f64; 3], [f64; 3]) {
    (
        [r.x, r.x + r.width / 2.0, r.right()],
        [r.y, r.y + r.height / 2.0, r.bottom()],
    )
}

/// What the moving nodes snap against: every other node but groups.
pub fn snap_targets(canvas: &Canvas, moving: &[usize]) -> Vec<Rect> {
    canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| n.kind != NodeType::Group && !moving.contains(i))
        .map(|(_, n)| Rect::of(n))
        .collect()
}

/// The smallest correction that puts one of `moving` on one of `fixed`,
/// if it is within `tolerance`. Ties keep the first found.
fn closest(moving: &[f64], fixed: &[f64], tolerance: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for m in moving {
        for f in fixed {
            let d = f - m;
            if d.abs() <= tolerance && best.is_none_or(|b| d.abs() < b.abs()) {
                best = Some(d);
            }
        }
    }
    best
}

/// The correction, per axis, that snaps the moving rectangles to the
/// others' lines: `None` on an axis with nothing within `tolerance` (canvas
/// units) or one the drag did not move along.
pub fn object_snap(
    moving: &[Rect],
    others: &[Rect],
    tolerance: f64,
    moved: (bool, bool),
) -> (Option<f64>, Option<f64>) {
    let (mut mx, mut my) = (Vec::new(), Vec::new());
    for r in moving {
        let (x, y) = lines(r);
        mx.extend(x);
        my.extend(y);
    }
    let (mut fx, mut fy) = (Vec::new(), Vec::new());
    for r in others {
        let (x, y) = lines(r);
        fx.extend(x);
        fy.extend(y);
    }
    (
        moved.0.then(|| closest(&mx, &fx, tolerance)).flatten(),
        moved.1.then(|| closest(&my, &fy, tolerance)).flatten(),
    )
}

/// The correction a drag applies on top of the pointer's delta, per axis:
/// objects first, then the grid (rounding `anchor`, the pressed card's top
/// left), and otherwise rounding `anchor` to whole units, since Obsidian
/// keeps positions whole (415.5 landed on 416). With Alt held the caller
/// passes [`Options::without_snapping`], which leaves only the rounding.
pub fn drag_correction(
    options: &Options,
    anchor: Point,
    moving: &[Rect],
    others: &[Rect],
    scale: f64,
    moved: (bool, bool),
) -> Point {
    let (ox, oy) = if options.snap_to_objects {
        object_snap(moving, others, SNAP_DISTANCE / scale, moved)
    } else {
        (None, None)
    };
    let grid = options.snap_to_grid;
    let axis = |object: Option<f64>, v: f64, moved: bool| match object {
        Some(d) => d,
        None if grid && moved => snap(v) - v,
        None => v.round() - v,
    };
    Point::new(axis(ox, anchor.x, moved.0), axis(oy, anchor.y, moved.1))
}

/// Snaps the sides a resize handle moves: to another card's line within
/// `tolerance` when snapping to objects, else to the grid when that is on,
/// else to whole units. The still sides stay where `origin` has them, and
/// the card stays at least `min`.
pub fn snap_resize(
    options: &Options,
    handle: Handle,
    origin: &Rect,
    target: &Rect,
    others: &[Rect],
    tolerance: f64,
    min: (f64, f64),
) -> Rect {
    let moves_left = matches!(handle, Handle::Left | Handle::TopLeft | Handle::BottomLeft);
    let moves_right = matches!(
        handle,
        Handle::Right | Handle::TopRight | Handle::BottomRight
    );
    let moves_top = matches!(handle, Handle::Top | Handle::TopLeft | Handle::TopRight);
    let moves_bottom = matches!(
        handle,
        Handle::Bottom | Handle::BottomLeft | Handle::BottomRight
    );
    let (mut fx, mut fy) = (Vec::new(), Vec::new());
    if options.snap_to_objects {
        for r in others {
            let (x, y) = lines(r);
            fx.extend(x);
            fy.extend(y);
        }
    }
    let grid = options.snap_to_grid;
    let side = |v: f64, fixed: &[f64]| match closest(&[v], fixed, tolerance) {
        Some(d) => v + d,
        None if grid => snap(v),
        None => v.round(),
    };
    let (mut x, mut right) = (target.x, target.right());
    let (mut y, mut bottom) = (target.y, target.bottom());
    if moves_left {
        x = side(x, &fx).min(origin.right() - min.0);
    }
    if moves_right {
        right = side(right, &fx).max(origin.x + min.0);
    }
    if moves_top {
        y = side(y, &fy).min(origin.bottom() - min.1);
    }
    if moves_bottom {
        bottom = side(bottom, &fy).max(origin.y + min.1);
    }
    Rect::new(x, y, right - x, bottom - y)
}

/// One alignment line drawn while cards snap: a 1 px accent line through
/// every point on it, from the first to the last, with a dot on each.
#[derive(Debug, Clone, PartialEq)]
pub struct Guide {
    pub from: Point,
    pub to: Point,
    pub points: Vec<Point>,
}

/// The points of a rectangle that lie on each of its lines: the two
/// corners of a side, or the centre for a centre line.
fn line_points(r: &Rect) -> [(bool, f64, Vec<Point>); 6] {
    let c = r.center();
    [
        (
            true,
            r.x,
            vec![Point::new(r.x, r.y), Point::new(r.x, r.bottom())],
        ),
        (true, c.x, vec![c]),
        (
            true,
            r.right(),
            vec![
                Point::new(r.right(), r.y),
                Point::new(r.right(), r.bottom()),
            ],
        ),
        (
            false,
            r.y,
            vec![Point::new(r.x, r.y), Point::new(r.right(), r.y)],
        ),
        (false, c.y, vec![c]),
        (
            false,
            r.bottom(),
            vec![
                Point::new(r.x, r.bottom()),
                Point::new(r.right(), r.bottom()),
            ],
        ),
    ]
}

/// The guides for moving rectangles at rest against the others: one per
/// line a moving card shares with a still one, through every point of
/// either on that line (observed: dragging a 100-unit card to (400, 150)
/// under a 200 by 100 card at (400, 0) drew x = 400 from y 0 to 250 with
/// four dots, and x = 500 from y 50 to 250 through the upper card's centre).
pub fn guides(moving: &[Rect], others: &[Rect]) -> Vec<Guide> {
    const EPS: f64 = 1e-6;
    let mut out: Vec<(bool, f64, Vec<Point>)> = Vec::new();
    for m in moving {
        for (vertical, at, pts) in line_points(m) {
            let matched: Vec<Point> = others
                .iter()
                .flat_map(|o| line_points(o).into_iter())
                .filter(|(v, a, _)| *v == vertical && (a - at).abs() < EPS)
                .flat_map(|(_, _, p)| p)
                .collect();
            if matched.is_empty() {
                continue;
            }
            match out
                .iter_mut()
                .find(|(v, a, _)| *v == vertical && (a - at).abs() < EPS)
            {
                Some((_, _, existing)) => {
                    existing.extend(pts);
                    existing.extend(matched);
                }
                None => {
                    let mut all = pts;
                    all.extend(matched);
                    out.push((vertical, at, all));
                }
            }
        }
    }
    out.into_iter()
        .map(|(vertical, at, mut points)| {
            points.sort_by(|a, b| {
                let (ka, kb) = if vertical { (a.y, b.y) } else { (a.x, b.x) };
                ka.total_cmp(&kb)
            });
            points.dedup_by(|a, b| (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS);
            let (first, last) = (points[0], points[points.len() - 1]);
            let (from, to) = if vertical {
                (Point::new(at, first.y), Point::new(at, last.y))
            } else {
                (Point::new(first.x, at), Point::new(last.x, at))
            };
            Guide { from, to, points }
        })
        .collect()
}
