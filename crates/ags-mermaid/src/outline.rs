//! Reading an indented outline, whatever the diagram calls its nodes.
//!
//! Four diagram types are written the same way — a line's indent says whose
//! child it is — and four parsers had written the walk themselves, identically.
//! Every copy reported the same coverage for the same reason `keyword.rs` was
//! extracted: none was reachable from a test of its own, only through whichever
//! grammar sat above it.
//!
//! What differs between them is the payload on a node, not the shape of the
//! tree, so the payload is what the trait leaves to the caller.
//!
//! One parser is deliberately not here. `ishikawa` counts a tab as reaching the
//! next even column rather than as four spaces, which looks like a copy that
//! drifted and is not: a fishbone is drawn from a two-space outline, and folding
//! it in would re-indent every diagram of that type.

/// A node in an outline: some payload, and the nodes written under it.
pub(crate) trait Outline: Sized {
    fn children_mut(&mut self) -> &mut Vec<Self>;
}

/// An outline whose levels are also counted.
///
/// Separate from [`Outline`] because not every parser needs it: `treeview` reads
/// a path straight off the indent and never asks how many nodes are already at a
/// level. Folding the two together would have left it carrying a method nothing
/// called — which the coverage floor noticed the moment they were folded.
pub(crate) trait Counted: Sized {
    fn children(&self) -> &[Self];
}

/// How far a line is indented, counting a tab as four columns.
pub(crate) fn indent_of(line: &str) -> usize {
    let mut n = 0usize;
    for c in line.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    n
}

/// Add `node` under the path of child indices leading to its parent.
///
/// A path naming a child that does not exist drops the node rather than
/// panicking: the source is agent-authored, and one bad line should not stop the
/// rest of the outline being read.
pub(crate) fn attach<T: Outline>(roots: &mut Vec<T>, path: &[usize], node: T) {
    let mut level = roots;
    for index in path {
        let Some(next) = level.get_mut(*index) else {
            return;
        };
        level = next.children_mut();
    }
    level.push(node);
}

/// How many nodes sit at the end of `path`, which is the index the next one
/// written there will take.
pub(crate) fn level_len<T: Counted>(roots: &[T], path: &[usize]) -> usize {
    let mut level = roots;
    for index in path {
        let Some(next) = level.get(*index) else {
            return 0;
        };
        level = next.children();
    }
    level.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Node {
        name: &'static str,
        children: Vec<Node>,
    }

    impl Outline for Node {
        fn children_mut(&mut self) -> &mut Vec<Self> {
            &mut self.children
        }
    }

    impl Counted for Node {
        fn children(&self) -> &[Self] {
            &self.children
        }
    }

    fn node(name: &'static str) -> Node {
        Node {
            name,
            children: Vec::new(),
        }
    }

    #[test]
    fn a_tab_counts_as_four_columns_and_stops_at_the_first_word() {
        assert_eq!(indent_of("word"), 0);
        assert_eq!(indent_of("  word"), 2);
        assert_eq!(indent_of("\tword"), 4);
        assert_eq!(indent_of(" \t word"), 6);
        // Only the leading run counts: a tab inside the text is text.
        assert_eq!(indent_of("  a\tb"), 2);
        assert_eq!(indent_of(""), 0);
    }

    #[test]
    fn an_empty_path_puts_a_node_at_the_top() {
        let mut roots = Vec::new();
        attach(&mut roots, &[], node("a"));
        attach(&mut roots, &[], node("b"));
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[1].name, "b");
    }

    #[test]
    fn a_path_puts_a_node_under_the_one_it_names() {
        let mut roots = vec![node("a")];
        attach(&mut roots, &[0], node("a1"));
        attach(&mut roots, &[0, 0], node("a1x"));
        assert_eq!(roots[0].children[0].name, "a1");
        assert_eq!(roots[0].children[0].children[0].name, "a1x");
    }

    #[test]
    fn a_path_naming_a_node_that_is_not_there_drops_the_line() {
        let mut roots = vec![node("a")];
        attach(&mut roots, &[7], node("lost"));
        assert_eq!(roots.len(), 1, "and the rest of the outline survives");
        assert!(roots[0].children.is_empty());
    }

    #[test]
    fn a_level_is_as_long_as_the_nodes_written_at_it() {
        let mut roots = vec![node("a"), node("b")];
        attach(&mut roots, &[0], node("a1"));
        assert_eq!(level_len(&roots, &[]), 2);
        assert_eq!(level_len(&roots, &[0]), 1);
        assert_eq!(level_len(&roots, &[1]), 0);
        // A path that leads nowhere has nothing at the end of it.
        assert_eq!(level_len(&roots, &[7]), 0);
    }
}
