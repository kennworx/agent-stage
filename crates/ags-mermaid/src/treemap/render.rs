//! A placed treemap, drawn into the scene.
//!
//! Identity contract: every cell, branch and leaf alike, is a group carrying
//! `data-id` and the value it stands for.
//!
//! Labels are drawn only where they fit. A treemap's small cells are genuinely
//! too small for text, and drawing it anyway would spill over the neighbours
//! and make the picture unreadable rather than merely incomplete.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{ink_css, series_css, style_block, Theme};

use super::layout::{layout, Cell, Placed, TITLE_FONT};

const BASELINE: &str = "0.35em";
const TITLE_WEIGHT: u32 = 600;
const BRANCH_FONT: f64 = 12.0;
const BRANCH_WEIGHT: u32 = 600;
const LEAF_FONT: f64 = 12.0;
const LEAF_WEIGHT: u32 = 500;
const VALUE_FONT: f64 = 11.0;
const VALUE_WEIGHT: u32 = 400;
/// A branch's name is inset from its own corner by this much.
const BRANCH_PAD: f64 = 6.0;

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

/// A value as it is written on a cell: whole numbers without a point.
fn shown(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{value}")
    } else {
        format!("{}", (value * 100.0).round() / 100.0)
    }
}

/// A branch's name, in the strip along its top.
fn branch_label(cell: &Cell) -> Option<Node> {
    // A second guard used to follow this one, for a cell "too narrow even for one
    // character" — width less than the padding plus a glyph. It could never fire:
    // that is `width < 24`, which is what this line already rejects. Writing the
    // test for it is what showed it was unreachable.
    if cell.rect.height < 16.0 || cell.rect.width < 24.0 || cell.label.is_empty() {
        return None;
    }
    Some(text(
        Point::new(cell.rect.at.x + BRANCH_PAD, cell.rect.at.y + 10.0),
        &cell.label,
        BRANCH_FONT,
        BRANCH_WEIGHT,
        Anchor::Start,
        "tm-branch-label",
    ))
}

/// A leaf's name and, where there is room, its value under it.
fn leaf_labels(cell: &Cell) -> Vec<Node> {
    if cell.rect.width < 28.0 || cell.rect.height < 18.0 || cell.label.is_empty() {
        return Vec::new();
    }
    // The name is not truncated or wrapped: either it fits or the cell says
    // nothing, and a half-word is worse than none.
    if crate::metrics::text_width(&cell.label, LEAF_FONT, LEAF_WEIGHT) > cell.rect.width - 8.0 {
        return Vec::new();
    }
    let centre = Point::new(
        cell.rect.at.x + cell.rect.width / 2.0,
        cell.rect.at.y + cell.rect.height / 2.0,
    );
    let with_value = cell.rect.height >= 34.0;
    // Written *on* the cell, so the ink is chosen against that cell's own fill —
    // see `leaf_ink_rules`.
    let on = cell.color_index.map(|index| format!("tm-on-{index}"));
    let inked = |node: Node| match &on {
        Some(class) => node.classed(class.clone()),
        None => node,
    };
    let mut out = vec![inked(text(
        Point::new(centre.x, if with_value { centre.y - 7.0 } else { centre.y }),
        &cell.label,
        LEAF_FONT,
        LEAF_WEIGHT,
        Anchor::Middle,
        "tm-leaf-label",
    ))];
    if with_value {
        out.push(inked(text(
            Point::new(centre.x, centre.y + 9.0),
            &shown(cell.value),
            VALUE_FONT,
            VALUE_WEIGHT,
            Anchor::Middle,
            "tm-leaf-value",
        )));
    }
    out
}

fn cell_node(cell: &Cell) -> Node {
    let mut box_node = Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: cell.rect.at,
            size: Size {
                width: cell.rect.width,
                height: cell.rect.height,
            },
            rx: 2.0,
            ry: 2.0,
        }),
    )
    .classed("tm-cell");
    box_node = if cell.is_leaf {
        box_node.classed("tm-leaf").classed(format!(
            "tm-color-{}",
            cell.color_index
                .map_or(-1, |i| i32::try_from(i).unwrap_or(-1))
        ))
    } else {
        box_node.classed("tm-branch")
    };
    let mut parts = vec![box_node];
    if cell.is_leaf {
        parts.extend(leaf_labels(cell));
    } else {
        parts.extend(branch_label(cell));
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(cell.path.clone())
        .valued(shown(cell.value))
}

/// The rules a treemap needs on top of the shared tokens.
///
/// One fill rule per top-level branch, so a subtree reads as one region.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let mut indices: Vec<usize> = placed
        .cells
        .iter()
        .filter(|c| c.is_leaf)
        .filter_map(|c| c.color_index)
        .collect();
    indices.sort_unstable();
    indices.dedup();
    let colors: String = indices
        .iter()
        .map(|index| {
            format!(
                ".tm-color-{index}{{fill:{}}}",
                series_css(*index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .tm-cell{{stroke:var(--ags-bg);stroke-width:2}}\
         .tm-branch{{fill:var(--_group-hdr);stroke:var(--_line);stroke-width:1}}\
         .tm-branch-label{{fill:var(--_text-sec)}}\
         .tm-leaf-label{{fill:var(--ags-bg)}}\
         .tm-leaf-value{{fill:var(--ags-bg);opacity:0.85}}\
         .tm-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}{}",
        style_block(theme, mode),
        leaf_ink_rules(&indices, theme, mode)
    )
}

/// The ink for each leaf's name and value, chosen against that leaf's fill.
///
/// The cells are coloured from the series ramp, which spreads them across a band
/// of lightness on purpose — so one ink cannot read on all of them, and a pale
/// cell had the page background written on it. The compound selector outranks the
/// `.tm-leaf-label` default above, which still covers a leaf with no colour.
fn leaf_ink_rules(indices: &[usize], theme: &Theme, mode: &ColorMode) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    for index in indices {
        let ink = ink_css(&series_css(*index, mode, theme), mode);
        _ = write!(
            out,
            ".tm-leaf-label.tm-on-{index}{{fill:{ink}}}\
             .tm-leaf-value.tm-on-{index}{{fill:{ink}}}"
        );
    }
    out
}

/// Draw a placed treemap.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for cell in &placed.cells {
        out.push(cell_node(cell));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            TITLE_WEIGHT,
            Anchor::Middle,
            "tm-title",
        ));
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
    use crate::treemap::layout::Rect;

    fn cell(label: &str, width: f64, height: f64) -> Cell {
        Cell {
            path: label.to_string(),
            label: label.to_string(),
            value: 1.0,
            rect: Rect {
                at: Point::new(0.0, 0.0),
                width,
                height,
            },
            depth: 0,
            is_leaf: false,
            color_index: None,
        }
    }

    #[test]
    fn a_branch_too_small_for_its_name_is_left_unlabelled() {
        // Drawing it anyway would spill over the neighbours, which makes the
        // picture unreadable rather than merely incomplete.
        assert!(
            branch_label(&cell("Projects", 200.0, 12.0)).is_none(),
            "too short"
        );
        assert!(
            branch_label(&cell("Projects", 20.0, 40.0)).is_none(),
            "too narrow"
        );
        assert!(
            branch_label(&cell("", 200.0, 40.0)).is_none(),
            "nothing to write"
        );
    }

    #[test]
    fn a_branch_with_room_gets_its_name_in_the_strip_along_the_top() {
        let node = branch_label(&cell("Projects", 200.0, 40.0)).expect("a label");
        let Content::Text(run) = &node.content else {
            panic!("a text run")
        };
        assert_eq!(run.content, "Projects");
        assert_eq!(run.anchor, Anchor::Start);
        assert!(run.at.x > 0.0, "inset from its own corner");
    }

    #[test]
    fn a_name_is_not_truncated_to_fit_a_narrow_branch() {
        // A branch's name is written whole or not at all, and the only thing that
        // stops it is the cell being too small to hold any name. Overflow is the
        // deliberate trade: a half-word reads as a different word.
        assert!(
            branch_label(&cell("x", 24.0, 40.0)).is_some(),
            "the narrowest cell that labels"
        );
        assert!(
            branch_label(&cell("a very long branch name", 24.0, 40.0)).is_some(),
            "and it is not rejected for being long"
        );
        assert!(
            branch_label(&cell("x", 23.9, 40.0)).is_none(),
            "just under is not"
        );
    }

    const TREE: &str = "treemap-beta\n\
        title Disk\n\
        \"Projects\"\n    \
            \"rust\" : 40\n    \
            \"web\"\n        \
                \"src\" : 20\n        \
                \"dist\" : 5";

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
    fn every_cell_is_addressable_and_carries_its_value() {
        let nodes = all(&drawn(TREE));
        let cells = with_class(&nodes, "node");
        assert_eq!(cells.len(), 5, "root, rust, web, src, dist");
        assert_eq!(cells[0].id.as_deref(), Some("Projects"));
        assert_eq!(cells[0].value.as_deref(), Some("65"));
    }

    #[test]
    fn a_branch_and_a_leaf_are_drawn_differently() {
        let nodes = all(&drawn(TREE));
        assert_eq!(with_class(&nodes, "tm-branch").len(), 2, "root and web");
        assert_eq!(with_class(&nodes, "tm-leaf").len(), 3);
    }

    #[test]
    fn a_leaf_shows_its_name_and_a_branch_shows_its_own() {
        let nodes = all(&drawn(TREE));
        assert!(!with_class(&nodes, "tm-leaf-label").is_empty());
        assert!(!with_class(&nodes, "tm-branch-label").is_empty());
    }

    #[test]
    fn a_cell_too_small_for_its_name_says_nothing_rather_than_spilling() {
        // One huge value beside many tiny ones leaves the tiny cells unlabelled.
        let tiny = "  \"tiny\" : 1\n".repeat(40);
        let source = format!("treemap\n\"top\"\n  \"huge\" : 10000\n{tiny}");
        let nodes = all(&drawn(&source));
        let labels = with_class(&nodes, "tm-leaf-label").len();
        let cells = with_class(&nodes, "tm-leaf").len();
        assert!(labels < cells, "{labels} labels for {cells} cells");
    }

    #[test]
    fn a_value_is_written_only_where_there_is_room_under_the_name() {
        let nodes = all(&drawn(TREE));
        assert!(
            with_class(&nodes, "tm-leaf-value").len() <= with_class(&nodes, "tm-leaf-label").len()
        );
    }

    #[test]
    fn a_whole_value_is_written_without_a_point() {
        assert_eq!(shown(65.0), "65");
        assert_eq!(shown(1.5), "1.5");
        assert_eq!(shown(1.23456), "1.23");
    }

    #[test]
    fn one_fill_rule_is_emitted_per_top_level_branch() {
        let style = drawn(TREE).style;
        assert!(
            style.contains(".tm-color-0{fill:var(--ags-accent"),
            "{style}"
        );
        assert!(style.contains(".tm-color-1{fill:hsl(from"), "{style}");
        assert!(!style.contains(".tm-color-2{"), "{style}");
    }

    #[test]
    fn a_leaf_is_inked_against_the_cell_it_is_written_on() {
        // The cells span a band of lightness on purpose, so one ink cannot read
        // on all of them — a pale cell had the page background written on it.
        let scene = drawn(TREE);
        let nodes = all(&scene);
        assert!(
            !with_class(&nodes, "tm-on-0").is_empty(),
            "a labelled leaf names its own ink"
        );
        for index in [0, 1] {
            assert!(
                scene
                    .style
                    .contains(&format!(".tm-leaf-label.tm-on-{index}{{fill:")),
                "{}",
                scene.style
            );
            assert!(
                scene
                    .style
                    .contains(&format!(".tm-leaf-value.tm-on-{index}{{fill:")),
                "{}",
                scene.style
            );
        }
        // The plain rule stays as the default for a leaf with no colour.
        assert!(
            scene.style.contains(".tm-leaf-label{fill:"),
            "{}",
            scene.style
        );
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(TREE)), "tm-title").len(), 1);
        assert!(with_class(&all(&drawn("treemap\n\"a\" : 1")), "tm-title").is_empty());
    }

    #[test]
    fn a_treemap_of_nothing_still_yields_a_canvas() {
        let scene = drawn("treemap");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(TREE, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
