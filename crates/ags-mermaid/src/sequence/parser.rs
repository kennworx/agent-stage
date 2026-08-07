//! Reading `sequenceDiagram` source.
//!
//! ```text
//! sequenceDiagram
//!   participant A as Alice
//!   actor B as Bob
//!   A->>B: Solid          A-->>B: Dashed      A-)B: Open      A--x B: Cross
//!   A->>+B: activate B    B-->>-A: deactivate B
//!   loop Label … end      alt L … else L … end      par L … and L … end
//!   Note left of A: text  Note over A,B: text
//! ```
//!
//! Two things here are deliberately looser than they look. A block keyword needs
//! no word boundary — `loopy` opens a `loop` labelled `y` — and a block keyword
//! is tried before a message, so an actor named `alt` opens a block instead of
//! speaking. Both are what the reference does, and a diagram in the wild may
//! well be relying on the second by accident.
//!
//! Matching is hand-rolled rather than regex-driven, as everywhere else in this
//! crate — see `text.rs` for why.

use std::collections::HashSet;

use crate::text::normalize_label;

use super::types::{
    Actor, ActorKind, ArrowHead, Block, BlockKind, Diagram, Divider, LineStyle, Message, Note,
    NotePosition,
};

/// The positions a note may take, in the order the reference tries them.
const POSITIONS: [(&str, NotePosition); 3] = [
    ("left of", NotePosition::Left),
    ("right of", NotePosition::Right),
    ("over", NotePosition::Over),
];

fn is_space(c: char) -> bool {
    c.is_whitespace()
}

/// The text after `word` and the whitespace that must follow it.
///
/// `fold` matches the reference's `/i`: the note rule ignores case, the actor
/// and block rules do not.
fn after_word<'a>(line: &'a str, word: &str, fold: bool) -> Option<&'a str> {
    let head = line.get(..word.len())?;
    let matches = if fold {
        head.eq_ignore_ascii_case(word)
    } else {
        head == word
    };
    if !matches {
        return None;
    }
    let rest = line.get(word.len()..)?;
    rest.starts_with(is_space).then(|| rest.trim_start())
}

/// The text after `word` at the head of `line`, with no boundary demanded.
fn after_keyword<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    (line.get(..word.len())? == word).then(|| line.get(word.len()..).unwrap_or("").trim())
}

/// Split `text` at its first run of whitespace.
fn split_token(text: &str) -> (&str, &str) {
    match text.find(is_space) {
        Some(at) => {
            let (token, tail) = text.split_at(at);
            (token, tail.trim_start())
        }
        None => (text, ""),
    }
}

/// `<id>` or `<id> as <label>` — the tail of an actor declaration.
fn declaration(tail: &str) -> Option<(String, String)> {
    let (id, rest) = split_token(tail);
    if id.is_empty() {
        return None;
    }
    if rest.is_empty() {
        return Some((id.to_string(), id.to_string()));
    }
    let after = rest.strip_prefix("as")?;
    if !after.starts_with(is_space) {
        return None;
    }
    let label = after.trim();
    (!label.is_empty()).then(|| (id.to_string(), label.to_string()))
}

/// `participant A as Alice` / `actor B`.
fn actor_declaration(line: &str) -> Option<(ActorKind, String, String)> {
    for kind in [ActorKind::Participant, ActorKind::Actor] {
        let Some(tail) = after_word(line, kind.token(), false) else {
            continue;
        };
        let (id, label) = declaration(tail)?;
        return Some((kind, id, label));
    }
    None
}

/// `Note over A,B: text`.
fn note_declaration(line: &str) -> Option<(NotePosition, Vec<String>, String)> {
    let rest = after_word(line, "Note", true)?;
    let (position, rest) = POSITIONS
        .iter()
        .find_map(|(word, position)| after_word(rest, word, true).map(|tail| (*position, tail)))?;
    let (names, text) = rest.split_once(':')?;
    if names.is_empty() {
        return None;
    }
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let actors = names
        .trim()
        .split(',')
        .map(|name| name.trim().to_string())
        .collect();
    Some((position, actors, normalize_label(text)))
}

/// `loop Every 30s` — the block a line opens, and its label.
fn block_open(line: &str) -> Option<(BlockKind, String)> {
    BlockKind::ALL
        .into_iter()
        .find_map(|kind| after_keyword(line, kind.token()).map(|label| (kind, label.to_string())))
}

/// `else Invalid` / `and Fetch orders`.
fn block_divider(line: &str) -> Option<String> {
    ["else", "and"]
        .into_iter()
        .find_map(|word| after_keyword(line, word).map(str::to_string))
}

/// The arrow operator at the head of `text`, and what follows it.
///
/// `--?>?>` and `--?[)x]` between them spell every arrow the reference accepts:
/// one or two dashes, then one or two `>`, or a `)` or an `x`.
fn arrow_operator(text: &str) -> Option<(LineStyle, ArrowHead, &str)> {
    let rest = text.strip_prefix('-')?;
    let (dashed, rest) = rest
        .strip_prefix('-')
        .map_or((false, rest), |tail| (true, tail));
    let (head, rest) = match rest.strip_prefix('>') {
        // A second `>` fills the head in; a lone one leaves it open.
        Some(tail) => match tail.strip_prefix('>') {
            Some(tail) => (ArrowHead::Filled, tail),
            None => (ArrowHead::Open, tail),
        },
        None => match rest.strip_prefix('x') {
            Some(tail) => (ArrowHead::Filled, tail),
            None => (ArrowHead::Open, rest.strip_prefix(')')?),
        },
    };
    let style = if dashed {
        LineStyle::Dashed
    } else {
        LineStyle::Solid
    };
    Some((style, head, rest))
}

/// The longest run of non-space characters at the head of `text`.
fn non_space(text: &str) -> &str {
    text.get(..text.find(is_space).unwrap_or(text.len()))
        .unwrap_or(text)
}

/// The target of a message, and its label: `B: Hello`, or `+B: Hello`.
fn message_target(text: &str) -> Option<(String, bool, bool, String)> {
    let (activate, deactivate, rest) = match text.split_at_checked(1) {
        Some(("+", tail)) => (true, false, tail),
        Some(("-", tail)) => (false, true, tail),
        _ => (false, false, text),
    };
    let head = non_space(rest);
    // The reference's `(\S+?)` is lazy, so the target is the shortest prefix
    // that still leaves a colon and a label behind it.
    for end in 1..=head.len() {
        let Some(to) = head.get(..end) else {
            continue;
        };
        let Some(after) = rest.get(end..) else {
            continue;
        };
        let Some(label) = after.trim_start().strip_prefix(':') else {
            continue;
        };
        if label.is_empty() {
            continue;
        }
        return Some((
            to.to_string(),
            activate,
            deactivate,
            normalize_label(label.trim()),
        ));
    }
    None
}

/// `A->>+B: Request` — a whole message line.
fn message(line: &str) -> Option<Message> {
    let head = non_space(line);
    // The sender is lazy too: the shortest prefix an arrow can follow.
    for end in 1..=head.len() {
        let (Some(from), Some(rest)) = (head.get(..end), line.get(end..)) else {
            continue;
        };
        let Some((line_style, arrow_head, rest)) = arrow_operator(rest.trim_start()) else {
            continue;
        };
        let Some((to, activate, deactivate, label)) = message_target(rest.trim_start()) else {
            continue;
        };
        return Some(Message {
            from: from.to_string(),
            to,
            label,
            line_style,
            arrow_head,
            activate,
            deactivate,
        });
    }
    None
}

/// A block still being read.
struct Open {
    kind: BlockKind,
    label: String,
    start_index: usize,
    dividers: Vec<Divider>,
}

/// The reading state.
#[derive(Default)]
struct Reader {
    diagram: Diagram,
    ids: HashSet<String>,
    stack: Vec<Open>,
}

impl Reader {
    /// Infer an actor a message named but nobody declared.
    fn ensure(&mut self, id: &str) {
        if self.ids.contains(id) {
            return;
        }
        self.ids.insert(id.to_string());
        self.diagram.actors.push(Actor {
            id: id.to_string(),
            label: id.to_string(),
            kind: ActorKind::Participant,
        });
    }

    /// Declare an actor. A second declaration of the same id is dropped, label
    /// and all — the reference never revisits one it has already made.
    fn declare(&mut self, kind: ActorKind, id: &str, label: &str) {
        if self.ids.contains(id) {
            return;
        }
        self.ids.insert(id.to_string());
        self.diagram.actors.push(Actor {
            id: id.to_string(),
            label: normalize_label(label),
            kind,
        });
    }

    fn note(&mut self, position: NotePosition, actors: Vec<String>, text: String) {
        for actor in &actors {
            self.ensure(actor);
        }
        let after_index = i64::try_from(self.diagram.messages.len()).unwrap_or(i64::MAX) - 1;
        self.diagram.notes.push(Note {
            actors,
            text,
            position,
            after_index,
        });
    }

    fn open(&mut self, kind: BlockKind, label: &str) {
        self.stack.push(Open {
            kind,
            label: normalize_label(label),
            start_index: self.diagram.messages.len(),
            dividers: Vec::new(),
        });
    }

    /// Record an `else`/`and`. Answers whether there was a block to record it in.
    fn divide(&mut self, label: &str) -> bool {
        let index = self.diagram.messages.len();
        let Some(open) = self.stack.last_mut() else {
            return false;
        };
        open.dividers.push(Divider {
            index,
            label: normalize_label(label),
        });
        true
    }

    /// Close the innermost block. Answers whether there was one.
    fn close(&mut self) -> bool {
        let end = self.diagram.messages.len().saturating_sub(1);
        let Some(open) = self.stack.pop() else {
            return false;
        };
        self.diagram.blocks.push(Block {
            kind: open.kind,
            label: open.label,
            start_index: open.start_index,
            end_index: end.max(open.start_index),
            dividers: open.dividers,
        });
        true
    }

    fn message(&mut self, message: Message) {
        self.ensure(&message.from);
        self.ensure(&message.to);
        self.diagram.messages.push(message);
    }

    /// Read one line. Rules are tried in the reference's own order.
    fn line(&mut self, line: &str) {
        if let Some((kind, id, label)) = actor_declaration(line) {
            self.declare(kind, &id, &label);
            return;
        }
        if let Some((position, actors, text)) = note_declaration(line) {
            self.note(position, actors, text);
            return;
        }
        if let Some((kind, label)) = block_open(line) {
            self.open(kind, &label);
            return;
        }
        if let Some(label) = block_divider(line) {
            if self.divide(&label) {
                return;
            }
        }
        if line == "end" && self.close() {
            return;
        }
        if let Some(message) = message(line) {
            self.message(message);
        }
        // Anything else — a stray `activate`, a comment the preprocessor kept —
        // is not an error, it just draws nothing.
    }
}

/// Parse a sequence diagram source.
pub fn parse(source: &str) -> Diagram {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
        .collect();
    let mut reader = Reader::default();
    // The header line is skipped by position, not by pattern: whatever spelling
    // of `sequenceDiagram` got the source here, it is line one.
    for line in lines.iter().skip(1) {
        reader.line(line);
    }
    reader.diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(diagram: &Diagram) -> Vec<&str> {
        diagram
            .actors
            .iter()
            .map(|actor| actor.id.as_str())
            .collect()
    }

    #[test]
    fn a_declaration_names_and_labels_an_actor() {
        let out = parse("sequenceDiagram\nparticipant A as Alice\nactor B as Bob\nparticipant C");
        assert_eq!(ids(&out), ["A", "B", "C"]);
        assert_eq!(out.actors[0].label, "Alice");
        assert_eq!(out.actors[0].kind, ActorKind::Participant);
        assert_eq!(out.actors[1].kind, ActorKind::Actor);
        assert_eq!(out.actors[2].label, "C");
    }

    #[test]
    fn a_declaration_keyword_is_spelled_in_lower_case_only() {
        // The reference's actor rule carries no `/i`.
        let out = parse("sequenceDiagram\nParticipant A as Alice");
        assert!(out.actors.is_empty());
    }

    #[test]
    fn a_declaration_that_is_not_one_declares_nobody() {
        for line in [
            "participant",       // no name at all
            "participant   ",    // nor here
            "participant A as",  // an alias with nothing after it
            "participant A asx", // `as` has to be a word of its own
            "participant A B",   // two names and no `as` between them
        ] {
            let out = parse(&format!("sequenceDiagram\n{line}"));
            assert!(out.actors.is_empty(), "{line}");
        }
    }

    #[test]
    fn a_second_declaration_of_the_same_actor_is_dropped() {
        let out = parse("sequenceDiagram\nparticipant A as Alice\nparticipant A as Someone");
        assert_eq!(out.actors.len(), 1);
        assert_eq!(out.actors[0].label, "Alice");
    }

    #[test]
    fn an_actor_a_message_names_is_inferred() {
        let out = parse("sequenceDiagram\nAlice->>Bob: Hello");
        assert_eq!(ids(&out), ["Alice", "Bob"]);
        assert_eq!(out.actors[0].kind, ActorKind::Participant);
    }

    #[test]
    fn every_arrow_spelling_is_read() {
        let out = parse(
            "sequenceDiagram\nA->>B: one\nB-->>A: two\nA-)B: three\nB--)A: four\nA-xB: five\nB--xA: six\nA->B: seven\nB-->A: eight",
        );
        let read: Vec<(LineStyle, ArrowHead)> = out
            .messages
            .iter()
            .map(|m| (m.line_style, m.arrow_head))
            .collect();
        assert_eq!(
            read,
            [
                (LineStyle::Solid, ArrowHead::Filled),
                (LineStyle::Dashed, ArrowHead::Filled),
                (LineStyle::Solid, ArrowHead::Open),
                (LineStyle::Dashed, ArrowHead::Open),
                (LineStyle::Solid, ArrowHead::Filled),
                (LineStyle::Dashed, ArrowHead::Filled),
                (LineStyle::Solid, ArrowHead::Open),
                (LineStyle::Dashed, ArrowHead::Open),
            ]
        );
    }

    #[test]
    fn a_dash_in_a_name_does_not_start_the_arrow() {
        let out = parse("sequenceDiagram\nmy-actor->>other-one: hi");
        assert_eq!(ids(&out), ["my-actor", "other-one"]);
        assert_eq!(out.messages[0].label, "hi");
    }

    #[test]
    fn a_plus_activates_the_target_and_a_minus_deactivates_the_source() {
        let out = parse("sequenceDiagram\nC->>+S: Request\nS-->>-C: Response");
        assert!(out.messages[0].activate);
        assert!(!out.messages[0].deactivate);
        assert_eq!(out.messages[0].to, "S");
        assert!(out.messages[1].deactivate);
        assert_eq!(out.messages[1].to, "C");
    }

    #[test]
    fn a_line_that_is_not_a_message_draws_nothing() {
        for line in ["A->>B", "A->>B:", "A-B: hi", "->>B: hi", "activate A"] {
            let out = parse(&format!("sequenceDiagram\n{line}"));
            assert!(out.messages.is_empty(), "{line}");
        }
    }

    #[test]
    fn a_block_wraps_the_messages_between_its_keyword_and_its_end() {
        let out =
            parse("sequenceDiagram\nA->>B: x\nloop Every 30s\nA->>B: y\nB-->>A: z\nend\nA->>B: w");
        assert_eq!(out.blocks.len(), 1);
        let block = &out.blocks[0];
        assert_eq!(block.kind, BlockKind::Loop);
        assert_eq!(block.label, "Every 30s");
        assert_eq!((block.start_index, block.end_index), (1, 2));
        assert_eq!(out.messages.len(), 4);
    }

    #[test]
    fn every_block_keyword_opens_a_block() {
        for kind in BlockKind::ALL {
            let out = parse(&format!(
                "sequenceDiagram\n{} L\nA->>B: x\nend",
                kind.token()
            ));
            assert_eq!(out.blocks.first().map(|b| b.kind), Some(kind));
            assert_eq!(out.blocks[0].label, "L");
        }
    }

    #[test]
    fn a_block_keyword_needs_no_word_boundary() {
        // The reference's pattern has no `\b`, so `loopy` is a `loop` labelled
        // `y`. Kept because a diagram in the wild may be leaning on it.
        let out = parse("sequenceDiagram\nloopy\nA->>B: x\nend");
        assert_eq!(out.blocks[0].kind, BlockKind::Loop);
        assert_eq!(out.blocks[0].label, "y");
    }

    #[test]
    fn a_block_keyword_wins_over_a_message() {
        // An actor called `alt` opens a block instead of speaking.
        let out = parse("sequenceDiagram\nalt->>B: x");
        assert!(out.messages.is_empty());
        assert!(out.blocks.is_empty(), "opened, and never closed");
    }

    #[test]
    fn dividers_split_a_block_at_the_messages_that_follow_them() {
        let out = parse(
            "sequenceDiagram\nalt Valid\nS-->>C: 200\nelse Invalid\nS-->>C: 401\nelse Locked\nS-->>C: 403\nend",
        );
        let block = &out.blocks[0];
        assert_eq!(block.label, "Valid");
        let split: Vec<(usize, &str)> = block
            .dividers
            .iter()
            .map(|d| (d.index, d.label.as_str()))
            .collect();
        assert_eq!(split, [(1, "Invalid"), (2, "Locked")]);
    }

    #[test]
    fn an_and_divides_a_par_block() {
        let out = parse("sequenceDiagram\npar One\nA->>B: x\nand Two\nA->>C: y\nend");
        assert_eq!(out.blocks[0].kind, BlockKind::Par);
        assert_eq!(out.blocks[0].dividers[0].label, "Two");
    }

    #[test]
    fn a_divider_outside_a_block_is_not_one() {
        let out = parse("sequenceDiagram\nelse->>B: x");
        // No block is open, so the line falls through and reads as a message.
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].from, "else");
    }

    #[test]
    fn an_end_outside_a_block_closes_nothing() {
        let out = parse("sequenceDiagram\nend\nA->>B: x");
        assert!(out.blocks.is_empty());
        assert_eq!(out.messages.len(), 1);
    }

    #[test]
    fn nested_blocks_close_innermost_first() {
        let out = parse("sequenceDiagram\nloop L\nalt A\nX->>Y: m\nend\nend");
        let kinds: Vec<BlockKind> = out.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, [BlockKind::Alt, BlockKind::Loop]);
    }

    #[test]
    fn a_block_that_wrapped_nothing_keeps_a_range_of_one_row() {
        let out = parse("sequenceDiagram\nA->>B: x\nopt Nothing\nend");
        let block = &out.blocks[0];
        assert_eq!((block.start_index, block.end_index), (1, 1));
    }

    #[test]
    fn a_block_left_open_never_becomes_one() {
        let out = parse("sequenceDiagram\nloop L\nA->>B: x");
        assert!(out.blocks.is_empty());
        assert_eq!(out.messages.len(), 1);
    }

    #[test]
    fn every_note_position_is_read() {
        let out = parse(
            "sequenceDiagram\nA->>B: x\nNote left of A: one\nNote right of B: two\nNote over A,B: three",
        );
        let read: Vec<(NotePosition, &str)> = out
            .notes
            .iter()
            .map(|n| (n.position, n.text.as_str()))
            .collect();
        assert_eq!(
            read,
            [
                (NotePosition::Left, "one"),
                (NotePosition::Right, "two"),
                (NotePosition::Over, "three"),
            ]
        );
        assert_eq!(out.notes[2].actors, ["A", "B"]);
    }

    #[test]
    fn a_note_is_read_whatever_case_it_is_written_in() {
        let out = parse("sequenceDiagram\nA->>B: x\nnote OVER A: hi");
        assert_eq!(out.notes.len(), 1);
        assert_eq!(out.notes[0].position, NotePosition::Over);
    }

    #[test]
    fn a_note_names_an_actor_nobody_declared() {
        let out = parse("sequenceDiagram\nA->>B: x\nNote over C: hi");
        assert_eq!(ids(&out), ["A", "B", "C"]);
    }

    #[test]
    fn a_note_before_the_first_message_hangs_from_nothing() {
        // The reference records -1 and then never looks it up, so the note is
        // parsed but never drawn.
        let out = parse("sequenceDiagram\nNote left of A: early\nA->>B: x");
        assert_eq!(out.notes[0].after_index, -1);
    }

    #[test]
    fn a_note_with_no_text_is_not_a_note() {
        for line in ["Note over A:", "Note over A: ", "Note sideways of A: hi"] {
            let out = parse(&format!("sequenceDiagram\nX->>Y: m\n{line}"));
            assert!(out.notes.is_empty(), "{line}");
        }
    }

    #[test]
    fn a_source_of_nothing_parses_to_nothing() {
        assert_eq!(parse("sequenceDiagram"), Diagram::default());
        assert_eq!(parse(""), Diagram::default());
    }

    #[test]
    fn a_comment_and_a_blank_line_are_dropped_before_reading() {
        let out = parse("sequenceDiagram\n\n%% a note\n  A->>B: hi  \n");
        assert_eq!(out.messages.len(), 1);
        assert_eq!(out.messages[0].label, "hi");
    }

    #[test]
    fn a_self_message_names_the_same_actor_twice() {
        let out = parse("sequenceDiagram\nS->>S: Internal process");
        assert_eq!(ids(&out), ["S"]);
        assert_eq!(out.messages[0].from, out.messages[0].to);
    }
}
