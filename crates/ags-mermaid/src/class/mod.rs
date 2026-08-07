//! Class diagrams: UML boxes with compartments, and the relationships between
//! them.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod style;
mod types;

pub use layout::{
    cardinality_at, compartments, layout, measure, Compartments, Placed, PlacedClass,
    PlacedRelationship,
};
pub use parser::{member, parse, relationship};
pub use render::{render, scene};
pub use types::{
    Class, Diagram, End, Member, Relation, Relationship, Visibility, AGGREGATION_MARKER, ARROWS,
    ARROW_MARKER, COMPOSITION_MARKER, INHERIT_MARKER,
};
