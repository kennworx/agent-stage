//! A placed board, drawn into the scene.
//!
//! Identity contract: each card is a group carrying `data-id` — a card is the
//! thing a reviewer comments on. Columns are addressable too, but under their
//! own class: a column is a container, not a unit of work.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{style_block, Theme};

use super::layout::{
    layout, Placed, PlacedCard, PlacedColumn, CARD_FONT, CARD_LINE_HEIGHT, CARD_PAD_X, CARD_PAD_Y,
    CARD_WEIGHT, HEADER_FONT, HEADER_WEIGHT, META_FONT, META_GAP, META_LINE_HEIGHT, META_WEIGHT,
    TITLE_FONT,
};

const BASELINE: &str = "0.35em";
const TITLE_WEIGHT: u32 = 600;
const CARD_RADIUS: f64 = 6.0;
const COLUMN_RADIUS: f64 = 8.0;

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

fn rect(at: Point, width: f64, height: f64, radius: f64, role: Role, class: &str) -> Node {
    Node::new(
        role,
        Content::Shape(Shape::Rect {
            at,
            size: Size { width, height },
            rx: radius,
            ry: radius,
        }),
    )
    .classed(class)
}

fn card_node(card: &PlacedCard) -> Node {
    let mut parts = vec![rect(
        card.at,
        card.width,
        card.height,
        CARD_RADIUS,
        Role::Node,
        "kanban-card-box",
    )];
    let text_x = card.at.x + CARD_PAD_X;
    let mut baseline = card.at.y + CARD_PAD_Y + CARD_LINE_HEIGHT / 2.0;
    for line in &card.lines {
        parts.push(text(
            Point::new(text_x, baseline),
            line,
            CARD_FONT,
            CARD_WEIGHT,
            Anchor::Start,
            "kanban-card-text",
        ));
        baseline += CARD_LINE_HEIGHT;
    }
    if let Some(meta) = &card.meta_line {
        // Measured back from where the next text line would have started, so
        // the gap below the last line is the same however many lines there are.
        let y = baseline - CARD_LINE_HEIGHT + META_GAP + META_LINE_HEIGHT / 2.0;
        parts.push(text(
            Point::new(text_x, y),
            meta,
            META_FONT,
            META_WEIGHT,
            Anchor::Start,
            "kanban-meta-text",
        ));
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(card.id.clone())
}

fn column_node(column: &PlacedColumn) -> Node {
    let mut parts = vec![
        rect(
            column.at,
            column.width,
            column.height,
            COLUMN_RADIUS,
            Role::Frame,
            "kanban-col-box",
        ),
        rect(
            column.at,
            column.width,
            column.header_height,
            COLUMN_RADIUS,
            Role::Frame,
            "kanban-col-header",
        ),
        text(
            Point::new(
                column.at.x + column.width / 2.0,
                column.at.y + column.header_height / 2.0,
            ),
            &column.title,
            HEADER_FONT,
            HEADER_WEIGHT,
            Anchor::Middle,
            "kanban-header-text",
        ),
    ];
    parts.extend(column.cards.iter().map(card_node));
    Node::new(Role::Frame, Content::Group(parts))
        .classed("kanban-column")
        .with_id(column.id.clone())
}

/// Draw a placed board.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = format!(
        "{}\
         .kanban-title{{fill:var(--_text)}}\
         .kanban-col-box{{fill:var(--_group-fill,var(--ags-bg));stroke:var(--_inner-stroke);stroke-width:1}}\
         .kanban-col-header{{fill:var(--_group-hdr,var(--_inner-stroke));stroke:none}}\
         .kanban-header-text{{fill:var(--_text)}}\
         .kanban-card-box{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .kanban-card-text{{fill:var(--_text)}}\
         .kanban-meta-text{{fill:var(--_text-sec)}}\
         text{{font-family:Inter,system-ui,sans-serif}}",
        style_block(theme, mode)
    );
    if let Some((title, at)) = &placed.title {
        // Ahead of the board rather than over it: nothing reaches up into the
        // title band, so drawing it first keeps the markup in reading order.
        out.push(
            text(
                *at,
                title,
                TITLE_FONT,
                TITLE_WEIGHT,
                Anchor::Middle,
                "kanban-title",
            )
            .on(Layer::Frame),
        );
    }
    for column in &placed.columns {
        out.push(column_node(column));
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

    const BOARD: &str = "kanban\n\
        title Sprint\n\
        todo[To do]\n    \
            t1[One]@{ assigned: me }\n    \
            t2[Two]\n\
        done[Done]";

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
    fn a_card_is_the_addressable_element_and_a_column_is_a_container() {
        let nodes = all(&drawn(BOARD));
        let cards = with_class(&nodes, "node");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].id.as_deref(), Some("t1"));
        let columns = with_class(&nodes, "kanban-column");
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].id.as_deref(), Some("todo"));
        // A column is not a `node`, so a note cannot be keyed to one by
        // accident when the reviewer meant a card.
        assert!(columns.iter().all(|c| !c.class.iter().any(|k| k == "node")));
    }

    #[test]
    fn a_card_lives_inside_its_column_group() {
        let scene = drawn(BOARD);
        let Content::Group(parts) = &scene.painted()[1].content else {
            panic!("a column is a group")
        };
        // Box, header, header text, then the cards.
        assert_eq!(parts.len(), 5);
        assert!(parts[3].class.iter().any(|c| c == "node"));
    }

    #[test]
    fn a_metadata_line_is_drawn_only_where_one_was_written() {
        assert_eq!(with_class(&all(&drawn(BOARD)), "kanban-meta-text").len(), 1);
    }

    #[test]
    fn wrapped_text_becomes_one_line_element_each() {
        let nodes = all(&drawn(
            "kanban\na[A]\n  c[A card whose text is far too long to fit on one line]",
        ));
        assert!(with_class(&nodes, "kanban-card-text").len() > 1);
    }

    #[test]
    fn every_column_draws_a_container_a_header_and_a_name() {
        let nodes = all(&drawn(BOARD));
        assert_eq!(with_class(&nodes, "kanban-col-box").len(), 2);
        assert_eq!(with_class(&nodes, "kanban-col-header").len(), 2);
        assert_eq!(with_class(&nodes, "kanban-header-text").len(), 2);
    }

    #[test]
    fn a_title_is_drawn_first_and_an_absent_one_is_not() {
        let scene = drawn(BOARD);
        assert_eq!(
            scene.painted().first().and_then(|n| n.class.first()),
            Some(&"kanban-title".to_string())
        );
        assert!(with_class(&all(&drawn("kanban\na[A]")), "kanban-title").is_empty());
    }

    #[test]
    fn a_board_of_nothing_still_yields_a_canvas() {
        let scene = drawn("kanban");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(BOARD, &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
