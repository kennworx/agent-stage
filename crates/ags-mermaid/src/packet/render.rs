//! A placed packet diagram, drawn into the scene.
//!
//! Identity contract: each field is a group carrying `data-id`, plus the bit
//! range it covers. A field that wraps keeps every rectangle inside that one
//! group, so a note lands on the field rather than on one of its halves.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{layout, PlacedField, Segment, TITLE_FONT};

const BASELINE: &str = "0.35em";

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
            dy: if class == "packet-bit-num" {
                // A bit number sits *above* its rectangle on its own baseline,
                // not centred on a point, so it takes no baseline shift.
                None
            } else {
                Some(BASELINE.to_string())
            },
            content: content.to_string(),
        }),
    )
    .classed(class)
}

/// One rectangle, its bit numbers, and the field's name.
fn segment_nodes(seg: &Segment, label: &str) -> Vec<Node> {
    let mut out = vec![Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: seg.at,
            size: Size {
                width: seg.width,
                height: seg.height,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    )
    .classed("packet-field-rect")];
    out.push(text(
        Point::new(seg.at.x + 2.0, seg.at.y - 4.0),
        &seg.start_bit.to_string(),
        10.0,
        400,
        Anchor::Start,
        "packet-bit-num",
    ));
    // A one-bit field is its own start and end; numbering it twice would print
    // the same digit on top of itself.
    if seg.end_bit != seg.start_bit {
        out.push(text(
            Point::new(seg.at.x + seg.width - 2.0, seg.at.y - 4.0),
            &seg.end_bit.to_string(),
            10.0,
            400,
            Anchor::End,
            "packet-bit-num",
        ));
    }
    out.push(text(
        seg.label_at,
        label,
        13.0,
        500,
        Anchor::Middle,
        "packet-field-label",
    ));
    out
}

fn field_node(field: &PlacedField) -> Node {
    let parts = field
        .segments
        .iter()
        .flat_map(|seg| segment_nodes(seg, &field.label))
        .collect();
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(field.id.clone())
        .tagged("start", field.start.to_string())
        .tagged("end", field.end.to_string())
}

/// Draw a placed packet diagram.
pub fn scene(placed: &super::layout::Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .packet-field-rect{{fill:var(--ags-bg);stroke:var(--ags-accent,var(--_line));stroke-width:1.5}}\
         .packet-field-label{{fill:var(--_text)}}\
         .packet-bit-num{{fill:var(--_text-sec)}}\
         .packet-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );
    for field in &placed.fields {
        out.push(field_node(field));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            600,
            Anchor::Middle,
            "packet-title",
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

    fn content_of(node: &Node) -> String {
        match &node.content {
            Content::Text(run) => run.content.clone(),
            _ => String::new(),
        }
    }

    #[test]
    fn every_field_is_addressable_and_names_its_bits() {
        let nodes = all(&drawn(
            "packet\n0-15: \"Source Port\"\n16-31: \"Dest Port\"",
        ));
        let fields = with_class(&nodes, "node");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id.as_deref(), Some("source-port"));
        assert!(fields[0].data.contains(&("start".into(), "0".into())));
        assert!(fields[0].data.contains(&("end".into(), "15".into())));
    }

    #[test]
    fn a_wrapped_field_keeps_all_its_rectangles_in_one_identity() {
        let nodes = all(&drawn("packet\n24-39: wraps"));
        assert_eq!(with_class(&nodes, "node").len(), 1);
        assert_eq!(with_class(&nodes, "packet-field-rect").len(), 2);
        // Its name is written on both halves, so neither reads as unlabelled.
        assert_eq!(with_class(&nodes, "packet-field-label").len(), 2);
    }

    #[test]
    fn a_range_is_numbered_at_both_ends_and_a_single_bit_once() {
        let range = all(&drawn("packet\n0-7: a"));
        let numbers: Vec<String> = with_class(&range, "packet-bit-num")
            .iter()
            .map(|n| content_of(n))
            .collect();
        assert_eq!(numbers, ["0", "7"]);
        let single = all(&drawn("packet\n3: a"));
        assert_eq!(with_class(&single, "packet-bit-num").len(), 1);
    }

    #[test]
    fn a_bit_number_sits_on_its_own_baseline_above_the_rectangle() {
        let nodes = all(&drawn("packet\n0-7: a"));
        let number = with_class(&nodes, "packet-bit-num")[0];
        let label = with_class(&nodes, "packet-field-label")[0];
        assert!(matches!(&number.content, Content::Text(run) if run.dy.is_none()));
        assert!(matches!(&label.content, Content::Text(run) if run.dy.is_some()));
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(
            with_class(&all(&drawn("packet title Frame\n0: a")), "packet-title").len(),
            1
        );
        assert!(with_class(&all(&drawn("packet\n0: a")), "packet-title").is_empty());
    }

    #[test]
    fn a_diagram_of_nothing_still_yields_a_canvas() {
        let scene = drawn("packet");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render("packet\n0: a", &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
