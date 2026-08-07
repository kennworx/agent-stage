//! Named glyphs, shared by every diagram type whose grammar declares an icon.
//!
//! Each glyph is authored on a 24×24 grid and drawn in `currentColor`, so the
//! element that owns it decides the colour through a class rather than through a
//! literal. A caller asks for a size and a position; the glyph arrives as one
//! group with a `translate … scale` on it, which keeps it inside the owning
//! element's identity group and out of the identity contract.
//!
//! An unknown name degrades to a plain rounded square rather than failing: a
//! stray icon name should cost one glyph, not the diagram.

use crate::scene::{Content, Node, Paint, Point, Role, Seg, Shape, Size, Transform};

/// The grid every glyph is authored on.
const GRID: f64 = 24.0;

/// Stroke width shared by the outline glyphs.
const OUTLINE_WIDTH: f64 = 1.6;

/// The class an outline part carries, so one CSS rule gives them all their
/// round caps and joins.
pub const OUTLINE_CLASS: &str = "bm-icon-line";

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn size(w: f64, h: f64) -> Size {
    Size {
        width: w,
        height: h,
    }
}

/// A filled part of a glyph, inheriting the owner's colour.
fn filled(shape: Shape) -> Node {
    Node::new(Role::Icon, Content::Shape(shape))
}

/// An outlined part of a glyph.
fn outlined(shape: Shape) -> Node {
    Node::new(Role::Icon, Content::Shape(shape))
        .classed(OUTLINE_CLASS)
        .painted(Paint {
            fill: Some(crate::scene::Color::None),
            stroke: Some(crate::scene::Color::Literal("currentColor".into())),
            stroke_width: Some(OUTLINE_WIDTH),
            ..Paint::default()
        })
}

fn rect(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Shape {
    Shape::Rect {
        at: p(x, y),
        size: size(width, height),
        rx: radius,
        ry: radius,
    }
}

/// The parts of one named glyph, in the 24×24 grid.
/// The parts of one named glyph.
///
/// One long table on purpose: every glyph is a literal list of shapes on the
/// same grid, and splitting it up would put the coordinates further from each
/// other rather than closer.
#[expect(
    clippy::too_many_lines,
    reason = "a table of glyph outlines, one arm each"
)]
fn glyph(name: &str) -> Vec<Node> {
    match name {
        "person" => vec![
            filled(Shape::Circle {
                c: p(12.0, 7.0),
                r: 4.0,
            }),
            // Shoulders. The reference wrote this as a relative cubic followed
            // by a smooth one; the second control point below is the reflection
            // that `s` implies.
            Node::new(
                Role::Icon,
                Content::Shape(Shape::Path(vec![
                    Seg::MoveTo(p(4.0, 21.0)),
                    Seg::Cubic {
                        c1: p(4.0, 16.6),
                        c2: p(7.6, 13.0),
                        to: p(12.0, 13.0),
                    },
                    Seg::Cubic {
                        c1: p(16.4, 13.0),
                        c2: p(20.0, 16.6),
                        to: p(20.0, 21.0),
                    },
                ])),
            )
            .classed(OUTLINE_CLASS)
            .painted(Paint {
                fill: Some(crate::scene::Color::None),
                stroke: Some(crate::scene::Color::Literal("currentColor".into())),
                stroke_width: Some(2.0),
                ..Paint::default()
            }),
        ],
        "database" => vec![
            filled(Shape::Ellipse {
                c: p(12.0, 6.0),
                rx: 8.0,
                ry: 3.0,
            }),
            filled(Shape::Path(vec![
                Seg::MoveTo(p(4.0, 6.0)),
                Seg::LineTo(p(4.0, 18.0)),
                Seg::Cubic {
                    c1: p(4.0, 19.66),
                    c2: p(7.58, 21.0),
                    to: p(12.0, 21.0),
                },
                Seg::Cubic {
                    c1: p(16.42, 21.0),
                    c2: p(20.0, 19.66),
                    to: p(20.0, 18.0),
                },
                Seg::LineTo(p(20.0, 6.0)),
            ])),
        ],
        "server" => vec![
            outlined(rect(3.0, 4.0, 18.0, 7.0, 1.5)),
            outlined(rect(3.0, 13.0, 18.0, 7.0, 1.5)),
            filled(Shape::Circle {
                c: p(7.0, 7.5),
                r: 1.0,
            }),
            filled(Shape::Circle {
                c: p(7.0, 16.5),
                r: 1.0,
            }),
        ],
        "queue" => vec![
            outlined(rect(4.0, 4.0, 16.0, 4.0, 1.0)),
            outlined(rect(4.0, 10.0, 16.0, 4.0, 1.0)),
            outlined(rect(4.0, 16.0, 16.0, 4.0, 1.0)),
        ],
        "cloud" => vec![outlined(Shape::Path(vec![
            Seg::MoveTo(p(6.5, 19.0)),
            Seg::Arc {
                r: size(4.5, 4.5),
                large: false,
                sweep: true,
                to: p(6.5, 10.0),
            },
            Seg::Arc {
                r: size(6.0, 6.0),
                large: false,
                sweep: true,
                to: p(17.9, 8.5),
            },
            Seg::Arc {
                r: size(4.0, 4.0),
                large: false,
                sweep: true,
                to: p(17.5, 19.0),
            },
            Seg::Close,
        ]))],
        "disk" => vec![
            outlined(rect(3.0, 5.0, 18.0, 14.0, 2.0)),
            outlined(Shape::Circle {
                c: p(12.0, 12.0),
                r: 4.0,
            }),
            filled(Shape::Circle {
                c: p(12.0, 12.0),
                r: 1.0,
            }),
        ],
        // A globe: the world, with a line of latitude and two of longitude.
        "internet" => vec![
            outlined(Shape::Circle {
                c: p(12.0, 12.0),
                r: 9.0,
            }),
            outlined(Shape::Line {
                a: p(3.0, 12.0),
                b: p(21.0, 12.0),
            }),
            outlined(Shape::Path(vec![
                Seg::MoveTo(p(12.0, 3.0)),
                Seg::Cubic {
                    c1: p(14.6, 5.7),
                    c2: p(14.6, 18.3),
                    to: p(12.0, 21.0),
                },
            ])),
            outlined(Shape::Path(vec![
                Seg::MoveTo(p(12.0, 3.0)),
                Seg::Cubic {
                    c1: p(9.4, 5.7),
                    c2: p(9.4, 18.3),
                    to: p(12.0, 21.0),
                },
            ])),
        ],
        // A chip: the die, and the legs it stands on.
        "cpu" => {
            let mut parts = vec![
                outlined(rect(6.0, 6.0, 12.0, 12.0, 1.5)),
                filled(rect(9.5, 9.5, 5.0, 5.0, 0.5)),
            ];
            for along in [9.0_f64, 15.0] {
                parts.push(outlined(Shape::Line {
                    a: p(along, 3.0),
                    b: p(along, 6.0),
                }));
                parts.push(outlined(Shape::Line {
                    a: p(along, 18.0),
                    b: p(along, 21.0),
                }));
                parts.push(outlined(Shape::Line {
                    a: p(3.0, along),
                    b: p(6.0, along),
                }));
                parts.push(outlined(Shape::Line {
                    a: p(18.0, along),
                    b: p(21.0, along),
                }));
            }
            parts
        }
        _ => vec![outlined(rect(3.0, 3.0, 18.0, 18.0, 3.0))],
    }
}

/// Whether `name` resolves to a built-in glyph rather than the fallback.
pub fn has_icon(name: &str) -> bool {
    matches!(
        name,
        "person" | "database" | "server" | "queue" | "cloud" | "disk" | "internet" | "cpu"
    )
}

/// A named glyph, drawn in a `size`-wide box with its top-left at `at`.
pub fn icon(name: &str, at: Point, size: f64, class: &str) -> Node {
    let mut node = Node::new(Role::Icon, Content::Group(glyph(name)))
        .classed("bm-icon")
        .painted(Paint {
            fill: Some(crate::scene::Color::Literal("currentColor".into())),
            ..Paint::default()
        });
    node.transform = Some(Transform::TranslateScale {
        at,
        scale: size / GRID,
    });
    if !class.is_empty() {
        node = node.classed(class);
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parts of a glyph, or nothing at all — which fails the count every
    /// caller asserts on.
    fn parts(name: &str) -> Vec<Node> {
        match icon(name, p(0.0, 0.0), 20.0, "c4-icon").content {
            Content::Group(children) => children,
            _ => Vec::new(),
        }
    }

    #[test]
    fn every_glyph_the_c4_boxes_ask_for_is_registered() {
        assert_eq!(parts("person").len(), 2);
        assert_eq!(parts("database").len(), 2);
        assert_eq!(parts("server").len(), 4);
        assert_eq!(parts("queue").len(), 3);
        for name in ["person", "database", "server", "queue"] {
            assert!(has_icon(name), "{name}");
        }
    }

    #[test]
    fn every_glyph_the_architecture_boxes_ask_for_is_registered() {
        assert_eq!(parts("cloud").len(), 1);
        assert_eq!(parts("disk").len(), 3);
        assert_eq!(parts("internet").len(), 4);
        // A die, its pad, and eight legs.
        assert_eq!(parts("cpu").len(), 10);
        for name in ["cloud", "disk", "internet", "cpu"] {
            assert!(has_icon(name), "{name}");
        }
    }

    #[test]
    fn an_unknown_name_draws_a_square_rather_than_failing() {
        assert_eq!(parts("no-such-glyph").len(), 1);
        assert!(!has_icon("no-such-glyph"));
    }

    #[test]
    fn a_glyph_is_scaled_from_its_own_grid_to_the_asked_for_size() {
        let node = icon("server", p(10.0, 20.0), 12.0, "c4-icon");
        assert_eq!(
            node.transform,
            Some(Transform::TranslateScale {
                at: p(10.0, 20.0),
                scale: 0.5
            })
        );
        assert_eq!(node.class, vec!["bm-icon", "c4-icon"]);
    }

    #[test]
    fn a_glyph_with_no_extra_class_still_carries_the_shared_one() {
        assert_eq!(icon("queue", p(0.0, 0.0), 24.0, "").class, vec!["bm-icon"]);
    }

    #[test]
    fn an_icon_paints_with_the_nodes_without_being_one() {
        // Constraints select on `Role::Node`; a glyph inside a box must not be
        // picked up as a box of its own.
        let node = icon("person", p(0.0, 0.0), 20.0, "");
        assert_eq!(node.role, Role::Icon);
        assert_eq!(node.layer, crate::scene::Layer::Node);
    }

    #[test]
    fn outline_parts_declare_their_own_stroke_and_no_fill() {
        let children = parts("queue");
        assert_eq!(children.len(), 3);
        for part in &children {
            assert_eq!(part.paint.fill, Some(crate::scene::Color::None));
            assert_eq!(part.paint.stroke_width, Some(OUTLINE_WIDTH));
            assert!(part.class.contains(&OUTLINE_CLASS.to_string()));
        }
    }
}
