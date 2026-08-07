//! Event models: time frames across three swimlanes.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Placed, PlacedFrame, PlacedLane, PlacedRelation};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Entity, Frame, Lane, Model};
