//! The parsed shape of a `ZenUML` diagram: participants, messages, fragments.
//!
//! `ZenUML` writes a sequence diagram as code — `Service.method()` rather than
//! `A->B: method` — so a call and its return are two messages that the source
//! only spells once. The parser expands them, and everything downstream sees a
//! flat, ordered message list with control-flow blocks recorded as ranges over
//! it.

/// What a control-flow block is, which decides the tab drawn on its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind {
    Alt,
    Opt,
    Loop,
    Par,
    Try,
    Critical,
    Group,
}

impl FragmentKind {
    /// The word written on the box's tab.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Alt => "alt",
            Self::Opt => "opt",
            Self::Loop => "loop",
            Self::Par => "par",
            Self::Try => "try",
            Self::Critical => "critical",
            Self::Group => "group",
        }
    }

    /// The kind an opening keyword names.
    ///
    /// Total rather than fallible: every keyword that opens a block draws a box,
    /// and one nobody has a tab for is still a group.
    pub fn from_keyword(keyword: &str) -> Self {
        match keyword.to_ascii_lowercase().as_str() {
            "if" | "alt" => Self::Alt,
            "opt" => Self::Opt,
            "loop" | "while" | "for" | "foreach" => Self::Loop,
            "par" => Self::Par,
            "try" => Self::Try,
            "critical" => Self::Critical,
            _ => Self::Group,
        }
    }
}

/// A continuation of a fragment: `else`, `catch`, `finally`, `and`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Index into the diagram's messages of the first message this section owns.
    pub index: usize,
    pub keyword: String,
    pub label: String,
}

/// A control-flow block, as a range over the message list.
///
/// Held as a range rather than as a nested tree because the messages inside a
/// block still stack in source order with every other message — the block only
/// decides where a box is drawn around them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub kind: FragmentKind,
    pub label: String,
    /// First message inside the block.
    pub start_index: usize,
    /// One past the last message inside the block.
    pub end_index: usize,
    pub sections: Vec<Section>,
    /// 0 for the outermost block.
    pub depth: usize,
}

/// Someone the messages run between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub id: String,
    pub label: String,
    /// A stereotype written `@Actor User`, drawn above the name in guillemets.
    pub annotator: Option<String>,
}

/// Whether a message goes out or comes back.
///
/// A `ZenUML` source never spells an asynchronous call, so there is no variant for
/// one: every message the parser produces is a call or its return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Sync,
    Return,
}

impl MessageKind {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Return => "return",
        }
    }
}

/// How a message's line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
}

impl LineStyle {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Dashed => "dashed",
        }
    }
}

/// How a message's arrowhead is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowHead {
    Filled,
    Open,
}

/// One message, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: MessageKind,
    pub line_style: LineStyle,
    pub arrow_head: ArrowHead,
}

/// A parsed `ZenUML` diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    pub participants: Vec<Participant>,
    pub messages: Vec<Message>,
    pub fragments: Vec<Fragment>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_opening_keyword_maps_onto_a_kind() {
        for (keyword, want) in [
            ("if", FragmentKind::Alt),
            ("alt", FragmentKind::Alt),
            ("opt", FragmentKind::Opt),
            ("loop", FragmentKind::Loop),
            ("while", FragmentKind::Loop),
            ("for", FragmentKind::Loop),
            ("forEach", FragmentKind::Loop),
            ("par", FragmentKind::Par),
            ("try", FragmentKind::Try),
            ("critical", FragmentKind::Critical),
            ("group", FragmentKind::Group),
            ("section", FragmentKind::Group),
        ] {
            assert_eq!(FragmentKind::from_keyword(keyword), want, "{keyword}");
        }
    }

    #[test]
    fn a_keyword_nobody_has_a_tab_for_is_still_a_group() {
        assert_eq!(FragmentKind::from_keyword("whenever"), FragmentKind::Group);
    }

    #[test]
    fn a_kind_is_written_as_the_word_on_its_tab() {
        for kind in [
            FragmentKind::Alt,
            FragmentKind::Opt,
            FragmentKind::Loop,
            FragmentKind::Par,
            FragmentKind::Try,
            FragmentKind::Critical,
            FragmentKind::Group,
        ] {
            assert!(!kind.token().is_empty());
            assert_eq!(FragmentKind::from_keyword(kind.token()), kind);
        }
    }

    #[test]
    fn a_message_carries_its_flavour_and_line_as_words() {
        assert_eq!(MessageKind::Sync.token(), "sync");
        assert_eq!(MessageKind::Return.token(), "return");
        assert_eq!(LineStyle::Solid.token(), "solid");
        assert_eq!(LineStyle::Dashed.token(), "dashed");
        assert_ne!(ArrowHead::Filled, ArrowHead::Open);
    }
}
