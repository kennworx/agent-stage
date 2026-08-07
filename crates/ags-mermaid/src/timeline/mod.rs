//! Timelines: periods left to right, events stacked beneath.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, Band, Placed, PlacedEvent, PlacedPeriod};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Period, Section, Timeline};
