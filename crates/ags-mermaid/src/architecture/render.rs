//! A placed architecture diagram, drawn into the scene.
//!
//! Identity contract: every group, service and junction is a group carrying
//! `data-id`, and every line carries `data-from` and `data-to`.

use crate::api::ColorMode;
use crate::icons::icon;
use crate::scene::{
    Anchor, Color, Content, Font, Marker, Node, Paint, Point, Role, Scene, Shape, Size, TextRun,
};
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, Placed, PlacedEdge, PlacedItem, GROUP_PAD, HEADER, ICON, ICON_TOP, LABEL_FONT,
    LABEL_WEIGHT, TITLE_FONT, TITLE_WEIGHT,
};
use super::types::Kind;

const BASELINE: &str = "0.35em";
const ARROW_ID: &str = "arch-arrow";
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 5.0;
/// How round a group's corners are, and a service's.
const GROUP_CORNER: f64 = 10.0;
const SERVICE_CORNER: f64 = 8.0;
/// The dot a junction is drawn as.
const JUNCTION_DOT: f64 = 5.0;
/// The glyph a service with no icon written on it falls back to.
const UNKNOWN_ICON: &str = "unknown";

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(item: &PlacedItem, role: Role, radius: f64, class: &str) -> Node {
    Node::new(
        role,
        Content::Shape(Shape::Rect {
            at: item.at,
            size: size(item.width, item.height),
            rx: radius,
            ry: radius,
        }),
    )
    .classed(class)
}

fn run(at: Point, content: &str, font: f64, weight: u32, anchor: Anchor, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor,
            font: Font {
                size: font,
                weight,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: crate::text::strip_formatting_tags(content),
        }),
    )
    .classed(class)
}

/// A group: the frame, and its name in the band at the top.
fn group_node(item: &PlacedItem) -> Vec<Node> {
    vec![
        rect(item, Role::Frame, GROUP_CORNER, "arch-group-box"),
        run(
            point(item.at.x + GROUP_PAD, item.at.y + HEADER / 2.0),
            &item.title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Start,
            "arch-group-title",
        ),
    ]
}

/// A service: the box, the glyph, and the name under it.
fn service_node(item: &PlacedItem) -> Vec<Node> {
    let glyph = if item.icon.is_empty() {
        UNKNOWN_ICON
    } else {
        &item.icon
    };
    vec![
        rect(item, Role::Node, SERVICE_CORNER, "arch-service-box"),
        icon(
            glyph,
            point(item.at.x + (item.width - ICON) / 2.0, item.at.y + ICON_TOP),
            ICON,
            "arch-icon",
        ),
        run(
            point(
                item.at.x + item.width / 2.0,
                item.at.y
                    + ICON_TOP
                    + ICON
                    + crate::architecture::layout::ICON_GAP
                    + LABEL_FONT / 2.0,
            ),
            &item.title,
            LABEL_FONT,
            LABEL_WEIGHT,
            Anchor::Middle,
            "arch-service-label",
        ),
    ]
}

/// A junction: a dot where lines meet.
fn junction_node(item: &PlacedItem) -> Vec<Node> {
    vec![Node::new(
        Role::Node,
        Content::Shape(Shape::Circle {
            c: item.centre(),
            r: JUNCTION_DOT.min(item.width / 2.0),
        }),
    )
    .classed("arch-junction")]
}

/// One declared thing, whatever it is drawn as.
///
/// A group is a frame, not a box. The distinction is what the checker selects
/// on: a frame is something lines are expected to cross, and calling one a box
/// reports every line inside a group as passing through it.
fn item_node(item: &PlacedItem) -> Node {
    let children = match item.kind {
        Kind::Group => group_node(item),
        Kind::Service => service_node(item),
        Kind::Junction => junction_node(item),
    };
    let role = if item.kind == Kind::Group {
        Role::Frame
    } else {
        Role::Node
    };
    Node::new(role, Content::Group(children))
        .classed("node")
        .classed(match item.kind {
            Kind::Group => "arch-group",
            Kind::Service => "arch-service",
            Kind::Junction => "arch-junction-node",
        })
        .with_id(item.id.clone())
        .tagged("label", item.title.clone())
        .tagged("depth", item.depth.to_string())
}

fn edge_node(edge: &PlacedEdge) -> Node {
    let mut paint = Paint::default();
    if edge.arrow_end {
        paint.marker_end = Some(ARROW_ID.to_string());
    }
    if edge.arrow_start {
        paint.marker_start = Some(ARROW_ID.to_string());
    }
    Node::new(
        Role::Edge,
        Content::Group(vec![Node::new(
            Role::Edge,
            Content::Shape(Shape::Polyline(edge.points.clone())),
        )
        .painted(paint)]),
    )
    .classed("edge")
    .classed("arch-edge")
    .tagged("from", edge.from.clone())
    .tagged("to", edge.to.clone())
}

fn arrow_marker() -> Marker {
    let ink = Color::Token {
        name: "_line".into(),
        fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
    };
    Marker {
        id: ARROW_ID.to_string(),
        view: size(ARROW_W, ARROW_H),
        size: size(ARROW_W, ARROW_H),
        // One short of the tip, so the head overlaps the line it caps rather
        // than leaving a hairline of background between the two.
        ref_x: ARROW_W - 1.0,
        ref_y: ARROW_H / 2.0,
        shape: Shape::Polygon(vec![
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

/// The rules an architecture diagram needs on top of the shared tokens.
fn style(theme: &Theme, mode: &ColorMode) -> String {
    format!(
        "{}\
         .arch-group-box{{fill:var(--_group-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .arch-group-title{{fill:var(--_text-sec)}}\
         .arch-service-box{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .arch-service-label{{fill:var(--_text)}}\
         .arch-icon{{color:var(--_text)}}\
         .{}{{stroke-linecap:round;stroke-linejoin:round}}\
         .arch-junction{{fill:var(--_line);stroke:var(--_group-fill);stroke-width:2}}\
         .arch-edge polyline{{fill:none;stroke:var(--_line);stroke-width:1}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode),
        crate::icons::OUTLINE_CLASS
    )
}

/// Draw a placed architecture diagram.
///
/// Frames first and outermost first, so a nested group paints over the one
/// round it; then the lines; then the things the lines join, which is what
/// keeps a line from crossing the glyph it points at.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(theme, mode);
    if placed
        .edges
        .iter()
        .any(|edge| edge.arrow_start || edge.arrow_end)
    {
        out.markers.push(arrow_marker());
    }
    let mut frames: Vec<&PlacedItem> = placed
        .items
        .iter()
        .filter(|item| item.kind == Kind::Group)
        .collect();
    frames.sort_by_key(|item| item.depth);
    for item in frames {
        out.push(item_node(item));
    }
    for edge in &placed.edges {
        if edge.points.len() >= 2 {
            out.push(edge_node(edge));
        }
    }
    for item in &placed.items {
        if item.kind != Kind::Group {
            out.push(item_node(item));
        }
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

    fn classed(scene: &Scene, class: &str) -> usize {
        every(scene)
            .into_iter()
            .filter(|node| node.class.iter().any(|name| name == class))
            .count()
    }

    const SAMPLE: &str = "architecture-beta\n  group cloud(cloud)[Cloud]\n  service web(server)[Web] in cloud\n  service db(database)[DB] in cloud\n  web:R --> L:db";

    #[test]
    fn every_declared_thing_is_drawn_under_its_own_name() {
        let scene = drawn(SAMPLE);
        for id in ["cloud", "web", "db"] {
            assert!(
                scene
                    .nodes
                    .iter()
                    .any(|node| node.id.as_deref() == Some(id)),
                "no {id}"
            );
        }
        let written = texts(&scene);
        assert!(written.contains(&"Cloud".to_string()));
        assert!(written.contains(&"Web".to_string()));
    }

    #[test]
    fn a_service_gets_the_glyph_it_named() {
        let scene = drawn(SAMPLE);
        assert_eq!(classed(&scene, "arch-icon"), 2);
        // And one that named nothing still gets a glyph rather than a gap.
        let bare = drawn("architecture-beta\n  service plain[Plain]");
        assert_eq!(classed(&bare, "arch-icon"), 1);
    }

    #[test]
    fn a_junction_is_a_dot_rather_than_a_box() {
        let scene = drawn("architecture-beta\n  service a(server)[A]\n  junction j\n  a:R -- L:j");
        assert_eq!(classed(&scene, "arch-junction"), 1);
        let dot = every(&scene)
            .into_iter()
            .find(|node| node.class.iter().any(|name| name == "arch-junction"))
            .expect("a dot");
        assert!(matches!(dot.content, Content::Shape(Shape::Circle { .. })));
        // Nothing is written on it.
        assert!(!texts(&scene).contains(&"j".to_string()));
    }

    #[test]
    fn a_line_says_what_it_joins() {
        let scene = drawn(SAMPLE);
        let edge = scene
            .nodes
            .iter()
            .find(|node| node.class.iter().any(|name| name == "edge"))
            .expect("a line");
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "from" && value == "web"));
        assert!(edge
            .data
            .iter()
            .any(|(key, value)| key == "to" && value == "db"));
    }

    #[test]
    fn an_arrowhead_is_defined_only_when_something_asks_for_one() {
        assert_eq!(drawn(SAMPLE).markers.len(), 1);
        let plain = drawn(
            "architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  a:R -- L:b",
        );
        assert!(plain.markers.is_empty());
    }

    #[test]
    fn an_arrowhead_sits_on_the_end_the_arrow_was_written_towards() {
        let forwards = drawn(SAMPLE);
        let line = every(&forwards)
            .into_iter()
            .find(|node| matches!(node.content, Content::Shape(Shape::Polyline(_))))
            .expect("a line");
        assert_eq!(line.paint.marker_end.as_deref(), Some(ARROW_ID));
        assert_eq!(line.paint.marker_start, None);
        let backwards = drawn(
            "architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  a:R <-- L:b",
        );
        let line = every(&backwards)
            .into_iter()
            .find(|node| matches!(node.content, Content::Shape(Shape::Polyline(_))))
            .expect("a line");
        assert_eq!(line.paint.marker_start.as_deref(), Some(ARROW_ID));
    }

    #[test]
    fn frames_are_drawn_behind_what_they_hold_and_outermost_first() {
        let scene = drawn(
            "architecture-beta\n  group cloud(cloud)[Cloud]\n  group region(server)[Region] in cloud\n  service web(server)[Web] in region\n  service cdn(internet)[CDN] in cloud\n  cdn:L --> T:web",
        );
        let order: Vec<String> = scene
            .painted()
            .iter()
            .filter_map(|node| node.id.clone())
            .collect();
        let at = |id: &str| order.iter().position(|held| held == id).expect(id);
        assert!(at("cloud") < at("region"), "the outer frame paints first");
        assert!(
            at("region") < at("web"),
            "frames paint before their content"
        );
    }

    #[test]
    fn a_diagram_with_nothing_in_it_draws_nothing() {
        let scene = scene(&Placed::default(), &Theme::default(), &ColorMode::Tokens);
        assert!(scene.nodes.is_empty());
        assert!(scene.markers.is_empty());
    }

    #[test]
    fn a_line_that_goes_nowhere_is_not_drawn() {
        let scene = drawn("architecture-beta\n  service a(server)[A]\n  a:R --> L:nowhere");
        assert!(!scene
            .nodes
            .iter()
            .any(|node| node.class.iter().any(|name| name == "edge")));
    }

    #[test]
    fn the_same_source_twice_draws_the_same_thing() {
        assert_eq!(drawn(SAMPLE), drawn(SAMPLE));
    }

    #[test]
    fn the_glyphs_get_the_rule_that_rounds_their_ends() {
        assert!(drawn(SAMPLE).style.contains(crate::icons::OUTLINE_CLASS));
    }
}
