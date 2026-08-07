//! Where each node goes.
//!
//! The root sits in the middle and its top-level children split into a right
//! group and a left one, so the map is double-sided the way a hand-drawn one is.
//! Right subtrees grow rightward, left ones mirror. Every node is centred on the
//! vertical span of its own subtree, so a parent lines up with its children
//! rather than with the first of them.

use crate::scene::Point;

use super::types::{Mindmap, Node, Shape};

pub const PADDING: f64 = 24.0;
/// Between a parent's edge and its children's near edge.
pub const H_GAP: f64 = 52.0;
/// Between sibling subtrees.
pub const V_GAP: f64 = 18.0;
pub const NODE_HEIGHT: f64 = 38.0;
/// Inside a node box, either side of its text.
pub const H_PAD: f64 = 16.0;
pub const MIN_WIDTH: f64 = 44.0;
pub const FONT: f64 = 14.0;
pub const WEIGHT: u32 = 500;

/// One node, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    pub depth: usize,
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// One parent-to-child run.
#[derive(Debug, Clone, PartialEq)]
pub struct Connector {
    pub from: String,
    pub to: String,
    pub a: Point,
    pub b: Point,
}

/// A laid-out mindmap.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    /// Parents before children, so a box is never drawn over its own parent.
    pub nodes: Vec<PlacedNode>,
    pub connectors: Vec<Connector>,
}

/// How wide a node is: its text, padded, but never narrower than the minimum.
fn node_width(label: &str) -> f64 {
    MIN_WIDTH.max((crate::metrics::text_width(label, FONT, WEIGHT) + H_PAD * 2.0).round())
}

/// What is being built while the tree is walked.
struct Build {
    nodes: Vec<PlacedNode>,
    connectors: Vec<Connector>,
}

impl Build {
    /// Move a contiguous run of nodes and connectors down by `dy`.
    fn shift(&mut self, nodes: (usize, usize), connectors: (usize, usize), dy: f64) {
        if dy == 0.0 {
            return;
        }
        for node in self.nodes.get_mut(nodes.0..nodes.1).unwrap_or_default() {
            node.at = Point::new(node.at.x, node.at.y + dy);
        }
        for run in self
            .connectors
            .get_mut(connectors.0..connectors.1)
            .unwrap_or_default()
        {
            run.a = Point::new(run.a.x, run.a.y + dy);
            run.b = Point::new(run.b.x, run.b.y + dy);
        }
    }

    /// Where a node was placed, by id.
    fn find(&self, id: &str) -> Option<&PlacedNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Place a subtree growing in `direction`, and report the vertical slot it took.
///
/// `anchor_x` is the edge facing the root: the left edge going right, the right
/// edge going left.
fn place(node: &Node, anchor_x: f64, y_top: f64, direction: f64, build: &mut Build) -> f64 {
    let width = node_width(&node.label);
    let x = if direction > 0.0 {
        anchor_x
    } else {
        anchor_x - width
    };
    let index = build.nodes.len();
    build.nodes.push(PlacedNode {
        id: node.id.clone(),
        label: node.label.clone(),
        shape: node.shape,
        depth: node.depth,
        // Filled in once this subtree's own height is known.
        at: Point::new(x, 0.0),
        width,
        height: NODE_HEIGHT,
    });

    if node.children.is_empty() {
        if let Some(slot) = build.nodes.get_mut(index) {
            // Centred in its own slot, so a run of leaves is evenly spaced.
            slot.at = Point::new(x, y_top + V_GAP / 2.0);
        }
        return NODE_HEIGHT + V_GAP;
    }

    let child_anchor = if direction > 0.0 {
        x + width + H_GAP
    } else {
        x - H_GAP
    };
    let mut y = y_top;
    for child in &node.children {
        y += place(child, child_anchor, y, direction, build);
    }
    let span = y - y_top;
    // Centred on the span of its children, not on the first of them.
    if let Some(slot) = build.nodes.get_mut(index) {
        slot.at = Point::new(x, y_top + span / 2.0 - NODE_HEIGHT / 2.0);
    }

    let Some(parent) = build.nodes.get(index).cloned() else {
        return span;
    };
    for child in &node.children {
        let Some(placed) = build.find(&child.id).cloned() else {
            continue;
        };
        build.connectors.push(Connector {
            from: node.id.clone(),
            to: child.id.clone(),
            a: Point::new(
                if direction > 0.0 {
                    parent.at.x + parent.width
                } else {
                    parent.at.x
                },
                parent.at.y + parent.height / 2.0,
            ),
            b: Point::new(
                if direction > 0.0 {
                    placed.at.x
                } else {
                    placed.at.x + placed.width
                },
                placed.at.y + placed.height / 2.0,
            ),
        });
    }
    span
}

/// Lay a real root out in the middle, splitting its children either side of it.
fn double_sided(root: &Node, build: &mut Build) {
    let width = node_width(&root.label);
    build.nodes.push(PlacedNode {
        id: root.id.clone(),
        label: root.label.clone(),
        shape: root.shape,
        depth: root.depth,
        at: Point::new(0.0, 0.0),
        width,
        height: NODE_HEIGHT,
    });

    // The first half goes right, so an odd child count leans that way.
    let split = root.children.len().div_ceil(2);
    let (right, left) = root.children.split_at(split.min(root.children.len()));

    let right_start = (build.nodes.len(), build.connectors.len());
    let mut y = PADDING;
    for child in right {
        y += place(child, width + H_GAP, y, 1.0, build);
    }
    let right_height = y - PADDING;

    let left_start = (build.nodes.len(), build.connectors.len());
    let mut y = PADDING;
    for child in left {
        y += place(child, -H_GAP, y, -1.0, build);
    }
    let left_height = y - PADDING;
    let left_end = (build.nodes.len(), build.connectors.len());

    // Both sides are centred against the taller one, so the root sits level
    // with the middle of the whole map rather than with one side of it.
    let tallest = right_height.max(left_height).max(NODE_HEIGHT);
    build.shift(
        (right_start.0, left_start.0),
        (right_start.1, left_start.1),
        (tallest - right_height) / 2.0,
    );
    build.shift(
        (left_start.0, left_end.0),
        (left_start.1, left_end.1),
        (tallest - left_height) / 2.0,
    );

    let root_y = PADDING + tallest / 2.0 - NODE_HEIGHT / 2.0;
    if let Some(slot) = build.nodes.first_mut() {
        slot.at = Point::new(0.0, root_y);
    }

    for (index, child) in root.children.iter().enumerate() {
        let Some(placed) = build.find(&child.id).cloned() else {
            continue;
        };
        let on_right = index < split;
        build.connectors.push(Connector {
            from: root.id.clone(),
            to: child.id.clone(),
            a: Point::new(
                if on_right { width } else { 0.0 },
                root_y + NODE_HEIGHT / 2.0,
            ),
            b: Point::new(
                if on_right {
                    placed.at.x
                } else {
                    placed.at.x + placed.width
                },
                placed.at.y + placed.height / 2.0,
            ),
        });
    }
}

/// Lay out a parsed mindmap.
pub fn layout(mindmap: &Mindmap) -> Placed {
    let mut build = Build {
        nodes: Vec::new(),
        connectors: Vec::new(),
    };
    // A nameless container has no centre to pivot around, so its trees are
    // stacked and each grows one way.
    if mindmap.root.id.is_empty() && mindmap.root.label.is_empty() {
        let mut y = PADDING;
        for top in &mindmap.root.children {
            y += place(top, PADDING, y, 1.0, &mut build);
        }
    } else {
        double_sided(&mindmap.root, &mut build);
    }

    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for node in &build.nodes {
        min_x = min_x.min(node.at.x);
        min_y = min_y.min(node.at.y);
        max_x = max_x.max(node.at.x + node.width);
        max_y = max_y.max(node.at.y + node.height);
    }
    if build.nodes.is_empty() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 0.0;
        max_y = 0.0;
    }

    // Leftward subtrees run into negative coordinates, so the whole drawing is
    // moved to sit at a uniform padding from the top-left.
    let offset = Point::new(PADDING - min_x, PADDING - min_y);
    for node in &mut build.nodes {
        node.at = Point::new(node.at.x + offset.x, node.at.y + offset.y);
    }
    for run in &mut build.connectors {
        run.a = Point::new(run.a.x + offset.x, run.a.y + offset.y);
        run.b = Point::new(run.b.x + offset.x, run.b.y + offset.y);
    }

    Placed {
        width: max_x - min_x + PADDING * 2.0,
        height: (max_y - min_y + PADDING * 2.0).max(PADDING * 2.0 + NODE_HEIGHT),
        nodes: build.nodes,
        connectors: build.connectors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built() -> Build {
        let node = |id: &str, y: f64| PlacedNode {
            id: id.to_string(),
            label: id.to_string(),
            shape: Shape::default(),
            depth: 0,
            at: Point::new(0.0, y),
            width: 10.0,
            height: 10.0,
        };
        Build {
            nodes: vec![node("a", 0.0), node("b", 100.0)],
            connectors: vec![Connector {
                from: "a".into(),
                to: "b".into(),
                a: Point::new(0.0, 0.0),
                b: Point::new(0.0, 100.0),
            }],
        }
    }

    #[test]
    fn a_subtree_moves_with_its_own_connectors() {
        // Both ends of a run move, or the wire is left behind by the nodes it
        // joins — which is what makes this one call rather than two.
        let mut build = built();
        build.shift((0, 2), (0, 1), 25.0);
        assert!((build.nodes[0].at.y - 25.0).abs() < 1e-9);
        assert!((build.nodes[1].at.y - 125.0).abs() < 1e-9);
        assert!((build.connectors[0].a.y - 25.0).abs() < 1e-9);
        assert!((build.connectors[0].b.y - 125.0).abs() < 1e-9);
    }

    #[test]
    fn moving_a_subtree_nowhere_touches_nothing() {
        let mut build = built();
        let before = (build.nodes.clone(), build.connectors.clone());
        build.shift((0, 2), (0, 1), 0.0);
        assert_eq!((build.nodes, build.connectors), before);
    }

    #[test]
    fn a_run_that_is_not_there_is_stepped_over() {
        // The ranges come from indices recorded before the walk went deeper, so a
        // subtree that placed nothing gives an empty or out-of-range span.
        let mut build = built();
        build.shift((5, 9), (5, 9), 10.0);
        assert!((build.nodes[0].at.y - 0.0).abs() < 1e-9, "nothing moved");
    }

    #[test]
    fn a_placed_node_is_found_by_its_id() {
        let build = built();
        assert_eq!(build.find("b").map(|n| n.id.clone()), Some("b".into()));
        assert!(build.find("nobody").is_none());
    }
    use crate::mindmap::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const MAP: &str = "mindmap\n\
        root((Centre))\n  \
          Left one\n  \
          Left two\n  \
            Deeper\n  \
          Right one\n  \
          Right two";

    #[test]
    fn the_root_is_first_and_children_split_either_side_of_it() {
        let out = placed(MAP);
        let root = &out.nodes[0];
        assert_eq!(root.label, "Centre");
        let left = out.nodes.iter().filter(|n| n.at.x < root.at.x).count();
        let right = out
            .nodes
            .iter()
            .filter(|n| n.at.x > root.at.x + root.width)
            .count();
        assert!(left > 0 && right > 0, "double-sided");
    }

    #[test]
    fn an_odd_child_count_leans_to_the_right() {
        let out = placed("mindmap\nroot\n a\n b\n c");
        let root = &out.nodes[0];
        let right = out
            .nodes
            .iter()
            .filter(|n| n.depth == 1 && n.at.x > root.at.x)
            .count();
        assert_eq!(right, 2, "two right, one left");
    }

    #[test]
    fn a_parent_is_centred_on_the_span_of_its_children() {
        let out = placed("mindmap\nroot\n parent\n  a\n  b\n  c");
        let parent = out
            .nodes
            .iter()
            .find(|n| n.label == "parent")
            .expect("parent");
        let children: Vec<&PlacedNode> = out.nodes.iter().filter(|n| n.depth == 2).collect();
        let top = children
            .iter()
            .map(|c| c.at.y)
            .fold(f64::INFINITY, f64::min);
        let bottom = children
            .iter()
            .map(|c| c.at.y + c.height)
            .fold(f64::NEG_INFINITY, f64::max);
        let middle = f64::midpoint(top, bottom);
        assert!((parent.at.y + parent.height / 2.0 - middle).abs() < 1e-6);
    }

    #[test]
    fn a_child_clears_its_parent_by_the_gap_on_whichever_side_it_is() {
        let out = placed("mindmap\nroot\n only\n  deep");
        let only = out.nodes.iter().find(|n| n.label == "only").expect("only");
        let deep = out.nodes.iter().find(|n| n.label == "deep").expect("deep");
        assert!((deep.at.x - (only.at.x + only.width + H_GAP)).abs() < 1e-9);
    }

    #[test]
    fn a_connector_runs_from_the_facing_edges() {
        let out = placed(MAP);
        let root = &out.nodes[0];
        for run in &out.connectors {
            if run.from != root.id {
                continue;
            }
            let from_edge = (run.a.x - root.at.x).abs() < 1e-9
                || (run.a.x - (root.at.x + root.width)).abs() < 1e-9;
            assert!(from_edge, "{run:?}");
        }
    }

    #[test]
    fn every_node_and_connector_lands_on_the_canvas() {
        let out = placed(MAP);
        for node in &out.nodes {
            assert!(node.at.x >= 0.0, "{node:?}");
            assert!(node.at.x + node.width <= out.width + 1e-9, "{node:?}");
            assert!(node.at.y + node.height <= out.height + 1e-9, "{node:?}");
        }
        for run in &out.connectors {
            assert!(run.a.x >= 0.0 && run.b.x >= 0.0, "{run:?}");
        }
    }

    #[test]
    fn a_node_is_at_least_as_wide_as_its_own_text() {
        let out = placed("mindmap\nA rather long label indeed");
        assert!(out.nodes[0].width > MIN_WIDTH);
        assert!((node_width("x") - MIN_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn a_forest_stacks_rather_than_pivoting() {
        let out = placed("mindmap\nfirst\nsecond");
        // No container node is drawn, and both trees grow the same way.
        assert_eq!(out.nodes.len(), 2);
        assert!((out.nodes[0].at.x - out.nodes[1].at.x).abs() < 1e-9);
        assert!(out.nodes[1].at.y > out.nodes[0].at.y);
    }

    #[test]
    fn an_empty_map_still_yields_a_canvas() {
        let out = placed("mindmap");
        assert!(out.nodes.is_empty());
        assert!((out.height - (PADDING * 2.0 + NODE_HEIGHT)).abs() < 1e-9);
    }
}
