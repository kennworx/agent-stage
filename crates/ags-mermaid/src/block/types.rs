//! The parsed shape of a block diagram: a uniform grid, and wires between cells.

/// A parsed block diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// Grid width, from `columns N`.
    pub columns: usize,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
}

impl Default for Diagram {
    /// One column, because a grid zero cells wide has nowhere to put a block.
    fn default() -> Self {
        Self {
            columns: 1,
            blocks: Vec::new(),
            edges: Vec::new(),
        }
    }
}

/// One block, in the cell it was written into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Matches edge endpoints, and becomes the drawn element's `data-id`.
    pub id: String,
    pub label: String,
    /// Zero-based, left to right.
    pub col: usize,
    /// Zero-based, top to bottom.
    pub row: usize,
}

/// A wire between two blocks, named by their ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub source: String,
    pub target: String,
}
