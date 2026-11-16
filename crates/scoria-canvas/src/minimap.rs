//! The minimap a canvas embedded in a note is drawn as: `![[Board.canvas]]`
//! is not a live canvas in Obsidian but a picture of the cards' outlines and
//! the edges between them, scaled to fit the note's width
//! ([[Specification#Observed canvas embeds, audio and video, Obsidian 1.15.0]]).
//!
//! Observed (`tools/obsidian-probe/results/2026-11-15-embeds-media.json`,
//! `s01b`, `s01e`): an SVG whose `viewBox` is the cards' bounding box grown
//! by 5 % of its longer side on every side; every card a rectangle with a
//! 20-unit corner radius and a 5-unit stroke of its own fill colour, groups
//! first, then the edges, then the other cards in file order; an edge a
//! cubic from the middle of one side to the middle of the other, its control
//! points half the distance out along the sides' normals and kept within
//! 70..150 units (the canvas view's own reach, without its 7-unit inset),
//! no arrow heads, no labels, its stroke `sqrt(0.1 × longer side)` units.
//! No text, no images, no group labels.

use scoria_formats::canvas::{Canvas, NodeType};

use crate::{anchor, facing_side, side_normal, Point, Rect, EDGE_REACH};

/// Corner radius of a card, in canvas units.
pub const CARD_RADIUS: f64 = 20.0;
/// Stroke of a card's outline, in canvas units (centred on its edge, so
/// half of it lies outside the card).
pub const CARD_STROKE: f64 = 5.0;
/// The margin round the cards, as a share of the bounding box's longer side.
pub const MARGIN: f64 = 0.05;
/// Opacity of a card's fill: an uncoloured card's, and a coloured one's.
/// The outline is drawn opaque.
pub const FILL_OPACITY: f32 = 0.65;
pub const COLORED_FILL_OPACITY: f32 = 0.5;

/// One card's outline.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub rect: Rect,
    /// The card's `color` as the file writes it: `"1"`..`"6"` or `#rrggbb`.
    pub color: Option<String>,
    pub group: bool,
}

/// One edge's curve.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub from: Point,
    pub c1: Point,
    pub c2: Point,
    pub to: Point,
    pub color: Option<String>,
}

/// What the embed draws, in canvas units: the box the picture fits and
/// the shapes inside it, in paint order.
#[derive(Debug, Clone, PartialEq)]
pub struct Minimap {
    pub view: Rect,
    /// The edges' stroke width, in canvas units.
    pub edge_width: f64,
    /// Groups, in file order: painted under everything.
    pub groups: Vec<Card>,
    pub edges: Vec<Curve>,
    /// The other cards, in file order: painted over the edges.
    pub cards: Vec<Card>,
}

impl Minimap {
    /// The minimap of `canvas`, or `None` for a canvas with no cards (Obsidian
    /// draws the title and no picture then).
    ///
    /// An edge whose card is missing is left out. One with no recorded side
    /// leaves from the side facing the other card, the way the canvas view
    /// draws it; Obsidian 1.15.0 draws **no picture at all** for a canvas
    /// holding such an edge (`s01e`, `Edges.canvas`), which reads as a fault
    /// rather than a rule, so Scoria does not copy it.
    pub fn of(canvas: &Canvas) -> Option<Minimap> {
        let bounds = crate::bounds(&canvas.nodes)?;
        let pad = MARGIN * bounds.width.max(bounds.height);
        let view = Rect::new(
            bounds.x - pad,
            bounds.y - pad,
            bounds.width + 2.0 * pad,
            bounds.height + 2.0 * pad,
        );
        let card = |n: &scoria_formats::canvas::Node| Card {
            rect: Rect::of(n),
            color: n.color.clone().filter(|c| !c.is_empty()),
            group: n.kind == NodeType::Group,
        };
        let groups = canvas
            .nodes
            .iter()
            .filter(|n| n.kind == NodeType::Group)
            .map(card)
            .collect();
        let cards = canvas
            .nodes
            .iter()
            .filter(|n| n.kind != NodeType::Group)
            .map(card)
            .collect();
        let edges = canvas
            .edges
            .iter()
            .filter_map(|e| {
                let from = canvas.nodes.iter().find(|n| n.id == e.from_node)?;
                let to = canvas.nodes.iter().find(|n| n.id == e.to_node)?;
                let (fr, tr) = (Rect::of(from), Rect::of(to));
                let fs = e.from_side.unwrap_or_else(|| facing_side(&fr, tr.center()));
                let ts = e.to_side.unwrap_or_else(|| facing_side(&tr, fr.center()));
                Some(curve(
                    anchor(&fr, fs),
                    fs,
                    anchor(&tr, ts),
                    ts,
                    e.color.clone().filter(|c| !c.is_empty()),
                ))
            })
            .collect();
        Some(Minimap {
            view,
            edge_width: (2.0 * pad).sqrt(),
            groups,
            edges,
            cards,
        })
    }

    /// The scale and offset that fit the view into a `width` × `height` box
    /// (the SVG's `xMidYMid meet`): screen = offset + (canvas − view.origin)
    /// × scale.
    pub fn fit(&self, width: f64, height: f64) -> (f64, Point) {
        let scale = (width / self.view.width).min(height / self.view.height);
        let offset = Point::new(
            (width - self.view.width * scale) / 2.0,
            (height - self.view.height * scale) / 2.0,
        );
        (scale, offset)
    }
}

/// The picture's height in a box `width` wide: the view's aspect at that
/// width less the 4-unit padding round it, the padding added back, and no
/// more than `max` (the embed's `max-height`, 400 px in Obsidian).
pub fn picture_height(view: &Rect, width: f64, padding: f64, max: f64) -> f64 {
    let inner = (width - 2.0 * padding).max(0.0);
    let h = if view.width > 0.0 {
        inner * view.height / view.width
    } else {
        0.0
    };
    (h + 2.0 * padding).min(max)
}

/// The curve between two anchors: `edge_path_points`'s, less its inset, so
/// the minimap's ends sit on the cards' edges.
fn curve(
    a: Point,
    fs: scoria_formats::canvas::Side,
    b: Point,
    ts: scoria_formats::canvas::Side,
    color: Option<String>,
) -> Curve {
    let na = side_normal(fs);
    let nb = side_normal(ts);
    let reach = (a.distance(b) * 0.5).clamp(EDGE_REACH.0, EDGE_REACH.1);
    Curve {
        from: a,
        c1: Point::new(a.x + na.x * reach, a.y + na.y * reach),
        c2: Point::new(b.x + nb.x * reach, b.y + nb.y * reach),
        to: b,
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(json: &str) -> Canvas {
        Canvas::from_json(json).unwrap()
    }

    const BOARD: &str = r##"{"nodes":[
        {"id":"g1","type":"group","x":-60,"y":-60,"width":760,"height":420,"label":"Group"},
        {"id":"t1","type":"text","x":0,"y":0,"width":250,"height":60,"text":"Text card"},
        {"id":"t2","type":"text","x":400,"y":0,"width":250,"height":120,"text":"x","color":"4"},
        {"id":"f1","type":"file","x":0,"y":160,"width":300,"height":160,"file":"Other.md"},
        {"id":"l1","type":"link","x":400,"y":200,"width":250,"height":100,"url":"https://example.com"}],
      "edges":[{"id":"e1","fromNode":"t1","fromSide":"right","toNode":"t2","toSide":"left","label":"edge"}]}"##;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn the_view_is_the_bounds_grown_by_a_twentieth_of_the_longer_side() {
        // Observed: viewBox="-98 -98 836 496" over cards spanning 760 × 420.
        let m = Minimap::of(&canvas(BOARD)).unwrap();
        assert_eq!(m.view, Rect::new(-98.0, -98.0, 836.0, 496.0));
        // Small.canvas: 300 × 40 → "-15 -15 330 70".
        let small = canvas(
            r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":100,"height":40,"text":"A"},
            {"id":"b","type":"text","x":200,"y":0,"width":100,"height":40,"text":"B"}],
            "edges":[{"id":"e","fromNode":"a","fromSide":"right","toNode":"b","toSide":"left"}]}"#,
        );
        let s = Minimap::of(&small).unwrap();
        assert_eq!(s.view, Rect::new(-15.0, -15.0, 330.0, 70.0));
        // The edge stroke: 8.7178 and 5.47723 observed.
        assert!(close(m.edge_width, 8.7178), "{}", m.edge_width);
        assert!(close(s.edge_width, 5.47723), "{}", s.edge_width);
        // "M100,20 C170,20 130,20 200,20": the reach is at least 70.
        let e = &s.edges[0];
        assert_eq!(
            (e.from, e.c1, e.c2, e.to),
            (
                Point::new(100.0, 20.0),
                Point::new(170.0, 20.0),
                Point::new(130.0, 20.0),
                Point::new(200.0, 20.0)
            )
        );
    }

    #[test]
    fn the_board_edge_reaches_half_its_length() {
        // "M250,30 C326.4852927038918,30 323.5147072961082,60 400,60".
        let m = Minimap::of(&canvas(BOARD)).unwrap();
        let e = &m.edges[0];
        assert_eq!(e.from, Point::new(250.0, 30.0));
        assert!(close(e.c1.x, 326.4852927038918) && close(e.c1.y, 30.0));
        assert!(close(e.c2.x, 323.5147072961082) && close(e.c2.y, 60.0));
        assert_eq!(e.to, Point::new(400.0, 60.0));
    }

    #[test]
    fn groups_paint_first_and_cards_keep_their_order_and_colours() {
        let m = Minimap::of(&canvas(BOARD)).unwrap();
        assert_eq!(m.groups.len(), 1);
        assert!(m.groups[0].group);
        assert_eq!(m.groups[0].rect, Rect::new(-60.0, -60.0, 760.0, 420.0));
        let colors: Vec<_> = m.cards.iter().map(|c| c.color.as_deref()).collect();
        assert_eq!(colors, vec![None, Some("4"), None, None]);
        assert_eq!(m.cards[2].rect, Rect::new(0.0, 160.0, 300.0, 160.0));
    }

    #[test]
    fn a_long_edge_is_held_to_150_and_keeps_its_colour() {
        // Edges2.canvas: "M100,50 C250,50 1050,350 1050,500", class color-1.
        let c = canvas(
            r##"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":100,"height":100,"text":"A"},
            {"id":"b","type":"text","x":1000,"y":500,"width":100,"height":100,"text":"B","color":"#ff0000"}],
            "edges":[{"id":"e2","fromNode":"a","fromSide":"right","toNode":"b","toSide":"top","color":"1"}]}"##,
        );
        let m = Minimap::of(&c).unwrap();
        assert_eq!(m.view, Rect::new(-55.0, -55.0, 1210.0, 710.0));
        assert!(close(m.edge_width, 10.4881));
        let e = &m.edges[0];
        assert_eq!(
            (e.from, e.c1, e.c2, e.to),
            (
                Point::new(100.0, 50.0),
                Point::new(250.0, 50.0),
                Point::new(1050.0, 350.0),
                Point::new(1050.0, 500.0)
            )
        );
        assert_eq!(e.color.as_deref(), Some("1"));
        assert_eq!(m.cards[1].color.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn no_cards_no_picture_and_dangling_edges_are_dropped() {
        assert!(Minimap::of(&canvas(r#"{"nodes":[],"edges":[]}"#)).is_none());
        let c = canvas(
            r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":100,"height":100,"text":"A"}],
            "edges":[{"id":"e","fromNode":"a","toNode":"gone"}]}"#,
        );
        assert!(Minimap::of(&c).unwrap().edges.is_empty());
    }

    #[test]
    fn an_edge_without_sides_faces_the_other_card() {
        let c = canvas(
            r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":100,"height":100,"text":"A"},
            {"id":"b","type":"text","x":1000,"y":0,"width":100,"height":100,"text":"B"}],
            "edges":[{"id":"e","fromNode":"a","toNode":"b"}]}"#,
        );
        let e = &Minimap::of(&c).unwrap().edges[0];
        assert_eq!(
            (e.from, e.to),
            (Point::new(100.0, 50.0), Point::new(1000.0, 50.0))
        );
    }

    #[test]
    fn the_picture_fits_the_width_up_to_400() {
        // Board: 700 wide, 4 of padding: (692 × 496 / 836) + 8 = 418.6 → 400.
        let board = Rect::new(-98.0, -98.0, 836.0, 496.0);
        assert_eq!(picture_height(&board, 700.0, 4.0, 400.0), 400.0);
        // Wide.canvas, 3000 × 100 → view 3300 × 400: 91.9 observed.
        let wide = Rect::new(-150.0, -150.0, 3300.0, 400.0);
        assert!((picture_height(&wide, 700.0, 4.0, 400.0) - 91.88).abs() < 0.05);
        // Meet: the board in 692 × 392 is height-bound and centred across.
        let m = Minimap::of(&canvas(BOARD)).unwrap();
        let (scale, offset) = m.fit(692.0, 392.0);
        assert!(close(scale, 392.0 / 496.0));
        assert!(close(offset.y, 0.0));
        assert!(close(offset.x, (692.0 - 836.0 * scale) / 2.0));
    }
}
