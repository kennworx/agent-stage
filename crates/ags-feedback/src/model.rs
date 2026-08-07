//! The feedback item model — the return-leg's anchored, typed feedback.
//!
//! v1 scope: an item names the block it targets by `#id` plus an optional
//! element-level [`SubTarget`], carries `resolutionTarget` (agent-vs-human routing)
//! and a reviewer-facing [`resolved`](FeedbackItem::resolved) axis independent of it.
//! Detached-anchor reconciliation is **not** a field here. Whether an anchor still
//! resolves is a fact about the artifact as it currently stands, not about the item
//! — the same recorded comment is attached before a redraw and detached after —
//! so it is derived per poll by [`crate::anchors`] and rendered as a column, never
//! stored. The item is delivered either way; only the agent can reconcile it,
//! having made the edit that broke it.
//!
//! The models derive serde, so the store persists them as JSON (`serde_json`) and
//! the poll output renders them as TOON (`toon-rs`) — one definition, two formats.

use serde::{Deserialize, Serialize};

/// What kind of feedback an item carries. All three travel one channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackKind {
    /// A human annotation (anchored comment).
    Annotation,
    /// A human answer to a `question` block.
    Answer,
    /// A machine render-audit finding from Gate 2.
    Finding,
}

/// Where an item sits in its lifecycle. The store is an append-only log, so an
/// edit or removal is a new row whose `status` marks intent (the agent replays
/// the sequence); nothing is mutated or dropped in place. Identity is the item's
/// `anchor` — one annotation per element — so no separate id is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackStatus {
    /// A newly created item (the default for answers and findings).
    #[default]
    New,
    /// An edit of an earlier item on the same anchor.
    Update,
    /// A removal of an earlier item on the same anchor.
    Delete,
}

/// Who a feedback item is routed to (`resolutionTarget`). Only `agent` items are an
/// actionable routing signal; `human` items are context; `mention`s are notify-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackTarget {
    /// Actionable — the agent should address it (the default; answers/findings too).
    #[default]
    Agent,
    /// Context — delivered for the agent to read, not to act on.
    Human,
    /// A notification, never a routing signal.
    Mention,
}

/// An element within a block that a feedback item targets. Absent = block-level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubTarget {
    /// A diagram node by its `data-id`.
    Node(String),
    /// A table cell by 0-based row and column.
    Cell { row: usize, col: usize },
    /// A code line or range, e.g. `"12"` or `"12-18"`.
    Lines(String),
    /// A prose text range: quoted text plus surrounding context.
    Text {
        quote: String,
        before: String,
        after: String,
    },
}

impl SubTarget {
    /// A short human-facing rendering of the sub-target for TOON anchors.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Node(id) => format!("node:{id}"),
            Self::Cell { row, col } => format!("cell:{row},{col}"),
            Self::Lines(range) => format!("line:{range}"),
            Self::Text { quote, .. } => format!("text:{quote}"),
        }
    }
}

/// A single feedback item on the return channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackItem {
    /// The block this targets, by `#id` (without the leading `#`).
    pub block_id: String,
    /// Optional element-level sub-target within the block.
    pub sub_target: Option<SubTarget>,
    /// What kind of feedback this is.
    pub kind: FeedbackKind,
    /// The body: comment text, answer value, or finding detail.
    pub body: String,
    /// Where this item sits in its lifecycle (`new`/`update`/`delete`). Identity is
    /// the [`anchor`](Self::anchor) — one annotation per element.
    #[serde(default)]
    pub status: FeedbackStatus,
    /// Who the item is routed to (`agent`/`human`/`mention`) — the agent acts only
    /// on `agent` items and treats the rest as context.
    #[serde(default)]
    pub target: FeedbackTarget,
    /// The reviewer-facing *resolved* axis (D5): a reviewer marked this thread done.
    /// Independent of `target`/`status` and of agent consumption — resolving neither
    /// deletes the item nor stops its delivery; it is a thread state the reviewer
    /// toggles, preserved in the append-only log. Defaults `false`; an omitted field
    /// (older logs, non-resolve posts) reads as unresolved.
    #[serde(default)]
    pub resolved: bool,
}

/// Error when a feedback item names no resolvable target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoTarget;

impl FeedbackItem {
    /// Build an item, rejecting one that names no resolvable target — every item
    /// (including an answer, which targets its `question` block) must carry a
    /// non-empty block `#id`.
    ///
    /// # Errors
    /// Returns [`NoTarget`] when `block_id` is empty or whitespace.
    pub fn new(
        block_id: impl Into<String>,
        sub_target: Option<SubTarget>,
        kind: FeedbackKind,
        body: impl Into<String>,
    ) -> Result<Self, NoTarget> {
        let block_id = block_id.into();
        if block_id.trim().is_empty() {
            return Err(NoTarget);
        }
        Ok(Self {
            block_id,
            sub_target,
            kind,
            body: body.into(),
            status: FeedbackStatus::New,
            target: FeedbackTarget::Agent,
            resolved: false,
        })
    }

    /// The anchor rendered in TOON: `#id` or `#id/<sub-target>`.
    #[must_use]
    pub fn anchor(&self) -> String {
        match &self.sub_target {
            Some(sub) => format!("#{}/{}", self.block_id, sub.describe()),
            None => format!("#{}", self.block_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_to_lowercase_tokens() {
        assert_eq!(
            serde_json::to_string(&FeedbackKind::Annotation).unwrap(),
            "\"annotation\""
        );
        assert_eq!(
            serde_json::to_string(&FeedbackKind::Answer).unwrap(),
            "\"answer\""
        );
        assert_eq!(
            serde_json::to_string(&FeedbackKind::Finding).unwrap(),
            "\"finding\""
        );
        let back: FeedbackKind = serde_json::from_str("\"finding\"").unwrap();
        assert_eq!(back, FeedbackKind::Finding);
    }

    #[test]
    fn status_serializes_to_lowercase_and_defaults_to_new() {
        assert_eq!(FeedbackStatus::default(), FeedbackStatus::New);
        assert_eq!(
            serde_json::to_string(&FeedbackStatus::New).unwrap(),
            "\"new\""
        );
        assert_eq!(
            serde_json::to_string(&FeedbackStatus::Update).unwrap(),
            "\"update\""
        );
        assert_eq!(
            serde_json::to_string(&FeedbackStatus::Delete).unwrap(),
            "\"delete\""
        );
        let back: FeedbackStatus = serde_json::from_str("\"delete\"").unwrap();
        assert_eq!(back, FeedbackStatus::Delete);
    }

    #[test]
    fn new_item_defaults_to_new_status() {
        let item = FeedbackItem::new("b", None, FeedbackKind::Annotation, "x").unwrap();
        assert_eq!(item.status, FeedbackStatus::New);
    }

    #[test]
    fn target_defaults_to_agent_and_serializes_lowercase() {
        let item = FeedbackItem::new("b", None, FeedbackKind::Annotation, "x").unwrap();
        assert_eq!(item.target, FeedbackTarget::Agent);
        assert_eq!(FeedbackTarget::default(), FeedbackTarget::Agent);
        assert_eq!(
            serde_json::to_string(&FeedbackTarget::Human).unwrap(),
            "\"human\""
        );
        let back: FeedbackTarget = serde_json::from_str("\"mention\"").unwrap();
        assert_eq!(back, FeedbackTarget::Mention);
    }

    #[test]
    fn subtarget_descriptions() {
        assert_eq!(SubTarget::Node("Auth".into()).describe(), "node:Auth");
        assert_eq!(SubTarget::Cell { row: 1, col: 2 }.describe(), "cell:1,2");
        assert_eq!(SubTarget::Lines("12-18".into()).describe(), "line:12-18");
        let text = SubTarget::Text {
            quote: "q".into(),
            before: "b".into(),
            after: "a".into(),
        };
        assert_eq!(text.describe(), "text:q");
    }

    #[test]
    fn item_requires_a_block_target() {
        assert_eq!(
            FeedbackItem::new("", None, FeedbackKind::Annotation, "hi"),
            Err(NoTarget)
        );
        assert_eq!(
            FeedbackItem::new("   ", None, FeedbackKind::Answer, "Rust"),
            Err(NoTarget)
        );
    }

    #[test]
    fn resolved_defaults_false_and_round_trips_when_omitted() {
        let item = FeedbackItem::new("b", None, FeedbackKind::Annotation, "x").unwrap();
        assert!(!item.resolved, "a new item is unresolved");
        // An older log line with no `resolved` field deserializes as unresolved.
        let legacy: FeedbackItem =
            serde_json::from_str(r#"{"block_id":"b","kind":"annotation","body":"x"}"#).unwrap();
        assert!(!legacy.resolved);
        // A resolved item serializes the flag and round-trips.
        let mut done = item.clone();
        done.resolved = true;
        let back: FeedbackItem =
            serde_json::from_str(&serde_json::to_string(&done).unwrap()).unwrap();
        assert!(back.resolved);
    }

    #[test]
    fn item_anchor_reflects_sub_target() {
        let block = FeedbackItem::new("flow", None, FeedbackKind::Annotation, "x").unwrap();
        assert_eq!(block.anchor(), "#flow");
        let node = FeedbackItem::new(
            "flow",
            Some(SubTarget::Node("Auth".into())),
            FeedbackKind::Annotation,
            "check this",
        )
        .unwrap();
        assert_eq!(node.anchor(), "#flow/node:Auth");
    }
}
