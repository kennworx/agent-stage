//! A placed sankey diagram, drawn into the scene.
//!
//! Identity contract: each node is a group carrying `data-id`; each band names
//! the two nodes it joins and the value it carries, so a reviewer can point at
//! a flow and read what it is worth without measuring it.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Seg, Shape, Size, TextRun};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{layout, Placed, PlacedLink, PlacedNode, Side, LABEL_FONT, LABEL_WEIGHT};

const BASELINE: &str = "0.35em";

/// The band between two edges: out along the top, back along the bottom.
///
/// Both curves put their control points on the horizontal midpoint, which is
/// what makes the two sides parallel and the band read as one ribbon rather
/// than as two unrelated curves.
fn band(link: &PlacedLink) -> Vec<Seg> {
    let mid = f64::midpoint(link.from.x, link.to.x);
    let from_bottom = link.from.y + link.thickness;
    let to_bottom = link.to.y + link.thickness;
    vec![
        Seg::MoveTo(link.from),
        Seg::Cubic {
            c1: Point::new(mid, link.from.y),
            c2: Point::new(mid, link.to.y),
            to: link.to,
        },
        Seg::LineTo(Point::new(link.to.x, to_bottom)),
        Seg::Cubic {
            c1: Point::new(mid, to_bottom),
            c2: Point::new(mid, from_bottom),
            to: Point::new(link.from.x, from_bottom),
        },
        Seg::Close,
    ]
}

fn link_node(link: &PlacedLink) -> Node {
    Node::new(Role::Edge, Content::Shape(Shape::Path(band(link))))
        .classed("sankey-link")
        .classed(format!("sankey-color-{}", link.color_index))
        .tagged("from", link.source.clone())
        .tagged("to", link.target.clone())
        .tagged("value", link.value.to_string())
}

fn node_node(node: &PlacedNode) -> Node {
    let bar = Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: node.at,
            size: Size {
                width: node.width,
                height: node.height,
            },
            rx: 2.0,
            ry: 2.0,
        }),
    )
    .classed("sankey-node")
    .classed(format!("sankey-color-{}", node.color_index));
    let label = Node::new(
        Role::Label,
        Content::Text(TextRun {
            at: node.label_at,
            anchor: match node.label_side {
                Side::Right => Anchor::Start,
                Side::Left => Anchor::End,
            },
            font: Font {
                size: LABEL_FONT,
                weight: LABEL_WEIGHT,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: node.id.clone(),
        }),
    )
    .classed("sankey-label");
    Node::new(Role::Node, Content::Group(vec![bar, label]))
        .classed("node")
        .with_id(node.id.clone())
}

/// The rules a sankey needs on top of the shared tokens.
///
/// One fill rule per node, because the palette is derived per index and CSS
/// cannot compute it. Bands reuse their source's rule, which is what makes a
/// flow read as coming *from* somewhere.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let colors: String = placed
        .nodes
        .iter()
        .map(|node| {
            format!(
                ".sankey-color-{}{{fill:{}}}",
                node.color_index,
                series_css(node.color_index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .sankey-node{{stroke:var(--ags-bg);stroke-width:1}}\
         .sankey-link{{fill-opacity:0.42;stroke:none}}\
         .sankey-link:hover{{fill-opacity:0.65}}\
         .sankey-label{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed sankey diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for link in &placed.links {
        out.push(link_node(link));
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

    const CHAIN: &str = "sankey-beta\nA,B,10\nB,C,4";

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
    fn every_node_is_addressable_and_every_band_names_its_ends() {
        let nodes = all(&drawn(CHAIN));
        assert_eq!(with_class(&nodes, "node").len(), 3);
        let links = with_class(&nodes, "sankey-link");
        assert_eq!(links.len(), 2);
        assert!(links[0].data.contains(&("from".into(), "A".into())));
        assert!(links[0].data.contains(&("to".into(), "B".into())));
        assert!(links[0].data.contains(&("value".into(), "10".into())));
    }

    #[test]
    fn a_band_takes_its_colour_from_where_it_comes_from() {
        let nodes = all(&drawn(CHAIN));
        let links = with_class(&nodes, "sankey-link");
        // B is node 1, so the flow out of it is coloured 1, not 2.
        assert!(links[1].class.iter().any(|c| c == "sankey-color-1"));
    }

    #[test]
    fn a_band_is_closed_so_it_fills_rather_than_outlines() {
        let nodes = all(&drawn(CHAIN));
        let Content::Shape(Shape::Path(segs)) = &with_class(&nodes, "sankey-link")[0].content
        else {
            panic!("a band is a path")
        };
        assert!(matches!(segs.last(), Some(Seg::Close)));
        // Out along one curve, across, and back along the other.
        assert_eq!(segs.len(), 5);
    }

    #[test]
    fn bands_paint_behind_the_nodes_they_join() {
        let scene = drawn(CHAIN);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        assert_eq!(
            order,
            ["sankey-link", "sankey-link", "node", "node", "node"]
        );
    }

    #[test]
    fn a_name_anchors_away_from_its_node_on_whichever_side_it_sits() {
        let nodes = all(&drawn(CHAIN));
        let labels = with_class(&nodes, "sankey-label");
        let anchors: Vec<Anchor> = labels
            .iter()
            .filter_map(|n| match &n.content {
                Content::Text(run) => Some(run.anchor),
                _ => None,
            })
            .collect();
        assert_eq!(anchors, [Anchor::Start, Anchor::Start, Anchor::End]);
    }

    #[test]
    fn one_fill_rule_is_emitted_for_each_node() {
        let style = drawn(CHAIN).style;
        for index in 0..3 {
            assert!(
                style.contains(&format!(".sankey-color-{index}{{fill:")),
                "{style}"
            );
        }
        assert!(!style.contains(".sankey-color-3{"), "{style}");
    }

    #[test]
    fn the_first_node_takes_the_accent_and_the_rest_are_derived() {
        let style = drawn(CHAIN).style;
        assert!(
            style.contains(".sankey-color-0{fill:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains(".sankey-color-1{fill:hsl(from"), "{style}");
    }

    #[test]
    fn a_diagram_of_nothing_still_yields_a_canvas() {
        let scene = drawn("sankey");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(CHAIN, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
