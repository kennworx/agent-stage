//! A placed event model, drawn into the scene.
//!
//! Identity contract: each frame is a group carrying `data-id`, the kind it is
//! and its name; each connector names the two frames it joins.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Color, Content, Font, Layer, Marker, Node, Paint, Point, Role, Scene, Shape, Size,
    TextRun,
};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{layout, Placed, PlacedFrame, PlacedLane, PlacedRelation, TITLE_FONT};

const BASELINE: &str = "0.35em";
const ARROW_ID: &str = "em-arrow";
const ARROW_W: f64 = 8.0;
const ARROW_H: f64 = 5.0;
const TITLE_WEIGHT: u32 = 600;
const LANE_FONT: f64 = 13.0;
const LANE_WEIGHT: u32 = 600;
const AXIS_FONT: f64 = 12.0;
const AXIS_WEIGHT: u32 = 600;
const NAME_FONT: f64 = 13.0;
const NAME_WEIGHT: u32 = 600;
const TYPE_FONT: f64 = 10.0;
const TYPE_WEIGHT: u32 = 500;
/// Below this a box has no room for a caption under its name.
const CAPTION_MIN_HEIGHT: f64 = 40.0;

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

/// A band, and the name of the lane it stands for.
fn lane_nodes(lane: &PlacedLane, index: usize, width: f64) -> Vec<Node> {
    let mut band = Node::new(
        Role::Frame,
        Content::Shape(Shape::Rect {
            at: Point::new(0.0, lane.y),
            size: Size {
                width,
                height: lane.height,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    )
    .classed("em-lane");
    // Every other band is tinted, which is what separates them without lines.
    if index % 2 == 1 {
        band = band.classed("em-lane-alt");
    }
    vec![
        band,
        text(
            lane.label_at,
            lane.lane.label(),
            LANE_FONT,
            LANE_WEIGHT,
            Anchor::Start,
            "em-lane-label",
        )
        .on(Layer::Frame),
    ]
}

fn relation_node(relation: &PlacedRelation) -> Node {
    Node::new(
        Role::Edge,
        Content::Shape(Shape::Polyline(vec![relation.a, relation.b])),
    )
    .classed("edge")
    .tagged("from", relation.from.clone())
    .tagged("to", relation.to.clone())
    .painted(Paint {
        marker_end: Some(ARROW_ID.to_string()),
        ..Paint::default()
    })
}

fn frame_node(frame: &PlacedFrame) -> Node {
    let centre = Point::new(
        frame.at.x + frame.width / 2.0,
        frame.at.y + frame.height / 2.0,
    );
    let with_caption = frame.height >= CAPTION_MIN_HEIGHT;
    let mut parts = vec![
        Node::new(
            Role::Node,
            Content::Shape(Shape::Rect {
                at: frame.at,
                size: Size {
                    width: frame.width,
                    height: frame.height,
                },
                rx: 6.0,
                ry: 6.0,
            }),
        )
        .classed("em-box")
        .classed(format!("em-color-{}", frame.entity.color_index())),
        text(
            Point::new(
                centre.x,
                if with_caption {
                    centre.y - 7.0
                } else {
                    centre.y
                },
            ),
            &frame.name,
            NAME_FONT,
            NAME_WEIGHT,
            Anchor::Middle,
            "em-box-name",
        ),
    ];
    if with_caption {
        parts.push(text(
            Point::new(centre.x, centre.y + 13.0),
            frame.entity.caption(),
            TYPE_FONT,
            TYPE_WEIGHT,
            Anchor::Middle,
            "em-box-type",
        ));
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(frame.id.clone())
        .tagged("type", frame.entity.token())
        .tagged("label", frame.name.clone())
}

fn arrow_marker() -> Marker {
    let arrow = Color::Token {
        name: "_arrow".into(),
        fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
    };
    Marker {
        id: ARROW_ID.to_string(),
        view: Size {
            width: ARROW_W,
            height: ARROW_H,
        },
        size: Size {
            width: ARROW_W,
            height: ARROW_H,
        },
        ref_x: ARROW_W - 1.0,
        ref_y: ARROW_H / 2.0,
        shape: Shape::Polygon(vec![
            Point::new(0.0, 0.0),
            Point::new(ARROW_W, ARROW_H / 2.0),
            Point::new(0.0, ARROW_H),
        ]),
        paint: Paint {
            fill: Some(arrow.clone()),
            stroke: Some(arrow),
            stroke_width: Some(0.75),
            ..Paint::default()
        },
    }
}

/// The rules an event model needs on top of the shared tokens.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let mut indices: Vec<usize> = placed
        .frames
        .iter()
        .map(|f| f.entity.color_index())
        .collect();
    indices.sort_unstable();
    indices.dedup();
    let colors: String = indices
        .iter()
        .map(|index| {
            format!(
                ".em-color-{index}{{fill:{}}}",
                series_css(*index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .em-lane{{fill:var(--ags-bg)}}\
         .em-lane-alt{{fill:var(--_group-hdr)}}\
         .em-lane-label{{fill:var(--_text-sec)}}\
         .em-axis-label{{fill:var(--_text-sec)}}\
         .em-box{{stroke:var(--ags-bg);stroke-width:1}}\
         .em-box-name{{fill:var(--ags-bg)}}\
         .em-box-type{{fill:var(--ags-bg);opacity:0.85}}\
         .edge{{fill:none;stroke:var(--_line);stroke-width:1}}\
         .em-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed event model.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    if !placed.relations.is_empty() {
        out.markers.push(arrow_marker());
    }
    for (index, lane) in placed.lanes.iter().enumerate() {
        for node in lane_nodes(lane, index, placed.width) {
            out.push(node);
        }
    }
    for relation in &placed.relations {
        out.push(relation_node(relation));
    }
    for frame in &placed.frames {
        out.push(frame_node(frame));
    }
    for (number, at) in &placed.axis {
        out.push(text(
            *at,
            number,
            AXIS_FONT,
            AXIS_WEIGHT,
            Anchor::Middle,
            "em-axis-label",
        ));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "em-title",
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

    const MODEL: &str = "eventmodeling\n\
        title Ordering\n\
        tf 01 ui Basket\n\
        tf 02 cmd PlaceOrder\n\
        tf 03 evt OrderPlaced";

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
    fn every_frame_is_addressable_and_says_what_kind_it_is() {
        let nodes = all(&drawn(MODEL));
        let frames = with_class(&nodes, "node");
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].id.as_deref(), Some("01"));
        assert!(frames[0].data.contains(&("type".into(), "ui".into())));
        assert!(frames[2].data.contains(&("type".into(), "evt".into())));
    }

    #[test]
    fn a_frame_writes_its_name_and_its_kind() {
        let nodes = all(&drawn(MODEL));
        assert_eq!(with_class(&nodes, "em-box-name").len(), 3);
        let captions: Vec<String> = with_class(&nodes, "em-box-type")
            .iter()
            .filter_map(|n| match &n.content {
                Content::Text(run) => Some(run.content.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(captions, ["UI", "Command", "Event"]);
    }

    #[test]
    fn every_other_band_is_tinted() {
        let nodes = all(&drawn(MODEL));
        assert_eq!(with_class(&nodes, "em-lane").len(), 3);
        assert_eq!(with_class(&nodes, "em-lane-alt").len(), 1, "the middle one");
    }

    #[test]
    fn the_bands_paint_behind_everything_on_them() {
        let scene = drawn(MODEL);
        let layers: Vec<(Layer, &str)> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(|c| (n.layer, c.as_str())))
            .collect();
        let first_edge = layers
            .iter()
            .position(|(_, c)| *c == "edge")
            .expect("an edge");
        assert!(layers
            .iter()
            .take(first_edge)
            .all(|(l, _)| *l == Layer::Frame));
    }

    #[test]
    fn the_sequence_is_drawn_as_arrows_between_consecutive_frames() {
        let nodes = all(&drawn(MODEL));
        let edges = with_class(&nodes, "edge");
        assert_eq!(edges.len(), 2);
        assert!(edges[0].data.contains(&("from".into(), "01".into())));
        assert_eq!(drawn(MODEL).markers.len(), 1);
        // One frame needs no arrow at all.
        assert!(drawn("eventmodeling\ntf 1 ui A").markers.is_empty());
    }

    #[test]
    fn every_frame_is_numbered_along_the_top() {
        assert_eq!(with_class(&all(&drawn(MODEL)), "em-axis-label").len(), 3);
    }

    #[test]
    fn one_fill_rule_is_emitted_per_kind_in_use() {
        let style = drawn(MODEL).style;
        assert!(
            style.contains(".em-color-0{fill:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains(".em-color-2{fill:hsl(from"), "{style}");
        // Nothing is a processor here, so slot 1 goes unused.
        assert!(!style.contains(".em-color-1{"), "{style}");
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(MODEL)), "em-title").len(), 1);
        assert!(with_class(&all(&drawn("eventmodeling\ntf 1 ui A")), "em-title").is_empty());
    }

    #[test]
    fn a_model_of_nothing_still_draws_its_lanes() {
        let scene = drawn("eventmodeling");
        assert!(scene.canvas.width > 0.0);
        assert_eq!(with_class(&all(&scene), "em-lane").len(), 3);
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(MODEL, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
