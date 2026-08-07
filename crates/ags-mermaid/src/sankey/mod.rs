//! Sankey diagrams: flows drawn as bands as thick as they are worth.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Placed, PlacedLink, PlacedNode, Side};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Diagram, Link};
