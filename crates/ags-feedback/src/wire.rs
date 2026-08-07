//! Feedback wire formats — both directions of the return leg.
//!
//! Outbound (store → agent): [`poll_to_toon`] encodes delivered feedback + the
//! ended flag as TOON via `toon-rs`, from a flat view of each item — the same TOON
//! family as the Gate-1 error output, so the agent parses one format for both legs.
//!
//! Inbound (browser → store): [`parse_feedback_json`] parses one submitted item
//! from JSON, funnelling it through [`FeedbackItem::new`] so the no-target rule is
//! enforced on the wire exactly as it is in the library.

use serde::Serialize;

use crate::model::{FeedbackItem, FeedbackKind, FeedbackStatus, FeedbackTarget};

/// Parse one inbound feedback item (browser → store) from JSON. The accepted
/// shape is the item's own serde form — the browser posts what the store persists:
/// `{"status":"update","block_id":"flow","sub_target":{"Node":"Auth"},"kind":"annotation","body":"…"}`
/// (`sub_target` optional/`null` = block-level; `status` optional, defaulting to
/// `new`; `kind` ∈ `annotation|answer|finding`). An empty block target is rejected
/// on the wire, not just at the API.
///
/// # Errors
/// Returns a human-readable message when the body is not a valid item or names no
/// resolvable target.
pub fn parse_feedback_json(body: &str) -> Result<FeedbackItem, String> {
    let item: FeedbackItem =
        serde_json::from_str(body).map_err(|e| format!("bad feedback json: {e}"))?;
    if item.block_id.trim().is_empty() {
        return Err("feedback names no resolvable target".to_string());
    }
    Ok(item)
}

/// A flattened feedback row for the tabular TOON output — the routing `target`,
/// lifecycle `status`, and reviewer-facing `resolved` axis, plus the item's
/// `#id`+sub-target collapsed to a single `anchor` (its identity: one annotation per
/// element).
#[derive(Serialize)]
struct FeedbackRow {
    target: FeedbackTarget,
    status: FeedbackStatus,
    resolved: bool,
    /// Whether the anchor no longer points at anything in the artifact as it now
    /// stands — the node was redrawn away, the block deleted, the quoted text
    /// rewritten. Derived per poll rather than stored, because it is a fact about
    /// the current file and the same log gives a different answer against a newer
    /// one. The item is still returned: the agent made the edit and is the only
    /// one who can reconcile it.
    detached: bool,
    anchor: String,
    kind: FeedbackKind,
    body: String,
}

/// A poll response: the rows delivered this pass plus the review state. `ended` = the
/// human finished; `closed` = the human left without finishing (the serving instance is
/// gone). At most one is true; both false means the review is still open.
#[derive(Serialize)]
struct PollResponse {
    feedback: Vec<FeedbackRow>,
    ended: bool,
    closed: bool,
}

/// Encode a poll response as TOON:
///
/// ```text
/// feedback[1]{target,status,resolved,detached,anchor,kind,body}:
///   agent,new,false,false,#flow/node:Auth,annotation,the arrow points the wrong way
/// ended: false
/// closed: false
/// ```
///
/// `detached` decides, per item, whether its anchor still points at anything.
///
/// Asked rather than worked out here, and that is the crate boundary: answering it
/// means knowing what a block id means, what a diagram drew, whether a quote
/// survived a rewrite — all of which belong to whatever produced the artifact.
/// This crate only knows that an item names a target and that the answer belongs
/// in a column. A caller with no renderer at all can pass `|_| false`.
#[must_use]
pub fn poll_to_toon(
    items: &[FeedbackItem],
    ended: bool,
    closed: bool,
    detached: impl Fn(&FeedbackItem) -> bool,
) -> String {
    let response = PollResponse {
        feedback: items
            .iter()
            .map(|i| FeedbackRow {
                target: i.target,
                status: i.status,
                resolved: i.resolved,
                detached: detached(i),
                anchor: i.anchor(),
                kind: i.kind,
                body: i.body.clone(),
            })
            .collect(),
        ended,
        closed,
    };
    toon_rs::encode_to_string(&response, &toon_rs::Options::default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SubTarget;

    fn item(block: &str, sub: Option<SubTarget>, kind: FeedbackKind, body: &str) -> FeedbackItem {
        FeedbackItem::new(block, sub, kind, body).unwrap()
    }

    /// Every anchor resolves. This crate cannot decide that for itself — deciding
    /// it is precisely what it delegates — so a test says so directly.
    fn attached(_: &FeedbackItem) -> bool {
        false
    }

    #[test]
    fn rows_carry_target_status_resolved_detached_anchor_kind_body() {
        let mut edit = item(
            "flow",
            Some(SubTarget::Node("Auth".into())),
            FeedbackKind::Annotation,
            "wrong way",
        );
        edit.status = FeedbackStatus::Update;
        edit.target = FeedbackTarget::Human;
        edit.resolved = true;
        let items = [edit, item("commit", None, FeedbackKind::Answer, "Rust")];
        let toon = poll_to_toon(&items, false, false, attached);
        assert!(
            toon.starts_with("feedback[2]{target,status,resolved,detached,anchor,kind,body}:"),
            "{toon}"
        );
        // node sub-target's `:` is TOON-significant, so the anchor is quoted; the
        // reviewer marked this thread resolved, and `Auth` is still drawn.
        assert!(
            toon.contains("human,update,true,false,\"#flow/node:Auth\",annotation,wrong way"),
            "{toon}"
        );
        // the answer defaults to `agent`/`new`/unresolved, and still resolves.
        assert!(
            toon.contains("agent,new,false,false,#commit,answer,Rust"),
            "{toon}"
        );
        assert!(toon.contains("\nended: false"));
        assert!(toon.ends_with("\nclosed: false"));
    }

    #[test]
    fn a_row_whose_anchor_no_longer_resolves_says_so() {
        // The column is whatever the caller said it is: this crate's contract is to
        // report the verdict faithfully, not to reach one.
        let items = [item(
            "flow",
            Some(SubTarget::Node("Billing".into())),
            FeedbackKind::Annotation,
            "wrong way",
        )];
        let toon = poll_to_toon(&items, false, false, |_| true);
        assert!(
            toon.contains("agent,new,false,true,\"#flow/node:Billing\",annotation,wrong way"),
            "{toon}"
        );
    }

    #[test]
    fn body_with_comma_is_quoted() {
        let items = [item(
            "h",
            None,
            FeedbackKind::Finding,
            "overflow, clipped text",
        )];
        assert!(poll_to_toon(&items, false, false, attached).contains("\"overflow, clipped text\""));
    }

    #[test]
    fn empty_pass_reports_ended_and_closed() {
        // An empty array has no column spec in canonical TOON; both state flags print.
        assert_eq!(
            poll_to_toon(&[], true, false, attached),
            "feedback[0]:\nended: true\nclosed: false"
        );
        assert_eq!(
            poll_to_toon(&[], false, false, attached),
            "feedback[0]:\nended: false\nclosed: false"
        );
        assert_eq!(
            poll_to_toon(&[], false, true, attached),
            "feedback[0]:\nended: false\nclosed: true"
        );
    }

    #[test]
    fn parses_a_block_level_annotation() {
        let item = parse_feedback_json(
            r#"{"block_id":"flow","sub_target":null,"kind":"annotation","body":"looks off"}"#,
        )
        .unwrap();
        assert_eq!(item.block_id, "flow");
        assert_eq!(item.sub_target, None);
        assert_eq!(item.kind, FeedbackKind::Annotation);
        assert_eq!(item.body, "looks off");
    }

    #[test]
    fn parses_a_node_sub_target_and_an_answer() {
        let ann = parse_feedback_json(
            r#"{"block_id":"flow","sub_target":{"Node":"Auth"},"kind":"annotation","body":"wrong way"}"#,
        )
        .unwrap();
        assert_eq!(ann.anchor(), "#flow/node:Auth");
        // A missing `sub_target` field is block-level (serde treats it as None).
        let answer =
            parse_feedback_json(r#"{"block_id":"q","kind":"answer","body":"Rust"}"#).unwrap();
        assert_eq!(answer.kind, FeedbackKind::Answer);
        assert_eq!(answer.anchor(), "#q");
    }

    #[test]
    fn rejects_malformed_and_targetless_bodies() {
        parse_feedback_json("not json").unwrap_err();
        // an empty block id names no resolvable target
        parse_feedback_json(r#"{"block_id":"","kind":"annotation","body":"x"}"#).unwrap_err();
    }

    #[test]
    fn parses_lifecycle_status() {
        let del = parse_feedback_json(
            r#"{"status":"delete","block_id":"flow","kind":"annotation","body":""}"#,
        )
        .unwrap();
        assert_eq!(del.status, FeedbackStatus::Delete);
        // an omitted status defaults to `new`.
        let plain =
            parse_feedback_json(r#"{"block_id":"q","kind":"answer","body":"Rust"}"#).unwrap();
        assert_eq!(plain.status, FeedbackStatus::New);
    }
}
