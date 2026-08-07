//! Fishbone diagrams: an effect, and the causes that feed it.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Head, Placed, PlacedCategory, PlacedCause};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Cause, Diagram};
