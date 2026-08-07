//! What can go wrong with a wire: where it leaves from, what it passes through,
//! and whether a reader can tell it from the one beside it.

use super::report::Violation;
use super::scene::{
    bounds, boxes, centre, faces_away, run_hits, runs, runs_cross, shared_length, Marked, Rect,
    BACKTRACK_TOLERANCE, MERGE_MIN_LENGTH,
};
use crate::scene::{Content, Node, Point, Role};

/// Edges crossing a box's interior rather than stopping at its border.
///
/// Only edges that declare an endpoint are checked. The rule's own sentence —
/// *passes through a box it does not connect* — presupposes the edge connects
/// something, and [`Marked::joins`] can only clear a crossing it is allowed to
/// make by matching `from`/`to` against the box. A stroke with neither is not an
/// edge in that sense: it is a chart series, an axis, a timeline spine, a bracket.
/// It joins nothing, so every box it crosses is "not connected" and every crossing
/// is reported.
///
/// This was measured, not guessed. Over `examples/diagram-gallery.md` the rule
/// produced 286 crossings and **not one of them named its edge** — every message
/// read "the edge something passes through something", because an id-less stroke
/// is exactly the kind that has no endpoints either. Seven of the fifteen
/// offending diagrams were `xychart`, where a line series crosses the bars drawn
/// under it, which is the chart working correctly.
pub(super) fn edges_through_nodes(nodes: &[Marked<'_>]) -> Vec<Violation> {
    let boxes: Vec<(&Marked<'_>, Rect)> = nodes
        .iter()
        .filter(|held| held.node.role == Role::Node)
        .filter_map(|held| match &held.node.content {
            Content::Shape(shape) => bounds(shape).map(|area| (held, area)),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for edge in nodes
        .iter()
        .filter(|held| held.is_route())
        .filter(|held| held.connects())
    {
        for (held, rect) in &boxes {
            // An edge legitimately touches the two boxes it connects.
            if edge.joins(held.id.as_ref()) || edge.id.is_some() && edge.id == held.id {
                continue;
            }
            // And a box drawn *on* something says whose it is: a sequence
            // activation is a bar on its actor's own lifeline, so a message
            // reaching that actor is arriving where it was always going rather
            // than cutting through a stranger. Without this the notation reports
            // itself — every activated message in the gallery was a violation.
            if held.owned_by(edge) {
                continue;
            }
            if runs(edge.node).iter().any(|run| run_hits(*run, *rect)) {
                out.push(Violation::EdgeThroughNode {
                    edge: edge.name(),
                    node: held.id.clone(),
                });
            }
        }
    }
    out
}

/// Whether two routes set off from the very same point, so their first runs are
/// one trunk rather than two wires.
///
/// A tree view draws its children off a single stem: every connector under one
/// folder starts at the same point below its glyph and peels off at its own row.
/// The stem is shared by design and reads as a stem — asking which wire is which
/// there is asking the wrong question, because they are not two wires yet.
///
/// The test is deliberately this exact rather than "do the two edges share a
/// box". Two edges leaving one node's face are pushed apart by `route::spread`
/// precisely so they do not run together, and excusing every pair with a node in
/// common would disarm the rule that catches it when that spreading fails —
/// which is a live defect elsewhere in this same gallery. Only routes departing
/// the identical point are a trunk.
fn same_departure(a: (Point, Point), b: (Point, Point)) -> bool {
    (a.0.x - b.0.x).abs() < 0.5 && (a.0.y - b.0.y).abs() < 0.5
}

/// The longest stretch two routes are drawn on top of each other, discounting a
/// trunk they legitimately share.
fn merged_length(a: &Node, b: &Node) -> f64 {
    let (left, right) = (runs(a), runs(b));
    let trunk = left
        .first()
        .zip(right.first())
        .is_some_and(|(x, y)| same_departure(*x, *y));
    let mut longest = 0.0_f64;
    for (i, x) in left.iter().enumerate() {
        for (j, y) in right.iter().enumerate() {
            if trunk && i == 0 && j == 0 {
                continue;
            }
            longest = longest.max(shared_length(*x, *y));
        }
    }
    longest
}

/// Pairs of edges drawn along the same line, which read as a single wire.
///
/// Endpoint-bearing edges only, for the same reason [`edges_through_nodes`] checks
/// them: the defect is that a reader cannot tell *which connection is which* when
/// two are drawn on top of each other. Two strokes that connect nothing carry no
/// connection to confuse — a chart drawing a line series over the bars of the same
/// data has drawn what it was asked to, and 21 of the gallery's remaining 32
/// findings were that one shape.
pub(super) fn merged_edges(nodes: &[Marked<'_>]) -> Vec<Violation> {
    let edges: Vec<&Marked<'_>> = nodes
        .iter()
        .filter(|held| held.is_route())
        .filter(|held| held.connects())
        .collect();
    let mut out = Vec::new();
    for (i, a) in edges.iter().enumerate() {
        for b in edges.iter().skip(i + 1) {
            let overlap = merged_length(a.node, b.node);
            if overlap > MERGE_MIN_LENGTH {
                out.push(Violation::MergedEdges {
                    a: a.name(),
                    b: b.name(),
                    length: overlap,
                });
            }
        }
    }
    out
}

/// Edges that leave or arrive on the face pointing away from the other end.
pub(super) fn wrong_faces(nodes: &[Marked<'_>]) -> Vec<Violation> {
    let boxes: Vec<(String, Rect)> = boxes(nodes);
    let find = |id: &Option<String>| {
        let id = id.as_ref()?;
        boxes.iter().find(|(name, _)| name == id).map(|(_, r)| *r)
    };
    let mut out = Vec::new();
    for edge in nodes.iter().filter(|h| h.is_route()) {
        let runs = runs(edge.node);
        let (Some(first), Some(last)) = (runs.first(), runs.last()) else {
            continue;
        };
        let (Some(from), Some(to)) = (find(&edge.from), find(&edge.to)) else {
            continue;
        };
        // Each end judged against the other: where the route leaves `from`, and
        // where it arrives at `to`.
        for (point, leaving, toward, id) in [
            (first.0, from, to, &edge.from),
            (last.1, to, from, &edge.to),
        ] {
            if faces_away(point, leaving, toward) {
                out.push(Violation::WrongFace {
                    edge: edge.name(),
                    node: id.clone(),
                });
            }
        }
    }
    out
}

/// Routes that travel away from their target before turning back.
///
/// Measured as travel projected against the straight line between the two boxes:
/// a corner contributes nothing, because moving across the line is neither towards
/// nor away. Only motion in the opposite direction counts, which is what a detour
/// is made of.
pub(super) fn backtracking(nodes: &[Marked<'_>]) -> Vec<Violation> {
    let boxes: Vec<(String, Rect)> = boxes(nodes);
    let find = |id: &Option<String>| {
        let id = id.as_ref()?;
        boxes.iter().find(|(name, _)| name == id).map(|(_, r)| *r)
    };
    let mut out = Vec::new();
    for edge in nodes.iter().filter(|h| h.is_route()) {
        let (Some(from), Some(to)) = (find(&edge.from), find(&edge.to)) else {
            continue;
        };
        let (a, b) = (centre(from), centre(to));
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let span = dx.hypot(dy);
        if span < 1.0 {
            continue;
        }
        let (ux, uy) = (dx / span, dy / span);
        let back: f64 = runs(edge.node)
            .iter()
            .map(|(p, q)| -((q.x - p.x) * ux + (q.y - p.y) * uy))
            .filter(|projected| *projected > 0.0)
            .sum();
        if back > BACKTRACK_TOLERANCE {
            out.push(Violation::Backtracks {
                edge: edge.name(),
                by: back,
            });
        }
    }
    out
}

/// Pairs of edges that cross one another.
///
/// Edges sharing a box are skipped: lines converging on the same node meet near it
/// by construction, and reporting that would report the diagram for being connected.
pub(super) fn crossing_edges(nodes: &[Marked<'_>]) -> Vec<Violation> {
    let edges: Vec<&Marked<'_>> = nodes
        .iter()
        .filter(|h| h.is_route())
        .filter(|h| h.connects())
        .collect();
    let mut out = Vec::new();
    for (i, a) in edges.iter().enumerate() {
        for b in edges.iter().skip(i + 1) {
            if a.shares_a_box_with(b) {
                continue;
            }
            if runs(a.node)
                .iter()
                .any(|ra| runs(b.node).iter().any(|rb| runs_cross(*ra, *rb)))
            {
                out.push(Violation::EdgesCross {
                    a: a.name(),
                    b: b.name(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::scene::{closed_shape, PLACEHOLDER};
    use super::*;
    use crate::constraint::check;
    use crate::constraint::fixture::*;
    use crate::scene::Shape;

    #[test]
    fn an_edge_crossing_an_unrelated_box_is_reported() {
        let mut s = canvas();
        s.push(box_at("mid", 60.0, 10.0, 40.0, 30.0));
        s.push(wire(
            "e",
            vec![Point::new(10.0, 25.0), Point::new(150.0, 25.0)],
        ));
        assert_eq!(
            check(&s),
            vec![Violation::EdgeThroughNode {
                edge: Some("e".into()),
                node: Some("mid".into()),
            }]
        );
    }

    #[test]
    fn leaving_by_the_face_pointing_away_is_reported() {
        // `b` is to the right, and the route leaves `a` by its left edge. These
        // rules found nothing over the whole reference gallery, which is only
        // reassuring if they can fire at all — so this constructs the defect.
        let mut s = two_boxes();
        s.push(wire_between(
            "e",
            "a",
            "b",
            vec![
                Point::new(10.0, 40.0),
                Point::new(0.0, 40.0),
                Point::new(0.0, 10.0),
                Point::new(160.0, 10.0),
                Point::new(160.0, 30.0),
            ],
        ));
        assert!(
            check(&s).contains(&Violation::WrongFace {
                edge: Some("e".into()),
                node: Some("a".into()),
            }),
            "{:?}",
            check(&s)
        );
    }

    #[test]
    fn leaving_by_a_side_face_toward_the_target_is_not_reported() {
        // The exemption that keeps this from flagging most of every lattice route:
        // leaving downward to reach something to the right is ordinary.
        let mut s = two_boxes();
        s.push(wire_between(
            "e",
            "a",
            "b",
            vec![
                Point::new(30.0, 50.0),
                Point::new(30.0, 80.0),
                Point::new(160.0, 80.0),
                Point::new(160.0, 50.0),
            ],
        ));
        assert!(
            !check(&s)
                .iter()
                .any(|v| matches!(v, Violation::WrongFace { .. })),
            "{:?}",
            check(&s)
        );
    }

    #[test]
    fn closure_tells_an_area_from_a_line() {
        use crate::scene::Seg;
        let open = Content::Shape(Shape::Polyline(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
        ]));
        assert!(!closed_shape(&open));
        assert!(closed_shape(&Content::Shape(Shape::Polygon(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
        ]))));
        assert!(closed_shape(&Content::Shape(Shape::Path(vec![
            Seg::MoveTo(Point::new(0.0, 0.0)),
            Seg::LineTo(Point::new(10.0, 0.0)),
            Seg::Close,
        ]))));
        assert!(!closed_shape(&Content::Shape(Shape::Path(vec![
            Seg::MoveTo(Point::new(0.0, 0.0)),
            Seg::LineTo(Point::new(10.0, 0.0)),
        ]))));
        // Anything that is not a shape at all — a group, a text run.
        assert!(!closed_shape(&Content::Group(Vec::new())));
    }

    #[test]
    fn a_route_that_doubles_back_is_reported_with_how_far() {
        let mut s = two_boxes();
        s.push(wire_between(
            "e",
            "a",
            "b",
            vec![
                Point::new(50.0, 40.0),
                Point::new(0.0, 40.0), // 50px the wrong way
                Point::new(0.0, 90.0),
                Point::new(140.0, 90.0),
                Point::new(140.0, 40.0),
            ],
        ));
        let found = check(&s);
        let Some(Violation::Backtracks { by, .. }) = found
            .iter()
            .find(|v| matches!(v, Violation::Backtracks { .. }))
        else {
            panic!("expected a backtrack, got {found:?}");
        };
        assert!((*by - 50.0).abs() < 1.0, "travelled {by}px backwards");
    }

    #[test]
    fn an_ordinary_dog_leg_is_not_backtracking() {
        // Moving across the line between the boxes is neither towards nor away, so
        // a corner costs nothing — otherwise every orthogonal route would report.
        let mut s = two_boxes();
        s.push(wire_between(
            "e",
            "a",
            "b",
            vec![
                Point::new(50.0, 40.0),
                Point::new(95.0, 40.0),
                Point::new(95.0, 80.0),
                Point::new(140.0, 80.0),
            ],
        ));
        assert!(
            !check(&s)
                .iter()
                .any(|v| matches!(v, Violation::Backtracks { .. })),
            "{:?}",
            check(&s)
        );
    }

    #[test]
    fn two_edges_that_cross_are_reported() {
        let mut s = canvas();
        for (id, x) in [("a", 10.0), ("b", 150.0), ("c", 10.0), ("d", 150.0)] {
            let y = if id == "a" || id == "b" { 10.0 } else { 70.0 };
            s.push(box_at(id, x, y, 30.0, 20.0));
        }
        // a→d and c→b, drawn so they must cross in the middle.
        s.push(wire_between(
            "e1",
            "a",
            "d",
            vec![Point::new(80.0, 20.0), Point::new(80.0, 80.0)],
        ));
        s.push(wire_between(
            "e2",
            "c",
            "b",
            vec![Point::new(60.0, 50.0), Point::new(120.0, 50.0)],
        ));
        assert!(
            check(&s).contains(&Violation::EdgesCross {
                a: Some("e1".into()),
                b: Some("e2".into()),
            }),
            "{:?}",
            check(&s)
        );
    }

    #[test]
    fn edges_meeting_at_a_shared_box_do_not_count_as_crossing() {
        // Lines converging on one node touch near it by construction; reporting
        // that would report the diagram for being connected.
        let mut s = two_boxes();
        s.push(box_at("c", 80.0, 70.0, 30.0, 20.0));
        s.push(wire_between(
            "e1",
            "a",
            "b",
            vec![Point::new(95.0, 20.0), Point::new(95.0, 85.0)],
        ));
        s.push(wire_between(
            "e2",
            "a",
            "c",
            vec![Point::new(60.0, 80.0), Point::new(130.0, 80.0)],
        ));
        assert!(
            !check(&s)
                .iter()
                .any(|v| matches!(v, Violation::EdgesCross { .. })),
            "{:?}",
            check(&s)
        );
    }

    #[test]
    fn a_closed_band_is_an_area_and_answers_no_route_questions() {
        // A sankey link: an edge with endpoints, drawn as a filled ribbon that runs
        // out along one side and back along the other. It "returns to its target"
        // because it is a closed shape, not because the drawing is wrong — all six
        // backtracking findings over the reference gallery were one such diagram.
        let mut s = two_boxes();
        s.push(
            Node::new(
                Role::Edge,
                Content::Shape(Shape::Path(vec![
                    crate::scene::Seg::MoveTo(Point::new(50.0, 35.0)),
                    crate::scene::Seg::LineTo(Point::new(140.0, 35.0)),
                    crate::scene::Seg::LineTo(Point::new(140.0, 45.0)),
                    crate::scene::Seg::LineTo(Point::new(50.0, 45.0)),
                    crate::scene::Seg::Close,
                ])),
            )
            .with_id("band")
            .tagged("from", "a")
            .tagged("to", "b"),
        );
        assert_eq!(check(&s), vec![], "a ribbon is not a route");
    }

    #[test]
    fn a_stroke_that_connects_nothing_may_cross_whatever_it_likes() {
        // The same geometry as the test above, minus the endpoints. A chart's line
        // series runs straight over the bars drawn under it, which is the chart
        // working; asking which boxes it connects has no answer, so the rule that
        // asks does not apply. Seven of the fifteen violating gallery diagrams were
        // this exact shape.
        let mut s = canvas();
        s.push(box_at("mid", 60.0, 10.0, 40.0, 30.0));
        s.push(stroke(
            "series",
            vec![Point::new(10.0, 25.0), Point::new(150.0, 25.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn an_edge_declaring_one_endpoint_is_still_checked() {
        // `connects()` is an either/or: a stroke with a source but no recorded
        // target still claims to be joining something, so a box it crosses on the
        // way is still a box it does not connect.
        let mut s = canvas();
        s.push(box_at("mid", 60.0, 10.0, 40.0, 30.0));
        s.push(
            Node::new(
                Role::Edge,
                Content::Shape(Shape::Polyline(vec![
                    Point::new(10.0, 25.0),
                    Point::new(150.0, 25.0),
                ])),
            )
            .with_id("half")
            .tagged("from", "a"),
        );
        assert_eq!(
            check(&s),
            vec![Violation::EdgeThroughNode {
                edge: Some("half".into()),
                node: Some("mid".into()),
            }]
        );
    }

    #[test]
    fn an_edge_may_cross_the_boxes_it_connects() {
        // The exemption `connects()` guards: an edge legitimately meets its own
        // endpoints, and `joins` clears those crossings by name.
        let mut s = canvas();
        s.push(box_at("mid", 60.0, 10.0, 40.0, 30.0));
        s.push(wire_between(
            "e",
            "mid",
            "far",
            vec![Point::new(10.0, 25.0), Point::new(150.0, 25.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn an_edge_is_excused_the_boxes_it_connects_however_they_are_grouped() {
        // The shape that matters: identity on the group, geometry on the child,
        // which is how a diagram type that draws a box out of several pieces
        // has to be built.
        let mut scene = canvas();
        scene.push(grouped_box("A", 0.0));
        scene.push(grouped_box("B", 100.0));
        scene.push(grouped_edge("A", "B", 20.0, 120.0));
        // The run crosses both boxes, and both are its own ends.
        assert_eq!(check(&scene), Vec::new());
    }

    #[test]
    fn a_bar_drawn_on_a_box_is_excused_that_boxs_own_wires() {
        // A sequence activation is a bar on its actor's lifeline, carrying the
        // actor in its data rather than an id of its own — it is not separately
        // addressable, because it is not a separate thing. A message to that
        // actor crosses it by construction.
        let mut scene = canvas();
        scene.push(grouped_box("A", 0.0));
        scene.push(grouped_box("B", 140.0));
        let mut bar = box_at("", 60.0, 10.0, 20.0, 40.0);
        bar.id = None;
        bar.data.push(("actor".to_string(), "B".to_string()));
        scene.push(bar);
        scene.push(grouped_edge("A", "B", 20.0, 160.0));
        assert_eq!(check(&scene), Vec::new());
    }

    #[test]
    fn a_bar_owned_by_someone_else_is_still_a_box_in_the_way() {
        // The excuse is ownership, not the mere presence of a datum.
        let mut scene = canvas();
        scene.push(grouped_box("A", 0.0));
        scene.push(grouped_box("B", 140.0));
        let mut bar = box_at("", 60.0, 10.0, 20.0, 40.0);
        bar.id = None;
        bar.data.push(("actor".to_string(), "C".to_string()));
        scene.push(bar);
        scene.push(grouped_edge("A", "B", 20.0, 160.0));
        assert_eq!(check(&scene).len(), 1);
    }

    #[test]
    fn an_edge_crossing_a_box_it_does_not_connect_is_still_reported() {
        let mut scene = canvas();
        scene.push(grouped_box("A", 0.0));
        scene.push(grouped_box("C", 60.0));
        scene.push(grouped_box("B", 140.0));
        scene.push(grouped_edge("A", "B", 20.0, 160.0));
        let found = check(&scene);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(matches!(
            found.first(),
            Some(Violation::EdgeThroughNode { node: Some(id), .. }) if id == "C"
        ));
    }

    #[test]
    fn an_edge_running_along_a_border_is_not_passing_through() {
        // A wire hugging a box edge is tidy, not a defect.
        let mut s = canvas();
        s.push(box_at("b", 60.0, 10.0, 40.0, 30.0));
        s.push(wire(
            "e",
            vec![Point::new(10.0, 10.0), Point::new(150.0, 10.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn a_diagonal_is_measured_as_a_line_and_not_as_the_box_it_spans() {
        // The requirement diagram, reduced: it joins its boxes with straight
        // diagonals, and one passing under a box's bottom-right corner has a
        // bounding box overlapping it while the line itself is nowhere near.
        let mut s = canvas();
        s.push(box_at("mid", 20.0, 10.0, 40.0, 30.0));
        s.push(wire(
            "e",
            vec![Point::new(30.0, 90.0), Point::new(120.0, 20.0)],
        ));
        // Bounding boxes overlap in x over 30..60 and in y over 20..40.
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn a_diagonal_that_really_does_cut_the_box_is_still_reported() {
        // The other half of the same rule: the fix must not have made the
        // predicate blind to diagonals, only exact about them.
        let mut s = canvas();
        s.push(box_at("mid", 60.0, 30.0, 40.0, 30.0));
        s.push(wire(
            "e",
            vec![Point::new(50.0, 20.0), Point::new(110.0, 70.0)],
        ));
        assert!(matches!(
            check(&s).first(),
            Some(Violation::EdgeThroughNode { node: Some(id), .. }) if id == "mid"
        ));
    }

    #[test]
    fn a_box_thinner_than_the_inset_has_no_interior_to_pass_through() {
        // A rule drawn as a zero-height rect is a line, and nothing passes
        // *through* a line — the inset would otherwise invert its bounds.
        let mut s = canvas();
        s.push(box_at("hair", 40.0, 50.0, 60.0, 0.0));
        s.push(wire(
            "e",
            vec![Point::new(70.0, 10.0), Point::new(70.0, 90.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn an_edge_is_named_by_the_pair_it_joins_when_it_carries_no_id() {
        // Every finding used to read "the edge something", which named neither
        // end of neither edge and left a reader to go and measure the drawing.
        let mut s = canvas();
        let mut a = wire_between(
            "",
            "Connected",
            "Disconnecting",
            vec![Point::new(10.0, 50.0), Point::new(90.0, 50.0)],
        );
        a.id = None;
        let mut b = wire_between(
            "",
            "Reconnecting",
            "Closed",
            vec![Point::new(40.0, 52.0), Point::new(150.0, 52.0)],
        );
        b.id = None;
        s.push(a);
        s.push(b);
        let said = check(&s)
            .first()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(said.contains("Connected → Disconnecting"), "{said}");
        assert!(said.contains("Reconnecting → Closed"), "{said}");
    }

    #[test]
    fn an_edge_knowing_only_one_of_its_ends_is_named_by_that_one() {
        let one = Marked {
            node: &PLACEHOLDER,
            id: None,
            from: Some("A".into()),
            to: None,
            holds: None,
        };
        assert_eq!(one.name(), Some("A →".into()));
        let other = Marked {
            to: Some("B".into()),
            from: None,
            ..one.clone()
        };
        assert_eq!(other.name(), Some("→ B".into()));
        let neither = Marked {
            from: None,
            to: None,
            ..one.clone()
        };
        assert_eq!(neither.name(), None);
    }

    #[test]
    fn two_routes_leaving_one_point_share_a_trunk_rather_than_merging() {
        // A tree view: both children hang off one stem below the folder, which
        // is the notation rather than a defect in it.
        let mut s = canvas();
        s.push(wire_between(
            "a",
            "src",
            "one",
            vec![
                Point::new(32.0, 45.0),
                Point::new(32.0, 63.0),
                Point::new(48.0, 63.0),
            ],
        ));
        s.push(wire_between(
            "b",
            "src",
            "two",
            vec![
                Point::new(32.0, 45.0),
                Point::new(32.0, 89.0),
                Point::new(48.0, 89.0),
            ],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn routes_from_the_same_box_but_different_points_are_two_wires() {
        // The exemption is the shared *point*, not the shared box. Two edges
        // off one node's face are spread apart precisely so they read as two,
        // and excusing them by name would stop reporting it when that fails.
        let mut s = canvas();
        s.push(wire_between(
            "a",
            "src",
            "one",
            vec![Point::new(32.0, 45.0), Point::new(32.0, 89.0)],
        ));
        s.push(wire_between(
            "b",
            "src",
            "two",
            vec![Point::new(34.0, 45.0), Point::new(34.0, 89.0)],
        ));
        assert!(matches!(
            check(&s).first(),
            Some(Violation::MergedEdges { .. })
        ));
    }

    #[test]
    fn a_trunk_is_forgiven_only_where_it_is_a_trunk() {
        // Sharing a start does not licence the rest of the route: these two
        // leave together, separate, and come back onto one line further on.
        let mut s = canvas();
        s.push(wire_between(
            "a",
            "src",
            "one",
            vec![
                Point::new(10.0, 10.0),
                Point::new(10.0, 30.0),
                Point::new(60.0, 30.0),
                Point::new(60.0, 90.0),
            ],
        ));
        s.push(wire_between(
            "b",
            "src",
            "two",
            vec![
                Point::new(10.0, 10.0),
                Point::new(10.0, 50.0),
                Point::new(62.0, 50.0),
                Point::new(62.0, 90.0),
            ],
        ));
        assert!(matches!(
            check(&s).first(),
            Some(Violation::MergedEdges { .. })
        ));
    }

    #[test]
    fn two_edges_on_one_line_are_reported_with_the_overlap() {
        // Not a crossing, so a crossing counter scores it perfect — and yet the
        // two wires are indistinguishable.
        let mut s = canvas();
        s.push(wire(
            "a",
            vec![Point::new(10.0, 50.0), Point::new(90.0, 50.0)],
        ));
        s.push(wire(
            "b",
            vec![Point::new(40.0, 52.0), Point::new(150.0, 52.0)],
        ));
        let found = check(&s);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(matches!(
            found[0],
            Violation::MergedEdges { length, .. } if (length - 50.0).abs() < 0.01
        ));
    }

    #[test]
    fn edges_far_enough_apart_are_distinct() {
        let mut s = canvas();
        s.push(wire(
            "a",
            vec![Point::new(10.0, 40.0), Point::new(90.0, 40.0)],
        ));
        s.push(wire(
            "b",
            vec![Point::new(10.0, 60.0), Point::new(90.0, 60.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn edges_sharing_a_line_but_not_a_span_are_distinct() {
        // End to end along one lane is a continuation, not a merge.
        let mut s = canvas();
        s.push(wire(
            "a",
            vec![Point::new(10.0, 50.0), Point::new(40.0, 50.0)],
        ));
        s.push(wire(
            "b",
            vec![Point::new(60.0, 50.0), Point::new(90.0, 50.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }
}
