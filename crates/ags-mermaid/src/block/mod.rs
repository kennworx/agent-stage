//! Block diagrams: a uniform grid of boxes, wired together.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Placed, PlacedBlock, PlacedEdge};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Block, Diagram, Edge};
