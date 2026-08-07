//! A placed timeline, drawn into the scene.
//!
//! Identity contract: each event card is a group carrying `data-id`, derived
//! from the event's own text.

use crate::api::ColorMode;
use crate::scene::{Anchor, Content, Font, Layer, Node, Point, Role, Scene, Shape, Size, TextRun};
use crate::theme::{ink_css, series_css, style_block, Theme};

use super::layout::{layout, Band, Placed, PlacedEvent, PlacedPeriod, MARKER_RADIUS, TITLE_FONT};

const BASELINE: &str = "0.35em";

fn text(at: Point, content: &str, size: f64, weight: u32, class: &str) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor: Anchor::Middle,
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

fn band_nodes(band: &Band) -> Vec<Node> {
    vec![
        Node::new(
            Role::Frame,
            Content::Shape(Shape::Rect {
                at: band.at,
                size: Size {
                    width: band.width,
                    height: band.height,
                },
                rx: 6.0,
                ry: 6.0,
            }),
        )
        .classed("tl-section-band")
        .classed(format!("tl-fill-{}", band.color_index)),
        // On the frame layer with its band: the name is written *on* the band,
        // and the label layer would lift it above the event cards instead. Its
        // ink is chosen against that band's own fill — see `style` below.
        text(band.label_at, &band.name, 14.0, 600, "tl-section-label")
            .classed(format!("tl-on-{}", band.color_index))
            .on(Layer::Frame),
    ]
}

fn period_nodes(period: &PlacedPeriod) -> Vec<Node> {
    vec![
        Node::new(
            Role::Node,
            Content::Shape(Shape::Circle {
                c: period.marker_at,
                r: MARKER_RADIUS,
            }),
        )
        .classed("tl-marker")
        .classed(format!("tl-fill-{}", period.color_index)),
        // With its marker, not on the label layer: a period's date and its dot
        // are one mark, and a layer between them would let an event card drawn
        // later slide in between.
        text(period.label_at, &period.label, 15.0, 600, "tl-period-label").on(Layer::Node),
    ]
}

fn event_node(event: &PlacedEvent) -> Node {
    Node::new(
        Role::Node,
        Content::Group(vec![
            Node::new(
                Role::Node,
                Content::Shape(Shape::Rect {
                    at: event.at,
                    size: Size {
                        width: event.width,
                        height: event.height,
                    },
                    rx: 6.0,
                    ry: 6.0,
                }),
            )
            .classed("tl-event")
            .classed(format!("tl-stroke-{}", event.color_index)),
            text(event.label_at, &event.text, 13.0, 500, "tl-event-label"),
        ]),
    )
    .classed("node")
    .with_id(event.id.clone())
}

/// One fill and one stroke rule per colour index in use.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let mut indices: Vec<usize> = placed
        .bands
        .iter()
        .map(|b| b.color_index)
        .chain(placed.periods.iter().map(|p| p.color_index))
        .chain(placed.events.iter().map(|e| e.color_index))
        .collect();
    indices.sort_unstable();
    indices.dedup();
    let colors: String = indices
        .iter()
        .map(|i| {
            let c = series_css(*i, mode, theme);
            let ink = ink_css(&c, mode);
            // The section name is written on the band, and the bands are spread
            // across a band of lightness, so the ink is chosen per section rather
            // than once for all of them. Compound, to outrank the default below.
            format!(
                ".tl-fill-{i}{{fill:{c}}}.tl-stroke-{i}{{stroke:{c}}}\
                 .tl-section-label.tl-on-{i}{{fill:{ink}}}"
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .tl-section-band{{stroke:none}}\
         .tl-section-label{{fill:var(--ags-bg)}}\
         .tl-axis{{stroke:var(--_line);stroke-width:2}}\
         .tl-marker{{stroke:var(--ags-bg);stroke-width:2}}\
         .tl-period-label{{fill:var(--_text)}}\
         .tl-event{{fill:var(--ags-bg);stroke-width:1.5}}\
         .tl-event-label{{fill:var(--_text)}}\
         .tl-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}",
        style_block(theme, mode)
    )
}

/// Draw a placed timeline.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for band in &placed.bands {
        for node in band_nodes(band) {
            out.push(node);
        }
    }
    if let Some((from, to)) = placed.axis {
        out.push(
            Node::new(Role::Edge, Content::Shape(Shape::Line { a: from, b: to }))
                .classed("tl-axis"),
        );
    }
    for period in &placed.periods {
        for node in period_nodes(period) {
            out.push(node);
        }
    }
    for event in &placed.events {
        out.push(event_node(event));
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(*at, title, TITLE_FONT, 600, "tl-title"));
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
    fn every_event_is_addressable() {
        let nodes = all(&drawn("timeline\n2024 : shipped : reviewed"));
        let events = with_class(&nodes, "node");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id.as_deref(), Some("shipped"));
        assert_eq!(events[1].id.as_deref(), Some("reviewed"));
    }

    #[test]
    fn the_spine_is_drawn_when_there_is_something_to_connect() {
        assert_eq!(
            with_class(&all(&drawn("timeline\n1 : a")), "tl-axis").len(),
            1
        );
        assert!(with_class(&all(&drawn("timeline")), "tl-axis").is_empty());
    }

    #[test]
    fn a_band_and_its_name_paint_together_behind_the_cards() {
        let scene = drawn("timeline\nsection Alpha\n1 : a");
        let nodes = all(&scene);
        let label = with_class(&nodes, "tl-section-label")[0];
        // On the band's own layer: the label layer would lift the name above
        // the event cards, which sit lower down the page.
        assert_eq!(label.layer, Layer::Frame);
        let order: Vec<Layer> = scene.painted().iter().map(|n| n.layer).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]), "{order:?}");
    }

    #[test]
    fn each_section_takes_its_own_colour() {
        let scene = drawn("timeline\nsection A\n1 : x\nsection B\n2 : y");
        assert!(
            scene.style.contains(".tl-fill-0{fill:var(--ags-accent"),
            "{}",
            scene.style
        );
        assert!(
            scene.style.contains(".tl-stroke-1{stroke:hsl(from"),
            "{}",
            scene.style
        );
    }

    #[test]
    fn a_section_name_is_inked_against_its_own_band() {
        // The bands are spread across a band of lightness, so a single ink leaves
        // the name unreadable on the pale ones.
        let scene = drawn("timeline\nsection A\n1 : x\nsection B\n2 : y");
        let nodes = all(&scene);
        assert!(
            !with_class(&nodes, "tl-on-0").is_empty(),
            "the name names its ink"
        );
        for index in [0, 1] {
            assert!(
                scene
                    .style
                    .contains(&format!(".tl-section-label.tl-on-{index}{{fill:")),
                "{}",
                scene.style
            );
        }
    }

    #[test]
    fn a_period_and_its_date_stay_one_mark() {
        let nodes = all(&drawn("timeline\n1 : a"));
        let label = with_class(&nodes, "tl-period-label")[0];
        let marker = with_class(&nodes, "tl-marker")[0];
        assert_eq!(label.layer, marker.layer);
    }

    #[test]
    fn a_marker_and_a_label_are_drawn_for_every_period() {
        let nodes = all(&drawn("timeline\n1 : a\n2 : b\n3 : c"));
        assert_eq!(with_class(&nodes, "tl-marker").len(), 3);
        assert_eq!(with_class(&nodes, "tl-period-label").len(), 3);
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(
            with_class(&all(&drawn("timeline title T\n1 : a")), "tl-title").len(),
            1
        );
        assert!(with_class(&all(&drawn("timeline\n1 : a")), "tl-title").is_empty());
    }

    #[test]
    fn a_timeline_of_nothing_still_yields_a_canvas() {
        let scene = drawn("timeline");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render("timeline\n1 : a", &Theme::default(), &ColorMode::Fixed);
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
