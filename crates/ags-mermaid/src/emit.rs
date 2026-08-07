//! Scene to SVG text.
//!
//! One writer for every diagram type. Escaping, coordinate formatting and colour
//! resolution happen here and nowhere else, so they cannot drift between types —
//! and a type module ends at "describe the drawing" rather than at "assemble a
//! string".

use std::fmt::Write as _;

use crate::round::coord;
use crate::scene::{
    Anchor, Color, Content, Font, Marker, Node, Paint, Point, Scene, Seg, Shape, TextRun, Transform,
};
use crate::text::escape_xml;

/// Render `scene` as an SVG document.
pub fn svg(scene: &Scene) -> String {
    let mut out = String::with_capacity(2048);
    let (w, h) = (coord(scene.canvas.width), coord(scene.canvas.height));
    // Resolved through the scene's own colour config, so a stylesheet authored in
    // the token vocabulary comes out written the way this target needs it —
    // references for a page, literals for an image.
    let css = scene.colors.resolve_css(&scene.style);
    let scope = (!css.is_empty()).then(|| scope_of(&css));
    let class = scope
        .as_ref()
        .map_or_else(String::new, |s| format!(" class=\"{s}\""));
    _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\"{class} width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">"
    );
    if let Some(scope) = &scope {
        _ = write!(out, "\n<style>{}</style>", scoped(&css, scope));
    }
    if !scene.markers.is_empty() {
        out.push_str("\n<defs>");
        for marker in &scene.markers {
            out.push_str(&marker_svg(marker, &scene.colors));
        }
        out.push_str("</defs>");
    }
    for node in scene.painted() {
        out.push('\n');
        out.push_str(&node_svg(node, &scene.colors));
    }
    out.push_str("\n</svg>");
    out
}

/// A name unique to this stylesheet, so one diagram's rules cannot reach another.
///
/// A `<style>` inside an inline SVG is **not** scoped to that SVG — it is a
/// stylesheet of the document embedding it. So every diagram on a page shares
/// every rule any of them declares, and the generic ones silently restyle each
/// other: a pie's wedges come out the colour of a flowchart's boxes, because
/// `.node path` outranks `.pie-color-0` and the flowchart declared it.
///
/// Derived from the stylesheet rather than counted or drawn at random, so the
/// same drawing emits the same bytes however many were rendered before it — a
/// property the raster reference and every byte-comparison in the suite rest on.
/// Two diagrams whose CSS is identical share a scope, which is harmless: the
/// rules they would trade are the rules they both already have.
fn scope_of(css: &str) -> String {
    // FNV-1a, rather than `DefaultHasher`, which is explicitly not promised to be
    // stable across releases. This has to hold across toolchains and machines.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in css.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("ags-{hash:016x}")
}

/// Rewrite every selector in `css` so it matches only inside `scope`.
///
/// The rules are flat — every one is `selectors{declarations}`, with no at-rules
/// and no nesting — because they are all generated here rather than authored, so
/// splitting on the braces is enough.
#[expect(
    clippy::string_slice,
    reason = "every index comes from `find` on an ASCII brace, which can only \
              land on a character boundary"
)]
fn scoped(css: &str, scope: &str) -> String {
    let mut out = String::with_capacity(css.len() + css.len() / 3);
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let (selectors, tail) = rest.split_at(open);
        // A rule that never closes cannot be confined, and emitting the rest
        // verbatim would put an unscoped selector on the page.
        let Some(close) = tail.find('}') else {
            return out;
        };
        for (index, one) in selectors.split(',').enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&confine(one.trim(), scope));
        }
        out.push_str(&tail[..=close]);
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// One selector, confined to `scope`.
///
/// A selector naming `svg` means this drawing's own root — the token block, and
/// C4's `svg:has(…)` hover pairs — so the class goes *on* it. Everything else
/// names something inside, and becomes a descendant of that root.
fn confine(selector: &str, scope: &str) -> String {
    if let Some(rest) = selector.strip_prefix("svg") {
        let root = rest.is_empty()
            || rest.starts_with([':', '[', '.', '#'])
            || rest.starts_with(char::is_whitespace);
        if root {
            return format!("svg.{scope}{rest}");
        }
    }
    format!("svg.{scope} {selector}")
}

/// An arrowhead definition.
fn marker_svg(m: &Marker, colors: &crate::theme::Colors) -> String {
    format!(
        "<marker id=\"{id}\" viewBox=\"0 0 {vw} {vh}\" markerWidth=\"{w}\" markerHeight=\"{h}\" \
         refX=\"{rx}\" refY=\"{ry}\" orient=\"auto-start-reverse\"><{tag}{paint}{geom}/></marker>",
        id = escape_xml(&m.id),
        vw = coord(m.view.width),
        vh = coord(m.view.height),
        w = coord(m.size.width),
        h = coord(m.size.height),
        rx = coord(m.ref_x),
        ry = coord(m.ref_y),
        tag = shape_tag(&m.shape),
        paint = paint_attrs(&m.paint, colors),
        geom = shape_attrs(&m.shape),
    )
}

/// One node, and its children when it is a group.
fn node_svg(node: &Node, colors: &crate::theme::Colors) -> String {
    let common = format!(
        "{}{}{}{}{}{}",
        attr("class", &node.class.join(" ")),
        attr_opt("data-id", node.id.as_deref()),
        attr_opt("data-value", node.value.as_deref()),
        node.data
            .iter()
            .map(|(k, v)| attr(&format!("data-{k}"), v))
            .collect::<String>(),
        node.transform.map(transform_attr).unwrap_or_default(),
        paint_attrs(&node.paint, colors),
    );
    let title = node
        .title
        .as_deref()
        .map(|t| format!("<title>{}</title>", escape_xml(t)))
        .unwrap_or_default();
    match &node.content {
        Content::Group(children) => {
            let inner: String = children.iter().map(|c| node_svg(c, colors)).collect();
            format!("<g{common}>{title}{inner}</g>")
        }
        Content::Shape(shape) => {
            let body = format!("<{}{common}{}", shape_tag(shape), shape_attrs(shape));
            if title.is_empty() {
                format!("{body}/>")
            } else {
                format!("{body}>{title}</{}>", shape_tag(shape))
            }
        }
        Content::Text(run) => text_svg(run, &common, &title),
    }
}

/// The element name for a shape.
const fn shape_tag(shape: &Shape) -> &'static str {
    match shape {
        Shape::Rect { .. } => "rect",
        Shape::Circle { .. } => "circle",
        Shape::Ellipse { .. } => "ellipse",
        Shape::Line { .. } => "line",
        Shape::Polyline(_) => "polyline",
        Shape::Polygon(_) => "polygon",
        Shape::Path(_) => "path",
    }
}

/// The geometry attributes for a shape.
fn shape_attrs(shape: &Shape) -> String {
    match shape {
        Shape::Rect { at, size, rx, ry } => format!(
            " x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{}",
            coord(at.x),
            coord(at.y),
            coord(size.width),
            coord(size.height),
            round_attr("rx", *rx),
            round_attr("ry", *ry),
        ),
        Shape::Circle { c, r } => format!(
            " cx=\"{}\" cy=\"{}\" r=\"{}\"",
            coord(c.x),
            coord(c.y),
            coord(*r)
        ),
        Shape::Ellipse { c, rx, ry } => format!(
            " cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"",
            coord(c.x),
            coord(c.y),
            coord(*rx),
            coord(*ry)
        ),
        Shape::Line { a, b } => format!(
            " x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
            coord(a.x),
            coord(a.y),
            coord(b.x),
            coord(b.y)
        ),
        Shape::Polyline(points) | Shape::Polygon(points) => {
            format!(" points=\"{}\"", points_attr(points))
        }
        Shape::Path(segs) => format!(" d=\"{}\"", path_data(segs)),
    }
}

fn points_attr(points: &[Point]) -> String {
    points
        .iter()
        .map(|p| format!("{},{}", coord(p.x), coord(p.y)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Segments to a `d` attribute.
///
/// The scene holds segments and this is the only place they become a string, so
/// nothing downstream has to read one back.
fn path_data(segs: &[Seg]) -> String {
    let mut out = String::new();
    for seg in segs {
        if !out.is_empty() {
            out.push(' ');
        }
        match seg {
            Seg::MoveTo(p) => {
                _ = write!(out, "M{},{}", coord(p.x), coord(p.y));
            }
            Seg::LineTo(p) => {
                _ = write!(out, "L{},{}", coord(p.x), coord(p.y));
            }
            Seg::Quad { ctrl, to } => {
                _ = write!(
                    out,
                    "Q{},{} {},{}",
                    coord(ctrl.x),
                    coord(ctrl.y),
                    coord(to.x),
                    coord(to.y)
                );
            }
            Seg::Cubic { c1, c2, to } => {
                _ = write!(
                    out,
                    "C{},{} {},{} {},{}",
                    coord(c1.x),
                    coord(c1.y),
                    coord(c2.x),
                    coord(c2.y),
                    coord(to.x),
                    coord(to.y)
                );
            }
            Seg::Arc {
                r,
                large,
                sweep,
                to,
            } => {
                _ = write!(
                    out,
                    "A{},{} 0 {} {} {},{}",
                    coord(r.width),
                    coord(r.height),
                    u8::from(*large),
                    u8::from(*sweep),
                    coord(to.x),
                    coord(to.y)
                );
            }
            Seg::Close => out.push('Z'),
        }
    }
    out
}

fn text_svg(run: &TextRun, common: &str, title: &str) -> String {
    format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\"{}{}{common}>{title}{}</text>",
        coord(run.at.x),
        coord(run.at.y),
        anchor_name(run.anchor),
        run.dy.as_deref().map(|d| attr("dy", d)).unwrap_or_default(),
        font_attrs(&run.font),
        escape_xml(&run.content),
    )
}

const fn anchor_name(anchor: Anchor) -> &'static str {
    match anchor {
        Anchor::Start => "start",
        Anchor::Middle => "middle",
        Anchor::End => "end",
    }
}

fn font_attrs(font: &Font) -> String {
    let italic = if font.italic {
        " font-style=\"italic\""
    } else {
        ""
    };
    format!(
        " font-size=\"{}\" font-weight=\"{}\"{italic}",
        coord(font.size),
        font.weight
    )
}

/// A colour as SVG sees it.
///
/// A token always carries its fallback, so a diagram lifted out of the page that
/// defines the token still renders.
fn color_value(color: &Color, colors: &crate::theme::Colors) -> String {
    match color {
        // The scene names the colour; the config says how to write it. This used
        // to hardcode the reference form, which meant a drawing with no cascade
        // behind it emitted `var()` calls that nothing would ever resolve.
        Color::Token { name, fallback } => colors.token(name, fallback),
        Color::Literal(value) => value.clone(),
        Color::None => "none".to_string(),
    }
}

fn paint_attrs(paint: &Paint, colors: &crate::theme::Colors) -> String {
    let dash = paint
        .dash
        .as_ref()
        .map(|d| {
            let list = d.iter().map(|v| coord(*v)).collect::<Vec<_>>().join(",");
            attr("stroke-dasharray", &list)
        })
        .unwrap_or_default();
    format!(
        "{}{}{}{dash}{}{}",
        paint
            .fill
            .as_ref()
            .map(|c| attr("fill", &color_value(c, colors)))
            .unwrap_or_default(),
        paint
            .stroke
            .as_ref()
            .map(|c| attr("stroke", &color_value(c, colors)))
            .unwrap_or_default(),
        paint
            .stroke_width
            .map(|w| attr("stroke-width", &coord(w)))
            .unwrap_or_default(),
        paint
            .marker_start
            .as_deref()
            .map(|m| attr("marker-start", &format!("url(#{m})")))
            .unwrap_or_default(),
        paint
            .marker_end
            .as_deref()
            .map(|m| attr("marker-end", &format!("url(#{m})")))
            .unwrap_or_default(),
    )
}

fn transform_attr(transform: Transform) -> String {
    let value = match transform {
        Transform::Rotate { deg, about } => format!(
            "rotate({},{},{})",
            coord(deg),
            coord(about.x),
            coord(about.y)
        ),
        Transform::Translate { by } => format!(
            "translate({},{})",
            crate::round::ratio(by.x),
            crate::round::ratio(by.y)
        ),
        // Three decimals rather than a coordinate's one. The offset is not
        // itself scaled — `translate` applies after `scale` — so this is purely
        // about how accurately a glyph lands, and a glyph is small enough that a
        // twentieth of a pixel is visible against the text beside it.
        Transform::TranslateScale { at, scale } => format!(
            "translate({},{}) scale({})",
            crate::round::ratio(at.x),
            crate::round::ratio(at.y),
            crate::round::ratio(scale)
        ),
    };
    attr("transform", &value)
}

/// An attribute, omitted when its value is empty.
fn attr(name: &str, value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    format!(" {name}=\"{}\"", escape_xml(value))
}

fn attr_opt(name: &str, value: Option<&str>) -> String {
    value.map(|v| attr(name, v)).unwrap_or_default()
}

/// A numeric attribute, omitted when zero.
fn round_attr(name: &str, value: f64) -> String {
    if value == 0.0 {
        return String::new();
    }
    attr(name, &coord(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_is_confined_to_the_drawing_it_was_written_for() {
        // A `<style>` inside inline SVG is document-global, so an unconfined
        // rule reaches every other diagram on the page.
        assert_eq!(confine("svg", "s"), "svg.s");
        assert_eq!(confine("svg:hover", "s"), "svg.s:hover");
        assert_eq!(confine("svg .node", "s"), "svg.s .node");
        assert_eq!(confine("svg[data-x]", "s"), "svg.s[data-x]");
        assert_eq!(confine("svg.node", "s"), "svg.s.node");
        assert_eq!(confine("svg#id", "s"), "svg.s#id");
        // `svgfoo` is a different element, not the root with a suffix.
        assert_eq!(confine("svgfoo", "s"), "svg.s svgfoo");
        assert_eq!(confine(".node text", "s"), "svg.s .node text");
    }

    #[test]
    fn every_shape_writes_the_attributes_svg_expects_of_it() {
        // One shape emitting another's attributes draws nothing at all — the
        // element is valid markup and simply has no geometry, which is why this
        // is asserted per variant rather than left to whichever diagram happens
        // to use each one.
        let at = Point::new(1.0, 2.0);
        let size = Size {
            width: 3.0,
            height: 4.0,
        };
        let cases = [
            (
                Shape::Rect {
                    at,
                    size,
                    rx: 0.0,
                    ry: 0.0,
                },
                vec!["x=", "y=", "width=", "height="],
            ),
            (Shape::Circle { c: at, r: 5.0 }, vec!["cx=", "cy=", "r="]),
            (
                Shape::Ellipse {
                    c: at,
                    rx: 5.0,
                    ry: 6.0,
                },
                vec!["cx=", "cy=", "rx=", "ry="],
            ),
            (
                Shape::Line {
                    a: at,
                    b: Point::new(7.0, 8.0),
                },
                vec!["x1=", "y1=", "x2=", "y2="],
            ),
            (Shape::Polyline(vec![at]), vec!["points="]),
            (Shape::Polygon(vec![at]), vec!["points="]),
            (Shape::Path(vec![Seg::MoveTo(at)]), vec!["d="]),
        ];
        for (shape, wanted) in cases {
            let out = shape_attrs(&shape);
            for key in wanted {
                assert!(out.contains(key), "{shape:?} is missing {key}: {out}");
            }
        }
    }

    #[test]
    fn a_rounded_rect_says_so_and_a_square_one_stays_silent() {
        let rect = |rx: f64| {
            shape_attrs(&Shape::Rect {
                at: Point::new(0.0, 0.0),
                size: Size {
                    width: 1.0,
                    height: 1.0,
                },
                rx,
                ry: rx,
            })
        };
        assert!(rect(4.0).contains("rx="));
        assert!(
            !rect(0.0).contains("rx="),
            "a zero radius is not an attribute"
        );
    }

    #[test]
    fn both_kinds_of_transform_are_written() {
        let rotate = transform_attr(Transform::Rotate {
            deg: 90.0,
            about: Point::new(1.0, 2.0),
        });
        assert!(rotate.contains("rotate(90,1,2)"), "{rotate}");
        let scaled = transform_attr(Transform::TranslateScale {
            at: Point::new(3.0, 4.0),
            scale: 0.5,
        });
        assert!(scaled.contains("translate(3,4) scale(0.5)"), "{scaled}");
    }
    use crate::scene::{Layer, Role, Size};

    fn scene_of(nodes: Vec<Node>) -> Scene {
        let mut s = Scene::new(Size {
            width: 100.0,
            height: 50.0,
        });
        for n in nodes {
            s.push(n);
        }
        s
    }

    fn rect() -> Shape {
        Shape::Rect {
            at: Point::new(1.0, 2.0),
            size: Size {
                width: 30.0,
                height: 20.0,
            },
            rx: 4.0,
            ry: 0.0,
        }
    }

    #[test]
    fn writes_a_document_with_the_canvas_size() {
        let out = svg(&scene_of(vec![]));
        assert!(out.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(out.contains("width=\"100\" height=\"50\" viewBox=\"0 0 100 50\""));
        assert!(out.ends_with("</svg>"));
    }

    #[test]
    fn omits_empty_attributes_rather_than_writing_them_blank() {
        let out = svg(&scene_of(vec![Node::new(
            Role::Node,
            Content::Shape(rect()),
        )]));
        assert!(!out.contains("class=\"\""), "{out}");
        assert!(!out.contains("data-id"), "{out}");
        // ry is zero, so it should not appear at all.
        assert!(!out.contains("ry="), "{out}");
        assert!(out.contains("rx=\"4\""), "{out}");
    }

    #[test]
    fn a_token_colour_carries_its_fallback() {
        // Without the fallback, a diagram opened away from its page is unstyled.
        let node = Node::new(Role::Node, Content::Shape(rect())).painted(Paint {
            fill: Some(Color::Token {
                name: "surface".into(),
                fallback: "#f5f5f5".into(),
            }),
            ..Paint::default()
        });
        assert!(svg(&scene_of(vec![node])).contains("fill=\"var(--ags-surface, #f5f5f5)\""));
    }

    #[test]
    fn a_literal_colour_is_written_as_given() {
        let node = Node::new(Role::Node, Content::Shape(rect())).painted(Paint {
            fill: Some(Color::Literal("#0d5ba5".into())),
            stroke: Some(Color::None),
            ..Paint::default()
        });
        let out = svg(&scene_of(vec![node]));
        assert!(out.contains("fill=\"#0d5ba5\""), "{out}");
        assert!(out.contains("stroke=\"none\""), "{out}");
    }

    #[test]
    fn paths_are_written_from_segments() {
        let node = Node::new(
            Role::Edge,
            Content::Shape(Shape::Path(vec![
                Seg::MoveTo(Point::new(0.0, 0.0)),
                Seg::LineTo(Point::new(10.0, 0.0)),
                Seg::Arc {
                    r: Size {
                        width: 5.0,
                        height: 5.0,
                    },
                    large: false,
                    sweep: true,
                    to: Point::new(20.0, 10.0),
                },
                Seg::Close,
            ])),
        );
        let out = svg(&scene_of(vec![node]));
        assert!(out.contains("d=\"M0,0 L10,0 A5,5 0 0 1 20,10 Z\""), "{out}");
    }

    #[test]
    fn text_is_escaped_and_positioned() {
        let node = Node::new(
            Role::Label,
            Content::Text(TextRun {
                at: Point::new(5.0, 6.0),
                anchor: Anchor::Middle,
                font: Font {
                    size: 11.0,
                    weight: 600,
                    italic: true,
                },
                dy: Some("0.35em".into()),
                content: "a < b & c".into(),
            }),
        );
        let out = svg(&scene_of(vec![node]));
        assert!(out.contains("text-anchor=\"middle\""), "{out}");
        assert!(
            out.contains("font-size=\"11\" font-weight=\"600\""),
            "{out}"
        );
        assert!(out.contains("font-style=\"italic\""), "{out}");
        assert!(out.contains(">a &lt; b &amp; c</text>"), "{out}");
    }

    #[test]
    fn a_title_becomes_a_native_tooltip() {
        let node = Node::new(Role::Node, Content::Shape(rect())).titled("what this is");
        let out = svg(&scene_of(vec![node]));
        assert!(out.contains("<title>what this is</title>"), "{out}");
        // A shape with a title cannot self-close.
        assert!(out.contains("</rect>"), "{out}");
    }

    #[test]
    fn groups_nest_their_children() {
        let child = Node::new(Role::Node, Content::Shape(rect())).with_id("inner");
        let group = Node::new(Role::Frame, Content::Group(vec![child])).with_id("outer");
        let out = svg(&scene_of(vec![group]));
        assert!(out.contains("data-id=\"outer\""), "{out}");
        assert!(out.contains("data-id=\"inner\""), "{out}");
        assert!(out.contains("</g>"), "{out}");
    }

    #[test]
    fn output_follows_paint_order_not_scene_order() {
        // The z-order guarantee, visible in the emitted text.
        let overlay = Node::new(Role::Decoration, Content::Shape(rect()))
            .on(Layer::Overlay)
            .with_id("tip");
        let label = Node::new(Role::Label, Content::Shape(rect())).with_id("badge");
        let out = svg(&scene_of(vec![overlay, label]));
        let tip = out.find("\"tip\"").expect("tip missing");
        let badge = out.find("\"badge\"").expect("badge missing");
        assert!(badge < tip, "overlay must be written last:\n{out}");
    }

    #[test]
    fn rendering_the_same_scene_twice_is_identical() {
        let build = || {
            scene_of(vec![
                Node::new(Role::Node, Content::Shape(rect())).with_id("a"),
                Node::new(Role::Edge, Content::Shape(rect())).with_id("b"),
            ])
        };
        assert_eq!(svg(&build()), svg(&build()));
    }

    #[test]
    fn dashes_and_markers_are_written() {
        let node = Node::new(Role::Edge, Content::Shape(rect())).painted(Paint {
            dash: Some(vec![4.0, 2.0]),
            marker_end: Some("arrow".into()),
            stroke_width: Some(1.5),
            ..Paint::default()
        });
        let out = svg(&scene_of(vec![node]));
        assert!(out.contains("stroke-dasharray=\"4,2\""), "{out}");
        assert!(out.contains("marker-end=\"url(#arrow)\""), "{out}");
        assert!(out.contains("stroke-width=\"1.5\""), "{out}");
    }

    #[test]
    fn transforms_use_the_two_supported_forms() {
        let rotated = Node::new(Role::Label, Content::Shape(rect()));
        let mut rotated = rotated;
        rotated.transform = Some(Transform::Rotate {
            deg: -90.0,
            about: Point::new(3.0, 4.0),
        });
        assert!(svg(&scene_of(vec![rotated])).contains("transform=\"rotate(-90,3,4)\""));

        let mut scaled = Node::new(Role::Icon, Content::Shape(rect()));
        scaled.transform = Some(Transform::TranslateScale {
            at: Point::new(2.0, 3.0),
            scale: 0.5,
        });
        assert!(svg(&scene_of(vec![scaled])).contains("transform=\"translate(2,3) scale(0.5)\""));
    }

    #[test]
    fn every_rule_is_confined_to_the_drawing_that_declared_it() {
        let mut s = scene_of(vec![]);
        // The three shapes a selector comes in: the root itself, the root
        // qualified, and something inside it.
        s.style =
            "svg{--_text:#fff}svg:has(.step:hover) .badge{fill:red}.node path{fill:blue}".into();
        let out = svg(&s);
        let scope = out
            .split("class=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the root carries the scope");
        assert!(
            out.contains(&format!("svg.{scope}{{--_text:#fff}}")),
            "{out}"
        );
        assert!(
            out.contains(&format!("svg.{scope}:has(.step:hover) .badge{{")),
            "{out}"
        );
        assert!(out.contains(&format!("svg.{scope} .node path{{")), "{out}");
        // Nothing may be left that could match outside this drawing.
        let css = out.split("<style>").nth(1).unwrap_or("");
        assert!(!css.contains("}.node"), "an unscoped rule remains: {css}");
    }

    #[test]
    fn a_selector_list_is_confined_part_by_part() {
        let mut s = scene_of(vec![]);
        s.style = ".a rect,.b circle{fill:red}".into();
        let out = svg(&s);
        let scope = out
            .split("class=\"")
            .nth(1)
            .and_then(|r| r.split('"').next());
        let scope = scope.expect("a scope");
        assert!(
            out.contains(&format!("svg.{scope} .a rect,svg.{scope} .b circle{{")),
            "{out}"
        );
    }

    #[test]
    fn the_same_stylesheet_always_scopes_to_the_same_name() {
        // Byte-identical output for the same input is what the raster reference
        // and every comparison in the suite depend on.
        let mut one = scene_of(vec![]);
        one.style = ".x{fill:red}".into();
        let mut two = scene_of(vec![]);
        two.style = ".x{fill:red}".into();
        assert_eq!(svg(&one), svg(&two));

        let mut other = scene_of(vec![]);
        other.style = ".x{fill:blue}".into();
        assert_ne!(
            scope_of(".x{fill:red}"),
            scope_of(".x{fill:blue}"),
            "two stylesheets that differ must not share a scope"
        );
        assert!(!svg(&other).contains(&scope_of(".x{fill:red}")));
    }

    #[test]
    fn a_drawing_with_no_rules_carries_no_scope() {
        // Nothing to confine, and nothing left unscoped that could reach it.
        let out = svg(&scene_of(vec![]));
        assert!(!out.contains("class="), "{out}");
        assert!(!out.contains("<style>"), "{out}");
    }

    #[test]
    fn an_unclosed_rule_is_dropped_rather_than_emitted_unscoped() {
        // Not reachable from a generated stylesheet, but the alternative to
        // stopping is emitting a rule that would style the whole page.
        assert_eq!(scoped(".a{fill:red}.b{", "s"), "svg.s .a{fill:red}");
    }

    #[test]
    fn style_and_markers_are_emitted_when_present() {
        let mut s = scene_of(vec![]);
        s.style = ".x { fill: red }".into();
        s.markers.push(Marker {
            id: "arrow".into(),
            view: Size {
                width: 10.0,
                height: 10.0,
            },
            size: Size {
                width: 7.0,
                height: 7.0,
            },
            ref_x: 6.0,
            ref_y: 3.5,
            shape: Shape::Polygon(vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 5.0),
                Point::new(0.0, 10.0),
            ]),
            paint: Paint {
                fill: Some(Color::Token {
                    name: "arrow".into(),
                    fallback: "#888".into(),
                }),
                ..Paint::default()
            },
        });
        let out = svg(&s);
        // The rule survives, confined to this drawing's own root.
        assert!(out.contains(".x{ fill: red }</style>"), "{out}");
        assert!(out.contains("<marker id=\"arrow\""), "{out}");
        // The glyph is authored on a 10x10 grid and drawn at 7x7; without the
        // viewBox saying so it is clipped rather than scaled.
        assert!(out.contains("viewBox=\"0 0 10 10\""), "{out}");
        assert!(out.contains("markerWidth=\"7\""), "{out}");
        assert!(out.contains("points=\"0,0 10,5 0,10\""), "{out}");
        // An arrowhead is themed like anything else — it must not be the one
        // element that hard-codes a colour.
        assert!(out.contains("fill=\"var(--arrow, #888)\""), "{out}");
    }
}
