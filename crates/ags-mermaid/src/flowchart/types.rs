//! The parsed shape of a flowchart: boxes, arrows and the groups round them.
//!
//! `stateDiagram-v2` reads into this same structure. The two syntaxes differ but
//! the drawing does not — a state is a box and a transition is an arrow — and
//! the reference has always treated them as one pipeline.

/// What a node is drawn as. The delimiters around its label decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    Rectangle,
    Rounded,
    Diamond,
    Stadium,
    Circle,
    /// `[[text]]` — a box with a rule inside each end.
    Subroutine,
    /// `(((text)))` — a circle within a circle.
    DoubleCircle,
    Hexagon,
    /// `[(text)]` — a database.
    Cylinder,
    /// `>text]` — a flag.
    Asymmetric,
    /// `[/text\]` — wider along the bottom.
    Trapezoid,
    /// `[\text/]` — wider along the top.
    TrapezoidAlt,
    /// A state diagram's opening dot.
    StateStart,
    /// A state diagram's closing ring.
    StateEnd,
}

impl Shape {
    /// The name this shape is written with, which becomes its class.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::Rounded => "rounded",
            Self::Diamond => "diamond",
            Self::Stadium => "stadium",
            Self::Circle => "circle",
            Self::Subroutine => "subroutine",
            Self::DoubleCircle => "doublecircle",
            Self::Hexagon => "hexagon",
            Self::Cylinder => "cylinder",
            Self::Asymmetric => "asymmetric",
            Self::Trapezoid => "trapezoid",
            Self::TrapezoidAlt => "trapezoid-alt",
            Self::StateStart => "state-start",
            Self::StateEnd => "state-end",
        }
    }

    /// Whether the outline fills its own bounding box.
    ///
    /// One that does not — a diamond, a hexagon, a trapezoid — needs an edge
    /// clipped to the outline rather than to the box, or the arrowhead floats in
    /// the gap between the two.
    pub const fn fills_its_box(self) -> bool {
        matches!(
            self,
            Self::Rectangle | Self::Rounded | Self::Stadium | Self::Subroutine | Self::Cylinder
        )
    }
}

/// How an arrow's line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeStyle {
    #[default]
    Solid,
    Dotted,
    Thick,
}

impl EdgeStyle {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dotted => "dotted",
            Self::Thick => "thick",
        }
    }
}

/// Which way the layout runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// `TD` and `TB` are the same thing written two ways.
    #[default]
    Down,
    Up,
    Right,
    Left,
}

impl Direction {
    /// The direction a header keyword names.
    pub fn from_keyword(word: &str) -> Option<Self> {
        match word.to_ascii_uppercase().as_str() {
            "TD" | "TB" => Some(Self::Down),
            "BT" => Some(Self::Up),
            "LR" => Some(Self::Right),
            "RL" => Some(Self::Left),
            _ => None,
        }
    }

    /// The same direction, as the layout engine names it.
    pub const fn as_layout(self) -> crate::layout::Direction {
        match self {
            Self::Down => crate::layout::Direction::Down,
            Self::Up => crate::layout::Direction::Up,
            Self::Right => crate::layout::Direction::Right,
            Self::Left => crate::layout::Direction::Left,
        }
    }
}

/// One box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub shape: Shape,
    /// The classes a `class` or `classDef` line put on it.
    pub classes: Vec<String>,
}

/// One arrow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub label: String,
    pub style: EdgeStyle,
    /// An arrowhead at the source end, from `<-->`.
    pub head_start: bool,
    pub head_end: bool,
}

/// One `subgraph`, or one composite state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Group {
    pub id: String,
    pub label: String,
    /// The nodes directly inside, by id.
    pub nodes: Vec<String>,
    /// The groups directly inside, by their position in `Graph::groups`.
    pub groups: Vec<usize>,
    /// A `direction` line inside the group, which lays it out on its own.
    pub direction: Option<Direction>,
}

/// Styling written on a `classDef`, a `style` or a `linkStyle` line.
///
/// Held as the properties were written rather than as a parsed colour: the
/// renderer decides what it can honour, and a property nobody supports is
/// dropped there rather than lost here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Style {
    pub props: Vec<(String, String)>,
}

/// Which edges a `linkStyle` line applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// `linkStyle default …`
    Every,
    /// `linkStyle 0,2 …`, by the edge's position in the source.
    At(usize),
}

/// A parsed flowchart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    pub direction: Direction,
    /// Nodes in the order they were first named.
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Groups innermost first, which is the order they close in.
    pub groups: Vec<Group>,
    /// `classDef name …`, by name.
    pub class_defs: Vec<(String, Style)>,
    /// `style A …`, by node id.
    pub node_styles: Vec<(String, Style)>,
    pub link_styles: Vec<(LinkTarget, Style)>,
}

impl Graph {
    /// Where `id` sits in `nodes`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_is_written_as_the_name_it_is_known_by() {
        for shape in [
            Shape::Rectangle,
            Shape::Rounded,
            Shape::Diamond,
            Shape::Stadium,
            Shape::Circle,
            Shape::Subroutine,
            Shape::DoubleCircle,
            Shape::Hexagon,
            Shape::Cylinder,
            Shape::Asymmetric,
            Shape::Trapezoid,
            Shape::TrapezoidAlt,
            Shape::StateStart,
            Shape::StateEnd,
        ] {
            assert!(!shape.token().is_empty(), "{shape:?}");
        }
        assert_eq!(Shape::default(), Shape::Rectangle);
    }

    #[test]
    fn a_shape_knows_whether_an_edge_can_stop_at_its_box() {
        // These fill their box, so an arrow may stop at the bounding rectangle.
        for shape in [
            Shape::Rectangle,
            Shape::Rounded,
            Shape::Stadium,
            Shape::Subroutine,
            Shape::Cylinder,
        ] {
            assert!(shape.fills_its_box(), "{shape:?}");
        }
        // These do not, so an arrow stopping at the box floats in the gap.
        for shape in [
            Shape::Diamond,
            Shape::Hexagon,
            Shape::Trapezoid,
            Shape::TrapezoidAlt,
            Shape::Asymmetric,
            Shape::Circle,
            Shape::DoubleCircle,
        ] {
            assert!(!shape.fills_its_box(), "{shape:?}");
        }
    }

    #[test]
    fn an_edge_style_is_written_as_its_own_name() {
        assert_eq!(EdgeStyle::Solid.token(), "solid");
        assert_eq!(EdgeStyle::Dotted.token(), "dotted");
        assert_eq!(EdgeStyle::Thick.token(), "thick");
        assert_eq!(EdgeStyle::default(), EdgeStyle::Solid);
    }

    #[test]
    fn every_direction_keyword_names_a_direction() {
        assert_eq!(Direction::from_keyword("TD"), Some(Direction::Down));
        assert_eq!(Direction::from_keyword("TB"), Some(Direction::Down));
        assert_eq!(Direction::from_keyword("bt"), Some(Direction::Up));
        assert_eq!(Direction::from_keyword("LR"), Some(Direction::Right));
        assert_eq!(Direction::from_keyword("RL"), Some(Direction::Left));
        assert_eq!(Direction::from_keyword("sideways"), None);
        assert_eq!(Direction::default(), Direction::Down);
    }

    #[test]
    fn a_direction_carries_over_to_the_engine_unchanged() {
        assert_eq!(Direction::Down.as_layout(), crate::layout::Direction::Down);
        assert_eq!(Direction::Up.as_layout(), crate::layout::Direction::Up);
        assert_eq!(
            Direction::Right.as_layout(),
            crate::layout::Direction::Right
        );
        assert_eq!(Direction::Left.as_layout(), crate::layout::Direction::Left);
    }

    #[test]
    fn a_graph_finds_a_node_by_the_name_it_was_given() {
        let graph = Graph {
            nodes: vec![
                Node {
                    id: "A".into(),
                    label: "Start".into(),
                    shape: Shape::Rectangle,
                    classes: Vec::new(),
                },
                Node {
                    id: "B".into(),
                    label: "End".into(),
                    shape: Shape::Circle,
                    classes: Vec::new(),
                },
            ],
            ..Graph::default()
        };
        assert_eq!(graph.index_of("B"), Some(1));
        assert_eq!(graph.index_of("C"), None);
    }
}
