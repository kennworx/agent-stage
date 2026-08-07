//! The parsed shape of a sankey diagram: a weighted flow graph.

/// A parsed sankey diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Diagram {
    /// Node names, in order of first appearance — which is also the order the
    /// palette is handed out in, so it decides colour as well as identity.
    pub nodes: Vec<String>,
    pub links: Vec<Link>,
}

/// One directed flow.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub value: f64,
}
