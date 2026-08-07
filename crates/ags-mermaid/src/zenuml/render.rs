//! A placed `ZenUML` diagram, drawn into the scene.
//!
//! Identity contract: each participant is a group carrying `data-id`, and each
//! message a group carrying `data-from` and `data-to`.
//!
//! Nothing inside those groups carries a class of its own, so the styling is
//! written as descendant rules. That is not shorthand — the two boxes in a
//! fragment and the two lines of text in an annotated participant differ only by
//! their position in the group, and giving them classes would make the drawing
//! disagree with the reference it is checked against.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Color, Content, Font, Marker, Node, Paint, Point, Role, Scene, Shape, Size, TextRun,
};
use crate::theme::{style_block, Theme};

use super::layout::{
    divider_label, layout, tab_label, tab_width, Lifeline, Placed, PlacedFragment, PlacedMessage,
    PlacedParticipant, EDGE_FONT, EDGE_WEIGHT, LABEL_FONT, LABEL_WEIGHT, SELF_LABEL_PAD,
    SELF_LOOP_HEIGHT, SELF_LOOP_WIDTH, TAB_HEIGHT, TAB_TEXT_PAD, TAB_WEIGHT,
};
use super::types::ArrowHead;

const BASELINE: &str = "0.35em";
const ARROW_ID: &str = "zen-arrow";
const ARROW_OPEN_ID: &str = "zen-arrow-open";
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 5.0;
const BOX_RADIUS: f64 = 4.0;
/// How far a stereotype and its name sit either side of the box's middle.
const ANNOTATOR_OFFSET: f64 = 7.0;
/// How far a message's label floats above its line.
const LABEL_LIFT: f64 = 8.0;
const DIVIDER_TEXT_PAD: f64 = 8.0;
const DIVIDER_TEXT_DROP: f64 = 13.0;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn text(at: Point, content: &str, font: f64, weight: u32, anchor: Anchor) -> Node {
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
            content: content.to_string(),
        }),
    )
}

fn rect(role: Role, at: Point, width: f64, height: f64, radius: f64) -> Node {
    Node::new(
        role,
        Content::Shape(Shape::Rect {
            at,
            size: size(width, height),
            rx: radius,
            ry: radius,
        }),
    )
}

fn arrow_marker(id: &str, filled: bool) -> Marker {
    let tip = vec![
        Point::new(0.0, 0.0),
        Point::new(ARROW_W, ARROW_H / 2.0),
        Point::new(0.0, ARROW_H),
    ];
    let ink = Color::Token {
        name: "_arrow".into(),
        fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
    };
    Marker {
        id: id.to_string(),
        view: size(ARROW_W, ARROW_H),
        size: size(ARROW_W, ARROW_H),
        ref_x: ARROW_W,
        ref_y: ARROW_H / 2.0,
        shape: if filled {
            Shape::Polygon(tip)
        } else {
            Shape::Polyline(tip)
        },
        paint: Paint {
            fill: Some(if filled { ink.clone() } else { Color::None }),
            stroke: Some(ink),
            stroke_width: Some(1.0),
            ..Paint::default()
        },
    }
}

fn lifeline_node(lifeline: &Lifeline) -> Node {
    Node::new(
        Role::Frame,
        Content::Shape(Shape::Line {
            a: Point::new(lifeline.x, lifeline.top),
            b: Point::new(lifeline.x, lifeline.bottom),
        }),
    )
    .classed("lifeline")
    .tagged("participant", lifeline.participant.clone())
}

/// The name a participant is drawn with, and its stereotype above it when it has
/// one.
fn participant_text(participant: &PlacedParticipant) -> Vec<Node> {
    let middle = participant.y + participant.height / 2.0;
    let Some(annotator) = &participant.annotator else {
        return vec![text(
            Point::new(participant.x, middle),
            &participant.label,
            LABEL_FONT,
            LABEL_WEIGHT,
            Anchor::Middle,
        )];
    };
    vec![
        text(
            Point::new(participant.x, middle - ANNOTATOR_OFFSET),
            &format!("«{annotator}»"),
            EDGE_FONT,
            EDGE_WEIGHT,
            Anchor::Middle,
        ),
        text(
            Point::new(participant.x, middle + ANNOTATOR_OFFSET),
            &participant.label,
            LABEL_FONT,
            LABEL_WEIGHT,
            Anchor::Middle,
        ),
    ]
}

fn participant_node(participant: &PlacedParticipant) -> Node {
    let mut children = vec![rect(
        Role::Node,
        Point::new(participant.x - participant.width / 2.0, participant.y),
        participant.width,
        participant.height,
        BOX_RADIUS,
    )];
    children.extend(participant_text(participant));
    let node = Node::new(Role::Node, Content::Group(children))
        .classed("node")
        .with_id(participant.id.clone())
        .tagged("label", participant.label.clone());
    match &participant.annotator {
        Some(annotator) => node.tagged("annotator", annotator.clone()),
        None => node,
    }
}

/// A self-message's loop out and back, and the label beside it.
///
/// The label element is written even when the message has none, which is what
/// the reference does — a self-message is the one arrow whose text is not
/// conditional.
fn self_message_parts(message: &PlacedMessage, paint: Paint) -> Vec<Node> {
    let out = message.x1 + SELF_LOOP_WIDTH;
    let bottom = message.y + SELF_LOOP_HEIGHT;
    vec![
        Node::new(
            Role::Edge,
            Content::Shape(Shape::Polyline(vec![
                Point::new(message.x1, message.y),
                Point::new(out, message.y),
                Point::new(out, bottom),
                Point::new(message.x2, bottom),
            ])),
        )
        .painted(paint),
        text(
            Point::new(out + SELF_LABEL_PAD, message.y + SELF_LOOP_HEIGHT / 2.0),
            &message.label,
            EDGE_FONT,
            EDGE_WEIGHT,
            Anchor::Start,
        ),
    ]
}

fn straight_message_parts(message: &PlacedMessage, paint: Paint) -> Vec<Node> {
    let mut out = vec![Node::new(
        Role::Edge,
        Content::Shape(Shape::Line {
            a: Point::new(message.x1, message.y),
            b: Point::new(message.x2, message.y),
        }),
    )
    .painted(paint)];
    if !message.label.is_empty() {
        out.push(text(
            Point::new(
                f64::midpoint(message.x1, message.x2),
                message.y - LABEL_LIFT,
            ),
            &message.label,
            EDGE_FONT,
            EDGE_WEIGHT,
            Anchor::Middle,
        ));
    }
    out
}

fn message_node(message: &PlacedMessage) -> Node {
    let paint = Paint {
        marker_end: Some(
            match message.arrow_head {
                ArrowHead::Filled => ARROW_ID,
                ArrowHead::Open => ARROW_OPEN_ID,
            }
            .to_string(),
        ),
        ..Paint::default()
    };
    let parts = if message.self_call {
        self_message_parts(message, paint)
    } else {
        straight_message_parts(message, paint)
    };
    Node::new(Role::Edge, Content::Group(parts))
        .classed("message")
        .tagged("from", message.from.clone())
        .tagged("to", message.to.clone())
        .tagged("label", message.label.clone())
        .tagged("kind", message.kind.token())
        .tagged("line-style", message.line_style.token())
        .tagged("self", message.self_call.to_string())
}

fn fragment_node(fragment: &PlacedFragment) -> Node {
    let mut children = vec![
        rect(
            Role::Frame,
            fragment.at,
            fragment.width,
            fragment.height,
            0.0,
        ),
        rect(
            Role::Frame,
            fragment.at,
            tab_width(fragment.kind, &fragment.label),
            TAB_HEIGHT,
            0.0,
        ),
        text(
            Point::new(
                fragment.at.x + TAB_TEXT_PAD,
                fragment.at.y + TAB_HEIGHT / 2.0,
            ),
            &tab_label(fragment.kind, &fragment.label),
            EDGE_FONT,
            TAB_WEIGHT,
            Anchor::Start,
        ),
    ];
    for divider in &fragment.dividers {
        children.push(Node::new(
            Role::Frame,
            Content::Shape(Shape::Line {
                a: Point::new(fragment.at.x, divider.y),
                b: Point::new(fragment.at.x + fragment.width, divider.y),
            }),
        ));
        children.push(text(
            Point::new(
                fragment.at.x + DIVIDER_TEXT_PAD,
                divider.y + DIVIDER_TEXT_DROP,
            ),
            &divider_label(divider),
            EDGE_FONT,
            EDGE_WEIGHT,
            Anchor::Start,
        ));
    }
    let node = Node::new(Role::Frame, Content::Group(children))
        .classed("fragment")
        .tagged("type", fragment.kind.token())
        .tagged("depth", fragment.depth.to_string());
    if fragment.label.is_empty() {
        node
    } else {
        node.tagged("label", fragment.label.clone())
    }
}

/// The rules a `ZenUML` diagram needs on top of the shared tokens.
///
/// The two positional selectors carry real meaning: a fragment's tab is the box
/// that follows the box, and a divider's caption is text that follows a rule.
fn style(theme: &Theme, mode: &ColorMode) -> String {
    format!(
        "{}\
         .lifeline{{stroke:var(--_line);stroke-width:0.75;stroke-dasharray:6 4}}\
         .node rect{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .node text{{fill:var(--_text)}}\
         .node[data-annotator] text:first-of-type{{fill:var(--_text-sec)}}\
         .fragment rect{{fill:none;stroke:var(--_node-stroke);stroke-width:1}}\
         .fragment rect+rect{{fill:var(--_group-hdr)}}\
         .fragment line{{stroke:var(--_line);stroke-width:0.75;stroke-dasharray:6 4}}\
         .fragment text{{fill:var(--_text-sec)}}\
         .fragment line~text{{fill:var(--_text-muted)}}\
         .message line,.message polyline{{fill:none;stroke:var(--_line);stroke-width:1}}\
         .message[data-line-style=\"dashed\"] line,\
         .message[data-line-style=\"dashed\"] polyline{{stroke-dasharray:6 4}}\
         .message text{{fill:var(--_text-muted)}}\
         .message[data-kind=\"return\"] text{{fill:var(--_text-sec)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    )
}

/// Draw a placed `ZenUML` diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(theme, mode);
    if !placed.messages.is_empty() {
        out.markers.push(arrow_marker(ARROW_ID, true));
        out.markers.push(arrow_marker(ARROW_OPEN_ID, false));
    }
    for lifeline in &placed.lifelines {
        out.push(lifeline_node(lifeline));
    }
    for fragment in &placed.fragments {
        out.push(fragment_node(fragment));
    }
    for message in &placed.messages {
        out.push(message_node(message));
    }
    for participant in &placed.participants {
        out.push(participant_node(participant));
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

    const FLOW: &str = "zenuml\nAlice->Bob: Request\nBob.process()\nBob->Alice: Response";

    fn drawn(source: &str) -> Scene {
        render(source, &Theme::default(), &ColorMode::Tokens)
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

    fn datum<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
        node.data
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn every_participant_is_addressable() {
        let nodes = all(&drawn(FLOW));
        let boxes = with_class(&nodes, "node");
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].id.as_deref(), Some("Alice"));
        assert_eq!(datum(boxes[0], "label"), Some("Alice"));
    }

    #[test]
    fn every_message_names_both_ends() {
        let nodes = all(&drawn(FLOW));
        let messages = with_class(&nodes, "message");
        assert_eq!(messages.len(), 4);
        assert_eq!(datum(messages[0], "from"), Some("Alice"));
        assert_eq!(datum(messages[0], "to"), Some("Bob"));
        assert_eq!(datum(messages[0], "kind"), Some("sync"));
        assert_eq!(datum(messages[2], "kind"), Some("return"));
        assert_eq!(datum(messages[2], "line-style"), Some("dashed"));
    }

    #[test]
    fn a_lifeline_runs_under_each_participant() {
        let nodes = all(&drawn(FLOW));
        let lifelines = with_class(&nodes, "lifeline");
        assert_eq!(lifelines.len(), 2);
        assert_eq!(datum(lifelines[0], "participant"), Some("Alice"));
        let Content::Shape(Shape::Line { a, b }) = &lifelines[0].content else {
            panic!("a straight rule")
        };
        assert!((a.x - b.x).abs() < 1e-9, "vertical");
    }

    #[test]
    fn a_stereotype_is_drawn_above_the_name_it_qualifies() {
        let nodes = all(&drawn("zenuml\n@Actor User\nUser->User: wait"));
        let node = with_class(&nodes, "node")[0];
        assert_eq!(datum(node, "annotator"), Some("Actor"));
        let Content::Group(children) = &node.content else {
            panic!("a group")
        };
        let texts: Vec<&TextRun> = children
            .iter()
            .filter_map(|child| match &child.content {
                Content::Text(run) => Some(run),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].content, "«Actor»");
        assert_eq!(texts[1].content, "User");
        assert!(texts[0].at.y < texts[1].at.y);
    }

    #[test]
    fn a_participant_with_no_stereotype_is_one_line_of_text() {
        let nodes = all(&drawn(FLOW));
        let node = with_class(&nodes, "node")[0];
        assert_eq!(datum(node, "annotator"), None);
        let Content::Group(children) = &node.content else {
            panic!("a group")
        };
        assert_eq!(children.len(), 2, "a box and its name");
    }

    #[test]
    fn a_self_message_loops_out_and_back() {
        let nodes = all(&drawn("zenuml\nA\nA->A: think"));
        let message = with_class(&nodes, "message")[0];
        assert_eq!(datum(message, "self"), Some("true"));
        let Content::Group(children) = &message.content else {
            panic!("a group")
        };
        let Content::Shape(Shape::Polyline(points)) = &children[0].content else {
            panic!("a loop")
        };
        assert_eq!(points.len(), 4);
        assert!((points[0].y - points[1].y).abs() < 1e-9);
        assert!(points[2].y > points[0].y, "it drops before returning");
    }

    #[test]
    fn a_self_message_keeps_its_label_element_even_when_empty() {
        // The reference writes the element unconditionally, so a reply with no
        // value still has one to match.
        let nodes = all(&drawn("zenuml\nA\nA.think()"));
        let messages = with_class(&nodes, "message");
        let Content::Group(children) = &messages[1].content else {
            panic!("a group")
        };
        assert_eq!(datum(messages[1], "kind"), Some("return"));
        assert_eq!(children.len(), 2, "the loop and an empty label");
        let Content::Text(run) = &children[1].content else {
            panic!("a label")
        };
        assert!(run.content.is_empty());
    }

    #[test]
    fn a_straight_message_drops_a_label_it_does_not_have() {
        let nodes = all(&drawn(FLOW));
        let messages = with_class(&nodes, "message");
        let Content::Group(labelled) = &messages[0].content else {
            panic!("a group")
        };
        let Content::Group(bare) = &messages[2].content else {
            panic!("a group")
        };
        assert_eq!(labelled.len(), 2);
        assert_eq!(bare.len(), 1, "an empty reply is just a line");
    }

    #[test]
    fn a_fragment_is_a_box_with_a_tab() {
        let nodes = all(&drawn("zenuml\nA\nB\nloop (3 times) {\nA->B: poll\n}"));
        let fragment = with_class(&nodes, "fragment")[0];
        assert_eq!(datum(fragment, "type"), Some("loop"));
        assert_eq!(datum(fragment, "label"), Some("3 times"));
        assert_eq!(datum(fragment, "depth"), Some("0"));
        let Content::Group(children) = &fragment.content else {
            panic!("a group")
        };
        assert_eq!(children.len(), 3, "a box, a tab, and the tab's words");
        let Content::Text(run) = &children[2].content else {
            panic!("the tab's words")
        };
        assert_eq!(run.content, "loop [3 times]");
    }

    #[test]
    fn a_fragment_with_no_condition_carries_no_label() {
        let nodes = all(&drawn("zenuml\nA\nB\npar {\nA->B: x\n}"));
        let fragment = with_class(&nodes, "fragment")[0];
        assert_eq!(datum(fragment, "label"), None);
    }

    #[test]
    fn a_section_adds_a_rule_and_a_caption() {
        let nodes = all(&drawn(
            "zenuml\nA\nB\nalt (ok) {\nA->B: yes\n} else {\nA->B: no\n}",
        ));
        let fragment = with_class(&nodes, "fragment")[0];
        let Content::Group(children) = &fragment.content else {
            panic!("a group")
        };
        assert_eq!(children.len(), 5, "the box, its tab, and the divider");
        let Content::Text(run) = &children[4].content else {
            panic!("the divider's words")
        };
        assert_eq!(run.content, "else");
    }

    #[test]
    fn the_boxes_paint_over_the_lines_that_run_under_them() {
        let scene = drawn(FLOW);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|node| node.class.first().map(String::as_str))
            .collect();
        let first_node = order.iter().position(|c| *c == "node").expect("a box");
        let last_line = order
            .iter()
            .rposition(|c| *c == "lifeline" || *c == "message")
            .expect("a line");
        assert!(last_line < first_node);
    }

    #[test]
    fn a_message_carries_the_arrowhead_its_flavour_asks_for() {
        let scene = drawn(FLOW);
        assert_eq!(scene.markers.len(), 2);
        let nodes = all(&scene);
        let messages = with_class(&nodes, "message");
        let head = |node: &Node| {
            let Content::Group(children) = &node.content else {
                panic!("a group")
            };
            children[0].paint.marker_end.clone()
        };
        assert_eq!(head(messages[0]).as_deref(), Some(ARROW_ID));
        assert_eq!(head(messages[2]).as_deref(), Some(ARROW_OPEN_ID));
    }

    #[test]
    fn an_open_head_is_a_line_and_a_filled_one_is_solid() {
        let filled = arrow_marker(ARROW_ID, true);
        let open = arrow_marker(ARROW_OPEN_ID, false);
        assert!(matches!(filled.shape, Shape::Polygon(_)));
        assert!(matches!(open.shape, Shape::Polyline(_)));
        assert_eq!(open.paint.fill, Some(Color::None));
        assert!((filled.ref_x - ARROW_W).abs() < 1e-9);
    }

    #[test]
    fn a_diagram_of_nothing_draws_nothing() {
        let scene = drawn("zenuml");
        assert!(scene.nodes.is_empty());
        assert!(scene.markers.is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(FLOW, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
