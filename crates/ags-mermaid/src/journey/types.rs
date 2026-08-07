//! The parsed shape of a user journey: sections of scored tasks.

/// A parsed journey.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Journey {
    pub title: Option<String>,
    pub sections: Vec<Section>,
}

/// One group of tasks. The name is empty for the implicit section that catches
/// tasks written before any `section` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub tasks: Vec<Task>,
}

/// One step of the journey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub name: String,
    /// Satisfaction, 1 to 5.
    pub score: i32,
    pub actors: Vec<String>,
}
