//! Where each commit sits.
//!
//! One column per commit in the order they were written, one lane per branch in
//! the order it first appeared. An edge to a parent on the same lane is a
//! straight line; one that changes lane curves with horizontal tangents, so a
//! branch leaves and rejoins smoothly rather than at a corner.

use crate::round::count;
use crate::scene::Point;

use super::types::{CommitType, Graph};

pub const PADDING: f64 = 24.0;
pub const NODE_R: f64 = 7.0;
pub const COL_GAP: f64 = 46.0;
pub const LANE_GAP: f64 = 52.0;
/// Reserved above the top lane for tags.
pub const TAG_SPACE: f64 = 26.0;
/// Reserved below the bottom lane for commit ids.
pub const LABEL_SPACE: f64 = 22.0;
pub const LABEL_FONT: f64 = 11.0;
pub const LABEL_WEIGHT: u32 = 400;
pub const BRANCH_FONT: f64 = 12.0;
pub const BRANCH_WEIGHT: u32 = 600;
pub const BRANCH_GAP: f64 = 14.0;
pub const TAG_FONT: f64 = 11.0;
pub const TAG_WEIGHT: u32 = 600;

/// One commit, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCommit {
    pub id: String,
    pub at: Point,
    pub kind: CommitType,
    pub is_merge: bool,
    pub tag: Option<String>,
    /// The lane it sits on, which is also its colour.
    pub color_index: usize,
}

/// One edge from a parent to a child.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEdge {
    pub from: String,
    pub to: String,
    /// Two points for a straight run, four for a cubic.
    pub curve: Vec<Point>,
    pub color_index: usize,
}

/// One branch's name, in the left-hand gutter.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchLabel {
    pub name: String,
    pub at: Point,
    pub color_index: usize,
}

/// A laid-out git graph.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub commits: Vec<PlacedCommit>,
    pub edges: Vec<PlacedEdge>,
    pub branch_labels: Vec<BranchLabel>,
}

/// The run between two commits.
///
/// Colinear means the same lane, which is the common case and wants a straight
/// line; anything else is a branch or a merge and curves.
fn connector(from: Point, to: Point) -> Vec<Point> {
    if (from.y - to.y).abs() < 0.5 {
        return vec![from, to];
    }
    let dx = (to.x - from.x) / 2.0;
    vec![
        from,
        Point::new(from.x + dx, from.y),
        Point::new(to.x - dx, to.y),
        to,
    ]
}

/// Lay out a parsed git graph.
pub fn layout(graph: &Graph) -> Placed {
    // The gutter is as wide as the longest branch name, so no name overlaps
    // the first commit whatever the branches are called.
    let widest = graph
        .branches
        .iter()
        .map(|b| crate::metrics::text_width(&b.name, BRANCH_FONT, BRANCH_WEIGHT))
        .fold(0.0_f64, f64::max);
    let left = PADDING + widest + BRANCH_GAP;
    let top = PADDING + TAG_SPACE;
    let lane_y = |order: usize| top + count(order) * LANE_GAP;

    let order_of = |name: &str| {
        graph
            .branches
            .iter()
            .find(|b| b.name == name)
            .map_or(0, |b| b.order)
    };

    let commits: Vec<PlacedCommit> = graph
        .commits
        .iter()
        .enumerate()
        .map(|(column, commit)| {
            let order = order_of(&commit.branch);
            PlacedCommit {
                id: commit.id.clone(),
                at: Point::new(left + count(column) * COL_GAP + NODE_R, lane_y(order)),
                kind: commit.kind,
                is_merge: commit.is_merge,
                tag: commit.tag.clone(),
                color_index: order,
            }
        })
        .collect();

    let find = |id: &str| commits.iter().find(|c| c.id == id);
    let edges: Vec<PlacedEdge> = graph
        .commits
        .iter()
        .filter_map(|commit| find(&commit.id).map(|child| (commit, child)))
        .flat_map(|(commit, child)| {
            commit
                .parents
                .iter()
                .enumerate()
                // A parent named but never committed is dropped rather than
                // drawn from nowhere — a cherry-pick may name anything.
                .filter_map(|(index, parent_id)| {
                    let parent = find(parent_id)?;
                    Some(PlacedEdge {
                        from: parent_id.clone(),
                        to: child.id.clone(),
                        curve: connector(parent.at, child.at),
                        // The first parent is this commit's own history, so it
                        // takes its colour; a merged-in one keeps the colour of
                        // where it came from.
                        color_index: if index == 0 {
                            child.color_index
                        } else {
                            parent.color_index
                        },
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let branch_labels = graph
        .branches
        .iter()
        .map(|branch| BranchLabel {
            name: branch.name.clone(),
            at: Point::new(PADDING, lane_y(branch.order)),
            color_index: branch.order,
        })
        .collect();

    let last_x = commits.iter().map(|c| c.at.x).fold(left, f64::max);
    let last_lane = graph.branches.len().saturating_sub(1);
    Placed {
        width: last_x + NODE_R + COL_GAP / 2.0 + PADDING,
        height: lane_y(last_lane) + NODE_R + LABEL_SPACE + PADDING,
        commits,
        edges,
        branch_labels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitgraph::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const GRAPH: &str = "gitGraph\n\
        commit id: \"one\"\n\
        branch feature\n\
        commit id: \"two\"\n\
        checkout main\n\
        commit id: \"three\"\n\
        merge feature id: \"four\"";

    #[test]
    fn one_column_per_commit_in_the_order_written() {
        let out = placed(GRAPH);
        let xs: Vec<f64> = out.commits.iter().map(|c| c.at.x).collect();
        for pair in xs.windows(2) {
            assert!((pair[1] - pair[0] - COL_GAP).abs() < 1e-9);
        }
    }

    #[test]
    fn one_lane_per_branch_in_the_order_it_appeared() {
        let out = placed(GRAPH);
        // main is lane 0, feature lane 1.
        assert!((out.commits[0].at.y - out.commits[2].at.y).abs() < 1e-9);
        assert!((out.commits[1].at.y - out.commits[0].at.y - LANE_GAP).abs() < 1e-9);
    }

    #[test]
    fn an_edge_along_one_lane_is_straight_and_one_that_changes_lane_curves() {
        let out = placed(GRAPH);
        let same = out
            .edges
            .iter()
            .find(|e| e.from == "one" && e.to == "three")
            .expect("main to main");
        assert_eq!(same.curve.len(), 2);
        let across = out
            .edges
            .iter()
            .find(|e| e.from == "one" && e.to == "two")
            .expect("main to feature");
        assert_eq!(across.curve.len(), 4, "a cubic");
        // Horizontal tangents: each control point is level with its own end.
        assert!((across.curve[1].y - across.curve[0].y).abs() < 1e-9);
        assert!((across.curve[2].y - across.curve[3].y).abs() < 1e-9);
    }

    #[test]
    fn a_merge_edge_keeps_the_colour_of_where_it_came_from() {
        let out = placed(GRAPH);
        let merge = out
            .edges
            .iter()
            .find(|e| e.from == "two" && e.to == "four")
            .expect("the merged-in edge");
        assert_eq!(merge.color_index, 1, "feature's lane, not main's");
        let own = out
            .edges
            .iter()
            .find(|e| e.from == "three" && e.to == "four")
            .expect("its own history");
        assert_eq!(own.color_index, 0);
    }

    #[test]
    fn an_edge_to_a_parent_that_was_never_committed_is_dropped() {
        // A cherry-pick may name anything at all.
        let out = placed("gitGraph\ncommit id: \"a\"\ncherry-pick id: \"ghost\"");
        assert_eq!(out.edges.len(), 1, "only the run along main");
    }

    #[test]
    fn a_branch_name_sits_in_a_gutter_wide_enough_for_it() {
        let short = placed("gitGraph\ncommit");
        let long = placed("gitGraph\nbranch a-very-long-branch-name-indeed\ncommit");
        assert!(long.commits[0].at.x > short.commits[0].at.x);
        // And the names line up with their own lanes.
        let out = placed(GRAPH);
        assert!((out.branch_labels[1].at.y - out.commits[1].at.y).abs() < 1e-9);
    }

    #[test]
    fn room_is_kept_above_for_tags_and_below_for_ids() {
        let out = placed(GRAPH);
        assert!(out.commits[0].at.y - NODE_R >= PADDING);
        let lowest = out.commits.iter().map(|c| c.at.y).fold(0.0_f64, f64::max);
        assert!(out.height >= lowest + NODE_R + LABEL_SPACE + PADDING - 1e-9);
    }

    #[test]
    fn a_commit_carries_the_kind_and_tag_it_was_given() {
        let out = placed("gitGraph\ncommit type: HIGHLIGHT tag: \"v1\"\ncommit type: REVERSE");
        assert_eq!(out.commits[0].kind, CommitType::Highlight);
        assert_eq!(out.commits[0].tag.as_deref(), Some("v1"));
        assert_eq!(out.commits[1].kind, CommitType::Reverse);
    }

    #[test]
    fn an_empty_graph_still_yields_a_canvas() {
        let out = placed("gitGraph");
        assert!(out.commits.is_empty());
        assert!(out.width > 0.0);
        assert_eq!(out.branch_labels.len(), 1, "main");
    }
}
