//! Where the periods, their markers and their events sit.
//!
//! Periods map onto evenly spaced columns and events stack under each, so no
//! graph layout is involved. The rows, top to bottom: section bands when any
//! section is named, then period labels, then the spine, then the event cards.

use crate::round::count;
use crate::scene::Point;

use super::types::Timeline;

pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 44.0;
pub const TITLE_FONT: f64 = 20.0;
pub const SECTION_HEADER_HEIGHT: f64 = 34.0;
pub const SECTION_HEADER_GAP: f64 = 14.0;
pub const COL_WIDTH: f64 = 180.0;
pub const COL_GAP: f64 = 18.0;
pub const PERIOD_LABEL_HEIGHT: f64 = 36.0;
pub const AXIS_GAP: f64 = 22.0;
pub const EVENT_GAP_Y: f64 = 12.0;
pub const EVENT_HEIGHT: f64 = 40.0;
pub const MARKER_RADIUS: f64 = 6.0;

/// A section header band, spanning the columns of its periods.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    pub name: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub label_at: Point,
    pub color_index: usize,
}

/// One period: a marker on the spine and a label above it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPeriod {
    pub label: String,
    pub marker_at: Point,
    pub label_at: Point,
    pub color_index: usize,
}

/// One event card.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEvent {
    /// Deterministic and unique within the diagram: a colliding id would
    /// misroute a reviewer's note to a different event.
    pub id: String,
    pub text: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub label_at: Point,
    pub color_index: usize,
}

/// A laid-out timeline.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub bands: Vec<Band>,
    /// The horizontal spine, absent when there are no periods to connect.
    pub axis: Option<(Point, Point)>,
    pub periods: Vec<PlacedPeriod>,
    pub events: Vec<PlacedEvent>,
}

/// A unique id for an event, derived from its own text.
fn unique_id(text: &str, used: &mut Vec<(String, usize)>) -> String {
    let base = match text.trim() {
        "" => "event",
        t => t,
    }
    .to_string();
    if let Some((_, seen)) = used.iter_mut().find(|(b, _)| *b == base) {
        *seen += 1;
        return format!("{base}#{seen}");
    }
    used.push((base.clone(), 1));
    base
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

/// Lay out a parsed timeline.
pub fn layout(timeline: &Timeline) -> Placed {
    let has_title = timeline.title.is_some();
    let has_bands = timeline.sections.iter().any(|s| s.name.is_some());

    let top = PADDING + if has_title { TITLE_HEIGHT } else { 0.0 };
    let period_label_y = top
        + if has_bands {
            SECTION_HEADER_HEIGHT + SECTION_HEADER_GAP
        } else {
            0.0
        };
    let axis_y = period_label_y + PERIOD_LABEL_HEIGHT;
    let events_top = axis_y + AXIS_GAP;

    let col_left = |i: usize| PADDING + count(i) * (COL_WIDTH + COL_GAP);
    let col_centre = |i: usize| col_left(i) + COL_WIDTH / 2.0;

    let mut bands = Vec::new();
    let mut periods = Vec::new();
    let mut events = Vec::new();
    let mut used: Vec<(String, usize)> = Vec::new();
    let mut col = 0usize;
    let mut lowest = events_top;

    for (si, section) in timeline.sections.iter().enumerate() {
        if section.periods.is_empty() {
            continue;
        }
        let first_col = col;
        for period in &section.periods {
            let cx = col_centre(col);
            periods.push(PlacedPeriod {
                label: period.label.clone(),
                marker_at: Point::new(cx, axis_y),
                label_at: Point::new(cx, period_label_y + PERIOD_LABEL_HEIGHT / 2.0),
                color_index: si,
            });
            for (j, text) in period.events.iter().enumerate() {
                let y = events_top + count(j) * (EVENT_HEIGHT + EVENT_GAP_Y);
                lowest = lowest.max(y + EVENT_HEIGHT);
                events.push(PlacedEvent {
                    id: unique_id(text, &mut used),
                    text: text.clone(),
                    at: Point::new(col_left(col), y),
                    width: COL_WIDTH,
                    height: EVENT_HEIGHT,
                    label_at: Point::new(cx, y + EVENT_HEIGHT / 2.0),
                    color_index: si,
                });
            }
            col += 1;
        }
        if let Some(name) = section.name.as_ref().filter(|_| has_bands) {
            let x = col_left(first_col);
            bands.push(Band {
                name: name.clone(),
                at: Point::new(x, top),
                width: col_left(col - 1) + COL_WIDTH - x,
                height: SECTION_HEADER_HEIGHT,
                label_at: Point::new(
                    x + (col_left(col - 1) + COL_WIDTH - x) / 2.0,
                    top + SECTION_HEADER_HEIGHT / 2.0,
                ),
                color_index: si,
            });
        }
    }

    let axis = (col > 0).then(|| {
        (
            Point::new(col_centre(0), axis_y),
            Point::new(col_centre(col - 1), axis_y),
        )
    });
    // An empty timeline is still one column wide, so it reads as a timeline
    // with nothing on it rather than as a rendering failure.
    let width = if col > 0 {
        col_left(col - 1) + COL_WIDTH + PADDING
    } else {
        2.0 * PADDING + COL_WIDTH
    };
    Placed {
        width,
        height: lowest.max(axis_y) + PADDING,
        title: timeline
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        bands,
        axis,
        periods,
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::parse;

    #[test]
    fn periods_run_left_to_right_on_one_spine() {
        let placed = layout(&parse("timeline\n1 : a\n2 : b\n3 : c"));
        let xs: Vec<f64> = placed.periods.iter().map(|p| p.marker_at.x).collect();
        assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
        let ys: Vec<f64> = placed.periods.iter().map(|p| p.marker_at.y).collect();
        assert!(ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9));
        let (from, to) = placed.axis.expect("a spine");
        assert!((from.x - xs[0]).abs() < 1e-9);
        assert!((to.x - xs[2]).abs() < 1e-9);
    }

    #[test]
    fn events_stack_downward_under_their_period() {
        let placed = layout(&parse("timeline\n1 : a : b : c"));
        let ys: Vec<f64> = placed.events.iter().map(|e| e.at.y).collect();
        assert!(ys.windows(2).all(|w| w[1] > w[0]), "{ys:?}");
        assert!(placed
            .events
            .iter()
            .all(|e| (e.at.x - placed.events[0].at.x).abs() < 1e-9));
        // Below the spine, not above it.
        assert!(ys[0] > placed.periods[0].marker_at.y);
    }

    #[test]
    fn a_named_section_gets_a_band_spanning_its_columns() {
        let placed = layout(&parse(
            "timeline\nsection Alpha\n1 : a\n2 : b\nsection Beta\n3 : c",
        ));
        assert_eq!(placed.bands.len(), 2);
        let alpha = &placed.bands[0];
        // Two columns wide, and the second band starts after it.
        assert!((alpha.width - (2.0 * COL_WIDTH + COL_GAP)).abs() < 1e-9);
        assert!(placed.bands[1].at.x > alpha.at.x + alpha.width);
    }

    #[test]
    fn a_timeline_with_no_named_section_has_no_bands_and_sits_higher() {
        let bare = layout(&parse("timeline\n1 : a"));
        let banded = layout(&parse("timeline\nsection S\n1 : a"));
        assert!(bare.bands.is_empty());
        assert!(banded.periods[0].marker_at.y > bare.periods[0].marker_at.y);
    }

    #[test]
    fn an_empty_section_takes_no_column() {
        let placed = layout(&parse("timeline\nsection Empty\nsection Full\n1 : a"));
        assert_eq!(placed.periods.len(), 1);
        assert_eq!(placed.bands.len(), 1);
        assert_eq!(placed.bands[0].name, "Full");
    }

    #[test]
    fn every_event_is_addressable_and_a_repeat_still_resolves() {
        let placed = layout(&parse("timeline\n1 : same\n2 : same : same"));
        let ids: Vec<&str> = placed.events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["same", "same#2", "same#3"]);
    }

    #[test]
    fn the_canvas_grows_with_the_columns_and_the_deepest_stack() {
        let one = layout(&parse("timeline\n1 : a"));
        let wide = layout(&parse("timeline\n1 : a\n2 : b"));
        let deep = layout(&parse("timeline\n1 : a : b : c : d"));
        assert!(wide.width > one.width);
        assert!(deep.height > one.height);
    }

    #[test]
    fn an_empty_timeline_is_still_one_column_wide() {
        let placed = layout(&parse("timeline"));
        assert!((placed.width - (2.0 * PADDING + COL_WIDTH)).abs() < 1e-9);
        assert!(placed.axis.is_none());
        assert!(placed.periods.is_empty());
    }

    #[test]
    fn a_title_pushes_everything_down_and_centres_itself() {
        let placed = layout(&parse("timeline title Ours\n1 : a"));
        let (text, at) = placed.title.clone().expect("a title");
        assert_eq!(text, "Ours");
        assert!((at.x - placed.width / 2.0).abs() < 1e-9);
        assert!(placed.periods[0].marker_at.y > TITLE_HEIGHT);
    }
}
