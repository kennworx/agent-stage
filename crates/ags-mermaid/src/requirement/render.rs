//! A placed requirement diagram, drawn into the scene.
//!
//! Identity contract: each box is a group carrying `data-id` and whether it is
//! a requirement or an element; each edge names both ends and its verb.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, Placed, PlacedEdge, PlacedNode, BODY_FONT, BODY_WEIGHT, EDGE_LABEL_FONT,
    EDGE_LABEL_WEIGHT, HEADER_HEIGHT, NAME_FONT, NAME_WEIGHT, ROW_HEIGHT, STEREO_FONT,
    STEREO_HEIGHT, STEREO_WEIGHT,
};

const BASELINE: &str = "0.35em";
/// The corner radius a box and its header band share.
const RADIUS: f64 = 4.0;
/// The arrowhead's length and half-width.
const ARROW_LEN: f64 = 9.0;
const ARROW_HALF: f64 = 4.0;
/// A row's text is inset from the box edge by this much.
const ROW_INSET: f64 = 10.0;

fn text(
    at: Point,
    content: &str,
    size: f64,
    weight: u32,
    italic: bool,
    anchor: Anchor,
    class: &str,
) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor,
            font: Font {
                size,
                weight,
                italic,
            },
            dy: Some(BASELINE.to_string()),
            content: content.to_string(),
        }),
    )
    .classed(class)
}

fn rect(at: Point, width: f64, height: f64, radius: f64, class: &str) -> Node {
    let node = Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at,
            size: Size { width, height },
            rx: radius,
            ry: radius,
        }),
    );
    if class.is_empty() {
        node
    } else {
        node.classed(class)
    }
}

/// Shorten `line` until it fits, marking that it was cut.
///
/// A requirement's `text:` field is prose and a box is not: cutting it with an
/// ellipsis says something was left out, where letting it overflow would say
/// nothing and look like a bug.
fn ellipsize(line: &str, max_width: f64) -> String {
    if crate::metrics::text_width(line, BODY_FONT, BODY_WEIGHT) <= max_width {
        return line.to_string();
    }
    let mut kept: Vec<char> = line.chars().collect();
    while kept.len() > 1 {
        kept.pop();
        let candidate: String = kept.iter().collect::<String>() + "…";
        if crate::metrics::text_width(&candidate, BODY_FONT, BODY_WEIGHT) <= max_width {
            return candidate;
        }
    }
    kept.iter().collect::<String>() + "…"
}

fn node_node(node: &PlacedNode) -> Node {
    let (x, y, w) = (node.at.x, node.at.y, node.width);
    let mut parts = vec![
        rect(node.at, w, node.height, RADIUS, "req-box"),
        rect(node.at, w, HEADER_HEIGHT, RADIUS, "req-header"),
        // The header is rounded on all four corners; this strip squares off the
        // bottom two so it meets the divider flush. It carries no class of its
        // own: it is the header, patched, not a thing anyone selects on.
        rect(
            Point::new(x, y + HEADER_HEIGHT - RADIUS),
            w,
            RADIUS,
            0.0,
            "",
        ),
        text(
            Point::new(x + w / 2.0, y + HEADER_HEIGHT / 2.0),
            &node.name,
            NAME_FONT,
            NAME_WEIGHT,
            false,
            Anchor::Middle,
            "req-name",
        ),
        Node::new(
            Role::Frame,
            // No class of its own, and painted from the shared line token: it
            // is the seam between the header and the body, not an element.
            Content::Shape(Shape::Line {
                a: Point::new(x, y + HEADER_HEIGHT),
                b: Point::new(x + w, y + HEADER_HEIGHT),
            }),
        ),
        text(
            Point::new(x + w / 2.0, y + HEADER_HEIGHT + STEREO_HEIGHT / 2.0 + 2.0),
            &node.stereotype,
            STEREO_FONT,
            STEREO_WEIGHT,
            true,
            Anchor::Middle,
            "req-stereotype",
        ),
    ];
    let rows_top = y + HEADER_HEIGHT + STEREO_HEIGHT;
    for (index, row) in node.rows.iter().enumerate() {
        parts.push(text(
            Point::new(
                x + ROW_INSET,
                rows_top + crate::round::count(index) * ROW_HEIGHT + ROW_HEIGHT / 2.0,
            ),
            &ellipsize(row, w - ROW_INSET * 2.0),
            BODY_FONT,
            BODY_WEIGHT,
            false,
            Anchor::Start,
            "req-row",
        ));
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(node.id.clone())
        .tagged("kind", node.kind.token())
}

/// An arrowhead at `b`, pointing away from `a`.
fn arrow_head(a: Point, b: Point) -> Option<Node> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx.hypot(dy);
    // Two boxes at the same point give the head no direction to point in.
    if length == 0.0 {
        return None;
    }
    let (ux, uy) = (dx / length, dy / length);
    let base = Point::new(b.x - ux * ARROW_LEN, b.y - uy * ARROW_LEN);
    Some(
        Node::new(
            Role::Decoration,
            Content::Shape(Shape::Polygon(vec![
                b,
                Point::new(base.x - uy * ARROW_HALF, base.y + ux * ARROW_HALF),
                Point::new(base.x + uy * ARROW_HALF, base.y - ux * ARROW_HALF),
            ])),
        )
        .classed("req-arrow"),
    )
}

/// One relationship: a dashed run, a head, and its verb on a small plate.
fn edge_nodes(edge: &PlacedEdge, id: usize) -> Vec<Node> {
    let label = format!("«{}»", edge.kind);
    let width = crate::metrics::text_width(&label, EDGE_LABEL_FONT, EDGE_LABEL_WEIGHT) + 8.0;
    let height = EDGE_LABEL_FONT + 6.0;
    let mut out = vec![Node::new(
        Role::Edge,
        Content::Shape(Shape::Line {
            a: edge.a,
            b: edge.b,
        }),
    )
    .classed("edge")
    .classed("req-edge")
    .tagged("from", edge.from.clone())
    .tagged("to", edge.to.clone())
    .tagged("type", edge.kind.clone())];
    out.extend(arrow_head(edge.a, edge.b));
    // A plate under the verb, so it stays readable where it crosses a line.
    //
    // Decoration, not a node: it is drawn *on* the wire it labels, so every
    // labelled edge crossed its own plate and the legibility checker reported
    // each one as passing through a box. It is not something a reader annotates
    // or that a wire should route around — it is the label's own backing.
    let mut plate = rect(
        Point::new(
            edge.label_at.x - width / 2.0,
            edge.label_at.y - height / 2.0,
        ),
        width,
        height,
        2.0,
        "req-edge-label-bg",
    );
    plate.role = Role::Decoration;
    out.push(plate);
    out.push(text(
        Point::new(edge.label_at.x, edge.label_at.y),
        &label,
        EDGE_LABEL_FONT,
        EDGE_LABEL_WEIGHT,
        false,
        Anchor::Middle,
        "req-edge-label",
    ));
    // A relationship is one run — its line, head, plate and verb. Letting the
    // layers separate them would draw every line, then every head, then every
    // plate, which is the same picture in a different order and a diff nobody
    // can read.
    // Every piece carries the pair name, because none of them can be grouped —
    // see `crate::hover`.
    out.into_iter()
        .map(|node| {
            node.on(Layer::Edge)
                .tagged(crate::hover::PAIR, id.to_string())
        })
        .collect()
}

/// Draw a placed requirement diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .req-box{{fill:var(--_node-fill,var(--ags-bg));stroke:var(--_line);stroke-width:1}}\
         .req-header{{fill:var(--_group-hdr);stroke:none}}\
         .req-name{{fill:var(--_text)}}\
         .req-stereotype{{fill:var(--_text-sec)}}\
         .req-row{{fill:var(--_text)}}\
         .req-edge{{stroke:var(--_line);stroke-width:1;stroke-dasharray:5 4;fill:none}}\
         .req-arrow{{fill:var(--_line)}}\
         .req-edge-label-bg{{fill:var(--ags-bg);stroke:var(--_inner-stroke);stroke-width:0.5}}\
         .req-edge-label{{fill:var(--_text-sec)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{}",
        style_block(theme, mode),
        // Every relationship here carries its verb, so every one is paired.
        crate::hover::pairs(&(0..placed.edges.len()).collect::<Vec<usize>>())
    );
    for (id, edge) in placed.edges.iter().enumerate() {
        for node in edge_nodes(edge, id) {
            out.push(node);
        }
    }
    for node in &placed.nodes {
        out.push(node_node(node));
    }
    out
}

/// Parse, lay out and draw in one step.
pub fn render(source: &str, theme: &Theme, mode: &ColorMode) -> Scene {
    scene(&layout(&super::parse(source)), theme, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIAGRAM: &str = "requirementDiagram\n\
        requirement test_req {\nid: 1\ntext: the test text.\n}\n\
        element test_entity {\ntype: simulation\n}\n\
        test_entity - satisfies -> test_req";

    fn drawn(source: &str) -> Scene {
        render(source, &Theme::default(), &ColorMode::Tokens)
    }

    fn flatten(nodes: &[&Node], out: &mut Vec<Node>) {
        for node in nodes {
            out.push((*node).clone());
            if let Content::Group(children) = &node.content {
                flatten(&children.iter().collect::<Vec<_>>(), out);
            }
        }
    }

    fn all(scene: &Scene) -> Vec<Node> {
        let mut out = Vec::new();
        flatten(&scene.painted(), &mut out);
        out
    }

    fn with_class<'a>(nodes: &'a [Node], class: &str) -> Vec<&'a Node> {
        nodes
            .iter()
            .filter(|n| n.class.iter().any(|c| c == class))
            .collect()
    }

    #[test]
    fn every_box_is_addressable_and_says_what_it_is() {
        let nodes = all(&drawn(DIAGRAM));
        let boxes = with_class(&nodes, "node");
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].id.as_deref(), Some("test_req"));
        assert!(boxes[0]
            .data
            .contains(&("kind".into(), "requirement".into())));
        assert!(boxes[1].data.contains(&("kind".into(), "element".into())));
    }

    #[test]
    fn a_box_draws_a_header_a_divider_a_stereotype_and_its_rows() {
        let nodes = all(&drawn(DIAGRAM));
        assert_eq!(with_class(&nodes, "req-box").len(), 2);
        assert_eq!(with_class(&nodes, "req-header").len(), 2);
        // The divider and the header's foot carry no class of their own —
        // they are parts of a box, not things anyone selects on.
        let unclassed = nodes.iter().filter(|n| n.class.is_empty()).count();
        assert_eq!(unclassed, 4, "two dividers and two header feet");
        assert_eq!(with_class(&nodes, "req-stereotype").len(), 2);
        assert_eq!(with_class(&nodes, "req-row").len(), 3, "id, text, type");
    }

    #[test]
    fn a_stereotype_is_drawn_in_italic() {
        let nodes = all(&drawn(DIAGRAM));
        let stereo = with_class(&nodes, "req-stereotype")[0];
        assert!(matches!(&stereo.content, Content::Text(run) if run.font.italic));
    }

    #[test]
    fn an_edge_names_both_ends_and_its_verb() {
        let nodes = all(&drawn(DIAGRAM));
        let edges = with_class(&nodes, "req-edge");
        assert_eq!(edges.len(), 1);
        assert!(edges[0].data.contains(&("type".into(), "satisfies".into())));
        assert_eq!(with_class(&nodes, "req-arrow").len(), 1);
        assert_eq!(with_class(&nodes, "req-edge-label-bg").len(), 1);
    }

    #[test]
    fn the_plate_under_a_verb_is_decoration_rather_than_a_box() {
        // It sits on the wire it labels, so as a node every labelled edge would
        // be reported as passing through a box — four of them were.
        let nodes = all(&drawn(DIAGRAM));
        let plate = with_class(&nodes, "req-edge-label-bg")[0];
        assert_eq!(plate.role, Role::Decoration);
    }

    #[test]
    fn an_edge_verb_is_written_in_guillemets() {
        let nodes = all(&drawn(DIAGRAM));
        let Content::Text(run) = &with_class(&nodes, "req-edge-label")[0].content else {
            panic!("text")
        };
        assert_eq!(run.content, "«satisfies»");
    }

    #[test]
    fn a_row_too_wide_for_its_box_is_cut_rather_than_left_to_overflow() {
        let long = "x".repeat(200);
        let out = ellipsize(&format!("text: {long}"), 100.0);
        assert!(out.ends_with('…'));
        assert!(crate::metrics::text_width(&out, BODY_FONT, BODY_WEIGHT) <= 100.0);
        // Something that fits is left exactly as written.
        assert_eq!(ellipsize("id: 1", 100.0), "id: 1");
    }

    #[test]
    fn edges_paint_behind_the_boxes_they_join() {
        let scene = drawn(DIAGRAM);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        let first_box = order.iter().position(|c| *c == "node").expect("a box");
        assert!(order.iter().take(first_box).all(|c| *c != "node"));
    }

    #[test]
    fn a_diagram_of_nothing_still_yields_a_canvas() {
        let scene = drawn("requirementDiagram");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(DIAGRAM, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
