//! A placed fishbone, drawn into the scene.
//!
//! Identity contract: the effect, every category and every cause is a group
//! carrying `data-id`; every bone names the two it joins, so the structure is
//! readable from the markup as well as from the picture.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, Head, Placed, PlacedCategory, PlacedCause, CAT_FONT, CAT_WEIGHT, CAUSE_FONT,
    CAUSE_WEIGHT, EFFECT_FONT, EFFECT_WEIGHT,
};

const BASELINE: &str = "0.35em";

fn text(
    at: Point,
    content: &str,
    size: f64,
    weight: u32,
    anchor: Anchor,
    dy: Option<&str>,
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
                italic: false,
            },
            dy: dy.map(str::to_string),
            content: content.to_string(),
        }),
    )
    .classed(class)
}

fn bone(a: Point, b: Point, class: &str, from: &str, to: &str) -> Node {
    Node::new(Role::Edge, Content::Shape(Shape::Line { a, b }))
        .classed(class)
        .tagged("from", from.to_string())
        .tagged("to", to.to_string())
}

fn cause_node(cause: &PlacedCause) -> Node {
    Node::new(
        Role::Node,
        Content::Group(vec![text(
            cause.label_at,
            &cause.text,
            CAUSE_FONT,
            CAUSE_WEIGHT,
            Anchor::End,
            Some(BASELINE),
            "ish-cause-label",
        )]),
    )
    .classed("node")
    .with_id(cause.id.clone())
}

fn category_node(category: &PlacedCategory) -> Node {
    Node::new(
        Role::Node,
        Content::Group(vec![text(
            category.label_at,
            &category.text,
            CAT_FONT,
            CAT_WEIGHT,
            Anchor::Middle,
            // No baseline shift: the layout already put this on the baseline it
            // wants, one font-size clear of the bone rather than centred on it.
            None,
            "ish-cat-label",
        )]),
    )
    .classed("node")
    .with_id(category.id.clone())
}

fn head_node(head: &Head) -> Node {
    Node::new(
        Role::Node,
        Content::Group(vec![
            Node::new(
                Role::Node,
                Content::Shape(Shape::Rect {
                    at: head.box_at,
                    size: Size {
                        width: head.box_width,
                        height: head.box_height,
                    },
                    rx: 6.0,
                    ry: 6.0,
                }),
            )
            .classed("ish-head-box"),
            text(
                head.at,
                &head.text,
                EFFECT_FONT,
                EFFECT_WEIGHT,
                Anchor::Middle,
                Some(BASELINE),
                "ish-head-label",
            ),
        ]),
    )
    .classed("node")
    .with_id(head.id.clone())
}

/// Draw a placed fishbone.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .ish-spine{{stroke:var(--_line);stroke-width:2.5;stroke-linecap:round}}\
         .ish-bone{{stroke:var(--_line);stroke-width:1.75;stroke-linecap:round}}\
         .ish-subbone{{stroke:var(--_line);stroke-width:1;stroke-linecap:round}}\
         .ish-head-box{{fill:var(--ags-bg);stroke:var(--ags-accent,var(--_line));stroke-width:2}}\
         .ish-head-label{{fill:var(--_text)}}\
         .ish-cat-label{{fill:var(--_text)}}\
         .ish-cause-label{{fill:var(--_text-sec)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );

    let head_id = placed
        .head
        .as_ref()
        .map_or_else(String::new, |h| h.id.clone());
    if let Some((a, b)) = placed.spine {
        // The spine joins the head to itself: it is the diagram's own axis, not
        // a relationship between two things.
        out.push(bone(a, b, "ish-spine", &head_id, &head_id));
    }
    for category in &placed.categories {
        out.push(bone(
            category.bone.0,
            category.bone.1,
            "ish-bone",
            &head_id,
            &category.id,
        ));
        for cause in &category.causes {
            out.push(bone(
                cause.bone.0,
                cause.bone.1,
                "ish-subbone",
                &cause.parent_id,
                &cause.id,
            ));
        }
    }
    for category in &placed.categories {
        for cause in &category.causes {
            out.push(cause_node(cause));
        }
        out.push(category_node(category));
    }
    if let Some(head) = &placed.head {
        out.push(head_node(head));
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

    const FISH: &str = "ishikawa\n\
        Late delivery\n  \
          People\n    \
            Understaffed\n  \
          Process";

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
    fn the_effect_every_category_and_every_cause_is_addressable() {
        let nodes = all(&drawn(FISH));
        let ids: Vec<&str> = with_class(&nodes, "node")
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert_eq!(ids, ["Understaffed", "People", "Process", "Late delivery"]);
    }

    #[test]
    fn a_bone_names_what_it_joins() {
        let nodes = all(&drawn(FISH));
        let bone = with_class(&nodes, "ish-bone")[0];
        assert!(bone.data.contains(&("from".into(), "Late delivery".into())));
        assert!(bone.data.contains(&("to".into(), "People".into())));
        let sub = with_class(&nodes, "ish-subbone")[0];
        assert!(sub.data.contains(&("from".into(), "People".into())));
    }

    #[test]
    fn there_is_one_spine_one_bone_per_category_and_one_per_cause() {
        let nodes = all(&drawn(FISH));
        assert_eq!(with_class(&nodes, "ish-spine").len(), 1);
        assert_eq!(with_class(&nodes, "ish-bone").len(), 2);
        assert_eq!(with_class(&nodes, "ish-subbone").len(), 1);
    }

    #[test]
    fn every_bone_paints_behind_every_label() {
        let scene = drawn(FISH);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        let first_label = order.iter().position(|c| *c == "node").expect("a label");
        assert!(order
            .iter()
            .take(first_label)
            .all(|c| c.starts_with("ish-")));
    }

    #[test]
    fn a_category_name_takes_no_baseline_shift_and_a_cause_name_does() {
        // The layout puts a category's name on the baseline it wants; shifting
        // it again would drop it onto its own bone.
        let nodes = all(&drawn(FISH));
        let category = with_class(&nodes, "ish-cat-label")[0];
        let cause = with_class(&nodes, "ish-cause-label")[0];
        assert!(matches!(&category.content, Content::Text(run) if run.dy.is_none()));
        assert!(matches!(&cause.content, Content::Text(run) if run.dy.is_some()));
    }

    #[test]
    fn a_cause_name_reads_back_toward_its_bone() {
        let nodes = all(&drawn(FISH));
        let cause = with_class(&nodes, "ish-cause-label")[0];
        assert!(matches!(&cause.content, Content::Text(run) if run.anchor == Anchor::End));
    }

    #[test]
    fn the_effect_is_drawn_last_so_nothing_crosses_its_box() {
        let scene = drawn(FISH);
        let last = scene.painted().last().copied().cloned().expect("a node");
        assert_eq!(last.id.as_deref(), Some("Late delivery"));
    }

    #[test]
    fn a_diagram_of_nothing_still_yields_a_canvas() {
        let scene = drawn("ishikawa");
        assert!(scene.canvas.width > 0.0);
        // The spine and an empty head are still drawn.
        assert_eq!(with_class(&all(&scene), "ish-spine").len(), 1);
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(FISH, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
