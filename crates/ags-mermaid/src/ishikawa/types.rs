//! The parsed shape of a fishbone: an effect, and what contributes to it.

/// A parsed Ishikawa diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    /// What is being explained — the fish's head.
    pub effect: String,
    /// The major bones.
    pub categories: Vec<Cause>,
}

/// One contributing cause, which may itself have contributing causes.
///
/// A category and a cause differ only in where they sit, so one type serves
/// both: the top level of the tree is the categories, everything below is
/// causes, and nesting is unbounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cause {
    pub text: String,
    pub causes: Vec<Cause>,
}
