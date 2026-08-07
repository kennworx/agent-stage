//! The parsed shape of a treemap: a hierarchy of weighted boxes.

/// A parsed treemap.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Treemap {
    pub title: Option<String>,
    pub root: Node,
}

/// One node. A branch's value is the sum of what it holds; a leaf carries its
/// own.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub label: String,
    /// The path from the root, so two leaves with the same name stay separable.
    pub path: String,
    pub value: f64,
    pub children: Vec<Node>,
    /// Which top-level branch this belongs to, and so which colour it takes.
    /// `None` on the root itself, which belongs to nothing.
    pub color_index: Option<usize>,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}
