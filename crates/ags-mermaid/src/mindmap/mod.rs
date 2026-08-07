//! Mindmaps: a double-sided tree around a central root.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Connector, Placed, PlacedNode};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Mindmap, Node, Shape};
