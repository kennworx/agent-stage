//! What can go wrong with an area: a frame drawn round a stranger, a shape off
//! the canvas, a label something later was painted over.

use super::report::Violation;
use super::scene::{bounds, boxes, Marked, Rect};
use crate::scene::{Content, Node, Role, Size};

/// Boxes a frame is drawn round without holding.
///
/// Only frames that declare a `holds` datum are checked: a diagram type whose
/// frames mean nothing in particular says nothing, and is left alone.
pub(super) fn enclosed_strangers(nodes: &[Marked<'_>]) -> Vec<Violation> {
    let frames: Vec<(&Marked<'_>, Rect, Vec<String>)> = nodes
        .iter()
        .filter(|held| held.node.role == Role::Frame)
        .filter_map(|held| {
            let holds = held.holds.clone()?;
            let Content::Shape(shape) = &held.node.content else {
                return None;
            };
            let area = bounds(shape)?;
            Some((
                held,
                area,
                holds.split_whitespace().map(str::to_string).collect(),
            ))
        })
        .collect();
    let mut out = Vec::new();
    for (frame, area, holds) in &frames {
        for (id, rect) in boxes(nodes) {
            if holds.contains(&id) || Some(&id) == frame.id.as_ref() {
                continue;
            }
            if area.contains_rect(rect) {
                out.push(Violation::Enclosed {
                    frame: frame.id.clone(),
                    node: Some(id),
                });
            }
        }
    }
    out
}

/// Anything drawn beyond the canvas, which a viewer cuts rather than shrinks.
pub(super) fn outside_canvas(nodes: &[&Node], canvas: Size) -> Vec<Violation> {
    let page = Rect {
        x: 0.0,
        y: 0.0,
        w: canvas.width,
        h: canvas.height,
    };
    nodes
        .iter()
        .filter_map(|n| match &n.content {
            Content::Shape(shape) => bounds(shape).map(|b| (n, b)),
            _ => None,
        })
        .filter(|(_, b)| !page.contains_rect(*b))
        .map(|(n, _)| Violation::OutsideCanvas { id: n.id.clone() })
        .collect()
}

/// Labels fully covered by something painted later.
///
/// Geometry alone cannot catch this: the coordinates are correct and only the
/// picture is wrong, which is why it needed an eye the last time it happened.
pub(super) fn occluded_labels(nodes: &[&Node]) -> Vec<Violation> {
    let mut out = Vec::new();
    for (i, label) in nodes.iter().enumerate() {
        if label.role != Role::Label {
            continue;
        }
        let Content::Shape(shape) = &label.content else {
            continue;
        };
        let Some(area) = bounds(shape) else { continue };
        let covered = nodes.iter().skip(i + 1).any(|other| {
            if other.layer < label.layer {
                return false;
            }
            match &other.content {
                Content::Shape(s) => bounds(s).is_some_and(|b| b.contains_rect(area)),
                _ => false,
            }
        });
        if covered {
            out.push(Violation::Occluded {
                id: label.id.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::check;
    use crate::constraint::fixture::*;
    use crate::scene::{Layer, Point, Shape};

    #[test]
    fn a_shape_beyond_the_canvas_is_reported() {
        let mut s = canvas();
        s.push(box_at("over", 180.0, 10.0, 40.0, 20.0));
        assert_eq!(
            check(&s),
            vec![Violation::OutsideCanvas {
                id: Some("over".into())
            }]
        );
    }

    #[test]
    fn a_label_covered_by_a_later_layer_is_reported() {
        // Correct coordinates, wrong picture — the case no geometric check
        // catches and the reason this rule exists.
        let mut s = canvas();
        s.push(
            Node::new(
                Role::Label,
                Content::Shape(Shape::Rect {
                    at: Point::new(20.0, 20.0),
                    size: Size {
                        width: 20.0,
                        height: 10.0,
                    },
                    rx: 0.0,
                    ry: 0.0,
                }),
            )
            .with_id("text"),
        );
        s.push(
            Node::new(
                Role::Decoration,
                Content::Shape(Shape::Rect {
                    at: Point::new(10.0, 10.0),
                    size: Size {
                        width: 60.0,
                        height: 40.0,
                    },
                    rx: 0.0,
                    ry: 0.0,
                }),
            )
            .on(Layer::Overlay)
            .with_id("bubble"),
        );
        assert_eq!(
            check(&s),
            vec![Violation::Occluded {
                id: Some("text".into())
            }]
        );
    }

    #[test]
    fn a_label_over_something_painted_earlier_is_fine() {
        // A node's own label sits on its box by design; the blunt version of
        // this rule would fire on every correct diagram.
        let mut s = canvas();
        s.push(box_at("b", 10.0, 10.0, 60.0, 40.0));
        s.push(
            Node::new(
                Role::Label,
                Content::Shape(Shape::Rect {
                    at: Point::new(20.0, 20.0),
                    size: Size {
                        width: 20.0,
                        height: 10.0,
                    },
                    rx: 0.0,
                    ry: 0.0,
                }),
            )
            .with_id("name"),
        );
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn an_icon_inside_a_box_is_not_a_box() {
        // Role and layer being separate is what makes this pass: an icon paints
        // with nodes, so a check selecting on layer would treat it as one and
        // report every edge that reaches the box it sits in.
        let mut s = canvas();
        s.push(box_at("b", 60.0, 10.0, 40.0, 30.0));
        let mut icon = Node::new(
            Role::Icon,
            Content::Shape(Shape::Circle {
                c: Point::new(80.0, 25.0),
                r: 5.0,
            }),
        );
        icon.id = Some("glyph".into());
        s.push(icon);
        s.push(wire(
            "e",
            vec![Point::new(10.0, 60.0), Point::new(150.0, 60.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn checks_reach_inside_groups() {
        let mut s = canvas();
        let inner = box_at("deep", 180.0, 10.0, 40.0, 20.0);
        s.push(Node::new(Role::Frame, Content::Group(vec![inner])).with_id("group"));
        assert_eq!(
            check(&s),
            vec![Violation::OutsideCanvas {
                id: Some("deep".into())
            }]
        );
    }

    #[test]
    fn a_frame_round_only_its_own_members_is_fine() {
        let mut s = canvas();
        s.push(frame("ci", "a b", 5.0, 5.0, 90.0, 90.0));
        s.push(box_at("a", 10.0, 10.0, 30.0, 20.0));
        s.push(box_at("b", 50.0, 10.0, 30.0, 20.0));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn a_frame_round_a_stranger_says_so() {
        let mut s = canvas();
        s.push(frame("ci", "a", 5.0, 5.0, 90.0, 90.0));
        s.push(box_at("a", 10.0, 10.0, 30.0, 20.0));
        s.push(box_at("outsider", 50.0, 50.0, 30.0, 20.0));
        assert_eq!(
            check(&s),
            vec![Violation::Enclosed {
                frame: Some("ci".into()),
                node: Some("outsider".into())
            }]
        );
    }

    #[test]
    fn a_box_only_partly_inside_a_frame_is_not_enclosed_by_it() {
        let mut s = canvas();
        s.push(frame("ci", "a", 5.0, 5.0, 90.0, 90.0));
        s.push(box_at("a", 10.0, 10.0, 30.0, 20.0));
        // Reaches out of the right-hand side, so the frame is not round it.
        s.push(box_at("half-in", 80.0, 50.0, 40.0, 20.0));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn a_frame_that_claims_nothing_is_not_asked_about_anything() {
        // Other diagram types draw frames that mean nothing in particular.
        let mut s = canvas();
        let outline = Node::new(
            Role::Frame,
            Content::Shape(Shape::Rect {
                at: Point::new(5.0, 5.0),
                size: Size {
                    width: 90.0,
                    height: 90.0,
                },
                rx: 0.0,
                ry: 0.0,
            }),
        );
        s.push(Node::new(Role::Frame, Content::Group(vec![outline])).with_id("plain"));
        s.push(box_at("anyone", 10.0, 10.0, 30.0, 20.0));
        assert_eq!(check(&s), vec![]);
    }

    #[test]
    fn a_box_drawn_from_several_shapes_is_asked_about_once() {
        // A subroutine is a rectangle and two rules; all three carry the same
        // name, and the box is one box.
        let mut s = canvas();
        s.push(frame("ci", "a", 5.0, 5.0, 90.0, 90.0));
        s.push(box_at("a", 10.0, 10.0, 30.0, 20.0));
        let parts = vec![
            Node::new(
                Role::Node,
                Content::Shape(Shape::Rect {
                    at: Point::new(50.0, 50.0),
                    size: Size {
                        width: 30.0,
                        height: 20.0,
                    },
                    rx: 0.0,
                    ry: 0.0,
                }),
            ),
            Node::new(
                Role::Node,
                Content::Shape(Shape::Line {
                    a: Point::new(56.0, 50.0),
                    b: Point::new(56.0, 70.0),
                }),
            ),
        ];
        s.push(Node::new(Role::Node, Content::Group(parts)).with_id("outsider"));
        assert_eq!(
            check(&s),
            vec![Violation::Enclosed {
                frame: Some("ci".into()),
                node: Some("outsider".into())
            }]
        );
    }
}
