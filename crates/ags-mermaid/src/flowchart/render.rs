//! A placed flowchart, drawn into the scene.
//!
//! Identity contract: each box is a group carrying `data-id`, and each arrow a
//! group carrying `data-from` and `data-to`.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Color, Content, Font, Layer, Marker, Node, Paint, Point, Role, Scene, Seg,
    Shape as Outline, Size, TextRun,
};
use crate::theme::{style_block, Theme};

use super::config::Config;
use super::layout::{layout, PlacedEdge, PlacedGroup, PlacedNode};
use super::types::Shape;

const BASELINE: &str = "0.35em";
const ARROW_ID: &str = "flow-arrow";
/// The same head in the highlight colour, swapped in on hover.
const ARROW_HOT_ID: &str = "flow-arrow-hot";
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 5.0;
const CORNER: f64 = 5.0;
/// How far the inner rules of a subroutine sit from its ends.
const SUBROUTINE_INSET: f64 = 8.0;
/// How much smaller the inner ring of a double circle is.
const RING_INSET: f64 = 6.0;
/// The height of the ellipse capping a cylinder.
const CYLINDER_CAP: f64 = 7.0;
/// How far in the notch of an asymmetric box reaches.
const FLAG_NOTCH: f64 = 12.0;
/// How far in a trapezoid's short side is drawn.
const TRAPEZOID_INSET: f64 = 0.15;
/// The dot at the start of a state diagram.
const MARKER_DOT: f64 = 9.0;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A label, one node per line, centred about `at`.
fn text(at: Point, label: &str, font: f64, weight: u32, class: &str, cfg: &Config) -> Vec<Node> {
    let plain = crate::text::strip_formatting_tags(label);
    let lines: Vec<&str> = plain.split('\n').collect();
    let step = font * cfg.line_height;
    let first = -(crate::layout::as_f64(lines.len().saturating_sub(1)) / 2.0) * step;
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            Node::new(
                Role::Label,
                Content::Text(TextRun {
                    at: Point::new(at.x, at.y + first + crate::layout::as_f64(index) * step),
                    anchor: Anchor::Middle,
                    font: Font {
                        size: font,
                        weight,
                        italic: false,
                    },
                    dy: Some(BASELINE.to_string()),
                    content: (*line).to_string(),
                }),
            )
            .classed(class)
        })
        .collect()
}

fn rect(at: Point, width: f64, height: f64, radius: f64) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Outline::Rect {
            at,
            size: size(width, height),
            rx: radius,
            ry: radius,
        }),
    )
}

fn polygon(points: Vec<Point>) -> Node {
    Node::new(Role::Node, Content::Shape(Outline::Polygon(points)))
}

fn line(a: Point, b: Point) -> Node {
    Node::new(Role::Node, Content::Shape(Outline::Line { a, b }))
}

fn circle(centre: Point, radius: f64) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Outline::Circle {
            c: centre,
            r: radius,
        }),
    )
}

/// The shapes drawn as a rectangle, however its corners are finished.
fn box_parts(node: &PlacedNode) -> Option<Vec<Node>> {
    let (x, y) = (node.at.x, node.at.y);
    let (w, h) = (node.width, node.height);
    match node.shape {
        Shape::Rectangle => Some(vec![rect(node.at, w, h, 0.0)]),
        Shape::Rounded => Some(vec![rect(node.at, w, h, CORNER)]),
        Shape::Stadium => Some(vec![rect(node.at, w, h, h / 2.0)]),
        // A rule inside each end, which is what says "this is defined
        // elsewhere".
        Shape::Subroutine => Some(vec![
            rect(node.at, w, h, 0.0),
            line(
                point(x + SUBROUTINE_INSET, y),
                point(x + SUBROUTINE_INSET, y + h),
            ),
            line(
                point(x + w - SUBROUTINE_INSET, y),
                point(x + w - SUBROUTINE_INSET, y + h),
            ),
        ]),
        _ => None,
    }
}

/// The shapes drawn as a straight-sided outline.
fn polygon_parts(node: &PlacedNode) -> Option<Vec<Node>> {
    let (x, y) = (node.at.x, node.at.y);
    let (w, h) = (node.width, node.height);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let inset = w * TRAPEZOID_INSET;
    let corners = match node.shape {
        Shape::Diamond => vec![
            point(cx, y),
            point(x + w, cy),
            point(cx, y + h),
            point(x, cy),
        ],
        Shape::Hexagon => {
            let notch = h / 4.0;
            vec![
                point(x + notch, y),
                point(x + w - notch, y),
                point(x + w, cy),
                point(x + w - notch, y + h),
                point(x + notch, y + h),
                point(x, cy),
            ]
        }
        Shape::Asymmetric => vec![
            point(x + FLAG_NOTCH, y),
            point(x + w, y),
            point(x + w, y + h),
            point(x + FLAG_NOTCH, y + h),
            point(x, cy),
        ],
        Shape::Trapezoid => vec![
            point(x + inset, y),
            point(x + w - inset, y),
            point(x + w, y + h),
            point(x, y + h),
        ],
        Shape::TrapezoidAlt => vec![
            point(x, y),
            point(x + w, y),
            point(x + w - inset, y + h),
            point(x + inset, y + h),
        ],
        _ => return None,
    };
    Some(vec![polygon(corners)])
}

/// The shapes drawn with a curve: circles, the cylinder, and the state markers.
fn round_parts(node: &PlacedNode) -> Vec<Node> {
    let (x, y) = (node.at.x, node.at.y);
    let (w, h) = (node.width, node.height);
    let centre = point(x + w / 2.0, y + h / 2.0);
    let radius = w.min(h) / 2.0;
    match node.shape {
        Shape::DoubleCircle => vec![
            circle(centre, radius),
            circle(centre, (radius - RING_INSET).max(1.0)).classed("flow-inner"),
        ],
        // A tube with an ellipse capping each end; only the top one is drawn,
        // because the bottom of the tube already is one.
        Shape::Cylinder => vec![
            Node::new(
                Role::Node,
                Content::Shape(Outline::Path(vec![
                    Seg::MoveTo(point(x, y + CYLINDER_CAP)),
                    Seg::Arc {
                        r: size(w / 2.0, CYLINDER_CAP),
                        large: false,
                        sweep: true,
                        to: point(x + w, y + CYLINDER_CAP),
                    },
                    Seg::LineTo(point(x + w, y + h - CYLINDER_CAP)),
                    Seg::Arc {
                        r: size(w / 2.0, CYLINDER_CAP),
                        large: false,
                        sweep: true,
                        to: point(x, y + h - CYLINDER_CAP),
                    },
                    Seg::Close,
                ])),
            ),
            Node::new(
                Role::Node,
                Content::Shape(Outline::Ellipse {
                    c: point(x + w / 2.0, y + CYLINDER_CAP),
                    rx: w / 2.0,
                    ry: CYLINDER_CAP,
                }),
            )
            .classed("flow-inner"),
        ],
        Shape::StateStart => vec![circle(centre, MARKER_DOT).classed("flow-marker")],
        Shape::StateEnd => vec![
            circle(centre, MARKER_DOT),
            circle(centre, MARKER_DOT - 4.0).classed("flow-marker"),
        ],
        // Everything else that is round is a plain circle.
        _ => vec![circle(centre, radius)],
    }
}

/// The outline a shape is drawn with, and whatever else it needs inside.
fn shape_parts(node: &PlacedNode) -> Vec<Node> {
    box_parts(node)
        .or_else(|| polygon_parts(node))
        .unwrap_or_else(|| round_parts(node))
}

fn node_group(node: &PlacedNode, cfg: &Config) -> Node {
    let mut children = shape_parts(node);
    // A state marker is a dot, and a dot with a name in it is unreadable.
    if !matches!(node.shape, Shape::StateStart | Shape::StateEnd) {
        children.extend(text(
            node.centre(),
            &node.label,
            cfg.label_font,
            cfg.label_weight,
            "flow-label",
            cfg,
        ));
    }
    let mut group = Node::new(Role::Node, Content::Group(children))
        .classed("node")
        .classed(format!("flow-{}", node.shape.token()))
        .with_id(node.id.clone());
    for class in &node.classes {
        group = group.classed(format!("flow-class-{class}"));
    }
    group
}

fn edge_group(edge: &PlacedEdge, cfg: &Config) -> Node {
    let mut paint = Paint::default();
    if edge.head_end {
        paint.marker_end = Some(ARROW_ID.to_string());
    }
    if edge.head_start {
        paint.marker_start = Some(ARROW_ID.to_string());
    }
    let mut children = vec![Node::new(
        Role::Edge,
        Content::Shape(Outline::Polyline(edge.points.clone())),
    )
    .painted(paint)];
    if let Some(label) = edge.label_at {
        children.extend(
            text(
                label.at,
                &edge.label,
                cfg.edge_label_font,
                cfg.edge_label_weight,
                "flow-edge-label",
                cfg,
            )
            .into_iter()
            .map(|node| node.on(Layer::Label)),
        );
    }
    let mut group = Node::new(Role::Edge, Content::Group(children))
        .classed("edge")
        .classed(format!("flow-{}", edge.style.token()));
    // Only a wire that draws a head at its tail has a `marker-start` for the
    // highlight to swap; naming it here keeps that out of the stylesheet's way.
    if edge.head_start {
        group = group.classed("flow-head-start");
    }
    group
        .tagged("from", edge.source.clone())
        .tagged("to", edge.target.clone())
}

/// A group's box, and its name in the band at the top.
fn group_node(group: &PlacedGroup, cfg: &Config) -> Node {
    let frame = Node::new(
        Role::Frame,
        Content::Shape(Outline::Rect {
            at: group.at,
            size: size(group.width, group.height),
            rx: CORNER,
            ry: CORNER,
        }),
    );
    let name = Node::new(
        Role::Label,
        Content::Text(TextRun {
            at: point(
                group.at.x + cfg.group_label_pad_x,
                group.at.y + cfg.group_header / 2.0,
            ),
            anchor: Anchor::Start,
            font: Font {
                size: cfg.group_label_font,
                weight: cfg.group_label_weight,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: crate::text::strip_formatting_tags(&group.label),
        }),
    )
    .classed("flow-group-label");
    Node::new(Role::Frame, Content::Group(vec![frame, name]))
        .classed("group")
        .with_id(group.id.clone())
        .tagged("depth", group.depth.to_string())
        .tagged("holds", group.holds.join(" "))
        .on(Layer::Frame)
}

fn arrow_marker() -> Marker {
    let ink = Color::Token {
        name: "_arrow".into(),
        fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
    };
    head(ARROW_ID, ink)
}

/// The same head in the highlight colour, for the wire under the pointer.
///
/// A second marker rather than a recoloured one: a marker is shared by every
/// wire that references it, so the only way to light one wire's head is to point
/// that wire at a different marker — which CSS can do, and which is why the
/// highlight can stay in the stylesheet where the rest of it lives.
fn arrow_marker_hot() -> Marker {
    let ink = Color::Token {
        name: "ags-accent".into(),
        fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
    };
    head(ARROW_HOT_ID, ink)
}

fn head(id: &str, ink: Color) -> Marker {
    Marker {
        id: id.to_string(),
        view: size(ARROW_W, ARROW_H),
        size: size(ARROW_W, ARROW_H),
        // One short of the tip, so the head overlaps the line it caps rather
        // than leaving a hairline of background between the two.
        ref_x: ARROW_W - 1.0,
        ref_y: ARROW_H / 2.0,
        shape: Outline::Polygon(vec![
            point(0.0, 0.0),
            point(ARROW_W, ARROW_H / 2.0),
            point(0.0, ARROW_H),
        ]),
        paint: Paint {
            fill: Some(ink.clone()),
            stroke: Some(ink),
            stroke_width: Some(0.75),
            ..Paint::default()
        },
    }
}

/// The rules a flowchart needs on top of the shared tokens.
fn style(theme: &Theme, mode: &ColorMode) -> String {
    format!(
        "{}\
         .node rect,.node polygon,.node circle,.node ellipse,.node path\
         {{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .node line{{stroke:var(--_node-stroke);stroke-width:1}}\
         .node .flow-inner{{fill:none}}\
         .flow-marker{{fill:var(--_text);stroke:none}}\
         .flow-state-start circle{{fill:var(--_text);stroke:none}}\
         .flow-label{{fill:var(--_text)}}\
         .edge polyline{{fill:none;stroke:var(--_line);stroke-width:1.5}}\
         .flow-dotted polyline{{stroke-dasharray:5 4}}\
         .flow-thick polyline{{stroke-width:3}}\
         .flow-edge-label{{fill:var(--_text)}}\
         .edge:has(.flow-edge-label){{cursor:default}}\
         .edge:has(.flow-edge-label):hover polyline\
         {{stroke:var(--ags-accent,var(--_arrow));marker-end:url(#{ARROW_HOT_ID})}}\
         .edge:has(.flow-edge-label).flow-head-start:hover polyline\
         {{marker-start:url(#{ARROW_HOT_ID})}}\
         .edge:not(.flow-thick):has(.flow-edge-label):hover polyline{{stroke-width:2.5}}\
         .edge:has(.flow-edge-label):hover .flow-edge-label\
         {{fill:var(--ags-accent,var(--_arrow))}}\
         .group rect{{fill:var(--_group-fill);stroke:var(--_node-stroke);stroke-width:1;stroke-dasharray:4 3}}\
         .flow-group-label{{fill:var(--_text-sec)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    )
}

/// Draw a placed flowchart.
pub fn scene(
    placed: &super::layout::Placed,
    theme: &Theme,
    mode: &ColorMode,
    cfg: &Config,
) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(theme, mode);
    if placed
        .edges
        .iter()
        .any(|edge| edge.head_start || edge.head_end)
    {
        out.markers.push(arrow_marker());
        // Only a labelled wire can be hovered, so only that needs a lit head.
        if placed.edges.iter().any(|edge| edge.label_at.is_some()) {
            out.markers.push(arrow_marker_hot());
        }
    }
    for group in &placed.groups {
        out.push(group_node(group, cfg));
    }
    for edge in &placed.edges {
        if edge.points.len() >= 2 {
            out.push(edge_group(edge, cfg));
        }
    }
    for node in &placed.nodes {
        out.push(node_group(node, cfg));
    }
    out
}

/// Parse, lay out and draw in one step.
pub fn render(source: &str, theme: &Theme, mode: &ColorMode, cfg: &Config) -> Scene {
    scene(&layout(&super::read(source), cfg), theme, mode, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(source: &str) -> Scene {
        render(
            source,
            &Theme::default(),
            &ColorMode::Tokens,
            &Config::default(),
        )
    }

    fn flatten(nodes: &[&Node], out: &mut Vec<Node>) {
        for node in nodes {
            out.push((*node).clone());
            if let Content::Group(children) = &node.content {
                flatten(&children.iter().collect::<Vec<&Node>>(), out);
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
            .filter(|node| node.class.iter().any(|c| c == class))
            .collect()
    }

    #[test]
    fn every_box_is_addressable_and_named() {
        let nodes = all(&drawn("graph TD\nA[Start] --> B[End]"));
        let boxes = with_class(&nodes, "node");
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].id.as_deref(), Some("A"));
        let labels = with_class(&nodes, "flow-label");
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn every_shape_draws_the_outline_it_names() {
        for (source, class, parts) in [
            ("A[x]", "flow-rectangle", 1),
            ("A(x)", "flow-rounded", 1),
            ("A([x])", "flow-stadium", 1),
            ("A[[x]]", "flow-subroutine", 3),
            ("A{x}", "flow-diamond", 1),
            ("A((x))", "flow-circle", 1),
            ("A(((x)))", "flow-doublecircle", 2),
            ("A{{x}}", "flow-hexagon", 1),
            ("A[(x)]", "flow-cylinder", 2),
            ("A>x]", "flow-asymmetric", 1),
            ("A[/x\\]", "flow-trapezoid", 1),
            ("A[\\x/]", "flow-trapezoid-alt", 1),
        ] {
            // One node on its own, so a second box of the same shape cannot be
            // mistaken for this one.
            let nodes = all(&drawn(&format!("graph TD\n{source}")));
            let found = with_class(&nodes, class);
            assert_eq!(found.len(), 1, "{source}");
            let Content::Group(children) = &found[0].content else {
                panic!("{source}")
            };
            // The outline, whatever it needs inside it, and one line of label.
            assert_eq!(children.len(), parts + 1, "{source}");
        }
    }

    #[test]
    fn an_arrow_is_a_polyline_naming_both_ends() {
        let nodes = all(&drawn("graph TD\nA --> B"));
        let edges = with_class(&nodes, "edge");
        assert_eq!(edges.len(), 1);
        let datum = |key: &str| {
            edges[0]
                .data
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(datum("from"), Some("A"));
        assert_eq!(datum("to"), Some("B"));
        let Content::Group(children) = &edges[0].content else {
            panic!("a group")
        };
        assert!(matches!(
            children[0].content,
            Content::Shape(Outline::Polyline(_))
        ));
    }

    #[test]
    fn an_arrow_carries_a_head_only_where_it_points() {
        let nodes = all(&drawn("graph TD\nA --> B\nB --- C\nC <--> D"));
        let edges = with_class(&nodes, "edge");
        let head = |at: usize, start: bool| {
            let Content::Group(children) = &edges[at].content else {
                panic!("a group")
            };
            if start {
                children[0].paint.marker_start.is_some()
            } else {
                children[0].paint.marker_end.is_some()
            }
        };
        assert!(head(0, false) && !head(0, true));
        assert!(!head(1, false), "`---` has no head");
        assert!(head(2, false) && head(2, true));
    }

    #[test]
    fn an_edge_style_becomes_a_class() {
        let nodes = all(&drawn("graph TD\nA --> B\nB -.-> C\nC ==> D"));
        assert_eq!(with_class(&nodes, "flow-solid").len(), 1);
        assert_eq!(with_class(&nodes, "flow-dotted").len(), 1);
        assert_eq!(with_class(&nodes, "flow-thick").len(), 1);
    }

    #[test]
    fn an_edge_label_is_drawn_on_the_edge_it_belongs_to() {
        let nodes = all(&drawn("graph TD\nA -->|Yes| B"));
        let labels = with_class(&nodes, "flow-edge-label");
        assert_eq!(labels.len(), 1);
        let Content::Text(run) = &labels[0].content else {
            panic!("a label")
        };
        assert_eq!(run.content, "Yes");
    }

    #[test]
    fn a_class_on_a_node_becomes_a_class_on_the_drawing() {
        let nodes = all(&drawn("graph TD\nA --> B\nclass A warn"));
        assert_eq!(with_class(&nodes, "flow-class-warn").len(), 1);
    }

    #[test]
    fn a_label_over_two_lines_is_drawn_over_two() {
        let nodes = all(&drawn("graph TD\nA[one<br>two] --> B"));
        let lines: Vec<&str> = with_class(&nodes, "flow-label")
            .iter()
            .filter_map(|node| match &node.content {
                Content::Text(run) => Some(run.content.as_str()),
                _ => None,
            })
            .collect();
        assert!(lines.contains(&"one") && lines.contains(&"two"));
    }

    #[test]
    fn the_boxes_paint_over_the_arrows_that_run_under_them() {
        let scene = drawn("graph TD\nA --> B --> C");
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|node| node.class.first().map(String::as_str))
            .collect();
        let first_box = order.iter().position(|c| *c == "node").unwrap_or(0);
        assert!(order.iter().take(first_box).all(|c| *c == "edge"));
    }

    #[test]
    fn a_group_is_a_box_with_its_name_in_the_band_at_the_top() {
        let nodes = all(&drawn(
            "graph TD\nsubgraph Frontend\nA --> B\nend\nsubgraph Backend\nC --> D\nend\nB --> C",
        ));
        let groups = with_class(&nodes, "group");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id.as_deref(), Some("Frontend"));
        let Content::Group(children) = &groups[0].content else {
            panic!("a group")
        };
        assert_eq!(children.len(), 2, "a box and its name");
        let Content::Text(run) = &children[1].content else {
            panic!("a name")
        };
        assert_eq!(run.content, "Frontend");
        assert_eq!(with_class(&nodes, "flow-group-label").len(), 2);
    }

    #[test]
    fn a_group_is_drawn_behind_everything_it_holds() {
        let scene = drawn("graph TD\nsubgraph Only\nA --> B\nend");
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|node| node.class.first().map(String::as_str))
            .collect();
        assert_eq!(order.first(), Some(&"group"));
    }

    #[test]
    fn a_diagram_with_no_arrows_needs_no_arrowhead() {
        assert!(drawn("graph TD\nA").markers.is_empty());
        assert_eq!(drawn("graph TD\nA --> B").markers.len(), 1);
    }

    #[test]
    fn hovering_a_titled_wire_lights_the_wire_and_its_title_together() {
        // The pairing is structural rather than named: the title is a child of
        // the wire's own group, so one rule covers every wire in the drawing —
        // where C4, whose badge is a separate node, needs one rule per step.
        let css = drawn("graph TD\nA -->|yes| B").style;
        assert!(
            css.contains(".edge:has(.flow-edge-label):hover polyline{stroke:var(--ags-accent"),
            "{css}"
        );
        assert!(
            css.contains(
                ".edge:has(.flow-edge-label):hover .flow-edge-label{fill:var(--ags-accent"
            ),
            "{css}"
        );
    }

    #[test]
    fn a_wire_with_no_title_has_nothing_to_hover_and_is_left_alone() {
        // Every highlight rule is gated on the wire *having* a title, so a bare
        // arrow cannot match one however the pointer moves over it.
        let css = drawn("graph TD\nA --> B").style;
        for rule in css.split("svg") {
            if rule.contains(":hover") {
                assert!(
                    rule.contains(":has(.flow-edge-label)"),
                    "an untitled wire could match this: {rule}"
                );
            }
        }
        // And no lit arrowhead is drawn for a diagram that can never show one.
        assert_eq!(drawn("graph TD\nA --> B").markers.len(), 1);
        assert_eq!(drawn("graph TD\nA -->|yes| B").markers.len(), 2);
    }

    #[test]
    fn only_a_wire_that_draws_a_head_at_its_tail_swaps_that_head() {
        let both = all(&drawn("graph TD\nA <-->|yes| B"));
        assert_eq!(with_class(&both, "flow-head-start").len(), 1);
        let one_way = all(&drawn("graph TD\nA -->|yes| B"));
        assert!(with_class(&one_way, "flow-head-start").is_empty());
    }

    #[test]
    fn a_source_of_nothing_draws_nothing() {
        let scene = drawn("graph TD");
        assert!(scene.nodes.is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(
            "graph TD\nA --> B",
            &Theme::default(),
            &ColorMode::Fixed,
            &Config::default(),
        );
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
