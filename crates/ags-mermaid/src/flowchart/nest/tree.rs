//! Which container holds what, and which one draws each edge.
//!
//! Pure topology: nothing here has a size or a coordinate, which is what lets a
//! parent answer questions about its children before any of them is laid out.

use crate::flowchart::types::Graph;

/// One thing a container holds directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Member {
    Node(usize),
    Group(usize),
}

pub(super) struct Tree {
    /// The group directly holding each node.
    pub(super) of_node: Vec<Option<usize>>,
    /// The group directly holding each group.
    pub(super) of_group: Vec<Option<usize>>,
}

impl Tree {
    pub(super) fn of(graph: &Graph) -> Self {
        let mut of_node = vec![None; graph.nodes.len()];
        let mut of_group = vec![None; graph.groups.len()];
        for (at, group) in graph.groups.iter().enumerate() {
            for id in &group.nodes {
                if let Some(node) = graph.index_of(id) {
                    if let Some(slot) = of_node.get_mut(node) {
                        *slot = Some(at);
                    }
                }
            }
            for child in &group.groups {
                if let Some(slot) = of_group.get_mut(*child) {
                    *slot = Some(at);
                }
            }
        }
        Self { of_node, of_group }
    }

    /// Every container from `start` outward, innermost first, ending at the
    /// drawing.
    ///
    /// The drawing is `None` and is always last, which is what makes "is the
    /// router an ancestor of this container" answerable without a special case.
    pub(super) fn upward(&self, start: Option<usize>) -> Vec<Option<usize>> {
        let mut out = Vec::new();
        let mut here = start;
        // Bounded: a group cannot hold itself, so the walk cannot revisit one.
        for _ in 0..=self.of_group.len() {
            out.push(here);
            match here {
                None => return out,
                Some(group) => here = self.of_group.get(group).copied().flatten(),
            }
        }
        out.push(None);
        out
    }

    /// Every container holding a node, innermost first, ending at the drawing.
    pub(super) fn chain(&self, node: usize) -> Vec<Option<usize>> {
        self.upward(self.of_node.get(node).copied().flatten())
    }

    /// Every container holding a group — not including the group itself.
    pub(super) fn over(&self, group: usize) -> Vec<Option<usize>> {
        self.upward(self.of_group.get(group).copied().flatten())
    }
}

/// The container that draws each edge whole: the innermost holding both ends.
pub(super) fn router(tree: &Tree, graph: &Graph, edge: usize) -> Option<usize> {
    let found = graph.edges.get(edge)?;
    let ends = (graph.index_of(&found.source), graph.index_of(&found.target));
    let (Some(source), Some(target)) = ends else {
        return None;
    };
    let theirs = tree.chain(target);
    tree.chain(source)
        .into_iter()
        .find(|holder| theirs.contains(holder))
        .flatten()
}

/// The direct member of `container` on the way to `node`, if it holds it at all.
pub(super) fn toward(tree: &Tree, container: Option<usize>, node: usize) -> Option<Member> {
    let chain = tree.chain(node);
    let at = chain.iter().position(|holder| *holder == container)?;
    match at {
        // Directly inside.
        0 => Some(Member::Node(node)),
        // Inside a group that is inside this one.
        _ => chain.get(at - 1).copied().flatten().map(Member::Group),
    }
}

/// The members of a container, in the order their contents begin.
///
/// Reading order, not "nodes then groups": the cycle break turns round whichever
/// edge closes a cycle from where its walk started, so a container that lists its
/// groups last starts the walk halfway down its own flow. Ordering by where each
/// member's contents first appear in the source is what the flat layout did for
/// free, because there a group simply *was* its nodes.
pub(super) fn members(graph: &Graph, tree: &Tree, container: Option<usize>) -> Vec<Member> {
    let mut out: Vec<(usize, Member)> = Vec::new();
    for node in 0..graph.nodes.len() {
        if tree.of_node.get(node).copied().flatten() == container {
            out.push((node, Member::Node(node)));
        }
    }
    for group in 0..graph.groups.len() {
        if tree.of_group.get(group).copied().flatten() != container {
            continue;
        }
        // A group begins where the first of its contents does.
        let first = (0..graph.nodes.len())
            .filter(|node| tree.chain(*node).contains(&Some(group)))
            .min()
            .unwrap_or(usize::MAX);
        out.push((first, Member::Group(group)));
    }
    out.sort_by_key(|(first, _)| *first);
    out.into_iter().map(|(_, member)| member).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowchart::parse;

    /// A group inside a group, with one edge reaching in from outside it.
    const NESTED: &str = "graph TD\n  subgraph outer\n    subgraph inner\n      A --> B\n    end\n  end\n  C --> A\n";

    fn tree_of(source: &str) -> (Graph, Tree) {
        let graph = parse(source);
        let tree = Tree::of(&graph);
        (graph, tree)
    }

    #[test]
    fn a_container_chain_runs_outward_and_ends_at_the_drawing() {
        let (graph, tree) = tree_of(NESTED);
        let a = graph.index_of("A").expect("declared");
        let chain = tree.chain(a);
        assert_eq!(chain.len(), 3, "inner, outer, the drawing");
        assert_eq!(chain.last(), Some(&None), "the drawing is always last");
        // A node in nothing is held only by the drawing.
        let c = graph.index_of("C").expect("declared");
        assert_eq!(tree.chain(c), vec![None]);
    }

    #[test]
    fn an_edge_is_drawn_by_the_innermost_container_holding_both_ends() {
        let (graph, tree) = tree_of(NESTED);
        // A --> B is wholly inside `inner`; C --> A spans out to the drawing.
        assert!(router(&tree, &graph, 0).is_some());
        assert_eq!(router(&tree, &graph, 1), None);
    }

    #[test]
    fn a_container_reaches_a_deeper_node_through_the_child_that_holds_it() {
        let (graph, tree) = tree_of(NESTED);
        let a = graph.index_of("A").expect("declared");
        let inner = tree.of_node.get(a).copied().flatten().expect("held");
        assert_eq!(toward(&tree, Some(inner), a), Some(Member::Node(a)));
        // From the drawing, the way to A is the outermost group, not A itself.
        assert!(matches!(toward(&tree, None, a), Some(Member::Group(_))));
        // And a container holding it not at all has no way toward it.
        let c = graph.index_of("C").expect("declared");
        assert_eq!(toward(&tree, Some(inner), c), None);
    }

    #[test]
    fn members_are_given_in_the_order_their_contents_begin() {
        // The cycle break turns round whichever edge closes a cycle from where
        // its walk started, so a container that listed its groups after its
        // loose nodes would start the walk halfway down its own flow.
        let (graph, tree) = tree_of("graph TD\n  subgraph g\n    A --> B\n  end\n  B --> C\n");
        let found = members(&graph, &tree, None);
        assert!(
            matches!(found.first(), Some(Member::Group(_))),
            "the group holds A, which is the first thing named"
        );
    }

    #[test]
    fn a_container_tree_that_held_itself_would_still_terminate() {
        // A group closes before the one round it, so a parse cannot produce this.
        // The walk is bounded rather than trusting that.
        let tree = Tree {
            of_node: vec![Some(0)],
            of_group: vec![Some(1), Some(0)],
        };
        assert_eq!(
            tree.chain(0).last(),
            Some(&None),
            "it ends at the drawing regardless"
        );
    }
}
