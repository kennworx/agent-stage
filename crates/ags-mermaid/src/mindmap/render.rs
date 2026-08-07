//! A placed mindmap, drawn into the scene.
//!
//! Identity contract: each node is a group carrying `data-id` and the shape it
//! was written as; each connector names the two it joins.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Seg, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{layout, Connector, Placed, PlacedNode, FONT, WEIGHT};
use super::types::Shape as NodeShape;

const BASELINE: &str = "0.35em";

fn text(at: Point, content: &str, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor: Anchor::Middle,
            font: Font {
                size: FONT,
                weight: WEIGHT,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: content.to_string(),
        }),
    )
    .classed(class)
}

/// The outline a node's shape calls for.
fn outline(node: &PlacedNode) -> Shape {
    let (x, y, w, h) = (node.at.x, node.at.y, node.width, node.height);
    let centre = Point::new(x + w / 2.0, y + h / 2.0);
    let rect = |radius: f64| Shape::Rect {
        at: node.at,
        size: Size {
            width: w,
            height: h,
        },
        rx: radius,
        ry: radius,
    };
    match node.shape {
        NodeShape::Circle | NodeShape::Bang => Shape::Ellipse {
            c: centre,
            rx: w / 2.0,
            ry: h / 2.0,
        },
        NodeShape::Hexagon => {
            // The corner inset is capped so a narrow node stays a hexagon
            // rather than collapsing into a diamond.
            let inset = (h / 2.0).min(w / 3.0);
            Shape::Polygon(vec![
                Point::new(x + inset, y),
                Point::new(x + w - inset, y),
                Point::new(x + w, centre.y),
                Point::new(x + w - inset, y + h),
                Point::new(x + inset, y + h),
                Point::new(x, centre.y),
            ])
        }
        // A pill: fully rounded ends.
        NodeShape::Cloud => rect(h / 2.0),
        NodeShape::Round => rect(12.0),
        NodeShape::Square => rect(2.0),
        NodeShape::Default => rect(8.0),
    }
}

fn node_node(node: &PlacedNode) -> Node {
    let is_root = node.depth == 0;
    let mut shape = Node::new(Role::Node, Content::Shape(outline(node))).classed("mm-shape");
    if is_root {
        shape = shape.classed("mm-shape-root");
    }
    if node.shape == NodeShape::Bang {
        shape = shape.classed("mm-bang");
    }
    let mut label = text(
        Point::new(node.at.x + node.width / 2.0, node.at.y + node.height / 2.0),
        &node.label,
        "mm-label",
    );
    if is_root {
        // The root is filled with the accent, so its text has to be knocked
        // out rather than inked.
        label = label.classed("mm-label-root");
    }
    Node::new(Role::Node, Content::Group(vec![shape, label]))
        .classed("node")
        .with_id(node.id.clone())
        .tagged("shape", node.shape.token())
}

/// A smooth run from one edge to the other, with horizontal tangents so a
/// branch leaves and arrives flat against the boxes it joins.
fn connector_node(run: &Connector) -> Node {
    let mid = f64::midpoint(run.a.x, run.b.x);
    Node::new(
        Role::Edge,
        Content::Shape(Shape::Path(vec![
            Seg::MoveTo(run.a),
            Seg::Cubic {
                c1: Point::new(mid, run.a.y),
                c2: Point::new(mid, run.b.y),
                to: run.b,
            },
        ])),
    )
    .classed("mm-connector")
    .tagged("from", run.from.clone())
    .tagged("to", run.to.clone())
}

/// Draw a placed mindmap.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .mm-connector{{fill:none;stroke:var(--_line);stroke-width:1.5}}\
         .mm-shape{{fill:var(--_node-fill,var(--ags-bg));stroke:var(--_node-stroke);stroke-width:1.5}}\
         .mm-shape-root{{fill:var(--ags-accent,var(--_arrow));stroke:var(--ags-accent,var(--_arrow))}}\
         .mm-bang{{stroke:var(--ags-accent,var(--_arrow));stroke-width:2}}\
         .mm-label{{fill:var(--_text)}}\
         .mm-label-root{{fill:var(--ags-bg)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );
    for run in &placed.connectors {
        out.push(connector_node(run));
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

    const MAP: &str = "mindmap\n\
        root((Centre))\n  \
          Origins\n    \
            Long history\n  \
          {{Research}}\n  \
          ))Bang((";

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
    fn every_node_is_addressable_and_says_what_shape_it_is() {
        let nodes = all(&drawn(MAP));
        let boxes = with_class(&nodes, "node");
        assert_eq!(boxes.len(), 5);
        assert_eq!(boxes[0].id.as_deref(), Some("Centre"));
        assert!(boxes[0].data.contains(&("shape".into(), "circle".into())));
    }

    #[test]
    fn each_shape_becomes_its_own_outline() {
        let nodes = all(&drawn(MAP));
        let kinds: Vec<&str> = with_class(&nodes, "mm-shape")
            .iter()
            .map(|n| match &n.content {
                Content::Shape(Shape::Ellipse { .. }) => "ellipse",
                Content::Shape(Shape::Polygon(_)) => "polygon",
                Content::Shape(Shape::Rect { .. }) => "rect",
                _ => "other",
            })
            .collect();
        // Circle root, plain child, plain grandchild, hexagon, bang.
        assert_eq!(kinds, ["ellipse", "rect", "rect", "polygon", "ellipse"]);
    }

    #[test]
    fn the_root_is_filled_and_its_text_knocked_out() {
        let nodes = all(&drawn(MAP));
        assert_eq!(with_class(&nodes, "mm-shape-root").len(), 1);
        assert_eq!(with_class(&nodes, "mm-label-root").len(), 1);
        // Every node still carries the shared classes.
        assert_eq!(with_class(&nodes, "mm-label").len(), 5);
    }

    #[test]
    fn a_bang_takes_an_extra_class_of_its_own() {
        assert_eq!(with_class(&all(&drawn(MAP)), "mm-bang").len(), 1);
    }

    #[test]
    fn connectors_paint_behind_the_boxes_and_name_both_ends() {
        let scene = drawn(MAP);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        let first_box = order.iter().position(|c| *c == "node").expect("a node");
        assert!(order.iter().take(first_box).all(|c| *c == "mm-connector"));
        let nodes = all(&scene);
        let runs = with_class(&nodes, "mm-connector");
        assert_eq!(runs.len(), 4, "one per parent-child pair");
        assert!(runs[0].data.iter().any(|(k, _)| k == "from"));
    }

    #[test]
    fn a_connector_leaves_and_arrives_flat() {
        let nodes = all(&drawn(MAP));
        let Content::Shape(Shape::Path(segs)) = &with_class(&nodes, "mm-connector")[0].content
        else {
            panic!("a cubic")
        };
        let (Some(Seg::MoveTo(from)), Some(Seg::Cubic { c1, c2, to })) =
            (segs.first(), segs.get(1))
        else {
            panic!("a move and a cubic")
        };
        assert!((c1.y - from.y).abs() < 1e-9);
        assert!((c2.y - to.y).abs() < 1e-9);
    }

    #[test]
    fn a_map_of_nothing_still_yields_a_canvas() {
        let scene = drawn("mindmap");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(MAP, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
