//! A placed xy chart, drawn into the scene.
//!
//! The visual language is deliberately spare: no axis lines and no tick marks,
//! a field of faint dots instead of ruled grid lines, bars with every corner
//! rounded, and curves smoothed through their points rather than joined
//! straight. A line also casts a soft shadow two pixels below itself, which is
//! the same path drawn wider and translated rather than a second geometry.
//!
//! The reference has an interactive mode that adds hover tooltips. Nothing in
//! this project ever turns it on — not the viewer, not the CLI — so it is not
//! ported; a sparse line still gets its dots, which is the part of that code
//! path that runs with interaction off.

use crate::api::ColorMode;
use crate::color::mix_hex;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Seg, Shape, Size, TextRun, Transform,
};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{
    layout, Align, AxisTitle, Bar, Curve, LegendItem, Placed, PlacedAxis, Tick, AXIS_TITLE_FONT,
    AXIS_TITLE_WEIGHT, LABEL_FONT, LABEL_WEIGHT, LEGEND_FONT, LEGEND_GAP, LEGEND_SWATCH_H,
    LEGEND_SWATCH_W, LEGEND_WEIGHT, TITLE_FONT, TITLE_WEIGHT,
};
use super::types::SeriesKind;

const BASELINE: &str = "0.35em";
/// The radius of one dot in the background field.
const GRID_DOT: f64 = 1.5;
/// The dots aim for about this far apart, then divide a tick's span evenly.
const GRID_TARGET: f64 = 20.0;
/// How far a curve's shadow falls below it.
const SHADOW_DROP: f64 = 2.0;
const DOT_RADIUS: f64 = 5.0;
const LINE_WIDTH: f64 = 2.5;
const BAR_RADIUS: f64 = 8.0;
/// Above this many points a curve is drawn without dots — they would crowd.
const SPARSE_LIMIT: usize = 12;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn text(at: Point, content: &str, font: f64, weight: u32, anchor: Anchor, class: &str) -> Node {
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
    .classed(class)
}

const fn anchor_of(align: Align) -> Anchor {
    match align {
        Align::Middle => Anchor::Middle,
        Align::End => Anchor::End,
    }
}

/// A rectangle with every corner rounded.
///
/// `from_left` only moves where the outline starts — the shape it traces is the
/// same either way, and the reference spells the two cases differently for no
/// reason beyond which corner it happened to begin at.
fn rounded_bar(bar: &Bar, from_left: bool) -> Vec<Seg> {
    let (left, top) = (bar.at.x, bar.at.y);
    let (right, bottom) = (left + bar.width, top + bar.height);
    let radius = BAR_RADIUS.min(bar.width / 2.0).min(bar.height / 2.0);
    if radius <= 0.0 {
        // Too small to round: a plain rectangle, which is what a bar sitting
        // flat on the baseline comes out as.
        return vec![
            Seg::MoveTo(point(left, top)),
            Seg::LineTo(point(right, top)),
            Seg::LineTo(point(right, bottom)),
            Seg::LineTo(point(left, bottom)),
            Seg::Close,
        ];
    }
    let top_left = [
        Seg::LineTo(point(left, top + radius)),
        Seg::Quad {
            ctrl: point(left, top),
            to: point(left + radius, top),
        },
    ];
    let mut out = if from_left {
        vec![Seg::MoveTo(point(left + radius, top))]
    } else {
        vec![
            Seg::MoveTo(point(left, top + radius)),
            Seg::Quad {
                ctrl: point(left, top),
                to: point(left + radius, top),
            },
        ]
    };
    out.extend([
        Seg::LineTo(point(right - radius, top)),
        Seg::Quad {
            ctrl: point(right, top),
            to: point(right, top + radius),
        },
        Seg::LineTo(point(right, bottom - radius)),
        Seg::Quad {
            ctrl: point(right, bottom),
            to: point(right - radius, bottom),
        },
        Seg::LineTo(point(left + radius, bottom)),
        Seg::Quad {
            ctrl: point(left, bottom),
            to: point(left, bottom - radius),
        },
    ]);
    if from_left {
        out.extend(top_left);
    }
    out.push(Seg::Close);
    out
}

/// The slope at each knot of a natural cubic spline through `points`.
///
/// The curve treats y as a function of x and minimises total curvature, so it
/// can never double back — which a Catmull-Rom or a cardinal spline can, and
/// does, on a series that climbs steeply and then flattens.
///
/// The second derivatives come from the usual tridiagonal system, solved by the
/// Thomas algorithm with natural boundaries (zero curvature at both ends).
fn spline_slopes(points: &[Point]) -> Vec<f64> {
    let n = points.len();
    let mut widths = Vec::with_capacity(n.saturating_sub(1));
    let mut secants = Vec::with_capacity(n.saturating_sub(1));
    for pair in points.windows(2) {
        let (a, b) = (pair.first().copied(), pair.get(1).copied());
        let (Some(a), Some(b)) = (a, b) else { continue };
        let width = b.x - a.x;
        widths.push(width);
        secants.push(if width == 0.0 {
            0.0
        } else {
            (b.y - a.y) / width
        });
    }
    let at = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(0.0);

    let mut curvature = vec![0.0; n];
    if n > 2 {
        let mut upper = vec![0.0; n];
        let mut rhs = vec![0.0; n];
        for i in 1..n - 1 {
            let diagonal = 2.0 * (at(&widths, i - 1) + at(&widths, i));
            let target = 3.0 * (at(&secants, i) - at(&secants, i - 1));
            let (u, d) = if i == 1 {
                (at(&widths, i) / diagonal, target / diagonal)
            } else {
                let pivot = diagonal - at(&widths, i - 1) * at(&upper, i - 1);
                (
                    at(&widths, i) / pivot,
                    (target - at(&widths, i - 1) * at(&rhs, i - 1)) / pivot,
                )
            };
            if let (Some(slot), Some(row)) = (upper.get_mut(i), rhs.get_mut(i)) {
                *slot = u;
                *row = d;
            }
        }
        for i in (1..n - 1).rev() {
            let value = at(&rhs, i) - at(&upper, i) * at(&curvature, i + 1);
            if let Some(slot) = curvature.get_mut(i) {
                *slot = value;
            }
        }
    }

    let mut slopes = vec![0.0; n];
    for i in 0..n.saturating_sub(1) {
        let value = at(&secants, i)
            - at(&widths, i) * (2.0 * at(&curvature, i) + at(&curvature, i + 1)) / 3.0;
        if let Some(slot) = slopes.get_mut(i) {
            *slot = value;
        }
    }
    if n >= 2 {
        let value = at(&secants, n - 2) + at(&widths, n - 2) * at(&curvature, n - 2) / 3.0;
        if let Some(slot) = slopes.get_mut(n - 1) {
            *slot = value;
        }
    }
    slopes
}

/// A smooth curve through every point.
fn smooth_path(points: &[Point]) -> Vec<Seg> {
    let Some(first) = points.first().copied() else {
        return Vec::new();
    };
    let mut out = vec![Seg::MoveTo(first)];
    match points.len() {
        1 => return out,
        2 => {
            if let Some(last) = points.get(1) {
                out.push(Seg::LineTo(*last));
            }
            return out;
        }
        _ => {}
    }
    let slopes = spline_slopes(points);
    for (i, pair) in points.windows(2).enumerate() {
        let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        // A third of the interval, so the control points stay strictly between
        // the two knots in x and the curve keeps its single-valued shape.
        let reach = (b.x - a.x) / 3.0;
        out.push(Seg::Cubic {
            c1: point(
                a.x + reach,
                a.y + slopes.get(i).copied().unwrap_or(0.0) * reach,
            ),
            c2: point(
                b.x - reach,
                b.y - slopes.get(i + 1).copied().unwrap_or(0.0) * reach,
            ),
            to: *b,
        });
    }
    out
}

/// The field of dots behind the plot, aligned to the ticks.
fn grid_dots(placed: &Placed) -> Vec<Node> {
    let plot = placed.plot;
    let across: Vec<f64> = placed.x_axis.ticks.iter().map(|t| t.at.x).collect();
    let down: Vec<f64> = if placed.horizontal {
        placed.y_axis.ticks.iter().map(|t| t.at.y).collect()
    } else {
        placed.grid.iter().map(|(a, _)| a.y).collect()
    };
    let gap = |ticks: &[f64], fallback: f64| {
        let base = match (ticks.first(), ticks.get(1)) {
            (Some(a), Some(b)) => (b - a).abs(),
            _ => fallback,
        };
        // A tick's span divided into whole steps of about the target size, so
        // the dots stay in register with the labels however wide the span is.
        base / (base / GRID_TARGET).round().max(1.0)
    };
    let (step_x, step_y) = (
        gap(&across, plot.width / 6.0),
        gap(&down, plot.height / 6.0),
    );
    let start = |anchor: f64, edge: f64, step: f64| anchor - ((anchor - edge) / step).ceil() * step;
    let first_x = start(
        across.first().copied().unwrap_or(plot.at.x),
        plot.at.x,
        step_x,
    );
    let first_y = start(
        down.first().copied().unwrap_or(plot.at.y),
        plot.at.y,
        step_y,
    );

    let mut out = Vec::new();
    let mut y = first_y;
    while y <= plot.at.y + plot.height + 0.5 {
        let mut x = first_x;
        while x <= plot.at.x + plot.width + 0.5 {
            out.push(
                Node::new(
                    Role::Decoration,
                    Content::Shape(Shape::Circle {
                        c: point(x, y),
                        r: GRID_DOT,
                    }),
                )
                .classed("xychart-grid")
                .on(Layer::Frame),
            );
            x += step_x;
        }
        y += step_y;
    }
    out
}

/// A datum as the reader sees it: a whole number keeps its digits, and anything
/// else keeps whatever precision it was written with.
fn value_text(value: f64) -> String {
    format!("{value}")
}

fn colour_class(index: usize) -> String {
    format!("xychart-color-{index}")
}

fn bar_node(bar: &Bar, horizontal: bool) -> Node {
    Node::new(
        Role::Node,
        Content::Shape(Shape::Path(rounded_bar(bar, horizontal))),
    )
    .classed("xychart-bar")
    .classed(colour_class(bar.color_index))
    .valued(value_text(bar.value))
    .tagged("label", bar.label.clone())
    .on(Layer::Frame)
}

/// A curve: its shadow, then the curve itself over the top.
fn curve_nodes(curve: &Curve) -> Vec<Node> {
    let points: Vec<Point> = curve.points.iter().map(|p| p.at).collect();
    let path = smooth_path(&points);
    if path.is_empty() {
        return Vec::new();
    }
    let mut shadow = Node::new(Role::Edge, Content::Shape(Shape::Path(path.clone())))
        .classed("xychart-line-shadow")
        .classed(colour_class(curve.color_index));
    shadow.transform = Some(Transform::Translate {
        by: point(0.0, SHADOW_DROP),
    });
    vec![
        shadow,
        Node::new(Role::Edge, Content::Shape(Shape::Path(path)))
            .classed("xychart-line")
            .classed(colour_class(curve.color_index)),
    ]
}

/// The dots on a sparse curve, in column order rather than in series order —
/// every point that shares an x is drawn before the next column starts.
fn dot_nodes(placed: &Placed) -> Vec<Node> {
    let longest = placed
        .curves
        .iter()
        .map(|curve| curve.points.len())
        .max()
        .unwrap_or(0);
    if longest == 0 || longest > SPARSE_LIMIT {
        return Vec::new();
    }
    let mut columns: Vec<(String, Vec<Node>)> = Vec::new();
    for curve in &placed.curves {
        for vertex in &curve.points {
            let key = crate::round::coord(vertex.at.x);
            let dot = Node::new(
                Role::Node,
                Content::Shape(Shape::Circle {
                    c: vertex.at,
                    r: DOT_RADIUS,
                }),
            )
            .classed("xychart-dot")
            .classed(colour_class(curve.color_index))
            .valued(value_text(vertex.value))
            .tagged("label", vertex.label.clone());
            match columns.iter_mut().find(|(at, _)| *at == key) {
                Some((_, dots)) => dots.push(dot),
                None => columns.push((key, vec![dot])),
            }
        }
    }
    columns.into_iter().flat_map(|(_, dots)| dots).collect()
}

fn tick_nodes(axis: &PlacedAxis) -> Vec<Node> {
    axis.ticks
        .iter()
        .map(|Tick { label, at, align }| {
            text(
                *at,
                label,
                LABEL_FONT,
                LABEL_WEIGHT,
                anchor_of(*align),
                "xychart-label",
            )
        })
        .collect()
}

fn axis_title_node(title: &AxisTitle) -> Node {
    let mut node = text(
        title.at,
        &title.text,
        AXIS_TITLE_FONT,
        AXIS_TITLE_WEIGHT,
        Anchor::Middle,
        "xychart-axis-title",
    );
    if title.turned {
        node.transform = Some(Transform::Rotate {
            deg: -90.0,
            about: title.at,
        });
    }
    node
}

/// One legend entry: a swatch shaped like the series it stands for, and its name.
fn legend_nodes(item: &LegendItem) -> Vec<Node> {
    let swatch = match item.kind {
        SeriesKind::Bar => Node::new(
            Role::Node,
            Content::Shape(Shape::Rect {
                at: point(item.at.x, item.at.y - LEGEND_SWATCH_H / 2.0),
                size: size(LEGEND_SWATCH_W, LEGEND_SWATCH_H),
                rx: 3.0,
                ry: 0.0,
            }),
        )
        .classed("xychart-bar"),
        SeriesKind::Line => Node::new(
            Role::Node,
            Content::Shape(Shape::Line {
                a: item.at,
                b: point(item.at.x + LEGEND_SWATCH_W, item.at.y),
            }),
        )
        .classed("xychart-legend-line"),
    };
    vec![
        swatch.classed(colour_class(item.color_index)),
        text(
            point(item.at.x + LEGEND_SWATCH_W + LEGEND_GAP, item.at.y),
            &item.label,
            LEGEND_FONT,
            LEGEND_WEIGHT,
            Anchor::Start,
            "xychart-label",
        ),
    ]
}

/// A series' fill: its own colour, mostly washed out into the background.
fn bar_fill(index: usize, theme: &Theme, mode: &ColorMode) -> String {
    let colour = series_css(index, mode, theme);
    match mode {
        // Left as a mix so a page changing its accent restyles the chart without
        // it being re-rendered.
        ColorMode::Tokens => format!("color-mix(in srgb, var(--ags-bg) 75%, {colour} 25%)"),
        ColorMode::Fixed => mix_hex(&theme.bg, &colour, 0.25),
    }
}

/// The rules an xy chart needs on top of the shared tokens.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let mut indices: Vec<usize> = placed
        .bars
        .iter()
        .map(|bar| bar.color_index)
        .chain(placed.curves.iter().map(|curve| curve.color_index))
        .collect();
    indices.sort_unstable();
    indices.dedup();
    let series: String = indices
        .iter()
        .map(|index| {
            let colour = series_css(*index, mode, theme);
            let fill = bar_fill(*index, theme, mode);
            format!(
                ".xychart-bar.xychart-color-{index}{{stroke:{colour};fill:{fill}}}\
                 path.xychart-color-{index},line.xychart-color-{index}{{stroke:{colour}}}\
                 circle.xychart-color-{index}{{fill:{colour}}}"
            )
        })
        .collect::<Vec<String>>()
        .concat();
    format!(
        "{}\
         .xychart-grid{{fill:var(--_inner-stroke);stroke:none;opacity:0.65}}\
         .xychart-bar{{stroke-width:1.5}}\
         .xychart-line{{fill:none;stroke-width:{LINE_WIDTH};stroke-linecap:round;stroke-linejoin:round}}\
         .xychart-line-shadow{{fill:none;stroke-width:5;stroke-linecap:round;stroke-linejoin:round;opacity:0.12}}\
         .xychart-legend-line{{stroke-width:{LINE_WIDTH};stroke-linecap:round}}\
         .xychart-dot{{stroke:var(--ags-bg);stroke-width:2}}\
         .xychart-label{{fill:var(--_text-muted)}}\
         .xychart-axis-title{{fill:var(--_text-sec)}}\
         .xychart-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{series}",
        style_block(theme, mode)
    )
}

/// Draw a placed xy chart.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(size(placed.width, placed.height));
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for dot in grid_dots(placed) {
        out.push(dot);
    }
    for bar in &placed.bars {
        out.push(bar_node(bar, placed.horizontal));
    }
    for curve in &placed.curves {
        for node in curve_nodes(curve) {
            out.push(node);
        }
    }
    for dot in dot_nodes(placed) {
        out.push(dot);
    }
    for axis in [&placed.x_axis, &placed.y_axis] {
        for node in tick_nodes(axis) {
            out.push(node);
        }
    }
    for axis in [&placed.x_axis, &placed.y_axis] {
        if let Some(title) = &axis.title {
            out.push(axis_title_node(title));
        }
    }
    if let Some((text_content, at)) = &placed.title {
        out.push(text(
            *at,
            text_content,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "xychart-title",
        ));
    }
    for item in &placed.legend {
        for node in legend_nodes(item) {
            out.push(node.on(Layer::Label));
        }
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

    const BARS: &str = "xychart-beta\ntitle \"Sales\"\nx-axis [A, B, C]\nbar [10, 20, 30]";

    fn drawn(source: &str) -> Scene {
        render(source, &Theme::default(), &ColorMode::Tokens)
    }

    fn with_class<'a>(scene: &'a Scene, class: &str) -> Vec<&'a Node> {
        scene
            .painted()
            .into_iter()
            .filter(|node| node.class.iter().any(|c| c == class))
            .collect()
    }

    fn order(scene: &Scene) -> Vec<String> {
        scene
            .painted()
            .iter()
            .filter_map(|node| node.class.first().cloned())
            .collect()
    }

    #[test]
    fn the_background_is_a_field_of_dots_inside_the_plot() {
        let scene = drawn(BARS);
        let dots = with_class(&scene, "xychart-grid");
        assert!(dots.len() > 50, "{} dots", dots.len());
        let placed = layout(&crate::xychart::parse(BARS));
        // The field is anchored to the ticks and stepped outwards from there, so
        // its first row and column may start up to one step outside the plot —
        // which is what the reference does, and what keeps the dots in register
        // with the labels.
        let slack = GRID_TARGET * 2.0;
        for dot in dots {
            let Content::Shape(Shape::Circle { c, r }) = &dot.content else {
                panic!("a dot")
            };
            assert!((r - GRID_DOT).abs() < 1e-9);
            assert!(c.x >= placed.plot.at.x - slack, "{}", c.x);
            assert!(c.x <= placed.plot.at.x + placed.plot.width + 0.5);
            assert!(c.y >= placed.plot.at.y - slack, "{}", c.y);
            assert!(c.y <= placed.plot.at.y + placed.plot.height + 0.5);
        }
    }

    #[test]
    fn a_bar_is_a_rounded_path_carrying_its_own_value() {
        let scene = drawn(BARS);
        let bars = with_class(&scene, "xychart-bar");
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].value.as_deref(), Some("10"));
        assert!(bars[0].class.iter().any(|c| c == "xychart-color-0"));
        let Content::Shape(Shape::Path(segs)) = &bars[0].content else {
            panic!("a path")
        };
        assert!(
            segs.iter().any(|s| matches!(s, Seg::Quad { .. })),
            "rounded"
        );
        assert!(matches!(segs.last(), Some(Seg::Close)));
    }

    #[test]
    fn a_bar_too_small_to_round_is_drawn_square() {
        // A value sitting on the baseline has no height to round.
        let scene = drawn("xychart-beta\nx-axis [A, B]\ny-axis 0 --> 10\nbar [0, 10]");
        let bars = with_class(&scene, "xychart-bar");
        let Content::Shape(Shape::Path(segs)) = &bars[0].content else {
            panic!("a path")
        };
        assert!(!segs.iter().any(|s| matches!(s, Seg::Quad { .. })));
        assert_eq!(segs.len(), 5, "four corners and a close");
    }

    #[test]
    fn a_horizontal_bar_opens_its_outline_at_a_different_corner() {
        let upright = rounded_bar(
            &Bar {
                at: point(0.0, 0.0),
                width: 40.0,
                height: 40.0,
                value: 1.0,
                label: String::new(),
                color_index: 0,
            },
            false,
        );
        let sideways = rounded_bar(
            &Bar {
                at: point(0.0, 0.0),
                width: 40.0,
                height: 40.0,
                value: 1.0,
                label: String::new(),
                color_index: 0,
            },
            true,
        );
        // Both trace the same outline; they only begin at different corners, so
        // the sideways one needs one extra segment to close the loop.
        assert!(matches!(upright.first(), Some(Seg::MoveTo(p)) if (p.x - 0.0).abs() < 1e-9));
        assert!(matches!(sideways.first(), Some(Seg::MoveTo(p)) if (p.x - 8.0).abs() < 1e-9));
        let corners = |segs: &[Seg]| {
            segs.iter()
                .filter(|s| matches!(s, Seg::Quad { .. }))
                .count()
        };
        assert_eq!(corners(&upright), 4);
        assert_eq!(corners(&sideways), 4);
    }

    #[test]
    fn a_curve_is_drawn_twice_the_second_time_over_its_own_shadow() {
        let scene = drawn("xychart-beta\nx-axis [A, B, C]\nline [1, 5, 3]");
        let shadow = with_class(&scene, "xychart-line-shadow");
        let line = with_class(&scene, "xychart-line");
        assert_eq!(shadow.len(), 1);
        assert_eq!(line.len(), 1);
        assert_eq!(shadow[0].content, line[0].content, "the same path");
        assert_eq!(
            shadow[0].transform,
            Some(Transform::Translate {
                by: point(0.0, SHADOW_DROP)
            })
        );
        assert_eq!(line[0].transform, None);
    }

    #[test]
    fn a_curve_of_three_or_more_points_is_smoothed_into_cubics() {
        let scene = drawn("xychart-beta\nx-axis [A, B, C]\nline [1, 5, 3]");
        let Content::Shape(Shape::Path(segs)) = &with_class(&scene, "xychart-line")[0].content
        else {
            panic!("a path")
        };
        assert!(matches!(segs.first(), Some(Seg::MoveTo(_))));
        assert_eq!(
            segs.iter()
                .filter(|s| matches!(s, Seg::Cubic { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn a_curve_of_two_points_is_a_straight_line_and_of_one_is_a_dot() {
        assert_eq!(smooth_path(&[]).len(), 0);
        assert_eq!(smooth_path(&[point(0.0, 0.0)]).len(), 1);
        let pair = smooth_path(&[point(0.0, 0.0), point(1.0, 1.0)]);
        assert_eq!(pair.len(), 2);
        assert!(matches!(pair.get(1), Some(Seg::LineTo(_))));
    }

    #[test]
    fn a_smoothed_curve_passes_through_every_point_it_was_given() {
        let points = [
            point(0.0, 10.0),
            point(10.0, 40.0),
            point(20.0, 20.0),
            point(30.0, 50.0),
        ];
        let segs = smooth_path(&points);
        let ends: Vec<Point> = segs
            .iter()
            .filter_map(|seg| match seg {
                Seg::MoveTo(p) => Some(*p),
                Seg::Cubic { to, .. } => Some(*to),
                _ => None,
            })
            .collect();
        assert_eq!(ends, points);
    }

    #[test]
    fn a_curve_whose_points_share_an_x_has_no_secant_to_take_there() {
        // Real data never reaches this — x comes from the category index — but
        // the guard keeps a zero interval from becoming an infinite slope.
        let slopes = spline_slopes(&[point(0.0, 1.0), point(0.0, 2.0), point(0.0, 3.0)]);
        assert_eq!(slopes.len(), 3);
    }

    #[test]
    fn a_sparse_curve_shows_its_points_and_a_crowded_one_does_not() {
        let sparse = drawn("xychart-beta\nx-axis [A, B, C]\nline [1, 2, 3]");
        assert_eq!(with_class(&sparse, "xychart-dot").len(), 3);
        let values: Vec<String> = (1..=13).map(|n| n.to_string()).collect();
        let crowded = drawn(&format!("xychart-beta\nline [{}]", values.join(", ")));
        assert!(
            with_class(&crowded, "xychart-dot").is_empty(),
            "13 is too many"
        );
    }

    #[test]
    fn dots_are_drawn_a_column_at_a_time_rather_than_a_curve_at_a_time() {
        let scene = drawn("xychart-beta\nx-axis [A, B]\nline [1, 2]\nline [3, 4]");
        let dots = with_class(&scene, "xychart-dot");
        assert_eq!(dots.len(), 4);
        let colour = |node: &Node| {
            node.class
                .iter()
                .find(|c| c.starts_with("xychart-color-"))
                .cloned()
                .unwrap_or_default()
        };
        // Column A holds both series before column B starts.
        assert_eq!(colour(dots[0]), "xychart-color-0");
        assert_eq!(colour(dots[1]), "xychart-color-1");
        assert_eq!(colour(dots[2]), "xychart-color-0");
    }

    #[test]
    fn every_label_and_title_is_drawn() {
        let scene = drawn(
            "xychart-beta\ntitle \"T\"\nx-axis \"C\" [A, B]\ny-axis \"V\" 0 --> 10\nbar [1, 2]",
        );
        assert_eq!(with_class(&scene, "xychart-title").len(), 1);
        assert_eq!(with_class(&scene, "xychart-axis-title").len(), 2);
        assert!(with_class(&scene, "xychart-label").len() >= 2);
    }

    #[test]
    fn a_side_axis_title_is_turned_and_a_bottom_one_is_not() {
        let scene = drawn("xychart-beta\nx-axis \"C\" [A]\ny-axis \"V\" 0 --> 10\nbar [1]");
        let titles = with_class(&scene, "xychart-axis-title");
        let turned = titles
            .iter()
            .filter(|node| matches!(node.transform, Some(Transform::Rotate { .. })))
            .count();
        assert_eq!(turned, 1);
    }

    #[test]
    fn a_legend_swatch_takes_the_shape_of_the_series_it_stands_for() {
        let scene = drawn("xychart-beta\nx-axis [A]\nbar [1]\nline [2]");
        let swatches: Vec<&Node> = scene
            .painted()
            .into_iter()
            .filter(|node| {
                node.class.iter().any(|c| c == "xychart-legend-line")
                    || (node.class.iter().any(|c| c == "xychart-bar")
                        && matches!(node.content, Content::Shape(Shape::Rect { .. })))
            })
            .collect();
        assert_eq!(swatches.len(), 2);
        assert!(matches!(
            swatches[0].content,
            Content::Shape(Shape::Rect { .. })
        ));
        assert!(matches!(
            swatches[1].content,
            Content::Shape(Shape::Line { .. })
        ));
    }

    #[test]
    fn the_drawing_is_stacked_grid_then_data_then_words() {
        let classes = order(&drawn(
            "xychart-beta\ntitle \"T\"\nx-axis [A, B, C]\nbar [1, 2, 3]\nline [3, 2, 1]",
        ));
        let first = |name: &str| classes.iter().position(|c| c == name).unwrap_or(usize::MAX);
        assert!(first("xychart-grid") < first("xychart-bar"));
        assert!(first("xychart-bar") < first("xychart-line-shadow"));
        assert!(first("xychart-line-shadow") < first("xychart-line"));
        assert!(first("xychart-line") < first("xychart-dot"));
        assert!(first("xychart-dot") < first("xychart-label"));
        assert!(first("xychart-label") < first("xychart-title"));
    }

    #[test]
    fn a_series_gets_a_rule_of_its_own_and_a_washed_out_fill() {
        let scene = drawn("xychart-beta\nx-axis [A]\nbar [1]\nline [2]");
        assert!(scene.style.contains(".xychart-bar.xychart-color-0"));
        assert!(scene.style.contains("circle.xychart-color-1"));
        assert!(scene.style.contains("color-mix(in srgb, var(--ags-bg) 75%"));
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(BARS, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }

    #[test]
    fn a_chart_of_nothing_still_draws_its_grid() {
        let scene = drawn("xychart-beta");
        assert!(with_class(&scene, "xychart-bar").is_empty());
        assert!(!with_class(&scene, "xychart-grid").is_empty());
    }
}
