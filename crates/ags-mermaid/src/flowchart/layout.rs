//! Sizing the boxes, handing the graph to the engine, and tidying what comes
//! back.
//!
//! Two things happen after the engine has run. An edge is clipped to the outline
//! it points at rather than to the bounding box, because a diamond's outline
//! sits well inside its box and an arrowhead stopping at the box floats in the
//! gap. And an edge's label is placed *beside* the run it belongs to, clear of
//! the boxes and of every wire but its own.

use crate::label::Placed as PlacedLabel;
use crate::layout;
use crate::metrics::text_width;
use crate::round::count;
use crate::scene::Point;

use super::clip::clip_ends;
use super::config::Config;
use super::frames::{group_boxes, make_room};
use super::label::{elsewhere, label_at};
use super::types::{EdgeStyle, Graph, Shape};

// Every measurement now lives in `Config`, so a caller can move one without
// editing this file. See `config.rs`.

/// One box, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    pub classes: Vec<String>,
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

impl PlacedNode {
    pub fn centre(&self) -> Point {
        Point::new(self.at.x + self.width / 2.0, self.at.y + self.height / 2.0)
    }
}

/// One arrow, routed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEdge {
    pub source: String,
    pub target: String,
    pub label: String,
    pub style: EdgeStyle,
    pub head_start: bool,
    pub head_end: bool,
    pub points: Vec<Point>,
    /// Where the label sits and how much room it takes, when there is one.
    pub label_at: Option<PlacedLabel>,
}

/// One `subgraph`, as the box drawn round what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGroup {
    pub id: String,
    pub label: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    /// How many groups enclose this one. Outermost is nought.
    pub depth: usize,
    /// Every node this frame is drawn round, including those in the groups
    /// nested inside it. What the source said it holds, so a checker can ask
    /// whether the drawing agrees.
    pub holds: Vec<String>,
}

/// A laid-out flowchart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<PlacedEdge>,
    pub groups: Vec<PlacedGroup>,
}

/// The engine's coordinates in the scene's own type.
///
/// Two `Point`s exist on purpose: the engine has no business knowing what a
/// scene is, and the scene has no business knowing what a layout pass is. This
/// is the one seam between them.
const fn at(point: layout::Point) -> Point {
    Point::new(point.x, point.y)
}

/// How wide and tall a label is, over however many lines it takes.
pub(super) fn label_size(label: &str, font: f64, weight: u32, cfg: &Config) -> (f64, f64) {
    let lines: Vec<&str> = label.split('\n').collect();
    let width = lines
        .iter()
        .map(|line| text_width(line, font, weight))
        .fold(0.0, f64::max);
    (width, count(lines.len().max(1)) * font * cfg.line_height)
}

/// How big a box has to be to hold its label.
///
/// Every rule here is the reference's. A shape whose outline cuts corners off
/// its box needs the room back, which is why so many of them add to the width.
pub fn measure(label: &str, shape: Shape, cfg: &Config) -> layout::Node {
    if matches!(shape, Shape::StateStart | Shape::StateEnd) {
        return layout::Node::new(cfg.marker_size, cfg.marker_size);
    }
    let (text_w, text_h) = label_size(label, cfg.label_font, cfg.label_weight, cfg);
    let mut width = text_w + cfg.pad_x * 2.0;
    let mut height = text_h + cfg.pad_y * 2.0;
    match shape {
        Shape::Diamond => {
            let side = width.max(height) + cfg.diamond_extra;
            width = side;
            height = side;
        }
        Shape::Circle | Shape::DoubleCircle => {
            // Wide enough that the text fits across the circle rather than
            // across its box.
            let across = width.hypot(height).ceil() + 8.0;
            width = if shape == Shape::DoubleCircle {
                across + 12.0
            } else {
                across
            };
            height = width;
        }
        Shape::Hexagon | Shape::Trapezoid | Shape::TrapezoidAlt => width += cfg.pad_x,
        Shape::Asymmetric => width += 12.0,
        Shape::Cylinder => height += 14.0,
        _ => {}
    }
    layout::Node::new(width.max(cfg.min_width), height.max(cfg.min_height))
}

/// Lay out a parsed flowchart.
pub fn layout(graph: &Graph, cfg: &Config) -> Placed {
    if graph.nodes.is_empty() {
        return Placed::default();
    }
    // Each subgraph is laid out as a unit and placed in its parent as one box,
    // which is what keeps a frame from enclosing a stranger — see `nest`.
    let placed = super::nest::layout(graph, cfg);

    let nodes: Vec<PlacedNode> = graph
        .nodes
        .iter()
        .zip(&placed.nodes)
        .map(|(node, placed)| PlacedNode {
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape,
            classes: node.classes.clone(),
            at: at(placed.at),
            width: placed.width,
            height: placed.height,
        })
        .collect();

    // Every route first, clipped, because placing a label needs to know where the
    // other wires run — not just this edge's own.
    let routes: Vec<Vec<Point>> = graph
        .edges
        .iter()
        .zip(&placed.edges)
        .map(|(edge, route)| {
            let mut points: Vec<Point> = route.points.iter().copied().map(at).collect();
            let source = graph.index_of(&edge.source).and_then(|at| nodes.get(at));
            let target = graph.index_of(&edge.target).and_then(|at| nodes.get(at));
            if let (Some(source), Some(target)) = (source, target) {
                clip_ends(&mut points, source, target);
            }
            points
        })
        .collect();

    // Seeded with the boxes, because a label pushed off a wire and into a box has
    // not been helped. It grows as labels are placed, so each keeps out of the way
    // of the ones before it.
    let mut taken: Vec<PlacedLabel> = nodes
        .iter()
        .map(|node| PlacedLabel::new(node.centre(), node.width, node.height))
        .collect();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let points = routes.get(index).cloned().unwrap_or_default();
            let placed_label = (!edge.label.is_empty())
                .then(|| {
                    label_at(
                        &points,
                        &edge.label,
                        cfg,
                        &taken,
                        &elsewhere(&routes, index),
                    )
                })
                .flatten();
            if let Some(label) = placed_label {
                taken.push(label);
            }
            PlacedEdge {
                source: edge.source.clone(),
                target: edge.target.clone(),
                label: edge.label.clone(),
                style: edge.style,
                head_start: edge.head_start,
                head_end: edge.head_end,
                label_at: placed_label,
                points,
            }
        })
        .collect();

    let groups = group_boxes(graph, &nodes, cfg);
    let mut out = Placed {
        width: placed.width,
        height: placed.height,
        nodes,
        edges,
        groups,
    };
    make_room(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::super::clip::clip;
    use super::super::label::LABEL_GAP;
    use super::*;
    use crate::flowchart::parse;
    use crate::label::runs;

    fn placed(source: &str) -> Placed {
        layout(&parse(source), &Config::default())
    }

    #[test]
    fn a_box_is_big_enough_for_its_label() {
        let narrow = measure("A", Shape::Rectangle, &Config::default());
        let wide = measure(
            "A much longer label than that",
            Shape::Rectangle,
            &Config::default(),
        );
        assert!(wide.width > narrow.width);
        assert!((narrow.width - Config::default().min_width).abs() < 1e-9);
        assert!(narrow.height >= Config::default().min_height);
    }

    #[test]
    fn a_label_over_two_lines_makes_a_taller_box() {
        let one = measure("one", Shape::Rectangle, &Config::default());
        let two = measure("one\ntwo", Shape::Rectangle, &Config::default());
        assert!(two.height > one.height);
    }

    #[test]
    fn a_diamond_is_square_and_larger_than_the_text_asks_for() {
        let diamond = measure("Ready?", Shape::Diamond, &Config::default());
        assert!((diamond.width - diamond.height).abs() < 1e-9);
        assert!(diamond.width > measure("Ready?", Shape::Rectangle, &Config::default()).width);
    }

    #[test]
    fn a_circle_is_wide_enough_to_hold_its_text_across_the_round() {
        let circle = measure("Go", Shape::Circle, &Config::default());
        assert!((circle.width - circle.height).abs() < 1e-9);
        let double = measure("Go", Shape::DoubleCircle, &Config::default());
        assert!(double.width > circle.width, "the outer ring needs room");
    }

    #[test]
    fn a_shape_that_cuts_its_corners_gets_the_room_back() {
        let plain = measure("Wait", Shape::Rectangle, &Config::default());
        for shape in [Shape::Hexagon, Shape::Trapezoid, Shape::TrapezoidAlt] {
            assert!(
                measure("Wait", shape, &Config::default()).width > plain.width,
                "{shape:?}"
            );
        }
        assert!(measure("Wait", Shape::Asymmetric, &Config::default()).width > plain.width);
        assert!(measure("Wait", Shape::Cylinder, &Config::default()).height > plain.height);
    }

    #[test]
    fn a_state_marker_is_a_fixed_dot() {
        assert_eq!(
            measure("anything at all", Shape::StateStart, &Config::default()),
            layout::Node::new(Config::default().marker_size, Config::default().marker_size)
        );
        assert_eq!(
            measure("", Shape::StateEnd, &Config::default()),
            layout::Node::new(Config::default().marker_size, Config::default().marker_size)
        );
    }

    #[test]
    fn a_chain_runs_the_way_the_header_says() {
        let downward = placed("graph TD\nA[Start] --> B[Process] --> C[End]");
        assert_eq!(downward.nodes.len(), 3);
        assert!(downward.nodes[1].at.y > downward.nodes[0].at.y);
        let rightward = placed("graph LR\nA[Start] --> B[Process] --> C[End]");
        assert!(rightward.nodes[1].at.x > rightward.nodes[0].at.x);
    }

    #[test]
    fn every_box_sits_inside_the_canvas() {
        let out = placed("graph TD\nA --> B\nA --> C\nB --> D\nC --> D\nA --> D");
        for node in &out.nodes {
            assert!(node.at.x >= -1e-6 && node.at.y >= -1e-6, "{node:?}");
            assert!(node.at.x + node.width <= out.width + 1e-6, "{node:?}");
            assert!(node.at.y + node.height <= out.height + 1e-6, "{node:?}");
        }
    }

    #[test]
    fn an_arrow_stops_on_the_outline_of_a_diamond_rather_than_its_box() {
        let diamond = PlacedNode {
            id: "B".into(),
            label: String::new(),
            shape: Shape::Diamond,
            classes: Vec::new(),
            at: Point::new(0.0, 0.0),
            width: 100.0,
            height: 100.0,
        };
        // Arriving straight down a quarter of the way in from the left. The
        // box's top edge is at zero, but the outline there is a quarter of the
        // way down the diamond's upper-left side.
        let hit = clip(Point::new(25.0, 0.0), Point::new(25.0, -50.0), &diamond);
        assert!((hit.x - 25.0).abs() < 1e-9, "{hit:?}");
        assert!((hit.y - 25.0).abs() < 1e-9, "{hit:?}");
        // Down the middle it is the box's top, because that is where the corner
        // of a diamond is.
        let centred = clip(Point::new(50.0, 0.0), Point::new(50.0, -50.0), &diamond);
        assert!((centred.y - 0.0).abs() < 1e-9, "{centred:?}");
    }

    #[test]
    fn an_arrow_stops_on_the_outline_of_every_shape_that_cuts_its_box() {
        for shape in [
            Shape::Hexagon,
            Shape::Asymmetric,
            Shape::Trapezoid,
            Shape::TrapezoidAlt,
        ] {
            let node = PlacedNode {
                id: "B".into(),
                label: String::new(),
                shape,
                classes: Vec::new(),
                at: Point::new(0.0, 0.0),
                width: 100.0,
                height: 100.0,
            };
            // Arriving from the left, along the middle.
            let hit = clip(Point::new(0.0, 50.0), Point::new(-50.0, 50.0), &node);
            assert!(hit.x >= 0.0 && hit.x <= 100.0, "{shape:?} {hit:?}");
            assert!((hit.y - 50.0).abs() < 1e-9, "{shape:?} {hit:?}");
        }
    }

    #[test]
    fn an_arrow_stops_on_the_edge_of_a_circle() {
        let out = placed("graph TD\nA[Start] --> B((Go))");
        let circle = out.nodes.iter().find(|n| n.id == "B").expect("a circle");
        let end = out.edges[0].points.last().copied().unwrap_or_default();
        let away = (end.x - circle.centre().x).hypot(end.y - circle.centre().y);
        let radius = circle.width.min(circle.height) / 2.0;
        assert!((away - radius).abs() < 1.0, "{away} against {radius}");
    }

    #[test]
    fn an_arrow_into_a_plain_box_stops_at_the_box() {
        let out = placed("graph TD\nA[Start] --> B[Process]");
        let target = out.nodes.iter().find(|n| n.id == "B").expect("a box");
        let end = out.edges[0].points.last().copied().unwrap_or_default();
        assert!((end.y - target.at.y).abs() < 1e-6, "{end:?}");
    }

    fn shaped(shape: Shape) -> PlacedNode {
        PlacedNode {
            id: "B".into(),
            label: String::new(),
            shape,
            classes: Vec::new(),
            at: Point::new(0.0, 0.0),
            width: 100.0,
            height: 100.0,
        }
    }

    #[test]
    fn an_arrow_meets_a_circle_from_whichever_side_it_arrives() {
        let circle = shaped(Shape::Circle);
        // Along the middle, from the left: the outline is the leftmost point.
        let across = clip(Point::new(0.0, 50.0), Point::new(-50.0, 50.0), &circle);
        assert!((across.x - 0.0).abs() < 1e-9, "{across:?}");
        // From the right, it stops on the other side.
        let back = clip(Point::new(100.0, 50.0), Point::new(150.0, 50.0), &circle);
        assert!((back.x - 100.0).abs() < 1e-9, "{back:?}");
        // From below, on the underside.
        let under = clip(Point::new(50.0, 100.0), Point::new(50.0, 150.0), &circle);
        assert!((under.y - 100.0).abs() < 1e-9, "{under:?}");
    }

    #[test]
    fn an_arrow_dropping_onto_a_circle_stops_on_its_upper_arc() {
        // The other half of the vertical case: arriving from above rather than
        // below, so the near side is the top of the arc and not the bottom.
        let circle = shaped(Shape::Circle);
        let onto = clip(Point::new(50.0, 0.0), Point::new(50.0, -50.0), &circle);
        assert!((onto.y - 0.0).abs() < 1e-9, "{onto:?}");
        // Off the axis, it still lands on the arc rather than on the box.
        let off = clip(Point::new(70.0, 0.0), Point::new(70.0, -50.0), &circle);
        assert!(off.y > 0.0 && off.y < 50.0, "on the arc: {off:?}");
    }

    #[test]
    fn an_arrow_stops_at_the_nearest_face_it_crosses_not_the_far_one() {
        // A ray through a diamond crosses two of its sides. The one to stop on is
        // the one nearer where the edge came from; the far one is where it would
        // end up after passing straight through.
        let diamond = shaped(Shape::Diamond);
        let from_above = clip(Point::new(50.0, 50.0), Point::new(50.0, -50.0), &diamond);
        assert!((from_above.y - 0.0).abs() < 1e-9, "{from_above:?}");
        let from_below = clip(Point::new(50.0, 50.0), Point::new(50.0, 150.0), &diamond);
        assert!((from_below.y - 100.0).abs() < 1e-9, "{from_below:?}");
    }

    #[test]
    fn an_arrow_that_misses_a_circle_is_left_where_it_was() {
        let circle = shaped(Shape::Circle);
        // Further out than the radius, so there is nothing to stop on.
        let wide = Point::new(400.0, 0.0);
        assert_eq!(clip(wide, Point::new(400.0, -50.0), &circle), wide);
        let high = Point::new(0.0, 400.0);
        assert_eq!(clip(high, Point::new(-50.0, 400.0), &circle), high);
    }

    #[test]
    fn an_arrow_into_a_shape_that_fills_its_box_is_left_where_it_was() {
        let plain = shaped(Shape::Rectangle);
        let at = Point::new(50.0, 0.0);
        assert_eq!(clip(at, Point::new(50.0, -50.0), &plain), at);
    }

    #[test]
    fn an_arrow_that_never_reaches_an_outline_is_left_where_it_was() {
        // A ray beside the diamond entirely: nothing crosses it.
        let diamond = shaped(Shape::Diamond);
        let beside = Point::new(400.0, 0.0);
        assert_eq!(clip(beside, Point::new(400.0, -50.0), &diamond), beside);
    }

    #[test]
    fn a_run_of_fewer_than_two_points_is_left_alone() {
        let mut lone = [Point::new(1.0, 2.0)];
        clip_ends(&mut lone, &shaped(Shape::Diamond), &shaped(Shape::Diamond));
        assert_eq!(lone, [Point::new(1.0, 2.0)]);
        let mut none: [Point; 0] = [];
        clip_ends(&mut none, &shaped(Shape::Diamond), &shaped(Shape::Diamond));
        assert!(none.is_empty());
    }

    #[test]
    fn both_ends_of_a_run_are_moved_onto_their_outlines() {
        let diamond = shaped(Shape::Diamond);
        let mut run = [
            Point::new(25.0, 100.0),
            Point::new(25.0, 150.0),
            Point::new(25.0, 200.0),
        ];
        let mut far = shaped(Shape::Diamond);
        far.at = Point::new(0.0, 200.0);
        clip_ends(&mut run, &diamond, &far);
        // The source's underside at a quarter in, and the target's upper side.
        assert!((run[0].y - 75.0).abs() < 1e-9, "{run:?}");
        assert!((run[2].y - 225.0).abs() < 1e-9, "{run:?}");
    }

    #[test]
    fn an_edge_with_a_label_says_where_it_goes() {
        let out = placed("graph TD\nA --> |Yes| B");
        let edge = &out.edges[0];
        assert_eq!(edge.label, "Yes");
        let label = edge.label_at.expect("somewhere to put it");
        assert!(
            label.at.y > out.nodes[0].at.y && label.at.y < out.nodes[1].at.y + out.nodes[1].height
        );
    }

    #[test]
    fn an_edge_with_no_label_has_nowhere_to_put_one() {
        assert_eq!(placed("graph TD\nA --> B").edges[0].label_at, None);
    }

    #[test]
    fn an_edge_naming_a_box_that_does_not_exist_is_not_drawn() {
        // The parser declares every name it sees, so this is built by hand.
        let mut graph = parse("graph TD\nA --> B");
        graph.edges.push(super::super::types::Edge {
            source: "A".into(),
            target: "nobody".into(),
            label: String::new(),
            style: EdgeStyle::Solid,
            head_start: false,
            head_end: true,
        });
        let out = layout(&graph, &Config::default());
        assert!(out.edges[1].points.is_empty());
    }

    #[test]
    fn a_label_steps_above_a_level_run_and_aside_from_an_upright_one() {
        let cfg = Config::default();
        let level = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(100.0, 10.0),
            Point::new(100.0, 20.0),
        ];
        // The long run is the horizontal one, so the label steps up off it —
        // same x as the middle of that run, clear of the wire in y.
        let across = label_at(&level, "Yes", &cfg, &[], &[]).expect("a placement");
        assert!((across.at.x - 50.0).abs() < 1e-9, "{across:?}");
        assert!(across.at.y < 10.0, "it must clear the wire: {across:?}");

        let upright = [Point::new(0.0, 0.0), Point::new(0.0, 100.0)];
        let down = label_at(&upright, "Yes", &cfg, &[], &[]).expect("a placement");
        assert!((down.at.y - 50.0).abs() < 1e-9, "{down:?}");
        assert!(
            down.at.x - down.width / 2.0 >= LABEL_GAP - 1e-9,
            "the whole word must be off the wire: {down:?}"
        );
    }

    #[test]
    fn a_route_with_no_run_has_nowhere_to_put_a_label() {
        let cfg = Config::default();
        assert_eq!(label_at(&[], "Yes", &cfg, &[], &[]), None);
        assert_eq!(
            label_at(&[Point::new(1.0, 1.0)], "Yes", &cfg, &[], &[]),
            None
        );
    }

    #[test]
    fn a_label_keeps_off_a_wire_that_is_not_its_own() {
        let cfg = Config::default();
        let mine = [Point::new(0.0, 0.0), Point::new(0.0, 100.0)];
        let clear = label_at(&mine, "Yes", &cfg, &[], &[]).expect("a placement");
        // A second wire running exactly where that label landed has to move it.
        let blocked = label_at(
            &mine,
            "Yes",
            &cfg,
            &[],
            &[(Point::new(clear.at.x, 0.0), Point::new(clear.at.x, 100.0))],
        )
        .expect("a placement");
        assert_ne!(clear.at, blocked.at, "the label must give way: {blocked:?}");
    }

    #[test]
    fn every_run_but_the_labelled_one_is_what_a_label_avoids() {
        let routes = vec![
            vec![Point::new(0.0, 0.0), Point::new(0.0, 10.0)],
            vec![Point::new(5.0, 0.0), Point::new(5.0, 10.0)],
        ];
        assert_eq!(elsewhere(&routes, 0), runs(&routes[1]));
        assert_eq!(elsewhere(&routes, 1), runs(&routes[0]));
    }

    #[test]
    fn the_drawing_grows_to_hold_a_label_that_reaches_past_its_edge() {
        // A label beside the riser between two stacked boxes lands outside the
        // width the engine returned, because the engine only sized the boxes.
        let out = placed("graph TD\nA -->|a long edge label| B");
        let label = out.edges[0].label_at.expect("a placement");
        assert!(out.width >= label.at.x + label.width / 2.0, "{out:?}");
        assert!(out.height >= label.at.y + label.height / 2.0, "{out:?}");
    }

    #[test]
    fn a_group_is_drawn_round_what_it_holds() {
        let out = placed(
            "graph TD\nsubgraph Frontend\nA --> B\nend\nsubgraph Backend\nC --> D\nend\nB --> C",
        );
        assert_eq!(out.groups.len(), 2);
        let front = out
            .groups
            .iter()
            .find(|group| group.id == "Frontend")
            .expect("a box");
        for id in ["A", "B"] {
            let node = out.nodes.iter().find(|node| node.id == id).expect(id);
            assert!(node.at.x >= front.at.x, "{id}");
            assert!(
                node.at.y >= front.at.y + Config::default().group_header,
                "{id} clears the band"
            );
            assert!(node.at.x + node.width <= front.at.x + front.width, "{id}");
            assert!(node.at.y + node.height <= front.at.y + front.height, "{id}");
        }
        assert_eq!(front.label, "Frontend");
        assert_eq!(front.depth, 0);
    }

    #[test]
    fn a_nested_group_sits_inside_the_one_round_it() {
        let out = placed("graph TD\nsubgraph Outer\nsubgraph Inner\nA --> B\nend\nend");
        let inner = out.groups.iter().find(|g| g.id == "Inner").expect("inner");
        let outer = out.groups.iter().find(|g| g.id == "Outer").expect("outer");
        assert_eq!(outer.depth, 0);
        assert_eq!(inner.depth, 1);
        assert!(inner.at.x >= outer.at.x && inner.at.y >= outer.at.y);
        // Outermost first, so a nested box paints over the one round it.
        assert!(out.groups[0].depth <= out.groups[1].depth);
    }

    #[test]
    fn a_group_box_reaching_off_the_page_makes_room_for_itself() {
        let out = placed("graph TD\nsubgraph Only\nA --> B\nend");
        assert!(out.groups[0].at.x >= -1e-9);
        assert!(out.groups[0].at.y >= -1e-9);
        for node in &out.nodes {
            assert!(node.at.x >= 0.0 && node.at.y >= 0.0, "{node:?}");
        }
        let group = &out.groups[0];
        assert!(group.at.x + group.width <= out.width + 1e-6);
        assert!(group.at.y + group.height <= out.height + 1e-6);
    }

    #[test]
    fn a_group_holding_nothing_is_not_drawn() {
        let out = placed("graph TD\nA --> B\nsubgraph Empty\nend");
        assert!(out.groups.is_empty());
    }

    #[test]
    fn the_same_source_lays_out_the_same_way_twice() {
        let source = "graph TD\nA --> B\nA --> C\nB --> D\nC --> D";
        assert_eq!(placed(source), placed(source));
    }

    #[test]
    fn a_source_of_nothing_lays_out_to_nothing() {
        assert_eq!(placed("graph TD"), Placed::default());
    }
}
