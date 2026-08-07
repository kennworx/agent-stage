//! A placed Venn diagram, drawn into the scene.
//!
//! Identity contract: each set is a group carrying `data-id`, and so is each
//! overlap — even an unlabelled one, which draws nothing at all. An empty group
//! looks pointless until you want to attach a note to a region that has no text
//! of its own, which is exactly what a reviewer wants to do with an overlap.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{layout, Placed, PlacedSet, PlacedUnion, TITLE_FONT};

const BASELINE: &str = "0.35em";
const SET_FONT: f64 = 16.0;
const SET_WEIGHT: u32 = 600;
const UNION_FONT: f64 = 13.0;
const UNION_WEIGHT: u32 = 500;
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

fn set_node(set: &PlacedSet) -> Node {
    let circle = Node::new(
        Role::Node,
        Content::Shape(Shape::Circle {
            c: set.at,
            r: set.r,
        }),
    )
    .classed("venn-circle")
    .classed(format!("venn-color-{}", set.color_index));
    let name = text(
        set.label_at,
        &set.label,
        SET_FONT,
        SET_WEIGHT,
        "venn-set-label",
    );
    Node::new(Role::Node, Content::Group(vec![circle, name]))
        .classed("node")
        .with_id(set.id.clone())
}

fn union_node(union: &PlacedUnion) -> Node {
    let parts = if union.label.is_empty() {
        Vec::new()
    } else {
        vec![text(
            union.at,
            &union.label,
            UNION_FONT,
            UNION_WEIGHT,
            "venn-union-label",
        )]
    };
    Node::new(Role::Label, Content::Group(parts))
        .classed("node")
        .classed("venn-union")
        .with_id(union.id.clone())
}

/// The rules a Venn diagram needs on top of the shared tokens.
///
/// One fill rule per set, drawn at a low opacity so an overlap reads as the two
/// colours mixed rather than as whichever circle happened to be drawn last.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let colors: String = placed
        .sets
        .iter()
        .map(|set| {
            format!(
                ".venn-color-{}{{fill:{}}}",
                set.color_index,
                series_css(set.color_index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .venn-circle{{stroke:var(--_line);stroke-width:1.5;fill-opacity:0.32}}\
         .venn-set-label{{fill:var(--_text)}}\
         .venn-union-label{{fill:var(--_text)}}\
         .venn-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed Venn diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for set in &placed.sets {
        out.push(set_node(set));
    }
    for union in &placed.unions {
        out.push(union_node(union));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(*at, title, TITLE_FONT, TITLE_WEIGHT, "venn-title"));
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

    const DIAGRAM: &str = "venn-beta\n\
        title Skills\n\
        set Design\n\
        set Code[\"Engineering\"]\n\
        union Design, Code\n\
        text Both";

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
    fn every_set_is_addressable_and_writes_its_label_not_its_id() {
        let nodes = all(&drawn(DIAGRAM));
        let sets = with_class(&nodes, "venn-circle");
        assert_eq!(sets.len(), 2);
        let labels: Vec<String> = with_class(&nodes, "venn-set-label")
            .iter()
            .filter_map(|n| match &n.content {
                Content::Text(run) => Some(run.content.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["Design", "Engineering"]);
    }

    #[test]
    fn an_overlap_is_addressable_by_the_sets_that_form_it() {
        let nodes = all(&drawn(DIAGRAM));
        let unions = with_class(&nodes, "venn-union");
        assert_eq!(unions.len(), 1);
        assert_eq!(unions[0].id.as_deref(), Some("Design∩Code"));
    }

    #[test]
    fn an_unlabelled_overlap_is_still_addressable_and_draws_nothing() {
        let nodes = all(&drawn("venn\nset A\nset B\nunion A, B"));
        let unions = with_class(&nodes, "venn-union");
        assert_eq!(unions.len(), 1);
        assert!(matches!(&unions[0].content, Content::Group(parts) if parts.is_empty()));
        assert!(with_class(&nodes, "venn-union-label").is_empty());
    }

    #[test]
    fn each_set_gets_its_own_fill_rule() {
        let style = drawn(DIAGRAM).style;
        assert!(
            style.contains(".venn-color-0{fill:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains(".venn-color-1{fill:hsl(from"), "{style}");
        assert!(!style.contains(".venn-color-2{"), "{style}");
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(DIAGRAM)), "venn-title").len(), 1);
        assert!(with_class(&all(&drawn("venn\nset A")), "venn-title").is_empty());
    }

    #[test]
    fn a_diagram_of_nothing_still_yields_a_canvas() {
        let scene = drawn("venn");
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
