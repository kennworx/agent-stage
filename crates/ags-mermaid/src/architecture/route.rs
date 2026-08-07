//! Running the lines between placed boxes.
//!
//! Every edge leaves a named side and arrives at one, so there is nothing to
//! search for: a short stub out of each side, then one turn or two to join
//! them. What does need care is a side several edges share — without spreading
//! them along it they leave from one point and read as a single line.

use std::cmp::Ordering;

use crate::scene::Point;

use super::layout::PlacedEdge;
use super::layout::PlacedItem;
use super::types::{Diagram, Kind, Side};

/// How far a line runs straight out of the side it leaves before it turns.
const STUB: f64 = 14.0;
/// Coordinates closer than this are the same coordinate.
const EPSILON: f64 = 0.01;

/// Which side of `from` faces `to`.
fn facing(from: &PlacedItem, to: &PlacedItem) -> Side {
    let (here, there) = (from.centre(), to.centre());
    let (dx, dy) = (there.x - here.x, there.y - here.y);
    if dx.abs() >= dy.abs() {
        return if dx < 0.0 { Side::Left } else { Side::Right };
    }
    if dy < 0.0 {
        Side::Top
    } else {
        Side::Bottom
    }
}

/// Where on a side the `at`th of `count` lines meets it.
///
/// Evenly spread, so two lines leaving the same side arrive at different
/// points and a reader can tell there are two.
fn anchor(item: &PlacedItem, side: Side, at: usize, count: usize) -> Point {
    let share = (crate::layout::as_f64(at) + 1.0) / (crate::layout::as_f64(count.max(1)) + 1.0);
    match side {
        Side::Left => Point::new(item.at.x, item.at.y + item.height * share),
        Side::Right => Point::new(item.at.x + item.width, item.at.y + item.height * share),
        Side::Top => Point::new(item.at.x + item.width * share, item.at.y),
        Side::Bottom => Point::new(item.at.x + item.width * share, item.at.y + item.height),
    }
}

/// A point `distance` out from `at`, away from the box.
fn out(at: Point, side: Side, distance: f64) -> Point {
    match side {
        Side::Left => Point::new(at.x - distance, at.y),
        Side::Right => Point::new(at.x + distance, at.y),
        Side::Top => Point::new(at.x, at.y - distance),
        Side::Bottom => Point::new(at.x, at.y + distance),
    }
}

/// Drop repeated points, and corners that are not corners.
fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        let same = out.last().is_some_and(|last| {
            (last.x - point.x).abs() < EPSILON && (last.y - point.y).abs() < EPSILON
        });
        if !same {
            out.push(point);
        }
    }
    let mut at = 1;
    while at + 1 < out.len() {
        let (Some(before), Some(here), Some(after)) =
            (out.get(at - 1), out.get(at), out.get(at + 1))
        else {
            break;
        };
        let level = (before.y - here.y).abs() < EPSILON && (here.y - after.y).abs() < EPSILON;
        let upright = (before.x - here.x).abs() < EPSILON && (here.x - after.x).abs() < EPSILON;
        if level || upright {
            out.remove(at);
        } else {
            at += 1;
        }
    }
    out
}

/// The orthogonal run from one anchor to the other, turning at `share`.
///
/// Which turns it takes depends only on whether each end leaves sideways or
/// up-and-down: two sideways ends meet at a shared column, two upright ends at
/// a shared row, and one of each turns exactly once and has no choice about
/// where.
fn bend(start: Point, from: Side, end: Point, to: Side, share: f64) -> Vec<Point> {
    let a = out(start, from, STUB);
    let b = out(end, to, STUB);
    let mut points = vec![start, a];
    match (from.across(), to.across()) {
        (true, true) => {
            points.push(Point::new(share, a.y));
            points.push(Point::new(share, b.y));
        }
        (true, false) => points.push(Point::new(b.x, a.y)),
        (false, true) => points.push(Point::new(a.x, b.y)),
        (false, false) => {
            points.push(Point::new(a.x, share));
            points.push(Point::new(b.x, share));
        }
    }
    points.push(b);
    points.push(end);
    simplify(points)
}

/// Where a run would turn if nothing were in the way: half way across.
fn between(start: Point, from: Side, end: Point, to: Side) -> f64 {
    let a = out(start, from, STUB);
    let b = out(end, to, STUB);
    if from.across() {
        f64::midpoint(a.x, b.x)
    } else {
        f64::midpoint(a.y, b.y)
    }
}

/// The orthogonal run from one anchor to the other.
pub fn path(start: Point, from: Side, end: Point, to: Side) -> Vec<Point> {
    bend(start, from, end, to, between(start, from, end, to))
}

/// Whether a run passes through a box.
///
/// Inset a little, so a run that ends on a box's edge is not counted as going
/// through it.
fn overlaps(a: Point, b: Point, item: &PlacedItem) -> bool {
    const INSET: f64 = 0.5;
    a.x.min(b.x) < item.at.x + item.width - INSET
        && a.x.max(b.x) > item.at.x + INSET
        && a.y.min(b.y) < item.at.y + item.height - INSET
        && a.y.max(b.y) > item.at.y + INSET
}

/// Whether a whole run misses everything it is meant to.
fn clear(points: &[Point], obstacles: &[&PlacedItem]) -> bool {
    !points
        .windows(2)
        .filter_map(|pair| Some((*pair.first()?, *pair.get(1)?)))
        .any(|(a, b)| obstacles.iter().any(|item| overlaps(a, b, item)))
}

/// The run that misses the boxes it is not meant to touch.
///
/// Half way across is where a run turns when it can. When something is in the
/// way, turning late — just before the far end — or early gets it round, and
/// only when all three are blocked does it go through anyway: a line drawn over
/// a box is worse than none, but a line not drawn at all is worse than both.
pub fn around(
    start: Point,
    from: Side,
    end: Point,
    to: Side,
    obstacles: &[&PlacedItem],
) -> Vec<Point> {
    let middle = between(start, from, end, to);
    let a = out(start, from, STUB);
    let b = out(end, to, STUB);
    let (late, early) = if from.across() {
        (b.x, a.x)
    } else {
        (b.y, a.y)
    };
    for share in [middle, late, early] {
        let points = bend(start, from, end, to, share);
        if clear(&points, obstacles) {
            return points;
        }
    }
    bend(start, from, end, to, middle)
}

/// Which box and which side each end of each edge lands on.
///
/// `None` for an edge naming something nobody declared, which is dropped rather
/// than drawn to nowhere.
fn ends(diagram: &Diagram, items: &[PlacedItem]) -> Vec<Option<(usize, Side, usize, Side)>> {
    diagram
        .edges
        .iter()
        .map(|edge| {
            let from = diagram.index_of(&edge.from)?;
            let to = diagram.index_of(&edge.to)?;
            let (here, there) = (items.get(from)?, items.get(to)?);
            Some((
                from,
                edge.from_side.unwrap_or_else(|| facing(here, there)),
                to,
                edge.to_side.unwrap_or_else(|| facing(there, here)),
            ))
        })
        .collect()
}

/// How many lines meet each side, and which of them each end is.
///
/// Sides are met in edge order and ranked within a side by where the far end
/// sits, so the same source always spreads them the same way.
fn shares(
    ends: &[Option<(usize, Side, usize, Side)>],
    items: &[PlacedItem],
) -> (Vec<(usize, Side, usize)>, Vec<usize>) {
    let landings = landings(ends);
    let mut totals: Vec<(usize, Side, usize)> = Vec::new();
    // A dropped edge still takes its two places, or every end after it would
    // read the wrong share of its side.
    let mut order = vec![0_usize; ends.len() * 2];
    for (at, side) in sides(&landings) {
        let mut meeting: Vec<(usize, usize)> = landings
            .iter()
            .filter(|(_, item, held, _)| *item == at && *held == side)
            .map(|(slot, _, _, other)| (*slot, *other))
            .collect();
        // Along the side in the order the far ends sit across it. Handing the
        // anchors out in declaration order instead lets a line coming from
        // below take the top one, and the two cross on their way in.
        meeting.sort_by(|a, b| {
            let (here, there) = (crossways(items, side, a.1), crossways(items, side, b.1));
            here.partial_cmp(&there)
                .unwrap_or(Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });
        totals.push((at, side, meeting.len()));
        for (rank, (slot, _)) in meeting.iter().enumerate() {
            if let Some(place) = order.get_mut(*slot) {
                *place = rank;
            }
        }
    }
    (totals, order)
}

/// Every end that lands on a box: where it sits in `order`, the box and side it
/// lands on, and the box at the other end of the line.
fn landings(ends: &[Option<(usize, Side, usize, Side)>]) -> Vec<(usize, usize, Side, usize)> {
    let mut out = Vec::new();
    for (index, end) in ends.iter().enumerate() {
        let Some((from, from_side, to, to_side)) = end else {
            continue;
        };
        out.push((index * 2, *from, *from_side, *to));
        out.push((index * 2 + 1, *to, *to_side, *from));
    }
    out
}

/// Each side that carries a line, in the order they were first met.
fn sides(landings: &[(usize, usize, Side, usize)]) -> Vec<(usize, Side)> {
    let mut out: Vec<(usize, Side)> = Vec::new();
    for (_, item, side, _) in landings {
        if !out.contains(&(*item, *side)) {
            out.push((*item, *side));
        }
    }
    out
}

/// Where a box sits along the axis that runs across `side`.
///
/// The axis anchors are spread along: down a side that faces sideways, across
/// one that faces up or down.
fn crossways(items: &[PlacedItem], side: Side, at: usize) -> f64 {
    let Some(item) = items.get(at) else {
        return 0.0;
    };
    let centre = item.centre();
    if side.across() {
        centre.y
    } else {
        centre.x
    }
}

/// How many lines a side carries in total.
fn total(seen: &[(usize, Side, usize)], at: usize, side: Side) -> usize {
    seen.iter()
        .find(|(item, held, _)| *item == at && *held == side)
        .map_or(1, |(_, _, count)| *count)
}

/// Route every edge of a placed diagram.
pub fn routes(diagram: &Diagram, items: &[PlacedItem]) -> Vec<PlacedEdge> {
    let landed = ends(diagram, items);
    let (seen, order) = shares(&landed, items);
    diagram
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let points = landed
                .get(index)
                .copied()
                .flatten()
                .and_then(|(from, from_side, to, to_side)| {
                    let (here, there) = (items.get(from)?, items.get(to)?);
                    let start = anchor(
                        here,
                        from_side,
                        order.get(index * 2).copied().unwrap_or(0),
                        total(&seen, from, from_side),
                    );
                    let finish = anchor(
                        there,
                        to_side,
                        order.get(index * 2 + 1).copied().unwrap_or(0),
                        total(&seen, to, to_side),
                    );
                    // Everything solid that this line is not meant to touch.
                    let obstacles: Vec<&PlacedItem> = items
                        .iter()
                        .enumerate()
                        .filter(|(index, item)| {
                            *index != from && *index != to && item.kind != Kind::Group
                        })
                        .map(|(_, item)| item)
                        .collect();
                    Some(around(start, from_side, finish, to_side, &obstacles))
                })
                .unwrap_or_default();
            PlacedEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                arrow_start: edge.arrow_start,
                arrow_end: edge.arrow_end,
                points,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture::parse;

    fn box_at(x: f64, y: f64) -> PlacedItem {
        PlacedItem {
            id: "a".into(),
            kind: Kind::Service,
            icon: String::new(),
            title: "A".into(),
            at: Point::new(x, y),
            width: 100.0,
            height: 80.0,
            depth: 0,
        }
    }

    #[test]
    fn a_box_faces_whichever_way_the_other_one_lies() {
        let here = box_at(0.0, 0.0);
        assert_eq!(facing(&here, &box_at(300.0, 0.0)), Side::Right);
        assert_eq!(facing(&here, &box_at(-300.0, 0.0)), Side::Left);
        assert_eq!(facing(&here, &box_at(0.0, 300.0)), Side::Bottom);
        assert_eq!(facing(&here, &box_at(0.0, -300.0)), Side::Top);
        // Dead level and dead on: sideways wins, so a pair side by side never
        // routes over its own boxes.
        assert_eq!(facing(&here, &here.clone()), Side::Right);
    }

    #[test]
    fn one_line_meets_the_middle_of_its_side() {
        let item = box_at(0.0, 0.0);
        assert_eq!(anchor(&item, Side::Right, 0, 1), Point::new(100.0, 40.0));
        assert_eq!(anchor(&item, Side::Left, 0, 1), Point::new(0.0, 40.0));
        assert_eq!(anchor(&item, Side::Top, 0, 1), Point::new(50.0, 0.0));
        assert_eq!(anchor(&item, Side::Bottom, 0, 1), Point::new(50.0, 80.0));
    }

    #[test]
    fn several_lines_on_one_side_are_spread_along_it() {
        let item = box_at(0.0, 0.0);
        let first = anchor(&item, Side::Right, 0, 3);
        let second = anchor(&item, Side::Right, 1, 3);
        let third = anchor(&item, Side::Right, 2, 3);
        assert!(first.y < second.y && second.y < third.y);
        // All on the side, none past its ends.
        for point in [first, second, third] {
            assert!((point.x - 100.0).abs() < 1e-9);
            assert!(point.y > 0.0 && point.y < 80.0);
        }
    }

    #[test]
    fn a_stub_runs_out_of_the_side_it_leaves() {
        let at = Point::new(10.0, 20.0);
        assert_eq!(out(at, Side::Right, 5.0), Point::new(15.0, 20.0));
        assert_eq!(out(at, Side::Left, 5.0), Point::new(5.0, 20.0));
        assert_eq!(out(at, Side::Top, 5.0), Point::new(10.0, 15.0));
        assert_eq!(out(at, Side::Bottom, 5.0), Point::new(10.0, 25.0));
    }

    #[test]
    fn a_straight_run_is_two_points_however_many_it_was_built_from() {
        let points = path(
            Point::new(0.0, 50.0),
            Side::Right,
            Point::new(200.0, 50.0),
            Side::Left,
        );
        assert_eq!(points, [Point::new(0.0, 50.0), Point::new(200.0, 50.0)]);
    }

    #[test]
    fn two_sideways_ends_meet_at_a_shared_column() {
        let points = path(
            Point::new(0.0, 0.0),
            Side::Right,
            Point::new(200.0, 100.0),
            Side::Left,
        );
        assert_eq!(points.len(), 4);
        // Every run is level or upright, and the two turns share a column.
        for pair in points.windows(2) {
            let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            assert!(
                (a.x - b.x).abs() < EPSILON || (a.y - b.y).abs() < EPSILON,
                "a run runs at an angle"
            );
        }
        assert!(
            (points.get(1).map_or(0.0, |p| p.x) - points.get(2).map_or(1.0, |p| p.x)).abs()
                < EPSILON
        );
    }

    #[test]
    fn one_end_of_each_kind_turns_exactly_once() {
        let points = path(
            Point::new(0.0, 0.0),
            Side::Right,
            Point::new(200.0, 100.0),
            Side::Top,
        );
        assert_eq!(points.len(), 3);
        assert_eq!(points.get(1), Some(&Point::new(200.0, 0.0)));
    }

    #[test]
    fn two_upright_ends_meet_at_a_shared_row() {
        let points = path(
            Point::new(0.0, 0.0),
            Side::Bottom,
            Point::new(100.0, 200.0),
            Side::Top,
        );
        assert_eq!(points.len(), 4);
        assert!(
            (points.get(1).map_or(0.0, |p| p.y) - points.get(2).map_or(1.0, |p| p.y)).abs()
                < EPSILON
        );
    }

    #[test]
    fn a_run_with_nothing_in_it_simplifies_to_nothing() {
        assert!(simplify(Vec::new()).is_empty());
        let one = simplify(vec![Point::new(1.0, 1.0), Point::new(1.0, 1.0)]);
        assert_eq!(one, [Point::new(1.0, 1.0)]);
    }

    #[test]
    fn every_edge_of_a_read_diagram_is_routed() {
        let diagram = parse(
            "architecture-beta\n  service web(server)[Web]\n  service db(database)[DB]\n  web:R -- L:db",
        );
        let items = super::super::layout::boxes(&diagram);
        let routed = routes(&diagram, &items);
        assert_eq!(routed.len(), 1);
        let first = routed.first().expect("a line");
        assert!(first.points.len() >= 2);
        // It starts on one box and ends on the other.
        let web = items.first().expect("a box");
        assert!((first.points.first().map_or(0.0, |p| p.x) - (web.at.x + web.width)).abs() < 1e-9);
    }

    #[test]
    fn an_edge_naming_something_nobody_declared_is_not_drawn() {
        let diagram = parse("architecture-beta\n  service web(server)[Web]\n  web:R -- L:nowhere");
        let items = super::super::layout::boxes(&diagram);
        let routed = routes(&diagram, &items);
        assert_eq!(routed.len(), 1);
        assert!(routed.first().expect("a line").points.is_empty());
    }

    #[test]
    fn an_edge_with_no_sides_written_leaves_the_side_that_faces() {
        let diagram =
            parse("architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  a -- b");
        let items = super::super::layout::boxes(&diagram);
        let routed = routes(&diagram, &items);
        let points = &routed.first().expect("a line").points;
        let (a, b) = (items.first().expect("a"), items.get(1).expect("b"));
        // b is placed to the right, so the line leaves a's right side.
        assert!((points.first().map_or(0.0, |p| p.x) - (a.at.x + a.width)).abs() < 1e-9);
        assert!((points.last().map_or(0.0, |p| p.x) - b.at.x).abs() < 1e-9);
    }

    #[test]
    fn two_lines_meeting_one_side_do_not_cross_on_their_way_in() {
        // Both workers feed the warehouse. Worker 2 sits below Worker 1, so its
        // line has to take the lower anchor however the two were declared —
        // and here the lower one is declared first.
        let diagram = parse(
            "architecture-beta\n  service w1(cpu)[W1]\n  service w2(cpu)[W2]\n  service store(disk)[Store]\n  w1:B -- T:w2\n  w2:R -- L:store\n  w1:R -- L:store",
        );
        let items = super::super::layout::boxes(&diagram);
        let routed = routes(&diagram, &items);
        let arrives = |at: usize| {
            routed
                .get(at)
                .and_then(|edge| edge.points.last())
                .copied()
                .expect("a line")
        };
        let (upper, lower) = (items.first().expect("w1"), items.get(1).expect("w2"));
        assert!(lower.at.y > upper.at.y, "w2 is the lower of the two");
        // Edge 1 is w2's, edge 2 is w1's: the one from below arrives lower.
        assert!(
            arrives(1).y > arrives(2).y,
            "the lines cross on their way in"
        );
    }

    #[test]
    fn two_lines_leaving_one_side_do_not_leave_from_one_point() {
        let diagram = parse(
            "architecture-beta\n  service j(server)[J]\n  service a(server)[A]\n  service b(server)[B]\n  j:R -- L:a\n  j:R -- L:b",
        );
        let items = super::super::layout::boxes(&diagram);
        let routed = routes(&diagram, &items);
        let first = routed.first().expect("a line").points.first().copied();
        let second = routed.get(1).expect("a line").points.first().copied();
        assert_ne!(first, second);
    }

    #[test]
    fn a_run_is_only_through_a_box_when_it_is_inside_it_both_ways() {
        // The box spans 0..100 across and 0..80 down.
        let item = box_at(0.0, 0.0);
        // Straight through the middle.
        assert!(overlaps(
            Point::new(-50.0, 40.0),
            Point::new(150.0, 40.0),
            &item
        ));
        // Past each edge in turn: left, right, above, below.
        assert!(!overlaps(
            Point::new(-50.0, 40.0),
            Point::new(-10.0, 40.0),
            &item
        ));
        assert!(!overlaps(
            Point::new(150.0, 40.0),
            Point::new(200.0, 40.0),
            &item
        ));
        assert!(!overlaps(
            Point::new(-50.0, -10.0),
            Point::new(150.0, -10.0),
            &item
        ));
        assert!(!overlaps(
            Point::new(-50.0, 100.0),
            Point::new(150.0, 100.0),
            &item
        ));
        // And a run that ends on the edge is not through it.
        assert!(!overlaps(
            Point::new(-50.0, 40.0),
            Point::new(0.0, 40.0),
            &item
        ));
    }

    #[test]
    fn a_run_turns_wherever_it_has_to_in_order_to_miss_a_box() {
        // Half way across is x=150, and the box sits over it. Turning late,
        // just before the far end, takes the run round.
        let between = box_at(130.0, 60.0);
        let obstacles = [&between];
        let start = Point::new(0.0, 0.0);
        let end = Point::new(300.0, 100.0);
        let straight = path(start, Side::Right, end, Side::Left);
        assert!(!clear(&straight, &obstacles), "the box is not in the way");
        let round = around(start, Side::Right, end, Side::Left, &obstacles);
        assert!(clear(&round, &obstacles), "the run still goes through");
        assert_ne!(round, straight);
    }

    #[test]
    fn a_run_with_nowhere_clear_to_turn_is_still_drawn() {
        // A wall across the whole gap: every column is blocked, and so is every
        // row the run could travel along. A line over a box beats no line.
        let mut wall = box_at(10.0, -100.0);
        wall.width = 280.0;
        wall.height = 300.0;
        let obstacles = [&wall];
        let points = around(
            Point::new(0.0, 0.0),
            Side::Right,
            Point::new(300.0, 0.0),
            Side::Left,
            &obstacles,
        );
        assert!(points.len() >= 2);
        assert!(!clear(&points, &obstacles));
    }

    #[test]
    fn a_side_is_ranked_across_the_axis_it_faces() {
        let items = [box_at(0.0, 0.0), box_at(300.0, 200.0)];
        // A sideways-facing side spreads its lines down; an upright one spreads
        // them across.
        assert!((crossways(&items, Side::Left, 1) - 240.0).abs() < 1e-9);
        assert!((crossways(&items, Side::Top, 1) - 350.0).abs() < 1e-9);
        // And a box nobody placed ranks at nought rather than panicking.
        assert!((crossways(&items, Side::Left, 9) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn an_end_that_lands_nowhere_still_takes_its_turn_in_the_count() {
        // The dropped edge has to consume its two slots, or every end after it
        // reads the wrong share of its side.
        let ends = [None, Some((0, Side::Right, 1, Side::Left))];
        let (seen, order) = shares(&ends, &[box_at(0.0, 0.0), box_at(300.0, 0.0)]);
        assert_eq!(order.len(), 4);
        assert_eq!(total(&seen, 0, Side::Right), 1);
        assert_eq!(total(&seen, 9, Side::Top), 1);
    }
}
