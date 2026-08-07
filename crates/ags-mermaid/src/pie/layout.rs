//! Where the wedges and the legend go.
//!
//! No graph layout is involved: a slice's angle follows directly from its share
//! of the total. The sweep starts at the top and runs clockwise, which is where
//! a reader expects a pie to start.

use crate::round::count;
use crate::scene::Point;

use super::types::Chart;

pub const RADIUS: f64 = 150.0;
/// The hole, as a share of the radius.
///
/// A ring rather than a disc: the eye compares arc lengths, which is what the
/// data is, where a wedge invites it to compare areas near the centre where
/// there is nothing to see. Wide enough a band that a percentage still fits in
/// it — at 0.58 the band is 63px and a 5% slice is 37px along its middle.
pub const INNER_RATIO: f64 = 0.58;
pub const INNER_RADIUS: f64 = RADIUS * INNER_RATIO;
pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
pub const LEGEND_GAP: f64 = 28.0;
pub const LEGEND_ROW_H: f64 = 24.0;
pub const LEGEND_SWATCH: f64 = 14.0;
pub const LEGEND_SWATCH_GAP: f64 = 8.0;
/// Between the longest legend label and the column of shares.
pub const LEGEND_SHARE_GAP: f64 = 20.0;
pub const LEGEND_FONT: f64 = 14.0;
pub const LEGEND_WEIGHT: u32 = 400;
/// How far out from the centre the percentage sits, as a share of the radius:
/// the middle of the band, which is the only place in a ring it can go.
const INNER_LABEL_RADIUS: f64 = f64::midpoint(1.0, INNER_RATIO);

/// One wedge, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSlice {
    pub label: String,
    pub value: f64,
    /// Share of the total, 0..100.
    pub percent: f64,
    /// Where the wedge starts and ends on the circle, in radians.
    pub from: f64,
    pub to: f64,
    /// A single slice covering everything is a disc, not a wedge — an arc from a
    /// point back to itself is degenerate and renders as nothing.
    pub whole: bool,
    /// Anchor for the percentage drawn inside the wedge.
    pub label_at: Point,
    pub color_index: usize,
}

/// One legend row.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendRow {
    pub label: String,
    pub text_at: Point,
    pub swatch_at: Point,
    /// This slice's share, written in the legend because a wedge too small to
    /// carry its own percentage is exactly the one a reader needs it for.
    pub share: String,
    /// The right-hand end of the share, which is drawn to it rather than from
    /// it, so the column lines up under labels of every length.
    pub share_at: Point,
    pub color_index: usize,
}

/// A laid-out pie chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub centre: Point,
    pub slices: Vec<PlacedSlice>,
    pub legend: Vec<LegendRow>,
}

/// A value as the legend shows it: whole numbers bare, the rest to two places.
fn format_value(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v}")
    } else {
        format!("{}", crate::round::round_half_up(v * 100.0) / 100.0)
    }
}

/// A share as the legend writes it.
///
/// Whole percents, except below one — where rounding to a whole number writes
/// "0%" for precisely the slice the reader went to the legend to find out about.
fn share_label(percent: f64) -> String {
    if percent > 0.0 && percent < 1.0 {
        return format!("{}%", crate::round::round_half_up(percent * 10.0) / 10.0);
    }
    format!("{}%", crate::round::round_half_up(percent))
}

/// Where the diagram's name sits: the middle of the band reserved above it.
///
/// Not one font-size below the padding, which is where a baseline naturally
/// lands and which reads as the title having slipped toward the drawing. The
/// band runs from the top of the canvas to where the content begins, so its
/// middle is the only place that looks deliberate.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// Lay out a parsed pie chart.
pub fn layout(chart: &Chart) -> Placed {
    let top = PADDING
        + if chart.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let centre = Point::new(PADDING + RADIUS, top + RADIUS);
    // An empty chart would divide by zero; one slice of nothing is still a
    // circle, so the total floors at one.
    let total: f64 = chart.slices.iter().map(|s| s.value).sum();
    let total = if total == 0.0 { 1.0 } else { total };

    let legend_x = centre.x + RADIUS + LEGEND_GAP;
    let legend_text_x = legend_x + LEGEND_SWATCH + LEGEND_SWATCH_GAP;

    let mut slices = Vec::with_capacity(chart.slices.len());
    let mut legend = Vec::with_capacity(chart.slices.len());
    let mut widest: f64 = 0.0;
    let mut widest_share: f64 = 0.0;
    let mut angle = -std::f64::consts::FRAC_PI_2;

    for (i, slice) in chart.slices.iter().enumerate() {
        let frac = slice.value / total;
        let from = angle;
        let to = angle + frac * std::f64::consts::PI * 2.0;
        angle = to;
        let mid = f64::midpoint(from, to);
        let lr = RADIUS * INNER_LABEL_RADIUS;
        slices.push(PlacedSlice {
            label: slice.label.clone(),
            value: slice.value,
            percent: frac * 100.0,
            from,
            to,
            whole: frac >= 0.9999,
            label_at: Point::new(centre.x + lr * mid.cos(), centre.y + lr * mid.sin()),
            color_index: i,
        });

        let row_y = top + count(i) * LEGEND_ROW_H + LEGEND_ROW_H / 2.0;
        let text = if chart.show_data {
            format!("{} ({})", slice.label, format_value(slice.value))
        } else {
            slice.label.clone()
        };
        widest = widest.max(crate::metrics::text_width(
            &text,
            LEGEND_FONT,
            LEGEND_WEIGHT,
        ));
        let share = share_label(frac * 100.0);
        widest_share = widest_share.max(crate::metrics::text_width(
            &share,
            LEGEND_FONT,
            LEGEND_WEIGHT,
        ));
        legend.push(LegendRow {
            label: text,
            text_at: Point::new(legend_text_x, row_y),
            swatch_at: Point::new(legend_x, row_y - LEGEND_SWATCH / 2.0),
            share,
            // Placed once the widest label is known, below.
            share_at: Point::new(0.0, row_y),
            color_index: i,
        });
    }

    // The column sits past the longest label, so every share ends on one line
    // however long the names are.
    let share_right = legend_text_x + widest + LEGEND_SHARE_GAP + widest_share;
    for row in &mut legend {
        row.share_at = Point::new(share_right, row.share_at.y);
    }

    let legend_bottom = top + count(chart.slices.len()) * LEGEND_ROW_H;
    let width = share_right + PADDING;
    Placed {
        width,
        height: (centre.y + RADIUS).max(legend_bottom) + PADDING,
        title: chart
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        centre,
        slices,
        legend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pie::parse;

    #[test]
    fn every_row_carries_its_share_including_the_wedges_too_small_to_write_on() {
        // 2% is under the threshold the renderer needs to write inside a wedge,
        // which is the whole reason the legend carries the number.
        let placed = layout(&parse("pie\n\"big\" : 98\n\"small\" : 2"));
        let shares: Vec<&str> = placed.legend.iter().map(|row| row.share.as_str()).collect();
        assert_eq!(shares, vec!["98%", "2%"]);
    }

    #[test]
    fn the_shares_line_up_in_a_column_of_their_own() {
        // Right-aligned, so they read as a column however long the names are.
        let placed = layout(&parse(
            "pie\n\"a very long browser name indeed\" : 1\n\"b\" : 1",
        ));
        let ends: Vec<f64> = placed.legend.iter().map(|row| row.share_at.x).collect();
        assert!(
            ends.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9),
            "{ends:?}"
        );
        // And past the longest label, not through it.
        let text_end = placed
            .legend
            .iter()
            .map(|row| {
                row.text_at.x + crate::metrics::text_width(&row.label, LEGEND_FONT, LEGEND_WEIGHT)
            })
            .fold(0.0, f64::max);
        assert!(placed.legend[0].share_at.x > text_end, "{placed:?}");
        // The canvas grew to hold the column.
        assert!(placed.width > placed.legend[0].share_at.x);
    }

    #[test]
    fn a_share_under_one_percent_is_not_written_as_nought() {
        // Rounding to whole percents would write "0%" for precisely the slice a
        // reader consults the legend about.
        let placed = layout(&parse("pie\n\"big\" : 999\n\"tiny\" : 1"));
        assert_eq!(placed.legend[1].share, "0.1%");
        // A whole percent still reads as one.
        assert_eq!(share_label(25.0), "25%");
        // And nothing at all is nought, not a decimal.
        assert_eq!(share_label(0.0), "0%");
    }

    #[test]
    fn the_sweep_starts_at_the_top_and_runs_clockwise() {
        let placed = layout(&parse("pie\n\"a\" : 1\n\"b\" : 1"));
        assert!((placed.slices[0].from + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!(placed.slices[0].to > placed.slices[0].from);
        assert!((placed.slices[0].to - placed.slices[1].from).abs() < 1e-9);
    }

    #[test]
    fn shares_are_proportional_and_sum_to_the_whole() {
        let placed = layout(&parse("pie\n\"a\" : 30\n\"b\" : 10"));
        assert!((placed.slices[0].percent - 75.0).abs() < 1e-9);
        assert!((placed.slices[1].percent - 25.0).abs() < 1e-9);
        let sweep: f64 = placed.slices.iter().map(|s| s.to - s.from).sum();
        assert!((sweep - std::f64::consts::TAU).abs() < 1e-9);
    }

    #[test]
    fn one_slice_covering_everything_is_a_disc() {
        let placed = layout(&parse("pie\n\"only\" : 5"));
        assert!(placed.slices[0].whole);
        assert!(!layout(&parse("pie\n\"a\" : 1\n\"b\" : 1")).slices[0].whole);
    }

    #[test]
    fn a_chart_of_nothing_still_has_a_canvas() {
        let placed = layout(&parse("pie"));
        assert!(placed.width > 0.0 && placed.height > 0.0);
        assert!(placed.slices.is_empty());
    }

    #[test]
    fn slices_that_are_all_zero_do_not_divide_by_it() {
        let placed = layout(&parse("pie\n\"a\" : 0\n\"b\" : 0"));
        assert!(placed.slices.iter().all(|s| s.percent == 0.0));
    }

    #[test]
    fn a_title_pushes_the_circle_down_and_centres_itself() {
        let bare = layout(&parse("pie\n\"a\" : 1"));
        let titled = layout(&parse("pie title Shares\n\"a\" : 1"));
        assert!((titled.centre.y - bare.centre.y - TITLE_HEIGHT).abs() < 1e-9);
        let (text, at) = titled.title.expect("a title");
        assert_eq!(text, "Shares");
        assert!((at.x - titled.width / 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_legend_sits_to_the_right_and_widens_the_canvas() {
        let short = layout(&parse("pie\n\"a\" : 1"));
        let long = layout(&parse("pie\n\"a considerably longer label\" : 1"));
        assert!(long.width > short.width);
        assert!(short.legend[0].swatch_at.x > short.centre.x + RADIUS);
    }

    #[test]
    fn showing_data_puts_the_value_beside_the_label() {
        let placed = layout(&parse(
            "pie showData\n\"a\" : 40\n\"b\" : 2.5\n\"c\" : 2.345\n\"d\" : 1.005",
        ));
        assert_eq!(placed.legend[0].label, "a (40)");
        assert_eq!(placed.legend[1].label, "b (2.5)");
        // Two places, no more.
        assert_eq!(placed.legend[2].label, "c (2.35)");
        // 1.005 is not really 1.005 in binary — it is a hair below, so it
        // rounds down. Pinned because it looks like a bug and is not one, and
        // because the renderer this replaces rounds it the same way.
        assert_eq!(placed.legend[3].label, "d (1)");
    }

    #[test]
    fn a_long_legend_decides_the_height_when_the_circle_does_not() {
        let many: String = (0..40)
            .map(|i| format!("\"s{i}\" : 1\n"))
            .collect::<Vec<_>>()
            .concat();
        let placed = layout(&parse(&format!("pie\n{many}")));
        assert!(placed.height > placed.centre.y + RADIUS);
    }
}
