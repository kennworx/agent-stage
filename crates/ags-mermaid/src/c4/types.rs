//! The parsed shape of a C4 diagram.
//!
//! A set of element boxes connected by labelled relationships, optionally
//! grouped inside boundaries that may nest.

/// The kind of element box, which drives how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    Person,
    System,
    Container,
    Component,
}

/// Storage shape carried by the `*Db` and `*Queue` forms.
///
/// These change the glyph, not the kind: a `ContainerDb` is still a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Db,
    Queue,
}

/// A direction hint from `Rel_U` and friends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelDirection {
    Up,
    Down,
    Left,
    Right,
}

/// What kind of container a boundary is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    Enterprise,
    System,
    Container,
    Deployment,
}

/// Layout hints from `UpdateLayoutConfig`.
///
/// The author stating how wide the diagram should be. Honouring it matters
/// beyond taste: a narrower row shortens every edge, which is what keeps
/// relationship labels from piling onto one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConfig {
    pub shape_in_row: usize,
    pub boundary_in_row: usize,
}

impl Default for LayoutConfig {
    /// Mermaid's own defaults, used unless the source says otherwise.
    fn default() -> Self {
        Self {
            shape_in_row: 4,
            boundary_in_row: 2,
        }
    }
}

/// One element box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Identifier from the source, and the identity feedback is keyed to.
    pub alias: String,
    pub kind: ElementKind,
    pub variant: Option<Variant>,
    pub label: String,
    pub techn: Option<String>,
    pub descr: Option<String>,
    pub external: bool,
    /// The boundary enclosing this element, if any.
    pub boundary: Option<String>,
}

/// One relationship arrow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub label: String,
    pub techn: Option<String>,
    pub direction: Option<RelDirection>,
    pub bidirectional: bool,
}

/// One boundary group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    pub alias: String,
    pub label: String,
    pub kind: BoundaryKind,
    /// The boundary enclosing this one.
    ///
    /// Deployment diagrams nest nodes freely — a workstation holding a shell
    /// session holding a process — and without this link an outer node that
    /// contains only other nodes has no members of its own and disappears.
    pub parent: Option<String>,
}

/// A parsed C4 diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    pub title: Option<String>,
    pub elements: Vec<Element>,
    pub relationships: Vec<Relationship>,
    pub boundaries: Vec<Boundary>,
    pub config: LayoutConfig,
}
