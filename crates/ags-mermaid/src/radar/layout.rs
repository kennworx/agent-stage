//! Where the disc goes, and where each value lands on it.
//!
//! Axis `i` sits at `-90° + 360°·i/n` — starting at the top and sweeping
//! clockwise — and a value maps to a radius in proportion to the scale.

use crate::round::count;
use crate::scene::{Anchor, Point};

use super::types::Chart;

pub const RADIUS: f64 = 160.0;
pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
/// Room around the disc for the axis names.
pub const LABEL_MARGIN: f64 = 64.0;
/// How far past the disc an axis name sits.
pub const LABEL_OFFSET: f64 = 18.0;
pub const RINGS: usize = 4;
pub const LEGEND_GAP: f64 = 36.0;
pub const LEGEND_ROW_H: f64 = 24.0;
pub const LEGEND_SWATCH: f64 = 14.0;
pub const LEGEND_SWATCH_GAP: f64 = 8.0;
pub const LEGEND_FONT: f64 = 14.0;
pub const LEGEND_WEIGHT: u32 = 400;
pub const POINT_RADIUS: f64 = 3.0;

/// One spoke, and where its name sits.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedAxis {
    pub id: String,
    pub label: String,
    pub at: Point,
    pub label_at: Point,
    pub anchor: Anchor,
}

/// One plotted vertex.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPoint {
    /// The series and the axis together, so every vertex is addressable.
    pub id: String,
    pub axis_id: String,
    pub value: f64,
    pub at: Point,
}

/// One curve.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSeries {
    pub id: String,
    pub label: String,
    pub color_index: usize,
    pub points: Vec<PlacedPoint>,
}

/// One legend row.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendRow {
    pub label: String,
    pub label_at: Point,
    pub swatch_at: Point,
    pub color_index: usize,
}

/// A laid-out radar chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub centre: Point,
    pub radius: f64,
    /// Graticule radii, inner to outer.
    pub rings: Vec<f64>,
    pub axes: Vec<PlacedAxis>,
    pub series: Vec<PlacedSeries>,
    pub legend: Vec<LegendRow>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// The angle of axis `i` of `n`: from the top, clockwise.
fn angle_at(i: usize, n: usize) -> f64 {
    -std::f64::consts::FRAC_PI_2 + 2.0 * std::f64::consts::PI * count(i) / count(n)
}

/// Which way an axis name reads, from where on the circle it sits.
///
/// A name near the top or bottom is centred; otherwise it reads away from the
/// disc, so it never runs back across the drawing.
fn anchor_for(angle: f64) -> Anchor {
    let cos = angle.cos();
    if cos.abs() < 0.25 {
        Anchor::Middle
    } else if cos > 0.0 {
        Anchor::Start
    } else {
        Anchor::End
    }
}

/// The upper bound of the radial scale.
fn scale_max(chart: &Chart) -> f64 {
    if let Some(max) = chart.max.filter(|m| *m > 0.0) {
        return max;
    }
    let largest = chart
        .series
        .iter()
        .flat_map(|s| s.values.iter())
        .filter(|v| v.is_finite())
        .fold(0.0_f64, |m, v| m.max(*v));
    // Never zero: an all-zero chart still needs a disc to draw nothing on.
    largest.max(1.0)
}

/// The spokes and their names.
fn place_axes(chart: &Chart, centre: Point, n: usize) -> Vec<PlacedAxis> {
    chart
        .axes
        .iter()
        .enumerate()
        .map(|(i, axis)| {
            let angle = angle_at(i, n);
            let along = |r: f64| Point::new(centre.x + r * angle.cos(), centre.y + r * angle.sin());
            PlacedAxis {
                id: axis.id.clone(),
                label: axis.label.clone(),
                at: along(RADIUS),
                label_at: along(RADIUS + LABEL_OFFSET),
                anchor: anchor_for(angle),
            }
        })
        .collect()
}

/// The curves, each with one vertex per axis.
fn place_series(chart: &Chart, centre: Point, n: usize, max: f64) -> Vec<PlacedSeries> {
    chart
        .series
        .iter()
        .enumerate()
        .map(|(si, s)| PlacedSeries {
            id: s.id.clone(),
            label: s.label.clone(),
            color_index: si,
            points: (0..n)
                .map(|i| {
                    let axis_id = chart
                        .axes
                        .get(i)
                        .map_or_else(|| format!("axis{i}"), |a| a.id.clone());
                    let value = s
                        .values
                        .get(i)
                        .copied()
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.0);
                    let angle = angle_at(i, n);
                    // A negative value is drawn at the centre rather than
                    // reflected through it onto the opposite spoke.
                    let r = value.max(0.0) / max * RADIUS;
                    PlacedPoint {
                        id: format!("{}::{axis_id}", s.id),
                        axis_id,
                        value,
                        at: Point::new(centre.x + r * angle.cos(), centre.y + r * angle.sin()),
                    }
                })
                .collect(),
        })
        .collect()
}

/// The legend column, and how far right its widest name reaches.
fn place_legend(chart: &Chart, centre: Point, top: f64) -> (Vec<LegendRow>, f64) {
    let swatch_x = centre.x + RADIUS + LABEL_MARGIN + LEGEND_GAP;
    let text_x = swatch_x + LEGEND_SWATCH + LEGEND_SWATCH_GAP;
    let rows: Vec<LegendRow> = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let y = top + count(i) * LEGEND_ROW_H + LEGEND_ROW_H / 2.0;
            LegendRow {
                label: s.label.clone(),
                label_at: Point::new(text_x, y),
                swatch_at: Point::new(swatch_x, y - LEGEND_SWATCH / 2.0),
                color_index: i,
            }
        })
        .collect();
    let widest = chart
        .series
        .iter()
        .map(|s| crate::metrics::text_width(&s.label, LEGEND_FONT, LEGEND_WEIGHT))
        .fold(0.0_f64, f64::max);
    // With no curves there is no legend column, so the canvas stops at the disc
    // rather than reserving room for nothing.
    let right = if rows.is_empty() {
        centre.x + RADIUS + LABEL_MARGIN
    } else {
        text_x + widest
    };
    (rows, right)
}

/// Lay out a parsed radar chart.
pub fn layout(chart: &Chart) -> Placed {
    let top = PADDING
        + if chart.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let centre = Point::new(PADDING + LABEL_MARGIN + RADIUS, top + LABEL_MARGIN + RADIUS);
    // One axis minimum, so the angle step never divides by zero.
    let n = chart.axes.len().max(1);

    let (legend, right) = place_legend(chart, centre, top);
    let width = right + PADDING;
    let disc_bottom = centre.y + RADIUS + LABEL_MARGIN;
    let legend_bottom = top + count(chart.series.len()) * LEGEND_ROW_H;

    Placed {
        width,
        height: disc_bottom.max(legend_bottom) + PADDING,
        title: chart
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        centre,
        radius: RADIUS,
        rings: (1..=RINGS)
            .map(|k| RADIUS * count(k) / count(RINGS))
            .collect(),
        axes: place_axes(chart, centre, n),
        series: place_series(chart, centre, n, scale_max(chart)),
        legend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::radar::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const CHART: &str = "radar-beta\n\
        title Skills\n\
        axis code, design, ops, docs\n\
        curve now{4, 2, 3, 1}\n\
        curve goal{5, 5, 5, 5}";

    #[test]
    fn the_first_axis_points_straight_up_and_the_rest_sweep_clockwise() {
        let out = placed(CHART);
        let first = &out.axes[0];
        assert!((first.at.x - out.centre.x).abs() < 1e-9);
        assert!(first.at.y < out.centre.y, "straight up");
        // Four axes, so the second is due east.
        assert!(out.axes[1].at.x > out.centre.x);
        assert!((out.axes[1].at.y - out.centre.y).abs() < 1e-9);
    }

    #[test]
    fn an_axis_name_reads_away_from_the_disc() {
        let out = placed(CHART);
        let anchors: Vec<Anchor> = out.axes.iter().map(|a| a.anchor).collect();
        assert_eq!(
            anchors,
            [Anchor::Middle, Anchor::Start, Anchor::Middle, Anchor::End]
        );
        // And sits beyond the ring it names.
        let offset = (out.axes[1].label_at.x - out.centre.x) - RADIUS;
        assert!((offset - LABEL_OFFSET).abs() < 1e-9);
    }

    #[test]
    fn a_value_maps_to_a_radius_in_proportion_to_the_scale() {
        let out = placed("radar\nmax 10\naxis a\ncurve x{5}");
        let point = &out.series[0].points[0];
        let r = (point.at.x - out.centre.x).hypot(point.at.y - out.centre.y);
        assert!((r - RADIUS / 2.0).abs() < 1e-6);
    }

    #[test]
    fn without_a_stated_maximum_the_largest_value_sets_the_scale() {
        let out = placed("radar\naxis a, b\ncurve x{2, 4}");
        let outer = &out.series[0].points[1];
        let r = (outer.at.x - out.centre.x).hypot(outer.at.y - out.centre.y);
        assert!((r - RADIUS).abs() < 1e-6, "the largest reaches the rim");
    }

    #[test]
    fn an_all_zero_chart_still_gets_a_disc() {
        // The scale would otherwise be zero and every radius a division by it.
        let out = placed("radar\naxis a, b\ncurve x{0, 0}");
        assert!(out.series[0].points.iter().all(|p| p.at.x.is_finite()));
        assert!((out.radius - RADIUS).abs() < 1e-9);
    }

    #[test]
    fn a_negative_value_is_drawn_at_the_centre_not_across_it() {
        let out = placed("radar\nmax 10\naxis a\ncurve x{-5}");
        let point = &out.series[0].points[0];
        assert!((point.at.x - out.centre.x).abs() < 1e-9);
        assert!((point.at.y - out.centre.y).abs() < 1e-9);
        // The value it carries is still the one that was written.
        assert!((point.value + 5.0).abs() < 1e-9);
    }

    #[test]
    fn every_vertex_is_addressable_by_its_series_and_axis() {
        let out = placed(CHART);
        assert_eq!(out.series[0].points[0].id, "now::code");
        assert_eq!(out.series[1].points[3].id, "goal::docs");
    }

    #[test]
    fn a_curve_gets_a_vertex_per_axis_even_where_it_named_no_value() {
        let out = placed("radar\naxis a, b, c\ncurve x{1}");
        assert_eq!(out.series[0].points.len(), 3);
        assert!(out.series[0].points[2].value.abs() < 1e-9);
    }

    #[test]
    fn there_are_four_rings_reaching_the_rim() {
        let out = placed(CHART);
        assert_eq!(out.rings.len(), RINGS);
        assert!((out.rings[RINGS - 1] - RADIUS).abs() < 1e-9);
        assert!(out.rings[0] < out.rings[1]);
    }

    #[test]
    fn a_legend_row_per_curve_stacks_beside_the_disc() {
        let out = placed(CHART);
        assert_eq!(out.legend.len(), 2);
        assert!((out.legend[1].label_at.y - out.legend[0].label_at.y - LEGEND_ROW_H).abs() < 1e-9);
        assert!(out.legend[0].swatch_at.x > out.centre.x + RADIUS);
    }

    #[test]
    fn a_chart_with_no_curves_reserves_no_legend_column() {
        let bare = placed("radar\naxis a, b");
        assert!(bare.legend.is_empty());
        assert!((bare.width - (bare.centre.x + RADIUS + LABEL_MARGIN + PADDING)).abs() < 1e-9);
    }

    #[test]
    fn the_canvas_covers_the_widest_legend_name() {
        let short = placed("radar\naxis a\ncurve x[\"S\"]{1}");
        let long = placed("radar\naxis a\ncurve x[\"A very long series name\"]{1}");
        assert!(long.width > short.width);
    }

    #[test]
    fn a_title_pushes_the_disc_down_and_centres_itself() {
        let out = placed(CHART);
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "Skills");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
        assert!((out.centre.y - (PADDING + TITLE_HEIGHT + LABEL_MARGIN + RADIUS)).abs() < 1e-9);
    }
}
