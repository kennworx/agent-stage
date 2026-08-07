//! A placed quadrant chart, drawn into the scene.
//!
//! Identity contract: each plotted point is a group carrying `data-id`, so a
//! reviewer can point at one dot. Nothing else in the chart is addressable —
//! the square and its regions are furniture, not content.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun, Transform,
};
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, AxisLabel, Placed, PlacedPoint, Rect, Region, POINT_RADIUS, TITLE_FONT,
};

const BASELINE: &str = "0.35em";
const AXIS_FONT: f64 = 14.0;
const AXIS_WEIGHT: u32 = 500;
const REGION_FONT: f64 = 15.0;
const REGION_WEIGHT: u32 = 600;
const POINT_FONT: f64 = 13.0;
const POINT_WEIGHT: u32 = 500;
const TITLE_WEIGHT: u32 = 600;

fn wh(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn text(at: Point, content: &str, size: f64, weight: u32, anchor: Anchor, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor,
            font: Font {
                size,
                weight,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: content.to_string(),
        }),
    )
    .classed(class)
}

fn rect_node(rect: Rect, class: &str, role: Role) -> Node {
    Node::new(
        role,
        Content::Shape(Shape::Rect {
            at: rect.at,
            size: wh(rect.width, rect.height),
            rx: 0.0,
            ry: 0.0,
        }),
    )
    .classed(class)
}

/// The tinted half of the checkerboard. The untinted regions draw nothing —
/// they are the background, and painting it again would only cost bytes.
fn region_nodes(regions: &[Region]) -> Vec<Node> {
    regions
        .iter()
        .filter(|r| r.tinted)
        .map(|r| rect_node(r.rect, "quad-fill", Role::Decoration).on(Layer::Frame))
        .collect()
}

fn cross_node(a: Point, b: Point) -> Node {
    Node::new(Role::Frame, Content::Shape(Shape::Line { a, b })).classed("quad-cross")
}

fn axis_label_node(label: &AxisLabel) -> Node {
    let mut node = text(
        label.at,
        &label.text,
        AXIS_FONT,
        AXIS_WEIGHT,
        Anchor::Middle,
        "quad-axis-label",
    )
    .on(Layer::Frame);
    // Turned about its own anchor, so the text stays where the layout put it.
    node.transform = label.rotate.map(|deg| Transform::Rotate {
        deg,
        about: label.at,
    });
    node
}

/// One plotted point: a dot, and its name below it, under one identity.
fn point_node(point: &PlacedPoint) -> Node {
    let dot = Node::new(
        Role::Node,
        Content::Shape(Shape::Circle {
            c: point.at,
            r: POINT_RADIUS,
        }),
    )
    .classed("quad-point");
    let name = text(
        point.label_at,
        &point.name,
        POINT_FONT,
        POINT_WEIGHT,
        Anchor::Middle,
        "quad-point-label",
    );
    Node::new(Role::Node, Content::Group(vec![dot, name]))
        .classed("node")
        .with_id(point.name.clone())
}

/// Draw a placed quadrant chart.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(wh(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .quad-fill{{fill:var(--_inner-stroke);opacity:0.5}}\
         .quad-border{{fill:none;stroke:var(--_line);stroke-width:1.5}}\
         .quad-cross{{stroke:var(--_line);stroke-width:1;opacity:0.6}}\
         .quad-region-label{{fill:var(--_text-faint)}}\
         .quad-axis-label{{fill:var(--_text-sec)}}\
         .quad-title{{fill:var(--_text)}}\
         .quad-point{{fill:var(--_arrow);stroke:var(--ags-bg);stroke-width:2}}\
         .quad-point-label{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );

    for node in region_nodes(&placed.regions) {
        out.push(node);
    }
    if let Some(plot) = placed.plot {
        out.push(rect_node(plot, "quad-border", Role::Frame));
    }
    for (a, b) in &placed.cross {
        out.push(cross_node(*a, *b));
    }
    for (name, at) in &placed.region_labels {
        out.push(
            text(
                *at,
                name,
                REGION_FONT,
                REGION_WEIGHT,
                Anchor::Middle,
                "quad-region-label",
            )
            // With the furniture: a region's name is behind the points that
            // land on it, not competing with them.
            .on(Layer::Frame),
        );
    }
    for label in &placed.axis_labels {
        out.push(axis_label_node(label));
    }
    for point in &placed.points {
        out.push(point_node(point));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "quad-title",
        ));
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

    const CHART: &str = "quadrantChart\n\
        title Reach\n\
        x-axis Low --> High\n\
        y-axis Down --> Up\n\
        quadrant-1 Expand\n\
        Campaign A: [0.3, 0.6]";

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
    fn every_point_is_addressable_and_names_itself_twice() {
        let nodes = all(&drawn(CHART));
        let points = with_class(&nodes, "node");
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].id.as_deref(), Some("Campaign A"));
        // Once as identity, once as visible text.
        assert_eq!(with_class(&nodes, "quad-point-label").len(), 1);
    }

    #[test]
    fn only_the_tinted_regions_are_painted() {
        // Four regions, two tinted: painting the other two would draw the
        // background on top of the background.
        assert_eq!(with_class(&all(&drawn(CHART)), "quad-fill").len(), 2);
    }

    #[test]
    fn the_furniture_paints_behind_the_points() {
        let scene = drawn(CHART);
        let layers: Vec<(Layer, &str)> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(|c| (n.layer, c.as_str())))
            .collect();
        let point = layers
            .iter()
            .position(|(_, c)| *c == "node")
            .expect("a point");
        assert!(
            layers.iter().take(point).all(|(l, _)| *l == Layer::Frame),
            "{layers:?}"
        );
    }

    #[test]
    fn a_vertical_axis_label_turns_about_its_own_anchor() {
        let nodes = all(&drawn(CHART));
        let labels = with_class(&nodes, "quad-axis-label");
        // Two flat, two turned; a turn about anything else would move the text.
        let turned: Vec<&&Node> = labels.iter().filter(|n| n.transform.is_some()).collect();
        assert_eq!(turned.len(), 2);
        for node in turned {
            let Some(Transform::Rotate { deg, about }) = node.transform else {
                panic!("a rotation")
            };
            let Content::Text(run) = &node.content else {
                panic!("text")
            };
            assert!((deg + 90.0).abs() < 1e-9);
            assert_eq!(about, run.at);
        }
    }

    #[test]
    fn a_chart_with_no_axes_named_still_draws_its_square() {
        let nodes = all(&drawn("quadrantChart"));
        assert_eq!(with_class(&nodes, "quad-border").len(), 1);
        assert_eq!(with_class(&nodes, "quad-cross").len(), 2);
        assert!(with_class(&nodes, "quad-axis-label").is_empty());
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(CHART)), "quad-title").len(), 1);
        assert!(with_class(&all(&drawn("quadrantChart")), "quad-title").is_empty());
    }

    #[test]
    fn a_region_name_is_drawn_only_when_it_was_given() {
        assert_eq!(
            with_class(&all(&drawn(CHART)), "quad-region-label").len(),
            1
        );
        assert!(with_class(&all(&drawn("quadrantChart")), "quad-region-label").is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(CHART, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
