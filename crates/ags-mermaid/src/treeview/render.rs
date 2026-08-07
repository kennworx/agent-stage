//! A placed tree view, drawn into the scene.
//!
//! Identity contract: each row is a group carrying its path as `data-id` and
//! whether it is a folder. The path rather than the name, so two files called
//! `index.ts` in different folders are separately addressable.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Node, Point, Role, Scene, Seg, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, Connector, Placed, Row, DESC_FONT, DESC_WEIGHT, FILE_WEIGHT, FOLDER_WEIGHT, GLYPH_SIZE,
    LABEL_FONT, TITLE_FONT,
};

const BASELINE: &str = "0.35em";
const TITLE_WEIGHT: u32 = 600;

fn text(at: Point, content: &str, size: f64, weight: u32, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor: Anchor::Start,
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

fn line(points: Vec<Point>) -> Vec<Seg> {
    let mut segs = Vec::with_capacity(points.len());
    for (i, point) in points.into_iter().enumerate() {
        segs.push(if i == 0 {
            Seg::MoveTo(point)
        } else {
            Seg::LineTo(point)
        });
    }
    segs
}

/// A folder: a body with a raised tab on its top-left.
fn folder_glyph(at: Point) -> Node {
    let (x, y) = (at.x, at.y);
    let mut segs = line(vec![
        Point::new(x + 0.5, y + 11.5),
        Point::new(x + 0.5, y + 3.0),
        Point::new(x + 5.5, y + 3.0),
        Point::new(x + 7.0, y + 4.5),
        Point::new(x + GLYPH_SIZE - 0.5, y + 4.5),
        Point::new(x + GLYPH_SIZE - 0.5, y + 11.5),
    ]);
    segs.push(Seg::Close);
    Node::new(Role::Icon, Content::Shape(Shape::Path(segs))).classed("tv-folder")
}

/// A file: a page with its top-right corner turned down. Two paths, because the
/// fold is a line across the body rather than part of its outline.
fn file_glyph(at: Point) -> Vec<Node> {
    let (x, y) = (at.x, at.y);
    let mut body = line(vec![
        Point::new(x + 3.0, y + 1.5),
        Point::new(x + 9.5, y + 1.5),
        Point::new(x + 13.0, y + 5.0),
        Point::new(x + 13.0, y + 14.5),
        Point::new(x + 3.0, y + 14.5),
    ]);
    body.push(Seg::Close);
    let fold = line(vec![
        Point::new(x + 9.5, y + 1.5),
        Point::new(x + 9.5, y + 5.0),
        Point::new(x + 13.0, y + 5.0),
    ]);
    vec![
        Node::new(Role::Icon, Content::Shape(Shape::Path(body))).classed("tv-file"),
        Node::new(Role::Icon, Content::Shape(Shape::Path(fold))).classed("tv-file-fold"),
    ]
}

fn row_node(row: &Row) -> Node {
    let mut parts = if row.is_folder {
        vec![folder_glyph(row.glyph_at)]
    } else {
        file_glyph(row.glyph_at)
    };
    let weight = if row.is_folder {
        FOLDER_WEIGHT
    } else {
        FILE_WEIGHT
    };
    let label = text(
        Point::new(row.text_x, row.centre_y),
        &row.label,
        LABEL_FONT,
        weight,
        "tv-label",
    );
    // A folder's name takes a second class as well as the shared one, so the
    // two can be restyled apart without duplicating the common rule.
    parts.push(if row.is_folder {
        label.classed("tv-folder-label")
    } else {
        label
    });
    if let Some(description) = &row.description {
        parts.push(text(
            Point::new(row.desc_x, row.centre_y),
            description,
            DESC_FONT,
            DESC_WEIGHT,
            "tv-desc",
        ));
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(row.path.clone())
        .tagged("folder", row.is_folder.to_string())
}

fn connector_node(connector: &Connector) -> Node {
    Node::new(
        Role::Edge,
        Content::Shape(Shape::Path(line(connector.corner.to_vec()))),
    )
    .classed("tv-connector")
    .tagged("from", connector.from.clone())
    .tagged("to", connector.to.clone())
}

/// Draw a placed tree view.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .tv-connector{{fill:none;stroke:var(--_line);stroke-width:1}}\
         .tv-folder{{fill:var(--ags-accent,var(--_text-sec));stroke:none}}\
         .tv-file{{fill:var(--ags-bg);stroke:var(--_text-sec);stroke-width:1.2;stroke-linejoin:round}}\
         .tv-file-fold{{fill:none;stroke:var(--_text-sec);stroke-width:1.2;stroke-linejoin:round}}\
         .tv-label{{fill:var(--_text)}}\
         .tv-folder-label{{fill:var(--_text)}}\
         .tv-desc{{fill:var(--_text-sec);font-style:italic}}\
         .tv-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );
    for connector in &placed.connectors {
        out.push(connector_node(connector));
    }
    for row in &placed.rows {
        out.push(row_node(row));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(*at, title, TITLE_FONT, TITLE_WEIGHT, "tv-title"));
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

    const TREE: &str = "treeView-beta\n\
        title Project\n\
        my-project/\n    \
            src/ ## the code\n        \
                index.js\n    \
            README.md";

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
    fn every_row_is_addressable_by_path_and_says_what_it_is() {
        let nodes = all(&drawn(TREE));
        let rows = with_class(&nodes, "node");
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2].id.as_deref(), Some("my-project/src/index.js"));
        assert!(rows[0].data.contains(&("folder".into(), "true".into())));
        assert!(rows[2].data.contains(&("folder".into(), "false".into())));
    }

    #[test]
    fn a_folder_and_a_file_get_different_glyphs() {
        let nodes = all(&drawn(TREE));
        assert_eq!(with_class(&nodes, "tv-folder").len(), 2);
        // A file is two paths: its outline and the fold across it.
        assert_eq!(with_class(&nodes, "tv-file").len(), 2);
        assert_eq!(with_class(&nodes, "tv-file-fold").len(), 2);
    }

    #[test]
    fn a_folder_name_carries_the_extra_class_a_file_name_does_not() {
        let nodes = all(&drawn(TREE));
        assert_eq!(with_class(&nodes, "tv-label").len(), 4);
        assert_eq!(with_class(&nodes, "tv-folder-label").len(), 2);
    }

    #[test]
    fn a_note_is_drawn_only_where_one_was_written() {
        assert_eq!(with_class(&all(&drawn(TREE)), "tv-desc").len(), 1);
    }

    #[test]
    fn connectors_paint_behind_the_rows_they_join() {
        let scene = drawn(TREE);
        let order: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.class.first().map(String::as_str))
            .collect();
        let first_row = order.iter().position(|c| *c == "node").expect("a row");
        assert!(order.iter().take(first_row).all(|c| *c == "tv-connector"));
    }

    #[test]
    fn a_connector_names_both_ends() {
        let nodes = all(&drawn(TREE));
        let elbows = with_class(&nodes, "tv-connector");
        assert_eq!(elbows.len(), 3);
        assert!(elbows[0]
            .data
            .contains(&("from".into(), "my-project".into())));
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(with_class(&all(&drawn(TREE)), "tv-title").len(), 1);
        assert!(with_class(&all(&drawn("treeView\na")), "tv-title").is_empty());
    }

    #[test]
    fn a_tree_of_nothing_still_yields_a_canvas() {
        let scene = drawn("treeView");
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
