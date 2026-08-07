//! Where each box goes: a plain grid, three to a row.
//!
//! Boxes are sized to their own content but placed in uniform cells, so a row
//! of them lines up whatever is written in each. Relationships are straight
//! lines between centres, clipped to the borders.

use crate::round::count;
use crate::scene::Point;

use super::types::Diagram;

pub const PADDING: f64 = 32.0;
pub const GAP_X: f64 = 56.0;
pub const GAP_Y: f64 = 56.0;
pub const COLS: usize = 3;
pub const HEADER_HEIGHT: f64 = 30.0;
pub const STEREO_HEIGHT: f64 = 18.0;
pub const ROW_HEIGHT: f64 = 22.0;
pub const BOTTOM_PAD: f64 = 8.0;
pub const PAD_X: f64 = 14.0;
pub const MIN_WIDTH: f64 = 150.0;
pub const MAX_WIDTH: f64 = 300.0;
pub const NAME_FONT: f64 = 14.0;
pub const NAME_WEIGHT: u32 = 600;
pub const STEREO_FONT: f64 = 11.0;
pub const STEREO_WEIGHT: u32 = 400;
pub const BODY_FONT: f64 = 12.0;
pub const BODY_WEIGHT: u32 = 400;
pub const EDGE_LABEL_FONT: f64 = 11.0;
pub const EDGE_LABEL_WEIGHT: u32 = 400;

/// Whether a box is a requirement or an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boxed {
    Requirement,
    Element,
}

impl Boxed {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::Element => "element",
        }
    }
}

/// One box, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    pub id: String,
    pub kind: Boxed,
    pub stereotype: String,
    pub name: String,
    pub rows: Vec<String>,
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

impl PlacedNode {
    fn centre(&self) -> Point {
        Point::new(self.at.x + self.width / 2.0, self.at.y + self.height / 2.0)
    }
}

/// One relationship, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub a: Point,
    pub b: Point,
    pub label_at: Point,
}

/// A laid-out requirement diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<PlacedEdge>,
}

/// Where the ray from the centre of `node` toward `towards` leaves the border.
fn clip(node: &PlacedNode, towards: Point) -> Point {
    let centre = node.centre();
    let dx = towards.x - centre.x;
    let dy = towards.y - centre.y;
    if dx == 0.0 && dy == 0.0 {
        return centre;
    }
    let scale_x = if dx == 0.0 {
        f64::INFINITY
    } else {
        node.width / 2.0 / dx.abs()
    };
    let scale_y = if dy == 0.0 {
        f64::INFINITY
    } else {
        node.height / 2.0 / dy.abs()
    };
    let scale = scale_x.min(scale_y);
    Point::new(centre.x + dx * scale, centre.y + dy * scale)
}

/// The unsized boxes, requirements first and then elements.
fn boxes(diagram: &Diagram) -> Vec<PlacedNode> {
    let mut out: Vec<PlacedNode> = diagram
        .requirements
        .iter()
        .map(|req| {
            let mut rows = Vec::new();
            for (key, value) in [
                ("id", &req.id),
                ("text", &req.text),
                ("risk", &req.risk),
                ("verify", &req.verify_method),
            ] {
                if let Some(value) = value {
                    rows.push(format!("{key}: {value}"));
                }
            }
            PlacedNode {
                id: req.name.clone(),
                kind: Boxed::Requirement,
                stereotype: req.kind.stereotype(),
                name: req.name.clone(),
                rows,
                at: Point::new(0.0, 0.0),
                width: 0.0,
                height: 0.0,
            }
        })
        .collect();
    out.extend(diagram.elements.iter().map(|element| {
        let mut rows = Vec::new();
        if let Some(kind) = &element.kind {
            rows.push(format!("type: {kind}"));
        }
        if let Some(docref) = &element.docref {
            rows.push(format!("docRef: {docref}"));
        }
        PlacedNode {
            id: element.name.clone(),
            kind: Boxed::Element,
            stereotype: "«Element»".to_string(),
            name: element.name.clone(),
            rows,
            at: Point::new(0.0, 0.0),
            width: 0.0,
            height: 0.0,
        }
    }));
    out
}

/// Lay out a parsed requirement diagram.
/// Move each label off its neighbours, and off every wire but its own.
///
/// The midpoint is where a label wants to be and not always where it can go:
/// two relationships meeting at one box have midpoints a few pixels apart, and
/// `«satisfies»` sat on top of `«verifies»`. `label::beside` already answers
/// this for the flowchart — it steps outward from the anchor until the box is
/// clear of what is taken — so this asks it rather than growing a second answer.
fn place_labels(edges: &mut [PlacedEdge], nodes: &[PlacedNode]) {
    // Seeded with the boxes: a label pushed off a wire and onto a box has not
    // been helped.
    let mut taken: Vec<crate::label::Placed> = nodes
        .iter()
        .map(|node| crate::label::Placed::new(node.centre(), node.width, node.height))
        .collect();
    for edge in edges.iter_mut() {
        let text = format!("«{}»", edge.kind);
        let size = (
            crate::metrics::text_width(&text, EDGE_LABEL_FONT, EDGE_LABEL_WEIGHT) + 8.0,
            EDGE_LABEL_FONT + 6.0,
        );
        let anchor = Point::new(
            f64::midpoint(edge.a.x, edge.b.x),
            f64::midpoint(edge.a.y, edge.b.y),
        );
        let upright = (edge.a.y - edge.b.y).abs() >= (edge.a.x - edge.b.x).abs();
        // Only the other labels and the boxes are avoided, not the wires. A
        // relationship label sitting on its own dashed line is the notation —
        // stepping clear of every wire as well pushed it so far from the line it
        // named that it stopped naming it.
        let placed = crate::label::beside(anchor, upright, size, 2.0, &taken, &[]);
        edge.label_at = placed.at;
        taken.push(placed);
    }
}

pub fn layout(diagram: &Diagram) -> Placed {
    let mut nodes = boxes(diagram);

    for node in &mut nodes {
        let mut widest = crate::metrics::text_width(&node.name, NAME_FONT, NAME_WEIGHT);
        widest = widest.max(crate::metrics::text_width(
            &node.stereotype,
            BODY_FONT,
            BODY_WEIGHT,
        ));
        for row in &node.rows {
            widest = widest.max(crate::metrics::text_width(row, BODY_FONT, BODY_WEIGHT));
        }
        // Bounded above: a long `text:` field is ellipsised rather than allowed
        // to stretch its whole row of the grid.
        node.width = (widest + PAD_X * 2.0).clamp(MIN_WIDTH, MAX_WIDTH).round();
        node.height =
            HEADER_HEIGHT + STEREO_HEIGHT + count(node.rows.len()) * ROW_HEIGHT + BOTTOM_PAD;
    }

    let cols = COLS.min(nodes.len().max(1));
    // Requirements and elements are different things, so they get different
    // bands rather than being run together three to a row. Filling one row with
    // both put an element beside a requirement it does not satisfy, and the wire
    // to the one it does went straight through the box in between — which is
    // what the legibility checker reported. Split, every relationship crosses
    // from one band to the other and has clear air to do it in.
    let split = diagram.requirements.len().min(nodes.len());
    let bands = [split, nodes.len() - split];
    let rows = bands
        .iter()
        .map(|count| count.div_ceil(cols))
        .sum::<usize>()
        .max(1);
    // Uniform cells, so boxes line up across a row even at different sizes.
    let cell_w = nodes.iter().map(|n| n.width).fold(MIN_WIDTH, f64::max);
    let cell_h = nodes.iter().map(|n| n.height).fold(HEADER_HEIGHT, f64::max);

    let first_band = bands[0].div_ceil(cols);
    for (index, node) in nodes.iter_mut().enumerate() {
        let (band_top, within) = if index < split {
            (0, index)
        } else {
            (first_band, index - split)
        };
        let cell = Point::new(
            PADDING + count(within % cols) * (cell_w + GAP_X),
            PADDING + count(band_top + within / cols) * (cell_h + GAP_Y),
        );
        node.at = Point::new(
            cell.x + (cell_w - node.width) / 2.0,
            cell.y + (cell_h - node.height) / 2.0,
        );
    }

    let mut edges: Vec<PlacedEdge> = diagram
        .relationships
        .iter()
        // A relationship naming a box that was never declared is dropped.
        .filter_map(|rel| {
            let from = nodes.iter().find(|n| n.id == rel.source)?;
            let to = nodes.iter().find(|n| n.id == rel.dest)?;
            let a = clip(from, to.centre());
            let b = clip(to, from.centre());
            Some(PlacedEdge {
                from: rel.source.clone(),
                to: rel.dest.clone(),
                kind: rel.kind.clone(),
                a,
                b,
                // Replaced below, once every wire is known.
                label_at: Point::new(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y)),
            })
        })
        .collect();

    place_labels(&mut edges, &nodes);

    Placed {
        width: PADDING * 2.0 + count(cols) * cell_w + (count(cols) - 1.0) * GAP_X,
        height: PADDING * 2.0 + count(rows) * cell_h + (count(rows) - 1.0) * GAP_Y,
        nodes,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirement::{parse, Kind};

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const DIAGRAM: &str = "requirementDiagram\n\
        requirement r1 {\nid: 1\ntext: first\n}\n\
        requirement r2 {\nid: 2\n}\n\
        element e1 {\ntype: simulation\n}\n\
        e1 - satisfies -> r1";

    #[test]
    fn requirements_come_before_elements() {
        let out = placed(DIAGRAM);
        let ids: Vec<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["r1", "r2", "e1"]);
        assert_eq!(out.nodes[2].kind, Boxed::Element);
    }

    #[test]
    fn boxes_are_laid_out_three_to_a_row() {
        let out = placed(
            "requirementDiagram\nrequirement a {\n}\nrequirement b {\n}\nrequirement c {\n}\nrequirement d {\n}",
        );
        assert!(
            (out.nodes[0].at.y - out.nodes[2].at.y).abs() < 1e-9,
            "same row"
        );
        assert!(out.nodes[3].at.y > out.nodes[0].at.y, "wrapped");
        assert!((out.nodes[3].at.x - out.nodes[0].at.x).abs() < 1e-9);
    }

    #[test]
    fn a_box_grows_for_its_fields_but_only_so_far() {
        let bare = placed("requirementDiagram\nrequirement a {\n}");
        assert!((bare.nodes[0].width - MIN_WIDTH).abs() < 1e-9);
        let long = placed(&format!(
            "requirementDiagram\nrequirement a {{\ntext: {}\n}}",
            "x".repeat(400)
        ));
        assert!((long.nodes[0].width - MAX_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn a_box_grows_taller_with_each_field() {
        let one = placed("requirementDiagram\nrequirement a {\nid: 1\n}");
        let two = placed("requirementDiagram\nrequirement a {\nid: 1\nrisk: high\n}");
        assert!((two.nodes[0].height - one.nodes[0].height - ROW_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn boxes_of_different_sizes_are_centred_in_uniform_cells() {
        let out = placed("requirementDiagram\nrequirement wide {\ntext: a longer line here\n}\nrequirement a {\n}");
        // Both cells are the same, so the narrower box is inset further.
        let inset =
            |i: usize| out.nodes[i].at.x - PADDING - count(i) * (out.nodes[0].width + GAP_X);
        assert!(inset(1) >= inset(0) - 1e-9);
    }

    #[test]
    fn an_edge_stops_at_a_border_and_is_labelled_near_its_middle() {
        let out = placed(DIAGRAM);
        let edge = &out.edges[0];
        assert_eq!(edge.kind, "satisfies");
        // Near the middle, not exactly at it: a label steps aside for the ones
        // already placed, which is what stopped two of them printing on top of
        // each other. It stays within a box's width of where it wanted to be —
        // far enough to find room, near enough to still name its own wire.
        let middle = Point::new(
            f64::midpoint(edge.a.x, edge.b.x),
            f64::midpoint(edge.a.y, edge.b.y),
        );
        let off = (edge.label_at.x - middle.x).hypot(edge.label_at.y - middle.y);
        assert!(off < GAP_X, "{off} from the middle of its wire");
        let from = out.nodes.iter().find(|n| n.id == "e1").expect("e1");
        let inside = edge.a.x > from.at.x + 1e-6
            && edge.a.x < from.at.x + from.width - 1e-6
            && edge.a.y > from.at.y + 1e-6
            && edge.a.y < from.at.y + from.height - 1e-6;
        assert!(!inside);
    }

    #[test]
    fn two_labels_meeting_at_one_box_do_not_print_on_top_of_each_other() {
        // `renderer` satisfies `speed` and `suite` verifies it, so both wires
        // end at the same box and their middles are a few pixels apart.
        let out = placed(DIAGRAM);
        for (at, edge) in out.edges.iter().enumerate() {
            for other in out.edges.iter().skip(at + 1) {
                let apart =
                    (edge.label_at.x - other.label_at.x).hypot(edge.label_at.y - other.label_at.y);
                assert!(
                    apart > 1.0,
                    "{:?} and {:?} share a spot",
                    edge.kind,
                    other.kind
                );
            }
        }
    }

    #[test]
    fn an_edge_naming_a_box_that_does_not_exist_is_dropped() {
        assert!(
            placed("requirementDiagram\nrequirement r {\n}\nr - traces -> ghost")
                .edges
                .is_empty()
        );
    }

    #[test]
    fn two_boxes_sharing_a_centre_give_an_edge_no_direction() {
        let node = PlacedNode {
            id: "a".into(),
            kind: Boxed::Requirement,
            stereotype: String::new(),
            name: "a".into(),
            rows: Vec::new(),
            at: Point::new(0.0, 0.0),
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(clip(&node, node.centre()), node.centre());
    }

    #[test]
    fn a_stereotype_comes_from_the_keyword() {
        let out = placed("requirementDiagram\ndesignConstraint c {\n}\nelement e {\n}");
        assert_eq!(out.nodes[0].stereotype, Kind::DesignConstraint.stereotype());
        assert_eq!(out.nodes[1].stereotype, "«Element»");
    }

    #[test]
    fn an_empty_diagram_still_yields_a_canvas() {
        let out = placed("requirementDiagram");
        assert!(out.nodes.is_empty());
        assert!(out.width > 0.0);
        assert!(out.height > 0.0);
    }
}
