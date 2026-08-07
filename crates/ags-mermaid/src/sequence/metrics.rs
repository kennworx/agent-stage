//! The spacings a sequence diagram is built from, and the measurements taken
//! straight off them.
//!
//! Held apart from the placement so a number can be read — and changed — without
//! reading the pass that consumes it.

use crate::metrics::text_width;

use super::types::BlockKind;

pub const PADDING: f64 = 30.0;
/// The least distance between two actors' centres.
pub const ACTOR_GAP: f64 = 140.0;
pub const ACTOR_HEIGHT: f64 = 40.0;
pub const ACTOR_PAD_X: f64 = 16.0;
pub const MIN_ACTOR_WIDTH: f64 = 80.0;
/// Between the actor boxes and the first message.
pub const HEADER_GAP: f64 = 20.0;
pub const MESSAGE_ROW: f64 = 40.0;
/// The extra a self-message takes, since it loops out and back.
pub const SELF_MESSAGE_HEIGHT: f64 = 30.0;
pub const SELF_LOOP_WIDTH: f64 = 30.0;
pub const SELF_LOOP_HEIGHT: f64 = 20.0;
pub const SELF_LABEL_PAD: f64 = 8.0;
pub const ACTIVATION_WIDTH: f64 = 10.0;
/// How far a nested activation bar steps to the right of the one it sits in.
pub const NESTING_OFFSET: f64 = 4.0;
pub const BLOCK_PAD_X: f64 = 10.0;
pub const BLOCK_PAD_TOP: f64 = 40.0;
pub const BLOCK_PAD_BOTTOM: f64 = 8.0;
/// Reserved above a block's first message for its header.
pub const BLOCK_HEADER_EXTRA: f64 = 28.0;
/// Reserved above a divided section's first message for its caption.
pub const DIVIDER_EXTRA: f64 = 24.0;
/// A divider's own rule sits this far above the message it introduces…
pub const DIVIDER_OFFSET: f64 = 28.0;
/// …unless its caption would share a line with the message's own label.
pub const DIVIDER_OFFSET_CLEAR: f64 = 36.0;
pub const MIN_NOTE_WIDTH: f64 = 60.0;
pub const NOTE_PAD_X: f64 = 12.0;
pub const NOTE_PAD_Y: f64 = 6.0;
/// Between an actor's box and a note beside it.
pub const NOTE_GAP: f64 = 10.0;
/// Between a message's arrow and the first note hanging off it.
pub const NOTE_DROP: f64 = 8.0;
/// Between two notes in a run.
pub const NOTE_STACK_GAP: f64 = 4.0;
pub const LABEL_FONT: f64 = 13.0;
pub const LABEL_WEIGHT: u32 = 500;
pub const EDGE_FONT: f64 = 11.0;
pub const EDGE_WEIGHT: u32 = 400;
pub const TAB_WEIGHT: u32 = 600;
pub const TAB_HEIGHT: f64 = 18.0;
pub const TAB_PAD_X: f64 = 16.0;
/// Where a self-message's label starts, for the overlap test only. The renderer
/// derives the same number from the loop's own width.
pub const SELF_LABEL_PROBE: f64 = 36.0;
pub const MIN_WIDTH: f64 = 200.0;
pub const MIN_HEIGHT: f64 = 100.0;

pub fn tab_label(kind: BlockKind, label: &str) -> String {
    if label.is_empty() {
        kind.token().to_string()
    } else {
        format!("{} [{label}]", kind.token())
    }
}

/// How wide that tab has to be. A tab wraps onto no second line, so only the
/// first line of a multi-line label decides it.
pub fn tab_width(kind: BlockKind, label: &str) -> f64 {
    let full = tab_label(kind, label);
    let first = full.split('\n').next().unwrap_or(&full);
    text_width(first, EDGE_FONT, TAB_WEIGHT) + TAB_PAD_X
}

/// The caption under a divider's rule.
pub fn divider_label(label: &str) -> String {
    format!("[{label}]")
}

/// How wide a note is: enough for its text, and never less than the minimum.
pub fn note_width(text: &str) -> f64 {
    MIN_NOTE_WIDTH.max(text_width(text, EDGE_FONT, EDGE_WEIGHT) + NOTE_PAD_X * 2.0)
}
