//! A placed sequence diagram, drawn into the scene.
//!
//! Identity contract: each actor is a group carrying `data-id`, each message a
//! group carrying `data-from` and `data-to`, each note a group naming the
//! lifelines it hangs from.
//!
//! Nothing inside those groups carries a class of its own, because the reference
//! emits none — the styling is written as descendant rules instead. See the
//! `style` function for the two selectors that carry real meaning.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Color, Content, Font, Marker, Node, Paint, Point, Role, Scene, Seg, Shape, Size,
    TextRun, Transform,
};
use crate::text::strip_formatting_tags;
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, Activation, Lifeline, Placed, PlacedActor, PlacedBlock, PlacedMessage, PlacedNote,
};
use super::metrics::{
    divider_label, tab_label, tab_width, EDGE_FONT, EDGE_WEIGHT, LABEL_FONT, LABEL_WEIGHT,
    SELF_LABEL_PAD, SELF_LOOP_HEIGHT, SELF_LOOP_WIDTH, TAB_HEIGHT, TAB_WEIGHT,
};
use super::types::{ActorKind, ArrowHead};

const ARROW_ID: &str = "seq-arrow";
const ARROW_OPEN_ID: &str = "seq-arrow-open";
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 5.0;
const BOX_RADIUS: f64 = 4.0;
/// How far a message's label floats above its line.
const LABEL_LIFT: f64 = 10.0;
/// How far an actor's name sits below its glyph.
const ICON_LABEL_DROP: f64 = 14.0;
const TAB_TEXT_PAD: f64 = 6.0;
const DIVIDER_TEXT_PAD: f64 = 8.0;
const DIVIDER_TEXT_DROP: f64 = 14.0;
/// The folded corner of a note.
const NOTE_FOLD: f64 = 6.0;
/// The grid the person glyph is drawn on, and how much of the box it fills.
const ICON_GRID: f64 = 24.0;
const ICON_FILL: f64 = 0.9;
/// A line of text is this much taller than the type it is set in.
const LINE_HEIGHT_RATIO: f64 = 1.3;
const BASELINE_RATIO: f64 = 0.35;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Text about a centre, one node per line.
///
/// The reference packs the lines into one `<text>` as `tspan`s; the scene has no
/// such nesting, so each line is its own run at the same offset the `tspan`
/// would have had. A single-line label — which is every label in practice — comes
/// out as exactly one node either way.
fn text(at: Point, content: &str, font: f64, weight: u32, anchor: Anchor) -> Vec<Node> {
    // Formatting tags are dropped rather than drawn: the scene has no rich text,
    // and leaving them in would put literal `<b>` on the page.
    let plain = strip_formatting_tags(content);
    let lines: Vec<&str> = plain.split('\n').collect();
    let step = font * LINE_HEIGHT_RATIO;
    let first = -(count_lines(lines.len()) / 2.0) * step + font * BASELINE_RATIO;
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let dy = if lines.len() == 1 {
                font * BASELINE_RATIO
            } else {
                first + count_index(index) * step
            };
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
                    dy: Some(format!("{dy}")),
                    content: (*line).to_string(),
                }),
            )
        })
        .collect()
}

/// `(n - 1)` as a float, for the half-step a stack of lines is lifted by.
fn count_lines(lines: usize) -> f64 {
    crate::round::count(lines.saturating_sub(1))
}

fn count_index(index: usize) -> f64 {
    crate::round::count(index)
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
        point(0.0, 0.0),
        point(ARROW_W, ARROW_H / 2.0),
        point(0.0, ARROW_H),
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

/// One outline of the person glyph, on the 24×24 grid.
fn glyph_path(segments: Vec<Seg>, stroke_width: f64) -> Node {
    Node::new(Role::Icon, Content::Shape(Shape::Path(segments))).painted(Paint {
        fill: Some(Color::None),
        stroke: Some(Color::Token {
            name: "_line".into(),
            fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
        }),
        stroke_width: Some(stroke_width),
        ..Paint::default()
    })
}

/// A person in a circle: the ring, the head, and the shoulders.
fn person_glyph(actor: &PlacedActor) -> Node {
    let scale = (actor.height / ICON_GRID) * ICON_FILL;
    // The stroke is drawn inside the scaled group, so it has to be divided back
    // out or a small glyph would come out with a hairline and a large one with a
    // slab.
    let stroke = 1.0 / scale;
    let ring = vec![
        Seg::MoveTo(point(21.0, 12.0)),
        Seg::Cubic {
            c1: point(21.0, 16.9706),
            c2: point(16.9706, 21.0),
            to: point(12.0, 21.0),
        },
        Seg::Cubic {
            c1: point(7.02944, 21.0),
            c2: point(3.0, 16.9706),
            to: point(3.0, 12.0),
        },
        Seg::Cubic {
            c1: point(3.0, 7.02944),
            c2: point(7.02944, 3.0),
            to: point(12.0, 3.0),
        },
        Seg::Cubic {
            c1: point(16.9706, 3.0),
            c2: point(21.0, 7.02944),
            to: point(21.0, 12.0),
        },
        Seg::Close,
    ];
    let head = vec![
        Seg::MoveTo(point(15.0, 10.0)),
        Seg::Cubic {
            c1: point(15.0, 11.6569),
            c2: point(13.6569, 13.0),
            to: point(12.0, 13.0),
        },
        Seg::Cubic {
            c1: point(10.3431, 13.0),
            c2: point(9.0, 11.6569),
            to: point(9.0, 10.0),
        },
        Seg::Cubic {
            c1: point(9.0, 8.34315),
            c2: point(10.3431, 7.0),
            to: point(12.0, 7.0),
        },
        Seg::Cubic {
            c1: point(13.6569, 7.0),
            c2: point(15.0, 8.34315),
            to: point(15.0, 10.0),
        },
        Seg::Close,
    ];
    let shoulders = vec![
        Seg::MoveTo(point(5.62842, 18.3563)),
        Seg::Cubic {
            c1: point(7.08963, 17.0398),
            c2: point(9.39997, 16.0),
            to: point(12.0, 16.0),
        },
        Seg::Cubic {
            c1: point(14.6, 16.0),
            c2: point(16.9104, 17.0398),
            to: point(18.3716, 18.3563),
        },
    ];
    let mut group = Node::new(
        Role::Icon,
        Content::Group(vec![
            glyph_path(ring, stroke),
            glyph_path(head, stroke),
            glyph_path(shoulders, stroke),
        ]),
    );
    group.transform = Some(Transform::TranslateScale {
        at: point(
            actor.x - ICON_GRID / 2.0 * scale,
            actor.y + (actor.height - ICON_GRID * scale) / 2.0,
        ),
        scale,
    });
    group
}

fn actor_node(actor: &PlacedActor) -> Node {
    let mut children = Vec::new();
    let label_at = match actor.kind {
        ActorKind::Actor => {
            children.push(person_glyph(actor));
            point(actor.x, actor.y + actor.height + ICON_LABEL_DROP)
        }
        ActorKind::Participant => {
            children.push(rect(
                Role::Node,
                point(actor.x - actor.width / 2.0, actor.y),
                actor.width,
                actor.height,
                BOX_RADIUS,
            ));
            point(actor.x, actor.y + actor.height / 2.0)
        }
    };
    children.extend(text(
        label_at,
        &actor.label,
        LABEL_FONT,
        LABEL_WEIGHT,
        Anchor::Middle,
    ));
    Node::new(Role::Node, Content::Group(children))
        .classed("actor")
        .with_id(actor.id.clone())
        .tagged("label", actor.label.clone())
        .tagged("type", actor.kind.token())
}

fn lifeline_node(lifeline: &Lifeline) -> Node {
    Node::new(
        Role::Frame,
        Content::Shape(Shape::Line {
            a: point(lifeline.x, lifeline.top),
            b: point(lifeline.x, lifeline.bottom),
        }),
    )
    .classed("lifeline")
    .tagged("actor", lifeline.actor.clone())
}

fn activation_node(activation: &Activation) -> Node {
    rect(
        Role::Node,
        point(activation.x, activation.top),
        activation.width,
        activation.bottom - activation.top,
        0.0,
    )
    .classed("activation")
    .tagged("actor", activation.actor.clone())
    .on(crate::scene::Layer::Frame)
}

fn message_parts(message: &PlacedMessage, paint: Paint) -> Vec<Node> {
    if message.self_call {
        let out = message.x1 + SELF_LOOP_WIDTH;
        let bottom = message.y + SELF_LOOP_HEIGHT;
        let mut parts = vec![Node::new(
            Role::Edge,
            Content::Shape(Shape::Polyline(vec![
                point(message.x1, message.y),
                point(out, message.y),
                point(out, bottom),
                point(message.x2, bottom),
            ])),
        )
        .painted(paint)];
        parts.extend(text(
            point(out + SELF_LABEL_PAD, message.y + SELF_LOOP_HEIGHT / 2.0),
            &message.label,
            EDGE_FONT,
            EDGE_WEIGHT,
            Anchor::Start,
        ));
        return parts;
    }
    let mut parts = vec![Node::new(
        Role::Edge,
        Content::Shape(Shape::Line {
            a: point(message.x1, message.y),
            b: point(message.x2, message.y),
        }),
    )
    .painted(paint)];
    parts.extend(text(
        point(
            f64::midpoint(message.x1, message.x2),
            message.y - LABEL_LIFT,
        ),
        &message.label,
        EDGE_FONT,
        EDGE_WEIGHT,
        Anchor::Middle,
    ));
    parts
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
    Node::new(Role::Edge, Content::Group(message_parts(message, paint)))
        .classed("message")
        .tagged("from", message.from.clone())
        .tagged("to", message.to.clone())
        .tagged("label", message.label.clone())
        .tagged("line-style", message.line_style.token())
        .tagged("arrow-head", message.arrow_head.token())
        .tagged("self", message.self_call.to_string())
}

fn block_node(block: &PlacedBlock) -> Node {
    let mut children = vec![
        rect(Role::Frame, block.at, block.width, block.height, 0.0),
        rect(
            Role::Frame,
            block.at,
            tab_width(block.kind, &block.label),
            TAB_HEIGHT,
            0.0,
        ),
    ];
    children.extend(text(
        point(block.at.x + TAB_TEXT_PAD, block.at.y + TAB_HEIGHT / 2.0),
        &tab_label(block.kind, &block.label),
        EDGE_FONT,
        TAB_WEIGHT,
        Anchor::Start,
    ));
    for divider in &block.dividers {
        children.push(Node::new(
            Role::Frame,
            Content::Shape(Shape::Line {
                a: point(block.at.x, divider.y),
                b: point(block.at.x + block.width, divider.y),
            }),
        ));
        if !divider.label.is_empty() {
            children.extend(text(
                point(block.at.x + DIVIDER_TEXT_PAD, divider.y + DIVIDER_TEXT_DROP),
                &divider_label(&divider.label),
                EDGE_FONT,
                EDGE_WEIGHT,
                Anchor::Start,
            ));
        }
    }
    let node = Node::new(Role::Frame, Content::Group(children))
        .classed("block")
        .tagged("type", block.kind.token());
    if block.label.is_empty() {
        node
    } else {
        node.tagged("label", block.label.clone())
    }
}

/// A note: a page with its top-right corner turned down.
fn note_node(note: &PlacedNote) -> Node {
    let (x, y, w, h) = (note.at.x, note.at.y, note.width, note.height);
    let body = Node::new(
        Role::Node,
        Content::Shape(Shape::Polygon(vec![
            point(x, y),
            point(x + w - NOTE_FOLD, y),
            point(x + w, y + NOTE_FOLD),
            point(x + w, y + h),
            point(x, y + h),
        ])),
    );
    let fold = Node::new(
        Role::Node,
        Content::Shape(Shape::Polygon(vec![
            point(x + w - NOTE_FOLD, y),
            point(x + w, y + NOTE_FOLD),
            point(x + w - NOTE_FOLD, y + NOTE_FOLD),
        ])),
    );
    let mut children = vec![body, fold];
    children.extend(text(
        point(x + w / 2.0, y + h / 2.0),
        &note.text,
        EDGE_FONT,
        EDGE_WEIGHT,
        Anchor::Middle,
    ));
    let page = Node::new(Role::Node, Content::Group(children))
        .classed("note")
        .tagged("position", note.position.token());
    if note.actors.is_empty() {
        page
    } else {
        page.tagged("actors", note.actors.join(","))
    }
}

/// The rules a sequence diagram needs on top of the shared tokens.
///
/// Two selectors carry meaning rather than convenience: a block's tab is the box
/// that follows the box, and a note's turned corner is the polygon that follows
/// the polygon.
fn style(theme: &Theme, mode: &ColorMode) -> String {
    format!(
        "{}\
         .lifeline{{stroke:var(--_line);stroke-width:0.75;stroke-dasharray:6 4}}\
         .activation{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:0.75}}\
         .actor rect{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .actor text{{fill:var(--_text)}}\
         .block rect{{fill:none;stroke:var(--_node-stroke);stroke-width:1}}\
         .block rect+rect{{fill:var(--_group-hdr)}}\
         .block line{{stroke:var(--_line);stroke-width:0.75;stroke-dasharray:6 4}}\
         .block text{{fill:var(--_text-sec)}}\
         .block line~text{{fill:var(--_text-muted)}}\
         .message line,.message polyline{{fill:none;stroke:var(--_line);stroke-width:1}}\
         .message[data-line-style=\"dashed\"] line,\
         .message[data-line-style=\"dashed\"] polyline{{stroke-dasharray:6 4}}\
         .message text{{fill:var(--_text-muted)}}\
         .note polygon{{fill:var(--ags-bg);stroke:var(--_node-stroke);stroke-width:0.75}}\
         .note polygon+polygon{{fill:var(--_inner-stroke)}}\
         .note text{{fill:var(--_text-muted)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    )
}

/// Draw a placed sequence diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(theme, mode);
    if !placed.messages.is_empty() {
        out.markers.push(arrow_marker(ARROW_ID, true));
        out.markers.push(arrow_marker(ARROW_OPEN_ID, false));
    }
    for block in &placed.blocks {
        out.push(block_node(block));
    }
    for lifeline in &placed.lifelines {
        out.push(lifeline_node(lifeline));
    }
    for activation in &placed.activations {
        out.push(activation_node(activation));
    }
    for message in &placed.messages {
        out.push(message_node(message));
    }
    for note in &placed.notes {
        out.push(note_node(note));
    }
    for actor in &placed.actors {
        out.push(actor_node(actor));
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

    const FLOW: &str =
        "sequenceDiagram\nparticipant A as Alice\nparticipant B as Bob\nA->>B: Hello\nB-->>A: Hi";

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

    fn children(node: &Node) -> &[Node] {
        match &node.content {
            Content::Group(items) => items,
            _ => panic!("a group"),
        }
    }

    #[test]
    fn every_actor_is_addressable() {
        let nodes = all(&drawn(FLOW));
        let actors = with_class(&nodes, "actor");
        assert_eq!(actors.len(), 2);
        assert_eq!(actors[0].id.as_deref(), Some("A"));
        assert_eq!(datum(actors[0], "label"), Some("Alice"));
        assert_eq!(datum(actors[0], "type"), Some("participant"));
    }

    #[test]
    fn a_participant_is_a_box_and_an_actor_is_a_person() {
        let nodes = all(&drawn(
            "sequenceDiagram\nactor U as User\nparticipant S\nU->>S: x",
        ));
        let actors = with_class(&nodes, "actor");
        assert_eq!(datum(actors[0], "type"), Some("actor"));
        let glyph = &children(actors[0])[0];
        assert_eq!(children(glyph).len(), 3, "a ring, a head and shoulders");
        assert!(matches!(
            glyph.transform,
            Some(Transform::TranslateScale { .. })
        ));
        assert!(matches!(
            children(actors[1])[0].content,
            Content::Shape(Shape::Rect { .. })
        ));
    }

    #[test]
    fn a_person_glyph_is_scaled_to_its_box_and_keeps_its_stroke_weight() {
        let nodes = all(&drawn("sequenceDiagram\nactor U\nU->>S: x"));
        let glyph = &children(with_class(&nodes, "actor")[0])[0];
        let Some(Transform::TranslateScale { scale, .. }) = glyph.transform else {
            panic!("a scaled glyph")
        };
        let outline = &children(glyph)[0];
        let width = outline.paint.stroke_width.expect("a stroke");
        assert!((width * scale - 1.0).abs() < 1e-9, "one pixel on the page");
    }

    #[test]
    fn a_persons_name_sits_below_the_glyph_and_a_participants_inside_the_box() {
        let nodes = all(&drawn(
            "sequenceDiagram\nactor U as User\nparticipant S as Sys\nU->>S: x",
        ));
        let actors = with_class(&nodes, "actor");
        let label_y = |node: &Node| match &children(node)[1].content {
            Content::Text(run) => run.at.y,
            _ => panic!("a name"),
        };
        // Both boxes sit at the same y and are the same height, so the
        // person's name being lower is it clearing the glyph.
        assert_eq!(actors[0].id.as_deref(), Some("U"));
        assert!(label_y(actors[0]) > label_y(actors[1]));
    }

    #[test]
    fn every_message_names_both_ends() {
        let nodes = all(&drawn(FLOW));
        let messages = with_class(&nodes, "message");
        assert_eq!(messages.len(), 2);
        assert_eq!(datum(messages[0], "from"), Some("A"));
        assert_eq!(datum(messages[0], "to"), Some("B"));
        assert_eq!(datum(messages[0], "line-style"), Some("solid"));
        assert_eq!(datum(messages[0], "arrow-head"), Some("filled"));
        assert_eq!(datum(messages[1], "line-style"), Some("dashed"));
        assert_eq!(datum(messages[0], "self"), Some("false"));
    }

    #[test]
    fn a_self_message_loops_out_and_back() {
        let nodes = all(&drawn("sequenceDiagram\nS->>S: think"));
        let message = with_class(&nodes, "message")[0];
        assert_eq!(datum(message, "self"), Some("true"));
        let Content::Shape(Shape::Polyline(points)) = &children(message)[0].content else {
            panic!("a loop")
        };
        assert_eq!(points.len(), 4);
        assert!(points[2].y > points[0].y);
    }

    #[test]
    fn a_message_carries_the_arrowhead_its_operator_asks_for() {
        let scene = drawn("sequenceDiagram\nA->>B: filled\nB-)A: open");
        assert_eq!(scene.markers.len(), 2);
        let nodes = all(&scene);
        let messages = with_class(&nodes, "message");
        let head = |node: &Node| children(node)[0].paint.marker_end.clone();
        assert_eq!(head(messages[0]).as_deref(), Some(ARROW_ID));
        assert_eq!(head(messages[1]).as_deref(), Some(ARROW_OPEN_ID));
    }

    #[test]
    fn an_open_head_is_a_line_and_a_filled_one_is_solid() {
        let filled = arrow_marker(ARROW_ID, true);
        let open = arrow_marker(ARROW_OPEN_ID, false);
        assert!(matches!(filled.shape, Shape::Polygon(_)));
        assert!(matches!(open.shape, Shape::Polyline(_)));
        assert_eq!(open.paint.fill, Some(Color::None));
    }

    #[test]
    fn an_activation_is_a_bar_on_its_own_lifeline() {
        let nodes = all(&drawn("sequenceDiagram\nC->>+S: x\nS-->>-C: y"));
        let bars = with_class(&nodes, "activation");
        assert_eq!(bars.len(), 1);
        assert_eq!(datum(bars[0], "actor"), Some("S"));
        assert!(matches!(
            bars[0].content,
            Content::Shape(Shape::Rect { .. })
        ));
    }

    #[test]
    fn a_lifeline_runs_under_each_actor() {
        let nodes = all(&drawn(FLOW));
        let lifelines = with_class(&nodes, "lifeline");
        assert_eq!(lifelines.len(), 2);
        assert_eq!(datum(lifelines[0], "actor"), Some("A"));
    }

    #[test]
    fn a_block_is_a_box_with_a_tab() {
        let nodes = all(&drawn("sequenceDiagram\nloop Every 30s\nA->>B: x\nend"));
        let block = with_class(&nodes, "block")[0];
        assert_eq!(datum(block, "type"), Some("loop"));
        assert_eq!(datum(block, "label"), Some("Every 30s"));
        let parts = children(block);
        assert_eq!(parts.len(), 3, "a box, a tab, and the tab's words");
        let Content::Text(run) = &parts[2].content else {
            panic!("the tab's words")
        };
        assert_eq!(run.content, "loop [Every 30s]");
    }

    #[test]
    fn a_block_with_no_label_carries_none() {
        let nodes = all(&drawn("sequenceDiagram\nloop\nA->>B: x\nend"));
        let block = with_class(&nodes, "block")[0];
        assert_eq!(datum(block, "label"), None);
    }

    #[test]
    fn a_divider_adds_a_rule_and_a_caption() {
        let nodes = all(&drawn(
            "sequenceDiagram\nalt Valid\nS-->>C: 200\nelse Invalid\nS-->>C: 401\nend",
        ));
        let parts = children(with_class(&nodes, "block")[0]);
        assert_eq!(parts.len(), 5);
        let Content::Text(run) = &parts[4].content else {
            panic!("the caption")
        };
        assert_eq!(run.content, "[Invalid]");
    }

    #[test]
    fn a_divider_with_no_caption_draws_only_its_rule() {
        let nodes = all(&drawn(
            "sequenceDiagram\npar One\nA->>B: x\nand\nA->>C: y\nend",
        ));
        let parts = children(with_class(&nodes, "block")[0]);
        assert_eq!(parts.len(), 4, "a box, a tab, its words, and a bare rule");
    }

    #[test]
    fn a_note_is_a_page_with_a_turned_corner() {
        let nodes = all(&drawn("sequenceDiagram\nA->>B: x\nNote over A,B: hello"));
        let note = with_class(&nodes, "note")[0];
        assert_eq!(datum(note, "position"), Some("over"));
        assert_eq!(datum(note, "actors"), Some("A,B"));
        let parts = children(note);
        assert_eq!(parts.len(), 3, "a body, a fold, and its words");
        let Content::Shape(Shape::Polygon(body)) = &parts[0].content else {
            panic!("a body")
        };
        let Content::Shape(Shape::Polygon(fold)) = &parts[1].content else {
            panic!("a fold")
        };
        assert_eq!(body.len(), 5, "a rectangle with one corner cut");
        assert_eq!(fold.len(), 3);
    }

    #[test]
    fn the_actors_paint_over_everything_that_runs_under_them() {
        let scene = drawn("sequenceDiagram\nloop L\nA->>B: x\nend\nNote over A: n");
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|node| node.class.first().map(String::as_str))
            .collect();
        let first_actor = order.iter().position(|c| *c == "actor").expect("an actor");
        assert!(order.iter().take(first_actor).all(|c| *c != "actor"));
        let block = order.iter().position(|c| *c == "block").expect("a block");
        let lifeline = order.iter().position(|c| *c == "lifeline").expect("a rule");
        assert!(block < lifeline, "the box is behind the rules");
    }

    #[test]
    fn a_label_written_over_two_lines_is_drawn_over_two() {
        let nodes = all(&drawn("sequenceDiagram\nA->>B: first<br>second"));
        let parts = children(with_class(&nodes, "message")[0]);
        let lines: Vec<&str> = parts
            .iter()
            .filter_map(|part| match &part.content {
                Content::Text(run) => Some(run.content.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(lines, ["first", "second"]);
    }

    #[test]
    fn a_formatting_tag_is_dropped_rather_than_drawn() {
        let nodes = all(&drawn("sequenceDiagram\nA->>B: **bold** text"));
        let parts = children(with_class(&nodes, "message")[0]);
        let Content::Text(run) = &parts[1].content else {
            panic!("a label")
        };
        assert_eq!(run.content, "bold text");
    }

    #[test]
    fn a_diagram_of_nothing_draws_nothing() {
        let scene = drawn("sequenceDiagram");
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
