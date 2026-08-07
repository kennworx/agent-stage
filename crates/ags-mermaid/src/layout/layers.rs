//! Which layer each node sits in, and the dummies that make every edge short.
//!
//! Two passes. Longest path puts each node as early as its predecessors allow,
//! which is the fewest layers the graph can have. Tightening then pushes each
//! node back down to just above its earliest successor, which shortens its
//! incoming edges without adding a layer — the difference between a source node
//! floating at the top of the drawing and sitting beside the thing it feeds.
//!
//! Then every edge spanning more than one layer is broken into a chain of
//! dummies, one per layer it crosses. After this the layering is *proper*:
//! every arc joins adjacent layers, which is what lets the ordering and
//! placement passes work a layer at a time.

use super::cycles::Arc;
use super::table::Table;
use super::types::Node;

/// One node in the layered graph — a box the caller gave, or a bend in an edge.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutNode {
    pub size: Node,
    /// The caller's node index, or `None` for a dummy.
    pub real: Option<usize>,
    pub layer: usize,
}

impl LayoutNode {
    pub const fn is_dummy(&self) -> bool {
        self.real.is_none()
    }
}

/// A proper layering: every arc joins one layer to the next.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layering {
    pub nodes: Vec<LayoutNode>,
    /// Arcs between adjacent layers, indexing into `nodes`.
    pub arcs: Vec<Arc>,
    /// The nodes in each layer, in the order they were added.
    pub layers: Vec<Vec<usize>>,
    /// For each of the caller's edges, the chain of nodes it runs through —
    /// source, its dummies, target. Empty for an edge that was dropped.
    pub chains: Vec<Vec<usize>>,
}

/// A topological order of `node_count` nodes under `arcs`, by Kahn's method.
///
/// Ties are broken by node index, so the order is the same every run.
fn topological(node_count: usize, arcs: &[Arc]) -> Vec<usize> {
    let mut incoming = Table::<usize>::new(node_count);
    for arc in arcs {
        incoming.update(arc.to, |n| n + 1);
    }
    let mut ready: Vec<usize> = (0..node_count).filter(|n| incoming.get(*n) == 0).collect();
    let mut out = Vec::with_capacity(node_count);
    while let Some(node) = ready.pop() {
        out.push(node);
        for arc in arcs.iter().filter(|arc| arc.from == node) {
            incoming.update(arc.to, |n| n.saturating_sub(1));
            if incoming.get(arc.to) == 0 {
                ready.push(arc.to);
            }
        }
        // Smallest index first, so the walk is reproducible. Sorting the ready
        // set rather than using a heap keeps the whole pass allocation-free of
        // anything but these two vectors.
        ready.sort_unstable_by(|a, b| b.cmp(a));
    }
    // A cycle would leave nodes unreached. `break_cycles` runs first, so this
    // only catches an arc list assembled by hand.
    for node in 0..node_count {
        if !out.contains(&node) {
            out.push(node);
        }
    }
    out
}

/// The layer of each node: as early as its predecessors allow.
fn longest_path(node_count: usize, arcs: &[Arc], order: &[usize]) -> Table<usize> {
    let mut layer = Table::<usize>::new(node_count);
    for node in order {
        for arc in arcs.iter().filter(|arc| arc.to == *node) {
            let after = layer.get(arc.from) + 1;
            if after > layer.get(*node) {
                layer.set(*node, after);
            }
        }
    }
    layer
}

/// Push each node down to just above its earliest successor.
fn tighten(arcs: &[Arc], order: &[usize], layer: &mut Table<usize>) {
    for node in order.iter().rev() {
        let earliest = arcs
            .iter()
            .filter(|arc| arc.from == *node)
            .map(|arc| layer.get(arc.to))
            .min();
        // A node with nothing after it is already as late as it can be — moving
        // it would stretch the drawing rather than tighten it.
        if let Some(earliest) = earliest {
            layer.set(*node, earliest.saturating_sub(1));
        }
    }
}

/// Lay out the layers of a graph whose cycles are already broken.
pub fn assign_layers(sizes: &[Node], arcs: &[Arc], edge_count: usize) -> Layering {
    let node_count = sizes.len();
    let order = topological(node_count, arcs);
    let mut layer = longest_path(node_count, arcs, &order);
    tighten(arcs, &order, &mut layer);

    let depth = layer.iter().copied().max().map_or(0, |max| max + 1);
    let mut out = Layering {
        nodes: (0..node_count)
            .map(|at| LayoutNode {
                size: sizes.get(at).copied().unwrap_or_default(),
                real: Some(at),
                layer: layer.get(at),
            })
            .collect(),
        arcs: Vec::with_capacity(arcs.len()),
        layers: vec![Vec::new(); depth],
        chains: vec![Vec::new(); edge_count],
    };
    for at in 0..node_count {
        if let Some(slot) = out.layers.get_mut(layer.get(at)) {
            slot.push(at);
        }
    }
    for arc in arcs {
        add_chain(&mut out, *arc);
    }
    out
}

/// Break one arc into a chain of adjacent-layer arcs, adding a dummy per layer
/// it crosses.
fn add_chain(out: &mut Layering, arc: Arc) {
    let (Some(from), Some(to)) = (out.nodes.get(arc.from), out.nodes.get(arc.to)) else {
        return;
    };
    let (first, last) = (from.layer, to.layer);
    let mut chain = vec![arc.from];
    let mut previous = arc.from;
    // A node whose successor shares its layer has nowhere to bend, so the arc
    // stays as it is and the ordering pass keeps the two apart.
    for layer in (first + 1)..last {
        let dummy = out.nodes.len();
        out.nodes.push(LayoutNode {
            size: Node::default(),
            real: None,
            layer,
        });
        if let Some(slot) = out.layers.get_mut(layer) {
            slot.push(dummy);
        }
        out.arcs.push(Arc {
            from: previous,
            to: dummy,
            source: arc.source,
            reversed: arc.reversed,
        });
        chain.push(dummy);
        previous = dummy;
    }
    out.arcs.push(Arc {
        from: previous,
        to: arc.to,
        source: arc.source,
        reversed: arc.reversed,
    });
    chain.push(arc.to);
    if let Some(slot) = out.chains.get_mut(arc.source) {
        *slot = chain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cycles::break_cycles;
    use crate::layout::types::Edge;

    fn boxes(n: usize) -> Vec<Node> {
        vec![Node::new(100.0, 40.0); n]
    }

    fn layered(n: usize, pairs: &[(usize, usize)]) -> Layering {
        let edges: Vec<Edge> = pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect();
        let acyclic = break_cycles(n, &edges);
        assign_layers(&boxes(n), &acyclic.arcs, edges.len())
    }

    fn layer_of(out: &Layering, node: usize) -> usize {
        out.nodes.get(node).map_or(0, |n| n.layer)
    }

    #[test]
    fn a_chain_takes_one_layer_per_step() {
        let out = layered(3, &[(0, 1), (1, 2)]);
        assert_eq!(
            (layer_of(&out, 0), layer_of(&out, 1), layer_of(&out, 2)),
            (0, 1, 2)
        );
        assert_eq!(out.layers.len(), 3);
    }

    #[test]
    fn nodes_with_nothing_before_them_share_the_first_layer() {
        let out = layered(3, &[(0, 2), (1, 2)]);
        assert_eq!(layer_of(&out, 0), 0);
        assert_eq!(layer_of(&out, 1), 0);
        assert_eq!(layer_of(&out, 2), 1);
    }

    #[test]
    fn a_node_is_pushed_down_to_meet_the_thing_it_feeds() {
        // Without tightening `a` would sit in layer 0, two layers above the only
        // node it feeds, leaving a long edge across an empty row.
        let out = layered(4, &[(0, 1), (1, 2), (3, 2)]);
        assert_eq!(layer_of(&out, 2), 2);
        assert_eq!(layer_of(&out, 3), 1, "pulled down beside `1`");
    }

    #[test]
    fn an_edge_across_two_layers_gets_a_bend_in_the_middle() {
        let out = layered(3, &[(0, 1), (1, 2), (0, 2)]);
        // The long edge is the third one.
        let chain = out.chains.get(2).cloned().unwrap_or_default();
        assert_eq!(chain.len(), 3, "source, one dummy, target");
        let dummy = chain.get(1).copied().unwrap_or_default();
        assert!(out.nodes.get(dummy).is_some_and(LayoutNode::is_dummy));
        assert_eq!(layer_of(&out, dummy), 1);
    }

    #[test]
    fn every_arc_joins_one_layer_to_the_next() {
        let out = layered(5, &[(0, 1), (1, 2), (2, 3), (0, 3), (0, 4), (4, 3)]);
        for arc in &out.arcs {
            assert_eq!(
                layer_of(&out, arc.to),
                layer_of(&out, arc.from) + 1,
                "{arc:?}"
            );
        }
    }

    #[test]
    fn a_short_edge_needs_no_bend() {
        let out = layered(2, &[(0, 1)]);
        assert_eq!(out.chains.first().map(Vec::len), Some(2));
        assert!(out.nodes.iter().all(|node| !node.is_dummy()));
    }

    #[test]
    fn every_node_appears_in_exactly_one_layer() {
        let out = layered(5, &[(0, 1), (1, 2), (2, 3), (0, 4)]);
        let mut seen = Table::<usize>::new(out.nodes.len());
        for layer in &out.layers {
            for node in layer {
                seen.update(*node, |n| n + 1);
            }
        }
        assert!(seen.iter().all(|count| *count == 1));
    }

    #[test]
    fn a_cycle_still_layers_once_it_is_broken() {
        let out = layered(3, &[(0, 1), (1, 2), (2, 0)]);
        for arc in &out.arcs {
            assert_eq!(layer_of(&out, arc.to), layer_of(&out, arc.from) + 1);
        }
    }

    #[test]
    fn a_graph_of_islands_puts_them_all_in_the_first_layer() {
        let out = layered(3, &[]);
        assert_eq!(out.layers.len(), 1);
        assert_eq!(out.layers.first().map(Vec::len), Some(3));
        assert!(out.chains.iter().all(Vec::is_empty));
    }

    #[test]
    fn a_graph_of_nothing_lays_out_to_nothing() {
        let out = assign_layers(&[], &[], 0);
        assert_eq!(out, Layering::default());
    }

    #[test]
    fn a_dummy_takes_up_no_room() {
        let out = layered(3, &[(0, 1), (1, 2), (0, 2)]);
        for node in out.nodes.iter().filter(|node| node.is_dummy()) {
            assert!((node.size.width - 0.0).abs() < 1e-9);
            assert!((node.size.height - 0.0).abs() < 1e-9);
        }
    }
}
