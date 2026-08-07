//! Where the plot area, the ticks, the bars and the curves sit.
//!
//! A fixed plot rectangle with margins grown around it: the y labels decide how
//! wide the left margin is, a title or a legend push the top down, and an axis
//! title claims a strip of its own. Nothing here is a graph layout — every
//! coordinate is a scale applied to a value.
//!
//! A horizontal chart is the same drawing with the two scales exchanged, not a
//! rotation: the categories run down the left and the values across the bottom,
//! and the axis *titles* swap with them.

use crate::metrics::text_width;
use crate::round::count;
use crate::scene::Point;

use super::types::{format_tick, Chart, Range, SeriesKind};

pub const PLOT_WIDTH: f64 = 600.0;
pub const PLOT_HEIGHT: f64 = 340.0;
pub const PADDING: f64 = 22.0;
pub const TITLE_FONT: f64 = 18.0;
pub const TITLE_WEIGHT: u32 = 600;
pub const TITLE_HEIGHT: f64 = 42.0;
pub const LABEL_FONT: f64 = 14.0;
pub const LABEL_WEIGHT: u32 = 400;
pub const AXIS_TITLE_FONT: f64 = 15.0;
pub const AXIS_TITLE_WEIGHT: u32 = 500;
/// The strip below the plot that the category labels sit in.
pub const X_LABEL_HEIGHT: f64 = 38.0;
/// The least the left margin may be, before the labels ask for more.
pub const MIN_Y_LABEL_WIDTH: f64 = 58.0;
/// Between a y label and the plot's edge.
pub const Y_LABEL_GAP: f64 = 18.0;
/// The strip an axis title claims.
pub const AXIS_TITLE_PAD: f64 = 30.0;
/// How far a label sits below the x axis.
pub const X_LABEL_DROP: f64 = 18.0;
/// A bar band gives this fraction of itself up to the gaps either side.
pub const BAR_PAD_RATIO: f64 = 0.2;
pub const BAR_GROUP_GAP: f64 = 0.0;
pub const MAX_BAR_WIDTH: f64 = 40.0;
/// The least a category label column may be, on a horizontal chart.
pub const MIN_CATEGORY_WIDTH: f64 = 40.0;
pub const LEGEND_FONT: f64 = 14.0;
pub const LEGEND_WEIGHT: u32 = 400;
pub const LEGEND_HEIGHT: f64 = 28.0;
pub const LEGEND_SWATCH_W: f64 = 14.0;
pub const LEGEND_SWATCH_H: f64 = 14.0;
/// Between a legend swatch and its words.
pub const LEGEND_GAP: f64 = 6.0;
/// Between two legend entries.
pub const LEGEND_ITEM_GAP: f64 = 16.0;

/// Which way a tick's label is aligned about its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Middle,
    End,
}

/// One labelled position on an axis.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub label: String,
    pub at: Point,
    pub align: Align,
}

/// An axis title, and the quarter turn it may be written on.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTitle {
    pub text: String,
    pub at: Point,
    pub turned: bool,
}

/// One axis: its labelled positions and its title.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacedAxis {
    pub ticks: Vec<Tick>,
    pub title: Option<AxisTitle>,
}

/// The rectangle the data is drawn inside.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Plot {
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// One bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub value: f64,
    pub label: String,
    pub color_index: usize,
}

/// One point on a curve.
#[derive(Debug, Clone, PartialEq)]
pub struct Vertex {
    pub at: Point,
    pub value: f64,
    pub label: String,
}

/// One curve.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub points: Vec<Vertex>,
    /// Which line series this is, counting only lines.
    pub series_index: usize,
    pub color_index: usize,
}

/// One legend entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendItem {
    pub label: String,
    pub at: Point,
    pub kind: SeriesKind,
    pub series_index: usize,
    pub color_index: usize,
}

/// A laid-out xy chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub horizontal: bool,
    pub title: Option<(String, Point)>,
    pub x_axis: PlacedAxis,
    pub y_axis: PlacedAxis,
    pub plot: Plot,
    pub bars: Vec<Bar>,
    pub curves: Vec<Curve>,
    /// The rules across the plot, one per value tick.
    pub grid: Vec<(Point, Point)>,
    pub legend: Vec<LegendItem>,
}

/// Round numbers to label an axis with, covering `min..=max`.
///
/// Six intervals is the target; the interval is then rounded out to a 1, 2 or 5
/// times a power of ten, so the labels read as numbers a person would choose.
pub fn nice_ticks(min: f64, max: f64) -> Vec<f64> {
    let span = max - min;
    if span <= 0.0 {
        return vec![min];
    }
    let raw = span / 6.0;
    let magnitude = 10.0_f64.powf(raw.log10().floor());
    let residual = raw / magnitude;
    let interval = if residual <= 1.5 {
        magnitude
    } else if residual <= 3.0 {
        2.0 * magnitude
    } else if residual <= 7.0 {
        5.0 * magnitude
    } else {
        10.0 * magnitude
    };
    let mut out = Vec::new();
    let mut value = (min / interval).ceil() * interval;
    // The slack on the last step is the reference's, and it keeps a tick that
    // lands exactly on `max` from being dropped by floating-point drift.
    while value <= max + interval * 0.001 {
        // Ten decimal places is well past anything a chart shows, and it is what
        // clears the noise that repeated addition leaves behind.
        out.push((value * 1e10).round() / 1e10);
        value += interval;
    }
    out
}

/// How wide the widest of `labels` is.
fn widest(labels: &[String], font: f64, weight: u32, floor: f64) -> f64 {
    labels
        .iter()
        .map(|label| text_width(label, font, weight))
        .fold(floor, f64::max)
}

/// The margins the chart's furniture claims around the plot.
struct Margins {
    top: f64,
    left: f64,
    width: f64,
    height: f64,
}

impl Margins {
    fn of(chart: &Chart, left_labels: f64) -> Self {
        let title = if chart.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
        let legend = if chart.series.len() > 1 {
            LEGEND_HEIGHT
        } else {
            0.0
        };
        // On a horizontal chart the axis titles change places with the axes, so
        // the strip on the left is claimed by the x title and the one below by
        // the y title.
        let (side_title, below_title) = if chart.horizontal {
            (chart.x_axis.title.is_some(), chart.y_axis.title.is_some())
        } else {
            (chart.y_axis.title.is_some(), chart.x_axis.title.is_some())
        };
        let top = PADDING + title + legend;
        let bottom = PADDING + X_LABEL_HEIGHT + if below_title { AXIS_TITLE_PAD } else { 0.0 };
        let left =
            PADDING + left_labels + Y_LABEL_GAP + if side_title { AXIS_TITLE_PAD } else { 0.0 };
        Self {
            top,
            left,
            width: left + PLOT_WIDTH + PADDING,
            height: top + PLOT_HEIGHT + bottom,
        }
    }
}

/// Where each series' colour comes from: its own position among all of them.
fn colour_indices(chart: &Chart) -> Vec<usize> {
    (0..chart.series.len()).collect()
}

/// How wide one bar is, and how wide a whole group of them is.
fn bar_metrics(bars: usize, band: f64) -> (f64, f64) {
    let usable = band * (1.0 - BAR_PAD_RATIO);
    let raw = if bars > 1 {
        (usable - count(bars - 1) * BAR_GROUP_GAP) / count(bars)
    } else {
        usable
    };
    let single = raw.min(MAX_BAR_WIDTH);
    let group = if bars > 1 {
        single * count(bars) + BAR_GROUP_GAP * count(bars - 1)
    } else {
        single
    };
    (single, group)
}

/// The legend, centred about `centre_x`.
fn legend(chart: &Chart, centre_x: f64, y: f64, colours: &[usize]) -> Vec<LegendItem> {
    if chart.series.len() <= 1 {
        return Vec::new();
    }
    let mut items = Vec::with_capacity(chart.series.len());
    let (mut bars, mut lines) = (0usize, 0usize);
    for (at, series) in chart.series.iter().enumerate() {
        let ordinal = if series.kind == SeriesKind::Bar {
            bars += 1;
            bars
        } else {
            lines += 1;
            lines
        };
        items.push(LegendItem {
            label: format!("{} {ordinal}", series.kind.token()),
            at: Point::new(0.0, y),
            kind: series.kind,
            series_index: ordinal - 1,
            color_index: colours.get(at).copied().unwrap_or(0),
        });
    }
    let widths: Vec<f64> = items
        .iter()
        .map(|item| {
            LEGEND_SWATCH_W + LEGEND_GAP + text_width(&item.label, LEGEND_FONT, LEGEND_WEIGHT)
        })
        .collect();
    let total: f64 = widths.iter().sum::<f64>() + count(items.len() - 1) * LEGEND_ITEM_GAP;
    let mut x = centre_x - total / 2.0;
    for (item, width) in items.iter_mut().zip(&widths) {
        item.at.x = x;
        x += width + LEGEND_ITEM_GAP;
    }
    items
}

/// Everything both directions need before the scales differ.
struct Frame {
    values: Vec<f64>,
    labels: Vec<String>,
    range: Range,
    colours: Vec<usize>,
    count: usize,
}

impl Frame {
    fn of(chart: &Chart) -> Self {
        let range = chart.value_range();
        Self {
            values: nice_ticks(range.min, range.max),
            labels: chart.category_labels(),
            range,
            colours: colour_indices(chart),
            count: chart.data_count(),
        }
    }

    /// Where a value falls along a `length`-long axis, from its start.
    fn offset(&self, value: f64, length: f64) -> f64 {
        let span = self.range.max - self.range.min;
        let span = if span == 0.0 { 1.0 } else { span };
        (value - self.range.min) / span * length
    }
}

/// The two scales a chart is drawn against, and which way round they go.
struct Scales<'a> {
    category: Box<dyn Fn(usize) -> f64 + 'a>,
    value: Box<dyn Fn(f64) -> f64 + 'a>,
    /// Whether the values run across the page rather than up it.
    across: bool,
}

impl Scales<'_> {
    /// A point at category `index` holding `value`.
    fn point(&self, index: usize, value: f64) -> Point {
        let (category, value) = ((self.category)(index), (self.value)(value));
        if self.across {
            Point::new(value, category)
        } else {
            Point::new(category, value)
        }
    }
}

/// Where a bar sits along its band, and how thick it is drawn.
struct Lane {
    base: f64,
    start: f64,
    thickness: f64,
    colour: usize,
}

/// One bar, given its lane and the datum it stands for.
fn place_bar(scales: &Scales, frame: &Frame, lane: &Lane, index: usize, value: f64) -> Bar {
    let label = frame.labels.get(index).cloned().unwrap_or_default();
    let Lane {
        base,
        start,
        thickness,
        colour,
    } = *lane;
    if scales.across {
        // A value below the axis is pinned to it rather than drawn off the plot,
        // which is what the reference does here and not the other way up.
        let reach = (scales.value)(value.max(frame.range.min));
        return Bar {
            at: Point::new(base.min(reach), start),
            width: (reach - base).abs(),
            height: thickness,
            value,
            label,
            color_index: colour,
        };
    }
    let reach = (scales.value)(value);
    Bar {
        at: Point::new(start, reach.min(base)),
        width: thickness,
        height: (base - reach).abs(),
        value,
        label,
        color_index: colour,
    }
}

/// Every series placed against `scales`: bars share their band, curves take a
/// point per value.
fn place_series(
    chart: &Chart,
    frame: &Frame,
    scales: &Scales,
    band: f64,
) -> (Vec<Bar>, Vec<Curve>) {
    let bar_count = chart
        .series
        .iter()
        .filter(|series| series.kind == SeriesKind::Bar)
        .count();
    let (thickness, group) = bar_metrics(bar_count, band);
    let base = (scales.value)(frame.range.min.max(0.0));
    let mut bars = Vec::new();
    let mut curves = Vec::new();
    let (mut bar_ordinal, mut line_ordinal) = (0usize, 0usize);
    for (at, series) in chart.series.iter().enumerate() {
        let colour = frame.colours.get(at).copied().unwrap_or(0);
        if series.kind == SeriesKind::Bar {
            for (index, value) in series.data.iter().enumerate() {
                let start = (scales.category)(index) - group / 2.0
                    + count(bar_ordinal) * (thickness + BAR_GROUP_GAP);
                bars.push(place_bar(
                    scales,
                    frame,
                    &Lane {
                        base,
                        start,
                        thickness,
                        colour,
                    },
                    index,
                    *value,
                ));
            }
            bar_ordinal += 1;
        } else {
            curves.push(Curve {
                points: series
                    .data
                    .iter()
                    .enumerate()
                    .map(|(index, value)| Vertex {
                        at: scales.point(index, *value),
                        value: *value,
                        label: frame.labels.get(index).cloned().unwrap_or_default(),
                    })
                    .collect(),
                series_index: line_ordinal,
                color_index: colour,
            });
            line_ordinal += 1;
        }
    }
    (bars, curves)
}

/// The labelled positions along the category axis.
fn category_ticks(frame: &Frame, at: impl Fn(usize) -> Point, align: Align) -> Vec<Tick> {
    frame
        .labels
        .iter()
        .enumerate()
        .map(|(index, label)| Tick {
            label: label.clone(),
            at: at(index),
            align,
        })
        .collect()
}

/// The labelled positions along the value axis.
fn value_ticks(frame: &Frame, at: impl Fn(f64) -> Point, align: Align) -> Vec<Tick> {
    frame
        .values
        .iter()
        .map(|value| Tick {
            label: format_tick(*value),
            at: at(*value),
            align,
        })
        .collect()
}

/// The furniture both directions share: the title, the plot and the legend.
fn shell(chart: &Chart, frame: &Frame, margins: &Margins) -> Placed {
    Placed {
        width: margins.width,
        height: margins.height,
        horizontal: chart.horizontal,
        title: chart
            .title
            .clone()
            .map(|text| (text, Point::new(margins.width / 2.0, PADDING + TITLE_FONT))),
        plot: Plot {
            at: Point::new(margins.left, margins.top),
            width: PLOT_WIDTH,
            height: PLOT_HEIGHT,
        },
        legend: legend(chart, margins.width / 2.0, legend_y(chart), &frame.colours),
        ..Placed::default()
    }
}

/// The title written along the bottom, under the value or category labels.
fn bottom_title(text: Option<String>, margins: &Margins) -> Option<AxisTitle> {
    text.map(|text| AxisTitle {
        text,
        at: Point::new(margins.left + PLOT_WIDTH / 2.0, margins.height - PADDING),
        turned: false,
    })
}

/// The title written up the left-hand side.
fn side_title(text: Option<String>, margins: &Margins) -> Option<AxisTitle> {
    text.map(|text| AxisTitle {
        text,
        at: Point::new(PADDING + 4.0, margins.top + PLOT_HEIGHT / 2.0),
        turned: true,
    })
}

/// A chart drawn the usual way up: categories across, values up.
fn vertical(chart: &Chart, frame: &Frame, margins: &Margins) -> Placed {
    let (left, top) = (margins.left, margins.top);
    let band = PLOT_WIDTH / count(frame.count);
    let scales = Scales {
        category: Box::new(move |index| left + (count(index) + 0.5) * band),
        value: Box::new(move |value| top + PLOT_HEIGHT - frame.offset(value, PLOT_HEIGHT)),
        across: false,
    };
    let (bars, curves) = place_series(chart, frame, &scales, band);
    Placed {
        x_axis: PlacedAxis {
            ticks: category_ticks(
                frame,
                |index| Point::new((scales.category)(index), top + PLOT_HEIGHT + X_LABEL_DROP),
                Align::Middle,
            ),
            title: bottom_title(chart.x_axis.title.clone(), margins),
        },
        y_axis: PlacedAxis {
            ticks: value_ticks(
                frame,
                |value| Point::new(left - Y_LABEL_GAP, (scales.value)(value)),
                Align::End,
            ),
            title: side_title(chart.y_axis.title.clone(), margins),
        },
        grid: frame
            .values
            .iter()
            .map(|value| {
                let y = (scales.value)(*value);
                (Point::new(left, y), Point::new(left + PLOT_WIDTH, y))
            })
            .collect(),
        bars,
        curves,
        ..shell(chart, frame, margins)
    }
}

/// A chart turned on its side: categories down, values across.
///
/// The axis *titles* change places along with the axes — the one the source
/// wrote for x now describes the left-hand side.
fn horizontal(chart: &Chart, frame: &Frame, margins: &Margins) -> Placed {
    let (left, top) = (margins.left, margins.top);
    let band = PLOT_HEIGHT / count(frame.count);
    let scales = Scales {
        category: Box::new(move |index| top + (count(index) + 0.5) * band),
        value: Box::new(move |value| left + frame.offset(value, PLOT_WIDTH)),
        across: true,
    };
    let (bars, curves) = place_series(chart, frame, &scales, band);
    Placed {
        x_axis: PlacedAxis {
            ticks: value_ticks(
                frame,
                |value| Point::new((scales.value)(value), top + PLOT_HEIGHT + X_LABEL_DROP),
                Align::Middle,
            ),
            title: bottom_title(chart.y_axis.title.clone(), margins),
        },
        y_axis: PlacedAxis {
            ticks: category_ticks(
                frame,
                |index| Point::new(left - Y_LABEL_GAP, (scales.category)(index)),
                Align::End,
            ),
            title: side_title(chart.x_axis.title.clone(), margins),
        },
        grid: frame
            .values
            .iter()
            .map(|value| {
                let x = (scales.value)(*value);
                (Point::new(x, top), Point::new(x, top + PLOT_HEIGHT))
            })
            .collect(),
        bars,
        curves,
        ..shell(chart, frame, margins)
    }
}

/// The middle of the strip a legend sits in.
fn legend_y(chart: &Chart) -> f64 {
    PADDING
        + if chart.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        }
        + LEGEND_HEIGHT / 2.0
}

/// Lay out a parsed xy chart.
pub fn layout(chart: &Chart) -> Placed {
    let frame = Frame::of(chart);
    if chart.horizontal {
        // The left margin holds the category names rather than the numbers.
        let margins = Margins::of(
            chart,
            widest(&frame.labels, LABEL_FONT, LABEL_WEIGHT, MIN_CATEGORY_WIDTH),
        );
        return horizontal(chart, &frame, &margins);
    }
    let numbers: Vec<String> = frame.values.iter().map(|v| format_tick(*v)).collect();
    let margins = Margins::of(
        chart,
        widest(&numbers, LABEL_FONT, LABEL_WEIGHT, MIN_Y_LABEL_WIDTH),
    );
    vertical(chart, &frame, &margins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xychart::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const BARS: &str = "xychart-beta\ntitle \"Sales\"\nx-axis [A, B, C]\nbar [10, 20, 30]";

    #[test]
    fn nice_ticks_land_on_numbers_a_person_would_pick() {
        assert_eq!(nice_ticks(0.0, 100.0), [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        assert_eq!(nice_ticks(0.0, 6.0), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(
            nice_ticks(0.0, 30.0),
            [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0]
        );
    }

    #[test]
    fn a_span_of_nothing_gets_a_single_tick() {
        assert_eq!(nice_ticks(5.0, 5.0), [5.0]);
        assert_eq!(nice_ticks(5.0, 1.0), [5.0]);
    }

    #[test]
    fn a_tick_run_starts_at_the_first_round_number_inside_the_span() {
        let ticks = nice_ticks(3.0, 97.0);
        assert!(ticks[0] >= 3.0);
        assert!(ticks.last().copied().unwrap_or(0.0) <= 97.0 + 1e-9);
    }

    #[test]
    fn the_plot_is_a_fixed_rectangle_with_margins_grown_round_it() {
        let out = placed(BARS);
        assert!((out.plot.width - PLOT_WIDTH).abs() < 1e-9);
        assert!((out.plot.height - PLOT_HEIGHT).abs() < 1e-9);
        assert!((out.width - (out.plot.at.x + PLOT_WIDTH + PADDING)).abs() < 1e-9);
        assert!(out.plot.at.y >= PADDING + TITLE_HEIGHT);
    }

    #[test]
    fn a_title_and_a_legend_each_push_the_plot_down() {
        let bare = placed("xychart-beta\nx-axis [A]\nbar [1]");
        let titled = placed("xychart-beta\ntitle \"T\"\nx-axis [A]\nbar [1]");
        let two = placed("xychart-beta\nx-axis [A]\nbar [1]\nbar [2]");
        assert!((titled.plot.at.y - bare.plot.at.y - TITLE_HEIGHT).abs() < 1e-9);
        assert!((two.plot.at.y - bare.plot.at.y - LEGEND_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn an_axis_title_claims_a_strip_of_its_own() {
        let bare = placed("xychart-beta\nx-axis [A]\nbar [1]");
        let sided = placed("xychart-beta\nx-axis [A]\ny-axis \"V\"\nbar [1]");
        let below = placed("xychart-beta\nx-axis \"C\" [A]\nbar [1]");
        assert!((sided.plot.at.x - bare.plot.at.x - AXIS_TITLE_PAD).abs() < 1e-9);
        assert!((below.height - bare.height - AXIS_TITLE_PAD).abs() < 1e-9);
        assert!(sided.y_axis.title.as_ref().is_some_and(|t| t.turned));
        assert!(below.x_axis.title.as_ref().is_some_and(|t| !t.turned));
    }

    #[test]
    fn a_wide_number_widens_the_left_margin() {
        let small = placed("xychart-beta\nx-axis [A]\ny-axis 0 --> 10\nbar [1]");
        let large = placed("xychart-beta\nx-axis [A]\ny-axis 0 --> 100000000\nbar [1]");
        assert!(large.plot.at.x > small.plot.at.x);
    }

    #[test]
    fn a_category_sits_under_the_middle_of_its_band() {
        let out = placed(BARS);
        assert_eq!(out.x_axis.ticks.len(), 3);
        let band = PLOT_WIDTH / 3.0;
        assert!((out.x_axis.ticks[0].at.x - (out.plot.at.x + band / 2.0)).abs() < 1e-9);
        assert_eq!(out.x_axis.ticks[0].align, Align::Middle);
        assert_eq!(out.x_axis.ticks[2].label, "C");
    }

    #[test]
    fn a_value_label_sits_left_of_the_plot_and_a_rule_runs_across_it() {
        let out = placed(BARS);
        assert_eq!(out.y_axis.ticks.len(), out.grid.len());
        assert_eq!(out.y_axis.ticks[0].align, Align::End);
        assert!((out.y_axis.ticks[0].at.x - (out.plot.at.x - Y_LABEL_GAP)).abs() < 1e-9);
        assert!((out.grid[0].0.x - out.plot.at.x).abs() < 1e-9);
        assert!((out.grid[0].1.x - (out.plot.at.x + PLOT_WIDTH)).abs() < 1e-9);
    }

    #[test]
    fn a_bar_stands_on_the_baseline_and_reaches_its_value() {
        let out = placed(BARS);
        assert_eq!(out.bars.len(), 3);
        let floor = out.plot.at.y + PLOT_HEIGHT;
        for bar in &out.bars {
            assert!((bar.at.y + bar.height - floor).abs() < 1.0, "on the floor");
        }
        assert!(out.bars[2].height > out.bars[0].height, "30 beats 10");
        assert_eq!(out.bars[0].label, "A");
        assert!((out.bars[0].value - 10.0).abs() < 1e-9);
    }

    #[test]
    fn two_bar_series_share_a_band_without_overlapping() {
        let out = placed("xychart-beta\nx-axis [A, B]\nbar [1, 2]\nbar [3, 4]");
        assert_eq!(out.bars.len(), 4);
        let first = &out.bars[0];
        let second = &out.bars[2];
        assert!((second.at.x - first.at.x - first.width).abs() < 1e-9);
        assert_eq!(first.color_index, 0);
        assert_eq!(second.color_index, 1);
    }

    #[test]
    fn a_bar_is_never_wider_than_its_ceiling() {
        // Two categories over 600px would give a 240px bar without the cap.
        let out = placed("xychart-beta\nx-axis [A, B]\nbar [1, 2]");
        assert!((out.bars[0].width - MAX_BAR_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn a_curve_takes_one_point_per_value() {
        let out = placed("xychart-beta\nx-axis [A, B, C]\nline [1, 2, 3]");
        assert_eq!(out.curves.len(), 1);
        assert_eq!(out.curves[0].points.len(), 3);
        assert_eq!(out.curves[0].points[1].label, "B");
        assert!(
            out.curves[0].points[2].at.y < out.curves[0].points[0].at.y,
            "3 is higher"
        );
    }

    #[test]
    fn bars_and_curves_keep_their_own_running_numbers_but_share_a_palette() {
        let out = placed("xychart-beta\nx-axis [A]\nbar [1]\nline [2]\nline [3]");
        assert_eq!(out.bars[0].color_index, 0);
        assert_eq!(out.curves[0].color_index, 1);
        assert_eq!(out.curves[1].color_index, 2);
        assert_eq!(out.curves[0].series_index, 0);
        assert_eq!(out.curves[1].series_index, 1);
    }

    #[test]
    fn a_legend_appears_only_once_there_is_more_than_one_series() {
        assert!(placed(BARS).legend.is_empty());
        let out = placed("xychart-beta\nx-axis [A]\nbar [1]\nline [2]\nbar [3]");
        let labels: Vec<&str> = out.legend.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Bar 1", "Line 1", "Bar 2"]);
    }

    #[test]
    fn a_legend_is_centred_across_the_chart() {
        let out = placed("xychart-beta\nx-axis [A]\nbar [1]\nbar [2]");
        let first = out.legend[0].at.x;
        let last = out.legend[1].at.x
            + LEGEND_SWATCH_W
            + LEGEND_GAP
            + text_width("Bar 2", LEGEND_FONT, LEGEND_WEIGHT);
        assert!((f64::midpoint(first, last) - out.width / 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_horizontal_chart_runs_its_categories_down_the_side() {
        let out = placed("xychart-beta horizontal\nx-axis [Python, Go]\nbar [30, 12]");
        assert!(out.horizontal);
        assert_eq!(out.y_axis.ticks.len(), 2);
        assert_eq!(out.y_axis.ticks[0].label, "Python");
        assert!(out.y_axis.ticks[1].at.y > out.y_axis.ticks[0].at.y);
        // The value ticks run along the bottom instead.
        assert_eq!(out.x_axis.ticks[0].align, Align::Middle);
        assert!(out.x_axis.ticks[1].at.x > out.x_axis.ticks[0].at.x);
    }

    #[test]
    fn a_horizontal_bar_grows_rightwards_from_the_baseline() {
        let out = placed("xychart-beta horizontal\nx-axis [A, B]\nbar [30, 12]");
        assert_eq!(out.bars.len(), 2);
        assert!((out.bars[0].at.x - out.plot.at.x).abs() < 1e-9);
        assert!(out.bars[0].width > out.bars[1].width, "30 beats 12");
        assert!((out.bars[0].height - MAX_BAR_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn a_horizontal_chart_swaps_which_title_goes_where() {
        let out = placed(
            "xychart-beta horizontal\nx-axis \"Team\" [A]\ny-axis \"Spend\" 0 --> 10\nbar [5]",
        );
        // The categories are on the left, so their title is the turned one.
        assert_eq!(
            out.y_axis.title.as_ref().map(|t| t.text.as_str()),
            Some("Team")
        );
        assert_eq!(
            out.x_axis.title.as_ref().map(|t| t.text.as_str()),
            Some("Spend")
        );
        assert!(out.y_axis.title.as_ref().is_some_and(|t| t.turned));
    }

    #[test]
    fn a_horizontal_chart_grows_its_left_margin_for_a_long_name() {
        let short = placed("xychart-beta horizontal\nx-axis [A]\nbar [1]");
        let long = placed("xychart-beta horizontal\nx-axis [A very long category name]\nbar [1]");
        assert!(long.plot.at.x > short.plot.at.x);
    }

    #[test]
    fn a_horizontal_curve_runs_down_the_categories() {
        let out = placed("xychart-beta horizontal\nx-axis [A, B]\nline [1, 2]");
        assert_eq!(out.curves[0].points.len(), 2);
        assert!(out.curves[0].points[1].at.y > out.curves[0].points[0].at.y);
        assert!(out.curves[0].points[1].at.x > out.curves[0].points[0].at.x);
    }

    #[test]
    fn a_chart_of_nothing_still_has_a_plot_to_draw_on() {
        let out = placed("xychart-beta");
        assert!(out.bars.is_empty());
        assert!(out.curves.is_empty());
        assert!((out.plot.width - PLOT_WIDTH).abs() < 1e-9);
        assert_eq!(out.x_axis.ticks.len(), 1, "one nameless column");
        assert!(out.title.is_none());
    }

    #[test]
    fn a_flat_span_does_not_divide_by_zero() {
        let out = placed("xychart-beta\nx-axis [A, B]\ny-axis 5 --> 5\nbar [5, 5]");
        assert!(out.bars.iter().all(|bar| bar.at.y.is_finite()));
    }
}
