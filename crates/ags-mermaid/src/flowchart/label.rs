//! Putting an edge's label beside its run.
//!
//! Beside rather than on: a label drawn over the line hides the thing it is
//! naming. It has to clear the boxes and every wire but its own, which is why
//! the routes are all in hand before any label is placed.

use crate::label::{beside, runs, Placed as PlacedLabel};
use crate::scene::Point;

use super::config::Config;
use super::layout::label_size;

/// How far a label stands off the wire it names.
pub(super) const LABEL_GAP: f64 = 4.0;

/// The middle of the longest run of a route, and whether that run is upright.
///
/// The *longest* run, so a label has room to sit rather than landing on a corner.
pub(super) fn longest(points: &[Point]) -> Option<(Point, bool)> {
    let (_, a, b) = points
        .windows(2)
        .filter_map(|pair| {
            let (a, b) = (pair.first()?, pair.get(1)?);
            Some(((b.x - a.x).hypot(b.y - a.y), *a, *b))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;
    Some((
        Point::new(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y)),
        (b.y - a.y).abs() > (b.x - a.x).abs(),
    ))
}

/// Where a label sits, given everything already placed.
///
/// Beside its run rather than on it — a label centred on its line has that line
/// drawn straight through the middle of the word. `taken` holds the boxes and the
/// labels already placed, `lines` every run but this edge's own, so a label
/// pushed off its own wire is not pushed onto a box or somebody else's.
pub(super) fn label_at(
    points: &[Point],
    label: &str,
    cfg: &Config,
    taken: &[PlacedLabel],
    lines: &[(Point, Point)],
) -> Option<PlacedLabel> {
    let (middle, upright) = longest(points)?;
    Some(beside(
        middle,
        upright,
        label_size(label, cfg.edge_label_font, cfg.edge_label_weight, cfg),
        LABEL_GAP,
        taken,
        lines,
    ))
}

/// Every run of every route but `mine`, which a label has to keep off.
pub(super) fn elsewhere(routes: &[Vec<Point>], mine: usize) -> Vec<(Point, Point)> {
    routes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != mine)
        .flat_map(|(_, points)| runs(points))
        .collect()
}
