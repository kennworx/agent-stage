//! Lining up the two ends of a wire so it can be drawn straight.
//!
//! Spreading the edges that share a box face keeps them from leaving on top of
//! one another, but it settles each face on its own. A wire whose source has
//! three siblings and whose target has none leaves off-centre, arrives at the
//! centre, and pays two corners for the difference — even though the target face
//! is empty and there was nothing to spread away from. Every such jog is two
//! bends the reader has to follow for no information.
//!
//! So after the spreading, each wire asks whether its ends could simply meet.
//! A port may move when the place it wants to move to is inside its own face,
//! clear of the columns crossing the gap, and still between the neighbours it
//! started between — that last part is what keeps a straightened wire from
//! stepping over a sibling and trading two bends for a crossing.

use super::route::{Placement, PORT_SPACING};

/// How close two ports on one face may sit before they read as one line.
///
/// Below the full spacing: the spreading pass has already put the ports where it
/// wants them, and this only has to keep a port that moves from landing on top of
/// one that did not.
const NEAR: f64 = PORT_SPACING * 0.6;

/// One end of one wire — the thing that can move.
#[derive(Clone, Copy, PartialEq)]
struct End {
    edge: usize,
    node: usize,
    /// Whether this is the wire's source end. Source ends leave the bottom face,
    /// target ends arrive at the top, and the two are spread separately.
    source: bool,
}

fn at(ports: &[(f64, f64)], end: End) -> f64 {
    let slot = ports.get(end.edge).copied().unwrap_or((0.0, 0.0));
    if end.source {
        slot.0
    } else {
        slot.1
    }
}

fn set(ports: &mut [(f64, f64)], end: End, x: f64) {
    if let Some(slot) = ports.get_mut(end.edge) {
        if end.source {
            slot.0 = x;
        } else {
            slot.1 = x;
        }
    }
}

/// The band of a node's face a port may sit in, inset so a port never straddles
/// a corner.
///
/// The inset is capped at half the width, so a node with no width — the stand-in
/// a boundary-crossing wire attaches to, which is a point on a frame rather than
/// a box — gets the single position it actually has. Insetting it unconditionally
/// turns the band inside out, and then no wire can ever line up with a boundary:
/// every wire leaving a subgraph kept the corner it did not need.
fn face(place: &Placement, node: usize) -> (f64, f64) {
    let half = place.width_of(node) / 2.0;
    let inset = (PORT_SPACING / 2.0).min(half);
    let centre = place.x_of(node);
    (centre - half + inset, centre + half - inset)
}

/// Every end of every wire, paired with the node and face it sits on.
fn ends_of(place: &Placement) -> Vec<End> {
    let mut out = Vec::new();
    for (edge, chain) in place.layering.chains.iter().enumerate() {
        if chain.len() < 2 {
            continue;
        }
        if let (Some(first), Some(last)) = (chain.first().copied(), chain.last().copied()) {
            out.push(End {
                edge,
                node: first,
                source: true,
            });
            out.push(End {
                edge,
                node: last,
                source: false,
            });
        }
    }
    out
}

/// How far one end may slide without changing places with a neighbour.
///
/// Order on a face is the spreading pass's answer to which edges cross; keeping
/// inside the neighbours preserves it. The bound is the neighbour's position
/// plus the room two ports need, so the interval is empty exactly when the face
/// is too crowded to straighten anything — which is the right answer there.
/// Two ports landing on the same coordinate are ordered by their edge, so that
/// "below me" and "above me" stay a total order. Without the tie-break a
/// coincident port counts as both, the interval collapses to nothing, and a wire
/// that could straighten is refused for a neighbour it is standing on.
fn room(place: &Placement, ends: &[End], ports: &[(f64, f64)], mine: End) -> (f64, f64) {
    let here = (at(ports, mine), mine.edge);
    let (mut low, mut high) = face(place, mine.node);
    for other in ends {
        if *other == mine || other.node != mine.node || other.source != mine.source {
            continue;
        }
        let there = (at(ports, *other), other.edge);
        if there.0 < here.0 || (same(there.0, here.0) && there.1 < here.1) {
            low = low.max(there.0 + NEAR);
        } else {
            high = high.min(there.0 - NEAR);
        }
    }
    (low, high)
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Whether a position stands clear of every column already crossing the gap this
/// end's leg has to.
fn clear(place: &Placement, mine: End, x: f64) -> bool {
    super::ports::crossing(place, mine.node, mine.edge, mine.source)
        .iter()
        .all(|column| (x - column).abs() >= place.spacing.edge - 1e-9)
}

/// Whether one end can take a position: inside its room, and clear of the
/// columns.
fn takes(place: &Placement, ends: &[End], ports: &[(f64, f64)], mine: End, x: f64) -> bool {
    let (low, high) = room(place, ends, ports, mine);
    x >= low - 1e-9 && x <= high + 1e-9 && clear(place, mine, x)
}

fn clamp(v: f64, low: f64, high: f64) -> f64 {
    low.max(high.min(v))
}

/// The wires in the order they get to claim a straight line: fewest layers
/// first.
///
/// A wire between neighbouring layers is the one a reader most expects to be
/// straight, and it is also the one with the least room to hide a bend in. A
/// long chain that has already threaded three dummies is not made much worse by
/// one more corner, so it gives way.
fn shortest_first(place: &Placement) -> Vec<usize> {
    let mut order: Vec<(usize, usize)> = place
        .layering
        .chains
        .iter()
        .enumerate()
        .filter(|(_, chain)| chain.len() >= 2)
        .map(|(edge, chain)| (edge, chain.len()))
        .collect();
    order.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    order.into_iter().map(|(edge, _)| edge).collect()
}

/// Straighten a wire that runs between neighbouring layers, by moving both ends
/// to one column.
///
/// Both ends are free here, so the column they share is whatever both faces can
/// reach — the midpoint of where they already are, pulled into the band both
/// have room for. A pinned end fixes that band to the single point it sits on,
/// which is how one end can still come to the other.
fn meet(place: &Placement, ends: &[End], ports: &mut [(f64, f64)], edge: usize, pinned: &[End]) {
    let (Some(source), Some(target)) = (
        ends.iter().find(|e| e.edge == edge && e.source).copied(),
        ends.iter().find(|e| e.edge == edge && !e.source).copied(),
    ) else {
        return;
    };
    let (start, finish) = (at(ports, source), at(ports, target));
    if same(start, finish) {
        return;
    }
    let band = |end: End, here: f64| {
        if pinned.contains(&end) {
            (here, here)
        } else {
            room(place, ends, ports, end)
        }
    };
    let (source_low, source_high) = band(source, start);
    let (target_low, target_high) = band(target, finish);
    let low = source_low.max(target_low);
    let high = source_high.min(target_high);
    if low > high {
        return;
    }
    // Meet in the middle of what both can reach, and if a column is sitting
    // there, try each end's own position before giving the corners back.
    let wanted = [f64::midpoint(start, finish), start, finish]
        .into_iter()
        .map(|x| clamp(x, low, high))
        .find(|x| clear(place, source, *x) && clear(place, target, *x));
    let Some(wanted) = wanted else { return };
    set(ports, source, wanted);
    set(ports, target, wanted);
}

/// Straighten the end legs of a wire that runs through dummies.
///
/// The columns between are already drawn to, so only the two ends can move — each
/// toward the column its own leg reaches, independently of the other.
fn reach(place: &Placement, ends: &[End], ports: &mut [(f64, f64)], edge: usize, pinned: &[End]) {
    let Some(chain) = place.layering.chains.get(edge) else {
        return;
    };
    let next = chain.get(1).copied();
    let prev = chain
        .len()
        .checked_sub(2)
        .and_then(|at| chain.get(at))
        .copied();
    for (source, toward) in [(true, next), (false, prev)] {
        let (Some(end), Some(toward)) = (
            ends.iter()
                .find(|e| e.edge == edge && e.source == source)
                .copied(),
            toward,
        ) else {
            continue;
        };
        if pinned.contains(&end) {
            continue;
        }
        let x = place.x_of(toward);
        if takes(place, ends, ports, end, x) {
            set(ports, end, x);
        }
    }
}

/// Move what ports can move so their wires come out straight.
pub(super) fn straighten(
    place: &Placement,
    pins: &[crate::layout::Port],
    reversed: &[bool],
    ports: &mut [(f64, f64)],
) {
    let ends = ends_of(place);
    let pinned: Vec<End> = pins
        .iter()
        .filter_map(|pin| {
            // The cycle break may have turned the wire round, so the end the
            // caller named is not always the end of the chain they meant.
            let flipped = reversed.get(pin.edge).copied().unwrap_or(false);
            let source = pin.source != flipped;
            ends.iter()
                .find(|e| e.edge == pin.edge && e.source == source)
                .copied()
        })
        .collect();

    for edge in shortest_first(place) {
        let len = place.layering.chains.get(edge).map_or(0, Vec::len);
        if len == 2 {
            meet(place, &ends, ports, edge, &pinned);
        } else {
            reach(place, &ends, ports, edge, &pinned);
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
    use crate::layout::route::route;
    use crate::layout::table::Table;
    use crate::layout::types::{Edge, Node, Point, Port, Spacing};
    use crate::layout::Layering;

    /// One laid-out graph, held together so a `Placement` can borrow from it.
    struct Case {
        layering: Layering,
        centres: Table<f64>,
        tops: Vec<(f64, f64)>,
        spacing: Spacing,
        reversed: Vec<bool>,
    }

    fn laid_out(sizes: &[Node], pairs: &[(usize, usize)]) -> Case {
        let edges: Vec<Edge> = pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect();
        let acyclic = break_cycles(sizes.len(), &edges);
        let layering = assign_layers(sizes, &acyclic.arcs, edges.len());
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
        Case {
            layering,
            centres,
            tops,
            spacing,
            reversed,
        }
    }

    impl Case {
        fn placement(&self) -> Placement<'_> {
            Placement {
                layering: &self.layering,
                centres: &self.centres,
                tops: &self.tops,
                spacing: &self.spacing,
            }
        }

        /// The routed points of every edge, which is where a bend is visible.
        fn routes(&self, pins: &[Port]) -> Vec<Vec<Point>> {
            let place = self.placement();
            route(&place, &[], &self.reversed, self.reversed.len(), pins)
        }
    }

    fn boxes(n: usize) -> Vec<Node> {
        vec![Node::new(100.0, 40.0); n]
    }

    /// Corners in a run — a change of axis, not merely a waypoint.
    fn bends(points: &[Point]) -> usize {
        points
            .windows(3)
            .filter(|w| match (w.first(), w.get(1), w.get(2)) {
                (Some(a), Some(b), Some(c)) => {
                    let straight = ((a.x - b.x).abs() < 0.5 && (b.x - c.x).abs() < 0.5)
                        || ((a.y - b.y).abs() < 0.5 && (b.y - c.y).abs() < 0.5);
                    !straight
                }
                _ => false,
            })
            .count()
    }

    #[test]
    fn a_wire_whose_far_face_is_empty_is_drawn_straight() {
        // `a` fans out to two targets, so its own face is shared and its ports
        // are spread; each target's face carries one wire and has room to come
        // and meet it.
        let case = laid_out(&boxes(3), &[(0, 1), (0, 2)]);
        for run in case.routes(&[]) {
            assert_eq!(bends(&run), 0, "{run:?}");
        }
    }

    #[test]
    fn a_wire_through_dummies_straightens_the_leg_it_can_reach() {
        // 0 -> 2 spans two layers, so it is drawn through a dummy; the port
        // moves onto that dummy's column rather than jogging across to it.
        let case = laid_out(&boxes(3), &[(0, 1), (1, 2), (0, 2)]);
        let place = case.placement();
        let long = 2;
        let chain = case.layering.chains.get(long).expect("the long chain");
        assert!(chain.len() > 2, "expected a dummy in {chain:?}");
        let mut ports = vec![(0.0, 0.0); 3];
        for (edge, chain) in case.layering.chains.iter().enumerate() {
            if let (Some(first), Some(last)) = (chain.first(), chain.last()) {
                if let Some(slot) = ports.get_mut(edge) {
                    *slot = (place.x_of(*first), place.x_of(*last));
                }
            }
        }
        straighten(&place, &[], &case.reversed, &mut ports);
        let next = chain.get(1).copied().expect("the dummy");
        let got = ports.get(long).copied().expect("the long edge's ports");
        assert!(
            (got.0 - place.x_of(next)).abs() < 1e-9,
            "port {got:?} did not reach column {}",
            place.x_of(next)
        );
    }

    #[test]
    fn a_pinned_end_is_left_where_the_caller_put_it() {
        let case = laid_out(&boxes(2), &[(0, 1)]);
        let place = case.placement();
        // Hard against the left of the source's face.
        let pins = [Port::new(0, true, 0.0)];
        let want = place.x_of(0) - place.width_of(0) / 2.0;
        let run = case.routes(&pins);
        let start = run.first().and_then(|r| r.first()).copied().expect("a run");
        assert!((start.x - want).abs() < 1e-9, "{start:?} moved off its pin");
    }

    #[test]
    fn a_face_with_no_room_left_keeps_every_port_where_it_was() {
        // Four wires onto one narrow face: the neighbours leave no interval to
        // move within, so nothing moves and nothing lands on anything.
        let mut sizes = boxes(5);
        if let Some(narrow) = sizes.get_mut(4) {
            narrow.width = 30.0;
        }
        let case = laid_out(&sizes, &[(0, 4), (1, 4), (2, 4), (3, 4)]);
        let place = case.placement();
        let mut ports: Vec<(f64, f64)> = (0..4).map(|_| (place.x_of(4), place.x_of(4))).collect();
        for (at, slot) in ports.iter_mut().enumerate() {
            slot.1 = place.x_of(4) + as_offset(at);
        }
        let before = ports.clone();
        straighten(&place, &[], &case.reversed, &mut ports);
        for (edge, (was, now)) in before.iter().zip(ports.iter()).enumerate() {
            assert!(
                (was.1 - now.1).abs() < 1e-9,
                "edge {edge} arrival moved from {was:?} to {now:?} on a full face"
            );
        }
    }

    /// Four arrivals a hair apart, so the room between neighbours is empty.
    fn as_offset(at: usize) -> f64 {
        match at {
            0 => -3.0,
            1 => -1.0,
            2 => 1.0,
            _ => 3.0,
        }
    }

    #[test]
    fn an_end_naming_an_edge_that_is_not_there_reads_and_writes_nothing() {
        // The guard the routed diagrams never reach: every caller builds its
        // ends from the chains it is holding, so the index is always good. It
        // still has to be a no-op rather than a panic.
        let missing = End {
            edge: 7,
            node: 0,
            source: false,
        };
        let mut ports = vec![(1.0, 2.0)];
        assert!((at(&ports, missing)).abs() < 1e-9);
        set(&mut ports, missing, 99.0);
        assert_eq!(ports, vec![(1.0, 2.0)]);
        // And the target end of a real edge is written, not just the source.
        let target = End {
            edge: 0,
            node: 0,
            source: false,
        };
        set(&mut ports, target, 5.0);
        assert_eq!(ports, vec![(1.0, 5.0)]);
    }

    #[test]
    fn a_wire_already_straight_is_not_disturbed() {
        let case = laid_out(&boxes(2), &[(0, 1)]);
        let place = case.placement();
        let mut ports = vec![(place.x_of(0), place.x_of(0))];
        straighten(&place, &[], &case.reversed, &mut ports);
        assert!((ports[0].0 - ports[0].1).abs() < 1e-9, "{ports:?}");
    }

    #[test]
    fn an_edge_with_no_chain_is_passed_over() {
        // A self-loop never reaches the layering, so its chain is empty; the
        // pass must not invent an end for it.
        let case = laid_out(&boxes(2), &[(0, 1), (0, 0)]);
        let place = case.placement();
        let mut ports = vec![(0.0, 0.0); 2];
        straighten(&place, &[], &case.reversed, &mut ports);
        assert!((ports[1].0).abs() < 1e-9, "{ports:?}");
    }

    #[test]
    fn the_shortest_wires_claim_their_straight_line_first() {
        let case = laid_out(&boxes(3), &[(0, 2), (0, 1), (1, 2)]);
        let order = shortest_first(&case.placement());
        let len = |edge: &usize| case.layering.chains.get(*edge).map_or(0, Vec::len);
        assert!(
            order.windows(2).all(|w| match (w.first(), w.get(1)) {
                (Some(a), Some(b)) => len(a) <= len(b),
                _ => true,
            }),
            "{order:?} is not shortest first"
        );
    }
}
