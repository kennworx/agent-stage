//! Sequence diagrams: actors on lifelines, messages down a timeline.
//!
//! Import-only per the workspace conventions.

mod layout;
mod metrics;
mod parser;
mod render;
mod types;

pub use layout::{
    layout, Activation, Lifeline, Placed, PlacedActor, PlacedBlock, PlacedDivider, PlacedMessage,
    PlacedNote,
};
pub use metrics::{divider_label, tab_label, tab_width};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{
    Actor, ActorKind, ArrowHead, Block, BlockKind, Diagram, Divider, LineStyle, Message, Note,
    NotePosition,
};
