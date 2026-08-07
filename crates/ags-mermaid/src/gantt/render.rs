//! A placed gantt chart, drawn into the scene.
//!
//! Identity contract: each task is a group carrying `data-id` — a bar or a
//! milestone diamond, whichever it is.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Seg, Shape, Size, TextRun, Transform,
};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{
    layout, GridLine, Placed, PlacedSection, PlacedTask, HEADER_FONT, HEADER_WEIGHT,
    MILESTONE_RADIUS, TASK_FONT, TASK_WEIGHT, TITLE_FONT,
};
use super::types::Status;

const BASELINE: &str = "0.35em";
const TITLE_WEIGHT: u32 = 600;
const SECTION_FONT: f64 = 12.0;
const SECTION_WEIGHT: u32 = 600;

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

/// A label turned a quarter turn about its own anchor.
fn turned(node: Node, about: Point) -> Node {
    let mut node = node;
    node.transform = Some(Transform::Rotate { deg: -90.0, about });
    node
}

fn band_node(section: &PlacedSection) -> Node {
    Node::new(
        Role::Frame,
        Content::Shape(Shape::Rect {
            at: section.band.at,
            size: Size {
                width: section.band.width,
                height: section.band.height,
            },
            rx: 0.0,
            ry: 0.0,
        }),
    )
    .classed("gantt-band")
}

fn grid_nodes(line: &GridLine) -> Vec<Node> {
    let mut out = vec![Node::new(
        Role::Frame,
        Content::Shape(Shape::Line {
            a: Point::new(line.x, line.y1),
            b: Point::new(line.x, line.y2),
        }),
    )
    .classed("gantt-grid")];
    if let Some((label, at)) = &line.label {
        // Turned to read bottom-to-top, anchored at its start so it grows
        // upward into the header band. A date written flat would overlap its
        // neighbour a day away.
        out.push(turned(
            text(
                *at,
                label,
                HEADER_FONT,
                HEADER_WEIGHT,
                Anchor::Start,
                "gantt-header",
            )
            .on(Layer::Frame),
            *at,
        ));
    }
    out
}

/// The status classes a task carries, `milestone` excepted — that one is a
/// shape rather than a style.
fn status_classes(task: &PlacedTask) -> Vec<String> {
    task.tags
        .iter()
        .filter(|tag| **tag != Status::Milestone)
        .map(|tag| format!("gantt-{}", tag.token()))
        .collect()
}

fn task_node(task: &PlacedTask) -> Node {
    let shape = if task.milestone {
        let m = MILESTONE_RADIUS;
        Node::new(
            Role::Node,
            Content::Shape(Shape::Path(vec![
                Seg::MoveTo(Point::new(task.centre.x, task.centre.y - m)),
                Seg::LineTo(Point::new(task.centre.x + m, task.centre.y)),
                Seg::LineTo(Point::new(task.centre.x, task.centre.y + m)),
                Seg::LineTo(Point::new(task.centre.x - m, task.centre.y)),
                Seg::Close,
            ])),
        )
        .classed("gantt-milestone")
    } else {
        Node::new(
            Role::Node,
            Content::Shape(Shape::Rect {
                at: task.bar.at,
                size: Size {
                    width: task.bar.width,
                    height: task.bar.height,
                },
                rx: 3.0,
                ry: 3.0,
            }),
        )
        .classed("gantt-bar")
    };
    let mut shape = shape.classed(format!("gantt-sec-{}", task.color_index));
    for class in status_classes(task) {
        shape = shape.classed(class);
    }
    Node::new(Role::Node, Content::Group(vec![shape]))
        .classed("node")
        .with_id(task.id.clone())
}

/// The rules a gantt chart needs on top of the shared tokens.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    // At least one, so a chart with no sections still emits a rule its bars
    // can reference.
    let sections = placed.sections.len().max(1);
    let colors: String = (0..sections)
        .map(|index| {
            format!(
                ".gantt-sec-{index}{{fill:{}}}",
                series_css(index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let accent = series_css(0, mode, theme);
    format!(
        "{}\
         .gantt-band{{fill:var(--_inner-stroke);opacity:0.4}}\
         .gantt-grid{{stroke:var(--_line);stroke-width:0.75;opacity:0.45}}\
         .gantt-header{{fill:var(--_text-sec)}}\
         .gantt-bar{{stroke:var(--ags-bg);stroke-width:1}}\
         .gantt-milestone{{stroke:var(--ags-bg);stroke-width:1}}\
         .gantt-task-label{{fill:var(--_text)}}\
         .gantt-section-label{{fill:var(--_text-sec)}}\
         .gantt-title{{fill:var(--_text)}}\
         .gantt-done{{opacity:0.55}}\
         .gantt-active{{stroke:{accent};stroke-width:1.5;stroke-dasharray:4 2}}\
         .gantt-crit{{stroke:{accent};stroke-width:2}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed gantt chart.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);

    // Every other band is tinted, which separates the sections without a rule.
    for section in placed.sections.iter().skip(1).step_by(2) {
        out.push(band_node(section));
    }
    for line in &placed.grid_lines {
        for node in grid_nodes(line) {
            out.push(node);
        }
    }
    for task in &placed.tasks {
        out.push(task_node(task));
    }
    for task in &placed.tasks {
        out.push(text(
            task.label_at,
            &task.name,
            TASK_FONT,
            TASK_WEIGHT,
            Anchor::End,
            "gantt-task-label",
        ));
    }
    for section in &placed.sections {
        out.push(turned(
            text(
                section.label_at,
                &section.name,
                SECTION_FONT,
                SECTION_WEIGHT,
                Anchor::Middle,
                "gantt-section-label",
            ),
            section.label_at,
        ));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "gantt-title",
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

    const CHART: &str = "gantt\n\
        title A plan\n\
        section Build\n\
        Design    :done, des, 2024-01-01, 5d\n\
        Implement :active, imp, after des, 10d\n\
        section Ship\n\
        Release   :milestone, rel, after imp, 0d";

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
    fn every_task_is_addressable() {
        let nodes = all(&drawn(CHART));
        let tasks = with_class(&nodes, "node");
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id.as_deref(), Some("des"));
    }

    #[test]
    fn a_milestone_is_a_diamond_and_a_task_is_a_bar() {
        let nodes = all(&drawn(CHART));
        assert_eq!(with_class(&nodes, "gantt-bar").len(), 2);
        let diamonds = with_class(&nodes, "gantt-milestone");
        assert_eq!(diamonds.len(), 1);
        let Content::Shape(Shape::Path(segs)) = &diamonds[0].content else {
            panic!("a closed path")
        };
        assert!(matches!(segs.last(), Some(Seg::Close)));
    }

    #[test]
    fn a_status_becomes_a_class_but_milestone_does_not() {
        let nodes = all(&drawn(CHART));
        assert_eq!(with_class(&nodes, "gantt-done").len(), 1);
        assert_eq!(with_class(&nodes, "gantt-active").len(), 1);
        // `milestone` is a shape, so it is not also a style.
        assert!(with_class(&nodes, "gantt-milestone")
            .iter()
            .all(|n| !n.class.iter().any(|c| c == "gantt-milestone-status")));
    }

    #[test]
    fn a_task_takes_its_section_colour() {
        let nodes = all(&drawn(CHART));
        assert_eq!(with_class(&nodes, "gantt-sec-0").len(), 2);
        assert_eq!(with_class(&nodes, "gantt-sec-1").len(), 1);
    }

    #[test]
    fn every_other_band_is_tinted() {
        // Two sections, so only the second gets a band.
        assert_eq!(with_class(&all(&drawn(CHART)), "gantt-band").len(), 1);
    }

    #[test]
    fn a_date_label_and_a_section_name_are_both_turned() {
        let nodes = all(&drawn(CHART));
        let header = with_class(&nodes, "gantt-header")[0];
        let section = with_class(&nodes, "gantt-section-label")[0];
        for node in [header, section] {
            let Some(Transform::Rotate { deg, about }) = node.transform else {
                panic!("a turned label")
            };
            let Content::Text(run) = &node.content else {
                panic!("text")
            };
            assert!((deg + 90.0).abs() < 1e-9);
            assert_eq!(about, run.at, "turned about its own anchor");
        }
    }

    #[test]
    fn the_grid_paints_behind_the_bars() {
        let scene = drawn(CHART);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        let first_task = order.iter().position(|c| *c == "node").expect("a task");
        assert!(order
            .iter()
            .take(first_task)
            .all(|c| *c == "gantt-band" || *c == "gantt-grid" || *c == "gantt-header"));
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(CHART)), "gantt-title").len(), 1);
        assert!(with_class(&all(&drawn("gantt\nA :1d")), "gantt-title").is_empty());
    }

    #[test]
    fn a_chart_of_nothing_still_draws_a_grid() {
        let scene = drawn("gantt");
        assert!(scene.canvas.width > 0.0);
        assert_eq!(with_class(&all(&scene), "gantt-grid").len(), 2);
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(CHART, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
