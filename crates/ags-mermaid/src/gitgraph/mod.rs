//! Git graphs: commits on branch lanes, with merges between them.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, BranchLabel, Placed, PlacedCommit, PlacedEdge};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Branch, Commit, CommitType, Graph, Orientation};
