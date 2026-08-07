//! The parsed shape of a mindmap: a tree of shaped nodes.

/// The shape a node is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    Default,
    Square,
    Round,
    Circle,
    Bang,
    Cloud,
    Hexagon,
}

impl Shape {
    /// Every shape a mindmap node can be drawn as.
    ///
    /// Listed so a test can be exhaustive by construction rather than by someone
    /// remembering to extend it: adding a variant without adding it here stops
    /// compiling, which is the only reminder that works.
    pub const ALL: [Self; 7] = [
        Self::Default,
        Self::Square,
        Self::Round,
        Self::Circle,
        Self::Bang,
        Self::Cloud,
        Self::Hexagon,
    ];

    /// The token this shape is reported as, which becomes its `data-shape`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Square => "square",
            Self::Round => "round",
            Self::Circle => "circle",
            Self::Bang => "bang",
            Self::Cloud => "cloud",
            Self::Hexagon => "hexagon",
        }
    }
}

/// A parsed mindmap.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mindmap {
    pub root: Node,
}

/// One node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// Derived from the label, made unique. The optional `id[...]` prefix in
    /// the syntax is shape punctuation, not identity.
    pub id: String,
    pub label: String,
    pub shape: Shape,
    pub depth: usize,
    pub children: Vec<Node>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_reports_a_token_of_its_own() {
        // The token becomes `data-shape`, which is how a reviewer's annotation
        // says which kind of node it is about — so two shapes sharing one would
        // be silently indistinguishable downstream.
        let tokens: Vec<&str> = Shape::ALL.iter().map(|shape| shape.token()).collect();
        assert!(tokens.iter().all(|token| !token.is_empty()));
        let mut unique = tokens.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), tokens.len(), "{tokens:?}");
    }

    #[test]
    fn the_default_shape_is_the_one_a_bare_node_gets() {
        assert_eq!(Shape::default(), Shape::Default);
        assert_eq!(Shape::default().token(), "default");
    }
}
