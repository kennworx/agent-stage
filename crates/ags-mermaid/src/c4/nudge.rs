//! Pulling apart edges that ended up drawn on top of one another.
//!
//! Routing scores each edge against the ones already placed, but nothing stops
//! two of them sharing a lane when that is the cheapest path for both — and the
//! lattice offers only three lanes per gutter, so a corridor carrying four edges
//! has no clear choice to make. The result is runs that coincide within a few
//! pixels: not crossings, so a crossing count reports them as fine, and yet
//! impossible to follow, because two edges render as one line.
//!
//! This is the ordering-and-nudging stage of the standard orthogonal routing
//! pipeline, applied after the fact: find the runs sharing a line, order them so
//! separating them introduces no new crossing, and fan them out across whatever
//! gap the boxes leave.

use super::config as l;
use super::geom::{clamp, count, dedupe, simplify, Axis, Point, Rect, EPS};

/// One straight run of a route that separation is allowed to move.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Run {
    /// Which route, and which leg of it: `points[i] → points[i + 1]`.
    route: usize,
    i: usize,
    axis: Axis,
    /// Fixed coordinate of the run: `y` when horizontal, `x` when vertical.
    coord: f64,
    lo: f64,
    hi: f64,
    /// The mean of the corners on either side, perpendicular to the run.
    ///
    /// Sorting a contended line by it puts the edge that arrives from the left on
    /// the left, which is what keeps separation from trading a merged run for a
    /// new crossing.
    order: f64,
}

/// The runs separation may move: every straight leg except the first and last.
///
/// The end legs carry the ports and the arrowhead. Moving one slides the edge off
/// the box face it was assigned, undoing face selection.
fn nudgeable_runs(routes: &[Vec<Point>]) -> Vec<Run> {
    let mut out = Vec::new();
    for (route, pts) in routes.iter().enumerate() {
        for i in 1..pts.len().saturating_sub(2) {
            let (Some(a), Some(b), Some(prev), Some(next)) =
                (pts.get(i), pts.get(i + 1), pts.get(i - 1), pts.get(i + 2))
            else {
                continue;
            };
            let vertical = (a.x - b.x).abs() < EPS;
            let horizontal = (a.y - b.y).abs() < EPS;
            if vertical == horizontal {
                continue;
            }
            let along = |p: &Point| if vertical { p.y } else { p.x };
            let across = |p: &Point| if vertical { p.x } else { p.y };
            out.push(Run {
                route,
                i,
                axis: if vertical { Axis::V } else { Axis::H },
                coord: across(a),
                lo: along(a).min(along(b)),
                hi: along(a).max(along(b)),
                order: f64::midpoint(across(prev), across(next)),
            });
        }
    }
    out
}

/// Split runs on one line into groups whose spans overlap.
fn overlapping_runs(line: &[Run]) -> Vec<Vec<Run>> {
    let mut sorted = line.to_vec();
    sorted.sort_by(|p, q| p.lo.total_cmp(&q.lo));
    let mut runs: Vec<Vec<Run>> = Vec::new();
    let mut cur: Vec<Run> = Vec::new();
    let mut reach = f64::NEG_INFINITY;
    for s in sorted {
        if !cur.is_empty() && s.lo > reach {
            runs.push(std::mem::take(&mut cur));
        }
        reach = reach.max(s.hi);
        cur.push(s);
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

/// Runs sharing a line, split into clusters that actually overlap along it.
fn contended_clusters(segs: &[Run]) -> Vec<Vec<Run>> {
    let mut clusters = Vec::new();
    for axis in [Axis::H, Axis::V] {
        let mut same: Vec<Run> = segs.iter().copied().filter(|s| s.axis == axis).collect();
        same.sort_by(|p, q| p.coord.total_cmp(&q.coord));
        let mut line: Vec<Run> = Vec::new();
        for s in same {
            let broke = line
                .last()
                .is_some_and(|prev| s.coord - prev.coord > l::NUDGE_EPS);
            if broke {
                clusters.extend(
                    overlapping_runs(&std::mem::take(&mut line))
                        .into_iter()
                        .filter(|run| run.len() > 1),
                );
            }
            line.push(s);
        }
        clusters.extend(
            overlapping_runs(&line)
                .into_iter()
                .filter(|run| run.len() > 1),
        );
    }
    clusters
}

/// How far a run may move either way before it meets a box.
fn free_span(seg: &Run, boxes: &[Rect]) -> (f64, f64) {
    let mut lo = f64::NEG_INFINITY;
    let mut hi = f64::INFINITY;
    for b in boxes {
        let (along_lo, along_hi, across_lo, across_hi) = match seg.axis {
            Axis::V => (b.y, b.bottom(), b.x, b.right()),
            Axis::H => (b.x, b.right(), b.y, b.bottom()),
        };
        if along_hi <= seg.lo || along_lo >= seg.hi {
            continue;
        }
        if across_hi <= seg.coord {
            lo = lo.max(across_hi);
        } else if across_lo >= seg.coord {
            hi = hi.min(across_lo);
        }
    }
    (lo, hi)
}

/// Slide one run to a new fixed coordinate, carrying its two corners with it.
fn move_run(seg: &Run, routes: &mut [Vec<Point>], to: f64) {
    let Some(route) = routes.get_mut(seg.route) else {
        return;
    };
    for idx in [seg.i, seg.i + 1] {
        if let Some(p) = route.get_mut(idx) {
            match seg.axis {
                Axis::V => p.x = to,
                Axis::H => p.y = to,
            }
        }
    }
}

/// Fan one contended cluster out around its own centre, within the free gap.
fn separate(cluster: &mut [Run], routes: &mut [Vec<Point>], boxes: &[Rect]) {
    cluster.sort_by(|p, q| p.order.total_cmp(&q.order));
    let mut lo = f64::NEG_INFINITY;
    let mut hi = f64::INFINITY;
    for s in cluster.iter() {
        let (gap_lo, gap_hi) = free_span(s, boxes);
        lo = lo.max(gap_lo);
        hi = hi.min(gap_hi);
    }
    let centre = cluster.iter().map(|s| s.coord).sum::<f64>() / count(cluster.len());
    let room = if lo.is_finite() && hi.is_finite() {
        hi - lo - l::NUDGE_SEP
    } else {
        f64::INFINITY
    };
    let sep = l::NUDGE_SEP.min(room / count(cluster.len().saturating_sub(1).max(1)));
    // A NaN separation is no separation: written out rather than as a negated
    // comparison, which reads as the opposite of what it does.
    if sep.is_nan() || sep <= 0.5 {
        return;
    }
    let first = centre - (sep * (count(cluster.len()) - 1.0)) / 2.0;
    for (k, s) in cluster.iter().enumerate() {
        let (bound_lo, bound_hi) = free_span(s, boxes);
        let half = l::NUDGE_SEP / 2.0;
        move_run(
            s,
            routes,
            clamp(first + count(k) * sep, bound_lo + half, bound_hi - half),
        );
    }
}

/// Separate every run that shares a line with another, then tidy the routes.
pub fn nudge_overlaps(routes: &mut [Vec<Point>], boxes: &[Rect]) {
    let runs = nudgeable_runs(routes);
    for mut cluster in contended_clusters(&runs) {
        separate(&mut cluster, routes, boxes);
    }
    for route in routes.iter_mut() {
        *route = simplify(&dedupe(route));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Two routes crossing the same gutter, drawn on the same lane.
    fn merged_pair() -> Vec<Vec<Point>> {
        vec![
            vec![p(0.0, 0.0), p(0.0, 50.0), p(200.0, 50.0), p(200.0, 100.0)],
            vec![p(10.0, 0.0), p(10.0, 50.0), p(190.0, 50.0), p(190.0, 100.0)],
        ]
    }

    #[test]
    fn only_the_middle_legs_may_move() {
        let runs = nudgeable_runs(&merged_pair());
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().all(|r| r.i == 1 && r.axis == Axis::H));
        // A two-point route has no middle leg at all.
        assert!(nudgeable_runs(&[vec![p(0.0, 0.0), p(10.0, 0.0)]]).is_empty());
    }

    #[test]
    fn two_runs_on_one_line_are_pulled_apart() {
        let mut routes = merged_pair();
        nudge_overlaps(&mut routes, &[]);
        let ys: Vec<f64> = routes
            .iter()
            .filter_map(|r| r.get(1).map(|p| p.y))
            .collect();
        assert_eq!(ys.len(), 2);
        assert!(
            (ys[0] - ys[1]).abs() >= l::NUDGE_SEP - 1e-9,
            "still merged: {ys:?}"
        );
        // Centred on where they were, so neither route is dragged far.
        assert!((f64::midpoint(ys[0], ys[1]) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_lone_run_is_left_where_the_router_put_it() {
        let mut routes = vec![merged_pair().remove(0)];
        let before = routes.clone();
        nudge_overlaps(&mut routes, &[]);
        assert_eq!(routes, before);
    }

    #[test]
    fn runs_on_the_same_line_that_do_not_overlap_are_left_alone() {
        // Same y, but one spans 0..50 and the other 200..250.
        let mut routes = vec![
            vec![p(0.0, 0.0), p(0.0, 50.0), p(50.0, 50.0), p(50.0, 100.0)],
            vec![
                p(200.0, 0.0),
                p(200.0, 50.0),
                p(250.0, 50.0),
                p(250.0, 100.0),
            ],
        ];
        let before = routes.clone();
        nudge_overlaps(&mut routes, &[]);
        assert_eq!(routes, before);
    }

    #[test]
    fn separation_stops_at_the_boxes_either_side() {
        // A gutter only 20px tall: the runs may move, but not into a box.
        let boxes = [
            Rect::new(0.0, 0.0, 300.0, 40.0),
            Rect::new(0.0, 60.0, 300.0, 40.0),
        ];
        let mut routes = merged_pair();
        nudge_overlaps(&mut routes, &boxes);
        for route in &routes {
            for point in route.iter().skip(1).take(2) {
                assert!(
                    point.y > 40.0 && point.y < 60.0,
                    "{point:?} left the gutter"
                );
            }
        }
    }

    #[test]
    fn a_free_span_reports_the_boxes_above_and_below() {
        let seg = Run {
            route: 0,
            i: 1,
            axis: Axis::H,
            coord: 50.0,
            lo: 0.0,
            hi: 100.0,
            order: 0.0,
        };
        let (lo, hi) = free_span(
            &seg,
            &[
                Rect::new(0.0, 0.0, 100.0, 40.0),
                Rect::new(0.0, 60.0, 100.0, 40.0),
                // Beside the run rather than across it: irrelevant.
                Rect::new(200.0, 40.0, 50.0, 20.0),
            ],
        );
        assert!((lo - 40.0).abs() < 1e-9);
        assert!((hi - 60.0).abs() < 1e-9);
    }

    #[test]
    fn a_cluster_with_no_room_is_left_merged_rather_than_pushed_into_a_box() {
        // Boxes leave a 2px gutter: no separation is possible, and forcing one
        // would draw both edges through a box.
        let boxes = [
            Rect::new(0.0, 0.0, 300.0, 49.0),
            Rect::new(0.0, 51.0, 300.0, 40.0),
        ];
        let mut routes = merged_pair();
        let before = routes.clone();
        nudge_overlaps(&mut routes, &boxes);
        assert_eq!(routes, before);
    }

    #[test]
    fn a_run_naming_a_route_that_is_not_there_moves_nothing() {
        let seg = Run {
            route: 9,
            i: 1,
            axis: Axis::V,
            coord: 0.0,
            lo: 0.0,
            hi: 1.0,
            order: 0.0,
        };
        let mut routes: Vec<Vec<Point>> = Vec::new();
        move_run(&seg, &mut routes, 5.0);
        assert!(routes.is_empty());
        // ... and neither does one naming a leg past the end of its route.
        let mut short = vec![vec![p(0.0, 0.0), p(1.0, 0.0)]];
        move_run(
            &Run {
                route: 0,
                i: 5,
                ..seg
            },
            &mut short,
            5.0,
        );
        assert_eq!(short, vec![vec![p(0.0, 0.0), p(1.0, 0.0)]]);
    }

    #[test]
    fn runs_on_lines_far_apart_are_separate_clusters() {
        // Three pairs: two sharing y=50, two sharing y=400. The gap between the
        // lines is what splits them, and each pair is separated on its own.
        let mut routes = merged_pair();
        routes.extend(vec![
            vec![
                p(0.0, 350.0),
                p(0.0, 400.0),
                p(200.0, 400.0),
                p(200.0, 450.0),
            ],
            vec![
                p(10.0, 350.0),
                p(10.0, 400.0),
                p(190.0, 400.0),
                p(190.0, 450.0),
            ],
        ]);
        nudge_overlaps(&mut routes, &[]);
        let ys: Vec<f64> = routes
            .iter()
            .filter_map(|r| r.get(1).map(|p| p.y))
            .collect();
        assert!((ys[0] - ys[1]).abs() >= l::NUDGE_SEP - 1e-9, "{ys:?}");
        assert!((ys[2] - ys[3]).abs() >= l::NUDGE_SEP - 1e-9, "{ys:?}");
        // Each pair stayed on its own line rather than merging into one cluster.
        assert!((f64::midpoint(ys[0], ys[1]) - 50.0).abs() < 1e-9, "{ys:?}");
        assert!((f64::midpoint(ys[2], ys[3]) - 400.0).abs() < 1e-9, "{ys:?}");
    }

    #[test]
    fn vertical_runs_separate_too() {
        let mut routes = vec![
            vec![p(0.0, 0.0), p(50.0, 0.0), p(50.0, 200.0), p(100.0, 200.0)],
            vec![p(0.0, 10.0), p(50.0, 10.0), p(50.0, 190.0), p(100.0, 190.0)],
        ];
        nudge_overlaps(&mut routes, &[]);
        let xs: Vec<f64> = routes
            .iter()
            .filter_map(|r| r.get(1).map(|p| p.x))
            .collect();
        assert!((xs[0] - xs[1]).abs() >= l::NUDGE_SEP - 1e-9, "{xs:?}");
    }
}
