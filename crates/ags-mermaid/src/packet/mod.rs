//! Packet diagrams: a bit-field map on a thirty-two-bit grid.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Placed, PlacedField, Segment};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Diagram, Field};
