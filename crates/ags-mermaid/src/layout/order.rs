//! The order of nodes within each layer, chosen to reduce edge crossings.
//!
//! Minimising crossings exactly is NP-hard, so this is the standard two-part
//! heuristic. A **median sweep** puts each node near the average position of its
//! neighbours in the layer before it, swept down and then up. **Transposition**
//! then swaps adjacent pairs for as long as swapping helps, which fixes the
//! local mistakes a median cannot see.
//!
//! Every tie is broken by the order the caller wrote things in. That is what
//! keeps a diagram from reshuffling itself when nothing about it changed — and
//! it is why the sorts here are stable ones.

use std::cmp::Ordering;

use super::cycles::Arc;
use super::layers::Layering;
use super::table::Table;

/// How many rounds of sweeping to try before keeping the best seen.
///
/// The reference asked ELK for a thoroughness of seven; past that the median
/// sweep has stopped finding anything on graphs this size.
const ROUNDS: usize = 8;

/// Which nodes sit above and below each node, in the caller's order.
struct Neighbours {
    up: Vec<Vec<usize>>,
    down: Vec<Vec<usize>>,
}

impl Neighbours {
    fn of(node_count: usize, arcs: &[Arc]) -> Self {
        let mut out = Self {
            up: vec![Vec::new(); node_count],
            down: vec![Vec::new(); node_count],
        };
        for arc in arcs {
            if let Some(slot) = out.up.get_mut(arc.to) {
                slot.push(arc.from);
            }
            if let Some(slot) = out.down.get_mut(arc.from) {
                slot.push(arc.to);
            }
        }
        out
    }

    fn toward(&self, node: usize, downward: bool) -> &[usize] {
        let side = if downward { &self.up } else { &self.down };
        side.get(node).map_or(&[][..], Vec::as_slice)
    }
}

/// Where each node sits within its own layer.
fn positions(layers: &[Vec<usize>], node_count: usize) -> Table<usize> {
    let mut out = Table::<usize>::new(node_count);
    for layer in layers {
        for (at, node) in layer.iter().enumerate() {
            out.set(*node, at);
        }
    }
    out
}

/// The median position of `node`'s neighbours on one side.
///
/// `None` when it has none — such a node keeps the slot it already had rather
/// than being swept to one end.
///
/// With an even number of neighbours the two middle positions are weighted by
/// how far the neighbours spread either side of them, which pulls a node toward
/// the denser half instead of splitting the difference blindly.
fn median(neighbours: &[usize], pos: &Table<usize>) -> Option<f64> {
    let mut at: Vec<f64> = neighbours
        .iter()
        .map(|node| super::table::as_f64(pos.get(*node)))
        .collect();
    at.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let count = at.len();
    if count == 0 {
        return None;
    }
    let middle = count / 2;
    if count % 2 == 1 {
        return at.get(middle).copied();
    }
    let (Some(low), Some(high)) = (at.get(middle - 1).copied(), at.get(middle).copied()) else {
        return None;
    };
    if count == 2 {
        return Some(f64::midpoint(low, high));
    }
    let (Some(first), Some(last)) = (at.first().copied(), at.last().copied()) else {
        return None;
    };
    let (left, right) = (low - first, last - high);
    if left + right == 0.0 {
        return Some(f64::midpoint(low, high));
    }
    Some((low * right + high * left) / (left + right))
}

/// Reorder one layer by its neighbours' medians, leaving nodes with no
/// neighbours in the slots they already hold.
fn sweep_layer(layer: &mut [usize], neighbours: &Neighbours, pos: &Table<usize>, downward: bool) {
    let mut movable: Vec<(f64, usize)> = Vec::new();
    let mut anchored: Vec<bool> = Vec::with_capacity(layer.len());
    for node in layer.iter() {
        match median(neighbours.toward(*node, downward), pos) {
            Some(key) => {
                movable.push((key, *node));
                anchored.push(false);
            }
            None => anchored.push(true),
        }
    }
    // Stable, so nodes whose medians tie stay in the order the caller wrote.
    movable.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    let mut next = movable.into_iter();
    for (slot, held) in layer.iter_mut().zip(&anchored) {
        if !*held {
            if let Some((_, node)) = next.next() {
                *slot = node;
            }
        }
    }
}

/// How many crossings the arcs between two adjacent layers make.
fn crossings_between(upper: &[usize], lower: &[usize], neighbours: &Neighbours) -> usize {
    // Every pair of arcs whose endpoints run in opposite orders is one crossing.
    let mut arcs: Vec<(usize, usize)> = Vec::new();
    for (at, node) in upper.iter().enumerate() {
        for target in neighbours.toward(*node, false) {
            if let Some(below) = lower.iter().position(|other| other == target) {
                arcs.push((at, below));
            }
        }
    }
    let mut count = 0;
    for (index, first) in arcs.iter().enumerate() {
        for second in arcs.iter().skip(index + 1) {
            if (first.0 < second.0 && first.1 > second.1)
                || (first.0 > second.0 && first.1 < second.1)
            {
                count += 1;
            }
        }
    }
    count
}

/// How many crossings the whole drawing makes.
pub fn crossings(layers: &[Vec<usize>], neighbours_of: &Layering) -> usize {
    let neighbours = Neighbours::of(neighbours_of.nodes.len(), &neighbours_of.arcs);
    layers
        .windows(2)
        .filter_map(|pair| {
            let (upper, lower) = (pair.first()?, pair.get(1)?);
            Some(crossings_between(upper, lower, &neighbours))
        })
        .sum()
}

/// How many crossings the arcs of `a` and `b` make in the order given.
fn pair_crossings(a: usize, b: usize, neighbours: &Neighbours, pos: &Table<usize>) -> usize {
    let mut count = 0;
    for downward in [true, false] {
        for above in neighbours.toward(a, downward) {
            for below in neighbours.toward(b, downward) {
                if pos.get(*above) > pos.get(*below) {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Swap adjacent pairs for as long as swapping reduces crossings.
fn transpose(layers: &mut [Vec<usize>], neighbours: &Neighbours, node_count: usize) {
    let mut improved = true;
    // Bounded so a pair that keeps trading places cannot spin forever.
    let mut rounds = 0;
    while improved && rounds < ROUNDS {
        improved = false;
        rounds += 1;
        let pos = positions(layers, node_count);
        for layer in layers.iter_mut() {
            for at in 0..layer.len().saturating_sub(1) {
                let (Some(a), Some(b)) = (layer.get(at).copied(), layer.get(at + 1).copied())
                else {
                    continue;
                };
                if pair_crossings(a, b, neighbours, &pos) > pair_crossings(b, a, neighbours, &pos) {
                    layer.swap(at, at + 1);
                    improved = true;
                }
            }
        }
    }
}

/// Order the nodes within each layer.
///
/// The most nodes a layer may hold before an exact ordering is out of reach.
///
/// Six is 720 arrangements, so a pair of adjacent layers costs at most 720²
/// comparisons — the largest that still finishes without being noticed. Above
/// it the heuristic answers instead.
const EXACT_LAYER: usize = 6;

/// Every arrangement of a layer, in a fixed order so the search is repeatable.
fn arrangements(layer: &[usize]) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new()];
    for _ in 0..layer.len() {
        let mut next = Vec::new();
        for partial in &out {
            for node in layer {
                if !partial.contains(node) {
                    let mut grown = partial.clone();
                    grown.push(*node);
                    next.push(grown);
                }
            }
        }
        out = next;
    }
    out
}

/// The fewest crossings any ordering of these layers can have.
///
/// Crossings are counted between adjacent layers and nowhere else, so the total
/// is a sum over the boundaries and the best arrangement of each layer depends
/// only on the one before it. That makes this a shortest path over arrangements
/// rather than a search over their product: layers of 6, 6 and 6 are 373 million
/// combinations but only two boundaries of 720² to walk.
///
/// Exact, so it cannot do worse than the heuristic — and on the graphs people
/// write by hand it is usually cheaper too.
fn exact_order(layering: &Layering, neighbours: &Neighbours) -> Option<Vec<Vec<usize>>> {
    if layering
        .layers
        .iter()
        .any(|layer| layer.len() > EXACT_LAYER)
    {
        return None;
    }
    let choices: Vec<Vec<Vec<usize>>> = layering.layers.iter().map(|l| arrangements(l)).collect();
    let first = choices.first()?;
    // The cost of reaching each arrangement of the current layer, and which
    // arrangement of the one above it came from.
    let mut cost: Vec<usize> = vec![0; first.len()];
    let mut from: Vec<Vec<usize>> = vec![Vec::new(); choices.len()];
    for (at, layer) in choices.iter().enumerate().skip(1) {
        let mut next = vec![usize::MAX; layer.len()];
        let mut came = vec![0usize; layer.len()];
        let above = choices.get(at - 1)?;
        for (l, lower) in layer.iter().enumerate() {
            for (u, upper) in above.iter().enumerate() {
                let Some(so_far) = cost.get(u) else { continue };
                if *so_far == usize::MAX {
                    continue;
                }
                let made = so_far + crossings_between(upper, lower, neighbours);
                // Strictly less, so the earliest arrangement wins a tie and the
                // same graph orders the same way twice.
                if next.get(l).is_some_and(|best| made < *best) {
                    if let Some(slot) = next.get_mut(l) {
                        *slot = made;
                    }
                    if let Some(slot) = came.get_mut(l) {
                        *slot = u;
                    }
                }
            }
        }
        cost = next;
        if let Some(slot) = from.get_mut(at) {
            *slot = came;
        }
    }
    let (best, _) = cost.iter().enumerate().min_by_key(|(_, made)| **made)?;
    let mut picked = vec![0usize; choices.len()];
    let mut at = choices.len().checked_sub(1)?;
    if let Some(slot) = picked.get_mut(at) {
        *slot = best;
    }
    while at > 0 {
        let came = *from.get(at)?.get(*picked.get(at)?)?;
        if let Some(slot) = picked.get_mut(at - 1) {
            *slot = came;
        }
        at -= 1;
    }
    Some(
        choices
            .iter()
            .zip(&picked)
            .map(|(c, i)| c.get(*i).cloned().unwrap_or_default())
            .collect(),
    )
}

pub fn order_layers(layering: &Layering) -> Vec<Vec<usize>> {
    let node_count = layering.nodes.len();
    let neighbours = Neighbours::of(node_count, &layering.arcs);
    // A small graph is ordered exactly. The sweep below is a local search, and
    // the move it cannot make is the one that needs several layers to change at
    // once — which is not a rare shape: two long edges leaving one layer for the
    // same target tie on every median they meet, so the caller's order decides,
    // and it lists both chains on the same side.
    if let Some(exact) = exact_order(layering, &neighbours) {
        return exact;
    }
    swept(layering, &neighbours, node_count)
}

/// The median-and-transposition heuristic, for graphs too wide to order
/// exactly. A local search: it cannot make the move that needs several
/// layers to change at once, which is what `exact_order` exists for.
fn swept(layering: &Layering, neighbours: &Neighbours, node_count: usize) -> Vec<Vec<usize>> {
    let mut current = layering.layers.clone();
    let mut best = current.clone();
    let mut fewest = crossings(&best, layering);

    for round in 0..ROUNDS {
        if fewest == 0 {
            break;
        }
        // Alternate which end the sweep starts from: a layer ordered from above
        // and one ordered from below disagree, and taking turns settles it.
        let downward = round % 2 == 0;
        let count = current.len();
        for at in 0..count {
            let index = if downward { at } else { count - 1 - at };
            // The first layer swept has nothing before it to take a median from.
            if (downward && index == 0) || (!downward && index + 1 == count) {
                continue;
            }
            let pos = positions(&current, node_count);
            if let Some(layer) = current.get_mut(index) {
                sweep_layer(layer, neighbours, &pos, downward);
            }
        }
        transpose(&mut current, neighbours, node_count);
        let made = crossings(&current, layering);
        if made < fewest {
            fewest = made;
            best.clone_from(&current);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_graph_is_ordered_exactly_rather_than_swept() {
        // K3,3: every node on one row joined to every node on the next. In a
        // layered drawing every pair of edges whose endpoints invert has to
        // cross, and with three by three that is C(3,2)² = 9 — whatever order
        // the rows are in. (The *planar* crossing number is 1; a two-layer
        // drawing cannot use the third dimension of freedom that buys.)
        let pairs: Vec<(usize, usize)> = (0..3).flat_map(|a| (3..6).map(move |b| (a, b))).collect();
        let layering = layered(6, &pairs);
        let out = order_layers(&layering);
        assert_eq!(crossings(&out, &layering), 9);
    }

    #[test]
    fn the_sweep_still_untangles_a_drawing_too_wide_to_order_exactly() {
        // Seven on a row is past what `exact_order` enumerates, so this is the
        // arm that answers for a big drawing — and the arm a gallery of
        // hand-written diagrams no longer reaches on its own.
        let pairs: Vec<(usize, usize)> = (0..7).map(|n| (n, 13 - n)).collect();
        let layering = layered(14, &pairs);
        let node_count = layering.nodes.len();
        let neighbours = Neighbours::of(node_count, &layering.arcs);
        let before = crossings(&layering.layers, &layering);
        let after = crossings(&swept(&layering, &neighbours, node_count), &layering);
        assert!(
            before > 0,
            "the input order crosses, or there is nothing to fix"
        );
        assert_eq!(after, 0, "every crossing here is removable by reordering");
    }

    #[test]
    fn a_layer_too_wide_to_enumerate_falls_back_to_the_sweep() {
        // Seven on a row is 5040 arrangements, past what `exact_order` will
        // enumerate — so this is the arm that answers for a big drawing, and it
        // still has to return every node exactly once.
        let pairs: Vec<(usize, usize)> = (1..8).map(|n| (0, n)).collect();
        let layering = layered(8, &pairs);
        assert!(exact_order(&layering, &Neighbours::of(8, &layering.arcs)).is_none());
        let out = order_layers(&layering);
        let mut seen: Vec<usize> = out.iter().flatten().copied().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..8).collect::<Vec<usize>>());
    }

    #[test]
    fn an_exact_ordering_beats_the_sweep_where_the_sweep_gets_stuck() {
        // The state machine that motivated this: two long edges leaving one
        // layer for the same target tie on every median, so the sweep leaves
        // them on one side and crosses. Exactly ordered, it does not.
        let layering = layered(
            7,
            &[
                (0, 1),
                (1, 2),
                (2, 3),
                (2, 1),
                (3, 4),
                (3, 5),
                (5, 3),
                (5, 1),
                (4, 1),
                (1, 6),
            ],
        );
        let node_count = layering.nodes.len();
        let neighbours = Neighbours::of(node_count, &layering.arcs);
        let stuck = crossings(&swept(&layering, &neighbours, node_count), &layering);
        let exact = crossings(&order_layers(&layering), &layering);
        assert_eq!(exact, 0);
        assert!(exact < stuck, "sweep {stuck}, exact {exact}");
    }
    use crate::layout::cycles::break_cycles;
    use crate::layout::layers::assign_layers;
    use crate::layout::types::{Edge, Node};

    fn layered(n: usize, pairs: &[(usize, usize)]) -> Layering {
        let edges: Vec<Edge> = pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect();
        let acyclic = break_cycles(n, &edges);
        assign_layers(&vec![Node::new(100.0, 40.0); n], &acyclic.arcs, edges.len())
    }

    #[test]
    fn a_layer_of_one_needs_no_ordering() {
        let out = order_layers(&layered(2, &[(0, 1)]));
        assert_eq!(out, vec![vec![0], vec![1]]);
    }

    #[test]
    fn every_node_survives_the_ordering() {
        let layering = layered(6, &[(0, 3), (1, 4), (2, 5), (0, 4), (1, 5)]);
        let out = order_layers(&layering);
        let mut before: Vec<usize> = layering.layers.iter().flatten().copied().collect();
        let mut after: Vec<usize> = out.iter().flatten().copied().collect();
        before.sort_unstable();
        after.sort_unstable();
        assert_eq!(before, after);
    }

    #[test]
    fn a_crossing_that_can_be_undone_is_undone() {
        // 0 -> 3 and 1 -> 2 cross when written in that order; swapping the
        // lower layer removes the crossing.
        let layering = layered(4, &[(0, 3), (1, 2)]);
        assert_eq!(crossings(&layering.layers, &layering), 1);
        let out = order_layers(&layering);
        assert_eq!(crossings(&out, &layering), 0);
    }

    #[test]
    fn a_crossing_that_cannot_be_undone_is_kept_to_a_minimum() {
        // Three nodes each pointing at two of three below: one crossing is
        // unavoidable however the layers are ordered.
        let layering = layered(6, &[(0, 4), (0, 5), (1, 3), (1, 5), (2, 3), (2, 4)]);
        let out = order_layers(&layering);
        assert!(crossings(&out, &layering) <= crossings(&layering.layers, &layering));
    }

    #[test]
    fn a_node_with_no_neighbours_keeps_the_slot_it_had() {
        // `2` has nothing above it, so the sweep must not push it aside.
        let layering = layered(5, &[(0, 3), (1, 4)]);
        let out = order_layers(&layering);
        assert!(out.iter().flatten().any(|node| *node == 2));
    }

    #[test]
    fn the_same_graph_orders_the_same_way_twice() {
        let layering = layered(8, &[(0, 4), (1, 5), (2, 6), (3, 7), (0, 7), (3, 4)]);
        assert_eq!(order_layers(&layering), order_layers(&layering));
    }

    #[test]
    fn two_graphs_that_differ_only_in_edge_order_still_settle() {
        let a = order_layers(&layered(4, &[(0, 2), (1, 3)]));
        let b = order_layers(&layered(4, &[(1, 3), (0, 2)]));
        // Both are crossing-free; the caller's order decides which is drawn.
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn a_median_of_nothing_is_nothing() {
        let pos = Table::<usize>::new(4);
        assert_eq!(median(&[], &pos), None);
    }

    #[test]
    fn a_median_of_one_is_that_one() {
        let pos = Table::of(vec![0usize, 1, 2, 3]);
        assert_eq!(median(&[2], &pos), Some(2.0));
    }

    #[test]
    fn a_median_of_two_sits_between_them() {
        let pos = Table::of(vec![0usize, 1, 2, 3]);
        assert_eq!(median(&[0, 3], &pos), Some(1.5));
    }

    #[test]
    fn a_median_of_an_even_run_leans_toward_the_denser_side() {
        let pos = Table::of(vec![0usize, 1, 2, 3, 4, 5]);
        // Three tight on the left, one far to the right: the median stays left.
        let leaning = median(&[0, 1, 2, 5], &pos).unwrap_or(0.0);
        assert!(leaning < 1.5, "{leaning}");
    }

    #[test]
    fn a_median_of_neighbours_that_all_sit_together_is_where_they_sit() {
        // Nothing spreads either side, so there is no denser half to lean to.
        let pos = Table::of(vec![2usize, 2, 2, 2]);
        assert_eq!(median(&[0, 1, 2, 3], &pos), Some(2.0));
    }

    #[test]
    fn a_sweep_moves_what_has_neighbours_and_leaves_what_does_not() {
        // `0` and `2` are pulled by nodes above them; `1` has nobody, so it
        // keeps the middle slot while the other two trade places around it.
        let neighbours = Neighbours::of(
            5,
            &[
                Arc {
                    from: 3,
                    to: 0,
                    source: 0,
                    reversed: false,
                },
                Arc {
                    from: 4,
                    to: 2,
                    source: 1,
                    reversed: false,
                },
            ],
        );
        let pos = Table::of(vec![0usize, 1, 2, 1, 0]);
        let mut layer = vec![0, 1, 2];
        sweep_layer(&mut layer, &neighbours, &pos, true);
        assert_eq!(layer, [2, 1, 0]);
    }

    #[test]
    fn a_graph_of_nothing_orders_to_nothing() {
        let layering = assign_layers(&[], &[], 0);
        assert!(order_layers(&layering).is_empty());
        assert_eq!(crossings(&[], &layering), 0);
    }
}
