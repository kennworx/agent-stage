//! Treemaps: a hierarchy as nested boxes, sized by value.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Cell, Placed, Rect};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Node, Treemap};
