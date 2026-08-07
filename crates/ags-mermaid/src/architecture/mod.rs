//! `architecture-beta`: services in groups, joined at named sides.
//!
//! The one type in the set that does not go through the layered engine — see
//! [`grid`] for why its own grammar rules that out.
//!
//! Import-only per the workspace conventions.

mod grid;
mod layout;
mod parser;
mod render;
mod route;
mod types;

pub use grid::{place, Cell, Link};
pub use layout::{boxes, layout, service_size, step_of, Placed, PlacedEdge, PlacedItem};
pub use parser::{edge, parse};
pub use render::{render, scene};
pub use route::{around, path, routes};
pub use types::{Diagram, Edge, Item, Kind, Side};
