//! Where each bar sits on the timeline.
//!
//! The day axis maps straight onto x, one row per task, grouped into section
//! bands. The header band's height is not fixed: date labels are drawn turned
//! on their side, so the *widest* label decides how tall the band has to be.

use crate::round::count;
use crate::scene::Point;

use super::parser::add_days;
use super::types::{Chart, Status};

pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
/// The least the header band may be, before the labels ask for more.
pub const HEADER_HEIGHT: f64 = 30.0;
pub const HEADER_LABEL_PAD: f64 = 12.0;
/// Between a turned label's foot and the top of the grid.
pub const HEADER_LABEL_GAP: f64 = 4.0;
pub const ROW_HEIGHT: f64 = 30.0;
pub const BAR_HEIGHT: f64 = 18.0;
pub const DAY_WIDTH: f64 = 26.0;
/// So a task of under a day is still something you can point at.
pub const MIN_BAR_WIDTH: f64 = 8.0;
pub const MILESTONE_RADIUS: f64 = 9.0;
pub const LABEL_GAP: f64 = 12.0;
pub const MIN_LABEL_COL: f64 = 80.0;
pub const MAX_LABEL_COL: f64 = 280.0;
pub const SECTION_LABEL_STRIP: f64 = 22.0;
pub const TASK_FONT: f64 = 13.0;
pub const TASK_WEIGHT: u32 = 500;
pub const HEADER_FONT: f64 = 11.0;
pub const HEADER_WEIGHT: u32 = 500;

/// A rectangle, in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// One day's rule, and the date on it when it carries one.
#[derive(Debug, Clone, PartialEq)]
pub struct GridLine {
    pub x: f64,
    pub y1: f64,
    pub y2: f64,
    /// Only every `tick_step`th line is labelled.
    pub label: Option<(String, Point)>,
}

/// One section's band.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSection {
    pub name: String,
    pub band: Rect,
    pub label_at: Point,
    pub color_index: usize,
}

/// One task, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedTask {
    pub id: String,
    pub name: String,
    pub tags: Vec<Status>,
    pub milestone: bool,
    pub bar: Rect,
    /// The centre a milestone's diamond is drawn about.
    pub centre: Point,
    pub label_at: Point,
    pub color_index: usize,
}

/// A laid-out gantt chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub grid: Option<Rect>,
    pub grid_lines: Vec<GridLine>,
    pub sections: Vec<PlacedSection>,
    pub tasks: Vec<PlacedTask>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// What a rule at day `d` is labelled: a date when the chart is anchored to
/// one, otherwise a count of days from the start.
fn tick_label(chart: &Chart, day: i64) -> String {
    chart
        .start_date
        .as_ref()
        .map_or_else(|| format!("+{day}d"), |start| add_days(start, day))
}

/// Where day `day` falls on the axis.
fn day_to_x(grid_x: f64, day: i64) -> f64 {
    grid_x + count(usize::try_from(day.max(0)).unwrap_or(0)) * DAY_WIDTH
}

/// How many days apart the labelled rules are, and how tall the band holding
/// them has to be.
///
/// The labels are drawn turned on their side, so a label's *width* is what eats
/// into the band's height, and the widest one decides it.
fn header_band(chart: &Chart, total_days: i64) -> (usize, f64) {
    // Turned labels only have to clear each other by a cap height. One day apart
    // already does, until the rules themselves get tight — then thin them out.
    let min_spacing = HEADER_FONT + 6.0;
    // A label needs this many days of room; one is the floor, since a rule
    // cannot be thinner than a day.
    // Bounded by the chart's own width: past that there is one label left and
    // nothing to collide with.
    let tick_step = (1..=32)
        .find(|n| count(*n) * DAY_WIDTH >= min_spacing)
        .unwrap_or(1);
    let widest = (0..=total_days)
        .step_by(tick_step)
        .map(|d| crate::metrics::text_width(&tick_label(chart, d), HEADER_FONT, HEADER_WEIGHT))
        .fold(0.0_f64, f64::max);
    (
        tick_step,
        HEADER_HEIGHT.max(widest.ceil() + HEADER_LABEL_PAD + HEADER_LABEL_GAP),
    )
}

/// One band per section, one row per task inside it.
fn place_rows(
    chart: &Chart,
    grid_x: f64,
    grid_y: f64,
    grid_w: f64,
) -> (Vec<PlacedSection>, Vec<PlacedTask>) {
    let mut sections = Vec::with_capacity(chart.sections.len());
    let mut tasks = Vec::with_capacity(chart.tasks.len());
    let mut row = 0usize;
    for (index, section) in chart.sections.iter().enumerate() {
        let band_y = grid_y + count(row) * ROW_HEIGHT;
        let band_h = count(section.tasks.len()) * ROW_HEIGHT;
        sections.push(PlacedSection {
            name: section.name.clone(),
            band: Rect {
                at: Point::new(PADDING, band_y),
                width: grid_x + grid_w - PADDING,
                height: band_h,
            },
            label_at: Point::new(PADDING + SECTION_LABEL_STRIP / 2.0, band_y + band_h / 2.0),
            color_index: index,
        });
        for task in &section.tasks {
            let row_y = grid_y + count(row) * ROW_HEIGHT;
            let x = day_to_x(grid_x, task.start_day);
            tasks.push(PlacedTask {
                id: task.id.clone(),
                name: task.name.clone(),
                tags: task.tags.clone(),
                milestone: task.milestone,
                bar: Rect {
                    at: Point::new(x, row_y + (ROW_HEIGHT - BAR_HEIGHT) / 2.0),
                    width: (count(usize::try_from(task.duration_days.max(0)).unwrap_or(0))
                        * DAY_WIDTH)
                        .max(MIN_BAR_WIDTH),
                    height: BAR_HEIGHT,
                },
                centre: Point::new(x, row_y + ROW_HEIGHT / 2.0),
                label_at: Point::new(grid_x - LABEL_GAP, row_y + ROW_HEIGHT / 2.0),
                color_index: index,
            });
            row += 1;
        }
    }
    (sections, tasks)
}

/// Lay out a parsed gantt chart.
pub fn layout(chart: &Chart) -> Placed {
    // At least one day, so a chart of nothing still has a grid to draw.
    let total_days = chart
        .tasks
        .iter()
        .map(|t| t.end_day)
        .max()
        .unwrap_or(1)
        .max(1);
    let top = PADDING
        + if chart.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };

    // The left column fits the longest name, within bounds.
    let label_col = chart
        .tasks
        .iter()
        .map(|t| crate::metrics::text_width(&t.name, TASK_FONT, TASK_WEIGHT) + 8.0)
        .fold(MIN_LABEL_COL, f64::max)
        .min(MAX_LABEL_COL);
    let grid_x = PADDING + SECTION_LABEL_STRIP + label_col + LABEL_GAP;

    let (tick_step, header) = header_band(chart, total_days);
    let grid_y = top + header;
    let grid_w = count(usize::try_from(total_days).unwrap_or(1)) * DAY_WIDTH;
    let grid_h = count(chart.tasks.len()) * ROW_HEIGHT;

    let grid_lines = (0..=total_days)
        .map(|d| {
            let x = day_to_x(grid_x, d);
            GridLine {
                x,
                y1: grid_y,
                y2: grid_y + grid_h,
                label: (usize::try_from(d).unwrap_or(0) % tick_step == 0).then(|| {
                    (
                        tick_label(chart, d),
                        Point::new(x, grid_y - HEADER_LABEL_GAP),
                    )
                }),
            }
        })
        .collect();

    let (sections, tasks) = place_rows(chart, grid_x, grid_y, grid_w);

    let width = grid_x + grid_w + PADDING;
    Placed {
        width,
        height: grid_y + grid_h + PADDING,
        title: chart
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        grid: Some(Rect {
            at: Point::new(grid_x, grid_y),
            width: grid_w,
            height: grid_h,
        }),
        grid_lines,
        sections,
        tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gantt::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const CHART: &str = "gantt\n\
        title A plan\n\
        section Build\n\
        Design    :done, des, 2024-01-01, 5d\n\
        Implement :active, imp, after des, 10d\n\
        section Ship\n\
        Release   :milestone, rel, after imp, 0d";

    #[test]
    fn a_day_maps_to_a_fixed_width() {
        let out = placed(CHART);
        assert!((out.tasks[0].bar.width - 5.0 * DAY_WIDTH).abs() < 1e-9);
        assert!((out.tasks[1].bar.at.x - out.tasks[0].bar.at.x - 5.0 * DAY_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn one_row_per_task_across_every_section() {
        let out = placed(CHART);
        assert_eq!(out.tasks.len(), 3);
        for pair in out.tasks.windows(2) {
            assert!((pair[1].centre.y - pair[0].centre.y - ROW_HEIGHT).abs() < 1e-9);
        }
    }

    #[test]
    fn a_section_band_spans_exactly_its_own_rows() {
        let out = placed(CHART);
        assert!((out.sections[0].band.height - ROW_HEIGHT * 2.0).abs() < 1e-9);
        assert!((out.sections[1].band.height - ROW_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn a_milestone_keeps_a_minimum_width_so_it_can_still_be_pointed_at() {
        let out = placed(CHART);
        assert!(out.tasks[2].milestone);
        assert!((out.tasks[2].bar.width - MIN_BAR_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn the_header_band_grows_to_fit_a_turned_date_label() {
        // Dated labels are far wider than `+0d`, and they are drawn on their
        // side — so the band has to be taller, not wider.
        let dated = placed("gantt\nA :2024-01-01, 3d");
        let relative = placed("gantt\nA :3d");
        let header = |p: &Placed| p.grid.expect("a grid").at.y;
        assert!(header(&dated) > header(&relative));
    }

    #[test]
    fn every_day_gets_a_rule_and_the_labelled_ones_are_evenly_spaced() {
        let out = placed(CHART);
        assert_eq!(out.grid_lines.len(), 16, "days 0 to 15");
        let labelled = out.grid_lines.iter().filter(|l| l.label.is_some()).count();
        assert_eq!(labelled, out.grid_lines.len(), "a day apart already clears");
    }

    #[test]
    fn the_name_column_fits_the_longest_name_within_bounds() {
        let short = placed("gantt\nA :1d");
        assert!(
            (short.tasks[0].label_at.x - (PADDING + SECTION_LABEL_STRIP + MIN_LABEL_COL)).abs()
                < 1e-9
        );
        let long = placed(&format!("gantt\n{} :1d", "x".repeat(200)));
        let column = long.tasks[0].label_at.x - PADDING - SECTION_LABEL_STRIP;
        assert!((column - MAX_LABEL_COL).abs() < 1e-9);
    }

    #[test]
    fn a_name_is_written_to_the_left_of_the_grid() {
        let out = placed(CHART);
        assert!(out.tasks[0].label_at.x < out.grid.expect("a grid").at.x);
    }

    #[test]
    fn a_title_pushes_the_header_down_and_centres_itself() {
        let out = placed(CHART);
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "A plan");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_chart_still_gets_a_day_of_grid() {
        let out = placed("gantt");
        assert!(out.tasks.is_empty());
        assert_eq!(out.grid_lines.len(), 2, "days 0 and 1");
        assert!(out.width > 0.0);
    }
}
