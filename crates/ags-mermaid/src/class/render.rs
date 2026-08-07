//! A placed class diagram, drawn into the scene.
//!
//! Identity contract: each box is a group carrying `data-id`, and each line a
//! group carrying `data-from` and `data-to`.
//!
//! A member line is drawn as several text runs rather than one, because its
//! parts are not the same colour — the visibility mark is faintest, the name
//! carries the weight, the type sits between them. They are set in a monospace
//! face, which is what lets each run be placed by measuring the one before it.

use crate::api::ColorMode;
use crate::metrics::mono_text_width;
use crate::scene::{
    Anchor, Color, Content, Font, Layer, Marker, Node, Paint, Point, Role, Scene, Shape, Size,
    TextRun,
};
use crate::theme::Theme;

use super::layout::{
    cardinality_at, layout, Placed, PlacedClass, PlacedRelationship, ANNOTATION_FONT,
    ANNOTATION_WEIGHT, LABEL_FONT, LABEL_WEIGHT, LINE_HEIGHT, MEMBER_FONT, MEMBER_WEIGHT,
    NAME_FONT, NAME_WEIGHT, PAD_X, ROW_HEIGHT,
};
use super::types::{
    End, Member, AGGREGATION_MARKER, ARROW_MARKER, COMPOSITION_MARKER, INHERIT_MARKER,
};

const BASELINE: &str = "0.35em";
/// How far down the header a stereotype's own baseline sits.
const ANNOTATION_DROP: f64 = 12.0;
/// How far the class name moves down to make room for a stereotype above it.
const NAME_DROP: f64 = 6.0;
/// The gap between a compartment's rule and the first member under it.
const ROW_TOP: f64 = 4.0;
/// The diamond and triangle markers, on their own 12×10 grid.
const MARK_W: f64 = 12.0;
const MARK_H: f64 = 10.0;
/// The open arrow, which is smaller.
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 6.0;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(at: Point, width: f64, height: f64) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at,
            size: size(width, height),
            rx: 0.0,
            ry: 0.0,
        }),
    )
}

fn rule(y: f64, from: f64, to: f64) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Shape::Line {
            a: point(from, y),
            b: point(to, y),
        }),
    )
    .classed("class-rule")
}

/// One run of text, however it is anchored.
fn run(at: Point, content: &str, font: f64, weight: u32, anchor: Anchor, italic: bool) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor,
            font: Font {
                size: font,
                weight,
                italic,
            },
            dy: Some(BASELINE.to_string()),
            content: content.to_string(),
        }),
    )
}

/// A label, one node per line, centred about `at`.
fn centred(at: Point, label: &str, font: f64, weight: u32, class: &str) -> Vec<Node> {
    let plain = crate::text::strip_formatting_tags(label);
    let lines: Vec<&str> = plain.split('\n').collect();
    let step = font * LINE_HEIGHT;
    let first = -(crate::layout::as_f64(lines.len().saturating_sub(1)) / 2.0) * step;
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            run(
                Point::new(at.x, at.y + first + crate::layout::as_f64(index) * step),
                line,
                font,
                weight,
                Anchor::Middle,
                false,
            )
            .classed(class)
        })
        .collect()
}

/// A member line, split into the runs it is coloured in.
fn member_runs(member: &Member) -> Vec<(String, &'static str)> {
    let mut runs = Vec::new();
    let mark = member.visibility.mark();
    if !mark.is_empty() {
        runs.push((format!("{mark} "), "class-vis"));
    }
    runs.push((member.written(), "class-member-name"));
    if !member.kind.is_empty() {
        runs.push((": ".to_string(), "class-vis"));
        runs.push((member.kind.clone(), "class-type"));
    }
    runs
}

/// One member line, laid left to right from `x`.
fn member_nodes(member: &Member, x: f64, y: f64) -> Vec<Node> {
    let mut cursor = x;
    member_runs(member)
        .into_iter()
        .map(|(text, class)| {
            let at = point(cursor, y);
            cursor += mono_text_width(&text, MEMBER_FONT);
            let mut node = run(
                at,
                &text,
                MEMBER_FONT,
                MEMBER_WEIGHT,
                Anchor::Start,
                member.is_abstract,
            )
            .classed("class-member")
            .classed(class);
            if member.is_static {
                node = node.classed("class-static");
            }
            node
        })
        .collect()
}

/// Every member of one compartment, from the rule under it.
fn compartment(members: &[Member], x: f64, top: f64) -> Vec<Node> {
    members
        .iter()
        .enumerate()
        .flat_map(|(index, member)| {
            let y = top + ROW_TOP + crate::layout::as_f64(index) * ROW_HEIGHT + ROW_HEIGHT / 2.0;
            member_nodes(member, x + PAD_X, y)
        })
        .collect()
}

/// The name of a class, and the stereotype above it when there is one.
fn header_text(class: &PlacedClass) -> Vec<Node> {
    let middle = class.at.x + class.width / 2.0;
    let mut out = Vec::new();
    let mut name_y = class.at.y + class.parts.header / 2.0;
    if !class.annotation.is_empty() {
        out.push(
            run(
                point(middle, class.at.y + ANNOTATION_DROP),
                &format!("<<{}>>", class.annotation),
                ANNOTATION_FONT,
                ANNOTATION_WEIGHT,
                Anchor::Middle,
                true,
            )
            .classed("class-annotation"),
        );
        name_y += NAME_DROP;
    }
    out.extend(centred(
        point(middle, name_y),
        &class.label,
        NAME_FONT,
        NAME_WEIGHT,
        "class-name",
    ));
    out
}

/// One class box: the outline, the header band, the two rules, and the text.
fn class_group(class: &PlacedClass) -> Node {
    let (x, y) = (class.at.x, class.at.y);
    let right = x + class.width;
    let attributes_top = y + class.parts.header;
    let methods_top = attributes_top + class.parts.attributes;
    let mut children = vec![
        rect(class.at, class.width, class.height),
        rect(class.at, class.width, class.parts.header).classed("class-header"),
        rule(attributes_top, x, right),
        rule(methods_top, x, right),
    ];
    children.extend(header_text(class));
    children.extend(compartment(&class.attributes, x, attributes_top));
    children.extend(compartment(&class.methods, x, methods_top));
    let mut group = Node::new(Role::Node, Content::Group(children))
        .classed("node")
        .classed("class-node")
        .with_id(class.id.clone())
        .tagged("label", class.label.clone());
    if !class.annotation.is_empty() {
        group = group.tagged("annotation", class.annotation.clone());
    }
    group
}

/// The multiplicity written at one end, placed off the line.
fn cardinality(text: &str, from: Point, towards: Point) -> Vec<Node> {
    centred(
        cardinality_at(from, towards),
        text,
        LABEL_FONT,
        LABEL_WEIGHT,
        "class-edge-label",
    )
}

/// Both multiplicities of a relationship, each beside the end it belongs to.
fn cardinalities(rel: &PlacedRelationship) -> Vec<Node> {
    let mut out = Vec::new();
    let count = rel.points.len();
    if count < 2 {
        return out;
    }
    if !rel.from_cardinality.is_empty() {
        if let (Some(first), Some(second)) = (rel.points.first(), rel.points.get(1)) {
            out.extend(cardinality(&rel.from_cardinality, *first, *second));
        }
    }
    if !rel.to_cardinality.is_empty() {
        if let (Some(last), Some(before)) = (rel.points.last(), rel.points.get(count - 2)) {
            out.extend(cardinality(&rel.to_cardinality, *last, *before));
        }
    }
    out
}

fn relationship_group(rel: &PlacedRelationship, id: usize) -> Node {
    let marker = rel.kind.marker().to_string();
    let paint = match rel.marker_at {
        End::From => Paint {
            marker_start: Some(marker),
            ..Paint::default()
        },
        End::To => Paint {
            marker_end: Some(marker),
            ..Paint::default()
        },
    };
    let children = vec![Node::new(
        Role::Edge,
        Content::Shape(Shape::Polyline(rel.points.clone())),
    )
    .painted(paint)];
    Node::new(Role::Edge, Content::Group(children))
        .classed("edge")
        .classed("class-relationship")
        .classed(format!("class-{}", rel.kind.token()))
        .tagged("from", rel.from.clone())
        .tagged("to", rel.to.clone())
        .tagged("type", rel.kind.token())
        // Names which label belongs to this line; see `crate::hover`.
        .tagged(crate::hover::PAIR, id.to_string())
}

/// A relationship's label and its multiplicities, as their own group.
///
/// Kept out of the line's group and drawn after the boxes. A layer set on a
/// child decides nothing about paint order — only a top-level node's does — so
/// a label inside the line's group is painted with the line, which is behind
/// every box. That is how a label ends up under the box it sits next to.
fn label_group(rel: &PlacedRelationship, id: usize) -> Option<Node> {
    let mut children = Vec::new();
    if let Some(at) = rel.label_at {
        children.extend(centred(
            at,
            &rel.label,
            LABEL_FONT,
            LABEL_WEIGHT,
            "class-edge-label",
        ));
    }
    children.extend(cardinalities(rel));
    if children.is_empty() {
        return None;
    }
    Some(
        Node::new(Role::Label, Content::Group(children))
            .classed("class-edge-text")
            .tagged("from", rel.from.clone())
            .tagged("to", rel.to.clone())
            .tagged(crate::hover::PAIR, id.to_string())
            .on(Layer::Label),
    )
}

/// The ink every marker is drawn in.
fn arrow_ink() -> Color {
    Color::Token {
        name: "_arrow".into(),
        fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
    }
}

/// A marker whose inside is the page, so the line behind it does not show
/// through: the hollow triangle and the hollow diamond.
fn hollow(theme: &Theme) -> Color {
    Color::Token {
        name: "_group-fill".into(),
        fallback: theme.bg.clone(),
    }
}

/// One marker, on its own grid.
///
/// `ref_x` is the full width for every one of them, which puts the tip of the
/// glyph on the endpoint and the body of it along the line. The renderer this
/// replaces used nought for the two diamonds, which drew them a full marker's
/// width inside the box they were meant to touch.
fn marker(id: &str, width: f64, height: f64, shape: Shape, paint: Paint) -> Marker {
    Marker {
        id: id.to_string(),
        view: size(width, height),
        size: size(width, height),
        ref_x: width,
        ref_y: height / 2.0,
        shape,
        paint,
    }
}

/// The hollow triangle of inheritance and realization.
fn inherit_marker(theme: &Theme) -> Marker {
    marker(
        INHERIT_MARKER,
        MARK_W,
        MARK_H,
        Shape::Polygon(vec![
            point(0.0, 0.0),
            point(MARK_W, MARK_H / 2.0),
            point(0.0, MARK_H),
        ]),
        Paint {
            fill: Some(hollow(theme)),
            stroke: Some(arrow_ink()),
            stroke_width: Some(1.5),
            ..Paint::default()
        },
    )
}

/// The diamond of composition and aggregation, filled or hollow.
fn diamond_marker(id: &str, fill: Color, width: f64) -> Marker {
    marker(
        id,
        MARK_W,
        MARK_H,
        Shape::Polygon(vec![
            point(MARK_W / 2.0, 0.0),
            point(MARK_W, MARK_H / 2.0),
            point(MARK_W / 2.0, MARK_H),
            point(0.0, MARK_H / 2.0),
        ]),
        Paint {
            fill: Some(fill),
            stroke: Some(arrow_ink()),
            stroke_width: Some(width),
            ..Paint::default()
        },
    )
}

/// The open arrow of association and dependency.
fn arrow_marker() -> Marker {
    marker(
        ARROW_MARKER,
        ARROW_W,
        ARROW_H,
        Shape::Polyline(vec![
            point(0.0, 0.0),
            point(ARROW_W, ARROW_H / 2.0),
            point(0.0, ARROW_H),
        ]),
        Paint {
            fill: Some(Color::None),
            stroke: Some(arrow_ink()),
            stroke_width: Some(1.5),
            ..Paint::default()
        },
    )
}

/// Only the markers the drawing actually uses, in a fixed order.
fn markers(placed: &Placed, theme: &Theme) -> Vec<Marker> {
    let wanted = |id: &str| {
        placed
            .relationships
            .iter()
            .any(|rel| rel.kind.marker() == id && rel.points.len() >= 2)
    };
    let mut out = Vec::new();
    if wanted(INHERIT_MARKER) {
        out.push(inherit_marker(theme));
    }
    if wanted(COMPOSITION_MARKER) {
        out.push(diamond_marker(COMPOSITION_MARKER, arrow_ink(), 1.0));
    }
    if wanted(AGGREGATION_MARKER) {
        out.push(diamond_marker(AGGREGATION_MARKER, hollow(theme), 1.5));
    }
    if wanted(ARROW_MARKER) {
        out.push(arrow_marker());
    }
    out
}

/// Draw a placed class diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.markers = markers(placed, theme);
    let drawn: Vec<&PlacedRelationship> = placed
        .relationships
        .iter()
        .filter(|rel| rel.points.len() >= 2)
        .collect();
    for (id, rel) in drawn.iter().enumerate() {
        out.push(relationship_group(rel, id));
    }
    for class in &placed.classes {
        out.push(class_group(class));
    }
    // Last, so a box never covers the label beside it.
    let mut labelled: Vec<usize> = Vec::new();
    for (id, rel) in drawn.iter().enumerate() {
        if let Some(node) = label_group(rel, id) {
            out.push(node);
            labelled.push(id);
        }
    }
    // Only the lines that ended up with something written on them are paired.
    out.style = super::style::style(theme, mode, &labelled);
    out
}

/// Parse, lay out and draw in one step.
pub fn render(source: &str, theme: &Theme, mode: &ColorMode) -> Scene {
    scene(&layout(&super::parse(source)), theme, mode)
}

/// Every relationship kind, so a caller can ask what a marker is for.
///
/// Only the tests use this, but it is the one place the four marker ids are
/// listed together, and a fifth relationship added without a marker would come
/// out of here rather than out of a drawing.
#[cfg(test)]
const KINDS: [super::types::Relation; 6] = [
    super::types::Relation::Inheritance,
    super::types::Relation::Composition,
    super::types::Relation::Aggregation,
    super::types::Relation::Association,
    super::types::Relation::Dependency,
    super::types::Relation::Realization,
];

#[cfg(test)]
mod tests {
    use super::super::types::Relation;
    use super::*;

    fn drawn(source: &str) -> Scene {
        render(source, &Theme::default(), &ColorMode::Tokens)
    }

    fn flatten(nodes: &[Node], out: &mut Vec<Node>) {
        for node in nodes {
            out.push(node.clone());
            if let Content::Group(children) = &node.content {
                flatten(children, out);
            }
        }
    }

    fn every(scene: &Scene) -> Vec<Node> {
        let mut out = Vec::new();
        flatten(&scene.nodes, &mut out);
        out
    }

    fn texts(scene: &Scene) -> Vec<String> {
        every(scene)
            .into_iter()
            .filter_map(|node| match node.content {
                Content::Text(text) => Some(text.content),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_class_box_carries_the_name_it_was_declared_with() {
        let scene = drawn("classDiagram\n  class Animal {\n    +String name\n  }");
        let group = scene.nodes.first().expect("a box");
        assert_eq!(group.id.as_deref(), Some("Animal"));
        assert!(group.class.iter().any(|c| c == "class-node"));
        assert!(group
            .data
            .iter()
            .any(|(key, value)| key == "label" && value == "Animal"));
    }

    #[test]
    fn a_class_box_has_three_compartments_divided_by_two_rules() {
        let scene = drawn("classDiagram\n  class A {\n    +int x\n    +go() void\n  }");
        let rules = every(&scene)
            .into_iter()
            .filter(|node| node.class.iter().any(|c| c == "class-rule"))
            .count();
        assert_eq!(rules, 2);
        // The outline and the header band.
        let rects = every(&scene)
            .into_iter()
            .filter(|node| matches!(node.content, Content::Shape(Shape::Rect { .. })))
            .count();
        assert_eq!(rects, 2);
    }

    #[test]
    fn a_member_is_drawn_as_its_parts_left_to_right() {
        let scene = drawn("classDiagram\n  class A {\n    +String name\n  }");
        let written = texts(&scene);
        assert!(written.contains(&"+ ".to_string()), "{written:?}");
        assert!(written.contains(&"name".to_string()), "{written:?}");
        assert!(written.contains(&": ".to_string()), "{written:?}");
        assert!(written.contains(&"String".to_string()), "{written:?}");
    }

    #[test]
    fn the_parts_of_a_member_do_not_sit_on_top_of_each_other() {
        let scene = drawn("classDiagram\n  class A {\n    +String name\n  }");
        let xs: Vec<f64> = every(&scene)
            .into_iter()
            .filter_map(|node| match node.content {
                Content::Text(text)
                    if node.class.iter().any(|c| c == "class-member")
                        && text.anchor == Anchor::Start =>
                {
                    Some(text.at.x)
                }
                _ => None,
            })
            .collect();
        assert_eq!(xs.len(), 4);
        for pair in xs.windows(2) {
            let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            assert!(b > a, "runs run backwards: {xs:?}");
        }
    }

    #[test]
    fn a_stereotype_is_drawn_above_the_name_in_its_own_brackets() {
        let scene = drawn("classDiagram\n  class Shape {\n    <<abstract>>\n  }");
        assert!(texts(&scene).contains(&"<<abstract>>".to_string()));
        let annotation = every(&scene)
            .into_iter()
            .find(|node| node.class.iter().any(|c| c == "class-annotation"))
            .expect("a stereotype");
        let Content::Text(text) = &annotation.content else {
            panic!("text")
        };
        assert!(text.font.italic);
        let name = every(&scene)
            .into_iter()
            .find(|node| node.class.iter().any(|c| c == "class-name"))
            .expect("a name");
        let Content::Text(named) = &name.content else {
            panic!("text")
        };
        assert!(named.at.y > text.at.y, "the name sits under the stereotype");
        // And the group says so, for anything reading the drawing back.
        assert!(scene
            .nodes
            .first()
            .expect("a box")
            .data
            .iter()
            .any(|(key, value)| key == "annotation" && value == "abstract"));
    }

    #[test]
    fn a_class_with_no_stereotype_does_not_claim_one() {
        let scene = drawn("classDiagram\n  class A");
        assert!(!scene
            .nodes
            .first()
            .expect("a box")
            .data
            .iter()
            .any(|(key, _)| key == "annotation"));
    }

    #[test]
    fn a_relationship_says_what_it_joins_and_what_kind_it_is() {
        let scene = drawn("classDiagram\n  Animal <|-- Dog");
        let edge = scene
            .nodes
            .iter()
            .find(|node| node.class.iter().any(|c| c == "edge"))
            .expect("a line");
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "from" && value == "Animal"));
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "to" && value == "Dog"));
        assert!(edge.class.iter().any(|c| c == "class-inheritance"));
    }

    #[test]
    fn the_marker_sits_on_the_end_the_arrow_was_written_towards() {
        // `<|--` puts the triangle on the class being inherited from.
        let from = drawn("classDiagram\n  Animal <|-- Dog");
        let line = every(&from)
            .into_iter()
            .find(|node| matches!(node.content, Content::Shape(Shape::Polyline(_))))
            .expect("a line");
        assert_eq!(line.paint.marker_start.as_deref(), Some(INHERIT_MARKER));
        assert_eq!(line.paint.marker_end, None);
        // Written the other way round it moves to the other end.
        let to = drawn("classDiagram\n  Dog --|> Animal");
        let line = every(&to)
            .into_iter()
            .find(|node| matches!(node.content, Content::Shape(Shape::Polyline(_))))
            .expect("a line");
        assert_eq!(line.paint.marker_end.as_deref(), Some(INHERIT_MARKER));
        assert_eq!(line.paint.marker_start, None);
    }

    #[test]
    fn only_the_markers_the_drawing_uses_are_defined() {
        let one = drawn("classDiagram\n  A <|-- B");
        assert_eq!(one.markers.len(), 1);
        assert_eq!(
            one.markers.first().map(|m| m.id.clone()),
            Some(INHERIT_MARKER.to_string())
        );
        let all = drawn(
            "classDiagram\n  A <|-- B\n  C *-- D\n  E o-- F\n  G --> H\n  I ..> J\n  K ..|> L",
        );
        // Six relationships, four markers between them.
        assert_eq!(all.markers.len(), 4);
        let none = drawn("classDiagram\n  class A");
        assert!(none.markers.is_empty());
    }

    #[test]
    fn every_marker_points_at_its_endpoint_rather_than_past_it() {
        let all = drawn(
            "classDiagram\n  A <|-- B\n  C *-- D\n  E o-- F\n  G --> H\n  I ..> J\n  K ..|> L",
        );
        for marker in &all.markers {
            assert!(
                (marker.ref_x - marker.view.width).abs() < 1e-9,
                "{} is offset from its tip",
                marker.id
            );
            assert!((marker.ref_y - marker.view.height / 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn every_kind_of_relationship_has_a_marker_defined_for_it() {
        for kind in KINDS {
            let source = format!("classDiagram\n  A {} B", arrow_for(kind));
            let scene = drawn(&source);
            assert_eq!(scene.markers.len(), 1, "{kind:?}");
            assert_eq!(
                scene.markers.first().map(|m| m.id.clone()),
                Some(kind.marker().to_string()),
                "{kind:?}"
            );
        }
    }

    /// One arrow that produces each kind, for the test above.
    fn arrow_for(kind: Relation) -> &'static str {
        match kind {
            Relation::Inheritance => "<|--",
            Relation::Composition => "*--",
            Relation::Aggregation => "o--",
            Relation::Association => "-->",
            Relation::Dependency => "..>",
            Relation::Realization => "..|>",
        }
    }

    #[test]
    fn a_dashed_relationship_is_told_apart_by_its_class() {
        let scene = drawn("classDiagram\n  A ..> B");
        let edge = scene
            .nodes
            .iter()
            .find(|node| node.class.iter().any(|c| c == "edge"))
            .expect("a line");
        assert!(edge.class.iter().any(|c| c == "class-dependency"));
        assert!(scene.style.contains(".class-dependency polyline"));
        assert!(scene.style.contains("stroke-dasharray"));
    }

    #[test]
    fn a_relationship_label_is_drawn_above_the_line() {
        let scene = drawn("classDiagram\n  Teacher --> Course : teaches");
        assert!(texts(&scene).contains(&"teaches".to_string()));
    }

    #[test]
    fn a_multiplicity_is_drawn_at_the_end_it_belongs_to() {
        let scene = drawn("classDiagram\n  Order \"1\" --> \"*\" Item");
        let written = texts(&scene);
        assert!(written.contains(&"1".to_string()), "{written:?}");
        assert!(written.contains(&"*".to_string()), "{written:?}");
    }

    #[test]
    fn a_relationship_with_no_route_is_not_drawn() {
        // Nothing routes when there is only one class, so the pair of ends
        // never resolves.
        let placed = Placed::default();
        let scene = scene(&placed, &Theme::default(), &ColorMode::Tokens);
        assert!(scene.nodes.is_empty());
        assert!(scene.markers.is_empty());
    }

    #[test]
    fn a_static_member_is_marked_for_the_underline() {
        let scene = drawn("classDiagram\n  class A {\n    +int count$\n  }");
        assert!(every(&scene)
            .into_iter()
            .any(|node| node.class.iter().any(|c| c == "class-static")));
        assert!(scene
            .style
            .contains(".class-static{text-decoration:underline}"));
    }

    #[test]
    fn an_abstract_member_is_drawn_in_italic() {
        let scene = drawn("classDiagram\n  class A {\n    +area()* double\n  }");
        assert!(every(&scene).into_iter().any(|node| matches!(
            &node.content,
            Content::Text(text) if text.font.italic && node.class.iter().any(|c| c == "class-member")
        )));
    }

    #[test]
    fn a_member_with_no_type_is_drawn_as_a_single_run() {
        let runs = member_runs(&Member {
            name: "ACTIVE".into(),
            ..Member::default()
        });
        assert_eq!(runs.len(), 1);
        assert_eq!(
            runs.first().map(|(text, _)| text.clone()),
            Some("ACTIVE".to_string())
        );
    }

    #[test]
    fn a_line_with_fewer_than_two_points_has_nowhere_to_hang_a_multiplicity() {
        let rel = PlacedRelationship {
            from: "A".into(),
            to: "B".into(),
            kind: Relation::Association,
            marker_at: End::To,
            label: String::new(),
            from_cardinality: "1".into(),
            to_cardinality: "*".into(),
            points: Vec::new(),
            label_at: None,
        };
        assert!(cardinalities(&rel).is_empty());
    }

    #[test]
    fn a_label_is_drawn_after_the_boxes_so_none_can_cover_it() {
        let scene = drawn("classDiagram\n  View --> Model : reads");
        let order = scene.painted();
        let text = order
            .iter()
            .rposition(|node| node.class.iter().any(|c| c == "class-edge-text"))
            .expect("a label");
        let last_box = order
            .iter()
            .rposition(|node| node.class.iter().any(|c| c == "class-node"))
            .expect("a box");
        assert!(text > last_box, "a box is drawn over the label");
        // And it is still attributable to the line it names.
        let node = order.get(text).expect("a label");
        assert!(node
            .data
            .iter()
            .any(|(key, value)| key == "from" && value == "View"));
    }

    #[test]
    fn only_a_labelled_relationship_pairs_its_line_with_its_label() {
        let scene = drawn("classDiagram\n  A --> B : uses");
        assert!(
            scene.style.contains("data-rel=\"0\"]:hover"),
            "{}",
            scene.style
        );
        // Both halves carry the name: the line's group and the label's.
        let named = every(&scene)
            .iter()
            .filter(|n| n.data.iter().any(|(k, v)| k == "rel" && v == "0"))
            .count();
        assert_eq!(named, 2);
        // Nothing written on it, nothing to hover, no rule.
        assert!(!drawn("classDiagram\n  A <|-- B").style.contains("data-rel"));
    }

    #[test]
    fn a_relationship_with_nothing_written_on_it_needs_no_label_group() {
        let scene = drawn("classDiagram\n  A <|-- B");
        assert!(!scene
            .nodes
            .iter()
            .any(|node| node.class.iter().any(|c| c == "class-edge-text")));
    }

    #[test]
    fn boxes_are_drawn_over_the_lines_between_them() {
        let scene = drawn("classDiagram\n  A <|-- B");
        let order = scene.painted();
        let first_box = order
            .iter()
            .position(|node| node.class.iter().any(|c| c == "class-node"));
        let first_line = order
            .iter()
            .position(|node| node.class.iter().any(|c| c == "edge"));
        assert!(first_line < first_box, "lines paint behind boxes");
    }

    #[test]
    fn the_same_source_twice_draws_the_same_thing() {
        let source = "classDiagram\n  class A {\n    +int x\n  }\n  A <|-- B\n  B --> C : uses";
        assert_eq!(drawn(source), drawn(source));
    }

    #[test]
    fn a_relationship_that_names_nothing_is_still_typed_for_the_reader() {
        let scene = drawn("classDiagram\n  A -- B");
        let edge = scene
            .nodes
            .iter()
            .find(|node| node.class.iter().any(|c| c == "edge"))
            .expect("a line");
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "type" && value == "association"));
    }

    #[test]
    fn a_relationship_uses_the_marker_its_kind_asks_for() {
        // The two hollow markers read their colour from the theme, so a diagram
        // drawn for a page and one drawn standalone get the same shape.
        let marker = inherit_marker(&Theme::default());
        assert!(matches!(marker.paint.fill, Some(Color::Token { .. })));
        let filled = diamond_marker(COMPOSITION_MARKER, arrow_ink(), 1.0);
        assert_eq!(filled.id, COMPOSITION_MARKER);
        assert!(matches!(filled.shape, Shape::Polygon(ref points) if points.len() == 4));
    }
}
