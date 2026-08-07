//! What the caller hands over, and what comes back.

/// A point in layout coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// One box to place. Its identity is its position in `Graph::nodes`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Node {
    pub width: f64,
    pub height: f64,
}

impl Node {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// One arrow, by the positions of the boxes it joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
}

impl Edge {
    pub const fn new(from: usize, to: usize) -> Self {
        Self { from, to }
    }
}

/// Which way the layers run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Down,
    Up,
    Right,
    Left,
}

impl Direction {
    /// Whether the layers run across the page rather than down it.
    ///
    /// Everything is laid out downwards; a direction that runs across the page
    /// turns each box first and the whole drawing back afterwards, so there is
    /// one implementation rather than four.
    pub const fn across(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }

    /// Whether the layers run backwards from the usual reading order.
    pub const fn reversed(self) -> bool {
        matches!(self, Self::Up | Self::Left)
    }
}

/// The gaps the layout leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Between two boxes in the same layer.
    pub node: f64,
    /// Between one layer and the next.
    pub layer: f64,
    /// Between two edge runs sharing the space between layers.
    pub edge: f64,
    /// Around the whole drawing.
    pub padding: f64,
}

impl Default for Spacing {
    /// The gaps the renderer this replaces asked for.
    fn default() -> Self {
        Self {
            node: 40.0,
            layer: 50.0,
            edge: 12.0,
            padding: 16.0,
        }
    }
}

/// Where an edge must meet a node, rather than wherever routing would put it.
///
/// Edges sharing a node are normally spread along its side, ordered so that two
/// of them need not cross to reach their own place. That is right when the node
/// is a box; it is wrong when the node is itself a drawing that has already been
/// laid out, because *that* layout decided where the wire belongs and the two
/// answers will not agree. A pin lets the caller say which one wins.
///
/// `at` is a fraction of the node's side, nought at the low end. The side is the
/// one the layers run across, so it is the width for a drawing that runs down the
/// page and the height for one that runs across it — the same axis the engine
/// spreads unpinned edges along.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Port {
    /// The edge, by its position in [`Graph::edges`].
    pub edge: usize,
    /// Whether this pins the end the edge leaves from.
    pub source: bool,
    /// How far along the node's side, `0.0..=1.0`.
    pub at: f64,
}

impl Port {
    pub const fn new(edge: usize, source: bool, at: f64) -> Self {
        Self { edge, source, at }
    }
}

/// A graph to lay out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub direction: Direction,
    pub spacing: Spacing,
    /// Ends that are pinned rather than spread. Empty is the ordinary case.
    pub ports: Vec<Port>,
}

impl Graph {
    /// Whether `edge` names two boxes that exist.
    ///
    /// An edge to a node nobody declared is dropped rather than rejected: a
    /// diagram with one bad line should still draw the rest of itself.
    pub fn holds(&self, edge: Edge) -> bool {
        edge.from < self.nodes.len() && edge.to < self.nodes.len()
    }
}

/// One box, placed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlacedNode {
    /// The top-left corner.
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

impl PlacedNode {
    pub fn centre(self) -> Point {
        Point::new(self.at.x + self.width / 2.0, self.at.y + self.height / 2.0)
    }
}

/// One arrow, routed. Empty when the edge named a box that does not exist.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacedEdge {
    pub points: Vec<Point>,
}

/// A laid-out graph. Nodes and edges keep the caller's order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<PlacedEdge>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_direction_knows_which_way_it_runs() {
        assert!(!Direction::Down.across() && !Direction::Down.reversed());
        assert!(!Direction::Up.across() && Direction::Up.reversed());
        assert!(Direction::Right.across() && !Direction::Right.reversed());
        assert!(Direction::Left.across() && Direction::Left.reversed());
    }

    #[test]
    fn a_placed_box_knows_its_own_middle() {
        let node = PlacedNode {
            at: Point::new(10.0, 20.0),
            width: 30.0,
            height: 40.0,
        };
        assert_eq!(node.centre(), Point::new(25.0, 40.0));
    }

    #[test]
    fn an_edge_naming_a_box_that_does_not_exist_is_not_held() {
        let graph = Graph {
            nodes: vec![Node::new(1.0, 1.0), Node::new(1.0, 1.0)],
            ..Graph::default()
        };
        assert!(graph.holds(Edge::new(0, 1)));
        assert!(!graph.holds(Edge::new(0, 2)));
        assert!(!graph.holds(Edge::new(9, 0)));
    }

    #[test]
    fn the_default_gaps_are_the_ones_the_old_renderer_asked_for() {
        let spacing = Spacing::default();
        assert!((spacing.node - 40.0).abs() < 1e-9);
        assert!((spacing.layer - 50.0).abs() < 1e-9);
        assert!((spacing.edge - 12.0).abs() < 1e-9);
        assert!((spacing.padding - 16.0).abs() < 1e-9);
    }
}
