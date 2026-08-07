//! A placed C4 diagram, drawn into the scene.
//!
//! Identity contract, which feedback is keyed to and which a port may not
//! change: every element box is a group carrying `data-id="<alias>"`, and every
//! relationship is a path carrying `data-from`, `data-to` and `data-step`.
//!
//! Paint order is expressed as layers rather than as document order — frames
//! behind wires, wires behind boxes, badges above them, and the
//! description bubbles above everything. The bubbles are the reason the layer is
//! a field at all: SVG has no z-index, so a bubble living inside its own step
//! group was painted over by every step drawn after it, which put badge 5 on top
//! of the text belonging to edge 4.

use crate::api::ColorMode;
use crate::icons::icon;
use crate::metrics::text_width;
use crate::scene::{
    Anchor, Color, Content, Font, Layer, Marker, Node, Paint, Point, Role, Scene, Seg, Shape, Size,
    TextRun,
};
use crate::theme::{style_block, Theme};

use super::config as l;
use super::positioned::{Placed, PlacedBoundary, PlacedElement, PlacedRelationship};
use super::style::style;
use super::types::{ElementKind, Variant};

/// Baseline offset that centres a line of text on its anchor point.
const BASELINE: &str = "0.35em";

/// Corner radius for an edge bend.
///
/// Deliberately small: a wide fillet on a short segment reads as a curve rather
/// than a corner, and neighbouring bends with different clamped radii look
/// inconsistent.
const CORNER: f64 = 5.0;

/// Hover radius around an arrowhead — the head itself is only a few pixels wide.
const TIP_HIT_R: f64 = 13.0;

/// The arrowhead every relationship ends with.
const ARROW_ID: &str = "c4-arrow";

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn wh(width: f64, height: f64) -> Size {
    Size { width, height }
}

/// A centred line of text.
fn label(at: Point, content: &str, size: f64, weight: u32, class: &str) -> Node {
    text_at(at, content, size, weight, class, Anchor::Middle, false)
}

fn text_at(
    at: Point,
    content: &str,
    size: f64,
    weight: u32,
    class: &str,
    anchor: Anchor,
    italic: bool,
) -> Node {
    Node::new(
        Role::Label,
        Content::Text(TextRun {
            at,
            anchor,
            font: Font {
                size,
                weight,
                italic,
            },
            dy: Some(BASELINE.to_string()),
            content: content.to_string(),
        }),
    )
    .classed(class)
}

fn kind_class(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::Person => "person",
        ElementKind::System => "system",
        ElementKind::Container => "container",
        ElementKind::Component => "component",
    }
}

/// The glyph an element's kind and storage variant ask for.
fn glyph_name(el: &PlacedElement) -> &'static str {
    if el.kind == ElementKind::Person {
        return "person";
    }
    match el.variant {
        Some(Variant::Db) => "database",
        Some(Variant::Queue) => "queue",
        None => "server",
    }
}

/// One element box: its outline, its glyph and its stack of text.
fn element(el: &PlacedElement) -> Node {
    let is_person = el.kind == ElementKind::Person;
    let rx = if is_person { 12.0 } else { 4.0 };
    let mut parts: Vec<Node> = Vec::new();

    let mut outline = Node::new(
        Role::Node,
        Content::Shape(Shape::Rect {
            at: pt(el.rect.x, el.rect.y),
            size: wh(el.rect.width, el.rect.height),
            rx,
            ry: rx,
        }),
    )
    .classed("c4-box")
    .classed(format!("c4-{}", kind_class(el.kind)));
    if el.external {
        outline = outline.classed("c4-box-ext");
    }
    parts.push(outline);

    if is_person {
        // An accent strip across the top. Drawn twice: once rounded to follow
        // the box corners, then squared off along its lower edge so it reads as
        // a strip rather than as a pill.
        parts.push(
            Node::new(
                Role::Decoration,
                Content::Shape(Shape::Rect {
                    at: pt(el.rect.x, el.rect.y),
                    size: wh(el.rect.width, l::PERSON_BAR),
                    rx,
                    ry: rx,
                }),
            )
            .classed("c4-accent"),
        );
        parts.push(
            Node::new(
                Role::Decoration,
                Content::Shape(Shape::Rect {
                    at: pt(el.rect.x, el.rect.y + l::PERSON_BAR / 2.0),
                    size: wh(el.rect.width, l::PERSON_BAR / 2.0),
                    rx: 0.0,
                    ry: 0.0,
                }),
            )
            .classed("c4-accent"),
        );
    }

    let cx = el.rect.center_x();
    parts.push(icon(
        glyph_name(el),
        pt(cx - l::ICON_SIZE / 2.0, el.rect.y + l::TOP_PAD),
        l::ICON_SIZE,
        "c4-icon",
    ));

    let mut ty = el.rect.y + l::TOP_PAD + l::ICON_SIZE + l::ICON_GAP;
    parts.push(label(
        pt(cx, ty + l::TAG_H / 2.0),
        &el.tag,
        l::TAG_FONT,
        l::TAG_WEIGHT,
        if is_person { "c4-tag-person" } else { "c4-tag" },
    ));
    ty += l::TAG_H;
    parts.push(label(
        pt(cx, ty + l::LABEL_H / 2.0),
        &el.label,
        l::LABEL_FONT,
        l::LABEL_WEIGHT,
        "c4-label",
    ));
    ty += l::LABEL_H;
    if let Some(techn) = &el.techn {
        parts.push(text_at(
            pt(cx, ty + l::TECHN_H / 2.0),
            techn,
            l::TECHN_FONT,
            l::TECHN_WEIGHT,
            "c4-techn",
            Anchor::Middle,
            true,
        ));
        ty += l::TECHN_H;
    }
    for line in &el.descr {
        parts.push(label(
            pt(cx, ty + l::DESCR_H / 2.0),
            line,
            l::DESCR_FONT,
            l::DESCR_WEIGHT,
            "c4-descr",
        ));
        ty += l::DESCR_H;
    }

    Node::new(Role::Node, Content::Group(parts))
        .classed("node")
        .with_id(el.alias.clone())
        .tagged("kind", kind_class(el.kind))
}

/// One boundary frame and its name.
fn boundary(b: &PlacedBoundary) -> Node {
    let frame = Node::new(
        Role::Frame,
        Content::Shape(Shape::Rect {
            at: pt(b.rect.x, b.rect.y),
            size: wh(b.rect.width, b.rect.height),
            rx: 6.0,
            ry: 6.0,
        }),
    )
    .classed("c4-boundary");
    let name = text_at(
        pt(b.rect.x + 12.0, b.rect.y + l::BOUNDARY_LABEL_H / 2.0),
        &b.label,
        l::TAG_FONT + 1.0,
        l::TAG_WEIGHT,
        "c4-boundary-label",
        Anchor::Start,
        false,
    );
    Node::new(Role::Frame, Content::Group(vec![frame, name]))
        .classed("c4-boundary-group")
        .tagged("boundary", b.alias.clone())
}

/// An axis-aligned polyline with rounded bends.
///
/// Each corner becomes a quadratic curve that starts and ends a short way back
/// along the adjacent legs, with the original corner as the control point. The
/// radius is clamped to half of the shorter neighbouring leg, so a tight jog can
/// never overshoot into the leg beyond it.
fn orthogonal_path(points: &[Point]) -> Vec<Seg> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let last = points.last().copied().unwrap_or(*first);
    if points.len() < 3 {
        return vec![Seg::MoveTo(*first), Seg::LineTo(last)];
    }

    let mut segs = vec![Seg::MoveTo(*first)];
    for w in points.windows(3) {
        let (Some(&prev), Some(&corner), Some(&next)) = (w.first(), w.get(1), w.get(2)) else {
            continue;
        };
        let in_len = (corner.x - prev.x).hypot(corner.y - prev.y);
        let out_len = (next.x - corner.x).hypot(next.y - corner.y);
        let rad = CORNER.min(in_len / 2.0).min(out_len / 2.0);
        if rad < 0.5 {
            segs.push(Seg::LineTo(corner));
            continue;
        }
        segs.push(Seg::LineTo(pt(
            corner.x - (corner.x - prev.x) / in_len * rad,
            corner.y - (corner.y - prev.y) / in_len * rad,
        )));
        segs.push(Seg::Quad {
            ctrl: corner,
            to: pt(
                corner.x + (next.x - corner.x) / out_len * rad,
                corner.y + (next.y - corner.y) / out_len * rad,
            ),
        });
    }
    segs.push(Seg::LineTo(last));
    segs
}

/// One relationship wire.
///
/// The `<title>` carries the step marker as well as the prose, because the badge
/// and the description bubble both show it — so the native tooltip reads as the
/// same object rather than as loose text.
fn edge(rel: &PlacedRelationship) -> Node {
    let mut node = Node::new(
        Role::Edge,
        Content::Shape(Shape::Path(orthogonal_path(&rel.points))),
    )
    .classed("c4-edge")
    .tagged("from", rel.from.clone())
    .tagged("to", rel.to.clone())
    .tagged("step", rel.step.clone())
    .painted(Paint {
        marker_start: rel.bidirectional.then(|| ARROW_ID.to_string()),
        marker_end: Some(ARROW_ID.to_string()),
        ..Paint::default()
    });
    if !rel.description.is_empty() {
        node = node.titled(format!("{}. {}", rel.step, rel.description));
    }
    node
}

/// The step circle a wire carries.
///
/// Both the circle and the digit are painted, so either can be the hover target
/// — the digit sits on top, and would otherwise be a dead spot in the middle.
fn badge(c: Point, step: &str) -> Vec<Node> {
    vec![
        Node::new(
            Role::Label,
            Content::Shape(Shape::Circle {
                c,
                r: l::BADGE_SIZE / 2.0,
            }),
        )
        .classed("c4-badge"),
        label(c, step, 11.0, 600, "c4-badge-text"),
    ]
}

/// The badge on a wire, plus a hover target over each arrowhead.
///
/// The badge sits near the wire's *source*, so the arrowhead — the end a reader
/// actually looks at to ask "what is this?" — can be a long way from it. A
/// transparent disc over each head joins the same step group, so hovering a tip
/// raises the description exactly as hovering the badge does.
fn edge_badge(rel: &PlacedRelationship) -> Option<Node> {
    if rel.description.is_empty() {
        return None;
    }
    let disc = |c: Point| {
        Node::new(
            Role::Label,
            Content::Shape(Shape::Circle { c, r: TIP_HIT_R }),
        )
        .classed("c4-tip-hit")
    };
    let mut parts = vec![disc(rel.end)];
    if rel.bidirectional {
        parts.push(disc(rel.start));
    }
    parts.extend(badge(rel.badge_center, &rel.step));
    Some(
        Node::new(Role::Label, Content::Group(parts))
            .classed("c4-step")
            .tagged("step", rel.step.clone()),
    )
}

/// The description bubble shown beside a hover target.
///
/// Drawn rather than left to a native tooltip because that waits about a second
/// before appearing — fine for an afterthought, too slow for the thing the reader
/// is asking about. Sits above its anchor, flipping below when there is no room,
/// and is clamped on both axes: anything outside the canvas is not drawn small,
/// it is cut in half.
fn bubble(anchor: Point, text: &str, canvas: Size, step: &str, at: &str) -> Node {
    let font = 11.5;
    let pad_x = 22.0;
    let height = 28.0;
    let width = text_width(text, font, 400) + 2.0 * pad_x;
    let clear = TIP_HIT_R + 6.0;
    let above = anchor.y - clear - height;
    let wanted = if above > 2.0 { above } else { anchor.y + clear };
    let y = 2.0_f64.max(wanted.min(canvas.height - height - 2.0));
    let x = 2.0_f64.max((anchor.x - width / 2.0).min(canvas.width - width - 2.0));
    let box_ = Node::new(
        Role::Decoration,
        Content::Shape(Shape::Rect {
            at: pt(x, y),
            size: wh(width, height),
            rx: 6.0,
            ry: 6.0,
        }),
    )
    .classed("c4-tip-box");
    let body = label(
        pt(x + width / 2.0, y + height / 2.0),
        text,
        font,
        400,
        "c4-tip-text",
    );
    Node::new(Role::Decoration, Content::Group(vec![box_, body]))
        .classed("c4-tip")
        .tagged("step", step.to_string())
        .tagged("at", at.to_string())
}

/// Every description bubble, as one trailing layer.
fn tips(placed: &Placed) -> Option<Node> {
    let canvas = wh(placed.width, placed.height);
    let mut parts = Vec::new();
    for rel in &placed.relationships {
        if rel.description.is_empty() {
            continue;
        }
        let text = format!("{}. {}", rel.step, rel.description);
        parts.push(bubble(rel.badge_center, &text, canvas, &rel.step, "badge"));
        parts.push(bubble(rel.end, &text, canvas, &rel.step, "tip"));
        if rel.bidirectional {
            parts.push(bubble(rel.start, &text, canvas, &rel.step, "tip"));
        }
    }
    (!parts.is_empty()).then(|| {
        Node::new(Role::Decoration, Content::Group(parts))
            .classed("c4-tips")
            .on(Layer::Overlay)
    })
}

/// The diagram's own name, centred in the band reserved above the content.
///
/// The band runs from the top of the canvas to where the first frame begins: the
/// page padding, the title's own height, and the gap below it. Centring on the
/// padding alone left 28px of clearance above and 14px below, which reads as the
/// title having slipped down toward the diagram.
fn title(placed: &Placed, text: &str) -> Node {
    let band = l::PADDING + l::TITLE_H + l::TITLE_GAP;
    label(
        pt(placed.width / 2.0, band / 2.0),
        text,
        16.0,
        600,
        "c4-title",
    )
    // With the frames, so it paints behind everything as the reference drew it.
    .on(Layer::Frame)
}

fn arrow_marker() -> Marker {
    Marker {
        id: ARROW_ID.to_string(),
        view: wh(10.0, 10.0),
        size: wh(7.0, 7.0),
        ref_x: 9.0,
        ref_y: 5.0,
        shape: Shape::Polygon(vec![pt(0.0, 0.0), pt(10.0, 5.0), pt(0.0, 10.0)]),
        paint: Paint {
            fill: Some(Color::Token {
                name: "_arrow".into(),
                fallback: crate::color::CHART_ACCENT_FALLBACK.into(),
            }),
            ..Paint::default()
        },
    }
}

/// Draw a placed C4 diagram.
pub fn scene(placed: &Placed, theme: &Theme, mode: &ColorMode) -> Scene {
    let mut out = Scene::new(wh(placed.width, placed.height));
    // Every numbered relationship gets a highlight rule. This used to read the
    // steps off the legend rows; with the legend gone the relationships are the
    // list, and an unlabelled one has no bubble to pair with.
    let steps: Vec<String> = placed
        .relationships
        .iter()
        .filter(|rel| !rel.description.is_empty())
        .map(|rel| rel.step.clone())
        .collect();
    out.colors = crate::theme::Colors::new(theme, mode);
    out.style = style(&style_block(theme, mode), &steps);
    out.markers.push(arrow_marker());

    if let Some(name) = &placed.title {
        out.push(title(placed, name));
    }
    for b in &placed.boundaries {
        out.push(boundary(b));
    }
    for rel in &placed.relationships {
        out.push(edge(rel));
    }
    for el in &placed.elements {
        out.push(element(el));
    }
    for rel in &placed.relationships {
        if let Some(node) = edge_badge(rel) {
            out.push(node);
        }
    }
    if let Some(node) = tips(placed) {
        out.push(node);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c4::{layout, parse};

    fn drawn(source: &str) -> Scene {
        scene(
            &layout(&parse(source)),
            &Theme::default(),
            &ColorMode::Tokens,
        )
    }

    /// Every node in the scene, groups flattened, in paint order.
    fn flatten(nodes: &[&Node], out: &mut Vec<Node>) {
        for node in nodes {
            out.push((*node).clone());
            if let Content::Group(children) = &node.content {
                let refs: Vec<&Node> = children.iter().collect();
                flatten(&refs, out);
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

    const SAMPLE: &str = "C4Container\ntitle A drawing\nPerson(dev,\"Developer\",\"Writes code\")\nContainerDb(store,\"code.db\",\"SQLite\")\nSystem_Ext(mail,\"Mail\")\nContainer_Boundary(app,\"App\"){\nContainer(api,\"API\",\"Rust\")\n}\nRel(dev,api,\"Queries\",\"HTTPS\")\nBiRel(api,store,\"Reads\")\n";

    #[test]
    fn every_element_carries_the_identity_feedback_is_keyed_to() {
        let scene = drawn(SAMPLE);
        let nodes = all(&scene);
        let ids: Vec<&str> = with_class(&nodes, "node")
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert_eq!(ids.len(), 4);
        for alias in ["dev", "store", "mail", "api"] {
            assert!(ids.contains(&alias), "{alias} missing from {ids:?}");
        }
    }

    #[test]
    fn every_wire_names_the_pair_it_joins_and_its_step() {
        let nodes = all(&drawn(SAMPLE));
        let edges = with_class(&nodes, "c4-edge");
        assert_eq!(edges.len(), 2);
        let first = edges[0];
        assert!(
            first.data.contains(&("from".into(), "dev".into())),
            "{:?}",
            first.data
        );
        assert!(first.data.contains(&("to".into(), "api".into())));
        assert!(first.data.iter().any(|(k, _)| k == "step"));
    }

    #[test]
    fn paint_order_puts_frames_behind_wires_behind_boxes() {
        let scene = drawn(SAMPLE);
        let layers: Vec<Layer> = scene.painted().iter().map(|n| n.layer).collect();
        assert!(layers.windows(2).all(|w| w[0] <= w[1]), "{layers:?}");
        assert_eq!(layers.first(), Some(&Layer::Frame));
        assert_eq!(layers.last(), Some(&Layer::Overlay));
    }

    #[test]
    fn a_bubble_paints_above_every_badge_including_later_ones() {
        // The bug the overlay layer exists to prevent: a bubble emitted inside
        // its own step group was covered by every step drawn after it.
        let scene = drawn(SAMPLE);
        let order = scene.painted();
        let tips = order
            .iter()
            .position(|n| n.class.iter().any(|c| c == "c4-tips"));
        let last_step = order
            .iter()
            .rposition(|n| n.class.iter().any(|c| c == "c4-step"));
        assert!(tips > last_step, "{tips:?} !> {last_step:?}");
    }

    #[test]
    fn a_person_gets_the_person_glyph_and_an_accent_strip() {
        let nodes = all(&drawn(SAMPLE));
        assert_eq!(with_class(&nodes, "c4-accent").len(), 2);
        assert_eq!(with_class(&nodes, "c4-tag-person").len(), 1);
        // ... and the box is more rounded than the rest.
        let person = with_class(&nodes, "c4-person");
        assert!(matches!(
            person.first().map(|n| &n.content),
            Some(Content::Shape(Shape::Rect { rx, .. })) if (*rx - 12.0).abs() < 1e-9
        ));
    }

    #[test]
    fn a_storage_variant_changes_the_glyph_but_not_the_kind() {
        let nodes = all(&drawn(SAMPLE));
        // `store` is a ContainerDb: still a container box, with a cylinder in it.
        assert_eq!(with_class(&nodes, "c4-container").len(), 2);
        let icons = with_class(&nodes, "c4-icon");
        assert_eq!(icons.len(), 4);
        assert!(icons.iter().all(|n| n.transform.is_some()));
    }

    #[test]
    fn every_kind_reaches_the_drawing_under_its_own_class() {
        // One box of each kind, so no arm of the mapping table is left to a
        // later diagram to discover.
        let nodes = all(&drawn(
            "C4Component\nPerson(p,\"P\")\nSystem(s,\"S\")\nContainer(c,\"C\",\"Rust\")\nComponent(k,\"K\",\"Rust\")\nSystemDb(sd,\"SD\")\nContainerQueue(cq,\"CQ\",\"Kafka\")",
        ));
        for class in ["c4-person", "c4-system", "c4-container", "c4-component"] {
            assert!(!with_class(&nodes, class).is_empty(), "{class} missing");
        }
        // A queue and a database are still a container and a system: the
        // variant changes the glyph, not the kind.
        assert_eq!(with_class(&nodes, "c4-system").len(), 2);
        assert_eq!(with_class(&nodes, "c4-container").len(), 2);
    }

    #[test]
    fn the_glyph_follows_the_storage_shape_rather_than_the_kind() {
        let placed = layout(&parse(
            "C4Container\nPerson(p,\"P\")\nSystemDb(sd,\"SD\")\nContainerQueue(cq,\"CQ\")\nContainer(plain,\"C\")",
        ));
        let glyph_of = |alias: &str| {
            placed
                .elements
                .iter()
                .find(|e| e.alias == alias)
                .map(glyph_name)
        };
        assert_eq!(glyph_of("p"), Some("person"));
        assert_eq!(glyph_of("sd"), Some("database"));
        assert_eq!(glyph_of("cq"), Some("queue"));
        assert_eq!(glyph_of("plain"), Some("server"));
    }

    #[test]
    fn an_external_element_is_marked_for_the_dashed_rule() {
        let nodes = all(&drawn(SAMPLE));
        assert_eq!(with_class(&nodes, "c4-box-ext").len(), 1);
    }

    #[test]
    fn a_two_headed_relationship_gets_a_head_and_a_hit_target_at_both_ends() {
        let nodes = all(&drawn(SAMPLE));
        let edges = with_class(&nodes, "c4-edge");
        let bi = edges
            .iter()
            .find(|n| n.data.contains(&("from".into(), "api".into())))
            .expect("the BiRel");
        assert_eq!(bi.paint.marker_start.as_deref(), Some(ARROW_ID));
        assert_eq!(bi.paint.marker_end.as_deref(), Some(ARROW_ID));
        // One disc for each head, and one for each end of the two-headed wire.
        assert_eq!(with_class(&nodes, "c4-tip-hit").len(), 3);
    }

    #[test]
    fn a_wire_carries_its_prose_as_a_native_tooltip_too() {
        let nodes = all(&drawn(SAMPLE));
        let edges = with_class(&nodes, "c4-edge");
        assert_eq!(edges[0].title.as_deref(), Some("1. Queries [HTTPS]"));
    }

    #[test]
    fn each_numbered_wire_carries_exactly_one_badge() {
        // Two, not four: there used to be a second copy of every badge in the
        // key beneath the diagram.
        let nodes = all(&drawn(SAMPLE));
        assert_eq!(with_class(&nodes, "c4-badge").len(), 2);
        assert!(with_class(&nodes, "c4-legend").is_empty());
    }

    #[test]
    fn the_style_block_carries_a_hover_group_for_every_step() {
        let scene = drawn(SAMPLE);
        assert!(scene.style.contains("[data-step=\"1\"]"), "{}", scene.style);
        assert!(scene.style.contains("[data-step=\"2\"]"), "{}", scene.style);
    }

    #[test]
    fn a_title_paints_behind_the_diagram_rather_than_over_it() {
        let nodes = all(&drawn(SAMPLE));
        let title = with_class(&nodes, "c4-title");
        assert_eq!(title.len(), 1);
        assert_eq!(title[0].layer, Layer::Frame);
        let scene = drawn("C4Context\nSystem(a,\"A\")");
        assert!(with_class(&all(&scene), "c4-title").is_empty());
    }

    #[test]
    fn a_boundary_frame_names_itself_for_the_reader_and_for_selection() {
        let nodes = all(&drawn(SAMPLE));
        let group = with_class(&nodes, "c4-boundary-group");
        assert_eq!(group.len(), 1);
        assert!(group[0].data.contains(&("boundary".into(), "app".into())));
        assert_eq!(with_class(&nodes, "c4-boundary-label").len(), 1);
    }

    #[test]
    fn a_straight_wire_is_two_points_and_a_bend_becomes_a_curve() {
        let straight = orthogonal_path(&[pt(0.0, 0.0), pt(100.0, 0.0)]);
        assert_eq!(straight.len(), 2);
        let bent = orthogonal_path(&[pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0)]);
        assert!(
            bent.iter().any(|s| matches!(s, Seg::Quad { .. })),
            "{bent:?}"
        );
        assert!(orthogonal_path(&[]).is_empty());
    }

    #[test]
    fn a_bend_too_tight_to_round_stays_a_corner() {
        // Legs shorter than the corner radius would overshoot into the leg
        // beyond, so the corner is drawn square instead.
        let tight = orthogonal_path(&[pt(0.0, 0.0), pt(0.6, 0.0), pt(0.6, 0.6)]);
        assert!(
            !tight.iter().any(|s| matches!(s, Seg::Quad { .. })),
            "{tight:?}"
        );
    }

    #[test]
    fn a_bubble_never_leaves_the_canvas() {
        let canvas = wh(200.0, 100.0);
        // Anchored hard against each corner in turn.
        for anchor in [pt(0.0, 0.0), pt(200.0, 100.0), pt(0.0, 100.0)] {
            let node = bubble(
                anchor,
                "a long enough description to be wide",
                canvas,
                "1",
                "tip",
            );
            let Content::Group(parts) = &node.content else {
                continue;
            };
            let Some(Content::Shape(Shape::Rect { at, size, .. })) =
                parts.first().map(|n| &n.content)
            else {
                continue;
            };
            assert!(at.x >= 2.0 - 1e-9, "{at:?}");
            assert!(at.y >= 2.0 - 1e-9, "{at:?}");
            assert!(at.y + size.height <= canvas.height - 2.0 + 1e-9, "{at:?}");
        }
    }

    #[test]
    fn a_bubble_flips_below_its_anchor_when_there_is_no_room_above() {
        let canvas = wh(400.0, 400.0);
        let high = bubble(pt(200.0, 10.0), "text", canvas, "1", "tip");
        let low = bubble(pt(200.0, 300.0), "text", canvas, "1", "tip");
        let y_of = |node: &Node| match &node.content {
            Content::Group(parts) => match parts.first().map(|n| &n.content) {
                Some(Content::Shape(Shape::Rect { at, .. })) => at.y,
                _ => f64::NAN,
            },
            _ => f64::NAN,
        };
        assert!(y_of(&high) > 10.0, "{}", y_of(&high));
        assert!(y_of(&low) < 300.0, "{}", y_of(&low));
    }

    #[test]
    fn an_unlabelled_relationship_gets_no_badge_and_no_bubble() {
        let scene = drawn("C4Context\nSystem(a,\"A\")\nSystem(b,\"B\")\nRel(a,b,\"\")");
        let nodes = all(&scene);
        assert!(with_class(&nodes, "c4-badge").is_empty());
        assert!(with_class(&nodes, "c4-tips").is_empty());
        // The wire itself is still drawn, and carries no tooltip.
        assert_eq!(with_class(&nodes, "c4-edge").len(), 1);
        assert!(with_class(&nodes, "c4-edge")[0].title.is_none());
    }

    #[test]
    fn an_empty_diagram_draws_a_canvas_and_nothing_else() {
        let scene = drawn("C4Context");
        assert!(scene.canvas.width > 0.0);
        assert!(all(&scene).is_empty());
        assert_eq!(scene.markers.len(), 1);
    }

    #[test]
    fn the_arrowhead_is_themed_like_everything_else() {
        let scene = drawn(SAMPLE);
        let marker = &scene.markers[0];
        assert_eq!(marker.view, wh(10.0, 10.0));
        assert!(matches!(marker.paint.fill, Some(Color::Token { .. })));
    }
}
