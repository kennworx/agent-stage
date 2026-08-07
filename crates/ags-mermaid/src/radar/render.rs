//! A placed radar chart, drawn into the scene.
//!
//! Identity contract: each curve is a group carrying `data-id`, and *every
//! vertex* carries its own — a reviewer disagreeing with a radar chart is
//! disagreeing with one score on one axis, not with the whole shape.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Seg, Shape, Size, TextRun,
};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{
    layout, LegendRow, Placed, PlacedAxis, PlacedSeries, LEGEND_FONT, LEGEND_SWATCH, LEGEND_WEIGHT,
    POINT_RADIUS, TITLE_FONT,
};

const BASELINE: &str = "0.35em";
const AXIS_FONT: f64 = 13.0;
const AXIS_WEIGHT: u32 = 500;
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

fn spoke_node(centre: Point, axis: &PlacedAxis) -> Node {
    Node::new(
        Role::Frame,
        Content::Shape(Shape::Line {
            a: centre,
            b: axis.at,
        }),
    )
    .classed("radar-spoke")
}

fn axis_label_node(axis: &PlacedAxis) -> Node {
    text(
        axis.label_at,
        &axis.label,
        AXIS_FONT,
        AXIS_WEIGHT,
        axis.anchor,
        "radar-axis-label",
    )
    .on(Layer::Frame)
}

/// The closed polygon a curve traces, and a dot at every vertex.
fn series_node(series: &PlacedSeries) -> Node {
    let color = format!("radar-color-{}", series.color_index);
    let mut parts = Vec::new();
    if !series.points.is_empty() {
        let mut segs: Vec<Seg> = series
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i == 0 {
                    Seg::MoveTo(p.at)
                } else {
                    Seg::LineTo(p.at)
                }
            })
            .collect();
        segs.push(Seg::Close);
        parts.push(
            Node::new(Role::Node, Content::Shape(Shape::Path(segs)))
                .classed("radar-area")
                .classed(color.clone()),
        );
    }
    for point in &series.points {
        parts.push(
            Node::new(
                Role::Node,
                Content::Shape(Shape::Circle {
                    c: point.at,
                    r: POINT_RADIUS,
                }),
            )
            .classed("radar-point")
            .classed(color.clone())
            .with_id(point.id.clone())
            .tagged("axis", point.axis_id.clone())
            .valued(point.value.to_string()),
        );
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(series.id.clone())
        .tagged("label", series.label.clone())
}

fn legend_nodes(row: &LegendRow) -> Vec<Node> {
    vec![
        Node::new(
            Role::Decoration,
            Content::Shape(Shape::Rect {
                at: row.swatch_at,
                size: Size {
                    width: LEGEND_SWATCH,
                    height: LEGEND_SWATCH,
                },
                rx: 3.0,
                ry: 3.0,
            }),
        )
        // The same rule as the curve it stands for, which is what makes a
        // legend a legend rather than a decoration that happens to match.
        .classed("radar-area")
        .classed(format!("radar-color-{}", row.color_index))
        // With its own row's text rather than with the curves: a swatch and the
        // name beside it are one row, and separating them by layer would draw
        // every swatch before every name.
        .on(Layer::Label),
        text(
            row.label_at,
            &row.label,
            LEGEND_FONT,
            LEGEND_WEIGHT,
            Anchor::Start,
            "radar-legend-label",
        ),
    ]
}

/// The rules a radar chart needs on top of the shared tokens.
///
/// One rule per curve setting both fill and stroke, because the area and its
/// outline are the same colour at different opacities.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let colors: String = placed
        .series
        .iter()
        .map(|s| {
            let color = series_css(s.color_index, mode, theme);
            format!(
                ".radar-color-{}{{fill:{color};stroke:{color}}}",
                s.color_index
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .radar-ring{{fill:none;stroke:var(--_inner-stroke);stroke-width:1}}\
         .radar-spoke{{stroke:var(--_line);stroke-width:1;opacity:0.6}}\
         .radar-axis-label{{fill:var(--_text-sec)}}\
         .radar-area{{fill-opacity:0.18;stroke-width:2;stroke-linejoin:round}}\
         .radar-point{{fill-opacity:1;stroke:var(--ags-bg);stroke-width:1}}\
         .radar-legend-label{{fill:var(--_text)}}\
         .radar-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed radar chart.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for ring in &placed.rings {
        out.push(
            Node::new(
                Role::Frame,
                Content::Shape(Shape::Circle {
                    c: placed.centre,
                    r: *ring,
                }),
            )
            .classed("radar-ring"),
        );
    }
    // Every spoke, then every name: a name drawn between two spokes would be
    // painted over by the second one where the two cross.
    for axis in &placed.axes {
        out.push(spoke_node(placed.centre, axis));
    }
    for axis in &placed.axes {
        out.push(axis_label_node(axis));
    }
    for series in &placed.series {
        out.push(series_node(series));
    }
    for row in &placed.legend {
        for node in legend_nodes(row) {
            out.push(node);
        }
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "radar-title",
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

    const CHART: &str = "radar-beta\n\
        title Skills\n\
        axis code, design, ops\n\
        curve now[\"Today\"]{4, 2, 3}\n\
        curve goal[\"Target\"]{5, 5, 5}";

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
    fn every_curve_and_every_vertex_on_it_is_addressable() {
        let nodes = all(&drawn(CHART));
        let curves = with_class(&nodes, "node");
        assert_eq!(curves.len(), 2);
        assert_eq!(curves[0].id.as_deref(), Some("now"));
        let points = with_class(&nodes, "radar-point");
        assert_eq!(points.len(), 6, "three axes, two curves");
        assert_eq!(points[0].id.as_deref(), Some("now::code"));
        assert_eq!(points[0].value.as_deref(), Some("4"));
        assert!(points[0].data.contains(&("axis".into(), "code".into())));
    }

    #[test]
    fn a_curve_closes_so_it_reads_as_an_area() {
        let nodes = all(&drawn(CHART));
        let Content::Shape(Shape::Path(segs)) = &with_class(&nodes, "radar-area")[0].content else {
            panic!("a closed path")
        };
        assert!(matches!(segs.last(), Some(Seg::Close)));
        assert_eq!(segs.len(), 4, "three vertices and the close");
    }

    #[test]
    fn a_legend_swatch_shares_the_rule_of_the_curve_it_stands_for() {
        let nodes = all(&drawn(CHART));
        // Two curves plus two swatches, all carrying `radar-area`.
        assert_eq!(with_class(&nodes, "radar-area").len(), 4);
        assert_eq!(
            with_class(&nodes, "radar-color-1").len(),
            5,
            "curve, 3 dots, swatch"
        );
    }

    #[test]
    fn the_grid_paints_behind_everything_plotted_on_it() {
        let scene = drawn(CHART);
        let layers: Vec<(Layer, &str)> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(|c| (n.layer, c.as_str())))
            .collect();
        let first_curve = layers
            .iter()
            .position(|(_, c)| *c == "node")
            .expect("a curve");
        assert!(layers
            .iter()
            .take(first_curve)
            .all(|(l, _)| *l == Layer::Frame));
    }

    #[test]
    fn the_rings_and_spokes_are_drawn() {
        let nodes = all(&drawn(CHART));
        assert_eq!(with_class(&nodes, "radar-ring").len(), 4);
        assert_eq!(with_class(&nodes, "radar-spoke").len(), 3);
        assert_eq!(with_class(&nodes, "radar-axis-label").len(), 3);
    }

    #[test]
    fn one_rule_per_curve_sets_both_fill_and_stroke() {
        let style = drawn(CHART).style;
        assert!(
            style.contains(".radar-color-0{fill:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains("stroke:var(--ags-accent"), "{style}");
        assert!(!style.contains(".radar-color-2{"), "{style}");
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(CHART)), "radar-title").len(), 1);
        assert!(with_class(&all(&drawn("radar\naxis a")), "radar-title").is_empty());
    }

    #[test]
    fn a_chart_of_nothing_still_yields_a_disc() {
        let scene = drawn("radar");
        assert!(scene.canvas.width > 0.0);
        assert_eq!(with_class(&all(&scene), "radar-ring").len(), 4);
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(CHART, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
