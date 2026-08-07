//! Where the square sits, and where each point lands inside it.
//!
//! The plot is a fixed square, so the unit data space maps onto it directly.
//! The vertical axis inverts on the way: data counts up from the bottom, screen
//! coordinates count down from the top.

use crate::scene::Point;

use super::types::Chart;

pub const PLOT_SIZE: f64 = 480.0;
pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
pub const X_LABEL_HEIGHT: f64 = 34.0;
pub const Y_LABEL_STRIP: f64 = 30.0;
pub const POINT_RADIUS: f64 = 6.0;
pub const POINT_LABEL_GAP: f64 = 16.0;

/// A rectangle, in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// One of the four regions, and whether it takes the checkerboard tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub rect: Rect,
    pub tinted: bool,
}

/// A label placed on an axis, and the turn it takes to sit alongside one.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisLabel {
    pub text: String,
    pub at: Point,
    /// Degrees; the vertical axis's labels read bottom-to-top.
    pub rotate: Option<f64>,
}

/// One plotted point: its dot, and its name below it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPoint {
    pub name: String,
    pub at: Point,
    pub label_at: Point,
}

/// A laid-out quadrant chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub plot: Option<Rect>,
    /// Vertical then horizontal, each through the centre.
    pub cross: Vec<(Point, Point)>,
    pub regions: Vec<Region>,
    /// Named regions only, in syntax order.
    pub region_labels: Vec<(String, Point)>,
    pub axis_labels: Vec<AxisLabel>,
    pub points: Vec<PlacedPoint>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
///
/// Not one font-size below the padding, which is where a baseline naturally
/// lands and which reads as the title having slipped toward the drawing.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// The four regions, tinted in a checkerboard so the split reads without lines.
fn regions(plot: Rect, centre: Point) -> Vec<Region> {
    let half = plot.width / 2.0;
    let quarter = |x: f64, y: f64, tinted: bool| Region {
        rect: Rect {
            at: Point::new(x, y),
            width: half,
            height: half,
        },
        tinted,
    };
    vec![
        quarter(centre.x, plot.at.y, true),
        quarter(plot.at.x, plot.at.y, false),
        quarter(plot.at.x, centre.y, true),
        quarter(centre.x, centre.y, false),
    ]
}

/// The names given to the regions, each centred in its own quarter.
fn region_labels(chart: &Chart, plot: Rect) -> Vec<(String, Point)> {
    let at = |fx: f64, fy: f64| Point::new(plot.at.x + PLOT_SIZE * fx, plot.at.y + PLOT_SIZE * fy);
    let quadrants = &chart.quadrants;
    [
        (&quadrants.q1, at(0.75, 0.25)),
        (&quadrants.q2, at(0.25, 0.25)),
        (&quadrants.q3, at(0.25, 0.75)),
        (&quadrants.q4, at(0.75, 0.75)),
    ]
    .into_iter()
    .filter_map(|(name, point)| name.clone().map(|text| (text, point)))
    .collect()
}

/// The axis end names, below the plot and to the left of it.
fn axis_labels(chart: &Chart, plot: Rect) -> Vec<AxisLabel> {
    let flat = |text: &String, x: f64, y: f64| AxisLabel {
        text: text.clone(),
        at: Point::new(x, y),
        rotate: None,
    };
    let turned = |text: &String, x: f64, y: f64| AxisLabel {
        text: text.clone(),
        at: Point::new(x, y),
        rotate: Some(-90.0),
    };
    // Each end sits over the middle of the half it names, not at the extreme.
    let x_y = plot.at.y + PLOT_SIZE + X_LABEL_HEIGHT / 2.0 + 2.0;
    let y_x = plot.at.x - Y_LABEL_STRIP / 2.0 - 2.0;
    let mut out = Vec::new();
    if let Some(text) = &chart.x_axis.low {
        out.push(flat(text, plot.at.x + PLOT_SIZE * 0.25, x_y));
    }
    if let Some(text) = &chart.x_axis.high {
        out.push(flat(text, plot.at.x + PLOT_SIZE * 0.75, x_y));
    }
    if let Some(text) = &chart.y_axis.low {
        out.push(turned(text, y_x, plot.at.y + PLOT_SIZE * 0.75));
    }
    if let Some(text) = &chart.y_axis.high {
        out.push(turned(text, y_x, plot.at.y + PLOT_SIZE * 0.25));
    }
    out
}

/// Lay out a parsed quadrant chart.
pub fn layout(chart: &Chart) -> Placed {
    let has_x = chart.x_axis.low.is_some() || chart.x_axis.high.is_some();
    let has_y = chart.y_axis.low.is_some() || chart.y_axis.high.is_some();

    let top = PADDING
        + if chart.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let left = PADDING + if has_y { Y_LABEL_STRIP } else { 0.0 };
    let bottom = PADDING + if has_x { X_LABEL_HEIGHT } else { 0.0 };

    let plot = Rect {
        at: Point::new(left, top),
        width: PLOT_SIZE,
        height: PLOT_SIZE,
    };
    let centre = Point::new(left + PLOT_SIZE / 2.0, top + PLOT_SIZE / 2.0);
    let width = left + PLOT_SIZE + PADDING;
    let height = top + PLOT_SIZE + bottom;

    Placed {
        width,
        height,
        title: chart
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        plot: Some(plot),
        cross: vec![
            (
                Point::new(centre.x, plot.at.y),
                Point::new(centre.x, plot.at.y + PLOT_SIZE),
            ),
            (
                Point::new(plot.at.x, centre.y),
                Point::new(plot.at.x + PLOT_SIZE, centre.y),
            ),
        ],
        regions: regions(plot, centre),
        region_labels: region_labels(chart, plot),
        axis_labels: axis_labels(chart, plot),
        points: chart
            .points
            .iter()
            .map(|p| {
                // Data counts up from the bottom; the screen counts down from
                // the top, so the vertical axis inverts here and only here.
                let at = Point::new(
                    plot.at.x + p.x * PLOT_SIZE,
                    plot.at.y + (1.0 - p.y) * PLOT_SIZE,
                );
                PlacedPoint {
                    name: p.name.clone(),
                    at,
                    label_at: Point::new(at.x, at.y + POINT_LABEL_GAP),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadrant::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    #[test]
    fn the_plot_is_square_whatever_surrounds_it() {
        for source in [
            "quadrantChart",
            "quadrantChart\ntitle T",
            "quadrantChart\nx-axis Low --> High\ny-axis Down --> Up\ntitle T",
        ] {
            let plot = placed(source).plot.expect("a plot");
            assert!((plot.width - plot.height).abs() < 1e-9, "{source}");
            assert!((plot.width - PLOT_SIZE).abs() < 1e-9, "{source}");
        }
    }

    #[test]
    fn each_band_of_labels_claims_room_only_when_it_is_used() {
        let bare = placed("quadrantChart");
        let titled = placed("quadrantChart\ntitle T");
        assert!((titled.height - bare.height - TITLE_HEIGHT).abs() < 1e-9);
        let x = placed("quadrantChart\nx-axis Low --> High");
        assert!((x.height - bare.height - X_LABEL_HEIGHT).abs() < 1e-9);
        let y = placed("quadrantChart\ny-axis Down --> Up");
        assert!((y.width - bare.width - Y_LABEL_STRIP).abs() < 1e-9);
    }

    #[test]
    fn one_named_end_is_enough_to_claim_the_band() {
        let one = placed("quadrantChart\nx-axis Low");
        assert!((one.height - placed("quadrantChart").height - X_LABEL_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn the_vertical_axis_inverts_between_data_and_screen() {
        let out = placed("quadrantChart\nlow: [0.5, 0]\nhigh: [0.5, 1]");
        let plot = out.plot.expect("a plot");
        // y = 0 is the bottom of the square, which is the largest screen y.
        assert!((out.points[0].at.y - (plot.at.y + PLOT_SIZE)).abs() < 1e-9);
        assert!((out.points[1].at.y - plot.at.y).abs() < 1e-9);
        // The horizontal axis does not invert.
        assert!((out.points[0].at.x - (plot.at.x + PLOT_SIZE / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_name_sits_below_its_dot() {
        let out = placed("quadrantChart\nA: [0.5, 0.5]");
        let point = &out.points[0];
        assert!((point.label_at.x - point.at.x).abs() < 1e-9);
        assert!((point.label_at.y - point.at.y - POINT_LABEL_GAP).abs() < 1e-9);
    }

    #[test]
    fn the_regions_tint_as_a_checkerboard() {
        let tinted: Vec<bool> = placed("quadrantChart")
            .regions
            .iter()
            .map(|r| r.tinted)
            .collect();
        // Top-right and bottom-left, so no two tinted regions share an edge.
        assert_eq!(tinted, [true, false, true, false]);
    }

    #[test]
    fn the_cross_runs_through_the_centre_of_the_square() {
        let out = placed("quadrantChart");
        let plot = out.plot.expect("a plot");
        let (a, b) = out.cross[0];
        assert!((a.x - (plot.at.x + PLOT_SIZE / 2.0)).abs() < 1e-9);
        assert!((a.x - b.x).abs() < 1e-9, "the first line is vertical");
        let (c, d) = out.cross[1];
        assert!((c.y - d.y).abs() < 1e-9, "the second line is horizontal");
    }

    #[test]
    fn only_a_named_region_gets_a_label_and_it_sits_in_its_own_quarter() {
        let out = placed("quadrantChart\nquadrant-1 TR\nquadrant-3 BL");
        assert_eq!(out.region_labels.len(), 2);
        let plot = out.plot.expect("a plot");
        let (text, at) = &out.region_labels[0];
        assert_eq!(text, "TR");
        assert!(at.x > plot.at.x + PLOT_SIZE / 2.0);
        assert!(at.y < plot.at.y + PLOT_SIZE / 2.0);
    }

    #[test]
    fn the_vertical_axis_labels_turn_and_the_horizontal_ones_do_not() {
        let out = placed("quadrantChart\nx-axis Low --> High\ny-axis Down --> Up");
        assert_eq!(out.axis_labels.len(), 4);
        assert_eq!(out.axis_labels[0].rotate, None);
        assert_eq!(out.axis_labels[2].rotate, Some(-90.0));
        // The turned ones sit left of the plot, the flat ones below it.
        let plot = out.plot.expect("a plot");
        assert!(out.axis_labels[2].at.x < plot.at.x);
        assert!(out.axis_labels[0].at.y > plot.at.y + PLOT_SIZE);
    }

    #[test]
    fn a_title_is_centred_over_the_whole_canvas() {
        let out = placed("quadrantChart\ntitle T\ny-axis Down --> Up");
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "T");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
        assert!(at.y < PADDING + TITLE_HEIGHT);
    }
}
