//! Where each wire attaches to its box, and how the ports on one face share it.
//!
//! Two jobs, in this order: spread the edges meeting a face so none of them
//! leaves on top of another, then keep each port's leg clear of the columns
//! already crossing the gap it has to. [`super::align`] runs after both and
//! moves back onto a straight line whatever can go there.

use std::cmp::Ordering;

use super::route::{Placement, PORT_SPACING};
use super::table::as_f64;

/// The columns another edge already occupies in the gap a port's leg crosses.
///
/// A dummy *is* a column: the run carries straight through its band and the gaps
/// either side. [`super::place`] keeps columns `spacing.edge` apart, but a port
/// leg is not a node, so nothing reserves room for it — a state diagram put one
/// 5px from a column, under the 6px at which two lines read as one. Only the
/// next layer's dummies can be in the way: one in the node's own layer is
/// already `spacing.node` from its box.
pub(super) fn crossing(place: &Placement, node: usize, edge: usize, source: bool) -> Vec<f64> {
    let layer = place.layer_of(node);
    let neighbour = if source {
        layer.checked_add(1)
    } else {
        layer.checked_sub(1)
    };
    let Some(neighbour) = neighbour else {
        return Vec::new();
    };
    let mine = place.layering.chains.get(edge);
    (0..place.layering.nodes.len())
        .filter(|other| {
            place
                .layering
                .nodes
                .get(*other)
                .is_some_and(|found| found.is_dummy() && found.layer == neighbour)
        })
        .filter(|other| !mine.is_some_and(|chain| chain.contains(other)))
        .map(|other| place.x_of(other))
        .collect()
}

/// The nearest place on the face standing `room` clear of every column there.
///
/// A position is only ever blocked by a column, so the nearest free one is hard
/// against a column or against the end of the face — a handful to search, and
/// exact, where stepping by some increment would be a guess about the increment.
pub(super) fn clear_of(at: f64, taken: &[f64], face: (f64, f64), room: f64) -> f64 {
    let clear = |x: f64| taken.iter().all(|c| (x - c).abs() >= room - 1e-9);
    if face.0 >= face.1 || clear(at) {
        return at;
    }
    taken
        .iter()
        .flat_map(|c| [c - room, c + room])
        .chain([face.0, face.1])
        .filter(|x| *x >= face.0 && *x <= face.1 && clear(*x))
        .min_by(|a, b| {
            (a - at)
                .abs()
                .partial_cmp(&(b - at).abs())
                .unwrap_or(Ordering::Equal)
        })
        .unwrap_or(at)
}

/// Where along a node's side one of `count` edges leaves or arrives.
///
/// Edges leaving one point share a line until they bend, and two on one line
/// read as one. Spreading them across the node's side keeps them apart.
fn spread(centre: f64, width: f64, at: usize, count: usize) -> f64 {
    if count <= 1 {
        return centre;
    }
    let step = (width / (as_f64(count) + 1.0)).max(PORT_SPACING);
    centre + (as_f64(at) - (as_f64(count) - 1.0) / 2.0) * step
}

/// Where every edge leaves its source and arrives at its target.
///
/// Edges sharing a node are spread across its side, ordered by where their far
/// end sits so two of them do not have to cross to reach their own port.
pub(super) fn ports(
    place: &Placement,
    edge_count: usize,
    pins: &[crate::layout::Port],
    reversed: &[bool],
) -> Vec<(f64, f64)> {
    let mut out = vec![(0.0, 0.0); edge_count];
    let width = |node: usize| {
        place
            .layering
            .nodes
            .get(node)
            .map_or(0.0, |found| found.size.width)
    };
    let mut ends: Vec<(usize, usize, bool, f64)> = Vec::new();
    for (edge, chain) in place.layering.chains.iter().enumerate() {
        let (Some(first), Some(last)) = (chain.first().copied(), chain.last().copied()) else {
            continue;
        };
        if chain.len() < 2 {
            continue;
        }
        // The *next* column along the chain orders the ports, not the far
        // endpoint: they differ whenever an edge detours. A back edge routed
        // round the drawing has its target one way and its route the other, and
        // ordering by the target attaches it on the side it must then cross
        // everything to leave from.
        let after = chain.get(1).copied().unwrap_or(last);
        let before = chain
            .len()
            .checked_sub(2)
            .and_then(|at| chain.get(at).copied())
            .unwrap_or(first);
        let (near, far) = (place.x_of(before), place.x_of(after));
        ends.push((first, edge, true, far));
        ends.push((last, edge, false, near));
    }
    for node in 0..place.layering.nodes.len() {
        for source in [true, false] {
            let mut group: Vec<(usize, f64)> = ends
                .iter()
                .filter(|(at, _, is_source, _)| *at == node && *is_source == source)
                .map(|(_, edge, _, toward)| (*edge, *toward))
                .collect();
            group.sort_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(Ordering::Equal)
                    .then(a.0.cmp(&b.0))
            });
            let count = group.len();
            for (at, (edge, _)) in group.iter().enumerate() {
                let half = width(node) / 2.0;
                let centre = place.x_of(node);
                let along = spread(centre, width(node), at, count);
                // Then off any column already crossing the gap this leg has to.
                let along = clear_of(
                    along,
                    &crossing(place, node, *edge, source),
                    (
                        centre - half + PORT_SPACING / 2.0,
                        centre + half - PORT_SPACING / 2.0,
                    ),
                    place.spacing.edge,
                );
                if let Some(slot) = out.get_mut(*edge) {
                    if source {
                        slot.0 = along;
                    } else {
                        slot.1 = along;
                    }
                }
            }
        }
    }
    pin_ends(place, pins, reversed, &mut out);
    super::align::straighten(place, pins, reversed, &mut out);
    out
}

/// Move pinned ends onto the place the caller asked for — after the spreading,
/// not instead of it, so one pinned wire does not cost the rest their ordering.
/// The chain's ends are read rather than the caller's edge, because the cycle
/// break may have turned it round.
fn pin_ends(
    place: &Placement,
    pins: &[crate::layout::Port],
    reversed: &[bool],
    out: &mut [(f64, f64)],
) {
    for pin in pins {
        let Some(chain) = place.layering.chains.get(pin.edge) else {
            continue;
        };
        let (Some(first), Some(last)) = (chain.first().copied(), chain.last().copied()) else {
            continue;
        };
        let flipped = reversed.get(pin.edge).copied().unwrap_or(false);
        // The end the caller named, after any turn the cycle break made.
        let at_start = pin.source != flipped;
        let node = if at_start { first } else { last };
        let width = place
            .layering
            .nodes
            .get(node)
            .map_or(0.0, |found| found.size.width);
        let along = place.x_of(node) + (pin.at.clamp(0.0, 1.0) - 0.5) * width;
        if let Some(slot) = out.get_mut(pin.edge) {
            if at_start {
                slot.0 = along;
            } else {
                slot.1 = along;
            }
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
    use crate::layout::types::{Edge, Node, Spacing};
    use crate::layout::{Layering, Table};

    /// One laid-out graph, held so a `Placement` can borrow from it.
    struct Case {
        layering: Layering,
        centres: Table<f64>,
        tops: Vec<(f64, f64)>,
        spacing: Spacing,
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
        Case {
            layering,
            centres,
            tops,
            spacing,
        }
    }

    #[test]
    fn a_leg_standing_clear_of_every_column_does_not_move() {
        assert!((clear_of(50.0, &[10.0, 90.0], (0.0, 100.0), 12.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_leg_between_two_columns_steps_outside_the_nearer_one() {
        // The state diagram's arrival leg, moved to the origin: it stood 5px
        // from one column and 7 from the next, in a gap the columns themselves
        // keep 12 across. Only outside the pair is there room for a third line.
        let moved = clear_of(49.7, &[44.7, 56.7], (0.0, 100.0), 12.0);
        assert!((moved - 32.7).abs() < 1e-9, "{moved}");
    }

    #[test]
    fn a_leg_steps_the_shorter_way_when_either_side_would_do() {
        let moved = clear_of(48.0, &[44.0], (0.0, 100.0), 12.0);
        assert!((moved - 56.0).abs() < 1e-9, "{moved}");
    }

    #[test]
    fn a_leg_with_nowhere_to_stand_stays_where_it_was() {
        // A face narrower than the clearance. Standing close to a column is bad;
        // standing off the box the edge has to meet is worse.
        assert!((clear_of(50.0, &[50.0], (49.0, 51.0), 12.0) - 50.0).abs() < 1e-9);
        // And a node too narrow to have a face at all is not asked the question.
        assert!((clear_of(50.0, &[50.0], (51.0, 49.0), 12.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_port_leg_is_told_which_columns_it_has_to_pass() {
        let case = routed(3, &[(0, 1), (1, 2), (0, 2)]);
        let place = Placement {
            layering: &case.layering,
            centres: &case.centres,
            tops: &case.tops,
            spacing: &case.spacing,
        };
        // `0 -> 2` skips a layer, so it stands a dummy in the middle band: a
        // column that the leg arriving at node 2 along `1 -> 2` has to clear.
        assert_eq!(crossing(&place, 2, 1, false).len(), 1);
        // An edge's own dummy is not in its way — it is where the edge is going.
        assert!(crossing(&place, 2, 2, false).is_empty());
        // Nothing arrives at the first layer from above, there being no above.
        assert!(crossing(&place, 0, 0, false).is_empty());
    }
}
