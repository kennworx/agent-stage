//! The scene: what a diagram layout produces and the emitter draws.
//!
//! Every diagram type lays out into this one structure, so there is a single SVG
//! writer rather than one per type, and a single set of constraint checks rather
//! than twenty-seven. The shapes and attributes here are exactly those the
//! existing renderers use across all types — no gradients, filters, clip paths,
//! masks or images, and `transform` in only two forms.
//!
//! Two properties matter more than the rest.
//!
//! **Paths are segments, not a `d` string.** An opaque path attribute would force
//! the constraint stage to re-parse what layout had just computed, which is how a
//! diagram with twenty-three edge crossings was reported as having four.
//!
//! **Role and layer are separate.** Role says what a thing is, and constraints
//! select on it; layer says where it paints. Folding them together breaks as soon
//! as an icon needs to paint at the node layer without being a box to the
//! checker.

/// Canvas extent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// A point in diagram coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// What an element *is*. Constraints select on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A boundary or grouping container.
    Frame,
    /// Primary content: a box, a bar, a wedge.
    Node,
    /// A connector between nodes.
    Edge,
    /// Text naming something.
    Label,
    /// A glyph inside a node — paints with nodes, but is not one.
    Icon,
    /// Anything else drawn for effect rather than meaning.
    Decoration,
}

/// Where an element *paints*. Later layers cover earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Frame,
    Edge,
    Node,
    Label,
    Overlay,
}

impl Role {
    /// The layer this role paints on unless told otherwise.
    ///
    /// Most emitters never set a layer: boundaries belong behind everything,
    /// wires behind the boxes they connect, labels above both. `Overlay` is
    /// deliberately not reachable from any role — something claiming to sit above
    /// everything should have said so on purpose.
    pub const fn layer(self) -> Layer {
        match self {
            Self::Frame => Layer::Frame,
            Self::Edge => Layer::Edge,
            Self::Node | Self::Icon | Self::Decoration => Layer::Node,
            Self::Label => Layer::Label,
        }
    }
}

/// A colour, as the renderer expresses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    /// A theme token with a literal fallback: `var(--name, #rrggbb)`.
    ///
    /// The fallback is not optional. Without it a diagram opened away from the
    /// page that defines the token renders unstyled rather than merely off-theme.
    Token { name: String, fallback: String },
    /// A literal, for output that has no cascade to read from.
    Literal(String),
    /// Explicitly nothing — `fill="none"` on a stroked path.
    None,
}

/// How a shape is painted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paint {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: Option<f64>,
    pub dash: Option<Vec<f64>>,
    pub marker_start: Option<String>,
    pub marker_end: Option<String>,
}

/// One straight or curved run of a path.
#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    MoveTo(Point),
    LineTo(Point),
    Quad {
        ctrl: Point,
        to: Point,
    },
    Cubic {
        c1: Point,
        c2: Point,
        to: Point,
    },
    Arc {
        r: Size,
        large: bool,
        sweep: bool,
        to: Point,
    },
    Close,
}

/// A drawable outline.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rect {
        at: Point,
        size: Size,
        rx: f64,
        ry: f64,
    },
    Circle {
        c: Point,
        r: f64,
    },
    Ellipse {
        c: Point,
        rx: f64,
        ry: f64,
    },
    Line {
        a: Point,
        b: Point,
    },
    Polyline(Vec<Point>),
    Polygon(Vec<Point>),
    Path(Vec<Seg>),
}

/// Horizontal alignment of a text run about its anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// Text styling.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub size: f64,
    pub weight: u32,
    pub italic: bool,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            size: 13.0,
            weight: 400,
            italic: false,
        }
    }
}

/// A run of text at a point.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub at: Point,
    pub anchor: Anchor,
    pub font: Font,
    /// Baseline offset, as an SVG `dy` value.
    pub dy: Option<String>,
    pub content: String,
}

/// The `transform` forms the diagram types actually use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transform {
    Rotate {
        deg: f64,
        about: Point,
    },
    /// A shift with no resizing — a drop shadow offset from what casts it.
    /// Distinct from `TranslateScale` with a scale of one, which would write a
    /// `scale(1)` that means nothing.
    Translate {
        by: Point,
    },
    TranslateScale {
        at: Point,
        scale: f64,
    },
}

/// What a node draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Group(Vec<Node>),
    Shape(Shape),
    Text(TextRun),
}

/// One element of a scene.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub role: Role,
    pub layer: Layer,
    /// Stable identity, so feedback can be keyed to this element.
    pub id: Option<String>,
    /// Datum carried for the reader, e.g. a pie slice's value.
    pub value: Option<String>,
    /// Further `data-` attributes, for the cross-references a drawing needs to
    /// make in CSS.
    ///
    /// A C4 badge and its legend row highlight together on hover, which no
    /// script arranges — the rule is `:has()` keyed on a step attribute. Held as
    /// pairs rather than as named fields because the key differs per diagram
    /// type, and a field per type would put twenty-seven of them on every node.
    pub data: Vec<(String, String)>,
    pub class: Vec<String>,
    pub paint: Paint,
    pub transform: Option<Transform>,
    /// Native tooltip text.
    pub title: Option<String>,
    pub content: Content,
}

impl Node {
    /// A node in its role's default layer.
    pub fn new(role: Role, content: Content) -> Self {
        Self {
            role,
            layer: role.layer(),
            id: None,
            value: None,
            data: Vec::new(),
            class: Vec::new(),
            paint: Paint::default(),
            transform: None,
            title: None,
            content,
        }
    }

    /// Paint this node on a different layer than its role implies.
    #[must_use]
    pub const fn on(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }

    /// Give the node a stable identity for feedback.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Carry a datum for the reader — a slice's number, a bar's height.
    #[must_use]
    pub fn valued(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Attach a `data-` attribute.
    #[must_use]
    pub fn tagged(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.push((key.into(), value.into()));
        self
    }

    /// Attach a CSS class.
    #[must_use]
    pub fn classed(mut self, class: impl Into<String>) -> Self {
        self.class.push(class.into());
        self
    }

    /// Set how the node is painted.
    #[must_use]
    pub fn painted(mut self, paint: Paint) -> Self {
        self.paint = paint;
        self
    }

    /// Attach hover text.
    #[must_use]
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// An arrowhead definition, the only `<defs>` content any type emits.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub id: String,
    /// The coordinate system the outline is drawn in: `viewBox="0 0 w h"`.
    ///
    /// Separate from `size` because they answer different questions — how big
    /// the drawing is, and how big it appears on the line. Without it a glyph
    /// authored on a 10×10 grid is clipped to the first few pixels.
    pub view: Size,
    pub size: Size,
    pub ref_x: f64,
    pub ref_y: f64,
    /// The arrowhead outline, in marker coordinates.
    pub shape: Shape,
    pub paint: Paint,
}

/// A laid-out diagram, ready to draw or to check.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub canvas: Size,
    pub markers: Vec<Marker>,
    /// CSS emitted with the diagram: token definitions and class rules.
    pub style: String,
    /// How a colour is written for the target this scene is drawn for — as a
    /// token reference a page can restyle, or as the literal it resolves to.
    ///
    /// Carried on the scene rather than passed to the emitter so that a scene is
    /// self-describing: whatever produced it already knew which it wanted, and
    /// nothing downstream has to be told again.
    pub colors: crate::theme::Colors,
    pub nodes: Vec<Node>,
}

impl Scene {
    pub fn new(canvas: Size) -> Self {
        Self {
            canvas,
            markers: Vec::new(),
            style: String::new(),
            colors: crate::theme::Colors::new(
                &crate::theme::Theme::default(),
                &crate::api::ColorMode::Tokens,
            ),
            nodes: Vec::new(),
        }
    }

    /// Add a node.
    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// The nodes in paint order: by layer, and within a layer by scene order.
    ///
    /// The sort is stable, which is not a detail — an unstable one would let the
    /// same source render differently between runs, breaking review diffs,
    /// caching, and every port-verification diff.
    pub fn painted(&self) -> Vec<&Node> {
        let mut order: Vec<&Node> = self.nodes.iter().collect();
        order.sort_by_key(|n| n.layer);
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(role: Role) -> Node {
        Node::new(
            role,
            Content::Shape(Shape::Circle {
                c: Point::new(0.0, 0.0),
                r: 1.0,
            }),
        )
    }

    #[test]
    fn an_unstated_font_is_the_body_face_upright() {
        // Every renderer states its own size and weight, so this is only reached
        // by a caller that does not care — which is exactly when a surprising
        // default would be hardest to notice.
        let font = Font::default();
        assert!((font.size - 13.0).abs() < f64::EPSILON);
        assert_eq!(font.weight, 400);
        assert!(!font.italic);
    }

    #[test]
    fn roles_map_onto_their_default_layers() {
        assert_eq!(Role::Frame.layer(), Layer::Frame);
        assert_eq!(Role::Edge.layer(), Layer::Edge);
        assert_eq!(Role::Node.layer(), Layer::Node);
        assert_eq!(Role::Label.layer(), Layer::Label);
    }

    #[test]
    fn an_icon_paints_with_nodes_without_being_one() {
        // The reason role and layer are separate fields: constraints that select
        // Role::Node must not pick up icons drawn inside a box.
        assert_eq!(Role::Icon.layer(), Layer::Node);
        assert_ne!(Role::Icon, Role::Node);
    }

    #[test]
    fn no_role_defaults_to_the_overlay_layer() {
        // Sitting above everything is a claim that should be made deliberately.
        for role in [
            Role::Frame,
            Role::Node,
            Role::Edge,
            Role::Label,
            Role::Icon,
            Role::Decoration,
        ] {
            assert_ne!(
                role.layer(),
                Layer::Overlay,
                "{role:?} defaulted to overlay"
            );
        }
    }

    #[test]
    fn paint_order_follows_the_layer_stack() {
        let mut scene = Scene::new(Size {
            width: 10.0,
            height: 10.0,
        });
        scene.push(dot(Role::Label));
        scene.push(dot(Role::Frame));
        scene.push(dot(Role::Node));
        scene.push(dot(Role::Edge));
        let layers: Vec<Layer> = scene.painted().iter().map(|n| n.layer).collect();
        assert_eq!(
            layers,
            vec![Layer::Frame, Layer::Edge, Layer::Node, Layer::Label]
        );
    }

    #[test]
    fn an_overlay_paints_above_content_added_after_it() {
        // The bug this exists to prevent: a tooltip emitted early was covered by
        // every badge drawn later, because SVG paints in document order.
        let mut scene = Scene::new(Size {
            width: 10.0,
            height: 10.0,
        });
        scene.push(dot(Role::Decoration).on(Layer::Overlay).with_id("tip"));
        scene.push(dot(Role::Label).with_id("badge"));
        let ids: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert_eq!(ids, vec!["badge", "tip"]);
    }

    #[test]
    fn order_within_a_layer_follows_the_scene() {
        // Stability is the contract: same input, same drawing, every time.
        let mut scene = Scene::new(Size {
            width: 10.0,
            height: 10.0,
        });
        for id in ["a", "b", "c", "d"] {
            scene.push(dot(Role::Node).with_id(id));
        }
        let ids: Vec<&str> = scene
            .painted()
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn builders_leave_the_defaults_alone() {
        let node = dot(Role::Node)
            .with_id("x")
            .classed("box")
            .titled("hi")
            .valued("7");
        assert_eq!(node.id.as_deref(), Some("x"));
        assert_eq!(node.value.as_deref(), Some("7"));
        assert_eq!(node.class, vec!["box"]);
        assert_eq!(node.title.as_deref(), Some("hi"));
        assert_eq!(node.layer, Layer::Node);
        assert!(node.transform.is_none());
    }
}
