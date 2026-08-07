//! Venn diagrams: sets as circles, and the regions where they meet.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Placed, PlacedSet, PlacedUnion};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Diagram, Set, Union};
