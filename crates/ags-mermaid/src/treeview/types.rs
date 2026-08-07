//! The parsed shape of a tree view: a directory-style hierarchy.

/// A parsed tree view. Several roots are allowed, so this is a forest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
    pub title: Option<String>,
    pub nodes: Vec<TreeNode>,
}

/// One entry in the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub label: String,
    /// The path from the root, which is what makes two files called `index.ts`
    /// in different folders separately addressable.
    pub path: String,
    /// Marked with a trailing slash, or implied by having children.
    pub is_folder: bool,
    pub description: Option<String>,
    pub depth: usize,
    pub children: Vec<TreeNode>,
}
