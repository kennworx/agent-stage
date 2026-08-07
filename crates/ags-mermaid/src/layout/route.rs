//! Turning each edge's chain of nodes into an orthogonal run of points.
//!
//! An edge leaves the bottom of its source and arrives at the top of its target.
//! When the ends do not share a column it steps sideways, and *where* matters:
//! two edges stepping at the same height along one span are drawn as one line,
//! and a reader counts one edge where there are two. So every sideways run gets
//! a lane of its own. The checker reports merged runs, which makes this pass
//! testable rather than a matter of taste.

use super::layers::Layering;
use super::table::{as_f64, Table};
use std::cmp::Ordering;

use super::ports::ports;
use super::types::{Point, Spacing};

/// How far a self-loop reaches out from the side of its node.
const LOOP_REACH: f64 = 24.0;

/// The most two edges leaving one node are spread apart.
pub(super) const PORT_SPACING: f64 = 14.0;

/// Where each node's box sits: its centre along the layer, and its own band.
pub struct Placement<'a> {
    pub layering: &'a Layering,
    pub centres: &'a Table<f64>,
    /// The top and height of each layer.
    pub tops: &'a [(f64, f64)],
    pub spacing: &'a Spacing,
}

impl Placement<'_> {
    pub(super) fn layer_of(&self, node: usize) -> usize {
        self.layering.nodes.get(node).map_or(0, |n| n.layer)
    }

    fn band(&self, node: usize) -> (f64, f64) {
        self.tops
            .get(self.layer_of(node))
            .copied()
            .unwrap_or((0.0, 0.0))
    }

    /// The top and bottom of a node's own box, centred in its layer's band.
    fn edges_of(&self, node: usize) -> (f64, f64) {
        let (top, height) = self.band(node);
        let own = self.layering.nodes.get(node).map_or(0.0, |n| n.size.height);
        let start = top + (height - own) / 2.0;
        (start, start + own)
    }

    pub(super) fn x_of(&self, node: usize) -> f64 {
        self.centres.get(node)
    }

    pub(super) fn width_of(&self, node: usize) -> f64 {
        self.layering
            .nodes
            .get(node)
            .map_or(0.0, |found| found.size.width)
    }

    /// Where the run between two layers may bend, and how much room it has.
    fn gap(&self, upper: usize) -> (f64, f64) {
        let (top, height) = self.tops.get(upper).copied().unwrap_or((0.0, 0.0));
        let next = self
            .tops
            .get(upper + 1)
            .map_or(top + height + self.spacing.layer, |(next, _)| *next);
        (top + height, next)
    }
}

/// A sideways run wanting a lane in one gap.
pub(super) struct Step {
    pub(super) edge: usize,
    pub(super) at: usize,
    pub(super) gap: usize,
    /// The columns the run enters and leaves the gap at, in that order.
    pub(super) order: (f64, f64),
}

/// Which lane, of how many, each sideways run gets.
///
/// Sorted first by where the run starts and ends, so the assignment repeats and
/// the reading order is the tie-break, then restacked within each gap by which
/// runs have to be above which — see [`super::channel`], which is where the
/// crossings are actually decided.
fn lanes(steps: &mut Vec<Step>) -> Table<f64> {
    steps.sort_by(|a, b| {
        a.gap
            .cmp(&b.gap)
            .then_with(|| a.order.0.partial_cmp(&b.order.0).unwrap_or(Ordering::Equal))
            .then_with(|| a.order.1.partial_cmp(&b.order.1).unwrap_or(Ordering::Equal))
            .then(a.edge.cmp(&b.edge))
            .then(a.at.cmp(&b.at))
    });
    *steps = super::channel::stacked(std::mem::take(steps));
    let mut out = Table::<f64>::new(steps.len());
    let mut gap = usize::MAX;
    let mut within = 0usize;
    let mut counts: Vec<usize> = Vec::new();
    for step in steps.iter() {
        if step.gap == gap {
            within += 1;
        } else {
            gap = step.gap;
            within = 0;
        }
        counts.push(within);
    }
    // How many share each gap, so the lanes can be spread evenly across it.
    let mut total: Vec<usize> = vec![0; steps.len()];
    for (at, step) in steps.iter().enumerate() {
        let count = steps.iter().filter(|other| other.gap == step.gap).count();
        if let Some(slot) = total.get_mut(at) {
            *slot = count;
        }
    }
    for (at, within) in counts.iter().enumerate() {
        let count = total.get(at).copied().unwrap_or(1).max(1);
        out.set(at, (as_f64(*within) + 1.0) / (as_f64(count) + 1.0));
    }
    out
}

/// A rounded coordinate, so two runs meant to share a line actually do.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Whether a sideways run is too short to be worth its two corners. Nothing
/// fits between ends closer than `spacing.edge`, so the step separates the wire
/// from nothing. A flowchart of seven boxes had four, each 7 to 14px.
fn negligible(from: f64, to: f64, spacing: &Spacing) -> bool {
    !same(from, to) && (from - to).abs() < spacing.edge
}

/// The points of one edge, from the first node of its chain to the last.
fn along(
    chain: &[usize],
    place: &Placement,
    lane_of: &dyn Fn(usize) -> f64,
    ends: (f64, f64),
) -> Vec<Point> {
    let Some(first) = chain.first().copied() else {
        return Vec::new();
    };
    let (_, bottom) = place.edges_of(first);
    let mut out = vec![Point::new(ends.0, bottom)];
    let last = chain.len().saturating_sub(2);
    for (at, pair) in chain.windows(2).enumerate() {
        let (Some(upper), Some(lower)) = (pair.first().copied(), pair.get(1).copied()) else {
            continue;
        };
        // The two ends of the whole run are where the ports put them; every
        // bend between is the dummy's own column.
        let mut from_x = if at == 0 { ends.0 } else { place.x_of(upper) };
        let mut to_x = if at == last {
            ends.1
        } else {
            place.x_of(lower)
        };
        // Only a port may move: a column is where the run has already been
        // drawn to, and shifting that leaves a diagonal behind it.
        if negligible(from_x, to_x, place.spacing) {
            if at == 0 {
                from_x = to_x;
                if let Some(first) = out.first_mut() {
                    first.x = to_x;
                }
            } else if at == last {
                to_x = from_x;
            }
        }
        let (top, _) = place.edges_of(lower);
        if !same(from_x, to_x) {
            let (start, end) = place.gap(place.layer_of(upper));
            let bend = start + (end - start) * lane_of(at);
            out.push(Point::new(from_x, bend));
            out.push(Point::new(to_x, bend));
        }
        out.push(Point::new(to_x, top));
        // A dummy is a bend, not a stop: the run carries straight through it to
        // the far side of its band.
        if place
            .layering
            .nodes
            .get(lower)
            .is_some_and(super::layers::LayoutNode::is_dummy)
        {
            out.pop();
        }
    }
    out
}

/// A self-loop: out of the right-hand side and back in.
fn loop_points(node: usize, place: &Placement) -> Vec<Point> {
    let (top, bottom) = place.edges_of(node);
    let width = place.layering.nodes.get(node).map_or(0.0, |n| n.size.width);
    let right = place.x_of(node) + width / 2.0;
    let (high, low) = (top + (bottom - top) / 3.0, bottom - (bottom - top) / 3.0);
    vec![
        Point::new(right, high),
        Point::new(right + LOOP_REACH, high),
        Point::new(right + LOOP_REACH, low),
        Point::new(right, low),
    ]
}

/// Route every edge. The result is in the caller's edge order.
///
/// `reversed` names the edges whose direction the cycle break turned around;
/// their points come back the way the caller wrote them.
/// Every sideways run, and which gap it crosses.
fn sideways(place: &Placement, ports: &[(f64, f64)]) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    for (edge, chain) in place.layering.chains.iter().enumerate() {
        let end = ports.get(edge).copied().unwrap_or((0.0, 0.0));
        let last = chain.len().saturating_sub(2);
        for (at, pair) in chain.windows(2).enumerate() {
            let (Some(upper), Some(lower)) = (pair.first().copied(), pair.get(1).copied()) else {
                continue;
            };
            let from_x = if at == 0 { end.0 } else { place.x_of(upper) };
            let to_x = if at == last { end.1 } else { place.x_of(lower) };
            if same(from_x, to_x) {
                continue;
            }
            steps.push(Step {
                edge,
                at,
                gap: place.layer_of(upper),
                // A run that starts further left takes an earlier lane, which
                // keeps two runs crossing the same gap from swapping over.
                order: (from_x, to_x),
            });
        }
    }
    steps
}

/// How many lanes each gap has to hold. Answered before the layer heights
/// exist, which a gap is sized from — not circular, because a port comes from
/// the node's column and width and never a layer's height.
pub fn gap_lanes(
    place: &Placement,
    edge_count: usize,
    pins: &[crate::layout::Port],
    reversed: &[bool],
) -> Vec<usize> {
    let ports = ports(place, edge_count, pins, reversed);
    let mut out = vec![0usize; place.tops.len()];
    for step in sideways(place, &ports) {
        if let Some(slot) = out.get_mut(step.gap) {
            *slot += 1;
        }
    }
    out
}

pub fn route(
    place: &Placement,
    loops: &[usize],
    reversed: &[bool],
    edge_count: usize,
    pins: &[crate::layout::Port],
) -> Vec<Vec<Point>> {
    let ports = ports(place, edge_count, pins, reversed);
    let mut steps = sideways(place, &ports);
    let fractions = lanes(&mut steps);
    let lane_at = |edge: usize, at: usize| {
        steps
            .iter()
            .position(|step| step.edge == edge && step.at == at)
            .map_or(0.5, |found| fractions.get(found))
    };

    let mut out = vec![Vec::new(); edge_count];
    for (edge, chain) in place.layering.chains.iter().enumerate() {
        if chain.len() < 2 {
            continue;
        }
        let lane_of = |at: usize| lane_at(edge, at);
        let mut points = along(
            chain,
            place,
            &lane_of,
            ports.get(edge).copied().unwrap_or((0.0, 0.0)),
        );
        if reversed.get(edge).copied().unwrap_or(false) {
            points.reverse();
        }
        if let Some(slot) = out.get_mut(edge) {
            *slot = points;
        }
    }
    for edge in loops {
        let Some(chain) = place.layering.chains.get(*edge) else {
            continue;
        };
        // A self-loop never reached the layering, so its chain is empty and the
        // node it belongs to has to come from the caller's own edge list.
        let _ = chain;
        if let Some(slot) = out.get_mut(*edge) {
            *slot = Vec::new();
        }
    }
    out
}

/// Route the self-loops, which never reached the layering.
pub fn route_loops(place: &Placement, loops: &[(usize, usize)], out: &mut [Vec<Point>]) {
    for (edge, node) in loops {
        if let Some(slot) = out.get_mut(*edge) {
            *slot = loop_points(*node, place);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cycles::break_cycles;
    use crate::layout::layers::assign_layers;
    use crate::layout::order::order_layers;
    use crate::layout::place::{layer_tops, place};
    use crate::layout::types::{Edge, Node, Port};

    struct Case {
        layering: Layering,
        centres: Table<f64>,
        tops: Vec<(f64, f64)>,
        spacing: Spacing,
        routes: Vec<Vec<Point>>,
    }

    fn routed(n: usize, pairs: &[(usize, usize)]) -> Case {
        let edges: Vec<Edge> = pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect();
        let sizes = vec![Node::new(100.0, 40.0); n];
        let acyclic = break_cycles(n, &edges);
        let layering = assign_layers(&sizes, &acyclic.arcs, edges.len());
        let layers = order_layers(&layering);
        let spacing = Spacing::default();
        let centres = place(&layering, &layers, &spacing);
        let tops = layer_tops(&layering, &layers, &spacing, &[]);
        let mut reversed = vec![false; edges.len()];
        for arc in &acyclic.arcs {
            if let Some(slot) = reversed.get_mut(arc.source) {
                *slot = arc.reversed;
            }
        }
        let routes = {
            let place = Placement {
                layering: &layering,
                centres: &centres,
                tops: &tops,
                spacing: &spacing,
            };
            let mut routes = route(&place, &acyclic.loops, &reversed, edges.len(), &[]);
            let pinned: Vec<(usize, usize)> = acyclic
                .loops
                .iter()
                .filter_map(|edge| edges.get(*edge).map(|e| (*edge, e.from)))
                .collect();
            route_loops(&place, &pinned, &mut routes);
            routes
        };
        Case {
            layering,
            centres,
            tops,
            spacing,
            routes,
        }
    }

    /// Route `pairs` with `pins` applied, and give back where each edge met its
    /// ends — which is the whole of what a pin is meant to move.
    fn ends_with_pins(n: usize, pairs: &[(usize, usize)], pins: &[Port]) -> Vec<(f64, f64)> {
        let edges: Vec<Edge> = pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect();
        let sizes = vec![Node::new(100.0, 40.0); n];
        let acyclic = break_cycles(n, &edges);
        let layering = assign_layers(&sizes, &acyclic.arcs, edges.len());
        let layers = order_layers(&layering);
        let spacing = Spacing::default();
        let centres = place(&layering, &layers, &spacing);
        let tops = layer_tops(&layering, &layers, &spacing, &[]);
        let mut reversed = vec![false; edges.len()];
        for arc in &acyclic.arcs {
            if let Some(slot) = reversed.get_mut(arc.source) {
                *slot = arc.reversed;
            }
        }
        let place = Placement {
            layering: &layering,
            centres: &centres,
            tops: &tops,
            spacing: &spacing,
        };
        ports(&place, edges.len(), pins, &reversed)
    }

    #[test]
    fn a_pinned_end_meets_its_node_where_the_caller_said() {
        // Two edges into one node are spread; pinning one moves it to the place
        // asked for, as a fraction of the node's own side.
        let free = ends_with_pins(3, &[(0, 2), (1, 2)], &[]);
        let pinned = ends_with_pins(3, &[(0, 2), (1, 2)], &[Port::new(0, false, 0.0)]);
        assert!(
            (free[0].1 - pinned[0].1).abs() > 1e-9,
            "the pinned end has to move"
        );
        // The other edge is untouched: one pinned wire must not cost the rest
        // their ordering.
        assert!((free[1].1 - pinned[1].1).abs() < 1e-9);
        // Nought is the low edge of a 100-wide box, so 50 left of its centre.
        let centre = ends_with_pins(3, &[(0, 2), (1, 2)], &[Port::new(0, false, 0.5)]);
        assert!(
            (centre[0].1 - pinned[0].1 - 50.0).abs() < 1e-9,
            "{:?} {:?}",
            centre[0],
            pinned[0]
        );
    }

    #[test]
    fn a_pin_outside_the_node_is_brought_back_to_its_edge() {
        let low = ends_with_pins(2, &[(0, 1)], &[Port::new(0, true, -5.0)]);
        let at_zero = ends_with_pins(2, &[(0, 1)], &[Port::new(0, true, 0.0)]);
        assert!((low[0].0 - at_zero[0].0).abs() < 1e-9);
        let high = ends_with_pins(2, &[(0, 1)], &[Port::new(0, true, 9.0)]);
        let at_one = ends_with_pins(2, &[(0, 1)], &[Port::new(0, true, 1.0)]);
        assert!((high[0].0 - at_one[0].0).abs() < 1e-9);
    }

    #[test]
    fn a_pin_names_the_end_the_caller_wrote_even_when_the_cycle_break_turned_it() {
        // `1 -> 0` closes a cycle, so the layered pass runs it the other way. A
        // pin on its source must still land on node 1.
        let pins = &[Port::new(1, true, 0.0)];
        let ends = ends_with_pins(2, &[(0, 1), (1, 0)], pins);
        let free = ends_with_pins(2, &[(0, 1), (1, 0)], &[]);
        assert!(
            (ends[1].0 - free[1].0).abs() > 1e-9 || (ends[1].1 - free[1].1).abs() > 1e-9,
            "the pin moved something"
        );
    }

    #[test]
    fn a_pin_naming_an_edge_that_does_not_exist_is_ignored() {
        let free = ends_with_pins(2, &[(0, 1)], &[]);
        let wild = ends_with_pins(2, &[(0, 1)], &[Port::new(99, true, 0.0)]);
        assert!((free[0].0 - wild[0].0).abs() < 1e-9);
        assert!((free[0].1 - wild[0].1).abs() < 1e-9);
    }

    #[test]
    fn a_pin_on_an_edge_that_was_dropped_is_ignored() {
        // An edge naming a box nobody declared is dropped rather than rejected,
        // so it has a place in the caller's list and no chain to route. A pin on
        // it must be stepped over rather than reaching for the chain's ends.
        use crate::layout::{layout, Graph, Node as LayoutNode, Port as Pin};
        let drawn = layout(&Graph {
            nodes: vec![LayoutNode::new(100.0, 40.0); 2],
            edges: vec![Edge::new(0, 1), Edge::new(0, 9)],
            ports: vec![Pin::new(1, true, 0.5), Pin::new(0, false, 0.0)],
            ..Graph::default()
        });
        assert!(
            drawn.edges[1].points.is_empty(),
            "the dropped edge is not drawn"
        );
        assert!(!drawn.edges[0].points.is_empty(), "the real one still is");
    }

    fn place_of(case: &Case) -> Placement<'_> {
        Placement {
            layering: &case.layering,
            centres: &case.centres,
            tops: &case.tops,
            spacing: &case.spacing,
        }
    }

    /// The columns a run turns at, which is what a bend costs.
    fn turns(points: &[Point]) -> Vec<f64> {
        points
            .windows(3)
            .filter(|w| match (w.first(), w.get(1), w.get(2)) {
                (Some(a), Some(b), Some(c)) => {
                    !(same(a.x, b.x) && same(b.x, c.x) || same(a.y, b.y) && same(b.y, c.y))
                }
                _ => false,
            })
            .filter_map(|w| w.get(1).map(|b| b.x))
            .collect()
    }

    #[test]
    fn a_step_too_short_to_hold_anything_is_not_drawn() {
        // Straightening lines most ends up exactly, but not all: where it can
        // only get close, a residue narrower than the room a wire needs beside
        // its neighbour still buys nothing, and `along` drops it. Driven
        // directly, because the whole point of the pass above is that a routed
        // diagram no longer produces one.
        let case = routed(2, &[(0, 1)]);
        let place = place_of(&case);
        let chain = case.layering.chains.first().cloned().unwrap_or_default();
        let start = place.x_of(0);
        let lane = |_: usize| 0.5;
        let short = along(&chain, &place, &lane, (start, start + 5.0));
        assert!(turns(&short).is_empty(), "{short:?}");
        // Both ends were pulled onto the one column, so nothing is left leaning.
        assert!(short.iter().all(|at| same(at.x, start + 5.0)), "{short:?}");
    }

    #[test]
    fn a_step_worth_its_corners_is_drawn() {
        // The same run, with the ends far enough apart that the step separates
        // the wire from something.
        let case = routed(2, &[(0, 1)]);
        let place = place_of(&case);
        let chain = case.layering.chains.first().cloned().unwrap_or_default();
        let start = place.x_of(0);
        let lane = |_: usize| 0.5;
        let long = along(&chain, &place, &lane, (start, start + 40.0));
        assert_eq!(turns(&long).len(), 2, "{long:?}");
    }

    #[test]
    fn the_last_leg_of_a_chain_drops_a_step_too_short_to_see() {
        // The other end: a long chain's final leg runs from the last dummy's
        // column to the target's port, and that gap can be negligible too.
        let case = routed(3, &[(0, 1), (1, 2), (0, 2)]);
        let place = place_of(&case);
        let chain = case.layering.chains.get(2).cloned().unwrap_or_default();
        assert!(chain.len() > 2, "expected a dummy in {chain:?}");
        let column = chain.get(1).map_or(0.0, |dummy| place.x_of(*dummy));
        let lane = |_: usize| 0.5;
        let run = along(&chain, &place, &lane, (column, column + 5.0));
        assert!(turns(&run).is_empty(), "{run:?}");
    }

    #[test]
    fn a_chain_with_no_nodes_draws_nothing() {
        let case = routed(2, &[(0, 1)]);
        let place = place_of(&case);
        let lane = |_: usize| 0.5;
        assert!(along(&[], &place, &lane, (0.0, 0.0)).is_empty());
    }

    #[test]
    fn an_edge_leaves_its_source_and_reaches_its_target() {
        let case = routed(2, &[(0, 1)]);
        let route = case.routes.first().cloned().unwrap_or_default();
        assert!(route.len() >= 2);
        let place = place_of(&case);
        let (_, bottom) = place.edges_of(0);
        let (top, _) = place.edges_of(1);
        assert!((route.first().map_or(0.0, |p| p.y) - bottom).abs() < 1e-9);
        assert!((route.last().map_or(0.0, |p| p.y) - top).abs() < 1e-9);
    }

    #[test]
    fn an_edge_between_columns_steps_sideways_once() {
        let case = routed(3, &[(0, 1), (0, 2)]);
        for route in &case.routes {
            let sideways = route
                .windows(2)
                .filter(|pair| {
                    pair.first()
                        .zip(pair.get(1))
                        .is_some_and(|(a, b)| !same(a.x, b.x))
                })
                .count();
            assert!(sideways <= 1, "{route:?}");
        }
    }

    #[test]
    fn every_run_is_axis_aligned() {
        let case = routed(6, &[(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5)]);
        for route in &case.routes {
            for pair in route.windows(2) {
                let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                assert!(same(a.x, b.x) || same(a.y, b.y), "{a:?} to {b:?}");
            }
        }
    }

    #[test]
    fn two_edges_crossing_the_same_gap_take_different_lanes() {
        // One source fanning to three targets: the outer two sit further out
        // than either face can reach, so those runs have to step sideways across
        // the same gap, and drawn at the same height they would read as one
        // line. (The middle one lines up and is drawn straight, which is why the
        // runs are counted rather than the edges.)
        let case = routed(4, &[(0, 1), (0, 2), (0, 3)]);
        let bends: Vec<f64> = case
            .routes
            .iter()
            .filter(|route| route.len() >= 4)
            .filter_map(|route| route.get(1).map(|point| point.y))
            .collect();
        assert_eq!(bends.len(), 2, "{:?}", case.routes);
        let (Some(a), Some(b)) = (bends.first(), bends.get(1)) else {
            return;
        };
        assert!(!same(*a, *b), "both bend at {a}");
    }

    #[test]
    fn a_long_edge_runs_through_its_dummies_without_stopping() {
        let case = routed(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let long = case.routes.get(3).cloned().unwrap_or_default();
        assert!(long.len() >= 2);
        // It starts at the source and ends at the target, whatever it does
        // between.
        let place = place_of(&case);
        let (_, bottom) = place.edges_of(0);
        let (top, _) = place.edges_of(3);
        assert!((long.first().map_or(0.0, |p| p.y) - bottom).abs() < 1e-9);
        assert!((long.last().map_or(0.0, |p| p.y) - top).abs() < 1e-9);
    }

    #[test]
    fn a_turned_edge_is_written_the_way_the_caller_wrote_it() {
        // `2 -> 0` closes a cycle and is reversed for layering; its points must
        // still run from 2 to 0.
        let case = routed(3, &[(0, 1), (1, 2), (2, 0)]);
        let back = case.routes.get(2).cloned().unwrap_or_default();
        assert!(back.len() >= 2);
        let place = place_of(&case);
        let (top_of_two, bottom_of_two) = place.edges_of(2);
        let first = back.first().map_or(0.0, |p| p.y);
        assert!(
            (first - top_of_two).abs() < 1e-9 || (first - bottom_of_two).abs() < 1e-9,
            "starts at 2, not {first}"
        );
    }

    #[test]
    fn a_self_loop_goes_out_of_the_side_and_back() {
        let case = routed(2, &[(0, 0), (0, 1)]);
        let looped = case.routes.first().cloned().unwrap_or_default();
        assert_eq!(looped.len(), 4);
        let place = place_of(&case);
        let right = place.x_of(0) + 50.0;
        assert!((looped.first().map_or(0.0, |p| p.x) - right).abs() < 1e-9);
        assert!((looped.last().map_or(0.0, |p| p.x) - right).abs() < 1e-9);
        assert!(looped.get(1).is_some_and(|p| p.x > right));
    }

    #[test]
    fn an_edge_naming_a_box_that_does_not_exist_is_routed_to_nothing() {
        let case = routed(2, &[(0, 1), (0, 9)]);
        assert!(case.routes.get(1).is_some_and(Vec::is_empty));
    }

    #[test]
    fn the_same_graph_routes_the_same_way_twice() {
        let a = routed(5, &[(0, 1), (0, 2), (1, 3), (2, 4), (0, 4)]);
        let b = routed(5, &[(0, 1), (0, 2), (1, 3), (2, 4), (0, 4)]);
        assert_eq!(a.routes, b.routes);
    }

    #[test]
    fn a_graph_of_nothing_routes_nothing() {
        let case = routed(0, &[]);
        assert!(case.routes.is_empty());
    }
}
