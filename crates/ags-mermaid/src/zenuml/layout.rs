//! Where the lifelines, messages and control-flow boxes sit.
//!
//! A timeline rather than a graph: participants space out along the top, and
//! every message takes the next row down. The only thing that complicates the
//! vertical pass is that a control-flow box is not a row of its own — it reserves
//! space *above* the first message it wraps for its tab, and *below* the last one
//! for its floor, so the rows inside it drift down as blocks nest.

use std::collections::HashMap;

use crate::metrics::text_width;
use crate::round::count;
use crate::scene::Point;

use super::types::{ArrowHead, Diagram, Fragment, FragmentKind, LineStyle, MessageKind};

pub const PADDING: f64 = 30.0;
/// The least distance between two participants' centres.
pub const PARTICIPANT_GAP: f64 = 140.0;
pub const PARTICIPANT_HEIGHT: f64 = 40.0;
pub const PARTICIPANT_PAD_X: f64 = 16.0;
pub const MIN_PARTICIPANT_WIDTH: f64 = 80.0;
/// Between the participant boxes and the first message.
pub const HEADER_GAP: f64 = 24.0;
pub const MESSAGE_ROW: f64 = 42.0;
/// The extra a self-message takes, since it loops out and back.
pub const SELF_MESSAGE_HEIGHT: f64 = 30.0;
pub const SELF_LOOP_WIDTH: f64 = 30.0;
pub const SELF_LOOP_HEIGHT: f64 = 20.0;
pub const SELF_LABEL_PAD: f64 = 8.0;
/// Reserved above a fragment's first message for its tab.
pub const FRAGMENT_HEADER: f64 = 30.0;
/// Reserved above a section's first message for its divider.
pub const FRAGMENT_SECTION: f64 = 22.0;
/// Below a fragment's last message, before the box closes.
pub const FRAGMENT_BOTTOM_PAD: f64 = 12.0;
pub const FRAGMENT_PAD_X: f64 = 14.0;
/// The least a fragment box may be past its tab, so an empty one still reads as
/// a box rather than as a stray line.
pub const FRAGMENT_MIN_BODY: f64 = 10.0;
pub const TAB_HEIGHT: f64 = 18.0;
pub const TAB_PAD_X: f64 = 16.0;
pub const TAB_TEXT_PAD: f64 = 6.0;
pub const LABEL_FONT: f64 = 13.0;
pub const LABEL_WEIGHT: u32 = 500;
pub const EDGE_FONT: f64 = 11.0;
pub const EDGE_WEIGHT: u32 = 400;
pub const TAB_WEIGHT: u32 = 600;
/// The least a diagram may be, so a nearly empty one is still a drawing.
pub const MIN_WIDTH: f64 = 200.0;
pub const MIN_HEIGHT: f64 = 100.0;

/// One participant's box, placed about its centre.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedParticipant {
    pub id: String,
    pub label: String,
    pub annotator: Option<String>,
    /// The centre of the box, which is also the lifeline's x.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The dashed rule running down from a participant.
#[derive(Debug, Clone, PartialEq)]
pub struct Lifeline {
    pub participant: String,
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
    pub kind: MessageKind,
    pub line_style: LineStyle,
    pub arrow_head: ArrowHead,
    pub x1: f64,
    pub x2: f64,
    pub y: f64,
    pub self_call: bool,
}

/// A section divider inside a fragment box.
#[derive(Debug, Clone, PartialEq)]
pub struct Divider {
    pub y: f64,
    pub keyword: String,
    pub label: String,
}

/// One control-flow box.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedFragment {
    pub kind: FragmentKind,
    pub label: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub depth: usize,
    pub dividers: Vec<Divider>,
}

/// A laid-out `ZenUML` diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub participants: Vec<PlacedParticipant>,
    pub lifelines: Vec<Lifeline>,
    pub messages: Vec<PlacedMessage>,
    pub fragments: Vec<PlacedFragment>,
}

/// The words on a fragment's tab: its kind, and the condition when it has one.
pub fn tab_label(kind: FragmentKind, label: &str) -> String {
    if label.is_empty() {
        kind.token().to_string()
    } else {
        format!("{} [{label}]", kind.token())
    }
}

/// How wide that tab has to be.
pub fn tab_width(kind: FragmentKind, label: &str) -> f64 {
    text_width(&tab_label(kind, label), EDGE_FONT, TAB_WEIGHT) + TAB_PAD_X
}

/// The words on a divider: its keyword, and the condition when it has one.
pub fn divider_label(divider: &Divider) -> String {
    if divider.label.is_empty() {
        divider.keyword.clone()
    } else {
        format!("{} [{}]", divider.keyword, divider.label)
    }
}

/// Each participant box's width: enough for its name, or its stereotype.
fn participant_widths(diagram: &Diagram) -> Vec<f64> {
    diagram
        .participants
        .iter()
        .map(|participant| {
            let label = text_width(&participant.label, LABEL_FONT, LABEL_WEIGHT);
            let annotator = participant.annotator.as_ref().map_or(0.0, |annotator| {
                text_width(&format!("«{annotator}»"), EDGE_FONT, EDGE_WEIGHT)
            });
            (label.max(annotator) + PARTICIPANT_PAD_X * 2.0).max(MIN_PARTICIPANT_WIDTH)
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
            cursor += PARTICIPANT_GAP.max((previous + width) / 2.0 + 40.0);
        }
        out.push(cursor);
    }
    out
}

/// Which fragments open, close, or start a section at each message index.
struct Marks {
    opens: Vec<Vec<usize>>,
    closes: Vec<Vec<usize>>,
    sections: Vec<Vec<(usize, usize)>>,
}

impl Marks {
    /// Outermost fragments open first, so their tab sits above the nested ones,
    /// and close last, so their floor sits below.
    fn of(diagram: &Diagram) -> Self {
        let slots = diagram.messages.len() + 1;
        let mut marks = Self {
            opens: vec![Vec::new(); slots],
            closes: vec![Vec::new(); slots],
            sections: vec![Vec::new(); slots],
        };
        for (index, fragment) in diagram.fragments.iter().enumerate() {
            if let Some(slot) = marks.opens.get_mut(fragment.start_index) {
                slot.push(index);
            }
            if let Some(slot) = marks.closes.get_mut(fragment.end_index) {
                slot.push(index);
            }
            for (order, section) in fragment.sections.iter().enumerate() {
                if let Some(slot) = marks.sections.get_mut(section.index) {
                    slot.push((index, order));
                }
            }
        }
        let depth = |index: &usize| diagram.fragments.get(*index).map_or(0, |f| f.depth);
        for slot in &mut marks.opens {
            slot.sort_by_key(depth);
        }
        for slot in &mut marks.closes {
            slot.sort_by_key(|index| std::cmp::Reverse(depth(index)));
        }
        marks
    }

    fn at(rows: &[Vec<usize>], index: usize) -> &[usize] {
        rows.get(index).map_or(&[], Vec::as_slice)
    }
}

/// The vertical pass: every message's row, every box's extent.
struct Plan {
    messages: Vec<PlacedMessage>,
    /// Top and bottom of each fragment's box, by fragment index.
    spans: Vec<(f64, f64)>,
    /// Each fragment's divider rows, by fragment index then section order.
    section_y: Vec<Vec<Option<f64>>>,
    bottom: f64,
}

fn schedule(diagram: &Diagram, centre_of: &dyn Fn(&str) -> f64) -> Plan {
    let marks = Marks::of(diagram);
    let mut spans: Vec<Option<(f64, f64)>> = vec![None; diagram.fragments.len()];
    let mut section_y: Vec<Vec<Option<f64>>> = diagram
        .fragments
        .iter()
        .map(|fragment| vec![None; fragment.sections.len()])
        .collect();
    let mut y = PADDING + PARTICIPANT_HEIGHT + HEADER_GAP;
    let mut messages = Vec::with_capacity(diagram.messages.len());

    for (index, message) in diagram.messages.iter().enumerate() {
        for &fragment in Marks::at(&marks.opens, index) {
            if let Some(span) = spans.get_mut(fragment) {
                *span = Some((y, y));
            }
            y += FRAGMENT_HEADER;
        }
        for &(fragment, order) in marks.sections.get(index).map_or(&[][..], Vec::as_slice) {
            if let Some(slot) = section_y.get_mut(fragment).and_then(|s| s.get_mut(order)) {
                *slot = Some(y);
            }
            y += FRAGMENT_SECTION;
        }

        let self_call = message.from == message.to;
        messages.push(PlacedMessage {
            from: message.from.clone(),
            to: message.to.clone(),
            label: message.label.clone(),
            kind: message.kind,
            line_style: message.line_style,
            arrow_head: message.arrow_head,
            x1: centre_of(&message.from),
            x2: centre_of(&message.to),
            y,
            self_call,
        });
        y += if self_call {
            SELF_MESSAGE_HEIGHT + MESSAGE_ROW
        } else {
            MESSAGE_ROW
        };

        for &fragment in Marks::at(&marks.closes, index + 1) {
            y += FRAGMENT_BOTTOM_PAD;
            if let Some(Some(span)) = spans.get_mut(fragment) {
                span.1 = y;
            }
        }
    }

    let spans = settle(&mut y, spans);
    Plan {
        messages,
        spans,
        section_y,
        bottom: y + PADDING,
    }
}

/// Give a box that wrapped nothing somewhere to be, and a box that closed on the
/// row it opened enough height to read as one.
fn settle(y: &mut f64, spans: Vec<Option<(f64, f64)>>) -> Vec<(f64, f64)> {
    spans
        .into_iter()
        .map(|span| {
            let (top, mut bottom) = span.unwrap_or_else(|| {
                let placed = (*y, *y + FRAGMENT_HEADER);
                *y += FRAGMENT_HEADER + FRAGMENT_BOTTOM_PAD;
                placed
            });
            if bottom <= top + FRAGMENT_HEADER {
                bottom = top + FRAGMENT_HEADER + FRAGMENT_MIN_BODY;
            }
            (top, bottom)
        })
        .collect()
}

/// How far left and right a fragment reaches: the lifelines its messages touch.
fn span_x(
    diagram: &Diagram,
    centre_of: &dyn Fn(&str) -> f64,
    fragment: &Fragment,
    fallback: (f64, f64),
) -> (f64, f64) {
    let mut left = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    for index in fragment.start_index..fragment.end_index {
        let Some(message) = diagram.messages.get(index) else {
            continue;
        };
        let (a, b) = (centre_of(&message.from), centre_of(&message.to));
        left = left.min(a).min(b);
        right = right.max(a).max(b);
        if message.from == message.to {
            right = right.max(a + SELF_LOOP_WIDTH);
        }
    }
    if left.is_finite() {
        (left, right)
    } else {
        // A box that wrapped nothing has no lifeline of its own to follow, so it
        // spans them all.
        fallback
    }
}

fn fragment_boxes(
    diagram: &Diagram,
    centre_of: &dyn Fn(&str) -> f64,
    plan: &Plan,
) -> Vec<PlacedFragment> {
    let fallback = (
        centre_of_first(diagram, centre_of),
        centre_of_last(diagram, centre_of),
    );
    let mut out: Vec<PlacedFragment> = diagram
        .fragments
        .iter()
        .enumerate()
        .map(|(index, fragment)| {
            let (left, right) = span_x(diagram, centre_of, fragment, fallback);
            let pad = (FRAGMENT_PAD_X - count(fragment.depth) * 3.0).max(6.0);
            let (top, bottom) = plan.spans.get(index).copied().unwrap_or((0.0, 0.0));
            PlacedFragment {
                kind: fragment.kind,
                label: fragment.label.clone(),
                at: Point::new(left - pad, top),
                width: (right + pad) - (left - pad),
                height: bottom - top,
                depth: fragment.depth,
                dividers: fragment
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(order, section)| Divider {
                        y: plan
                            .section_y
                            .get(index)
                            .and_then(|rows| rows.get(order).copied().flatten())
                            .unwrap_or(top),
                        keyword: section.keyword.clone(),
                        label: section.label.clone(),
                    })
                    .collect(),
            }
        })
        .collect();
    // Outermost first, so a nested box paints over the one around it.
    out.sort_by_key(|fragment| fragment.depth);
    out
}

fn centre_of_first(diagram: &Diagram, centre_of: &dyn Fn(&str) -> f64) -> f64 {
    diagram
        .participants
        .first()
        .map_or(0.0, |p| centre_of(&p.id))
}

fn centre_of_last(diagram: &Diagram, centre_of: &dyn Fn(&str) -> f64) -> f64 {
    diagram
        .participants
        .last()
        .map_or(0.0, |p| centre_of(&p.id))
}

/// How far right anything reaches — a box, a self-message's label, a tab.
fn extent(participants: &[PlacedParticipant], plan: &Plan, fragments: &[PlacedFragment]) -> f64 {
    let mut max_x: f64 = 0.0;
    for participant in participants {
        max_x = max_x.max(participant.x + participant.width / 2.0);
    }
    for message in &plan.messages {
        if message.self_call && !message.label.is_empty() {
            let label = text_width(&message.label, EDGE_FONT, EDGE_WEIGHT);
            max_x =
                max_x.max(message.x1 + SELF_LOOP_WIDTH + SELF_LABEL_PAD + label + SELF_LABEL_PAD);
        }
    }
    for fragment in fragments {
        max_x = max_x.max(fragment.at.x + fragment.width);
        max_x = max_x.max(fragment.at.x + tab_width(fragment.kind, &fragment.label));
    }
    max_x
}

/// Lay out a parsed `ZenUML` diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    if diagram.participants.is_empty() {
        return Placed::default();
    }
    let widths = participant_widths(diagram);
    let centres = centres(&widths);
    let index_by_id: HashMap<&str, usize> = diagram
        .participants
        .iter()
        .enumerate()
        .map(|(index, participant)| (participant.id.as_str(), index))
        .collect();
    // An unknown participant falls back on the first lifeline, which is where the
    // reference puts one too.
    let centre_of = |id: &str| -> f64 {
        centres
            .get(index_by_id.get(id).copied().unwrap_or(0))
            .copied()
            .unwrap_or(0.0)
    };

    let participants: Vec<PlacedParticipant> = diagram
        .participants
        .iter()
        .zip(&centres)
        .zip(&widths)
        .map(|((participant, x), width)| PlacedParticipant {
            id: participant.id.clone(),
            label: participant.label.clone(),
            annotator: participant.annotator.clone(),
            x: *x,
            y: PADDING,
            width: *width,
            height: PARTICIPANT_HEIGHT,
        })
        .collect();

    let plan = schedule(diagram, &centre_of);
    let fragments = fragment_boxes(diagram, &centre_of, &plan);
    let lifelines = participants
        .iter()
        .map(|participant| Lifeline {
            participant: participant.id.clone(),
            x: participant.x,
            top: PADDING + PARTICIPANT_HEIGHT,
            bottom: plan.bottom - PADDING,
        })
        .collect();

    Placed {
        width: (extent(&participants, &plan, &fragments) + PADDING).max(MIN_WIDTH),
        height: plan.bottom.max(MIN_HEIGHT),
        participants,
        lifelines,
        messages: plan.messages,
        fragments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zenuml::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const FLOW: &str = "zenuml\nAlice->Bob: Request\nBob.process()\nBob->Alice: Response";

    #[test]
    fn participants_stand_a_minimum_gap_apart() {
        let out = placed(FLOW);
        assert_eq!(out.participants.len(), 2);
        let gap = out.participants[1].x - out.participants[0].x;
        assert!((gap - PARTICIPANT_GAP).abs() < 1e-9, "{gap}");
    }

    #[test]
    fn two_wide_boxes_are_pushed_further_apart_than_the_minimum() {
        let out = placed(&format!(
            "zenuml\nparticipant A as {0}\nparticipant B as {0}\nA->B: x",
            "wide name ".repeat(6)
        ));
        let gap = out.participants[1].x - out.participants[0].x;
        assert!(gap > PARTICIPANT_GAP, "{gap}");
        let touching = f64::midpoint(out.participants[0].width, out.participants[1].width);
        assert!((gap - (touching + 40.0)).abs() < 1e-9);
    }

    #[test]
    fn a_stereotype_can_be_what_makes_a_box_wide() {
        let narrow = placed("zenuml\nA\nA->A: x");
        let wide = placed("zenuml\n@VeryLongStereotypeName A\nA->A: x");
        assert!(wide.participants[0].width > narrow.participants[0].width);
    }

    #[test]
    fn a_short_name_still_gets_a_minimum_box() {
        let out = placed("zenuml\nA\nA->A: x");
        assert!((out.participants[0].width - MIN_PARTICIPANT_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn messages_stack_one_row_at_a_time() {
        let out = placed(FLOW);
        assert_eq!(out.messages.len(), 4, "the call brings its own reply");
        for pair in out.messages.windows(2) {
            assert!((pair[1].y - pair[0].y - MESSAGE_ROW).abs() < 1e-9);
        }
    }

    #[test]
    fn a_self_message_takes_the_room_its_loop_needs() {
        let out = placed("zenuml\nA\nA->A: think\nA->A: again");
        let step = out.messages[1].y - out.messages[0].y;
        assert!((step - (MESSAGE_ROW + SELF_MESSAGE_HEIGHT)).abs() < 1e-9);
        assert!(out.messages[0].self_call);
        assert!((out.messages[0].x1 - out.messages[0].x2).abs() < 1e-9);
    }

    #[test]
    fn a_message_runs_between_the_two_lifelines_it_names() {
        let out = placed(FLOW);
        let alice = out.participants[0].x;
        let bob = out.participants[1].x;
        assert!((out.messages[0].x1 - alice).abs() < 1e-9);
        assert!((out.messages[0].x2 - bob).abs() < 1e-9);
    }

    #[test]
    fn a_lifeline_runs_from_its_box_to_the_last_row() {
        let out = placed(FLOW);
        assert_eq!(out.lifelines.len(), 2);
        assert!((out.lifelines[0].top - (PADDING + PARTICIPANT_HEIGHT)).abs() < 1e-9);
        assert!((out.lifelines[0].bottom - (out.height - PADDING)).abs() < 1e-9);
        assert!(out.lifelines[0].bottom > out.messages[0].y);
    }

    #[test]
    fn a_fragment_reserves_a_tab_above_its_first_message() {
        let plain = placed("zenuml\nA\nB\nA->B: x");
        let wrapped = placed("zenuml\nA\nB\nloop (n) {\nA->B: x\n}");
        let lift = wrapped.messages[0].y - plain.messages[0].y;
        assert!((lift - FRAGMENT_HEADER).abs() < 1e-9);
    }

    #[test]
    fn a_fragment_box_covers_the_rows_it_wraps() {
        let out = placed("zenuml\nA\nB\nloop (n) {\nA->B: x\n}");
        let fragment = &out.fragments[0];
        assert!(fragment.at.y < out.messages[0].y);
        assert!(fragment.at.y + fragment.height > out.messages[0].y);
        assert_eq!(fragment.kind, FragmentKind::Loop);
        assert_eq!(fragment.label, "n");
    }

    #[test]
    fn a_fragment_spans_the_lifelines_its_messages_touch_and_no_more() {
        let out = placed("zenuml\nA\nB\nC\nalt (ok) {\nA->B: x\n}");
        let fragment = &out.fragments[0];
        let (a, b, c) = (
            out.participants[0].x,
            out.participants[1].x,
            out.participants[2].x,
        );
        assert!(fragment.at.x < a);
        assert!(fragment.at.x + fragment.width > b);
        assert!(fragment.at.x + fragment.width < c);
    }

    #[test]
    fn a_nested_box_is_inset_and_painted_after_the_one_around_it() {
        let out = placed("zenuml\nA\nB\nloop (n) {\nalt (ok) {\nA->B: x\n}\n}");
        assert_eq!(out.fragments.len(), 2);
        let depths: Vec<usize> = out.fragments.iter().map(|f| f.depth).collect();
        assert_eq!(depths, [0, 1], "outermost first");
        assert!(out.fragments[1].at.x > out.fragments[0].at.x);
        assert!(out.fragments[1].at.y > out.fragments[0].at.y);
    }

    #[test]
    fn a_section_divider_sits_between_the_rows_it_separates() {
        let out = placed("zenuml\nA\nB\nalt (ok) {\nA->B: yes\n} else {\nA->B: no\n}");
        let fragment = &out.fragments[0];
        assert_eq!(fragment.dividers.len(), 1);
        let divider = &fragment.dividers[0];
        assert!(divider.y > out.messages[0].y);
        assert!(divider.y < out.messages[1].y);
        assert_eq!(divider_label(divider), "else");
    }

    #[test]
    fn a_section_pushes_the_rows_below_it_down() {
        let flat = placed("zenuml\nA\nB\nalt (ok) {\nA->B: yes\nA->B: no\n}");
        let split = placed("zenuml\nA\nB\nalt (ok) {\nA->B: yes\n} else {\nA->B: no\n}");
        let step = split.messages[1].y - flat.messages[1].y;
        assert!((step - FRAGMENT_SECTION).abs() < 1e-9);
    }

    #[test]
    fn a_box_that_wrapped_nothing_still_gets_room_and_spans_everyone() {
        let out = placed("zenuml\nA\nB\nA->B: x\nopt (never) {\n}");
        assert_eq!(out.fragments.len(), 1);
        let fragment = &out.fragments[0];
        assert!(fragment.height >= FRAGMENT_HEADER + FRAGMENT_MIN_BODY);
        assert!(fragment.at.y > out.messages[0].y);
        assert!(fragment.at.x < out.participants[0].x);
        assert!(fragment.at.x + fragment.width > out.participants[1].x);
    }

    #[test]
    fn a_divider_with_no_row_of_its_own_falls_back_to_the_top_of_its_box() {
        // The `else` opens past the last message, so nothing places its row.
        let out = placed("zenuml\nA\nB\nalt (ok) {\nA->B: yes\n} else {\n}");
        let fragment = &out.fragments[0];
        assert_eq!(fragment.dividers.len(), 1);
        assert!((fragment.dividers[0].y - fragment.at.y).abs() < 1e-9);
    }

    #[test]
    fn a_self_message_label_widens_the_canvas() {
        let short = placed("zenuml\nA\nA->A: x");
        let long = placed("zenuml\nA\nA->A: a very much longer label than the other one");
        assert!(long.width > short.width);
    }

    #[test]
    fn a_long_tab_widens_the_canvas() {
        let out =
            placed("zenuml\nA\nB\nalt (a condition long enough to run past the box) {\nA->B: x\n}");
        let fragment = &out.fragments[0];
        let tab = tab_width(fragment.kind, &fragment.label);
        assert!(tab > fragment.width, "the tab overhangs its box");
        assert!(out.width >= fragment.at.x + tab + PADDING);
    }

    #[test]
    fn a_tab_reads_as_its_kind_and_its_condition() {
        assert_eq!(tab_label(FragmentKind::Loop, ""), "loop");
        assert_eq!(tab_label(FragmentKind::Alt, "ok"), "alt [ok]");
        assert_eq!(
            divider_label(&Divider {
                y: 0.0,
                keyword: "catch".into(),
                label: "e".into(),
            }),
            "catch [e]"
        );
    }

    #[test]
    fn a_diagram_of_nothing_lays_out_to_nothing() {
        assert_eq!(placed("zenuml"), Placed::default());
    }

    #[test]
    fn a_lone_participant_still_gets_a_canvas_worth_drawing_on() {
        let out = placed("zenuml\nA");
        assert!(out.messages.is_empty());
        assert!(
            (out.width - MIN_WIDTH).abs() < 1e-9,
            "one narrow box leaves the floor to decide"
        );
        assert!(out.height >= MIN_HEIGHT);
    }
}
