//! The routing lattice, and the shortest path across it.
//!
//! Blocks sit on a grid, so the bands between them — and the margins around them
//! — are guaranteed free of boxes. Giving each band three lanes lets parallel
//! edges share a gutter without drawing on top of one another, and routing on the
//! lattice means a path is clear by construction rather than by trial.
//!
//! This is the standard orthogonal-routing shape (visibility graph, search,
//! nudging) with the visibility graph replaced by a fixed lattice: the boxes are
//! already on a grid, so the lanes that a visibility graph would discover are
//! known in advance.

use std::collections::{BinaryHeap, HashMap, HashSet};

use super::config as l;
use super::geom::{crossings, dedupe, fixed1, legs, Axis, Point, Rect, Side, SidePair, EPS};

/// Lane centres, keyed by the coordinate they are fixed at.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lattice {
    /// Vertical lanes, by x.
    pub vx: Vec<f64>,
    /// Horizontal lanes, by y.
    pub hy: Vec<f64>,
}

/// How busy each lane is, so parallel edges fan out instead of stacking.
///
/// Keyed by axis and quantised coordinate: `Axis::H` with a `y` is a horizontal
/// lane.
pub type LaneLoad = HashMap<(Axis, i64), f64>;

/// Which axis a route already travels through each point on, so a later route
/// can be charged for crossing it rather than joining it.
pub type Occupancy = HashMap<(i64, i64), HashSet<Axis>>;

/// Which way of `v` a lane has to lie.
///
/// Screen coordinates grow down and right, so "greater" is below on the vertical
/// axis and to the right on the horizontal one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Greater,
    Less,
}

/// Lane centres for one band, at 1/4, 1/2 and 3/4 of its width.
fn lanes_in(lo: f64, hi: f64) -> [f64; 3] {
    let span = hi - lo;
    [lo + span * 0.25, lo + span * 0.5, lo + span * 0.75]
}

/// The lanes between a set of spans, plus a band in each margin.
fn bands(spans: &[(f64, f64)], margin: f64) -> Vec<f64> {
    let mut sorted: Vec<(f64, f64)> = spans.to_vec();
    sorted.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut lanes: Vec<f64> = Vec::new();
    let mut reach = f64::NEG_INFINITY;
    let mut first = f64::INFINITY;
    let mut last = f64::NEG_INFINITY;
    for (lo, hi) in sorted {
        if reach > f64::NEG_INFINITY && lo > reach {
            lanes.extend(lanes_in(reach, lo));
        }
        reach = reach.max(hi);
        first = first.min(lo);
        last = last.max(hi);
    }
    // Margins outside the content are gutters too — an edge that cannot get
    // through the middle can still go round.
    lanes.extend(lanes_in(first - margin, first));
    lanes.extend(lanes_in(last, last + margin));
    lanes.sort_by(f64::total_cmp);
    lanes
}

/// Derive the lattice from the placed blocks.
pub fn build_lattice(elements: &[Rect], gap_x: f64, gap_y: f64) -> Lattice {
    if elements.is_empty() {
        return Lattice::default();
    }
    let xs: Vec<(f64, f64)> = elements.iter().map(|e| (e.x, e.right())).collect();
    let ys: Vec<(f64, f64)> = elements.iter().map(|e| (e.y, e.bottom())).collect();
    Lattice {
        vx: bands(&xs, gap_x),
        hy: bands(&ys, gap_y),
    }
}

/// Nearest usable lane to `v` on the given side.
///
/// Lanes closer than `min_stub` are skipped. An edge that joins the lattice a few
/// pixels off the box face has no straight run to carry its arrowhead: the corner
/// rounding starts immediately and the head sits in the curve, which reads as a
/// hook rather than an arrow meeting a box. Falls back to the nearest lane of any
/// distance when nothing satisfies the minimum, so a cramped diagram still routes.
pub fn nearest_lane_on(lanes: &[f64], v: f64, want: Want, min_stub: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    let mut fallback: Option<f64> = None;
    for &lane in lanes {
        match want {
            Want::Greater if lane <= v => continue,
            Want::Less if lane >= v => continue,
            _ => {}
        }
        if fallback.is_none_or(|f| (lane - v).abs() < (f - v).abs()) {
            fallback = Some(lane);
        }
        if (lane - v).abs() < min_stub {
            continue;
        }
        if best.is_none_or(|b| (lane - v).abs() < (b - v).abs()) {
            best = Some(lane);
        }
    }
    best.or(fallback)
}

/// Where an edge meets the lattice after leaving its box perpendicular.
pub fn approach(port: Point, side: Side, lat: &Lattice) -> Option<(Point, Axis)> {
    if side.is_horizontal_face() {
        let want = if side == Side::Bottom {
            Want::Greater
        } else {
            Want::Less
        };
        let lane = nearest_lane_on(&lat.hy, port.y, want, l::EDGE_STUB)?;
        return Some((Point::new(port.x, lane), Axis::H));
    }
    let want = if side == Side::Right {
        Want::Greater
    } else {
        Want::Less
    };
    let lane = nearest_lane_on(&lat.vx, port.x, want, l::EDGE_STUB)?;
    Some((Point::new(lane, port.y), Axis::V))
}

/// Indices of the lanes either side of `v` — the ones an approach can reach.
fn neighbour_idx(lanes: &[f64], v: f64) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, &lane) in lanes.iter().enumerate() {
        let below = lane <= v && lanes.get(i + 1).is_none_or(|&next| next > v);
        let above = lane >= v
            && (i == 0
                || i.checked_sub(1)
                    .and_then(|j| lanes.get(j))
                    .is_some_and(|&prev| prev < v));
        if (below || above) && !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

/// A lane's identity in the congestion map.
fn lane_key(axis: Axis, coord: f64) -> (Axis, i64) {
    (axis, fixed1(coord))
}

/// A lattice point's identity in the occupancy map.
fn point_key(p: Point) -> (i64, i64) {
    (fixed1(p.x), fixed1(p.y))
}

/// Charge the lanes a finished route used, so the next edge prefers others.
pub fn charge_lanes(points: &[Point], load: &mut LaneLoad) {
    for (a, b) in legs(points) {
        let key = if (a.x - b.x).abs() < EPS {
            lane_key(Axis::V, a.x)
        } else {
            lane_key(Axis::H, a.y)
        };
        *load.entry(key).or_insert(0.0) += 1.0;
    }
}

/// Record which axis a finished route travels through each point on.
pub fn mark_occupied(points: &[Point], occupied: &mut Occupancy) {
    for (a, b) in legs(points) {
        let axis = if (a.y - b.y).abs() < EPS {
            Axis::H
        } else {
            Axis::V
        };
        for p in [a, b] {
            occupied.entry(point_key(p)).or_default().insert(axis);
        }
    }
}

/// Which lane a coordinate *is*, not which lane is nearest.
///
/// Compared exactly on purpose: an approach point is built by copying a lane
/// value out of the lattice, so anything that is not bit-identical to one of them
/// is not on the lattice at all and has no index to find.
#[expect(
    clippy::float_cmp,
    reason = "the value was copied from this very list, so equality is identity"
)]
fn lane_index(lanes: &[f64], v: f64) -> Option<usize> {
    lanes.iter().position(|&lane| lane == v)
}

/// One directed lattice step.
#[derive(Debug, Clone, Copy)]
struct Link {
    to: usize,
    cost: f64,
    axis: Axis,
}

/// A search state, ordered so the cheapest pops first and ties break by the
/// order they were pushed.
///
/// The reference re-sorted an array and shifted the front; `Array.prototype.sort`
/// is stable, so equal-cost states came out in push order. Reproducing that
/// exactly is what keeps two implementations of this router drawing the same
/// picture — with several lattice paths tied on cost, the tie-break *is* the
/// route.
#[derive(Debug, Clone, Copy)]
struct State {
    d: f64,
    seq: usize,
    node: usize,
    axis: Axis,
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.d == other.d && self.seq == other.seq
    }
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reversed: `BinaryHeap` pops the greatest, and we want the cheapest.
        other
            .d
            .total_cmp(&self.d)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Dijkstra over (node, incoming axis), charging for direction changes.
fn shortest_path(
    nodes: &[Point],
    adj: &[Vec<Link>],
    from: usize,
    to: usize,
    occupied: &Occupancy,
) -> Option<Vec<usize>> {
    let mut dist: HashMap<(usize, Axis), f64> = HashMap::new();
    let mut prev: HashMap<(usize, Axis), Option<(usize, Axis)>> = HashMap::new();
    let mut queue: BinaryHeap<State> = BinaryHeap::new();
    let mut seq = 0;
    for axis in [Axis::H, Axis::V] {
        dist.insert((from, axis), 0.0);
        prev.insert((from, axis), None);
        queue.push(State {
            d: 0.0,
            seq,
            node: from,
            axis,
        });
        seq += 1;
    }
    while let Some(cur) = queue.pop() {
        if cur.d
            > dist
                .get(&(cur.node, cur.axis))
                .copied()
                .unwrap_or(f64::INFINITY)
        {
            continue;
        }
        if cur.node == to {
            let mut path: Vec<usize> = Vec::new();
            let mut step = Some((cur.node, cur.axis));
            while let Some(state) = step {
                path.insert(0, state.0);
                step = prev.get(&state).copied().flatten();
            }
            return Some(path);
        }
        for link in adj.get(cur.node).into_iter().flatten() {
            let turn = if link.axis == cur.axis {
                0.0
            } else {
                l::TURN_PENALTY
            };
            // Arriving at a point another route already passes through on the
            // other axis is a crossing. Charging for it steers edges onto lanes
            // busy in the *same* direction, where they run alongside rather than
            // over.
            let crossing = nodes
                .get(link.to)
                .and_then(|p| occupied.get(&point_key(*p)))
                .is_some_and(|axes| axes.contains(&link.axis.other()));
            let nd = cur.d + link.cost + turn + if crossing { l::CROSS_PENALTY } else { 0.0 };
            let key = (link.to, link.axis);
            if nd < dist.get(&key).copied().unwrap_or(f64::INFINITY) {
                dist.insert(key, nd);
                prev.insert(key, Some((cur.node, cur.axis)));
                queue.push(State {
                    d: nd,
                    seq,
                    node: link.to,
                    axis: link.axis,
                });
                seq += 1;
            }
        }
    }
    None
}

/// The lattice as a graph: a node per lane intersection, plus the two approaches.
struct Graph {
    nodes: Vec<Point>,
    adj: Vec<Vec<Link>>,
}

impl Graph {
    fn link(&mut self, a: usize, b: usize, axis: Axis, load: &LaneLoad, end: Point) {
        let (Some(&pa), Some(&pb)) = (self.nodes.get(a), self.nodes.get(b)) else {
            return;
        };
        let len = (pa.x - pb.x).abs() + (pa.y - pb.y).abs();
        let key = match axis {
            Axis::V => lane_key(Axis::V, pa.x),
            Axis::H => lane_key(Axis::H, pa.y),
        };
        let congestion = load.get(&key).copied().unwrap_or(0.0) * l::LANE_CONGESTION;
        // Distance still to run once a step is taken. A step that increases it is
        // the route doubling back, and pays `AWAY_FACTOR` times its length.
        let remaining = |p: Point| (p.x - end.x).abs() + (p.y - end.y).abs();
        let away = remaining(pb) > remaining(pa);
        let forward = len * if away { l::AWAY_FACTOR } else { 1.0 } + congestion;
        let backward = len * if away { 1.0 } else { l::AWAY_FACTOR } + congestion;
        if let Some(edges) = self.adj.get_mut(a) {
            edges.push(Link {
                to: b,
                cost: forward,
                axis,
            });
        }
        if let Some(edges) = self.adj.get_mut(b) {
            edges.push(Link {
                to: a,
                cost: backward,
                axis,
            });
        }
    }
}

/// Build the lattice graph and attach the two approach points to it.
fn build_graph(
    lat: &Lattice,
    entry: (Point, Axis),
    exit: (Point, Axis),
    load: &LaneLoad,
    end: Point,
) -> (Graph, usize, usize) {
    let key = |i: usize, j: usize| i * lat.hy.len() + j;
    let mut nodes = vec![Point::new(0.0, 0.0); lat.vx.len() * lat.hy.len()];
    for (i, &x) in lat.vx.iter().enumerate() {
        for (j, &y) in lat.hy.iter().enumerate() {
            if let Some(slot) = nodes.get_mut(key(i, j)) {
                *slot = Point::new(x, y);
            }
        }
    }
    let entry_id = nodes.len();
    nodes.push(entry.0);
    let exit_id = nodes.len();
    nodes.push(exit.0);

    let count = nodes.len();
    let mut graph = Graph {
        nodes,
        adj: vec![Vec::new(); count],
    };
    for i in 0..lat.vx.len() {
        for j in 0..lat.hy.len() {
            if i + 1 < lat.vx.len() {
                graph.link(key(i, j), key(i + 1, j), Axis::H, load, end);
            }
            if j + 1 < lat.hy.len() {
                graph.link(key(i, j), key(i, j + 1), Axis::V, load, end);
            }
        }
    }
    // Approach points join the lattice along their own lane.
    for (id, (point, axis)) in [(entry_id, entry), (exit_id, exit)] {
        if axis == Axis::V {
            if let Some(i) = lane_index(&lat.vx, point.x) {
                for j in neighbour_idx(&lat.hy, point.y) {
                    graph.link(id, key(i, j), Axis::V, load, end);
                }
            }
        } else if let Some(j) = lane_index(&lat.hy, point.y) {
            for i in neighbour_idx(&lat.vx, point.x) {
                graph.link(id, key(i, j), Axis::H, load, end);
            }
        }
    }
    (graph, entry_id, exit_id)
}

/// The short route between two boxes that face each other: out of both, along one
/// shared lane, in.
///
/// Returns `None` when no such lane is clear, leaving the caller to fall back to
/// a full lattice search. This exists because the lattice can only turn at lane
/// *intersections*, so the obvious hop between two boxes that face each other is
/// inexpressible on it — the path has to slide sideways to the nearest crossing
/// and back, which reads as a detour for what should be a straight line.
fn direct_route(
    start: Point,
    end: Point,
    sides: SidePair,
    lat: &Lattice,
    load: &LaneLoad,
    obstacles: &[Rect],
) -> Option<Vec<Point>> {
    if !sides.facing() {
        return None;
    }
    let vertical = sides.start.is_horizontal_face();
    let lanes = if vertical { &lat.hy } else { &lat.vx };
    let from = if vertical { start.y } else { start.x };
    let to = if vertical { end.y } else { end.x };
    let lo = from.min(to);
    let hi = from.max(to);

    let mid = f64::midpoint(from, to);
    let clear: Vec<f64> = lanes
        .iter()
        .copied()
        .filter(|&lane| lane > lo + l::EDGE_STUB && lane < hi - l::EDGE_STUB)
        .collect();
    let mut between: Vec<f64> = if clear.is_empty() {
        lanes
            .iter()
            .copied()
            .filter(|&lane| lane > lo && lane < hi)
            .collect()
    } else {
        clear
    };
    // Distance from the middle is weighted above congestion, so a lane only moves
    // off-centre when it is genuinely busy — a turn hard against a box edge is
    // more distracting than two edges sharing a lane.
    let rank = |v: f64| {
        let axis = if vertical { Axis::H } else { Axis::V };
        load.get(&lane_key(axis, v)).copied().unwrap_or(0.0) * l::LANE_CONGESTION
            + (v - mid).abs() * 3.0
    };
    between.sort_by(|a, b| rank(*a).total_cmp(&rank(*b)));

    between.into_iter().find_map(|lane| {
        let route = dedupe(&if vertical {
            [
                start,
                Point::new(start.x, lane),
                Point::new(end.x, lane),
                end,
            ]
        } else {
            [
                start,
                Point::new(lane, start.y),
                Point::new(lane, end.y),
                end,
            ]
        });
        (crossings(&route, obstacles) == 0).then_some(route)
    })
}

/// Route one edge across the lattice by shortest path.
///
/// The edge leaves its box perpendicular into the *adjacent* gutter — never
/// across a neighbour — joins the lane nearest that face, then takes the cheapest
/// lattice path to the target's adjacent gutter. Cost is distance plus a penalty
/// per turn, so routes stay straight where they can, plus a congestion charge on
/// lanes already carrying traffic, so parallel edges fan out across the three
/// lanes instead of stacking on the middle one.
pub fn route_on_lattice(
    start: Point,
    end: Point,
    sides: SidePair,
    lat: &Lattice,
    load: &LaneLoad,
    obstacles: &[Rect],
    occupied: &Occupancy,
) -> Vec<Point> {
    // Neighbours first: try the direct shapes and only fall back to the lattice
    // when none of them is clear.
    if let Some(direct) = direct_route(start, end, sides, lat, load, obstacles) {
        return direct;
    }
    let (Some(entry), Some(exit)) = (
        approach(start, sides.start, lat),
        approach(end, sides.end, lat),
    ) else {
        return dedupe(&[start, end]);
    };

    let (graph, entry_id, exit_id) = build_graph(lat, entry, exit, load, end);
    let Some(path) = shortest_path(&graph.nodes, &graph.adj, entry_id, exit_id, occupied) else {
        return dedupe(&[start, end]);
    };
    let mut points = vec![start];
    points.extend(
        path.into_iter()
            .filter_map(|id| graph.nodes.get(id).copied()),
    );
    points.push(end);
    dedupe(&points)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_boxes() -> Vec<Rect> {
        vec![
            Rect::new(100.0, 100.0, 100.0, 60.0),
            Rect::new(300.0, 100.0, 100.0, 60.0),
        ]
    }

    #[test]
    fn a_band_gets_three_lanes() {
        let lanes = lanes_in(0.0, 100.0);
        for (got, want) in lanes.into_iter().zip([25.0, 50.0, 75.0]) {
            assert!((got - want).abs() < 1e-9, "{lanes:?}");
        }
    }

    #[test]
    fn the_lattice_puts_lanes_in_the_gutter_and_the_margins() {
        let lat = build_lattice(&two_boxes(), 56.0, 68.0);
        // Between the boxes: the band from 200 to 300.
        assert!(lat.vx.contains(&250.0));
        // Margins on both sides, a full gap wide.
        assert!(lat.vx.iter().any(|&x| x < 100.0));
        assert!(lat.vx.iter().any(|&x| x > 400.0));
        // Sorted, so the direct route can scan them in order.
        assert!(lat.vx.windows(2).all(|w| w[0] <= w[1]));
        // Boxes share a row, so there is no horizontal gutter — only margins.
        assert_eq!(lat.hy.len(), 6);
    }

    #[test]
    fn an_empty_diagram_has_no_lattice() {
        assert_eq!(build_lattice(&[], 10.0, 10.0), Lattice::default());
    }

    #[test]
    fn the_nearest_lane_respects_the_stub_and_falls_back() {
        let lanes = [10.0, 30.0, 90.0];
        assert_eq!(
            nearest_lane_on(&lanes, 20.0, Want::Greater, 0.0),
            Some(30.0)
        );
        // 30 is inside the stub, so 90 wins.
        assert_eq!(
            nearest_lane_on(&lanes, 20.0, Want::Greater, 18.0),
            Some(90.0)
        );
        assert_eq!(nearest_lane_on(&lanes, 20.0, Want::Less, 0.0), Some(10.0));
        // Nothing satisfies the stub, so the nearest of any distance is used.
        assert_eq!(nearest_lane_on(&lanes, 20.0, Want::Less, 50.0), Some(10.0));
        assert_eq!(nearest_lane_on(&lanes, 200.0, Want::Greater, 0.0), None);
        assert_eq!(nearest_lane_on(&[], 5.0, Want::Less, 0.0), None);
    }

    #[test]
    fn an_approach_leaves_the_box_perpendicular() {
        let lat = Lattice {
            vx: vec![50.0, 250.0],
            hy: vec![50.0, 250.0],
        };
        assert_eq!(
            approach(Point::new(150.0, 100.0), Side::Top, &lat),
            Some((Point::new(150.0, 50.0), Axis::H))
        );
        assert_eq!(
            approach(Point::new(150.0, 100.0), Side::Bottom, &lat),
            Some((Point::new(150.0, 250.0), Axis::H))
        );
        assert_eq!(
            approach(Point::new(100.0, 150.0), Side::Left, &lat),
            Some((Point::new(50.0, 150.0), Axis::V))
        );
        assert_eq!(
            approach(Point::new(100.0, 150.0), Side::Right, &lat),
            Some((Point::new(250.0, 150.0), Axis::V))
        );
        assert_eq!(
            approach(Point::new(150.0, 100.0), Side::Top, &Lattice::default()),
            None
        );
    }

    #[test]
    fn neighbouring_lanes_are_the_ones_either_side() {
        assert_eq!(neighbour_idx(&[0.0, 10.0, 20.0], 15.0), vec![1, 2]);
        // Landing exactly on a lane reaches only that lane.
        assert_eq!(neighbour_idx(&[0.0, 10.0, 20.0], 10.0), vec![1]);
        assert_eq!(neighbour_idx(&[0.0, 10.0], -5.0), vec![0]);
        assert_eq!(neighbour_idx(&[0.0, 10.0], 50.0), vec![1]);
    }

    #[test]
    fn a_lane_records_the_traffic_it_carries() {
        let mut load = LaneLoad::new();
        charge_lanes(
            &[
                Point::new(0.0, 40.0),
                Point::new(100.0, 40.0),
                Point::new(100.0, 90.0),
            ],
            &mut load,
        );
        assert_eq!(load.get(&(Axis::H, 400)), Some(&1.0));
        assert_eq!(load.get(&(Axis::V, 1000)), Some(&1.0));
        charge_lanes(&[Point::new(0.0, 40.0), Point::new(50.0, 40.0)], &mut load);
        assert_eq!(load.get(&(Axis::H, 400)), Some(&2.0));
    }

    #[test]
    fn occupancy_records_the_axis_a_route_passes_through_a_point_on() {
        let mut occ = Occupancy::new();
        mark_occupied(&[Point::new(0.0, 0.0), Point::new(10.0, 0.0)], &mut occ);
        assert!(occ
            .get(&(100, 0))
            .is_some_and(|axes| axes.contains(&Axis::H)));
        // Only the two ends of a leg are marked, so a route merely passing
        // through must actually turn there to register.
        mark_occupied(&[Point::new(10.0, -5.0), Point::new(10.0, 0.0)], &mut occ);
        assert_eq!(occ.get(&(100, 0)).map(HashSet::len), Some(2));
        assert_eq!(occ.get(&(100, -50)).map(HashSet::len), Some(1));
    }

    #[test]
    fn facing_neighbours_take_the_direct_route() {
        let lat = build_lattice(&two_boxes(), 56.0, 68.0);
        let route = route_on_lattice(
            Point::new(200.0, 130.0),
            Point::new(300.0, 130.0),
            SidePair {
                start: Side::Right,
                end: Side::Left,
            },
            &lat,
            &LaneLoad::new(),
            &[],
            &Occupancy::new(),
        );
        // Out, across a lane, in — and with both ends level, the middle two
        // points collapse onto the line, leaving one straight run.
        assert_eq!(route.first(), Some(&Point::new(200.0, 130.0)));
        assert_eq!(route.last(), Some(&Point::new(300.0, 130.0)));
        assert!(route.iter().all(|p| (p.y - 130.0).abs() < 1e-9));
    }

    #[test]
    fn a_blocked_direct_route_falls_back_to_the_lattice() {
        let boxes = two_boxes();
        let lat = build_lattice(&boxes, 56.0, 68.0);
        // A box straddling the gutter blocks every lane between the two faces.
        let wall = [Rect::new(200.0, 60.0, 100.0, 200.0)];
        let route = route_on_lattice(
            Point::new(200.0, 130.0),
            Point::new(300.0, 130.0),
            SidePair {
                start: Side::Right,
                end: Side::Left,
            },
            &lat,
            &LaneLoad::new(),
            &wall,
            &Occupancy::new(),
        );
        assert!(route.len() > 2, "{route:?}");
        assert_eq!(route.first(), Some(&Point::new(200.0, 130.0)));
        assert_eq!(route.last(), Some(&Point::new(300.0, 130.0)));
    }

    #[test]
    fn a_route_with_no_lattice_to_join_is_the_bare_pair() {
        let route = route_on_lattice(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            SidePair {
                start: Side::Top,
                end: Side::Left,
            },
            &Lattice::default(),
            &LaneLoad::new(),
            &[],
            &Occupancy::new(),
        );
        assert_eq!(route, vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
    }

    #[test]
    fn a_route_between_boxes_on_different_rows_turns_once_it_is_clear() {
        let boxes = vec![
            Rect::new(100.0, 100.0, 100.0, 60.0),
            Rect::new(400.0, 400.0, 100.0, 60.0),
        ];
        let lat = build_lattice(&boxes, 56.0, 68.0);
        let route = route_on_lattice(
            Point::new(150.0, 160.0),
            Point::new(450.0, 400.0),
            SidePair {
                start: Side::Bottom,
                end: Side::Top,
            },
            &lat,
            &LaneLoad::new(),
            &[],
            &Occupancy::new(),
        );
        assert_eq!(route.first(), Some(&Point::new(150.0, 160.0)));
        assert_eq!(route.last(), Some(&Point::new(450.0, 400.0)));
        // Orthogonal throughout: every leg moves on exactly one axis.
        assert!(route
            .windows(2)
            .all(|w| (w[0].x - w[1].x).abs() < EPS || (w[0].y - w[1].y).abs() < EPS));
    }

    #[test]
    fn states_order_by_cost_then_by_the_order_they_were_queued() {
        let cheap = State {
            d: 1.0,
            seq: 9,
            node: 0,
            axis: Axis::H,
        };
        let dear = State {
            d: 2.0,
            seq: 0,
            node: 0,
            axis: Axis::H,
        };
        let tied = State {
            d: 1.0,
            seq: 10,
            node: 1,
            axis: Axis::V,
        };
        // `BinaryHeap` pops the greatest, so cheaper must compare greater.
        assert!(cheap > dear);
        // A tie goes to whichever was pushed first.
        assert!(cheap > tied);
        assert_eq!(cheap.partial_cmp(&dear), Some(std::cmp::Ordering::Greater));
        assert_eq!(
            cheap,
            State {
                d: 1.0,
                seq: 9,
                node: 5,
                axis: Axis::V
            }
        );
        assert_ne!(cheap, tied);
    }

    #[test]
    fn a_search_with_no_path_gives_up_rather_than_looping() {
        // Two nodes, no links between them.
        let nodes = [Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
        let adj = [Vec::new(), Vec::new()];
        assert_eq!(shortest_path(&nodes, &adj, 0, 1, &Occupancy::new()), None);
        // ... and finds the trivial path when start and end coincide.
        assert_eq!(
            shortest_path(&nodes, &adj, 0, 0, &Occupancy::new()),
            Some(vec![0])
        );
    }

    #[test]
    fn a_busy_lane_is_avoided_when_another_is_free() {
        let boxes = two_boxes();
        let lat = build_lattice(&boxes, 56.0, 68.0);
        let mut load = LaneLoad::new();
        // Load the centre lane of the gutter heavily.
        *load.entry((Axis::V, fixed1(250.0))).or_insert(0.0) += 50.0;
        let route = route_on_lattice(
            Point::new(200.0, 110.0),
            Point::new(300.0, 150.0),
            SidePair {
                start: Side::Right,
                end: Side::Left,
            },
            &lat,
            &load,
            &[],
            &Occupancy::new(),
        );
        let used = route.iter().map(|p| p.x).find(|&x| x > 200.0 && x < 300.0);
        assert!(used.is_some_and(|x| (x - 250.0).abs() > 1e-9), "{route:?}");
    }
}
