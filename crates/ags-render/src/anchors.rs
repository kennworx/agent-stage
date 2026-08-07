//! Which anchors the artifact can still resolve — and so which feedback detached.
//!
//! A review outlives the thing it reviews. The human comments on the `Auth` node,
//! the agent redraws the diagram without it, and that comment now points at
//! nothing. Dropping it would hide the one item most in need of an answer, so it
//! comes back marked **detached** and the agent reconciles it.
//!
//! This used to be a DOM walk in the viewer: render, then look for the element the
//! anchor names. The renderer runs here now and knows what it drew, so the same
//! question is a set difference over what the artifact currently offers — no
//! browser, and answerable by `ags poll` without a page open at all.
//!
//! Detachment is *derived*, never stored. It is a fact about the artifact as it
//! stands, so the same log read against a newer file gives a different answer;
//! writing it into the log would freeze one render's verdict over every later one.

use std::collections::{HashMap, HashSet};

use ags_mermaid::{Content, Node, Options, Scene};

use crate::block::Block;
use ags_feedback::{FeedbackItem, SubTarget};

/// What one block can currently be pointed at.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    /// A diagram, and every `data-id` it drew.
    Diagram(HashSet<String>),
    /// A table, by its row and column counts.
    Table { rows: usize, cols: usize },
    /// A block addressed by line — its line count.
    Lines(usize),
    /// Anything else. Block-level feedback resolves; nothing finer does.
    Whole,
}

/// Every anchor the artifact currently resolves, built once and asked many times.
#[derive(Debug, Clone, Default)]
pub struct Anchors {
    /// Block id (as feedback stores it, without the `#`) to what it offers.
    blocks: HashMap<String, Target>,
    /// Each block's body, for locating a quoted text range.
    bodies: HashMap<String, String>,
}

/// Collect a scene's element ids — the `data-id`s the SVG will carry.
///
/// Recursive because a diagram puts the identity on a group and the geometry on
/// its children; the group is the thing a reviewer clicks and the id that reaches
/// the feedback item.
fn scene_ids(nodes: &[Node], out: &mut HashSet<String>) {
    for node in nodes {
        if let Some(id) = &node.id {
            out.insert(id.clone());
        }
        if let Content::Group(children) = &node.content {
            scene_ids(children, out);
        }
    }
}

/// Every `data-id` a diagram source draws, or none when it will not draw.
fn diagram_ids(body: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    if let Ok(scene) = ags_mermaid::inspect(body, &Options::default()) {
        let Scene { nodes, .. } = &scene;
        scene_ids(nodes, &mut ids);
    }
    ids
}

/// A table's shape: how many rows of cells, and the widest row's column count.
///
/// The separator row (`|---|---|`) is not a row of data and is not counted, so a
/// cell anchor's row index means what a reader would think it means.
fn table_shape(body: &str) -> (usize, usize) {
    let rows: Vec<Vec<&str>> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_matches('|').split('|').collect())
        .filter(|cells: &Vec<&str>| {
            !cells
                .iter()
                .all(|c| !c.trim().is_empty() && c.trim().chars().all(|ch| ch == '-' || ch == ':'))
        })
        .collect();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    (rows.len(), cols)
}

/// What a block offers to point at.
fn target_for(block: &Block) -> Target {
    match block.type_token.as_str() {
        "mermaid" => Target::Diagram(diagram_ids(&block.body)),
        "table" => {
            let (rows, cols) = table_shape(&block.body);
            Target::Table { rows, cols }
        }
        "code" => Target::Lines(block.body.lines().count()),
        _ => Target::Whole,
    }
}

/// The ids a block answers to: its `#id`, and the `<type>@<ordinal>` fallback that
/// addresses one the author never named.
fn ids_for(block: &Block) -> Vec<String> {
    let mut ids = vec![format!("{}@{}", block.type_token, block.ordinal)];
    if let Some(id) = &block.id {
        ids.push(id.clone());
    }
    ids
}

/// Read what `source` currently offers to anchor against.
#[must_use]
pub fn anchors(source: &str) -> Anchors {
    let artifact = crate::parse::parse_artifact(source);
    let mut found = Anchors::default();
    for block in &artifact.blocks {
        let target = target_for(block);
        for id in ids_for(block) {
            found.blocks.insert(id.clone(), target.clone());
            found.bodies.insert(id, block.body.clone());
        }
    }
    found
}

/// The largest line number a `code` sub-target names (`"12"` or `"12-18"`).
///
/// An unparseable range yields `None`, which is treated as resolving: an anchor the
/// tool cannot read is not evidence that the content moved, and guessing "detached"
/// would put a badge on an item nothing is wrong with.
fn last_line(range: &str) -> Option<usize> {
    range
        .split('-')
        .map(|part| part.trim().parse::<usize>().ok())
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max()
}

impl Anchors {
    /// Whether `item`'s anchor still points at something.
    ///
    /// A block that is gone detaches everything on it. A block that is still there
    /// resolves block-level feedback outright; a sub-target has to be found within
    /// it.
    #[must_use]
    pub fn resolves(&self, item: &FeedbackItem) -> bool {
        let Some(target) = self.blocks.get(&item.block_id) else {
            return false;
        };
        let Some(sub) = &item.sub_target else {
            return true;
        };
        match (sub, target) {
            (SubTarget::Node(id), Target::Diagram(ids)) => ids.contains(id),
            (SubTarget::Cell { row, col }, Target::Table { rows, cols }) => {
                row < rows && col < cols
            }
            (SubTarget::Lines(range), Target::Lines(count)) => {
                last_line(range).is_none_or(|last| last <= *count)
            }
            (SubTarget::Text { quote, .. }, _) => self
                .bodies
                .get(&item.block_id)
                .is_some_and(|body| body.contains(quote)),
            // A sub-target of the wrong shape for the block it names — a cell
            // anchor on what is now a diagram. The block was rewritten into
            // something else, which is precisely a detachment.
            _ => false,
        }
    }

    /// Whether `item`'s anchor no longer points at anything.
    #[must_use]
    pub fn detached(&self, item: &FeedbackItem) -> bool {
        !self.resolves(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ags_feedback::FeedbackKind;

    const FLOW: &str = "```mermaid #flow\ngraph TD\n  Auth[Sign in] --> Home\n```\n";

    fn on(block: &str, sub: Option<SubTarget>) -> FeedbackItem {
        FeedbackItem::new(block, sub, FeedbackKind::Annotation, "a note").unwrap()
    }

    #[test]
    fn block_level_feedback_resolves_while_its_block_exists() {
        let found = anchors(FLOW);
        assert!(found.resolves(&on("flow", None)));
        assert!(found.detached(&on("gone", None)));
    }

    #[test]
    fn a_diagram_node_resolves_only_while_the_diagram_draws_it() {
        let found = anchors(FLOW);
        assert!(
            found.resolves(&on("flow", Some(SubTarget::Node("Auth".into())))),
            "Auth is in the diagram"
        );
        assert!(
            found.detached(&on("flow", Some(SubTarget::Node("Billing".into())))),
            "Billing never was"
        );
    }

    #[test]
    fn redrawing_without_a_node_detaches_its_annotation() {
        // The case the whole thing exists for: the human commented on `Auth`, the
        // agent redrew without it. The comment must come back, marked.
        let item = on("flow", Some(SubTarget::Node("Auth".into())));
        assert!(anchors(FLOW).resolves(&item));
        let redrawn = "```mermaid #flow\ngraph TD\n  Home --> Done\n```\n";
        assert!(anchors(redrawn).detached(&item));
    }

    #[test]
    fn a_node_that_comes_back_reattaches() {
        // Detachment is derived, so it is not a one-way door: restoring the node
        // resolves the anchor again with nothing to undo.
        let item = on("flow", Some(SubTarget::Node("Auth".into())));
        let without = "```mermaid #flow\ngraph TD\n  Home --> Done\n```\n";
        assert!(anchors(without).detached(&item));
        assert!(anchors(FLOW).resolves(&item));
    }

    #[test]
    fn an_id_less_block_is_addressable_by_type_and_ordinal() {
        let found = anchors("```mermaid\ngraph TD\n  A-->B\n```\n");
        assert!(found.resolves(&on("mermaid@0", None)));
    }

    #[test]
    fn a_table_cell_resolves_inside_the_grid_and_detaches_outside_it() {
        let source = "```table #t\n| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n```\n";
        let found = anchors(source);
        // Header plus two data rows; the separator is not a row.
        assert!(found.resolves(&on("t", Some(SubTarget::Cell { row: 2, col: 1 }))));
        assert!(found.detached(&on("t", Some(SubTarget::Cell { row: 3, col: 0 }))));
        assert!(found.detached(&on("t", Some(SubTarget::Cell { row: 0, col: 2 }))));
    }

    #[test]
    fn a_code_line_resolves_within_the_excerpt() {
        let source = "```code #c lang=rust\nfn a() {}\nfn b() {}\nfn c() {}\n```\n";
        let found = anchors(source);
        assert!(found.resolves(&on("c", Some(SubTarget::Lines("2".into())))));
        assert!(found.resolves(&on("c", Some(SubTarget::Lines("1-3".into())))));
        assert!(found.detached(&on("c", Some(SubTarget::Lines("4".into())))));
        assert!(found.detached(&on("c", Some(SubTarget::Lines("2-9".into())))));
    }

    #[test]
    fn an_unreadable_line_range_is_left_attached() {
        // Not evidence the content moved, so not grounds for a badge.
        let source = "```code #c lang=rust\nfn a() {}\n```\n";
        assert!(anchors(source).resolves(&on("c", Some(SubTarget::Lines("what".into())))));
        assert_eq!(last_line("12-18"), Some(18));
        assert_eq!(last_line("7"), Some(7));
        assert_eq!(last_line("x-2"), None);
    }

    #[test]
    fn a_text_range_resolves_while_its_quote_survives_the_rewrite() {
        let source = "```note #n kind=claim\nRust is the right host for v1.\n```\n";
        let quote = |text: &str| {
            Some(SubTarget::Text {
                quote: text.into(),
                before: String::new(),
                after: String::new(),
            })
        };
        let found = anchors(source);
        assert!(found.resolves(&on("n", quote("the right host"))));
        assert!(found.detached(&on("n", quote("the wrong host"))));
    }

    #[test]
    fn a_sub_target_of_the_wrong_shape_is_detached() {
        // The block id survived a rewrite that turned a table into a diagram, so a
        // cell anchor names something with no cells. That is a detachment, not a
        // resolution.
        let found = anchors(FLOW);
        assert!(found.detached(&on("flow", Some(SubTarget::Cell { row: 0, col: 0 }))));
        assert!(found.detached(&on("flow", Some(SubTarget::Lines("1".into())))));
    }

    #[test]
    fn a_diagram_that_no_longer_draws_offers_no_nodes() {
        // Gate 1 refuses to serve this, but `poll` reads whatever the file says now
        // — including a broken intermediate state — and must not panic on it.
        let found = anchors("```mermaid #flow\nsunburstChart\n  a: 1\n```\n");
        assert!(
            found.resolves(&on("flow", None)),
            "the block is still there"
        );
        assert!(found.detached(&on("flow", Some(SubTarget::Node("Auth".into())))));
    }

    #[test]
    fn an_empty_artifact_resolves_nothing() {
        let found = anchors("");
        assert!(found.detached(&on("anything", None)));
    }
}
