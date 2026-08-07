//! A placed journey, drawn into the scene.
//!
//! Identity contract: each task is a group carrying `data-id` and the score it
//! was given, so a reviewer can point at a step and disagree with its rating.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{
    layout, Placed, PlacedSection, PlacedTask, ScoreLine, MARKER_RADIUS, TITLE_FONT,
};

const BASELINE: &str = "0.35em";
const SECTION_FONT: f64 = 14.0;
const SECTION_WEIGHT: u32 = 600;
const AXIS_FONT: f64 = 12.0;
const AXIS_WEIGHT: u32 = 500;
const SCORE_FONT: f64 = 13.0;
const SCORE_WEIGHT: u32 = 700;
const TASK_FONT: f64 = 13.0;
const TASK_WEIGHT: u32 = 600;
const ACTOR_FONT: f64 = 11.0;
const ACTOR_WEIGHT: u32 = 400;
const TITLE_WEIGHT: u32 = 600;

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

fn rule_nodes(line: &ScoreLine) -> Vec<Node> {
    vec![
        Node::new(
            Role::Frame,
            Content::Shape(Shape::Line {
                a: line.a,
                b: line.b,
            }),
        )
        .classed("journey-grid"),
        text(
            line.label_at,
            &line.score.to_string(),
            AXIS_FONT,
            AXIS_WEIGHT,
            Anchor::End,
            "journey-axis-label",
        )
        .on(Layer::Frame),
    ]
}

fn section_nodes(section: &PlacedSection) -> Vec<Node> {
    let mut out = vec![Node::new(
        Role::Frame,
        Content::Shape(Shape::Rect {
            at: section.at,
            size: Size {
                width: section.width,
                height: section.height,
            },
            rx: 6.0,
            ry: 6.0,
        }),
    )
    .classed("journey-section")
    .classed(format!("journey-color-{}", section.color_index))];
    // The implicit section has no name, and an empty band label would be an
    // empty text element rather than nothing.
    if !section.name.is_empty() {
        out.push(
            text(
                section.label_at,
                &section.name,
                SECTION_FONT,
                SECTION_WEIGHT,
                Anchor::Middle,
                "journey-section-label",
            )
            .on(Layer::Frame),
        );
    }
    out
}

fn task_node(task: &PlacedTask) -> Node {
    let (from, to) = task.connector;
    let mut parts = vec![
        Node::new(Role::Edge, Content::Shape(Shape::Line { a: from, b: to }))
            .classed("journey-connector"),
        Node::new(
            Role::Node,
            Content::Shape(Shape::Circle {
                c: task.at,
                r: MARKER_RADIUS,
            }),
        )
        .classed("journey-marker")
        .classed(format!("journey-color-{}", task.color_index)),
        text(
            task.at,
            &task.score.to_string(),
            SCORE_FONT,
            SCORE_WEIGHT,
            Anchor::Middle,
            "journey-score",
        ),
        text(
            task.label_at,
            &task.name,
            TASK_FONT,
            TASK_WEIGHT,
            Anchor::Middle,
            "journey-task-label",
        ),
    ];
    if !task.actors.is_empty() {
        parts.push(text(
            task.actors_at,
            &task.actors.join(", "),
            ACTOR_FONT,
            ACTOR_WEIGHT,
            Anchor::Middle,
            "journey-actors",
        ));
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(task.id.clone())
        .valued(task.score.to_string())
}

/// The rules a journey needs on top of the shared tokens.
///
/// One fill rule per section, because the palette is derived per index and CSS
/// cannot compute it. A task reuses its section's rule.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let mut indices: Vec<usize> = placed.sections.iter().map(|s| s.color_index).collect();
    indices.dedup();
    let colors: String = indices
        .iter()
        .map(|index| {
            format!(
                ".journey-color-{index}{{fill:{}}}",
                series_css(*index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .journey-grid{{stroke:var(--_inner-stroke);stroke-width:1}}\
         .journey-axis-label{{fill:var(--_text-sec)}}\
         .journey-baseline{{stroke:var(--_line);stroke-width:1.5}}\
         .journey-section{{opacity:0.18}}\
         .journey-section-label{{fill:var(--_text)}}\
         .journey-connector{{stroke:var(--_line);stroke-width:1;stroke-dasharray:2 3;opacity:0.7}}\
         .journey-marker{{stroke:var(--ags-bg);stroke-width:2}}\
         .journey-score{{fill:var(--ags-bg)}}\
         .journey-task-label{{fill:var(--_text)}}\
         .journey-actors{{fill:var(--_text-sec)}}\
         .journey-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed journey.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for line in &placed.score_lines {
        for node in rule_nodes(line) {
            out.push(node);
        }
    }
    if let Some((a, b)) = placed.baseline {
        out.push(
            Node::new(Role::Frame, Content::Shape(Shape::Line { a, b }))
                .classed("journey-baseline"),
        );
    }
    for section in &placed.sections {
        for node in section_nodes(section) {
            out.push(node);
        }
    }
    for task in &placed.tasks {
        out.push(task_node(task));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "journey-title",
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

    const JOURNEY: &str = "journey\n\
        title Day\n\
        section Morning\n\
        Wake: 3: Me\n\
        Tea: 5: Me, Cat\n\
        section Evening\n\
        Sleep: 4";

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
    fn every_task_is_addressable_and_carries_its_score() {
        let nodes = all(&drawn(JOURNEY));
        let tasks = with_class(&nodes, "node");
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id.as_deref(), Some("Wake"));
        assert_eq!(tasks[0].value.as_deref(), Some("3"));
    }

    #[test]
    fn a_task_names_its_actors_only_when_it_has_some() {
        let nodes = all(&drawn(JOURNEY));
        let actors = with_class(&nodes, "journey-actors");
        assert_eq!(actors.len(), 2, "the third task named nobody");
        let Content::Text(run) = &actors[1].content else {
            panic!("text")
        };
        assert_eq!(run.content, "Me, Cat");
    }

    #[test]
    fn the_score_is_written_inside_its_own_marker() {
        let nodes = all(&drawn(JOURNEY));
        let scores = with_class(&nodes, "journey-score");
        let markers = with_class(&nodes, "journey-marker");
        let (Content::Text(run), Content::Shape(Shape::Circle { c, .. })) =
            (&scores[0].content, &markers[0].content)
        else {
            panic!("a score in a marker")
        };
        assert_eq!(run.content, "3");
        assert_eq!(run.at, *c);
    }

    #[test]
    fn an_unnamed_section_draws_a_band_but_no_label() {
        let nodes = all(&drawn("journey\nOrphan: 3"));
        assert_eq!(with_class(&nodes, "journey-section").len(), 1);
        assert!(with_class(&nodes, "journey-section-label").is_empty());
    }

    #[test]
    fn the_scale_and_the_bands_paint_behind_the_tasks() {
        let scene = drawn(JOURNEY);
        let layers: Vec<(Layer, &str)> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(|c| (n.layer, c.as_str())))
            .collect();
        let first_task = layers
            .iter()
            .position(|(_, c)| *c == "node")
            .expect("a task");
        assert!(
            layers
                .iter()
                .take(first_task)
                .all(|(l, _)| *l == Layer::Frame),
            "{layers:?}"
        );
    }

    #[test]
    fn the_whole_scale_is_ruled_and_numbered() {
        let nodes = all(&drawn(JOURNEY));
        assert_eq!(with_class(&nodes, "journey-grid").len(), 5);
        assert_eq!(with_class(&nodes, "journey-axis-label").len(), 5);
        assert_eq!(with_class(&nodes, "journey-baseline").len(), 1);
    }

    #[test]
    fn one_fill_rule_is_emitted_for_each_band() {
        let style = drawn(JOURNEY).style;
        assert!(
            style.contains(".journey-color-0{fill:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains(".journey-color-1{fill:hsl(from"), "{style}");
        assert!(!style.contains(".journey-color-2{"), "{style}");
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(JOURNEY)), "journey-title").len(), 1);
        assert!(with_class(&all(&drawn("journey\nA: 3")), "journey-title").is_empty());
    }

    #[test]
    fn a_journey_of_nothing_still_yields_a_canvas() {
        let scene = drawn("journey");
        assert!(scene.canvas.width > 0.0);
        // The scale is still drawn, so the canvas is not blank.
        assert_eq!(with_class(&all(&scene), "journey-grid").len(), 5);
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(JOURNEY, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
