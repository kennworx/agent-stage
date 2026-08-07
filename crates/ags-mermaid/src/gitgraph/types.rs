//! The parsed shape of a git graph: branches, and the commits on them.

/// How a commit is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitType {
    #[default]
    Normal,
    /// Marked as undoing something: a disc with a cross through it.
    Reverse,
    /// Called out: a square rather than a disc.
    Highlight,
}

/// Which way the graph runs. Parsed and carried, but every layout here is
/// left to right — the reference reads the keyword and ignores it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    LeftRight,
    TopBottom,
    BottomTop,
}

/// A parsed git graph.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    pub orientation: Orientation,
    pub branches: Vec<Branch>,
    pub commits: Vec<Commit>,
}

/// One branch, and the lane it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    /// Order of first appearance, which is also its lane.
    pub order: usize,
}

/// One commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub branch: String,
    /// The first is the previous commit on this branch; a second is whatever
    /// was merged or cherry-picked in.
    pub parents: Vec<String>,
    pub tag: Option<String>,
    pub kind: CommitType,
    pub is_merge: bool,
    pub is_cherry_pick: bool,
}
