//! The parsed shape of a timeline: periods left to right, events beneath.

/// A parsed timeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timeline {
    pub title: Option<String>,
    /// Ordered sections. An unnamed one holds the periods declared before any
    /// `section` directive.
    pub sections: Vec<Section>,
}

/// A named grouping of periods, or the implicit one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    pub name: Option<String>,
    pub periods: Vec<Period>,
}

/// One time period, and what happened during it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    pub label: String,
    /// Events in source order, drawn top to bottom.
    pub events: Vec<String>,
}
