//! The parsed shape of a sequence diagram: actors, messages, blocks, notes.

/// How an actor is drawn: a box, or a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Participant,
    Actor,
}

impl ActorKind {
    /// The keyword this kind is declared with, which becomes its `data-type`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Participant => "participant",
            Self::Actor => "actor",
        }
    }
}

/// Someone the messages run between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub id: String,
    pub label: String,
    pub kind: ActorKind,
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

impl ArrowHead {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Filled => "filled",
            Self::Open => "open",
        }
    }
}

/// One message, in chronological order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub label: String,
    pub line_style: LineStyle,
    pub arrow_head: ArrowHead,
    /// `+` on the target: open an activation on its lifeline.
    pub activate: bool,
    /// `-` on the target: close the source's innermost activation.
    pub deactivate: bool,
}

/// What a structural block is, which decides the word on its tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
    Rect,
}

impl BlockKind {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Alt => "alt",
            Self::Opt => "opt",
            Self::Par => "par",
            Self::Critical => "critical",
            Self::Break => "break",
            Self::Rect => "rect",
        }
    }

    /// Every keyword that opens a block, in the order the reference tries them.
    pub const ALL: [Self; 7] = [
        Self::Loop,
        Self::Alt,
        Self::Opt,
        Self::Par,
        Self::Critical,
        Self::Break,
        Self::Rect,
    ];
}

/// An `else` or `and` inside a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divider {
    /// Index of the first message on this side of the divider.
    pub index: usize,
    pub label: String,
}

/// A structural block, as an inclusive range over the message list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub label: String,
    pub start_index: usize,
    /// The last message inside the block — inclusive, unlike the usual half-open
    /// convention, because the reference's box is drawn from row to row.
    pub end_index: usize,
    pub dividers: Vec<Divider>,
}

/// Where a note sits relative to the actors it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePosition {
    Left,
    Right,
    Over,
}

impl NotePosition {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Over => "over",
        }
    }
}

/// A note pinned to one or more lifelines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub actors: Vec<String>,
    pub text: String,
    pub position: NotePosition,
    /// The message this note follows, or `-1` when it came before any of them.
    ///
    /// Signed on purpose: a note written above the first message has nothing to
    /// hang from, and the reference drops it rather than floating it. Folding
    /// that into `0` would silently start drawing notes it never drew.
    pub after_index: i64,
}

/// A parsed sequence diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    pub actors: Vec<Actor>,
    pub messages: Vec<Message>,
    pub blocks: Vec<Block>,
    pub notes: Vec<Note>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_is_written_as_the_keyword_it_was_declared_with() {
        assert_eq!(ActorKind::Participant.token(), "participant");
        assert_eq!(ActorKind::Actor.token(), "actor");
    }

    #[test]
    fn every_block_keyword_has_a_word_on_its_tab() {
        assert_eq!(BlockKind::ALL.len(), 7);
        for kind in BlockKind::ALL {
            assert!(!kind.token().is_empty());
        }
        let tokens: Vec<&str> = BlockKind::ALL.iter().map(|k| k.token()).collect();
        assert_eq!(
            tokens,
            ["loop", "alt", "opt", "par", "critical", "break", "rect"]
        );
    }

    #[test]
    fn a_line_an_arrowhead_and_a_position_are_all_written_as_words() {
        assert_eq!(LineStyle::Solid.token(), "solid");
        assert_eq!(LineStyle::Dashed.token(), "dashed");
        assert_eq!(ArrowHead::Filled.token(), "filled");
        assert_eq!(ArrowHead::Open.token(), "open");
        assert_eq!(NotePosition::Left.token(), "left");
        assert_eq!(NotePosition::Right.token(), "right");
        assert_eq!(NotePosition::Over.token(), "over");
    }
}
