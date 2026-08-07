//! The parsed shape of a Venn diagram: sets, and the regions where they meet.

/// A parsed Venn diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    pub title: Option<String>,
    pub sets: Vec<Set>,
    pub unions: Vec<Union>,
}

/// One set: a circle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    /// How unions refer to it, and the drawn element's `data-id`.
    pub id: String,
    /// What is written in it; the id when no separate label was given.
    pub label: String,
}

/// One overlap region, naming the sets that form it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Union {
    /// The member ids joined by `∩`, made unique if that collides.
    pub id: String,
    pub set_ids: Vec<String>,
    /// Only an explicit label is drawn; there is no derived caption.
    pub label: Option<String>,
}
