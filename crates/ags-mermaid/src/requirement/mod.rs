//! Requirement diagrams: requirement and element boxes, typed relationships.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Boxed, Placed, PlacedEdge, PlacedNode};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Diagram, Element, Kind, Relationship, Requirement};
