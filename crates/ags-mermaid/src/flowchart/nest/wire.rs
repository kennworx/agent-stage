//! Joining the pieces of one edge back into a single run.
//!
//! An edge crossing a boundary is drawn in stretches, one per container. Which
//! end each stretch came from — never how deep it was drawn — is what puts them
//! back in the order the wire is travelled.

use crate::layout::Point;

/// module note.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    /// Inside the source's containers, running outward.
    Source,
    /// Drawn by the container that routes the whole edge.
    Whole,
    /// Inside the target's containers, running inward.
    Target,
}

/// A stretch of one edge, drawn by one container.
pub(super) struct Piece {
    pub(super) edge: usize,
    pub(super) side: Side,
    pub(super) depth: usize,
    pub(super) points: Vec<Point>,
}

/// the container that routes the whole edge.
pub(super) fn joined(mut pieces: Vec<&Piece>, room: f64) -> Vec<Point> {
    pieces.sort_by_key(|piece| match piece.side {
        Side::Source => (0usize, usize::MAX - piece.depth),
        Side::Whole => (1, 0),
        Side::Target => (2, piece.depth),
    });
    let mut out: Vec<Point> = Vec::new();
    for piece in pieces {
        if let (Some(last), Some(next)) = (out.last().copied(), piece.points.first().copied()) {
            let apart = ((last.x - next.x).abs() > 0.5, (last.y - next.y).abs() > 0.5);
            // Two pieces normally meet on one point, inside the padding band.
            // They do not when the child runs across its parent's grain: the
            // port is then on a face at right angles to the one the parent
            // arrived at, and the pieces have to be squared up. Carrying on the
            // way the last run was already going turns once rather than twice.
            if apart.0 && apart.1 {
                let carried = out
                    .iter()
                    .rev()
                    .nth(1)
                    .is_some_and(|before| (before.y - last.y).abs() < 0.5);
                out.push(if carried {
                    Point::new(next.x, last.y)
                } else {
                    Point::new(last.x, next.y)
                });
            }
        }
        for at in &piece.points {
            // The join itself falls on one point, which would otherwise be drawn
            // twice and read as a zero-length run.
            if out.last().is_some_and(|last: &Point| {
                (last.x - at.x).abs() < 0.5 && (last.y - at.y).abs() < 0.5
            }) {
                continue;
            }
            out.push(*at);
        }
    }
    unjog(&out, room)
}

/// Drop a step too short to be worth the two corners it costs.
///
/// Each container routes its own stretch of a wire and settles where that
/// stretch sits without seeing the next, so two stretches meet a hair apart —
/// 1.5px on one measured diagram — and the wire pays a full corner each side of
/// a step nobody can see. Below the room a run needs to separate itself from
/// anything, a step separates the wire from nothing.
///
/// The tail is slid onto the earlier line rather than the step being deleted,
/// because a wire is a chain: shortening one run leaves everything after it
/// where it was. Sliding moves the far end along the face it arrives at, by less
/// than `room` — invisible where a kink in the middle of a straight line is not.
fn unjog(points: &[Point], room: f64) -> Vec<Point> {
    let mut out = points.to_vec();
    // Interior runs only: the first and last are a wire's grip on its two boxes,
    // and sliding those off their faces is a worse fault than a small step.
    let mut at = 1;
    while at + 2 < out.len() {
        let (Some(before), Some(from), Some(to), Some(after)) = (
            out.get(at - 1).copied(),
            out.get(at).copied(),
            out.get(at + 1).copied(),
            out.get(at + 2).copied(),
        ) else {
            break;
        };
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let short = |d: f64| d.abs() > 1e-9 && d.abs() < room;
        // A step, not merely a short run: a run whose neighbours both lie along
        // the other axis. A short run *between two runs of its own axis* is one
        // stretch of a straight line, and pulling that in would drag the whole
        // rest of the wire back along itself.
        let across = |a: Point, b: Point, horizontal: bool| {
            if horizontal {
                (a.y - b.y).abs() > 1e-9
            } else {
                (a.x - b.x).abs() > 1e-9
            }
        };
        let step =
            |horizontal: bool| across(before, from, horizontal) && across(to, after, horizontal);
        if short(dx) && dy.abs() < 1e-9 && step(true) {
            for point in out.iter_mut().skip(at + 1) {
                point.x -= dx;
            }
        } else if short(dy) && dx.abs() < 1e-9 && step(false) {
            for point in out.iter_mut().skip(at + 1) {
                point.y -= dy;
            }
        } else {
            at += 1;
        }
    }
    tidy(&out)
}

/// Drop the points a collapsed step left sitting on a straight run.
fn tidy(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for at in points {
        if out
            .last()
            .is_some_and(|last: &Point| (last.x - at.x).abs() < 0.5 && (last.y - at.y).abs() < 0.5)
        {
            continue;
        }
        let flat = match (out.len().checked_sub(2), out.last()) {
            (Some(i), Some(last)) => out.get(i).is_some_and(|before| {
                ((before.x - last.x).abs() < 0.5 && (last.x - at.x).abs() < 0.5)
                    || ((before.y - last.y).abs() < 0.5 && (last.y - at.y).abs() < 0.5)
            }),
            _ => false,
        };
        if flat {
            if let Some(last) = out.last_mut() {
                *last = *at;
            }
        } else {
            out.push(*at);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_are_joined_in_the_order_the_wire_is_travelled() {
        // Deepest source piece first, then the router's, then target pieces
        // outermost first. Sorting by depth alone puts both extremes together.
        // Each piece runs at its own height, so the order they were joined in is
        // still readable after the collinear points have been dropped.
        let piece = |side, depth, x: f64, y: f64| Piece {
            edge: 0,
            side,
            depth,
            points: vec![Point::new(x, y), Point::new(x + 1.0, y)],
        };
        let all = [
            piece(Side::Target, 2, 40.0, 90.0),
            piece(Side::Whole, 0, 20.0, 30.0),
            piece(Side::Source, 1, 0.0, 0.0),
            piece(Side::Target, 1, 30.0, 60.0),
        ];
        let run = joined(all.iter().collect(), 12.0);
        let xs: Vec<f64> = run.iter().map(|at| at.x).collect();
        assert!(
            xs.windows(2).all(|w| match (w.first(), w.get(1)) {
                (Some(a), Some(b)) => a <= b,
                _ => true,
            }),
            "the wire doubles back: {run:?}"
        );
        for want in [0.0_f64, 20.0, 30.0, 40.0] {
            assert!(
                xs.iter().any(|x| (x - want).abs() < 0.5),
                "the piece at {want} is missing from {run:?}"
            );
        }
    }

    #[test]
    fn two_pieces_meeting_at_a_point_do_not_repeat_it() {
        let run = joined(
            vec![
                &Piece {
                    edge: 0,
                    side: Side::Whole,
                    depth: 0,
                    points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
                },
                &Piece {
                    edge: 0,
                    side: Side::Target,
                    depth: 1,
                    points: vec![Point::new(10.0, 0.0), Point::new(10.0, 5.0)],
                },
            ],
            12.0,
        );
        assert_eq!(run.len(), 3, "the shared point is drawn once: {run:?}");
    }

    #[test]
    fn pieces_at_right_angles_are_squared_up_rather_than_cut_across() {
        // A group running across its parent's grain leaves its port on a face at
        // right angles to the one the parent arrived at.
        let run = joined(
            vec![
                &Piece {
                    edge: 0,
                    side: Side::Whole,
                    depth: 0,
                    points: vec![Point::new(0.0, 0.0), Point::new(0.0, 10.0)],
                },
                &Piece {
                    edge: 0,
                    side: Side::Target,
                    depth: 1,
                    points: vec![Point::new(20.0, 30.0), Point::new(25.0, 30.0)],
                },
            ],
            12.0,
        );
        for pair in run.windows(2) {
            let square = (pair[0].x - pair[1].x).abs() < 0.5 || (pair[0].y - pair[1].y).abs() < 0.5;
            assert!(square, "every run stays axis-aligned: {run:?}");
        }
    }

    fn at(points: &[(f64, f64)]) -> Vec<Point> {
        points.iter().map(|(x, y)| Point::new(*x, *y)).collect()
    }

    fn pairs(points: &[Point]) -> Vec<(f64, f64)> {
        points.iter().map(|p| (p.x, p.y)).collect()
    }

    #[test]
    fn a_step_too_short_to_see_costs_no_corners() {
        // The measured case: two containers' stretches met 1.5px apart, and the
        // wire bent twice to cover it.
        let run = unjog(
            &at(&[
                (0.0, 100.0),
                (50.0, 100.0),
                (50.0, 98.5),
                (120.0, 98.5),
                (160.0, 98.5),
            ]),
            12.0,
        );
        assert_eq!(pairs(&run), vec![(0.0, 100.0), (160.0, 100.0)]);
    }

    #[test]
    fn a_step_worth_its_corners_is_left_alone() {
        let points = at(&[(0.0, 100.0), (50.0, 100.0), (50.0, 60.0), (160.0, 60.0)]);
        assert_eq!(pairs(&unjog(&points, 12.0)), pairs(&points));
    }

    #[test]
    fn a_short_step_sideways_is_collapsed_too() {
        // The same fault on the other axis, which a diagram laid out across the
        // page reaches instead.
        let run = unjog(
            &at(&[
                (100.0, 0.0),
                (100.0, 50.0),
                (98.5, 50.0),
                (98.5, 120.0),
                (98.5, 160.0),
            ]),
            12.0,
        );
        assert_eq!(pairs(&run), vec![(100.0, 0.0), (100.0, 160.0)]);
    }

    #[test]
    fn the_two_ends_keep_their_grip_on_their_boxes() {
        // A short first run is the wire leaving its box, and a short last run is
        // it arriving: neither is a step to collapse.
        let points = at(&[(0.0, 0.0), (4.0, 0.0), (4.0, 60.0), (4.0, 64.0)]);
        assert_eq!(
            pairs(&unjog(&points, 12.0)),
            vec![(0.0, 0.0), (4.0, 0.0), (4.0, 64.0)]
        );
    }

    #[test]
    fn a_run_with_nothing_to_collapse_comes_back_as_it_went_in() {
        let points = at(&[(0.0, 0.0), (60.0, 0.0)]);
        assert_eq!(pairs(&unjog(&points, 12.0)), pairs(&points));
        assert!(unjog(&[], 12.0).is_empty());
    }
}
