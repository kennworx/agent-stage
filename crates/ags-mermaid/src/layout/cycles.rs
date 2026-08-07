//! Making the graph acyclic, and remembering what that cost.
//!
//! Layering needs every edge to point forwards, which a cycle makes impossible.
//! The usual answer is to reverse the smallest set of edges that breaks every
//! cycle; finding the *smallest* such set is NP-hard, so this does what every
//! layered engine does and reverses the back edges a depth-first walk finds.
//!
//! Each reversal is recorded. The drawing restores it at the end, so an
//! arrowhead points where the source said it did rather than where the layering
//! found it convenient.

use super::table::Table;
use super::types::Edge;

/// An edge as the layering sees it: always pointing forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arc {
    pub from: usize,
    pub to: usize,
    /// Which of the caller's edges this came from.
    pub source: usize,
    /// Whether it points the opposite way to the edge that produced it.
    pub reversed: bool,
}

/// A graph with its cycles broken.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Acyclic {
    /// Every edge that takes part in layering, in the caller's order.
    pub arcs: Vec<Arc>,
    /// The caller's edges joining a node to itself. They never reach layering —
    /// a self-loop has no direction to run in — and are routed on their own.
    pub loops: Vec<usize>,
}

/// How far a walk has got through one node's out-edges.
struct Frame {
    node: usize,
    next: usize,
}

/// Where a node stands in the walk.
const UNSEEN: u8 = 0;
const ON_STACK: u8 = 1;
const DONE: u8 = 2;

/// Each node's out-edges, by their position in the caller's list.
fn out_edges(node_count: usize, edges: &[Edge]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut loops = Vec::new();
    for (at, edge) in edges.iter().enumerate() {
        if edge.from >= node_count || edge.to >= node_count {
            // An edge naming a box nobody declared is dropped, so one bad line
            // does not stop the rest of the diagram drawing.
            continue;
        }
        if edge.from == edge.to {
            loops.push(at);
            continue;
        }
        if let Some(slot) = out.get_mut(edge.from) {
            slot.push(at);
        }
    }
    (out, loops)
}

/// Break every cycle by reversing the back edges a depth-first walk finds.
///
/// Roots are taken in index order and out-edges in source order, so the same
/// graph breaks the same way every time.
///
/// Index order matters more than it looks: the walk turns round whichever edge
/// closes a cycle *from where it started*, so beginning halfway down a flow calls
/// a perfectly good forward edge a back edge. Callers that care put the node the
/// reading starts from first — see `flowchart::nest`, where a subgraph is ordered
/// by where its contents begin for exactly this reason.
pub fn break_cycles(node_count: usize, edges: &[Edge]) -> Acyclic {
    let (out, loops) = out_edges(node_count, edges);
    let mut state = Table::<u8>::new(node_count);
    let mut arcs: Vec<Arc> = Vec::with_capacity(edges.len());

    for root in 0..node_count {
        if state.get(root) != UNSEEN {
            continue;
        }
        state.set(root, ON_STACK);
        let mut stack = vec![Frame {
            node: root,
            next: 0,
        }];
        while let Some(&Frame { node, next }) = stack.last() {
            let Some(at) = out.get(node).and_then(|edges| edges.get(next)).copied() else {
                state.set(node, DONE);
                stack.pop();
                continue;
            };
            if let Some(frame) = stack.last_mut() {
                frame.next += 1;
            }
            let Some(edge) = edges.get(at) else { continue };
            let to = edge.to;
            match state.get(to) {
                // Back to a node the walk is still inside: this closes a cycle,
                // so it is the one to turn around.
                ON_STACK => arcs.push(Arc {
                    from: to,
                    to: node,
                    source: at,
                    reversed: true,
                }),
                DONE => arcs.push(Arc {
                    from: node,
                    to,
                    source: at,
                    reversed: false,
                }),
                _ => {
                    arcs.push(Arc {
                        from: node,
                        to,
                        source: at,
                        reversed: false,
                    });
                    state.set(to, ON_STACK);
                    stack.push(Frame { node: to, next: 0 });
                }
            }
        }
    }

    // Back into the caller's order, so every later pass can use source order as
    // its tie-break without first having to recover it.
    arcs.sort_by_key(|arc| arc.source);
    Acyclic { arcs, loops }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(pairs: &[(usize, usize)]) -> Vec<Edge> {
        pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect()
    }

    fn pairs(out: &Acyclic) -> Vec<(usize, usize, bool)> {
        out.arcs
            .iter()
            .map(|arc| (arc.from, arc.to, arc.reversed))
            .collect()
    }

    #[test]
    fn an_acyclic_graph_is_left_alone() {
        let out = break_cycles(3, &edges(&[(0, 1), (1, 2), (0, 2)]));
        assert_eq!(pairs(&out), [(0, 1, false), (1, 2, false), (0, 2, false)]);
        assert!(out.loops.is_empty());
    }

    #[test]
    fn a_cycle_has_exactly_one_edge_turned_around() {
        let out = break_cycles(3, &edges(&[(0, 1), (1, 2), (2, 0)]));
        let turned: Vec<&Arc> = out.arcs.iter().filter(|arc| arc.reversed).collect();
        assert_eq!(turned.len(), 1);
        // The walk reaches 2 from 0, so `2 -> 0` is the edge that closes it.
        assert_eq!((turned[0].from, turned[0].to), (0, 2));
        assert_eq!(turned[0].source, 2);
    }

    #[test]
    fn two_nodes_pointing_at_each_other_keep_one_direction() {
        let out = break_cycles(2, &edges(&[(0, 1), (1, 0)]));
        assert_eq!(pairs(&out), [(0, 1, false), (0, 1, true)]);
    }

    #[test]
    fn every_arc_points_forwards_once_the_cycles_are_broken() {
        // Two cycles sharing a node. The proof that none survives is that a
        // topological order exists, which Kahn's method finds only when it does.
        let out = break_cycles(4, &edges(&[(0, 1), (1, 2), (2, 0), (2, 3), (3, 1)]));
        let mut incoming = Table::<usize>::new(4);
        for arc in &out.arcs {
            incoming.update(arc.to, |n| n + 1);
        }
        let mut ready: Vec<usize> = (0..4).filter(|n| incoming.get(*n) == 0).collect();
        let mut placed = 0;
        while let Some(node) = ready.pop() {
            placed += 1;
            for arc in out.arcs.iter().filter(|arc| arc.from == node) {
                incoming.update(arc.to, |n| n.saturating_sub(1));
                if incoming.get(arc.to) == 0 {
                    ready.push(arc.to);
                }
            }
        }
        assert_eq!(placed, 4, "every node ordered, so no cycle is left");
    }

    #[test]
    fn a_self_loop_never_reaches_the_layering() {
        let out = break_cycles(2, &edges(&[(0, 0), (0, 1)]));
        assert_eq!(out.loops, [0]);
        assert_eq!(pairs(&out), [(0, 1, false)]);
    }

    #[test]
    fn an_edge_naming_a_box_that_does_not_exist_is_dropped() {
        let out = break_cycles(2, &edges(&[(0, 1), (0, 7), (9, 1)]));
        assert_eq!(pairs(&out), [(0, 1, false)]);
        assert!(out.loops.is_empty());
    }

    #[test]
    fn the_arcs_come_back_in_the_order_the_caller_gave_them() {
        let out = break_cycles(4, &edges(&[(2, 3), (0, 1), (1, 2)]));
        assert_eq!(
            out.arcs
                .iter()
                .map(|arc| arc.source)
                .collect::<Vec<usize>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn a_graph_of_nothing_breaks_nothing() {
        assert_eq!(break_cycles(0, &[]), Acyclic::default());
        assert_eq!(break_cycles(3, &[]), Acyclic::default());
    }

    #[test]
    fn two_walks_of_the_same_graph_break_it_the_same_way() {
        let source = edges(&[(0, 1), (1, 2), (2, 0), (0, 3), (3, 1)]);
        assert_eq!(break_cycles(4, &source), break_cycles(4, &source));
    }
}
