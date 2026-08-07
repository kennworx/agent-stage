//! The parsed shape of a packet diagram: a bit-field map.

/// A parsed packet diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    pub title: Option<String>,
    pub fields: Vec<Field>,
}

/// One field, occupying the inclusive bit range `start..=end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub start: usize,
    pub end: usize,
    pub label: String,
}
