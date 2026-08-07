//! A placed pie chart, drawn into the scene.
//!
//! Identity contract: each wedge is a group carrying `data-id` (its label) and
//! `data-value` (its number), so a reviewer's note lands on a slice rather than
//! on a coordinate.

use crate::api::ColorMode;
use crate::scene::{
    Anchor, Content, Font, Layer, Node, Point, Role, Scene, Seg, Shape, Size, TextRun,
};
use crate::theme::{ink_css, series_css, style_block, Theme};

use super::layout::{
    layout, LegendRow, Placed, PlacedSlice, INNER_RADIUS, LEGEND_FONT, LEGEND_SWATCH,
    LEGEND_WEIGHT, RADIUS, TITLE_FONT,
};
/// Baseline offset that centres a line of text on its anchor point.
const BASELINE: &str = "0.35em";

/// Below this share, a wedge is too thin to hold its own number.
const LABEL_MIN_PERCENT: f64 = 5.0;

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

/// A circle of radius `radius`, as two half-arcs.
///
/// Two, because an arc that starts and ends at the same point sweeps nothing at
/// all and would leave the chart blank. `sweep` is which way round it is drawn,
/// which is what decides whether it fills or cuts.
fn round_trip(centre: Point, radius: f64, sweep: bool) -> Vec<Seg> {
    let r = Size {
        width: radius,
        height: radius,
    };
    let (left, right) = (
        Point::new(centre.x - radius, centre.y),
        Point::new(centre.x + radius, centre.y),
    );
    vec![
        Seg::MoveTo(left),
        Seg::Arc {
            r,
            large: true,
            sweep,
            to: right,
        },
        Seg::Arc {
            r,
            large: true,
            sweep,
            to: left,
        },
        Seg::Close,
    ]
}

/// The outline of one wedge: a sector of the ring, rather than of the disc.
///
/// Out along the rim and back along the hole, which is the whole of the shape —
/// a ring sector never touches the centre.
///
/// A slice covering the whole circle is the exception: it has no radial edges to
/// join, so it is drawn as two closed circles wound in opposite directions. The
/// non-zero fill rule then takes the inner one out of the outer, which is what
/// leaves a hole rather than a disc.
fn wedge(slice: &PlacedSlice, centre: Point) -> Vec<Seg> {
    let ring = |radius: f64| Size {
        width: radius,
        height: radius,
    };
    let on = |radius: f64, angle: f64| {
        Point::new(
            centre.x + radius * angle.cos(),
            centre.y + radius * angle.sin(),
        )
    };
    if slice.whole {
        let mut out = round_trip(centre, RADIUS, false);
        out.extend(round_trip(centre, INNER_RADIUS, true));
        return out;
    }
    let large = slice.to - slice.from > std::f64::consts::PI;
    vec![
        Seg::MoveTo(on(RADIUS, slice.from)),
        Seg::Arc {
            r: ring(RADIUS),
            large,
            sweep: true,
            to: on(RADIUS, slice.to),
        },
        Seg::LineTo(on(INNER_RADIUS, slice.to)),
        Seg::Arc {
            r: ring(INNER_RADIUS),
            large,
            sweep: false,
            to: on(INNER_RADIUS, slice.from),
        },
        Seg::Close,
    ]
}

/// A number as the wedge shows it.
fn percent_label(percent: f64) -> String {
    format!("{}%", crate::round::round_half_up(percent))
}

/// One wedge, with its share written inside it when there is room.
fn slice_node(slice: &PlacedSlice, centre: Point) -> Node {
    let mut parts = vec![Node::new(
        Role::Node,
        Content::Shape(Shape::Path(wedge(slice, centre))),
    )
    .classed("pie-slice")
    .classed(format!("pie-color-{}", slice.color_index))];
    if slice.percent >= LABEL_MIN_PERCENT {
        // The share is written *on* the wedge, so its ink is chosen per wedge —
        // see `pie_label_rules`.
        parts.push(
            text(
                slice.label_at,
                &percent_label(slice.percent),
                13.0,
                600,
                Anchor::Middle,
                "pie-slice-label",
            )
            .classed(format!("pie-on-{}", slice.color_index)),
        );
    }
    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(slice.label.clone())
        .valued(format!("{}", slice.value))
        // Names which legend row this wedge belongs to; see `wedge_rules`.
        .tagged(crate::hover::PAIR, slice.color_index.to_string())
}

/// One legend row: a swatch in the slice's own colour, and its label.
fn legend_row(row: &LegendRow) -> Vec<Node> {
    vec![
        Node::new(
            Role::Decoration,
            Content::Shape(Shape::Rect {
                at: row.swatch_at,
                size: Size {
                    width: LEGEND_SWATCH,
                    height: LEGEND_SWATCH,
                },
                rx: 3.0,
                ry: 3.0,
            }),
        )
        .classed("pie-slice")
        .classed(format!("pie-color-{}", row.color_index))
        // With the label rather than with the wedges: a swatch belongs beside
        // the words it explains, and the two must not be separated by a layer.
        .on(Layer::Label)
        .tagged(crate::hover::PAIR, row.color_index.to_string()),
        text(
            row.text_at,
            &row.label,
            LEGEND_FONT,
            LEGEND_WEIGHT,
            Anchor::Start,
            "pie-legend-label",
        )
        .tagged(crate::hover::PAIR, row.color_index.to_string()),
        // Drawn to its right-hand end, so the column lines up whatever the
        // labels beside it are.
        text(
            row.share_at,
            &row.share,
            LEGEND_FONT,
            LEGEND_WEIGHT,
            Anchor::End,
            "pie-legend-share",
        )
        .tagged(crate::hover::PAIR, row.color_index.to_string()),
    ]
}

/// Pair each wedge with its legend row, so hovering either raises both.
///
/// Its own generator rather than `crate::hover::pairs`, because a wedge's
/// highlight has to mean something different. A wire is recoloured, which costs
/// nothing: its colour carries no information. A wedge's fill *is* the datum —
/// repainting it would say something false, and repainting it to the accent
/// would say the same false thing as whichever slice is already that colour. So
/// the wedge is outlined instead, and only the legend's words take the accent.
/// The share written on the wedge keeps its own ink, which was chosen to read
/// against that fill.
///
/// A slice's `color_index` is its position — wedges and legend rows are built in
/// one pass — so it names the pair without anything having to be threaded
/// through.
fn wedge_rules(placed: &Placed) -> String {
    let lit = "var(--ags-accent,var(--_text))";
    let pair = crate::hover::PAIR;
    let mut out: Vec<String> = Vec::new();
    for slice in &placed.slices {
        let id = slice.color_index;
        let hovered = format!("svg:has([data-{pair}=\"{id}\"]:hover)");
        out.push(format!(
            "{hovered} [data-{pair}=\"{id}\"] .pie-slice,\
             {hovered} .pie-slice[data-{pair}=\"{id}\"]{{stroke:{lit};stroke-width:3}}\
             {hovered} .pie-legend-label[data-{pair}=\"{id}\"],\
             {hovered} .pie-legend-share[data-{pair}=\"{id}\"]{{fill:{lit}}}\
             [data-{pair}=\"{id}\"]{{cursor:default}}"
        ));
    }
    out.concat()
}

/// The rules a pie needs on top of the shared tokens.
///
/// One fill rule per slice, because the palette is derived per index and CSS
/// cannot compute it. The stroke is the page background, which is what separates
/// two adjacent wedges without drawing a line between them.
fn style(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    let colors: String = placed
        .slices
        .iter()
        .map(|s| {
            format!(
                ".pie-color-{}{{fill:{}}}",
                s.color_index,
                series_css(s.color_index, mode, theme)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        "{}\
         .pie-slice{{stroke:var(--ags-bg);stroke-width:2}}\
         .pie-legend-label{{fill:var(--_text)}}\
         .pie-legend-share{{fill:var(--_text-muted)}}\
         .pie-title{{fill:var(--_text)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{colors}{}{}",
        style_block(theme, mode),
        pie_label_rules(placed, theme, mode),
        wedge_rules(placed)
    )
}

/// The ink for each wedge's percentage, chosen against that wedge.
///
/// One rule per wedge rather than one for all of them: the shares are written on
/// the wedges, and the wedges are deliberately spread across a band of lightness,
/// so no single colour reads on all of them. A pale wedge had white on it.
fn pie_label_rules(placed: &Placed, theme: &Theme, mode: &ColorMode) -> String {
    use std::fmt::Write as _;

    let mut seen: Vec<usize> = Vec::new();
    let mut out = String::new();
    for slice in &placed.slices {
        if slice.percent < LABEL_MIN_PERCENT || seen.contains(&slice.color_index) {
            continue;
        }
        seen.push(slice.color_index);
        let wedge = series_css(slice.color_index, mode, theme);
        _ = write!(
            out,
            ".pie-on-{}{{fill:{}}}",
            slice.color_index,
            ink_css(&wedge, mode)
        );
    }
    out
}

/// Draw a placed pie chart.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(Size {
        width: placed.width,
        height: placed.height,
    });
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(placed, theme, mode);
    for slice in &placed.slices {
        out.push(slice_node(slice, placed.centre));
    }
    for row in &placed.legend {
        for node in legend_row(row) {
            out.push(node);
        }
    }
    if let Some((title, at)) = &placed.title {
        out.push(text(
            *at,
            title,
            TITLE_FONT,
            600,
            Anchor::Middle,
            "pie-title",
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
    fn a_wedge_and_its_legend_row_are_named_as_one() {
        let scene = drawn("pie\n\"Rust\" : 40\n\"Go\" : 10");
        // Four parts per slice: the wedge, the swatch, the words and the share.
        let named = all(&scene)
            .iter()
            .filter(|n| n.data.iter().any(|(k, v)| k == "rel" && v == "0"))
            .count();
        assert_eq!(named, 4);
        assert!(
            scene.style.contains("svg:has([data-rel=\"0\"]:hover)"),
            "{}",
            scene.style
        );
    }

    #[test]
    fn a_hovered_wedge_is_outlined_rather_than_repainted() {
        // Its fill is the datum. Repainting it would say something false — and
        // say the same false thing as whichever slice is already that colour.
        let css = drawn("pie\n\"Rust\" : 40\n\"Go\" : 10").style;
        for rule in css.split("svg:has") {
            if !rule.starts_with("([data-rel=") {
                continue;
            }
            let painted = rule.split('{').nth(1).unwrap_or_default();
            // Only the legend's own words may be repainted; the wedge is
            // outlined.
            let legend = rule.contains(".pie-legend-label") || rule.contains(".pie-legend-share");
            assert!(
                !painted.contains("fill:") || legend,
                "a wedge rule repaints: {rule}"
            );
        }
        assert!(css.contains(".pie-slice[data-rel=\"0\"]{stroke:"), "{css}");
    }

    #[test]
    fn every_slice_is_addressable_and_carries_its_number() {
        let nodes = all(&drawn("pie\n\"Rust\" : 40\n\"Go\" : 10"));
        let slices = with_class(&nodes, "node");
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].id.as_deref(), Some("Rust"));
        assert_eq!(slices[0].value.as_deref(), Some("40"));
        assert_eq!(slices[1].value.as_deref(), Some("10"));
    }

    /// The arcs of a path, as (radius, which way round).
    fn arcs(node: &Node) -> Vec<(f64, bool)> {
        let Content::Shape(Shape::Path(segs)) = &node.content else {
            return Vec::new();
        };
        segs.iter()
            .filter_map(|seg| match seg {
                Seg::Arc { r, sweep, .. } => Some((r.width, *sweep)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_wedge_is_a_sector_of_the_ring_and_never_reaches_the_centre() {
        let two = all(&drawn("pie\n\"a\" : 1\n\"b\" : 1"));
        let paths: Vec<&Node> = with_class(&two, "pie-slice");
        // Two wedges plus two legend swatches.
        assert_eq!(paths.len(), 4);
        // Out along the rim and back along the hole: two arcs, two radii. A
        // wedge of the disc would have one arc and a corner at the centre.
        assert_eq!(arcs(paths[0]), vec![(RADIUS, true), (INNER_RADIUS, false)]);
        let Content::Shape(Shape::Path(segs)) = &paths[0].content else {
            panic!("a wedge is a path");
        };
        assert_eq!(segs.len(), 5);
        assert!(matches!(segs.first(), Some(Seg::MoveTo(_))));
    }

    #[test]
    fn a_whole_circle_is_a_ring_rather_than_a_disc() {
        let one = all(&drawn("pie\n\"only\" : 3"));
        let ring = with_class(&one, "pie-slice");
        // Two half-arcs each way round: a single arc back to its own start
        // sweeps nothing at all.
        assert_eq!(
            arcs(ring[0]),
            vec![
                (RADIUS, false),
                (RADIUS, false),
                (INNER_RADIUS, true),
                (INNER_RADIUS, true),
            ]
        );
        // The hole is wound against the rim, which is what cuts it out under the
        // non-zero fill rule rather than painting over it.
        let (rim, hole) = (arcs(ring[0])[0].1, arcs(ring[0])[3].1);
        assert_ne!(rim, hole);
    }

    #[test]
    fn a_wedge_wider_than_a_half_turn_takes_the_long_way_round() {
        let nodes = all(&drawn("pie\n\"big\" : 9\n\"small\" : 1"));
        let paths = with_class(&nodes, "pie-slice");
        let large = |node: &Node| match &node.content {
            Content::Shape(Shape::Path(segs)) => segs
                .iter()
                .any(|s| matches!(s, Seg::Arc { large: true, .. })),
            _ => false,
        };
        assert!(large(paths[0]), "the 90% wedge needs the long arc");
        assert!(!large(paths[1]), "the 10% wedge does not");
    }

    #[test]
    fn a_share_too_thin_to_hold_its_number_does_not_get_one() {
        let nodes = all(&drawn("pie\n\"big\" : 97\n\"sliver\" : 3"));
        assert_eq!(with_class(&nodes, "pie-slice-label").len(), 1);
    }

    #[test]
    fn each_share_is_inked_against_the_wedge_it_is_written_on() {
        // The wedges span a band of lightness on purpose, so one ink for all of
        // them cannot read on all of them: a pale wedge had white on it.
        let scene = drawn("pie\n\"a\" : 40\n\"b\" : 25\n\"c\" : 20\n\"d\" : 15");
        let nodes = all(&scene);
        for index in 0..4 {
            assert_eq!(
                with_class(&nodes, &format!("pie-on-{index}")).len(),
                1,
                "every labelled wedge names its own ink"
            );
            assert!(
                scene.style.contains(&format!(".pie-on-{index}{{fill:")),
                "{}",
                scene.style
            );
        }
        // And the ink is decided from the wedge rather than from the page.
        assert!(
            !scene.style.contains(".pie-slice-label{fill:"),
            "{}",
            scene.style
        );
    }

    #[test]
    fn a_wedge_with_no_number_needs_no_ink_rule() {
        // The sliver has no label, so a rule for it would style nothing.
        let scene = drawn("pie\n\"big\" : 97\n\"sliver\" : 3");
        assert!(scene.style.contains(".pie-on-0{"), "{}", scene.style);
        assert!(!scene.style.contains(".pie-on-1{"), "{}", scene.style);
    }

    #[test]
    fn a_standalone_pie_inks_its_shares_with_literals() {
        // No cascade to defer the choice to, so it is made here.
        let scene = super::render(
            "pie\n\"a\" : 60\n\"b\" : 40",
            &Theme::default(),
            &ColorMode::Fixed,
        );
        assert!(scene.style.contains(".pie-on-0{fill:#"), "{}", scene.style);
        assert!(!scene.style.contains("hsl(from"), "{}", scene.style);
    }

    #[test]
    fn the_number_inside_a_wedge_is_a_whole_percent() {
        let nodes = all(&drawn("pie\n\"a\" : 1\n\"b\" : 2"));
        let labels = with_class(&nodes, "pie-slice-label");
        let content = |n: &Node| match &n.content {
            Content::Text(run) => run.content.clone(),
            _ => String::new(),
        };
        assert_eq!(content(labels[0]), "33%");
        assert_eq!(content(labels[1]), "67%");
    }

    #[test]
    fn the_legend_names_every_slice_and_shows_its_colour() {
        let nodes = all(&drawn("pie\n\"a\" : 1\n\"b\" : 1"));
        assert_eq!(with_class(&nodes, "pie-legend-label").len(), 2);
        assert_eq!(with_class(&nodes, "pie-color-1").len(), 2);
    }

    #[test]
    fn the_first_slice_takes_the_accent_and_the_rest_derive_from_it() {
        let scene = drawn("pie\n\"a\" : 1\n\"b\" : 1\n\"c\" : 1");
        assert!(
            scene.style.contains(".pie-color-0{fill:var(--ags-accent"),
            "{}",
            scene.style
        );
        // A hue rotation, which no blend can express.
        assert!(
            scene.style.contains(".pie-color-1{fill:hsl(from"),
            "{}",
            scene.style
        );
    }

    #[test]
    fn a_title_is_drawn_and_an_absent_one_is_not() {
        assert_eq!(
            with_class(&all(&drawn("pie title X\n\"a\" : 1")), "pie-title").len(),
            1
        );
        assert!(with_class(&all(&drawn("pie\n\"a\" : 1")), "pie-title").is_empty());
    }

    #[test]
    fn a_chart_of_nothing_still_yields_a_canvas() {
        let scene = drawn("pie");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
    }

    #[test]
    fn a_standalone_image_leaves_nothing_for_a_cascade_to_resolve() {
        let scene = render(
            "pie\n\"a\" : 1\n\"b\" : 1",
            &Theme::default(),
            &ColorMode::Fixed,
        );
        assert!(!scene.style.contains("color-mix"), "{}", scene.style);
        assert!(scene.style.contains("--ags-bg:#"), "{}", scene.style);
    }
}
