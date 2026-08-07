//! The parsed shape of a Wardley map: components in a value-chain plane.

/// What a placed thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Component,
    /// A user or customer — the top of the value chain, drawn filled.
    Anchor,
}

/// How a dependency is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    #[default]
    Solid,
    Dashed,
    /// A flow of value rather than a dependency, drawn in the accent.
    Flow,
}

/// A parsed Wardley map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map {
    pub title: Option<String>,
    pub components: Vec<Component>,
    pub links: Vec<Link>,
}

/// One component, in the unit plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// Both the display name and how links refer to it.
    pub name: String,
    /// Position in the value chain: 0 invisible, 1 visible to the user.
    pub visibility: f64,
    /// Evolution stage: 0 Genesis, 1 Commodity.
    pub evolution: f64,
    pub kind: Kind,
}

/// One dependency between two components, named as they were written.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: Style,
}
