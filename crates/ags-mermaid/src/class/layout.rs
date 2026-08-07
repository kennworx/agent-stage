//! Sizing the boxes and handing the graph to the engine.
//!
//! A class box is a stack of three compartments, so its height is decided
//! before layout runs and its internal divisions have to travel with it — the
//! renderer draws the dividing rules from the same numbers that produced the
//! height, rather than measuring the members a second time.
//!
//! Nothing is clipped afterwards. A class box is a plain rectangle that fills
//! its own bounding box, so the engine's endpoints already sit on the outline.

use crate::label::{beside, runs, Placed as PlacedLabel};
use crate::layout;
use crate::metrics::{mono_text_width, text_width};
use crate::scene::Point;

use super::types::{Class, Diagram, End, Member, Relation};

/// Inside a box, either side of its members.
pub const PAD_X: f64 = 8.0;
/// The header band, before an annotation is allowed for.
pub const HEADER_HEIGHT: f64 = 32.0;
/// What a `<<stereotype>>` adds to the header.
pub const ANNOTATION_HEIGHT: f64 = 16.0;
/// One member line.
pub const ROW_HEIGHT: f64 = 20.0;
/// Above and below a compartment that has something in it.
pub const SECTION_PAD: f64 = 8.0;
/// The height of a compartment with nothing in it, which is still drawn.
pub const EMPTY_SECTION: f64 = 8.0;
pub const MIN_WIDTH: f64 = 120.0;
pub const MEMBER_FONT: f64 = 11.0;
pub const MEMBER_WEIGHT: u32 = 400;
pub const NAME_FONT: f64 = 13.0;
/// The class name is drawn bold, so it is measured bold. The renderer this
/// replaces measured it at 500 and drew it at 700, which left every box a few
/// pixels too narrow for the name it holds.
pub const NAME_WEIGHT: u32 = 700;
pub const ANNOTATION_FONT: f64 = 10.0;
pub const ANNOTATION_WEIGHT: u32 = 500;
pub const LABEL_FONT: f64 = 11.0;
pub const LABEL_WEIGHT: u32 = 400;
/// A line of text is this much taller than the type it is set in.
pub const LINE_HEIGHT: f64 = 1.3;
/// Between two boxes in one layer.
pub const NODE_GAP: f64 = 40.0;
/// Between one layer and the next.
pub const LAYER_GAP: f64 = 60.0;
/// Around the whole drawing.
pub const PADDING: f64 = 40.0;
/// How far a multiplicity sits off the line it belongs to.
pub const CARDINALITY_OFFSET: f64 = 14.0;

/// How tall each part of a box is. Decided before layout, drawn after it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Compartments {
    pub header: f64,
    pub attributes: f64,
    pub methods: f64,
}

impl Compartments {
    pub fn height(self) -> f64 {
        self.header + self.attributes + self.methods
    }
}

/// One class box, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedClass {
    pub id: String,
    pub label: String,
    pub annotation: String,
    pub attributes: Vec<Member>,
    pub methods: Vec<Member>,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub parts: Compartments,
}

/// One relationship, routed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedRelationship {
    pub from: String,
    pub to: String,
    pub kind: Relation,
    pub marker_at: End,
    pub label: String,
    pub from_cardinality: String,
    pub to_cardinality: String,
    pub points: Vec<Point>,
    /// Where the label sits, when there is one.
    pub label_at: Option<Point>,
}

/// A laid-out class diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub classes: Vec<PlacedClass>,
    pub relationships: Vec<PlacedRelationship>,
}

/// The engine's coordinates in the scene's own type.
const fn at(point: layout::Point) -> Point {
    Point::new(point.x, point.y)
}

/// How tall a compartment holding `count` members is.
///
/// An empty one is not zero: UML draws all three compartments whether or not
/// they have anything in them, and a box with only a name still has the two
/// rules under it.
fn section(count: usize) -> f64 {
    if count == 0 {
        return EMPTY_SECTION;
    }
    layout::as_f64(count) * ROW_HEIGHT + SECTION_PAD
}

/// The widest member line in a list, set in the mono face they are drawn in.
fn widest(members: &[Member]) -> f64 {
    members
        .iter()
        .map(|member| mono_text_width(&member.line(), MEMBER_FONT))
        .fold(0.0, f64::max)
}

/// How tall each compartment of a class is.
pub fn compartments(class: &Class) -> Compartments {
    Compartments {
        header: if class.annotation.is_empty() {
            HEADER_HEIGHT
        } else {
            HEADER_HEIGHT + ANNOTATION_HEIGHT
        },
        attributes: section(class.attributes.len()),
        methods: section(class.methods.len()),
    }
}

/// How big a class box has to be to hold what is in it.
pub fn measure(class: &Class) -> layout::Node {
    let name = text_width(&class.label, NAME_FONT, NAME_WEIGHT);
    let annotation = if class.annotation.is_empty() {
        0.0
    } else {
        // The stereotype is drawn with its own brackets, which are part of what
        // has to fit.
        text_width(
            &format!("<<{}>>", class.annotation),
            ANNOTATION_FONT,
            ANNOTATION_WEIGHT,
        )
    };
    let members = widest(&class.attributes).max(widest(&class.methods));
    let width = MIN_WIDTH
        .max(name + PAD_X * 2.0)
        .max(annotation + PAD_X * 2.0)
        .max(members + PAD_X * 2.0);
    layout::Node::new(width, compartments(class).height())
}

/// How far a label sits clear of the line it names.
const LABEL_GAP: f64 = 6.0;

/// How wide and tall a label is, over however many lines it takes.
pub fn label_size(label: &str) -> (f64, f64) {
    let lines: Vec<&str> = label.split('\n').collect();
    let width = lines
        .iter()
        .map(|line| text_width(line, LABEL_FONT, LABEL_WEIGHT))
        .fold(0.0, f64::max);
    (
        width,
        layout::as_f64(lines.len().max(1)) * LABEL_FONT * LINE_HEIGHT,
    )
}

/// The middle of the longest run of a route, whether that run is upright, and
/// which run it is.
///
/// The *longest* run, so a label has room to sit rather than landing on a
/// corner.
fn longest(points: &[Point]) -> Option<(Point, bool, usize)> {
    let (at, (_, a, b)) = points
        .windows(2)
        .enumerate()
        .filter_map(|(at, pair)| {
            let (a, b) = (pair.first()?, pair.get(1)?);
            Some((at, ((b.x - a.x).hypot(b.y - a.y), *a, *b)))
        })
        .max_by(|a, b| {
            a.1 .0
                .partial_cmp(&b.1 .0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    Some((
        Point::new(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y)),
        (b.y - a.y).abs() > (b.x - a.x).abs(),
        at,
    ))
}

/// Every leg of a label's own route except the one it is anchored to.
///
/// A label sits beside its own line to read as its label, so that leg cannot be
/// an obstacle — but the rest of the route is line like any other. Exempting the
/// whole route is what let a verb be drawn with its own turn struck through it.
fn own_legs(points: &[Point], anchored: usize) -> Vec<(Point, Point)> {
    runs(points)
        .into_iter()
        .enumerate()
        .filter(|(at, _)| *at != anchored)
        .map(|(_, leg)| leg)
        .collect()
}

/// Where a label sits, given everything already placed.
///
/// Two relationships between the same pair run in adjacent lanes, so their
/// labels land within a few pixels of each other; `taken` is what keeps the
/// second from being drawn over the first, and `lines` what keeps either from
/// being drawn over a line it does not belong beside — every other route, and
/// every leg of its own but the one it is anchored to.
fn label_at(
    points: &[Point],
    label: &str,
    taken: &[PlacedLabel],
    lines: &[(Point, Point)],
) -> Option<PlacedLabel> {
    let (middle, upright, anchored) = longest(points)?;
    let mut lines = lines.to_vec();
    lines.extend(own_legs(points, anchored));
    Some(beside(
        middle,
        upright,
        label_size(label),
        LABEL_GAP,
        taken,
        &lines,
    ))
}

/// Every run of every route but `mine`, which a label has to keep off.
fn elsewhere(routes: &[Vec<Point>], mine: usize) -> Vec<(Point, Point)> {
    routes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != mine)
        .flat_map(|(_, points)| runs(points))
        .collect()
}

/// Where a multiplicity sits relative to the end of the line it labels.
///
/// Beside the line rather than on it, and past the end rather than before it,
/// so it does not land under the marker it belongs to. `from` is the end being
/// labelled and `towards` the point after it, which is what says which way the
/// line leaves.
pub fn cardinality_at(from: Point, towards: Point) -> Point {
    let (dx, dy) = (towards.x - from.x, towards.y - from.y);
    if dx.abs() > dy.abs() {
        let along = if dx > 0.0 {
            CARDINALITY_OFFSET
        } else {
            -CARDINALITY_OFFSET
        };
        return Point::new(from.x + along, from.y - 10.0);
    }
    let along = if dy > 0.0 {
        CARDINALITY_OFFSET
    } else {
        -CARDINALITY_OFFSET
    };
    Point::new(from.x - CARDINALITY_OFFSET, from.y + along)
}

/// Lay out a parsed class diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    if diagram.classes.is_empty() {
        return Placed::default();
    }
    let nodes: Vec<layout::Node> = diagram.classes.iter().map(measure).collect();
    let edges: Vec<layout::Edge> = diagram
        .relationships
        .iter()
        .map(|rel| {
            layout::Edge::new(
                diagram.index_of(&rel.from).unwrap_or(usize::MAX),
                diagram.index_of(&rel.to).unwrap_or(usize::MAX),
            )
        })
        .collect();
    let placed = layout::layout(&layout::Graph {
        nodes,
        edges,
        // Inheritance reads downwards, and every other relationship is drawn
        // against that grain rather than turning the diagram.
        direction: layout::Direction::Down,
        spacing: layout::Spacing {
            node: NODE_GAP,
            layer: LAYER_GAP,
            padding: PADDING,
            ..layout::Spacing::default()
        },
        ports: Vec::new(),
    });

    let classes: Vec<PlacedClass> = diagram
        .classes
        .iter()
        .zip(&placed.nodes)
        .map(|(class, node)| PlacedClass {
            id: class.id.clone(),
            label: class.label.clone(),
            annotation: class.annotation.clone(),
            attributes: class.attributes.clone(),
            methods: class.methods.clone(),
            at: at(node.at),
            width: node.width,
            height: node.height,
            parts: compartments(class),
        })
        .collect();

    // Every route first, because a label has to know where the other lines run
    // before it can keep off them.
    let routes: Vec<Vec<Point>> = placed
        .edges
        .iter()
        .map(|route| route.points.iter().copied().map(at).collect())
        .collect();
    // Seeded with the boxes, because a label pushed off a line and into a box
    // has not been helped. It then grows as the labels are placed, so each
    // keeps out of the way of the ones before it.
    let mut taken: Vec<PlacedLabel> = classes
        .iter()
        .map(|class| {
            PlacedLabel::new(
                Point::new(
                    class.at.x + class.width / 2.0,
                    class.at.y + class.height / 2.0,
                ),
                class.width,
                class.height,
            )
        })
        .collect();
    let relationships = diagram
        .relationships
        .iter()
        .enumerate()
        .map(|(index, rel)| {
            let points = routes.get(index).cloned().unwrap_or_default();
            let label_at = (!rel.label.is_empty())
                .then(|| label_at(&points, &rel.label, &taken, &elsewhere(&routes, index)))
                .flatten();
            if let Some(placed) = label_at {
                taken.push(placed);
            }
            PlacedRelationship {
                from: rel.from.clone(),
                to: rel.to.clone(),
                kind: rel.kind,
                marker_at: rel.marker_at,
                label: rel.label.clone(),
                from_cardinality: rel.from_cardinality.clone(),
                to_cardinality: rel.to_cardinality.clone(),
                label_at: label_at.map(|placed| placed.at),
                points,
            }
        })
        .collect();

    Placed {
        width: placed.width,
        height: placed.height,
        classes,
        relationships,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::parse;

    fn sized(source: &str) -> layout::Node {
        let diagram = parse(source);
        measure(diagram.classes.first().expect("one class"))
    }

    #[test]
    fn an_empty_compartment_still_takes_room() {
        assert!((section(0) - EMPTY_SECTION).abs() < 1e-9);
        assert!(section(1) > section(0));
        assert!((section(2) - section(1) - ROW_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn a_box_with_nothing_in_it_is_the_smallest_one() {
        let bare = sized("classDiagram\n  class A");
        assert!((bare.width - MIN_WIDTH).abs() < 1e-9);
        assert!((bare.height - (HEADER_HEIGHT + EMPTY_SECTION * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_stereotype_makes_the_header_taller() {
        let plain = sized("classDiagram\n  class A");
        let marked = sized("classDiagram\n  class A {\n    <<interface>>\n  }");
        assert!((marked.height - plain.height - ANNOTATION_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn each_member_adds_a_row() {
        let one = sized("classDiagram\n  class A {\n    +int x\n  }");
        let two = sized("classDiagram\n  class A {\n    +int x\n    +int y\n  }");
        assert!((two.height - one.height - ROW_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn fields_and_methods_are_counted_in_their_own_compartments() {
        let split = parse("classDiagram\n  class A {\n    +int x\n    +go() void\n  }");
        let parts = compartments(split.classes.first().expect("one class"));
        assert!((parts.attributes - section(1)).abs() < 1e-9);
        assert!((parts.methods - section(1)).abs() < 1e-9);
        // Two of one kind leaves the other compartment at its empty height.
        let fields = parse("classDiagram\n  class A {\n    +int x\n    +int y\n  }");
        let only = compartments(fields.classes.first().expect("one class"));
        assert!((only.attributes - section(2)).abs() < 1e-9);
        assert!((only.methods - EMPTY_SECTION).abs() < 1e-9);
        // Either way the box comes out the same height, because an empty
        // compartment costs what a section's padding does.
        assert!((parts.height() - only.height()).abs() < 1e-9);
    }

    #[test]
    fn a_box_widens_for_whatever_is_longest_in_it() {
        let short = sized("classDiagram\n  class A");
        let long_name = sized("classDiagram\n  class AVeryLongClassNameIndeed");
        assert!(long_name.width > short.width);
        let long_member =
            sized("classDiagram\n  class A {\n    +aRatherLongMemberNameHere() String\n  }");
        assert!(long_member.width > short.width);
        let long_stereotype = sized("classDiagram\n  class A {\n    <<averylongstereotype>>\n  }");
        assert!(long_stereotype.width > short.width);
    }

    #[test]
    fn a_method_is_measured_with_its_parentheses_and_parameters() {
        let bare = sized("classDiagram\n  class A {\n    +go() void\n  }");
        let with_params = sized("classDiagram\n  class A {\n    +go(one, two) void\n  }");
        assert!(with_params.width > bare.width);
    }

    #[test]
    fn a_diagram_with_no_classes_lays_out_to_nothing() {
        assert_eq!(layout(&Diagram::default()), Placed::default());
    }

    #[test]
    fn every_class_is_placed_and_every_relationship_routed() {
        let placed = layout(&parse(
            "classDiagram\n  class Animal\n  class Dog\n  Animal <|-- Dog",
        ));
        assert_eq!(placed.classes.len(), 2);
        assert_eq!(placed.relationships.len(), 1);
        let route = placed.relationships.first().unwrap();
        assert!(route.points.len() >= 2);
        assert!(placed.width > 0.0 && placed.height > 0.0);
    }

    #[test]
    fn the_compartments_add_up_to_the_height_of_the_box() {
        let placed = layout(&parse(
            "classDiagram\n  class A {\n    <<interface>>\n    +int x\n    +go() void\n  }",
        ));
        let class = placed.classes.first().unwrap();
        assert!((class.parts.height() - class.height).abs() < 1e-9);
        assert!(class.parts.header > HEADER_HEIGHT);
    }

    #[test]
    fn a_relationship_with_no_label_has_nowhere_to_put_one() {
        let placed = layout(&parse("classDiagram\n  A --> B"));
        assert_eq!(placed.relationships.first().unwrap().label_at, None);
    }

    #[test]
    fn a_labelled_relationship_puts_its_label_beside_its_longest_run() {
        let placed = layout(&parse("classDiagram\n  A --> B : uses"));
        let rel = placed.relationships.first().unwrap();
        let at = rel.label_at.unwrap();
        let (first, last) = (rel.points.first().unwrap(), rel.points.last().unwrap());
        // Between the ends along the line, and clear of it across.
        assert!(at.y > first.y && at.y < last.y);
        assert!(at.x > first.x, "the label sits on the line it names");
    }

    #[test]
    fn a_label_needs_a_run_to_sit_on() {
        assert_eq!(label_at(&[], "x", &[], &[]), None);
        assert_eq!(label_at(&[Point::new(1.0, 2.0)], "x", &[], &[]), None);
        assert_eq!(longest(&[]), None);
    }

    #[test]
    fn a_label_steps_aside_from_a_vertical_line_and_above_a_horizontal_one() {
        let across = label_at(
            &[Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
            "uses",
            &[],
            &[],
        )
        .expect("a place");
        assert!((across.at.x - 50.0).abs() < 1e-9);
        assert!(across.at.y < 0.0, "a horizontal run is labelled above");
        let down = label_at(
            &[Point::new(0.0, 0.0), Point::new(0.0, 100.0)],
            "uses",
            &[],
            &[],
        )
        .expect("a place");
        assert!((down.at.y - 50.0).abs() < 1e-9);
        // Far enough across that the whole word clears the line.
        assert!(down.at.x - down.width / 2.0 > 0.0);
    }

    #[test]
    fn the_longest_run_is_the_one_a_label_sits_on() {
        // A short level run and a long upright one: the label belongs on the
        // long one, and knows it is upright.
        let (middle, upright, anchored) = longest(&[
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 200.0),
        ])
        .expect("a run");
        assert!(upright);
        assert!((middle.y - 100.0).abs() < 1e-9);
        // And names it, so the short leg round the corner is left to be treated
        // as the obstacle it is.
        assert_eq!(anchored, 1);
    }

    #[test]
    fn two_relationships_between_one_pair_do_not_share_a_label() {
        // Both run in the gap between the same two boxes, so both labels want
        // the same few pixels.
        let placed = layout(&parse(
            "classDiagram\n  View --> Model : reads\n  Model ..> View : notifies",
        ));
        let boxes: Vec<PlacedLabel> = placed
            .relationships
            .iter()
            .filter_map(|rel| {
                let at = rel.label_at?;
                let (width, height) = label_size(&rel.label);
                Some(PlacedLabel::new(at, width, height))
            })
            .collect();
        assert_eq!(boxes.len(), 2);
        let (Some(first), Some(second)) = (boxes.first(), boxes.get(1)) else {
            panic!("two labels")
        };
        assert!(
            !first.overlaps(*second),
            "the two labels overlap: {boxes:?}"
        );
    }

    #[test]
    fn a_multiplicity_sits_beside_the_line_and_past_its_end() {
        let start = Point::new(100.0, 100.0);
        // A line leaving downwards puts the number below and to the left.
        let down = cardinality_at(start, Point::new(100.0, 200.0));
        assert!(down.y > start.y);
        assert!(down.x < start.x);
        // Leaving upwards puts it above.
        let up = cardinality_at(start, Point::new(100.0, 0.0));
        assert!(up.y < start.y);
        // Leaving rightwards puts it to the right and above.
        let right = cardinality_at(start, Point::new(200.0, 100.0));
        assert!(right.x > start.x);
        assert!(right.y < start.y);
        let left = cardinality_at(start, Point::new(0.0, 100.0));
        assert!(left.x < start.x);
    }

    #[test]
    fn a_relationship_naming_a_class_that_does_not_exist_is_dropped_not_drawn() {
        // The parser declares whatever a relationship names, so this can only be
        // reached by building the diagram directly — which the renderer's own
        // callers can do.
        let diagram = Diagram {
            classes: vec![Class {
                id: "A".into(),
                label: "A".into(),
                ..Class::default()
            }],
            relationships: vec![super::super::types::Relationship {
                from: "A".into(),
                to: "Nowhere".into(),
                kind: Relation::Association,
                marker_at: End::To,
                label: String::new(),
                from_cardinality: String::new(),
                to_cardinality: String::new(),
            }],
        };
        let placed = layout(&diagram);
        assert_eq!(placed.classes.len(), 1);
        assert!(placed.relationships.first().unwrap().points.is_empty());
    }
}
