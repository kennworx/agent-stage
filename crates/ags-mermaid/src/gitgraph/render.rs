//! A placed git graph, drawn into the scene.
//!
//! Identity contract: each commit is a group carrying `data-id`, and every edge
//! names the parent and child it joins.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Seg, Shape, Size, TextRun,
};
use crate::theme::{series_css, style_block, Theme};

use super::layout::{
    layout, BranchLabel, Placed, PlacedCommit, PlacedEdge, BRANCH_FONT, BRANCH_WEIGHT, LABEL_FONT,
    LABEL_WEIGHT, NODE_R, TAG_FONT, TAG_WEIGHT,
};
use super::types::CommitType;

const BASELINE: &str = "0.35em";
/// How far below a commit its id is written.
const LABEL_DROP: f64 = 14.0;
/// How far above a commit its tag is written.
const TAG_RISE: f64 = 8.0;

fn text(at: Point, content: &str, size: f64, weight: u32, anchor: Anchor, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor,
            font: Font {
                size,
                weight,
                italic: false,
            },
            dy: Some(BASELINE.to_string()),
            content: content.to_string(),
        }),
    )
    .classed(class)
}

fn edge_node(edge: &PlacedEdge) -> Node {
    let segs = match edge.curve.as_slice() {
        [from, to] => vec![Seg::MoveTo(*from), Seg::LineTo(*to)],
        [from, c1, c2, to] => vec![
            Seg::MoveTo(*from),
            Seg::Cubic {
                c1: *c1,
                c2: *c2,
                to: *to,
            },
        ],
        _ => Vec::new(),
    };
    Node::new(Role::Edge, Content::Shape(Shape::Path(segs)))
        .classed("git-edge")
        .classed(format!("git-stroke-{}", edge.color_index))
        .tagged("from", edge.from.clone())
        .tagged("to", edge.to.clone())
}

/// The shape a commit is drawn as, which is what its kind means.
fn commit_shape(commit: &PlacedCommit) -> Vec<Node> {
    let fill = format!("git-fill-{}", commit.color_index);
    let disc = |extra: Option<&str>| {
        let node = Node::new(
            Role::Node,
            Content::Shape(Shape::Circle {
                c: commit.at,
                r: NODE_R,
            }),
        )
        .classed("git-commit")
        .classed(fill.clone());
        match extra {
            Some(class) => node.classed(class),
            None => node,
        }
    };
    match commit.kind {
        // A square rather than a disc: a highlighted commit has to be findable
        // by shape, not only by colour.
        CommitType::Highlight => {
            let side = NODE_R * 2.6;
            vec![Node::new(
                Role::Node,
                Content::Shape(Shape::Rect {
                    at: Point::new(commit.at.x - side / 2.0, commit.at.y - side / 2.0),
                    size: Size {
                        width: side,
                        height: side,
                    },
                    rx: 2.0,
                    ry: 2.0,
                }),
            )
            .classed("git-commit")
            .classed(fill)]
        }
        CommitType::Reverse => {
            let d = NODE_R * 0.62;
            let cross = vec![
                Seg::MoveTo(Point::new(commit.at.x - d, commit.at.y - d)),
                Seg::LineTo(Point::new(commit.at.x + d, commit.at.y + d)),
                Seg::MoveTo(Point::new(commit.at.x + d, commit.at.y - d)),
                Seg::LineTo(Point::new(commit.at.x - d, commit.at.y + d)),
            ];
            vec![
                disc(None),
                Node::new(Role::Decoration, Content::Shape(Shape::Path(cross)))
                    .classed("git-reverse-cross"),
            ]
        }
        CommitType::Normal if commit.is_merge => vec![
            disc(None),
            // A hollow middle, so a merge reads as a ring at a glance.
            Node::new(
                Role::Decoration,
                Content::Shape(Shape::Circle {
                    c: commit.at,
                    r: NODE_R * 0.45,
                }),
            )
            .classed("git-merge-inner"),
        ],
        CommitType::Normal => vec![disc(Some(&format!("git-stroke-{}", commit.color_index)))],
    }
}

fn commit_node(commit: &PlacedCommit) -> Node {
    Node::new(Role::Node, Content::Group(commit_shape(commit)))
        .classed("node")
        .with_id(commit.id.clone())
}

fn branch_label_node(label: &BranchLabel) -> Node {
    text(
        label.at,
        &label.name,
        BRANCH_FONT,
        BRANCH_WEIGHT,
        Anchor::Start,
        "git-branch-label",
    )
    .classed(format!("git-fill-{}", label.color_index))
    // With the edges rather than above the commits: a branch name is furniture
    // in the gutter, and the reference draws it before any commit.
    .on(Layer::Edge)
}

/// The rules a git graph needs on top of the shared tokens.
///
/// A pair per lane — stroke for its edges, fill for its commits — because the
/// palette is derived per index and CSS cannot compute it.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let lanes = placed
        .commits
        .iter()
        .map(|c| c.color_index)
        .chain(placed.branch_labels.iter().map(|b| b.color_index))
        .max()
        .unwrap_or(0);
    let colors: String = (0..=lanes)
        .map(|index| {
            let color = series_css(index, mode, theme);
            format!(".git-stroke-{index}{{stroke:{color}}}.git-fill-{index}{{fill:{color}}}")
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .git-edge{{fill:none;stroke-width:2.5}}\
         .git-commit{{stroke:var(--ags-bg);stroke-width:1.5}}\
         .git-merge-inner{{fill:var(--ags-bg)}}\
         .git-reverse-cross{{stroke:var(--ags-bg);stroke-width:1.5;fill:none}}\
         .git-commit-label{{fill:var(--_text-sec)}}\
         .git-branch-label{{font-weight:600}}\
         .git-tag{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed git graph.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for edge in &placed.edges {
        out.push(edge_node(edge));
    }
    for label in &placed.branch_labels {
        out.push(branch_label_node(label));
    }
    for commit in &placed.commits {
        out.push(commit_node(commit));
    }
    // Ids and tags last, so nothing is drawn over a commit's own name.
    for commit in &placed.commits {
        out.push(text(
            Point::new(commit.at.x, commit.at.y + NODE_R + LABEL_DROP),
            &commit.id,
            LABEL_FONT,
            LABEL_WEIGHT,
            Anchor::Middle,
            "git-commit-label",
        ));
        if let Some(tag) = &commit.tag {
            out.push(
                text(
                    Point::new(commit.at.x, commit.at.y - NODE_R - TAG_RISE),
                    tag,
                    TAG_FONT,
                    TAG_WEIGHT,
                    Anchor::Middle,
                    "git-tag",
                )
                .classed(format!("git-fill-{}", commit.color_index)),
            );
        }
    }
    out
}

/// Parse, lay out and draw in one step.
pub fn render(source: &str, theme: &Theme, mode: &ColorMode) -> Scene {
    scene(&layout(&super::parse(source)), theme, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAPH: &str = "gitGraph\n\
        commit id: \"one\" tag: \"v1\"\n\
        branch feature\n\
        commit id: \"two\" type: HIGHLIGHT\n\
        checkout main\n\
        commit id: \"three\" type: REVERSE\n\
        merge feature id: \"four\"";

    fn drawn(source: &str) -> Scene {
        render(source, &Theme::default(), &ColorMode::Tokens)
    }

    fn flatten(nodes: &[&Node], out: &mut Vec<Node>) {
        for node in nodes {
            out.push((*node).clone());
            if let Content::Group(children) = &node.content {
                flatten(&children.iter().collect::<Vec<_>>(), out);
            }
        }
    }

    fn all(scene: &Scene) -> Vec<Node> {
        let mut out = Vec::new();
        flatten(&scene.painted(), &mut out);
        out
    }

    fn with_class<'a>(nodes: &'a [Node], class: &str) -> Vec<&'a Node> {
        nodes
            .iter()
            .filter(|n| n.class.iter().any(|c| c == class))
            .collect()
    }

    #[test]
    fn every_commit_is_addressable_and_every_edge_names_its_ends() {
        let nodes = all(&drawn(GRAPH));
        let commits = with_class(&nodes, "node");
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].id.as_deref(), Some("one"));
        let edges = with_class(&nodes, "git-edge");
        assert!(edges[0].data.contains(&("from".into(), "one".into())));
    }

    #[test]
    fn each_commit_kind_gets_its_own_shape() {
        let nodes = all(&drawn(GRAPH));
        let shapes: Vec<&str> = with_class(&nodes, "node")
            .iter()
            .filter_map(|n| match &n.content {
                Content::Group(parts) => parts.first().map(|p| match &p.content {
                    Content::Shape(Shape::Rect { .. }) => "rect",
                    Content::Shape(Shape::Circle { .. }) => "circle",
                    _ => "other",
                }),
                _ => None,
            })
            .collect();
        // normal, highlight, reverse, merge.
        assert_eq!(shapes, ["circle", "rect", "circle", "circle"]);
        assert_eq!(with_class(&nodes, "git-reverse-cross").len(), 1);
        assert_eq!(with_class(&nodes, "git-merge-inner").len(), 1);
    }

    #[test]
    fn a_tag_is_drawn_only_where_one_was_written() {
        assert_eq!(with_class(&all(&drawn(GRAPH)), "git-tag").len(), 1);
        // And every commit gets its id written under it.
        assert_eq!(with_class(&all(&drawn(GRAPH)), "git-commit-label").len(), 4);
    }

    #[test]
    fn edges_and_branch_names_paint_behind_the_commits() {
        let scene = drawn(GRAPH);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        let first_commit = order.iter().position(|c| *c == "node").expect("a commit");
        assert!(order
            .iter()
            .take(first_commit)
            .all(|c| *c == "git-edge" || *c == "git-branch-label"));
    }

    #[test]
    fn one_stroke_and_fill_rule_is_emitted_per_lane() {
        let style = drawn(GRAPH).style;
        assert!(
            style.contains(".git-stroke-0{stroke:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains(".git-fill-1{fill:hsl(from"), "{style}");
        assert!(!style.contains(".git-fill-2{"), "{style}");
    }

    #[test]
    fn a_graph_of_nothing_still_yields_a_canvas() {
        let scene = drawn("gitGraph");
        assert!(scene.canvas.width > 0.0);
        // main's name is drawn even with no commits on it.
        assert_eq!(with_class(&all(&scene), "git-branch-label").len(), 1);
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(GRAPH, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
