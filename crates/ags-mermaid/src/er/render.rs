//! A placed ER diagram, drawn into the scene.
//!
//! Identity contract: each box is a group carrying `data-id`, and each line a
//! group carrying `data-from` and `data-to`.
//!
//! The crow's feet are drawn from plain lines rather than from SVG markers. A
//! marker is one shape scaled by the stroke width; a foot is up to five shapes
//! whose spacing has to stay fixed however the line is drawn, and whose ring
//! has to be filled with the page rather than with ink.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{
    badge_width, layout, Foot, Placed, PlacedEntity, PlacedRelationship, KEY_FONT, KEY_WEIGHT,
    LABEL_FONT, LABEL_WEIGHT, LINE_HEIGHT, NAME_FONT, NAME_WEIGHT, RING_RADIUS, ROW_FONT,
    ROW_HEIGHT, ROW_WEIGHT,
};
use super::types::Attribute;

const BASELINE: &str = "0.35em";
/// How far a row's text sits in from the edge of its box.
const ROW_PAD: f64 = 8.0;
/// How far the badge sits in, and how far the type sits past it.
const BADGE_PAD: f64 = 6.0;
/// The height of the tint behind an attribute's keys.
const BADGE_HEIGHT: f64 = 14.0;
const BADGE_CORNER: f64 = 2.0;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(at: Point, width: f64, height: f64, radius: f64) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at,
            size: size(width, height),
            rx: radius,
            ry: radius,
        }),
    )
}

fn line(a: Point, b: Point) -> Node {
    Node::new(Role::Node, Content::Shape(Shape::Line { a, b }))
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
        .map(|(index, text)| {
            run(
                Point::new(at.x, at.y + first + crate::layout::as_f64(index) * step),
                text,
                font,
                weight,
                Anchor::Middle,
                false,
            )
            .classed(class)
        })
        .collect()
}

/// The tint and the letters of an attribute's keys.
fn badge(attribute: &Attribute, x: f64, y: f64) -> Vec<Node> {
    let width = badge_width(attribute);
    if width <= 0.0 {
        return Vec::new();
    }
    vec![
        rect(
            point(x, y - BADGE_HEIGHT / 2.0),
            width,
            BADGE_HEIGHT,
            BADGE_CORNER,
        )
        .classed("er-badge"),
        run(
            point(x + width / 2.0, y),
            &attribute.badge(),
            KEY_FONT,
            KEY_WEIGHT,
            Anchor::Middle,
            false,
        )
        .classed("er-key"),
    ]
}

/// One column: its keys, its type at the left, its name at the right.
///
/// The two ends rather than one run, because a reader scanning an entity for a
/// column name is scanning one edge of the box, not the middle of every line.
fn column(attribute: &Attribute, at: Point, width: f64) -> Node {
    let keys = badge_width(attribute);
    let mut children = badge(attribute, at.x + BADGE_PAD, at.y);
    let type_x = at.x + ROW_PAD + if keys > 0.0 { keys + BADGE_PAD } else { 0.0 };
    children.push(
        run(
            point(type_x, at.y),
            &attribute.kind,
            ROW_FONT,
            ROW_WEIGHT,
            Anchor::Start,
            false,
        )
        .classed("er-mono")
        .classed("er-type"),
    );
    children.push(
        run(
            point(at.x + width - ROW_PAD, at.y),
            &attribute.name,
            ROW_FONT,
            ROW_WEIGHT,
            Anchor::End,
            false,
        )
        .classed("er-mono")
        .classed("er-column"),
    );
    let node = Node::new(Role::Node, Content::Group(children)).classed("er-row");
    if attribute.comment.is_empty() {
        return node;
    }
    node.titled(attribute.comment.clone())
}

/// Every column of an entity, or a note that it has none.
fn columns(entity: &PlacedEntity, top: f64) -> Vec<Node> {
    if entity.attributes.is_empty() {
        return vec![run(
            point(entity.at.x + entity.width / 2.0, top + ROW_HEIGHT / 2.0),
            "(no attributes)",
            ROW_FONT,
            ROW_WEIGHT,
            Anchor::Middle,
            true,
        )
        .classed("er-empty")];
    }
    entity
        .attributes
        .iter()
        .enumerate()
        .map(|(index, attribute)| {
            let y = top + crate::layout::as_f64(index) * ROW_HEIGHT + ROW_HEIGHT / 2.0;
            column(attribute, point(entity.at.x, y), entity.width)
        })
        .collect()
}

/// One entity box: the outline, the header band, the rule and the columns.
fn entity_group(entity: &PlacedEntity) -> Node {
    let top = entity.at.y + entity.header;
    let mut children = vec![
        rect(entity.at, entity.width, entity.height, 0.0),
        rect(entity.at, entity.width, entity.header, 0.0).classed("er-header"),
        line(
            point(entity.at.x, top),
            point(entity.at.x + entity.width, top),
        )
        .classed("er-rule"),
    ];
    children.extend(centred(
        point(
            entity.at.x + entity.width / 2.0,
            entity.at.y + entity.header / 2.0,
        ),
        &entity.label,
        NAME_FONT,
        NAME_WEIGHT,
        "er-name",
    ));
    children.extend(columns(entity, top));
    Node::new(Role::Node, Content::Group(children))
        .classed("node")
        .classed("entity")
        .with_id(entity.id.clone())
        .tagged("label", entity.label.clone())
}

/// The bars, foot and ring at one end of a line.
///
/// Decoration rather than node or edge, and deliberately. A box would have the
/// checker report every line as passing through a foot it belongs to; a
/// connector would have it report the two bars of "exactly one" — parallel and
/// four pixels apart — as one line drawn twice.
fn foot_nodes(foot: &Foot) -> Vec<Node> {
    let mut out: Vec<Node> = foot
        .bars
        .iter()
        .chain(&foot.toes)
        .map(|(a, b)| {
            Node::new(
                Role::Decoration,
                Content::Shape(Shape::Line { a: *a, b: *b }),
            )
            .classed("er-foot")
        })
        .collect();
    if let Some(centre) = foot.ring {
        out.push(
            Node::new(
                Role::Decoration,
                Content::Shape(Shape::Circle {
                    c: centre,
                    r: RING_RADIUS,
                }),
            )
            .classed("er-ring"),
        );
    }
    out
}

/// One relationship: the line, and a crow's foot at each end.
fn relationship_group(rel: &PlacedRelationship, id: usize) -> Node {
    let mut children = vec![Node::new(
        Role::Edge,
        Content::Shape(Shape::Polyline(rel.points.clone())),
    )];
    for foot in &rel.feet {
        children.extend(
            foot_nodes(foot)
                .into_iter()
                .map(|node| node.on(Layer::Node)),
        );
    }
    let mut group = Node::new(Role::Edge, Content::Group(children))
        .classed("edge")
        .classed("er-relationship")
        .tagged("from", rel.from.clone())
        .tagged("to", rel.to.clone())
        .tagged("from-cardinality", rel.from_cardinality.token())
        .tagged("to-cardinality", rel.to_cardinality.token())
        // Names which verb belongs to this line; see `crate::hover`.
        .tagged(crate::hover::PAIR, id.to_string());
    if !rel.identifying {
        group = group.classed("er-optional");
    }
    group
}

/// A relationship's verb, beside the line it names.
///
/// Beside it, and with nothing painted behind it. The verb used to sit on the
/// line on a filled pill, which hid the line where the two met — and with it,
/// whether a second line crossed there or stopped. A reader cannot tell a
/// junction from a break if something is painted over both.
///
/// Drawn after the boxes rather than with the line, because a box laid over a
/// label is the one thing here that makes a diagram unreadable rather than
/// merely untidy.
fn label_group(rel: &PlacedRelationship, id: usize) -> Option<Node> {
    let at = rel.label_at?;
    let children = centred(at, &rel.label, LABEL_FONT, LABEL_WEIGHT, "er-label");
    Some(
        Node::new(Role::Label, Content::Group(children))
            .classed("er-verb")
            .tagged("from", rel.from.clone())
            .tagged("to", rel.to.clone())
            .tagged(crate::hover::PAIR, id.to_string())
            .on(Layer::Label),
    )
}

/// The rules an ER diagram needs on top of the shared tokens.
fn style(theme: &Theme, mode: &ColorMode, labelled: &[usize]) -> String {
    format!(
        "{}\
         .entity rect{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .entity .er-header{{fill:var(--_group-hdr)}}\
         .entity .er-badge{{fill:var(--_key-badge);stroke:none}}\
         .er-rule{{stroke:var(--_node-stroke);stroke-width:0.75}}\
         .er-name{{fill:var(--_text)}}\
         .er-key{{fill:var(--_text-sec)}}\
         .er-type{{fill:var(--_text-muted)}}\
         .er-column{{fill:var(--_text-sec)}}\
         .er-empty{{fill:var(--_text-faint)}}\
         .er-mono{{font-family:'JetBrains Mono','SF Mono','Fira Code',ui-monospace,monospace}}\
         .er-relationship polyline{{fill:none;stroke:var(--_line);stroke-width:1}}\
         .er-optional polyline{{stroke-dasharray:6 4}}\
         .er-foot{{stroke:var(--_line);stroke-width:1.25}}\
         .er-ring{{fill:var(--_group-fill);stroke:var(--_line);stroke-width:1.25}}\
         .er-label{{fill:var(--_text-muted)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{}",
        style_block(theme, mode),
        crate::hover::pairs(labelled)
    )
}

/// Draw a placed ER diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    let drawn: Vec<&PlacedRelationship> = placed
        .relationships
        .iter()
        .filter(|rel| rel.points.len() >= 2)
        .collect();
    for (id, rel) in drawn.iter().enumerate() {
        out.push(relationship_group(rel, id));
    }
    for entity in &placed.entities {
        out.push(entity_group(entity));
    }
    let mut labelled: Vec<usize> = Vec::new();
    for (id, rel) in drawn.iter().enumerate() {
        if let Some(node) = label_group(rel, id) {
            out.push(node);
            labelled.push(id);
        }
    }
    // Only the lines that ended up with a verb on them are paired.
    out.style = style(theme, mode, &labelled);
    out
}

/// Parse, lay out and draw in one step.
pub fn render(source: &str, theme: &Theme, mode: &ColorMode) -> Scene {
    scene(&layout(&super::parse(source)), theme, mode)
}

#[cfg(test)]
mod tests {
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

    fn with_class<'a>(nodes: &'a [Node], class: &str) -> Vec<&'a Node> {
        nodes
            .iter()
            .filter(|node| node.class.iter().any(|name| name == class))
            .collect()
    }

    #[test]
    fn an_entity_box_carries_the_name_it_was_declared_with() {
        let scene = drawn("erDiagram\n  CUSTOMER {\n    int id PK\n  }");
        let group = scene.nodes.first().expect("a box");
        assert_eq!(group.id.as_deref(), Some("CUSTOMER"));
        assert!(group.class.iter().any(|c| c == "entity"));
        assert!(group
            .data
            .iter()
            .any(|(key, value)| key == "label" && value == "CUSTOMER"));
        assert!(texts(&scene).contains(&"CUSTOMER".to_string()));
    }

    #[test]
    fn a_column_puts_its_type_at_one_edge_and_its_name_at_the_other() {
        let scene = drawn("erDiagram\n  A {\n    varchar email\n  }");
        let nodes = every(&scene);
        let kind = with_class(&nodes, "er-type")
            .first()
            .copied()
            .expect("a type");
        let name = with_class(&nodes, "er-column")
            .first()
            .copied()
            .expect("a name");
        let (Content::Text(kind), Content::Text(name)) = (&kind.content, &name.content) else {
            panic!("text")
        };
        assert_eq!(kind.content, "varchar");
        assert_eq!(name.content, "email");
        assert_eq!(kind.anchor, Anchor::Start);
        assert_eq!(name.anchor, Anchor::End);
        assert!(name.at.x > kind.at.x);
        // Both on the same row.
        assert!((name.at.y - kind.at.y).abs() < 1e-9);
    }

    #[test]
    fn a_key_gets_a_badge_and_pushes_the_type_along() {
        let plain = drawn("erDiagram\n  A {\n    int id\n  }");
        let keyed = drawn("erDiagram\n  A {\n    int id PK\n  }");
        assert!(with_class(&every(&plain), "er-badge").is_empty());
        assert_eq!(with_class(&every(&keyed), "er-badge").len(), 1);
        assert!(texts(&keyed).contains(&"PK".to_string()));
        let at = |scene: &Scene| {
            let nodes = every(scene);
            let node = with_class(&nodes, "er-type")
                .first()
                .copied()
                .expect("a type");
            let Content::Text(text) = &node.content else {
                panic!("text")
            };
            text.at.x
        };
        assert!(at(&keyed) > at(&plain), "the badge takes room at the left");
    }

    #[test]
    fn a_note_on_a_column_is_hover_text_rather_than_another_row() {
        let scene = drawn("erDiagram\n  A {\n    int id PK \"the primary key\"\n  }");
        let nodes = every(&scene);
        let row = with_class(&nodes, "er-row")
            .first()
            .copied()
            .expect("a row");
        assert_eq!(row.title.as_deref(), Some("the primary key"));
        assert!(!texts(&scene).contains(&"the primary key".to_string()));
        // And a column with no note carries none.
        let plain = drawn("erDiagram\n  A {\n    int id\n  }");
        let plain_nodes = every(&plain);
        let row = with_class(&plain_nodes, "er-row")
            .first()
            .copied()
            .expect("a row");
        assert_eq!(row.title, None);
    }

    #[test]
    fn an_entity_with_no_columns_says_so() {
        let scene = drawn("erDiagram\n  A ||--|| B : x");
        assert!(texts(&scene).contains(&"(no attributes)".to_string()));
        let nodes = every(&scene);
        let empty = with_class(&nodes, "er-empty")
            .first()
            .copied()
            .expect("a note");
        let Content::Text(text) = &empty.content else {
            panic!("text")
        };
        assert!(text.font.italic);
    }

    #[test]
    fn a_relationship_says_what_it_joins_and_how_many_of_each() {
        let scene = drawn("erDiagram\n  CUSTOMER ||--o{ ORDER : places");
        let edge = scene
            .nodes
            .iter()
            .find(|node| node.class.iter().any(|c| c == "edge"))
            .expect("a line");
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "from" && value == "CUSTOMER"));
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "to-cardinality" && value == "zero-many"));
    }

    #[test]
    fn a_non_identifying_relationship_is_told_apart_by_its_class() {
        let solid = drawn("erDiagram\n  A ||--|| B : x");
        let dashed = drawn("erDiagram\n  A ||..|| B : x");
        let has = |scene: &Scene| {
            scene
                .nodes
                .iter()
                .any(|node| node.class.iter().any(|c| c == "er-optional"))
        };
        assert!(!has(&solid));
        assert!(has(&dashed));
        assert!(dashed.style.contains(".er-optional polyline"));
    }

    #[test]
    fn every_end_of_every_line_gets_the_glyphs_its_cardinality_asks_for() {
        let scene = drawn("erDiagram\n  A ||--o{ B : x");
        let nodes = every(&scene);
        // Exactly one at the left: two bars. Any number at the right: three
        // lines of foot and a ring.
        assert_eq!(with_class(&nodes, "er-foot").len(), 5);
        assert_eq!(with_class(&nodes, "er-ring").len(), 1);
    }

    #[test]
    fn a_cardinality_with_no_ring_draws_none() {
        let scene = drawn("erDiagram\n  A ||--|| B : x");
        assert!(with_class(&every(&scene), "er-ring").is_empty());
    }

    #[test]
    fn a_verb_is_drawn_over_the_boxes() {
        let scene = drawn("erDiagram\n  CUSTOMER ||--o{ ORDER : places");
        assert!(texts(&scene).contains(&"places".to_string()));
        // Last in the scene, so nothing is drawn over it.
        let order = scene.painted();
        let verb = order
            .iter()
            .position(|node| node.class.iter().any(|c| c == "er-verb"))
            .expect("a verb");
        let last_box = order
            .iter()
            .rposition(|node| node.class.iter().any(|c| c == "entity"))
            .expect("a box");
        assert!(verb > last_box);
    }

    #[test]
    fn a_relationship_with_no_verb_writes_nothing() {
        let scene = drawn("erDiagram\n  A ||--|| B");
        assert!(with_class(&every(&scene), "er-verb").is_empty());
    }

    #[test]
    fn nothing_is_painted_behind_a_verb() {
        // A filled pill under the verb hides the line where the two meet, and
        // with it whether a second line crosses there or stops.
        let scene = drawn("erDiagram\n  A ||--o{ B : a rather long verb");
        let nodes = every(&scene);
        let verb = with_class(&nodes, "er-verb")
            .first()
            .copied()
            .expect("a verb");
        let Content::Group(children) = &verb.content else {
            panic!("a group")
        };
        assert!(
            children
                .iter()
                .all(|node| matches!(node.content, Content::Text(_))),
            "something is drawn behind the verb"
        );
    }

    #[test]
    fn a_diagram_with_nothing_in_it_draws_nothing() {
        let scene = scene(&Placed::default(), &Theme::default(), &ColorMode::Tokens);
        assert!(scene.nodes.is_empty());
    }

    #[test]
    fn boxes_are_drawn_over_the_lines_between_them() {
        let scene = drawn("erDiagram\n  A ||--o{ B : x");
        let order = scene.painted();
        let first_box = order
            .iter()
            .position(|node| node.class.iter().any(|c| c == "entity"));
        let first_line = order
            .iter()
            .position(|node| node.class.iter().any(|c| c == "edge"));
        assert!(first_line < first_box);
    }

    #[test]
    fn the_same_source_twice_draws_the_same_thing() {
        let source = "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  ORDER {\n    int id PK\n  }";
        assert_eq!(drawn(source), drawn(source));
    }
}
