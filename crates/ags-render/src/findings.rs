//! Legibility findings, from the drawing to the agent.
//!
//! [`crate::validate`] refuses an artifact whose diagram will not draw. This is
//! the other half: a diagram that *does* draw, but reads badly. Those are not
//! grounds to deny a human the review — they are something the agent should know
//! and can act on, so they travel the feedback channel as
//! [`FeedbackKind::Finding`] items and come back out of `ags poll` beside the
//! human's own replies.
//!
//! **One finding per diagram block, not per violation.** An item's identity in the
//! log is `(kind, anchor)`, so several findings on one block would collapse into
//! whichever was written last. Collecting a block's violations into a single body
//! matches that identity instead of fighting it, and makes re-presenting the same
//! artifact idempotent: the same block yields the same anchor, and the log folds
//! the repeat away.

use ags_mermaid::{render_svg, Options};

use crate::block::Block;
use ags_feedback::{FeedbackItem, FeedbackKind, FeedbackStatus};

/// The block id a finding anchors to.
///
/// A diagram with no `#id` still gets one: `mermaid@3` is the same fallback
/// [`Block::anchor`] uses, so a finding names the block a reader can count to even
/// when the author never named it. Returned without a leading `#`, which is what
/// [`FeedbackItem`] stores.
fn finding_target(block: &Block) -> String {
    block
        .id
        .clone()
        .unwrap_or_else(|| format!("{}@{}", block.type_token, block.ordinal))
}

/// Everything one drawing gets wrong, as a single body.
///
/// Sentences rather than a structure, because a [`ags_mermaid::Violation`] already
/// renders itself as one and the reader is a model deciding whether to redraw.
fn describe(violations: &[ags_mermaid::Violation]) -> String {
    violations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// One finding for each diagram block whose drawing breaks a legibility rule.
///
/// A block whose diagram draws cleanly produces nothing. A block whose diagram
/// does not draw at all produces nothing either — that is a Gate-1 error, and the
/// artifact never reaches here.
#[must_use]
pub fn render_findings(source: &str) -> Vec<FeedbackItem> {
    let artifact = crate::parse::parse_artifact(source);
    artifact
        .blocks
        .iter()
        .filter(|block| block.type_token == "mermaid")
        .filter_map(|block| {
            let rendered = render_svg(&block.body, &Options::default()).ok()?;
            if rendered.violations.is_empty() {
                return None;
            }
            FeedbackItem::new(
                finding_target(block),
                None,
                FeedbackKind::Finding,
                describe(&rendered.violations),
            )
            .ok()
        })
        .collect()
}

/// The lines to append so the recorded findings match the drawing as it stands now.
///
/// Findings are derived, not authored: a redraw that fixes a diagram should retire
/// its finding, and nothing else will retire it, because the log only ever grows.
/// So a finding recorded against a block that no longer reports anything is
/// followed by a [`FeedbackStatus::Delete`] line, which [`crate::Session`]'s replay
/// drops on the way through.
///
/// Deletes are emitted only for findings actually on record. Writing one for every
/// clean diagram would mean 117 lines per present on the gallery, to retire
/// findings that were never there.
///
/// A finding identical to the one already recorded is dropped rather than
/// rewritten. Replay would fold the repeat away regardless — three presents of the
/// gallery still settle to seven findings — but the log is the artifact's history,
/// and re-presenting an unchanged file should not add twenty-one lines saying
/// nothing changed.
#[must_use]
pub fn finding_updates(current: Vec<FeedbackItem>, recorded: &[FeedbackItem]) -> Vec<FeedbackItem> {
    let still_wrong: Vec<String> = current.iter().map(FeedbackItem::anchor).collect();
    let mut out: Vec<FeedbackItem> = current
        .into_iter()
        .filter(|item| !recorded.contains(item))
        .collect();
    for item in recorded {
        if item.kind != FeedbackKind::Finding || still_wrong.contains(&item.anchor()) {
            continue;
        }
        let mut gone = item.clone();
        gone.status = FeedbackStatus::Delete;
        out.push(gone);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A diagram that draws but reads wrong: every node on one row joined to
    /// every node on the next. In a layered drawing every pair of edges whose
    /// endpoints invert has to cross, and with three by three that is nine — in
    /// any order the rows are put in, so no later work on the layout can quietly
    /// take this fixture away.
    ///
    /// It has been a real gallery diagram twice: the `ci` subgraph enclosing a
    /// node not in it, then a state machine whose transitions crossed. The
    /// engine fixed both, which is a good problem to have and a poor way to keep
    /// a fixture.
    const VIOLATING: &str = "```mermaid #bad\ngraph TD\n  A1 --> B1\n  A1 --> B2\n  A1 --> B3\n  A2 --> B1\n  A2 --> B2\n  A2 --> B3\n  A3 --> B1\n  A3 --> B2\n  A3 --> B3\n```\n";

    #[test]
    fn a_clean_diagram_produces_no_finding() {
        assert_eq!(
            render_findings("```mermaid #ok\ngraph TD\n  A-->B\n```\n"),
            vec![]
        );
    }

    #[test]
    fn prose_and_other_blocks_are_not_examined() {
        let source = "# Title\n\nSome prose.\n\n```rust\nfn main() {}\n```\n\n```note #n kind=info\nhi\n```\n";
        assert_eq!(render_findings(source), vec![]);
    }

    #[test]
    fn an_id_less_diagram_is_still_addressable() {
        // `mermaid@0` is the same fallback a validation error uses, so a finding
        // names the block even when the author did not.
        let block = Block {
            type_token: "mermaid".into(),
            id: None,
            attrs: vec![],
            body: String::new(),
            line: 1,
            end: 3,
            ordinal: 3,
        };
        assert_eq!(finding_target(&block), "mermaid@3");
    }

    #[test]
    fn a_named_diagram_anchors_to_its_id_without_the_hash() {
        let block = Block {
            type_token: "mermaid".into(),
            id: Some("flow".into()),
            attrs: vec![],
            body: String::new(),
            line: 1,
            end: 3,
            ordinal: 0,
        };
        assert_eq!(finding_target(&block), "flow");
    }

    #[test]
    fn one_block_yields_one_finding_listing_everything_wrong() {
        // Several violations, one item: the log keys on (kind, anchor), so separate
        // items would overwrite one another and the agent would see only the last.
        let body = describe(&[
            ags_mermaid::Violation::Occluded {
                id: Some("L".into()),
            },
            ags_mermaid::Violation::OutsideCanvas {
                id: Some("Z".into()),
            },
        ]);
        assert!(body.contains("L is completely covered"), "{body}");
        assert!(body.contains("Z is drawn outside"), "{body}");
        assert!(body.contains("; "), "{body}");
    }

    #[test]
    fn a_violating_diagram_becomes_one_agent_targeted_finding() {
        let found = render_findings(VIOLATING);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].block_id, "bad");
        assert_eq!(found[0].kind, FeedbackKind::Finding);
        assert_eq!(found[0].status, FeedbackStatus::New);
        assert!(!found[0].body.is_empty());
    }

    #[test]
    fn a_finding_that_no_longer_applies_is_retired() {
        let recorded =
            vec![
                FeedbackItem::new("gone", None, FeedbackKind::Finding, "something was wrong")
                    .unwrap(),
            ];
        let updates = finding_updates(Vec::new(), &recorded);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].status, FeedbackStatus::Delete);
        assert_eq!(updates[0].block_id, "gone");
    }

    #[test]
    fn a_finding_that_still_applies_is_not_retired() {
        let current = vec![FeedbackItem::new("still", None, FeedbackKind::Finding, "now").unwrap()];
        let recorded =
            vec![FeedbackItem::new("still", None, FeedbackKind::Finding, "before").unwrap()];
        let updates = finding_updates(current, &recorded);
        assert_eq!(updates.len(), 1, "{updates:?}");
        assert_eq!(updates[0].status, FeedbackStatus::New);
        assert_eq!(updates[0].body, "now", "the fresh description wins");
    }

    #[test]
    fn an_unchanged_finding_is_not_written_again() {
        // Presenting the same file twice should leave the log as it was.
        let item = FeedbackItem::new("same", None, FeedbackKind::Finding, "still wrong").unwrap();
        assert_eq!(finding_updates(vec![item.clone()], &[item]), vec![]);
    }

    #[test]
    fn a_changed_finding_is_written_again() {
        // Same block, different verdict — the log has to learn the new one.
        let before = FeedbackItem::new("b", None, FeedbackKind::Finding, "one problem").unwrap();
        let after = FeedbackItem::new("b", None, FeedbackKind::Finding, "two problems").unwrap();
        let updates = finding_updates(vec![after], &[before]);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].body, "two problems");
        assert_eq!(updates[0].status, FeedbackStatus::New);
    }

    #[test]
    fn a_humans_annotation_is_never_retired_as_a_finding() {
        // Only findings are derived. An annotation on a block that now draws
        // cleanly is still the human's, and deleting it would discard review.
        let recorded = vec![FeedbackItem::new(
            "bad",
            None,
            FeedbackKind::Annotation,
            "I disagree with this",
        )
        .unwrap()];
        assert_eq!(finding_updates(Vec::new(), &recorded), vec![]);
    }
}
