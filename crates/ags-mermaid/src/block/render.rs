//! A placed block diagram, drawn into the scene.
//!
//! Identity contract: each block is a group carrying `data-id` and `data-label`,
//! each wire a polyline carrying the two ids it joins. Colour lives in rules
//! selected by those two class names rather than in attributes on each shape —
//! the classes are the contract, so nothing else may be hung on the shapes.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Color, Content, Font, Marker, Node, Paint, Point, Role, Scene, Shape, Size, TextRun,
};
use crate::theme::{style_block, Theme};

use super::layout::{layout, PlacedBlock, PlacedEdge, LABEL_FONT, LABEL_WEIGHT};

const BASELINE: &str = "0.35em";
const ARROW_ID: &str = "block-arrow";
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 5.0;

fn wh(width: f64, height: f64) -> Size {
    Size { width, height }
}

/// One block: its box, and its name centred in it.
fn block_node(block: &PlacedBlock) -> Node {
    let box_shape = Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: block.at,
            size: wh(block.width, block.height),
            rx: 6.0,
            ry: 6.0,
        }),
    );
    let label = Node::new(
        Role::Label,
        Content::Text(TextRun {
            at: Point::new(
                block.at.x + block.width / 2.0,
                block.at.y + block.height / 2.0,
            ),
            anchor: Anchor::Middle,
            font: Font {
                size: LABEL_FONT,
                weight: LABEL_WEIGHT,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: block.label.clone(),
        }),
    );
    Node::new(Role::Node, Content::Group(vec![box_shape, label]))
        .classed("node")
        .with_id(block.id.clone())
        .tagged("label", block.label.clone())
}

/// One wire, from border to border, with a head at the target end.
fn edge_node(edge: &PlacedEdge) -> Node {
    Node::new(Role::Edge, Content::Shape(Shape::Polyline(edge.points())))
        .classed("edge")
        .tagged("from", edge.source.clone())
        .tagged("to", edge.target.clone())
        .painted(Paint {
            marker_end: Some(ARROW_ID.to_string()),
            ..Paint::default()
        })
}

fn arrow_marker() -> Marker {
    Marker {
        id: ARROW_ID.to_string(),
        view: wh(ARROW_W, ARROW_H),
        size: wh(ARROW_W, ARROW_H),
        // One unit short of the tip, so the head overlaps the line it caps
        // instead of leaving a hairline of background between the two.
        ref_x: ARROW_W - 1.0,
        ref_y: ARROW_H / 2.0,
        shape: Shape::Polygon(vec![
            Point::new(0.0, 0.0),
            Point::new(ARROW_W, ARROW_H / 2.0),
            Point::new(0.0, ARROW_H),
        ]),
        paint: Paint {
            fill: Some(Color::Token {
                name: "_arrow".into(),
                fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
            }),
            stroke: Some(Color::Token {
                name: "_arrow".into(),
                fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
            }),
            stroke_width: Some(0.75),
            ..Paint::default()
        },
    }
}

/// Draw a placed block diagram.
pub fn scene(placed: &super::layout::Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(wh(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .node rect{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .node text{{fill:var(--_text)}}\
         .edge{{fill:none;stroke:var(--_line);stroke-width:1}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );
    if !placed.edges.is_empty() {
        out.markers.push(arrow_marker());
    }
    for edge in &placed.edges {
        out.push(edge_node(edge));
    }
    for block in &placed.blocks {
        out.push(block_node(block));
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

    const GRID: &str = "block-beta\ncolumns 2\nA[\"Alpha\"] B[\"Beta\"]\nA --> B";

    #[test]
    fn every_block_is_addressable_and_carries_its_name() {
        let nodes = all(&drawn(GRID));
        let blocks = with_class(&nodes, "node");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id.as_deref(), Some("A"));
        assert!(blocks[0].data.contains(&("label".into(), "Alpha".into())));
    }

    #[test]
    fn a_wire_names_both_ends() {
        let nodes = all(&drawn(GRID));
        let edges = with_class(&nodes, "edge");
        assert_eq!(edges.len(), 1);
        assert!(edges[0].data.contains(&("from".into(), "A".into())));
        assert!(edges[0].data.contains(&("to".into(), "B".into())));
    }

    #[test]
    fn a_wire_paints_behind_the_blocks_it_joins() {
        let scene = drawn(GRID);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        assert_eq!(order, ["edge", "node", "node"]);
    }

    #[test]
    fn the_shapes_inside_a_block_carry_no_class_of_their_own() {
        // The rule selects `.node rect`, so a class here would be a second way
        // to say the same thing — and the reference's own markup has none.
        let nodes = all(&drawn(GRID));
        let inner: Vec<&Node> = nodes
            .iter()
            .filter(|n| !matches!(n.content, Content::Group(_)))
            .filter(|n| n.data.is_empty())
            .collect();
        assert!(inner.iter().all(|n| n.class.is_empty()), "{inner:?}");
    }

    #[test]
    fn a_head_is_defined_once_and_only_when_something_needs_it() {
        let with = drawn(GRID);
        assert_eq!(with.markers.len(), 1);
        assert_eq!(with.markers[0].id, ARROW_ID);
        assert!(drawn("block-beta\nA").markers.is_empty());
    }

    #[test]
    fn a_head_points_along_the_line_it_caps() {
        let marker = &drawn(GRID).markers[0];
        // The tip is the middle vertex; a head whose widest point led would
        // read as an arrow pointing backwards.
        assert_eq!(
            marker.shape,
            Shape::Polygon(vec![
                Point::new(0.0, 0.0),
                Point::new(ARROW_W, ARROW_H / 2.0),
                Point::new(0.0, ARROW_H),
            ])
        );
        assert!((marker.ref_y - ARROW_H / 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_label_is_centred_in_its_box() {
        let nodes = all(&drawn(GRID));
        let block = &with_class(&nodes, "node")[0].clone();
        let Content::Group(children) = &block.content else {
            panic!("a block is a group")
        };
        let (Content::Shape(Shape::Rect { at, size, .. }), Content::Text(run)) =
            (&children[0].content, &children[1].content)
        else {
            panic!("a box and its name")
        };
        assert!((run.at.x - (at.x + size.width / 2.0)).abs() < 1e-9);
        assert!((run.at.y - (at.y + size.height / 2.0)).abs() < 1e-9);
        assert_eq!(run.anchor, Anchor::Middle);
    }

    #[test]
    fn a_diagram_of_nothing_still_yields_a_canvas() {
        let scene = drawn("block-beta");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(GRID, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
