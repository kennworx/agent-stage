//! Entity-relationship diagrams: boxes of columns, joined by crow's feet.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{
    badge_width, foot, label_size, layout, measure, midpoint, Foot, Placed, PlacedEntity,
    PlacedRelationship,
};
pub use parser::{attribute, parse, relationship};
pub use render::{render, scene};
pub use types::{Attribute, Cardinality, Diagram, Entity, Key, Relationship};
