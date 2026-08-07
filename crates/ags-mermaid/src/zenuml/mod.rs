//! `ZenUML`: a sequence diagram written as method calls rather than arrows.
//!
//! Import-only per the workspace conventions.

mod layout;
mod lex;
mod parser;
mod render;
mod types;

pub use layout::{
    divider_label, layout, tab_label, tab_width, Divider, Lifeline, Placed, PlacedFragment,
    PlacedMessage, PlacedParticipant,
};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{
    ArrowHead, Diagram, Fragment, FragmentKind, LineStyle, Message, MessageKind, Participant,
    Section,
};
