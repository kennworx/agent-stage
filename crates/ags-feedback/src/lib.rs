//! Anchored review feedback: the model, the log, and the poll that returns it.
//!
//! The return leg of a review loop, and deliberately ignorant of what is being
//! reviewed. An item names a block by id and optionally an element inside it — a
//! node, a cell, a line, a quoted range — and this crate never asks what those
//! mean. It stores them in an append-only local log keyed by a path, folds that
//! log into settled state, and encodes it as TOON.
//!
//! Whether an anchor still *resolves* is the one question it cannot answer, so it
//! takes the answer as an argument. That is what keeps a renderer out of its
//! dependency list, and what lets it be used by something that renders nothing at
//! all.
//!
//! Import-only root, per the workspace convention: declarations and re-exports.

mod form;
mod model;
mod poll;
mod store;
mod wire;

pub use form::parse_feedback_form;
pub use model::{FeedbackItem, FeedbackKind, FeedbackStatus, FeedbackTarget, NoTarget, SubTarget};
pub use poll::poll_blocking;
pub use store::Session;
pub use wire::{parse_feedback_json, poll_to_toon};
