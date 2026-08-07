//! A placed Wardley map, drawn into the scene.
//!
//! Identity contract: each component is a group carrying `data-id` and the kind
//! it is; each dependency names the two ids it joins.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun, Transform,
};
use crate::theme::{style_block, Theme};

use super::layout::{layout, AxisLabel, Placed, PlacedComponent, PlacedLink, Rect, TITLE_FONT};
use super::types::{Kind, Style};

const BASELINE: &str = "0.35em";
const AXIS_FONT: f64 = 14.0;
const AXIS_WEIGHT: u32 = 500;
const STAGE_FONT: f64 = 12.0;
const STAGE_WEIGHT: u32 = 500;
const NAME_FONT: f64 = 13.0;
const NAME_WEIGHT: u32 = 500;
const TITLE_WEIGHT: u32 = 600;

fn text(at: Point, content: &str, size: f64, weight: u32, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor: Anchor::Middle,
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

fn border_node(plot: Rect) -> Node {
    Node::new(
        Role::Frame,
        Content::Shape(Shape::Rect {
            at: plot.at,
            size: Size {
                width: plot.width,
                height: plot.height,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    )
    .classed("wardley-border")
}

fn axis_label_node(label: &AxisLabel) -> Node {
    let mut node = text(
        label.at,
        &label.text,
        AXIS_FONT,
        AXIS_WEIGHT,
        "wardley-axis-label",
    )
    .on(Layer::Frame);
    node.transform = label.rotate.map(|deg| Transform::Rotate {
        deg,
        about: label.at,
    });
    node
}

fn link_node(link: &PlacedLink) -> Node {
    let style = match link.style {
        Style::Solid => "wardley-link-solid",
        Style::Dashed => "wardley-link-dashed",
        Style::Flow => "wardley-link-flow",
    };
    Node::new(
        Role::Edge,
        Content::Shape(Shape::Line {
            a: link.a,
            b: link.b,
        }),
    )
    .classed("wardley-link")
    .classed(style)
    .tagged("from", link.from.clone())
    .tagged("to", link.to.clone())
}

fn component_node(component: &PlacedComponent) -> Node {
    let dot = Node::new(
        Role::Node,
        Content::Shape(Shape::Circle {
            c: component.at,
            r: component.radius(),
        }),
    )
    .classed(match component.kind {
        Kind::Anchor => "wardley-anchor",
        Kind::Component => "wardley-component",
    });
    let name = text(
        component.label_at,
        &component.name,
        NAME_FONT,
        NAME_WEIGHT,
        "wardley-component-label",
    );
    Node::new(Role::Node, Content::Group(vec![dot, name]))
        .classed("node")
        .with_id(component.id.clone())
        .tagged(
            "kind",
            match component.kind {
                Kind::Anchor => "anchor",
                Kind::Component => "component",
            },
        )
}

/// Draw a placed Wardley map.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .wardley-border{{fill:none;stroke:var(--_line);stroke-width:1.5}}\
         .wardley-grid{{stroke:var(--_line);stroke-width:1;opacity:0.4;stroke-dasharray:4 4}}\
         .wardley-stage-label{{fill:var(--_text-sec)}}\
         .wardley-axis-label{{fill:var(--_text-sec)}}\
         .wardley-title{{fill:var(--_text)}}\
         .wardley-link{{stroke:var(--_line);stroke-width:1.5}}\
         .wardley-link-dashed{{stroke-dasharray:5 4}}\
         .wardley-link-flow{{stroke:var(--ags-accent,var(--_arrow));stroke-width:2}}\
         .wardley-component{{fill:var(--ags-bg);stroke:var(--_text);stroke-width:2}}\
         .wardley-anchor{{fill:var(--ags-accent,var(--_arrow));stroke:var(--ags-bg);stroke-width:2}}\
         .wardley-component-label{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );

    if let Some(plot) = placed.plot {
        out.push(border_node(plot));
    }
    for (a, b) in &placed.grid {
        out.push(
            Node::new(Role::Frame, Content::Shape(Shape::Line { a: *a, b: *b }))
                .classed("wardley-grid"),
        );
    }
    for (name, at) in &placed.stage_labels {
        out.push(text(*at, name, STAGE_FONT, STAGE_WEIGHT, "wardley-stage-label").on(Layer::Frame));
    }
    for label in &placed.axis_labels {
        out.push(axis_label_node(label));
    }
    for link in &placed.links {
        out.push(link_node(link));
    }
    for component in &placed.components {
        out.push(component_node(component));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(*at, title, TITLE_FONT, TITLE_WEIGHT, "wardley-title"));
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

    const MAP: &str = "wardley-beta\n\
        title Photos\n\
        anchor Customer [0.95, 0.63]\n\
        component Website [0.79, 0.61]\n\
        Platform [0.4, 0.75]\n\
        Customer -> Website\n\
        Website -.-> Platform";

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
    fn every_component_is_addressable_and_says_what_it_is() {
        let nodes = all(&drawn(MAP));
        let components = with_class(&nodes, "node");
        assert_eq!(components.len(), 3);
        assert_eq!(components[0].id.as_deref(), Some("Customer"));
        assert!(components[0]
            .data
            .contains(&("kind".into(), "anchor".into())));
        assert!(components[1]
            .data
            .contains(&("kind".into(), "component".into())));
    }

    #[test]
    fn an_anchor_and_a_component_are_drawn_differently() {
        let nodes = all(&drawn(MAP));
        assert_eq!(with_class(&nodes, "wardley-anchor").len(), 1);
        assert_eq!(with_class(&nodes, "wardley-component").len(), 2);
    }

    #[test]
    fn a_dependency_names_both_ends_and_carries_its_style() {
        let nodes = all(&drawn(MAP));
        let links = with_class(&nodes, "wardley-link");
        assert_eq!(links.len(), 2);
        assert!(links[0].data.contains(&("from".into(), "Customer".into())));
        assert!(links[1].class.iter().any(|c| c == "wardley-link-dashed"));
    }

    #[test]
    fn the_plane_paints_behind_everything_placed_on_it() {
        let scene = drawn(MAP);
        let layers: Vec<(Layer, &str)> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(|c| (n.layer, c.as_str())))
            .collect();
        let first_link = layers
            .iter()
            .position(|(_, c)| *c == "wardley-link")
            .expect("a link");
        assert!(
            layers
                .iter()
                .take(first_link)
                .all(|(l, _)| *l == Layer::Frame),
            "{layers:?}"
        );
    }

    #[test]
    fn the_four_stages_are_named_under_three_dividers() {
        let nodes = all(&drawn("wardley"));
        assert_eq!(with_class(&nodes, "wardley-grid").len(), 3);
        assert_eq!(with_class(&nodes, "wardley-stage-label").len(), 4);
    }

    #[test]
    fn both_axes_are_named_whether_or_not_anything_is_placed() {
        let nodes = all(&drawn("wardley"));
        assert_eq!(with_class(&nodes, "wardley-axis-label").len(), 2);
        assert_eq!(with_class(&nodes, "wardley-border").len(), 1);
    }

    #[test]
    fn the_turned_axis_name_rotates_about_its_own_anchor() {
        let nodes = all(&drawn("wardley"));
        let turned = with_class(&nodes, "wardley-axis-label")
            .into_iter()
            .find(|n| n.transform.is_some())
            .expect("a turned label");
        let (Some(Transform::Rotate { about, .. }), Content::Text(run)) =
            (turned.transform, &turned.content)
        else {
            panic!("a turned text run")
        };
        assert_eq!(about, run.at);
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(MAP)), "wardley-title").len(), 1);
        assert!(with_class(&all(&drawn("wardley")), "wardley-title").is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(MAP, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
