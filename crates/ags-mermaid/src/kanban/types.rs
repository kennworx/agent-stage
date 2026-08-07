//! The parsed shape of a kanban board: columns of cards.

/// A parsed board.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Board {
    pub title: Option<String>,
    pub columns: Vec<Column>,
}

/// One column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Unique across the whole board, columns and cards alike.
    pub id: String,
    pub title: String,
    pub cards: Vec<Card>,
}

/// One card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub text: String,
    /// Whatever `@{…}` carried, in the order it was written. Held as pairs
    /// rather than named fields because the syntax admits any key, and only
    /// three of them are drawn.
    pub metadata: Vec<(String, String)>,
}

impl Card {
    /// The one line of metadata a card shows, if any.
    ///
    /// Only these three keys and only in this order: a card is small, and the
    /// syntax lets an author attach anything at all.
    pub fn meta_line(&self) -> Option<String> {
        let value = |key: &str| {
            self.metadata
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_empty())
        };
        let parts: Vec<String> = ["assigned", "ticket", "priority"]
            .into_iter()
            .filter_map(value)
            .collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}
