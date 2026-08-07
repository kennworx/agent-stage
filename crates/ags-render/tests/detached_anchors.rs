//! Where the two crates meet: a comment survives a redraw, and says it did.
//!
//! `ags-feedback` carries an item and reports whatever verdict it is handed;
//! `ags-render` reaches a verdict by asking what the artifact currently draws.
//! Neither can demonstrate the behaviour alone, which is why this lives here
//! rather than in either one's unit tests — it is the seam, tested as a seam.

use ags_feedback::{poll_to_toon, FeedbackItem, FeedbackKind, SubTarget};
use ags_render::anchors;

const WITH_AUTH: &str = "```mermaid #flow\ngraph TD\n  Auth[Sign in] --> Home\n```\n";
const WITHOUT_AUTH: &str = "```mermaid #flow\ngraph TD\n  Home --> Done\n```\n";

fn comment_on_auth() -> FeedbackItem {
    FeedbackItem::new(
        "flow",
        Some(SubTarget::Node("Auth".into())),
        FeedbackKind::Annotation,
        "this arrow points the wrong way",
    )
    .expect("a comment naming a block has a resolvable target")
}

/// The TOON a poll would return for these items against this artifact.
fn rows(items: &[FeedbackItem], source: &str) -> String {
    let resolved = anchors(source);
    poll_to_toon(items, false, false, |item| resolved.detached(item))
}

#[test]
fn redrawing_without_a_node_detaches_its_comment_but_still_delivers_it() {
    let items = [comment_on_auth()];

    let before = rows(&items, WITH_AUTH);
    assert!(
        before.contains("false,\"#flow/node:Auth\""),
        "Auth is drawn, so the anchor resolves: {before}"
    );

    let after = rows(&items, WITHOUT_AUTH);
    assert!(
        after.contains("true,\"#flow/node:Auth\""),
        "Auth is gone, so the anchor does not: {after}"
    );
    assert!(
        after.contains("this arrow points the wrong way"),
        "and the comment is still returned — losing it is the failure this \
         whole axis exists to prevent: {after}"
    );
}

#[test]
fn restoring_the_node_reattaches_the_comment() {
    // Derived per poll, so detachment is not a one-way door and there is nothing
    // to undo when the agent puts the node back.
    let items = [comment_on_auth()];
    assert!(rows(&items, WITHOUT_AUTH).contains("true,\"#flow/node:Auth\""));
    assert!(rows(&items, WITH_AUTH).contains("false,\"#flow/node:Auth\""));
}

#[test]
fn a_deleted_artifact_detaches_everything_rather_than_reporting_it_fine() {
    let items =
        [FeedbackItem::new("a", None, FeedbackKind::Annotation, "hi").expect("names a block")];
    let toon = rows(&items, "");
    assert!(toon.contains("true,#a,annotation,hi"), "{toon}");
}

#[test]
fn a_block_level_comment_survives_an_edit_inside_the_block() {
    // Only the sub-target was at risk; the block is still there, so a comment on
    // the block as a whole still points at something.
    let items = [
        FeedbackItem::new("flow", None, FeedbackKind::Annotation, "rethink this")
            .expect("names a block"),
    ];
    assert!(rows(&items, WITHOUT_AUTH).contains("false,#flow,annotation,rethink this"));
}
