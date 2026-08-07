//! Scenes the rules are asked about.
//!
//! Shared, because a rule is only ever tested by building a drawing and asking
//! [`super::check`] what it makes of it.

#![allow(unused_imports, reason = "each rule module builds a different subset")]

use crate::scene::{Content, Layer, Node, Point, Role, Scene, Shape, Size};

pub(crate) fn canvas() -> Scene {
    Scene::new(Size {
        width: 200.0,
        height: 100.0,
    })
}

pub(crate) fn box_at(id: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: Point::new(x, y),
            size: Size {
                width: w,
                height: h,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    )
    .with_id(id)
}

/// A box drawn as an unnamed child of a group that carries the name.
pub(crate) fn grouped_box(id: &str, x: f64) -> Node {
    let outline = Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: Point::new(x, 0.0),
            size: Size {
                width: 40.0,
                height: 40.0,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    );
    Node::new(Role::Node, Content::Group(vec![outline])).with_id(id)
}

/// An edge drawn the same way, naming its ends on the group.
pub(crate) fn grouped_edge(from: &str, to: &str, start: f64, end: f64) -> Node {
    let run = Node::new(
        Role::Edge,
        Content::Shape(Shape::Polyline(vec![
            Point::new(start, 20.0),
            Point::new(end, 20.0),
        ])),
    );
    Node::new(Role::Edge, Content::Group(vec![run]))
        .tagged("from", from)
        .tagged("to", to)
}

/// An edge that connects two boxes, the way a real one does.
///
/// The endpoints are not decoration: `edges_through_nodes` only questions a
/// stroke that claims to connect something, so a helper that omitted them
/// would build a chart series and quietly stop testing the rule.
pub(crate) fn wire(id: &str, points: Vec<Point>) -> Node {
    wire_between(id, "a", "b", points)
}

/// An edge connecting the two boxes named, for tests that care which.
pub(crate) fn wire_between(id: &str, from: &str, to: &str, points: Vec<Point>) -> Node {
    Node::new(Role::Edge, Content::Shape(Shape::Polyline(points)))
        .with_id(id)
        .tagged("from", from)
        .tagged("to", to)
}

/// A stroke that connects nothing — a chart series, an axis, a spine.
pub(crate) fn stroke(id: &str, points: Vec<Point>) -> Node {
    Node::new(Role::Edge, Content::Shape(Shape::Polyline(points))).with_id(id)
}

/// Two boxes, `a` on the left and `b` on the right, with a clear gap between.
pub(crate) fn two_boxes() -> Scene {
    let mut s = canvas();
    s.push(box_at("a", 10.0, 30.0, 40.0, 20.0));
    s.push(box_at("b", 140.0, 30.0, 40.0, 20.0));
    s
}

/// A frame round `x, y, w, h` declaring what it is drawn round.
pub(crate) fn frame(id: &str, holds: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    let outline = Node::new(
        Role::Frame,
        Content::Shape(Shape::Rect {
            at: Point::new(x, y),
            size: Size {
                width: w,
                height: h,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    );
    Node::new(Role::Frame, Content::Group(vec![outline]))
        .with_id(id)
        .tagged("holds", holds)
        .on(Layer::Frame)
}
