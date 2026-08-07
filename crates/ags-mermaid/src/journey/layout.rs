//! Where each task sits along the journey.
//!
//! Tasks run left to right in fixed-width columns regardless of which section
//! they belong to; a section is a band drawn over the columns it owns. A task's
//! marker floats above the baseline at a height its score decides.

use crate::round::count;
use crate::scene::Point;

use super::types::Journey;

pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
pub const SECTION_HEADER_H: f64 = 30.0;
pub const SECTION_GAP: f64 = 10.0;
pub const TASK_WIDTH: f64 = 150.0;
pub const PLOT_HEIGHT: f64 = 240.0;
pub const MARKER_RADIUS: f64 = 16.0;
pub const TASK_LABEL_GAP: f64 = 24.0;
pub const ACTOR_GAP: f64 = 20.0;
pub const AXIS_STRIP_W: f64 = 40.0;
pub const SCORE_MIN: i32 = 1;
pub const SCORE_MAX: i32 = 5;

/// One section's band over the columns it owns.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSection {
    pub name: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub label_at: Point,
    pub color_index: usize,
}

/// One task, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedTask {
    /// Derived from the name, disambiguated so two identical steps stay apart.
    pub id: String,
    pub name: String,
    pub score: i32,
    pub actors: Vec<String>,
    pub at: Point,
    /// The dotted drop from the marker to the baseline.
    pub connector: (Point, Point),
    pub label_at: Point,
    pub actors_at: Point,
    /// Inherited from the section, so a step is coloured by where it belongs.
    pub color_index: usize,
}

/// One horizontal rule of the satisfaction scale.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreLine {
    pub score: i32,
    pub a: Point,
    pub b: Point,
    pub label_at: Point,
}

/// A laid-out journey.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub sections: Vec<PlacedSection>,
    pub tasks: Vec<PlacedTask>,
    pub score_lines: Vec<ScoreLine>,
    pub baseline: Option<(Point, Point)>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// The height a score floats at: 5 at the top of the plot, 1 just above the
/// baseline.
fn y_for_score(score: i32, baseline_y: f64) -> f64 {
    let span = PLOT_HEIGHT - MARKER_RADIUS * 2.0;
    let range = f64::from(SCORE_MAX - SCORE_MIN);
    let frac = f64::from(score - SCORE_MIN) / range;
    baseline_y - MARKER_RADIUS - frac * span
}

/// A unique id per task, so two steps with the same name stay separable.
fn unique_id(name: &str, seen: &mut Vec<(String, usize)>) -> String {
    let base = if name.trim().is_empty() {
        "task".to_string()
    } else {
        name.trim().to_string()
    };
    if let Some((_, n)) = seen.iter_mut().find(|(b, _)| *b == base) {
        *n += 1;
        return format!("{base}-{n}");
    }
    seen.push((base.clone(), 1));
    base
}

/// Lay out a parsed journey.
pub fn layout(journey: &Journey) -> Placed {
    let top = PADDING
        + if journey.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let band_y = top;
    let plot_top = band_y + SECTION_HEADER_H + SECTION_GAP;
    let baseline_y = plot_top + PLOT_HEIGHT;
    let left = PADDING + AXIS_STRIP_W;

    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut tasks: Vec<PlacedTask> = Vec::new();
    let mut sections: Vec<PlacedSection> = Vec::new();
    let mut column = 0usize;

    for (index, section) in journey.sections.iter().enumerate() {
        let first = column;
        for task in &section.tasks {
            let x = left + count(column) * TASK_WIDTH + TASK_WIDTH / 2.0;
            let y = y_for_score(task.score, baseline_y);
            let at = Point::new(x, y);
            tasks.push(PlacedTask {
                id: unique_id(&task.name, &mut seen),
                name: task.name.clone(),
                score: task.score,
                actors: task.actors.clone(),
                at,
                connector: (Point::new(x, y + MARKER_RADIUS), Point::new(x, baseline_y)),
                label_at: Point::new(x, baseline_y + TASK_LABEL_GAP),
                actors_at: Point::new(x, baseline_y + TASK_LABEL_GAP + ACTOR_GAP),
                // The section's own index, not the band's — an empty section
                // still consumes a palette slot.
                color_index: index,
            });
            column += 1;
        }
        // A section with no tasks spans nothing, so it draws no band.
        if column > first {
            let band_x = left + count(first) * TASK_WIDTH;
            let band_w = count(column - first) * TASK_WIDTH;
            sections.push(PlacedSection {
                name: section.name.clone(),
                at: Point::new(band_x, band_y),
                width: band_w,
                height: SECTION_HEADER_H,
                label_at: Point::new(band_x + band_w / 2.0, band_y + SECTION_HEADER_H / 2.0),
                color_index: index,
            });
        }
    }

    // An empty journey still gets one column's worth of plot, so the scale has
    // something to be drawn against.
    let right = left + count(column.max(1)) * TASK_WIDTH;
    let score_lines = (SCORE_MIN..=SCORE_MAX)
        .map(|score| {
            let y = y_for_score(score, baseline_y);
            ScoreLine {
                score,
                a: Point::new(left, y),
                b: Point::new(right, y),
                label_at: Point::new(left - 12.0, y),
            }
        })
        .collect();

    // Actors take a second line below the names, but only if anyone named one.
    let has_actors = tasks.iter().any(|t| !t.actors.is_empty());
    let bottom = baseline_y + TASK_LABEL_GAP + if has_actors { ACTOR_GAP } else { 0.0 };
    let width = right + PADDING;

    Placed {
        width,
        height: bottom + PADDING,
        title: journey
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        sections,
        tasks,
        score_lines,
        baseline: Some((Point::new(left, baseline_y), Point::new(right, baseline_y))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const JOURNEY: &str = "journey\n\
        title Day\n\
        section Morning\n\
        Wake: 3: Me\n\
        Tea: 5: Me\n\
        section Evening\n\
        Sleep: 4: Me";

    #[test]
    fn tasks_run_in_columns_across_every_section() {
        let out = placed(JOURNEY);
        let xs: Vec<f64> = out.tasks.iter().map(|t| t.at.x).collect();
        assert!((xs[1] - xs[0] - TASK_WIDTH).abs() < 1e-9);
        // The third task is in the third column even though it opens a section.
        assert!((xs[2] - xs[0] - TASK_WIDTH * 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_higher_score_floats_higher() {
        let out = placed(JOURNEY);
        assert!(out.tasks[1].at.y < out.tasks[0].at.y, "5 sits above 3");
        // The lowest score still clears the baseline by the marker's radius.
        let bottom = y_for_score(1, 100.0);
        assert!((bottom - (100.0 - MARKER_RADIUS)).abs() < 1e-9);
    }

    #[test]
    fn a_section_band_spans_exactly_the_columns_it_owns() {
        let out = placed(JOURNEY);
        assert_eq!(out.sections.len(), 2);
        assert!((out.sections[0].width - TASK_WIDTH * 2.0).abs() < 1e-9);
        assert!((out.sections[1].width - TASK_WIDTH).abs() < 1e-9);
        assert!((out.sections[1].at.x - out.sections[0].at.x - TASK_WIDTH * 2.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_section_draws_no_band_but_still_takes_its_colour() {
        let out = placed("journey\nsection Empty\nsection Full\nA: 3");
        assert_eq!(out.sections.len(), 1);
        // Colour 1, not 0 — the empty section consumed the first slot, and
        // reusing it would give two adjacent sections the same colour.
        assert_eq!(out.sections[0].color_index, 1);
        assert_eq!(out.tasks[0].color_index, 1);
    }

    #[test]
    fn a_task_takes_its_section_colour() {
        let out = placed(JOURNEY);
        let colors: Vec<usize> = out.tasks.iter().map(|t| t.color_index).collect();
        assert_eq!(colors, [0, 0, 1]);
    }

    #[test]
    fn a_repeated_task_name_still_gets_a_distinct_id() {
        let out = placed("journey\nTea: 3\nTea: 5");
        let ids: Vec<&str> = out.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["Tea", "Tea-2"]);
    }

    #[test]
    fn the_connector_drops_from_the_marker_to_the_baseline() {
        let out = placed(JOURNEY);
        let task = &out.tasks[0];
        let (from, to) = task.connector;
        assert!((from.y - (task.at.y + MARKER_RADIUS)).abs() < 1e-9);
        assert!((to.y - out.baseline.expect("a baseline").0.y).abs() < 1e-9);
        assert!((from.x - task.at.x).abs() < 1e-9);
    }

    #[test]
    fn the_whole_scale_is_ruled_whether_or_not_a_score_uses_it() {
        let out = placed("journey\nOnly: 3");
        let scores: Vec<i32> = out.score_lines.iter().map(|l| l.score).collect();
        assert_eq!(scores, [1, 2, 3, 4, 5]);
        // Each rule spans the plot and is numbered to the left of it.
        assert!(out.score_lines[0].label_at.x < out.score_lines[0].a.x);
    }

    #[test]
    fn actors_claim_a_line_only_when_someone_named_one() {
        let with = placed("journey\nA: 3: Me");
        let without = placed("journey\nA: 3");
        assert!((with.height - without.height - ACTOR_GAP).abs() < 1e-9);
    }

    #[test]
    fn an_empty_journey_still_gets_a_plot_to_rule() {
        let out = placed("journey");
        assert!(out.tasks.is_empty());
        assert!((out.width - (PADDING * 2.0 + AXIS_STRIP_W + TASK_WIDTH)).abs() < 1e-9);
        assert_eq!(out.score_lines.len(), 5);
    }

    #[test]
    fn a_title_pushes_the_bands_down_and_centres_itself() {
        let out = placed(JOURNEY);
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "Day");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
        assert!((out.sections[0].at.y - (PADDING + TITLE_HEIGHT)).abs() < 1e-9);
    }
}
