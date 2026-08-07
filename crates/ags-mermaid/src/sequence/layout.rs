//! Where the lifelines, arrows, activation bars, blocks and notes sit.
//!
//! A timeline: actors space out along the top, and every message takes the next
//! row down. Three things push rows further apart than the plain step —
//! a block header, a divider caption, and a run of notes — and one thing moves
//! the whole drawing sideways: a note written to the left of the first actor
//! sits at a negative x until everything is shifted right to make room for it.

use std::collections::HashMap;

use crate::metrics::text_width;
use crate::round::count;
use crate::scene::Point;

use super::types::{
    ActorKind, ArrowHead, Block, BlockKind, Diagram, LineStyle, Note, NotePosition,
};

use super::metrics::{
    divider_label, note_width, ACTIVATION_WIDTH, ACTOR_GAP, ACTOR_HEIGHT, ACTOR_PAD_X,
    BLOCK_HEADER_EXTRA, BLOCK_PAD_BOTTOM, BLOCK_PAD_TOP, BLOCK_PAD_X, DIVIDER_EXTRA,
    DIVIDER_OFFSET, DIVIDER_OFFSET_CLEAR, EDGE_FONT, EDGE_WEIGHT, HEADER_GAP, LABEL_FONT,
    LABEL_WEIGHT, MESSAGE_ROW, MIN_ACTOR_WIDTH, MIN_HEIGHT, MIN_WIDTH, NESTING_OFFSET, NOTE_DROP,
    NOTE_GAP, NOTE_PAD_Y, NOTE_STACK_GAP, PADDING, SELF_LABEL_PAD, SELF_LABEL_PROBE,
    SELF_LOOP_WIDTH, SELF_MESSAGE_HEIGHT,
};

/// One actor's box, placed about its centre.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedActor {
    pub id: String,
    pub label: String,
    pub kind: ActorKind,
    /// The centre of the box, which is also the lifeline's x.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The dashed rule running down from an actor.
#[derive(Debug, Clone, PartialEq)]
pub struct Lifeline {
    pub actor: String,
    pub x: f64,
    pub top: f64,
    pub bottom: f64,
}

/// One message, placed on its row.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedMessage {
    pub from: String,
    pub to: String,
    pub label: String,
    pub line_style: LineStyle,
    pub arrow_head: ArrowHead,
    pub x1: f64,
    pub x2: f64,
    pub y: f64,
    pub self_call: bool,
}

/// A bar on a lifeline, for as long as that actor is busy.
#[derive(Debug, Clone, PartialEq)]
pub struct Activation {
    pub actor: String,
    pub x: f64,
    pub top: f64,
    pub bottom: f64,
    pub width: f64,
}

/// A rule across a block, and the caption under it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedDivider {
    pub y: f64,
    pub label: String,
}

/// One structural block.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedBlock {
    pub kind: BlockKind,
    pub label: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub dividers: Vec<PlacedDivider>,
}

/// One note.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNote {
    pub text: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub actors: Vec<String>,
    pub position: NotePosition,
}

/// A laid-out sequence diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub actors: Vec<PlacedActor>,
    pub lifelines: Vec<Lifeline>,
    pub messages: Vec<PlacedMessage>,
    pub activations: Vec<Activation>,
    pub blocks: Vec<PlacedBlock>,
    pub notes: Vec<PlacedNote>,
}

/// Each actor box's width: enough for its name.
fn actor_widths(diagram: &Diagram) -> Vec<f64> {
    diagram
        .actors
        .iter()
        .map(|actor| {
            (text_width(&actor.label, LABEL_FONT, LABEL_WEIGHT) + ACTOR_PAD_X * 2.0)
                .max(MIN_ACTOR_WIDTH)
        })
        .collect()
}

/// Where each lifeline stands: a fixed gap apart, widened when two neighbouring
/// boxes would otherwise touch.
fn centres(widths: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(widths.len());
    let mut cursor = PADDING + widths.first().copied().unwrap_or(0.0) / 2.0;
    for (index, width) in widths.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).and_then(|i| widths.get(i)) {
            cursor += ACTOR_GAP.max((previous + width) / 2.0 + 40.0);
        }
        out.push(cursor);
    }
    out
}

/// How much room a message needs above it, over and above the plain step.
fn extra_before(diagram: &Diagram) -> HashMap<usize, f64> {
    let mut out: HashMap<usize, f64> = HashMap::new();
    for block in &diagram.blocks {
        let slot = out.entry(block.start_index).or_insert(0.0);
        *slot = slot.max(BLOCK_HEADER_EXTRA);
        for divider in &block.dividers {
            let slot = out.entry(divider.index).or_insert(0.0);
            *slot = slot.max(DIVIDER_EXTRA);
        }
    }
    out
}

/// The horizontal frame: how wide each actor is, and where each one stands.
struct Columns<'a> {
    diagram: &'a Diagram,
    widths: Vec<f64>,
    centres: Vec<f64>,
    index: HashMap<&'a str, usize>,
}

impl<'a> Columns<'a> {
    fn of(diagram: &'a Diagram) -> Self {
        let widths = actor_widths(diagram);
        let centres = centres(&widths);
        let index = diagram
            .actors
            .iter()
            .enumerate()
            .map(|(at, actor)| (actor.id.as_str(), at))
            .collect();
        Self {
            diagram,
            widths,
            centres,
            index,
        }
    }

    /// An actor nobody declared falls on the first lifeline, which is where the
    /// reference puts one too.
    fn at(&self, id: &str) -> usize {
        self.index.get(id).copied().unwrap_or(0)
    }

    fn centre(&self, id: &str) -> f64 {
        self.centres.get(self.at(id)).copied().unwrap_or(0.0)
    }

    fn width_at(&self, at: usize) -> f64 {
        self.widths.get(at).copied().unwrap_or(0.0)
    }
}

/// One actor's open activations, innermost last.
struct Bars {
    actor: String,
    open: Vec<(f64, usize)>,
}

/// The vertical pass.
struct Rows {
    messages: Vec<PlacedMessage>,
    activations: Vec<Activation>,
    notes: Vec<PlacedNote>,
    /// Where the last row left the cursor.
    bottom: f64,
}

/// Where `actor`'s bars are held, adding it if this is its first activation.
///
/// Kept as a list rather than a map because the bars nobody closed are flushed
/// in the order their actors were first activated, and a hash map would make
/// that order arbitrary.
fn bars_for(bars: &mut Vec<Bars>, actor: &str) -> usize {
    if let Some(at) = bars.iter().position(|entry| entry.actor == actor) {
        return at;
    }
    bars.push(Bars {
        actor: actor.to_string(),
        open: Vec::new(),
    });
    bars.len() - 1
}

/// Where a note sits across the page, given the actors it names.
fn note_x(columns: &Columns, note: &Note, width: f64) -> f64 {
    let first = columns.at(note.actors.first().map_or("", String::as_str));
    let centre = columns.centres.get(first).copied().unwrap_or(0.0);
    match note.position {
        NotePosition::Left => centre - columns.width_at(first) / 2.0 - width - NOTE_GAP,
        NotePosition::Right => centre + columns.width_at(first) / 2.0 + NOTE_GAP,
        NotePosition::Over => {
            let last = note
                .actors
                .last()
                .filter(|_| note.actors.len() > 1)
                .map_or(first, |id| columns.at(id));
            let far = columns.centres.get(last).copied().unwrap_or(centre);
            f64::midpoint(centre, far) - width / 2.0
        }
    }
}

fn place_notes(
    columns: &Columns,
    at: usize,
    message: &PlacedMessage,
    out: &mut Vec<PlacedNote>,
) -> f64 {
    // A self-message's loop hangs below its own row; a straight arrow does not.
    let drop = if message.self_call {
        SELF_MESSAGE_HEIGHT
    } else {
        0.0
    };
    let mut y = message.y + drop + NOTE_DROP;
    let height = EDGE_FONT + NOTE_PAD_Y * 2.0;
    for note in columns
        .diagram
        .notes
        .iter()
        .filter(|note| note.after_index == i64::try_from(at).unwrap_or(i64::MAX))
    {
        let width = note_width(&note.text);
        out.push(PlacedNote {
            text: note.text.clone(),
            at: Point::new(note_x(columns, note, width), y),
            width,
            height,
            actors: note.actors.clone(),
            position: note.position,
        });
        y += height + NOTE_STACK_GAP;
    }
    y
}

fn stack(columns: &Columns) -> Rows {
    let extra = extra_before(columns.diagram);
    let mut y = PADDING + ACTOR_HEIGHT + HEADER_GAP;
    let mut messages: Vec<PlacedMessage> = Vec::with_capacity(columns.diagram.messages.len());
    let mut activations = Vec::new();
    let mut notes = Vec::new();
    let mut bars: Vec<Bars> = Vec::new();

    for (at, message) in columns.diagram.messages.iter().enumerate() {
        y += extra.get(&at).copied().unwrap_or(0.0);
        let self_call = message.from == message.to;
        let placed = PlacedMessage {
            from: message.from.clone(),
            to: message.to.clone(),
            label: message.label.clone(),
            line_style: message.line_style,
            arrow_head: message.arrow_head,
            x1: columns.centre(&message.from),
            x2: columns.centre(&message.to),
            y,
            self_call,
        };

        if message.activate {
            let at = bars_for(&mut bars, &message.to);
            if let Some(entry) = bars.get_mut(at) {
                let depth = entry.open.len();
                entry.open.push((y, depth));
            }
        }
        if message.deactivate {
            if let Some(entry) = bars.iter_mut().find(|b| b.actor == message.from) {
                if let Some((top, depth)) = entry.open.pop() {
                    activations.push(Activation {
                        actor: message.from.clone(),
                        x: columns.centre(&message.from) - ACTIVATION_WIDTH / 2.0
                            + count(depth) * NESTING_OFFSET,
                        top,
                        bottom: y,
                        width: ACTIVATION_WIDTH,
                    });
                }
            }
        }

        y += if self_call {
            SELF_MESSAGE_HEIGHT + MESSAGE_ROW
        } else {
            MESSAGE_ROW
        };
        // Notes hang from the message's own row, not from the advanced cursor,
        // and a run of them can reach past it — then the next row gives way.
        let before = notes.len();
        let after = place_notes(columns, at, &placed, &mut notes);
        if notes.len() > before {
            y = y.max(after + MESSAGE_ROW / 2.0);
        }
        messages.push(placed);
    }

    // A bar nobody closed runs to just short of the last row.
    for entry in &bars {
        for (top, depth) in &entry.open {
            activations.push(Activation {
                actor: entry.actor.clone(),
                x: columns.centre(&entry.actor) - ACTIVATION_WIDTH / 2.0
                    + count(*depth) * NESTING_OFFSET,
                top: *top,
                bottom: y - MESSAGE_ROW / 2.0,
                width: ACTIVATION_WIDTH,
            });
        }
    }

    Rows {
        messages,
        activations,
        notes,
        bottom: y,
    }
}

/// How far a divider's rule sits above the message it introduces.
///
/// Far enough that its caption clears the message's own label — which only
/// needs the extra when the two share horizontal space, since the caption is
/// pinned to the block's left edge and the label is centred between lifelines.
fn divider_offset(left: f64, label: &str, message: Option<&PlacedMessage>) -> f64 {
    let Some(message) = message.filter(|m| !m.label.is_empty()) else {
        return DIVIDER_OFFSET;
    };
    if label.is_empty() {
        return DIVIDER_OFFSET;
    }
    let caption = text_width(&divider_label(label), EDGE_FONT, EDGE_WEIGHT);
    let caption_left = left + 8.0;
    let width = text_width(&message.label, EDGE_FONT, EDGE_WEIGHT);
    let label_left = if message.self_call {
        message.x1 + SELF_LABEL_PROBE
    } else {
        f64::midpoint(message.x1, message.x2) - width / 2.0
    };
    if caption_left + caption > label_left && caption_left < label_left + width {
        DIVIDER_OFFSET_CLEAR
    } else {
        DIVIDER_OFFSET
    }
}

/// How far left and right a block reaches: the boxes of every actor it involves.
fn block_span(columns: &Columns, block: &Block) -> (f64, f64) {
    let mut involved: Vec<usize> = Vec::new();
    for at in block.start_index..=block.end_index {
        let Some(message) = columns.diagram.messages.get(at) else {
            continue;
        };
        involved.push(columns.at(&message.from));
        involved.push(columns.at(&message.to));
    }
    if involved.is_empty() {
        involved.extend(0..columns.diagram.actors.len());
    }
    let first = involved.iter().copied().min().unwrap_or(0);
    let last = involved.iter().copied().max().unwrap_or(0);
    let left = columns.centres.get(first).copied().unwrap_or(0.0)
        - columns.width_at(first) / 2.0
        - BLOCK_PAD_X;
    let right = columns.centres.get(last).copied().unwrap_or(0.0)
        + columns.width_at(last) / 2.0
        + BLOCK_PAD_X;
    (left, right)
}

fn place_blocks(columns: &Columns, rows: &Rows) -> Vec<PlacedBlock> {
    columns
        .diagram
        .blocks
        .iter()
        .map(|block| {
            let row = |at: usize| rows.messages.get(at).map_or(rows.bottom, |m| m.y);
            let top = row(block.start_index) - BLOCK_PAD_TOP;
            let bottom = row(block.end_index) + BLOCK_PAD_BOTTOM + 12.0;
            let (left, right) = block_span(columns, block);
            PlacedBlock {
                kind: block.kind,
                label: block.label.clone(),
                at: Point::new(left, top),
                width: right - left,
                height: bottom - top,
                dividers: block
                    .dividers
                    .iter()
                    .map(|divider| PlacedDivider {
                        y: row(divider.index)
                            - divider_offset(
                                left,
                                &divider.label,
                                rows.messages.get(divider.index),
                            ),
                        label: divider.label.clone(),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// The leftmost and rightmost point anything reaches.
fn bounds(
    actors: &[PlacedActor],
    blocks: &[PlacedBlock],
    notes: &[PlacedNote],
    messages: &[PlacedMessage],
) -> (f64, f64) {
    let mut min_x = PADDING;
    let mut max_x: f64 = 0.0;
    for actor in actors {
        min_x = min_x.min(actor.x - actor.width / 2.0);
        max_x = max_x.max(actor.x + actor.width / 2.0);
    }
    for block in blocks {
        min_x = min_x.min(block.at.x);
        max_x = max_x.max(block.at.x + block.width);
    }
    for note in notes {
        min_x = min_x.min(note.at.x);
        max_x = max_x.max(note.at.x + note.width);
    }
    for message in messages {
        if message.self_call && !message.label.is_empty() {
            let left = message.x1 + SELF_LOOP_WIDTH + SELF_LABEL_PAD;
            let label = text_width(&message.label, EDGE_FONT, EDGE_WEIGHT);
            max_x = max_x.max(left + label + SELF_LABEL_PAD);
        }
    }
    (min_x, max_x)
}

/// Move everything right, so a note written left of the first actor is on the
/// page rather than off it.
fn shift(placed: &mut Placed, centres: &mut [f64], by: f64) {
    for actor in &mut placed.actors {
        actor.x += by;
    }
    for message in &mut placed.messages {
        message.x1 += by;
        message.x2 += by;
    }
    for activation in &mut placed.activations {
        activation.x += by;
    }
    for block in &mut placed.blocks {
        block.at.x += by;
    }
    for note in &mut placed.notes {
        note.at.x += by;
    }
    for centre in centres {
        *centre += by;
    }
}

/// Lay out a parsed sequence diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    if diagram.actors.is_empty() {
        return Placed::default();
    }
    let columns = Columns::of(diagram);
    let rows = stack(&columns);
    let blocks = place_blocks(&columns, &rows);
    let actors: Vec<PlacedActor> = diagram
        .actors
        .iter()
        .zip(&columns.centres)
        .zip(&columns.widths)
        .map(|((actor, x), width)| PlacedActor {
            id: actor.id.clone(),
            label: actor.label.clone(),
            kind: actor.kind,
            x: *x,
            y: PADDING,
            width: *width,
            height: ACTOR_HEIGHT,
        })
        .collect();

    let (min_x, max_x) = bounds(&actors, &blocks, &rows.notes, &rows.messages);
    let by = if min_x < PADDING {
        PADDING - min_x
    } else {
        0.0
    };
    let mut placed = Placed {
        width: 0.0,
        height: 0.0,
        actors,
        lifelines: Vec::new(),
        messages: rows.messages,
        activations: rows.activations,
        blocks,
        notes: rows.notes,
    };
    let mut centres = columns.centres.clone();
    if by > 0.0 {
        shift(&mut placed, &mut centres, by);
    }

    let bottom = rows.bottom + PADDING;
    placed.lifelines = diagram
        .actors
        .iter()
        .zip(&centres)
        .map(|(actor, x)| Lifeline {
            actor: actor.id.clone(),
            x: *x,
            top: PADDING + ACTOR_HEIGHT,
            bottom: bottom - PADDING,
        })
        .collect();
    placed.width = (max_x + by + PADDING).max(MIN_WIDTH);
    placed.height = bottom.max(MIN_HEIGHT);
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::metrics::{tab_label, tab_width, MIN_NOTE_WIDTH};
    use crate::sequence::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const FLOW: &str =
        "sequenceDiagram\nparticipant A as Alice\nparticipant B as Bob\nA->>B: Hello\nB-->>A: Hi";

    #[test]
    fn actors_stand_a_minimum_gap_apart() {
        let out = placed(FLOW);
        assert_eq!(out.actors.len(), 2);
        let gap = out.actors[1].x - out.actors[0].x;
        assert!((gap - ACTOR_GAP).abs() < 1e-9, "{gap}");
    }

    #[test]
    fn two_wide_boxes_are_pushed_further_apart_than_the_minimum() {
        let out = placed(&format!(
            "sequenceDiagram\nparticipant A as {0}\nparticipant B as {0}\nA->>B: x",
            "a wide name ".repeat(6)
        ));
        let gap = out.actors[1].x - out.actors[0].x;
        let touching = f64::midpoint(out.actors[0].width, out.actors[1].width);
        assert!((gap - (touching + 40.0)).abs() < 1e-9);
    }

    #[test]
    fn a_short_name_still_gets_a_minimum_box() {
        let out = placed("sequenceDiagram\nA->>B: x");
        assert!((out.actors[0].width - MIN_ACTOR_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn messages_stack_one_row_at_a_time() {
        let out = placed(FLOW);
        assert_eq!(out.messages.len(), 2);
        assert!((out.messages[1].y - out.messages[0].y - MESSAGE_ROW).abs() < 1e-9);
    }

    #[test]
    fn a_self_message_takes_the_room_its_loop_needs() {
        let out = placed("sequenceDiagram\nS->>S: one\nS->>S: two");
        let step = out.messages[1].y - out.messages[0].y;
        assert!((step - (MESSAGE_ROW + SELF_MESSAGE_HEIGHT)).abs() < 1e-9);
        assert!(out.messages[0].self_call);
    }

    #[test]
    fn a_lifeline_runs_from_its_box_to_the_last_row() {
        let out = placed(FLOW);
        assert_eq!(out.lifelines.len(), 2);
        assert!((out.lifelines[0].top - (PADDING + ACTOR_HEIGHT)).abs() < 1e-9);
        assert!((out.lifelines[0].bottom - (out.height - PADDING)).abs() < 1e-9);
    }

    #[test]
    fn a_block_header_pushes_its_first_message_down() {
        let plain = placed("sequenceDiagram\nA->>B: x\nA->>B: y");
        let wrapped = placed("sequenceDiagram\nA->>B: x\nloop L\nA->>B: y\nend");
        let lift = wrapped.messages[1].y - plain.messages[1].y;
        assert!((lift - BLOCK_HEADER_EXTRA).abs() < 1e-9);
    }

    #[test]
    fn a_divider_pushes_the_messages_after_it_down() {
        let flat = placed("sequenceDiagram\nalt A\nX->>Y: one\nX->>Y: two\nend");
        let split = placed("sequenceDiagram\nalt A\nX->>Y: one\nelse B\nX->>Y: two\nend");
        let step = split.messages[1].y - flat.messages[1].y;
        assert!((step - DIVIDER_EXTRA).abs() < 1e-9);
    }

    #[test]
    fn a_block_box_covers_the_rows_it_wraps_and_the_lifelines_it_touches() {
        let out = placed(
            "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\nloop L\nA->>B: x\nend",
        );
        assert_eq!(out.blocks.len(), 1);
        let block = &out.blocks[0];
        assert!(block.at.y < out.messages[0].y);
        assert!(block.at.y + block.height > out.messages[0].y);
        assert!(block.at.x < out.actors[0].x);
        assert!(block.at.x + block.width > out.actors[1].x);
        assert!(block.at.x + block.width < out.actors[2].x);
    }

    #[test]
    fn a_block_that_involves_nobody_spans_everyone() {
        let out =
            placed("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: x\nopt Nothing\nend");
        let block = &out.blocks[0];
        assert!(block.at.x < out.actors[0].x - out.actors[0].width / 2.0);
        assert!(block.at.x + block.width > out.actors[1].x + out.actors[1].width / 2.0);
    }

    #[test]
    fn a_divider_rule_sits_above_the_message_it_introduces() {
        let out = placed("sequenceDiagram\nalt A\nX->>Y: one\nelse B\nX->>Y: two\nend");
        let divider = &out.blocks[0].dividers[0];
        assert!(divider.y > out.messages[0].y);
        assert!(divider.y < out.messages[1].y);
        assert_eq!(divider_label(&divider.label), "[B]");
    }

    #[test]
    fn a_divider_caption_that_would_share_a_line_with_a_label_drops_further() {
        // A long caption reaches under a centred message label; a short one on a
        // wide diagram does not.
        let near = placed(
            "sequenceDiagram\nparticipant C as Client\nparticipant S as Server\nalt Valid\nS-->>C: 200 OK\nelse Account locked and then some\nS-->>C: 403 Forbidden\nend",
        );
        let far = placed(
            "sequenceDiagram\nparticipant C as Client\nparticipant S as Server\nalt Valid\nS-->>C: 200 OK\nelse x\nS-->>C: y\nend",
        );
        let drop = |out: &Placed| out.messages[1].y - out.blocks[0].dividers[0].y;
        assert!((drop(&near) - DIVIDER_OFFSET_CLEAR).abs() < 1e-9);
        assert!((drop(&far) - DIVIDER_OFFSET).abs() < 1e-9);
    }

    #[test]
    fn a_divider_with_no_message_after_it_keeps_the_plain_offset() {
        // The `else` opens past the last message, so there is no label to clear.
        let out = placed("sequenceDiagram\nalt A\nX->>Y: one\nelse B\nend");
        let block = &out.blocks[0];
        let drop = out.height - PADDING - block.dividers[0].y;
        assert!(drop > 0.0, "the rule is still on the page");
        assert!((block.dividers[0].y - (out.height - PADDING - DIVIDER_OFFSET)).abs() < 1e-9);
    }

    #[test]
    fn a_divider_with_no_caption_keeps_the_plain_offset() {
        let out = placed("sequenceDiagram\npar One\nA->>B: x\nand\nA->>C: y\nend");
        let drop = out.messages[1].y - out.blocks[0].dividers[0].y;
        assert!((drop - DIVIDER_OFFSET).abs() < 1e-9);
    }

    #[test]
    fn an_activation_runs_from_the_message_that_opened_it_to_the_one_that_closed_it() {
        let out = placed("sequenceDiagram\nC->>+S: Request\nS-->>-C: Response");
        assert_eq!(out.activations.len(), 1);
        let bar = &out.activations[0];
        assert_eq!(bar.actor, "S");
        assert!((bar.top - out.messages[0].y).abs() < 1e-9);
        assert!((bar.bottom - out.messages[1].y).abs() < 1e-9);
        assert!((bar.width - ACTIVATION_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn a_nested_activation_steps_to_the_right_of_the_one_it_sits_in() {
        let out = placed("sequenceDiagram\nC->>+S: one\nS->>+S: two\nS->>-S: three\nS-->>-C: four");
        assert_eq!(out.activations.len(), 2);
        let inner = out.activations.iter().find(|b| b.top > out.messages[0].y);
        let outer = out.activations.iter().find(|b| b.top <= out.messages[0].y);
        let (inner, outer) = (inner.expect("inner"), outer.expect("outer"));
        assert!((inner.x - outer.x - NESTING_OFFSET).abs() < 1e-9);
    }

    #[test]
    fn an_activation_nobody_closed_runs_to_just_short_of_the_last_row() {
        let out = placed("sequenceDiagram\nC->>+S: Request\nS-->>C: Response");
        assert_eq!(out.activations.len(), 1);
        let bar = &out.activations[0];
        assert!(bar.bottom > out.messages[1].y);
        assert!(bar.bottom < out.height - PADDING);
    }

    #[test]
    fn a_deactivation_with_nothing_open_draws_no_bar() {
        let out = placed("sequenceDiagram\nS-->>-C: Response");
        assert!(out.activations.is_empty());
    }

    #[test]
    fn a_note_hangs_below_the_message_it_follows() {
        let out = placed(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: x\nNote right of B: thinking",
        );
        assert_eq!(out.notes.len(), 1);
        let note = &out.notes[0];
        assert!(note.at.y > out.messages[0].y);
        assert_eq!(note.position, NotePosition::Right);
        assert!(note.at.x > out.actors[1].x);
    }

    #[test]
    fn a_note_left_of_the_first_actor_shifts_the_whole_drawing_right() {
        let plain = placed("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: x");
        let shifted = placed(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: x\nNote left of A: a long note that hangs off the left",
        );
        assert!(shifted.actors[0].x > plain.actors[0].x);
        assert!(shifted.notes[0].at.x >= PADDING - 1e-9);
        assert!((shifted.lifelines[0].x - shifted.actors[0].x).abs() < 1e-9);
    }

    #[test]
    fn a_note_over_two_actors_is_centred_between_them() {
        let out =
            placed("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: x\nNote over A,B: both");
        let note = &out.notes[0];
        let middle = f64::midpoint(out.actors[0].x, out.actors[1].x);
        assert!((note.at.x + note.width / 2.0 - middle).abs() < 1e-9);
    }

    #[test]
    fn a_note_over_one_actor_is_centred_on_it() {
        let out = placed("sequenceDiagram\nparticipant A\nA->>A: x\nNote over A: alone");
        let note = &out.notes[0];
        assert!((note.at.x + note.width / 2.0 - out.actors[0].x).abs() < 1e-9);
    }

    #[test]
    fn notes_on_one_message_stack_without_overlapping() {
        let out = placed(
            "sequenceDiagram\nparticipant R\nR->>R: work\nNote over R: one\nNote over R: two",
        );
        assert_eq!(out.notes.len(), 2);
        let step = out.notes[1].at.y - out.notes[0].at.y;
        assert!((step - (out.notes[0].height + NOTE_STACK_GAP)).abs() < 1e-9);
    }

    #[test]
    fn a_note_clears_the_loop_of_the_self_message_it_follows() {
        let out = placed("sequenceDiagram\nparticipant R\nR->>R: work\nNote over R: after");
        assert!(out.notes[0].at.y >= out.messages[0].y + SELF_MESSAGE_HEIGHT);
    }

    #[test]
    fn a_run_of_notes_pushes_the_next_message_clear_of_it() {
        let out = placed(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: one\nNote over A: n1\nNote over A: n2\nNote over A: n3\nA->>B: two",
        );
        let last = out.notes.last().expect("a note");
        assert!(out.messages[1].y > last.at.y + last.height);
    }

    #[test]
    fn a_note_before_the_first_message_is_never_placed() {
        let out = placed(
            "sequenceDiagram\nparticipant A\nparticipant B\nNote left of A: early\nA->>B: x",
        );
        assert!(out.notes.is_empty(), "the reference drops it too");
    }

    #[test]
    fn a_note_is_wide_enough_for_its_text_but_never_narrow() {
        assert!((note_width("x") - MIN_NOTE_WIDTH).abs() < 1e-9);
        assert!(note_width("a much longer note than that one") > MIN_NOTE_WIDTH);
    }

    #[test]
    fn a_self_message_label_widens_the_canvas() {
        let short = placed("sequenceDiagram\nS->>S: x");
        let long = placed("sequenceDiagram\nS->>S: a very much longer label than the other");
        assert!(long.width > short.width);
    }

    #[test]
    fn a_tab_reads_as_its_kind_and_its_label() {
        assert_eq!(tab_label(BlockKind::Loop, ""), "loop");
        assert_eq!(tab_label(BlockKind::Alt, "Valid"), "alt [Valid]");
        // A tab is one line, so only the first decides how wide it is.
        let short = tab_width(BlockKind::Loop, "a\nb");
        let long = tab_width(BlockKind::Loop, "a\nb but very much longer than that");
        assert!((short - long).abs() < 1e-9, "{short} vs {long}");
    }

    #[test]
    fn a_diagram_of_nothing_lays_out_to_nothing() {
        assert_eq!(placed("sequenceDiagram"), Placed::default());
    }

    #[test]
    fn a_lone_actor_still_gets_a_canvas_worth_drawing_on() {
        let out = placed("sequenceDiagram\nparticipant A");
        assert!(out.messages.is_empty());
        assert!((out.width - MIN_WIDTH).abs() < 1e-9);
        assert!(out.height >= MIN_HEIGHT);
    }
}
